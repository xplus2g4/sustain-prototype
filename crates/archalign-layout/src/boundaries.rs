//! Boundary nesting and rectangles (FR-LAY-06).

use std::collections::{BTreeSet, HashMap};

use archalign_model::ArchitectureDoc;

use crate::geom::{Rect, bounding_box};

/// Padding on the left, top and right without `pad` (Archify).
pub const PAD_SIDE: f64 = 30.0;
/// Padding at the bottom without `pad` (Archify).
pub const PAD_BOTTOM: f64 = 50.0;

/// The nesting forest of a document's boundaries.
#[derive(Debug, Clone, PartialEq)]
pub struct Nesting {
    /// Member component indices of each boundary (deduplicated; unknown IDs dropped).
    pub members: Vec<BTreeSet<usize>>,
    /// The enclosing boundary, if any. Boundaries without members have none.
    pub parent: Vec<Option<usize>>,
    /// 0 for top-level boundaries.
    pub depth: Vec<usize>,
    /// The innermost boundary of each component.
    pub innermost: Vec<Option<usize>>,
}

/// Builds the nesting forest. A boundary's parent is the smallest other boundary whose
/// members include all of its members; among equal member sets, the earlier boundary is the
/// parent of the later one. Partially overlapping boundaries are not nested.
pub fn nesting(doc: &ArchitectureDoc) -> Nesting {
    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, c) in doc.components.iter().enumerate() {
        index.entry(c.id.as_str()).or_insert(i);
    }
    let members: Vec<BTreeSet<usize>> = doc
        .boundaries()
        .iter()
        .map(|b| {
            b.wraps
                .iter()
                .filter_map(|id| index.get(id.as_str()).copied())
                .collect()
        })
        .collect();
    let n = members.len();
    let mut parent = vec![None; n];
    for b in 0..n {
        if members[b].is_empty() {
            continue;
        }
        let mut best: Option<usize> = None;
        for a in 0..n {
            if a == b || members[a].is_empty() || !members[a].is_superset(&members[b]) {
                continue;
            }
            let equal = members[a].len() == members[b].len();
            if equal && a > b {
                continue;
            }
            best = match best {
                None => Some(a),
                // Smallest set wins; among equal sizes the later boundary is the deeper one.
                Some(cur)
                    if (members[a].len(), std::cmp::Reverse(a))
                        < (members[cur].len(), std::cmp::Reverse(cur)) =>
                {
                    Some(a)
                }
                keep => keep,
            };
        }
        parent[b] = best;
    }
    let mut depth = vec![0; n];
    for (b, d) in depth.iter_mut().enumerate() {
        let mut cur = parent[b];
        // Parents always have more members or a lower index, so the chain is acyclic and
        // shorter than `n`; the bound is a belt-and-braces guard.
        while let Some(p) = cur {
            *d += 1;
            if *d > n {
                break;
            }
            cur = parent[p];
        }
    }
    let mut innermost = vec![None; doc.components.len()];
    for (c, slot) in innermost.iter_mut().enumerate() {
        *slot = (0..n)
            .filter(|&b| members[b].contains(&c))
            .min_by_key(|&b| {
                (
                    members[b].len(),
                    std::cmp::Reverse(depth[b]),
                    std::cmp::Reverse(b),
                )
            });
    }
    Nesting {
        members,
        parent,
        depth,
        innermost,
    }
}

/// The padding of a boundary: `(left, top, right, bottom)`.
pub fn padding(pad: Option<f64>) -> (f64, f64, f64, f64) {
    match pad {
        Some(p) => (p, p, p, p),
        None => (PAD_SIDE, PAD_SIDE, PAD_SIDE, PAD_BOTTOM),
    }
}

/// Computes boundary rectangles from component rectangles: the bounding box of the members
/// and nested boundaries, plus padding. Boundaries without members get `None`.
pub fn boundary_rects(doc: &ArchitectureDoc, nest: &Nesting, nodes: &[Rect]) -> Vec<Option<Rect>> {
    let n = nest.members.len();
    let mut order: Vec<usize> = (0..n).collect();
    // Deepest first, so children exist before their parents.
    order.sort_by_key(|&b| (std::cmp::Reverse(nest.depth[b]), b));
    let mut rects: Vec<Option<Rect>> = vec![None; n];
    for b in order {
        let member_rects = nest.members[b].iter().filter_map(|&c| nodes.get(c));
        let child_rects: Vec<Rect> = (0..n)
            .filter(|&c| nest.parent[c] == Some(b))
            .filter_map(|c| rects[c])
            .collect();
        let Some(inner) = bounding_box(member_rects.chain(child_rects.iter())) else {
            continue;
        };
        let pad = doc.boundaries()[b]
            .pad
            .map(|p| p.get())
            .filter(|p| crate::geom::sane(*p));
        let (l, t, r, bt) = padding(pad);
        rects[b] = Some(Rect::new(
            inner.x - l,
            inner.y - t,
            inner.w + l + r,
            inner.h + t + bt,
        ));
    }
    rects
}
