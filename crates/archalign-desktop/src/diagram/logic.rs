//! Pure diagram logic: reach, legend, finder, badges and text for the strip and tooltip.

use std::collections::{BTreeSet, HashMap, VecDeque};

use archalign_model::{ArchitectureDoc, ComponentType, Issue, IssueCode, LegendMode, SourceRef};

/// Reach direction (FR-DIA-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Follow connections backwards (`U`).
    Upstream,
    /// Follow connections forwards (`D`).
    Downstream,
}

/// What a reach highlight covers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReachSet {
    /// Component indices, including the start.
    pub nodes: BTreeSet<usize>,
    /// Connection indices traversed.
    pub edges: BTreeSet<usize>,
}

/// Component index by ID (first occurrence wins).
pub fn index_by_id(doc: &ArchitectureDoc) -> HashMap<&str, usize> {
    let mut m = HashMap::new();
    for (i, c) in doc.components.iter().enumerate() {
        m.entry(c.id.as_str()).or_insert(i);
    }
    m
}

/// Everything transitively reachable from `start` along authored directed connections.
/// Breadth-first; cycles terminate because visited nodes are skipped.
pub fn reach(doc: &ArchitectureDoc, start: usize, dir: Direction) -> ReachSet {
    let ids = index_by_id(doc);
    let edges: Vec<(usize, usize, usize)> = doc
        .connections()
        .iter()
        .enumerate()
        .filter_map(|(k, c)| {
            let a = *ids.get(c.from.as_str())?;
            let b = *ids.get(c.to.as_str())?;
            Some(match dir {
                Direction::Downstream => (k, a, b),
                Direction::Upstream => (k, b, a),
            })
        })
        .collect();
    let mut out = ReachSet::default();
    if start >= doc.components.len() {
        return out;
    }
    out.nodes.insert(start);
    let mut queue = VecDeque::from([start]);
    while let Some(n) = queue.pop_front() {
        for &(k, from, to) in &edges {
            if from == n {
                out.edges.insert(k);
                if out.nodes.insert(to) {
                    queue.push_back(to);
                }
            }
        }
    }
    out
}

/// One legend row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendRow {
    /// Component type.
    pub ty: ComponentType,
    /// Text.
    pub label: String,
}

/// Legend rows per `meta.legend` (FR-DIA-01): `auto` shows the types present, `all` shows
/// all seven, `hidden` none; `entries.<type>.label` overrides the text and `visible: false`
/// hides the row.
pub fn legend(doc: &ArchitectureDoc) -> Vec<LegendRow> {
    let legend = doc.meta.legend.clone().unwrap_or_default();
    let mode = legend.mode.unwrap_or_default();
    let present: BTreeSet<ComponentType> = doc.components.iter().map(|c| c.ty).collect();
    ComponentType::ALL
        .into_iter()
        .filter(|ty| match mode {
            LegendMode::Hidden => false,
            LegendMode::All => true,
            LegendMode::Auto => present.contains(ty),
        })
        .filter_map(|ty| {
            let entry = legend.entries.as_ref().and_then(|e| e.get(ty));
            if entry.and_then(|e| e.visible) == Some(false) {
                return None;
            }
            let label = entry
                .and_then(|e| e.label.clone())
                .unwrap_or_else(|| ty.default_label().to_string());
            Some(LegendRow { ty, label })
        })
        .collect()
}

/// Maximum finder results (FR-DIA-07).
pub const FINDER_LIMIT: usize = 10;

/// Components whose label or ID contains `query` (case-insensitive), in document order.
pub fn find_nodes(doc: &ArchitectureDoc, query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    doc.components
        .iter()
        .enumerate()
        .filter(|(_, c)| c.label.to_lowercase().contains(&q) || c.id.to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .take(FINDER_LIMIT)
        .collect()
}

/// Per-node problem badges (FR-DIA-09).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Badges {
    /// A citation is broken.
    pub broken: bool,
    /// A cited file has uncommitted changes.
    pub modified: bool,
}

/// The component index a pointer like `/components/3/...` refers to.
pub fn component_of(pointer: &str) -> Option<usize> {
    let mut parts = pointer.split('/').skip(1);
    match (parts.next(), parts.next()) {
        (Some("components"), Some(i)) => i.parse().ok(),
        _ => None,
    }
}

/// Badges from the diagram's warnings.
pub fn badges(doc: &ArchitectureDoc, warnings: &[Issue]) -> Vec<Badges> {
    let mut out = vec![Badges::default(); doc.components.len()];
    for w in warnings {
        let Some(b) = component_of(&w.pointer).and_then(|i| out.get_mut(i)) else {
            continue;
        };
        match w.code {
            IssueCode::BrokenCitation | IssueCode::InvalidCitationPath => b.broken = true,
            IssueCode::ModifiedCitation => b.modified = true,
            _ => {}
        }
    }
    out
}

/// `path:line–end_line` for a citation.
pub fn source_label(s: &SourceRef) -> String {
    match (s.line, s.end_line) {
        (Some(l), Some(e)) if e != l => format!("{}:{l}–{e}", s.path),
        (Some(l), _) => format!("{}:{l}", s.path),
        (None, _) => s.path.clone(),
    }
}

