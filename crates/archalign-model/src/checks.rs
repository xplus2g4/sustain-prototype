//! Semantic checks that JSON Schema cannot express (FR-VAL-04, FR-VAL-05 step 4).

use std::collections::hash_map::Entry;
use std::collections::{BTreeSet, HashMap, HashSet};

use crate::issue::{Issue, IssueCode, Severity};
use crate::types::ArchitectureDoc;

/// Validation strictness (ADR 0027).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Opening files: reference and ID problems are errors; the rest are warnings.
    Import,
    /// Agent output: every rule is an error.
    Submission,
}

impl Profile {
    fn soft(self) -> Severity {
        match self {
            Profile::Import => Severity::Warning,
            Profile::Submission => Severity::Error,
        }
    }
}

fn issue(severity: Severity, code: IssueCode, pointer: String, message: String) -> Issue {
    Issue {
        severity,
        code,
        pointer,
        message,
    }
}

/// How two boundaries relate, by their member sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryRelation {
    /// No shared members.
    Disjoint,
    /// The first strictly contains the second.
    Contains,
    /// The second strictly contains the first.
    ContainedBy,
    /// Same member set.
    Equal,
    /// Shared members, neither contains the other.
    PartialOverlap,
}

/// Compares two member sets.
pub fn boundary_relation(a: &BTreeSet<&str>, b: &BTreeSet<&str>) -> BoundaryRelation {
    if a.is_disjoint(b) {
        BoundaryRelation::Disjoint
    } else if a == b {
        BoundaryRelation::Equal
    } else if a.is_superset(b) {
        BoundaryRelation::Contains
    } else if b.is_superset(a) {
        BoundaryRelation::ContainedBy
    } else {
        BoundaryRelation::PartialOverlap
    }
}

/// Runs the semantic checks. Under [`Profile::Import`], missing sources and boundary
/// overlap are warnings; under [`Profile::Submission`] everything is an error.
pub fn semantic(doc: &ArchitectureDoc, profile: Profile) -> Vec<Issue> {
    let mut out = Vec::new();
    let soft = profile.soft();

    // Unique component IDs.
    let mut first_seen: HashMap<&str, usize> = HashMap::new();
    for (i, c) in doc.components.iter().enumerate() {
        match first_seen.entry(c.id.as_str()) {
            Entry::Occupied(prev) => out.push(issue(
                Severity::Error,
                IssueCode::DuplicateId,
                format!("/components/{i}/id"),
                format!(
                    "Duplicate component id `{}` (first used at /components/{})",
                    c.id,
                    prev.get()
                ),
            )),
            Entry::Vacant(slot) => {
                slot.insert(i);
            }
        }
    }
    let ids: HashSet<&str> = doc.components.iter().map(|c| c.id.as_str()).collect();

    // Unique connection IDs, where present; references resolve.
    let mut conn_ids: HashMap<&str, usize> = HashMap::new();
    for (i, conn) in doc.connections().iter().enumerate() {
        if let Some(id) = conn.id.as_deref() {
            match conn_ids.entry(id) {
                Entry::Occupied(prev) => out.push(issue(
                    Severity::Error,
                    IssueCode::DuplicateId,
                    format!("/connections/{i}/id"),
                    format!(
                        "Duplicate connection id `{id}` (first used at /connections/{})",
                        prev.get()
                    ),
                )),
                Entry::Vacant(slot) => {
                    slot.insert(i);
                }
            }
        }
        for (field, target) in [("from", &conn.from), ("to", &conn.to)] {
            if !ids.contains(target.as_str()) {
                out.push(issue(
                    Severity::Error,
                    IssueCode::UnknownReference,
                    format!("/connections/{i}/{field}"),
                    format!("Connection `{field}` names unknown component `{target}`"),
                ));
            }
        }
    }
    for (i, b) in doc.boundaries().iter().enumerate() {
        for (j, w) in b.wraps.iter().enumerate() {
            if !ids.contains(w.as_str()) {
                out.push(issue(
                    Severity::Error,
                    IssueCode::UnknownReference,
                    format!("/boundaries/{i}/wraps/{j}"),
                    format!("Boundary `{}` wraps unknown component `{w}`", b.label),
                ));
            }
        }
    }

    // 1–3 sources per component.
    for (i, c) in doc.components.iter().enumerate() {
        let n = c.sources().len();
        if n == 0 {
            out.push(issue(
                soft,
                IssueCode::MissingSources,
                format!("/components/{i}"),
                format!("Component `{}` has no sources", c.id),
            ));
        } else if n > 3 {
            out.push(issue(
                Severity::Error,
                IssueCode::Schema,
                format!("/components/{i}/sources"),
                format!(
                    "Component `{}` has {n} sources; at most 3 are allowed",
                    c.id
                ),
            ));
        }
    }

    // Boundaries are separate or strictly nested.
    let sets: Vec<BTreeSet<&str>> = doc
        .boundaries()
        .iter()
        .map(|b| b.wraps.iter().map(String::as_str).collect())
        .collect();
    for i in 0..sets.len() {
        for j in (i + 1)..sets.len() {
            let rel = boundary_relation(&sets[i], &sets[j]);
            let (li, lj) = (&doc.boundaries()[i].label, &doc.boundaries()[j].label);
            let msg = match rel {
                BoundaryRelation::PartialOverlap => format!(
                    "Boundaries `{li}` (/boundaries/{i}) and `{lj}` share components but neither contains the other"
                ),
                BoundaryRelation::Equal => format!(
                    "Boundaries `{li}` (/boundaries/{i}) and `{lj}` wrap the same components; nesting must be strict"
                ),
                _ => continue,
            };
            out.push(issue(
                soft,
                IssueCode::BoundaryOverlap,
                format!("/boundaries/{j}"),
                msg,
            ));
        }
    }
    out
}
