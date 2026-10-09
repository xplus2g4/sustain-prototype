//! Turning a document into a [`Scene`] of absolute geometry, and writing geometry back.

use std::collections::HashMap;

use archalign_model::{
    ArchitectureDoc, BoundaryKind, ComponentType, Issue, IssueCode, Num, Variant,
};
use serde::Serialize;

use crate::boundaries::{boundary_rects, nesting};
use crate::engine::{DugongEngine, GridFallbackEngine, LayoutEngine, LayoutGraph};
use crate::geom::{Point, Rect, bounding_box};
use crate::measure::{TextMeasure, auto_node_size};
use crate::mode::{Mode, select_mode};
use crate::route::{EdgeRequest, Route, route_all};

/// Margin added around the drawing in auto layouts, and around `bounds`.
pub const ORIGIN: f64 = 40.0;

/// Whether geometry was taken from the file or computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SceneMode {
    /// Positions as stored.
    Authored,
    /// Positions computed by the layout engine.
    Auto,
}

/// A component box.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SceneNode {
    /// Component ID.
    pub id: String,
    /// Index into `components`.
    pub index: usize,
    /// Box.
    pub rect: Rect,
    /// Component type.
    pub ty: ComponentType,
}

/// A boundary rectangle.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SceneBoundary {
    /// Index into `boundaries`.
    pub index: usize,
    /// Region or security group.
    pub kind: BoundaryKind,
    /// Rectangle including padding.
    pub rect: Rect,
    /// Nesting depth (0 = top level).
    pub depth: usize,
    /// Enclosing boundary index.
    pub parent: Option<usize>,
    /// Top-left anchor of the label text.
    pub label_anchor: Point,
}

/// A routed connection.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SceneEdge {
    /// Index into `connections`.
    pub index: usize,
    /// Source component index.
    pub from: usize,
    /// Target component index.
    pub to: usize,
    /// Line style.
    pub variant: Variant,
    /// Route geometry.
    pub route: Route,
}

/// Absolute geometry of a document, in diagram space.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Scene {
    /// Authored or auto.
    pub mode: SceneMode,
    /// Boxes, in component order.
    pub nodes: Vec<SceneNode>,
    /// Boundaries with at least one known member, outermost first.
    pub boundaries: Vec<SceneBoundary>,
    /// Connections whose endpoints exist, in connection order.
    pub edges: Vec<SceneEdge>,
    /// Bounding box of everything plus a margin.
    pub bounds: Rect,
    /// Layout warnings (partial positions ignored).
    #[serde(skip)]
    pub warnings: Vec<Issue>,
}

impl Scene {
    /// The node of a component ID.
    pub fn node(&self, id: &str) -> Option<&SceneNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

/// Lays out a document with the default engine (dagre via dugong, with a grid fallback).
pub fn layout_document(doc: &ArchitectureDoc, measure: &dyn TextMeasure) -> Scene {
    layout_with(doc, measure, &DugongEngine::default())
}

/// Lays out a document with a specific placement engine.
pub fn layout_with(
    doc: &ArchitectureDoc,
    measure: &dyn TextMeasure,
    engine: &dyn LayoutEngine,
) -> Scene {
    let nest = nesting(doc);
    let mut warnings = Vec::new();
    let (mode, rects) = match select_mode(doc) {
        Mode::Authored(rects) => (SceneMode::Authored, rects),
        Mode::Auto { partial } => {
            if partial {
                warnings.push(Issue::warning(
                    IssueCode::PartialPositions,
                    "/components",
                    "Only some components have positions; they were ignored and the whole diagram was laid out automatically",
                ));
            }
            (SceneMode::Auto, auto_rects(doc, measure, engine, &nest))
        }
    };
    let brects = boundary_rects(doc, &nest, &rects);

    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, c) in doc.components.iter().enumerate() {
        index.entry(c.id.as_str()).or_insert(i);
    }
    let mut requests = Vec::new();
    let mut meta = Vec::new();
    for (k, conn) in doc.connections().iter().enumerate() {
        let (Some(&from), Some(&to)) = (index.get(conn.from.as_str()), index.get(conn.to.as_str()))
        else {
            continue;
        };
        let via = if mode == SceneMode::Authored {
            conn.via
                .iter()
                .flatten()
                .map(|[x, y]| Point::new(x.get(), y.get()))
                .filter(|p| p.x.is_finite() && p.y.is_finite())
                .collect()
        } else {
            Vec::new()
        };
        requests.push(EdgeRequest {
            from,
            to,
            from_side: conn.from_side,
            to_side: conn.to_side,
            via,
        });
        meta.push((k, from, to, conn.variant()));
    }
    let routes = route_all(&rects, &requests);
    let edges: Vec<SceneEdge> = meta
        .into_iter()
        .zip(routes)
        .map(|((index, from, to, variant), route)| SceneEdge {
            index,
            from,
            to,
            variant,
            route,
        })
        .collect();