/// Tooltip lines for a node (FR-DIA-04): label, sublabel, tag, then the sources.
pub fn tooltip_lines(doc: &ArchitectureDoc, node: usize) -> Vec<String> {
    let Some(c) = doc.components.get(node) else {
        return Vec::new();
    };
    let mut lines = vec![c.label.clone()];
    lines.extend(c.sublabel.clone().filter(|s| !s.is_empty()));
    lines.extend(
        c.tag
            .clone()
            .filter(|s| !s.is_empty())
            .map(|t| format!("[{t}]")),
    );
    if c.sources().is_empty() {
        lines.push("No source".into());
    }
    lines.extend(c.sources().iter().map(source_label));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc(v: serde_json::Value) -> ArchitectureDoc {
        serde_json::from_value(v).unwrap()
    }

    fn graph() -> ArchitectureDoc {
        doc(json!({
            "schema_version": 1, "diagram_type": "architecture",
            "meta": { "title": "t", "output": "t.html" },
            "components": [
                { "id": "a", "type": "frontend", "label": "Alpha" },
                { "id": "b", "type": "backend", "label": "Beta" },
                { "id": "c", "type": "backend", "label": "Gamma" },
                { "id": "d", "type": "database", "label": "Delta" },
                { "id": "e", "type": "external", "label": "Epsilon" }
            ],
            "connections": [
                { "from": "a", "to": "b" },
                { "from": "b", "to": "c" },
                { "from": "c", "to": "b" },
                { "from": "c", "to": "d" },
                { "from": "e", "to": "a" }
            ]
        }))
    }

    #[test]
    fn downstream_reach_follows_edges_and_handles_cycles() {
        let r = reach(&graph(), 0, Direction::Downstream);
        assert_eq!(r.nodes, BTreeSet::from([0, 1, 2, 3]));
        assert_eq!(r.edges, BTreeSet::from([0, 1, 2, 3]));
    }

    #[test]
    fn upstream_reach_follows_edges_backwards() {
        let r = reach(&graph(), 1, Direction::Upstream);
        assert_eq!(r.nodes, BTreeSet::from([0, 1, 2, 4]));
        assert_eq!(r.edges, BTreeSet::from([0, 1, 2, 4]));
        let r = reach(&graph(), 4, Direction::Upstream);
        assert_eq!(r.nodes, BTreeSet::from([4]));
        assert!(r.edges.is_empty());
    }

    #[test]
    fn legend_modes_and_overrides() {
        let mut d = graph();
        let types = |d: &ArchitectureDoc| legend(d).into_iter().map(|r| r.ty).collect::<Vec<_>>();
        assert_eq!(
            types(&d),
            [
                ComponentType::Frontend,
                ComponentType::Backend,
                ComponentType::Database,
                ComponentType::External
            ]
        );
        d.meta.legend = Some(serde_json::from_value(json!({ "mode": "all" })).unwrap());
        assert_eq!(types(&d).len(), 7);
        d.meta.legend = Some(serde_json::from_value(json!({ "mode": "hidden" })).unwrap());
        assert!(types(&d).is_empty());
        d.meta.legend = Some(
            serde_json::from_value(json!({
                "entries": { "backend": { "label": "Services" }, "external": { "visible": false } }
            }))
            .unwrap(),
        );
        let rows = legend(&d);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].label, "Services");
        assert_eq!(rows[0].label, "Frontend");
        assert!(rows.iter().all(|r| r.ty != ComponentType::External));
    }

    #[test]
    fn finder_matches_labels_and_ids_case_insensitively_up_to_ten() {
        let d = graph();
        assert_eq!(find_nodes(&d, "ETA"), vec![1]);
        assert_eq!(find_nodes(&d, "lTa"), vec![3]);
        assert_eq!(find_nodes(&d, "E"), vec![1, 3, 4], "IDs match too (e)");
        assert_eq!(find_nodes(&d, "e"), vec![1, 3, 4]);
        assert!(find_nodes(&d, "  ").is_empty());
        let many = doc(json!({
            "schema_version": 1, "diagram_type": "architecture",
            "meta": { "title": "t", "output": "t.html" },
            "components": (0..15).map(|i| json!({ "id": format!("n{i}"), "type": "backend", "label": "Node" })).collect::<Vec<_>>()
        }));
        assert_eq!(find_nodes(&many, "node").len(), FINDER_LIMIT);
    }

    #[test]
    fn badges_come_from_citation_warnings() {
        let d = graph();
        let w = vec![
            Issue::warning(
                IssueCode::BrokenCitation,
                "/components/2/sources/0/line",
                "x",
            ),
            Issue::warning(IssueCode::ModifiedCitation, "/components/3/sources/0", "x"),
            Issue::warning(IssueCode::MissingSources, "/components/4", "x"),
        ];
        let b = badges(&d, &w);
        assert!(b[2].broken && !b[2].modified);
        assert!(b[3].modified && !b[3].broken);
        assert_eq!(b[4], Badges::default());
    }

    #[test]
    fn source_labels() {
        let s = |line: Option<u64>, end: Option<u64>| SourceRef {
            path: "a.rs".into(),
            line,
            end_line: end,
            label: None,
        };
        assert_eq!(source_label(&s(Some(3), Some(9))), "a.rs:3–9");
        assert_eq!(source_label(&s(Some(3), None)), "a.rs:3");
        assert_eq!(source_label(&s(None, None)), "a.rs");
    }
}
