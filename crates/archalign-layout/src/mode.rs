//! Layout mode selection and Archify grid conversion (FR-LAY-01, FR-LAY-02, ADR 0019).

use archalign_model::{ArchitectureDoc, Component, GridLayout, LayoutMode as GridMode};

use crate::geom::{Point, Rect};
use crate::measure::DEFAULT_AUTHORED;

/// Archify's grid defaults (`grid.mjs` L3–11).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    /// Grid origin.
    pub origin: Point,
    /// Column count.
    pub cols: u64,
    /// Horizontal gap.
    pub gap_x: f64,
    /// Vertical gap.
    pub gap_y: f64,
    /// Cell width.
    pub cell_w: f64,
    /// Cell height.
    pub cell_h: f64,
}

impl Default for Grid {
    fn default() -> Self {
        Self {
            origin: Point::new(40.0, 80.0),
            cols: 4,
            gap_x: 30.0,
            gap_y: 40.0,
            cell_w: 130.0,
            cell_h: 64.0,
        }
    }
}

impl Grid {
    /// The grid of a `layout` block, with Archify's defaults for missing fields.
    pub fn from_layout(layout: &GridLayout) -> Self {
        let d = Grid::default();
        Self {
            origin: layout
                .origin
                .map(|[x, y]| Point::new(x.get(), y.get()))
                .unwrap_or(d.origin),
            cols: layout.cols.unwrap_or(d.cols),
            gap_x: layout.gap_x.map_or(d.gap_x, |n| n.get()),
            gap_y: layout.gap_y.map_or(d.gap_y, |n| n.get()),
            cell_w: layout.cell_w.map_or(d.cell_w, |n| n.get()),
            cell_h: layout.cell_h.map_or(d.cell_h, |n| n.get()),
        }
    }

    /// `origin + (col·(cellW+gapX), row·(cellH+gapY))`.
    pub fn cell_pos(&self, row: u64, col: u64) -> Point {
        Point::new(
            self.origin.x + col as f64 * (self.cell_w + self.gap_x),
            self.origin.y + row as f64 * (self.cell_h + self.gap_y),
        )
    }
}

/// How a document is laid out.
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    /// Every component has a position; draw as stored.
    Authored(Vec<Rect>),
    /// Positions are computed. `partial` is true when some components had positions,
    /// which are ignored (and reported as a warning).
    Auto {
        /// Some, but not all, components were positioned.
        partial: bool,
    },
}

fn has_cell(c: &Component) -> bool {
    c.row.is_some() && c.col.is_some()
}

/// The authored rectangle of a component, if it has one: `pos` wins over `row`/`col`.
pub fn authored_rect(c: &Component, grid: Option<&Grid>) -> Option<Rect> {
    let pos = match (c.pos, grid) {
        (Some([x, y]), _) => Point::new(x.get(), y.get()),
        (None, Some(g)) if has_cell(c) => g.cell_pos(c.row.unwrap_or(0), c.col.unwrap_or(0)),
        _ => return None,
    };
    let [w, h] = c
        .size
        .map(|[w, h]| [w.get(), h.get()])
        .unwrap_or([DEFAULT_AUTHORED.w, DEFAULT_AUTHORED.h]);
    // Out-of-range geometry counts as missing (NFR-QUAL-04).
    [pos.x, pos.y, w, h]
        .into_iter()
        .all(crate::geom::sane)
        .then(|| Rect::new(pos.x, pos.y, w, h))
}

/// Chooses the layout mode (FR-LAY-01).
pub fn select_mode(doc: &ArchitectureDoc) -> Mode {
    let grid = doc
        .layout
        .as_ref()
        .filter(|l| l.mode == GridMode::Grid)
        .map(Grid::from_layout);
    let rects: Vec<Option<Rect>> = doc
        .components
        .iter()
        .map(|c| authored_rect(c, grid.as_ref()))
        .collect();
    if !rects.is_empty() && rects.iter().all(Option::is_some) {
        return Mode::Authored(rects.into_iter().flatten().collect());
    }
    let partial = doc
        .components
        .iter()
        .any(|c| c.pos.is_some() || (grid.is_some() && has_cell(c)));
    Mode::Auto { partial }
}
