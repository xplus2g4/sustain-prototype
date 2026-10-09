//! Diagram layout and orthogonal routing (pure, no I/O).
//!
//! [`layout_document`] turns an `ArchitectureDoc` into a [`Scene`]: authored positions are
//! used as stored (grid cells converted with Archify's formula), otherwise `dugong` places
//! the components as a compound graph ranked left to right. Every connection is routed
//! orthogonally around the boxes, and boundaries wrap their members (ADR 0018, ADR 0019).

pub mod boundaries;
pub mod engine;
pub mod geom;
pub mod measure;
pub mod mode;
pub mod route;
pub mod scene;

pub use engine::{
    DugongEngine, GridFallbackEngine, LayoutEngine, LayoutError, LayoutGraph, Placement,
};
pub use geom::{Point, Rect, Size};
pub use measure::{FixedAdvance, TextMeasure};
pub use mode::{Mode, select_mode};
pub use route::{Arrow, Route};
pub use scene::{
    Scene, SceneBoundary, SceneEdge, SceneMode, SceneNode, layout_document, layout_with, write_back,
};