    let nodes: Vec<SceneNode> = doc
        .components
        .iter()
        .zip(&rects)
        .enumerate()
        .map(|(i, (c, r))| SceneNode {
            id: c.id.clone(),
            index: i,
            rect: *r,
            ty: c.ty,
        })
        .collect();
    let mut boundaries: Vec<SceneBoundary> = brects
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            r.map(|rect| SceneBoundary {
                index: i,
                kind: doc.boundaries()[i].kind,
                rect,
                depth: nest.depth[i],
                parent: nest.parent[i],
                label_anchor: Point::new(rect.x + 10.0, rect.y + 8.0),
            })
        })
        .collect();
    boundaries.sort_by_key(|b| (b.depth, b.index));

    let mut all: Vec<Rect> = rects.clone();
    all.extend(boundaries.iter().map(|b| b.rect));
    for e in &edges {
        for p in &e.route.points {
            all.push(Rect::new(p.x, p.y, 0.0, 0.0));
        }
    }
    let bounds = bounding_box(all.iter())
        .map(|r| r.inflate(ORIGIN))
        .unwrap_or(Rect::new(0.0, 0.0, 2.0 * ORIGIN, 2.0 * ORIGIN));
    Scene {
        mode,
        nodes,
        boundaries,
        edges,
        bounds,
        warnings,
    }
}

/// Computes auto-layout rectangles: measured sizes, placement, integer coordinates, and a
/// translation so that the drawing (boundaries included) starts at [`ORIGIN`].
fn auto_rects(
    doc: &ArchitectureDoc,
    measure: &dyn TextMeasure,
    engine: &dyn LayoutEngine,
    nest: &crate::boundaries::Nesting,
) -> Vec<Rect> {
    let sizes: Vec<_> = doc
        .components
        .iter()
        .map(|c| auto_node_size(c, measure))
        .collect();
    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, c) in doc.components.iter().enumerate() {
        index.entry(c.id.as_str()).or_insert(i);
    }
    let graph = LayoutGraph {
        nodes: sizes.clone(),
        node_parent: nest.innermost.clone(),
        cluster_parent: nest
            .parent
            .iter()
            .enumerate()
            .map(|(b, p)| if nest.members[b].is_empty() { None } else { *p })
            .collect(),
        edges: doc
            .connections()
            .iter()
            .filter_map(|c| Some((*index.get(c.from.as_str())?, *index.get(c.to.as_str())?)))
            .collect(),
    };
    let placement = engine
        .place(&graph)
        .ok()
        .filter(|p| p.centers.len() == sizes.len())
        .filter(|p| no_overlap(&to_rects(&p.centers, &sizes)))
        .or_else(|| GridFallbackEngine.place(&graph).ok())
        .map(|p| p.centers)
        .unwrap_or_default();
    let mut rects = to_rects(&placement, &sizes);
    let brects = boundary_rects(doc, nest, &rects);
    let all: Vec<Rect> = rects
        .iter()
        .copied()
        .chain(brects.into_iter().flatten())
        .collect();
    if let Some(bb) = bounding_box(all.iter()) {
        let (dx, dy) = ((ORIGIN - bb.x).round(), (ORIGIN - bb.y).round());
        for r in &mut rects {
            *r = r.translate(dx, dy);
        }
    }
    rects
}

fn to_rects(centers: &[Point], sizes: &[crate::geom::Size]) -> Vec<Rect> {
    centers
        .iter()
        .zip(sizes)
        .map(|(c, s)| {
            Rect::new(
                (c.x - s.w / 2.0).round(),
                (c.y - s.h / 2.0).round(),
                s.w,
                s.h,
            )
        })
        .collect()
}

fn no_overlap(rects: &[Rect]) -> bool {
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            if rects[i].overlaps(&rects[j]) {
                return false;
            }
        }
    }
    true
}

/// Writes integer `pos`/`size` for every component (FR-LAY-08).
pub fn write_back(doc: &mut ArchitectureDoc, scene: &Scene) {
    for node in &scene.nodes {
        if let Some(c) = doc.components.get_mut(node.index) {
            let r = node.rect;
            c.pos = Some([Num(r.x.round()), Num(r.y.round())]);
            c.size = Some([Num(r.w.round().max(1.0)), Num(r.h.round().max(1.0))]);
        }
    }
}
