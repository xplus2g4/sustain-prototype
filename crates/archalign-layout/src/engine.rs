//! Node placement engines behind [`LayoutEngine`] (ADR 0031, ADR 0035).

use std::panic::{AssertUnwindSafe, catch_unwind};

use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, NodeLabel, RankDir};

use crate::geom::{Point, Rect, Size};

/// A compound graph to place: nodes with sizes, clusters (boundaries) with parents.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutGraph {
    /// Node sizes, indexed like the document's components.
    pub nodes: Vec<Size>,
    /// The innermost cluster of each node.
    pub node_parent: Vec<Option<usize>>,
    /// The parent cluster of each cluster.
    pub cluster_parent: Vec<Option<usize>>,
    /// Directed edges between node indices (self-loops are ignored by engines).
    pub edges: Vec<(usize, usize)>,
}

/// Node centers computed by an engine.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    /// Center of each node.
    pub centers: Vec<Point>,
}

/// Why placement failed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LayoutError {
    /// The engine reported an error.
    #[error("layout engine failed: {0}")]
    Engine(String),
    /// The engine panicked or produced non-finite coordinates.
    #[error("layout engine produced no usable placement")]
    Unusable,
}

/// Computes node positions for a graph.
pub trait LayoutEngine {
    /// Places every node. Must be deterministic.
    fn place(&self, graph: &LayoutGraph) -> Result<Placement, LayoutError>;
}

/// The direction ranks advance in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RankDirection {
    /// Ranks are columns, left to right.
    #[default]
    LeftToRight,
    /// Ranks are rows, top to bottom.
    TopToBottom,
}

/// Dagre (via `dugong`) as a compound graph ranked in one direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DugongEngine {
    /// Rank direction.
    pub direction: RankDirection,
    /// Separation between nodes in the same rank.
    pub nodesep: f64,
    /// Separation between ranks.
    pub ranksep: f64,
    /// Separation between edges.
    pub edgesep: f64,
}

impl Default for DugongEngine {
    fn default() -> Self {
        Self {
            direction: RankDirection::LeftToRight,
            nodesep: 60.0,
            ranksep: 110.0,
            edgesep: 20.0,
        }
    }
}

/// The reference viewport for choosing a direction: 16:10, like the default window.
pub const REFERENCE_VIEWPORT: Size = Size {
    w: 1600.0,
    h: 1000.0,
};

/// FR-LAY-04 (ADR 0035): places the graph left to right and top to bottom, and keeps the
/// placement whose node bounding box fits [`REFERENCE_VIEWPORT`] at the larger scale.
/// Ties, and graphs where only one direction yields a usable placement, keep that one;
/// left to right wins ties.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AutoDirectionEngine {
    /// Spacing shared by both directions (`direction` is ignored).
    pub base: DugongEngine,
}

impl AutoDirectionEngine {
    /// Places in one direction and keeps the result only if it is usable (every node
    /// placed, no overlaps).
    fn try_place(&self, lg: &LayoutGraph, direction: RankDirection) -> Option<Placement> {
        let engine = DugongEngine {
            direction,
            ..self.base
        };
        engine
            .place(lg)
            .ok()
            .filter(|p| p.centers.len() == lg.nodes.len() && !overlapping(p, &lg.nodes))
    }
}

impl LayoutEngine for AutoDirectionEngine {
    fn place(&self, lg: &LayoutGraph) -> Result<Placement, LayoutError> {
        let lr = self.try_place(lg, RankDirection::LeftToRight);
        let tb = self.try_place(lg, RankDirection::TopToBottom);
        match (lr, tb) {
            (Some(a), Some(b)) => {
                if fit_scale(&b, &lg.nodes) > fit_scale(&a, &lg.nodes) * (1.0 + 1e-9) {
                    Ok(b)
                } else {
                    Ok(a)
                }
            }
            (Some(a), None) => Ok(a),
            (None, Some(b)) => Ok(b),
            (None, None) => Err(LayoutError::Unusable),
        }
    }
}

fn node_rects<'a>(p: &'a Placement, sizes: &'a [Size]) -> impl Iterator<Item = Rect> + 'a {
    p.centers
        .iter()
        .zip(sizes)
        .map(|(c, s)| Rect::new(c.x - s.w / 2.0, c.y - s.h / 2.0, s.w, s.h))
}

