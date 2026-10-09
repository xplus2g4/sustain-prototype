//! Node placement engines behind [`LayoutEngine`] (ADR 0018).

use std::panic::{AssertUnwindSafe, catch_unwind};

use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, NodeLabel, RankDir};

use crate::geom::{Point, Size};

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

/// Dagre (via `dugong`) as a compound graph ranked left to right.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DugongEngine {
    /// Separation between nodes in the same rank (vertical, in LR).
    pub nodesep: f64,
    /// Separation between ranks (horizontal, in LR).
    pub ranksep: f64,
    /// Separation between edges.
    pub edgesep: f64,
}

impl Default for DugongEngine {
    fn default() -> Self {
        Self {
            nodesep: 60.0,
            ranksep: 110.0,
            edgesep: 20.0,
        }
    }
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
            rankdir: RankDir::LR,
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