fn overlapping(p: &Placement, sizes: &[Size]) -> bool {
    let rects: Vec<Rect> = node_rects(p, sizes).collect();
    (0..rects.len()).any(|i| ((i + 1)..rects.len()).any(|j| rects[i].overlaps(&rects[j])))
}

/// The scale at which the placement's node bounding box fits [`REFERENCE_VIEWPORT`].
pub fn fit_scale(p: &Placement, sizes: &[Size]) -> f64 {
    let mut it = node_rects(p, sizes);
    let Some(first) = it.next() else {
        return f64::INFINITY;
    };
    let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x + first.w, first.y + first.h);
    for r in it {
        x0 = x0.min(r.x);
        y0 = y0.min(r.y);
        x1 = x1.max(r.x + r.w);
        y1 = y1.max(r.y + r.h);
    }
    let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    (REFERENCE_VIEWPORT.w / w).min(REFERENCE_VIEWPORT.h / h)
}

fn node_id(i: usize) -> String {
    format!("n{i}")
}

fn cluster_id(i: usize) -> String {
    format!("c{i}")
}

impl LayoutEngine for DugongEngine {
    fn place(&self, lg: &LayoutGraph) -> Result<Placement, LayoutError> {
        if lg.nodes.is_empty() {
            return Ok(Placement {
                centers: Vec::new(),
            });
        }
        let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        });
        g.set_graph(GraphLabel {
            rankdir: match self.direction {
                RankDirection::LeftToRight => RankDir::LR,
                RankDirection::TopToBottom => RankDir::TB,
            },
            nodesep: self.nodesep,
            ranksep: self.ranksep,
            edgesep: self.edgesep,
            ..Default::default()
        });
        g.set_default_edge_label(EdgeLabel::default);
        for i in 0..lg.cluster_parent.len() {
            g.set_node(cluster_id(i), NodeLabel::default());
        }
        for (i, size) in lg.nodes.iter().enumerate() {
            g.set_node(
                node_id(i),
                NodeLabel {
                    width: size.w,
                    height: size.h,
                    ..Default::default()
                },
            );
        }
        for (i, parent) in lg.cluster_parent.iter().enumerate() {
            if let Some(p) = parent {
                g.set_parent(cluster_id(i), cluster_id(*p));
            }
        }
        for (i, parent) in lg.node_parent.iter().enumerate() {
            if let Some(p) = parent {
                g.set_parent(node_id(i), cluster_id(*p));
            }
        }
        for (k, &(a, b)) in lg.edges.iter().enumerate() {
            if a == b || a >= lg.nodes.len() || b >= lg.nodes.len() {
                continue;
            }
            g.set_edge_named(
                node_id(a),
                node_id(b),
                Some(format!("e{k}")),
                Some(EdgeLabel {
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                }),
            );
        }
        let result = catch_unwind(AssertUnwindSafe(|| dugong::layout(&mut g)))
            .map_err(|_| LayoutError::Unusable)?;
        result.map_err(|e| LayoutError::Engine(format!("{e:?}")))?;
        let mut centers = Vec::with_capacity(lg.nodes.len());
        for i in 0..lg.nodes.len() {
            let label = g.node(&node_id(i)).ok_or(LayoutError::Unusable)?;
            match (label.x, label.y) {
                (Some(x), Some(y)) if x.is_finite() && y.is_finite() => {
                    centers.push(Point::new(x, y))
                }
                _ => return Err(LayoutError::Unusable),
            }
        }
        Ok(Placement { centers })
    }
}

/// A last-resort engine: nodes in a square grid, row-major. Never fails and never overlaps.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GridFallbackEngine;

impl LayoutEngine for GridFallbackEngine {
    fn place(&self, lg: &LayoutGraph) -> Result<Placement, LayoutError> {
        let n = lg.nodes.len();
        let cols = (n as f64).sqrt().ceil().max(1.0) as usize;
        let cell_w = lg.nodes.iter().map(|s| s.w).fold(0.0, f64::max) + 120.0;
        let cell_h = lg.nodes.iter().map(|s| s.h).fold(0.0, f64::max) + 120.0;
        let centers = (0..n)
            .map(|i| {
                Point::new(
                    (i % cols) as f64 * cell_w + cell_w / 2.0,
                    (i / cols) as f64 * cell_h + cell_h / 2.0,
                )
            })
            .collect();
        Ok(Placement { centers })
    }
}
