//! The three-pane `pane_grid` layout (FR-PANE-01..07, ADR 0006).

use iced::Size;
use iced::widget::pane_grid::{self, Axis, Configuration, DragEvent, Pane, ResizeEvent};
use serde::{Deserialize, Serialize};

/// The three panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaneKind {
    /// Project tree.
    Tree,
    /// Diagram canvas.
    Diagram,
    /// Source viewer.
    Source,
}

impl PaneKind {
    /// All panes, in default order.
    pub const ALL: [PaneKind; 3] = [PaneKind::Tree, PaneKind::Diagram, PaneKind::Source];

    /// Title-bar name.
    pub fn name(self) -> &'static str {
        match self {
            PaneKind::Tree => "Tree",
            PaneKind::Diagram => "Diagram",
            PaneKind::Source => "Source",
        }
    }
}

/// Default width ratios Tree / Diagram / Source (Appendix C, Tunable).
pub const DEFAULT_RATIOS: [f32; 3] = [0.20, 0.50, 0.30];
/// Minimum pane size in either dimension (Appendix C, Tunable).
pub const MIN_PANE: f32 = 160.0;
/// Space between panes.
pub const SPACING: f32 = 4.0;

/// The default arrangement: Tree | Diagram | Source at 0.20 / 0.50 / 0.30.
pub fn default_configuration() -> Configuration<PaneKind> {
    let [t, d, _] = DEFAULT_RATIOS;
    Configuration::Split {
        axis: Axis::Vertical,
        ratio: t,
        a: Box::new(Configuration::Pane(PaneKind::Tree)),
        b: Box::new(Configuration::Split {
            axis: Axis::Vertical,
            ratio: d / (1.0 - t),
            a: Box::new(Configuration::Pane(PaneKind::Diagram)),
            b: Box::new(Configuration::Pane(PaneKind::Source)),
        }),
    }
}

/// Pane arrangement plus which panes are hidden.
#[derive(Debug, Clone)]
pub struct PaneLayout {
    /// The pane grid state; each pane's value is its kind.
    pub state: pane_grid::State<PaneKind>,
}

impl Default for PaneLayout {
    fn default() -> Self {
        Self {
            state: pane_grid::State::with_configuration(default_configuration()),
        }
    }
}

impl PaneLayout {
    /// The pane showing `kind`, if visible.
    pub fn pane_of(&self, kind: PaneKind) -> Option<Pane> {
        self.state
            .iter()
            .find(|(_, k)| **k == kind)
            .map(|(p, _)| *p)
    }

    /// Whether `kind` is visible.
    pub fn is_visible(&self, kind: PaneKind) -> bool {
        self.pane_of(kind).is_some()
    }

    /// Hidden panes, in default order.
    pub fn hidden(&self) -> Vec<PaneKind> {
        PaneKind::ALL
            .into_iter()
            .filter(|k| !self.is_visible(*k))
            .collect()
    }

    /// Number of visible panes.
    pub fn visible_count(&self) -> usize {
        self.state.len()
    }

    /// FR-PANE-03: applies a drag. Drops follow `pane_grid::State::drop`; dragging is ignored
    /// while a pane is maximized.
    pub fn drag(&mut self, event: DragEvent) {
        if self.state.maximized().is_some() {
            return;
        }
        if let DragEvent::Dropped { pane, target } = event {
            self.state.drop(pane, target);
        }
    }

    /// FR-PANE-04: applies a resize, clamped so no pane gets smaller than [`MIN_PANE`] in
    /// a grid of `grid_size` (when the grid is large enough to allow it).
    pub fn resize(&mut self, event: ResizeEvent, grid_size: Size) {
        let ratio = clamp_ratio(&self.state, event.split, event.ratio, grid_size);
        self.state.resize(event.split, ratio);
    }

    /// FR-PANE-05: maximizes a pane.
    pub fn maximize(&mut self, pane: Pane) {
        self.state.maximize(pane);
    }

    /// FR-PANE-05: restores from maximized.
    pub fn restore(&mut self) {
        self.state.restore();
    }

    /// FR-PANE-06: closes a pane unless it is the last visible one.
    pub fn close(&mut self, pane: Pane) -> bool {
        if self.visible_count() <= 1 {
            return false;
        }
        self.state.close(pane).is_some()
    }

    /// FR-PANE-06: reopens a hidden pane by splitting the largest visible pane. Tree goes on
    /// the left, Diagram and Source on the right.
    pub fn reopen(&mut self, kind: PaneKind, grid_size: Size) {
        if self.is_visible(kind) {
            return;
        }
        self.state.restore();
        let regions = self
            .state
            .layout()
            .pane_regions(SPACING, MIN_PANE, grid_size);
        let largest = regions
            .iter()
            .max_by(|a, b| {
                let area = |r: &iced::Rectangle| r.width * r.height;
                area(a.1).total_cmp(&area(b.1)).then(b.0.cmp(a.0))
            })
            .map(|(p, _)| *p);
        let Some(target) = largest else {
            // No pane at all should never happen; rebuild the default rather than panic.
            *self = Self::default();
            return;
        };
        if let Some((new_pane, _)) = self.state.split(Axis::Vertical, target, kind)
            && kind == PaneKind::Tree
        {
            self.state.swap(target, new_pane);
        }
    }

    /// Shows or hides a pane (the Panels menu and ⌘1/⌘2/⌘3).
    pub fn toggle(&mut self, kind: PaneKind, grid_size: Size) {
        match self.pane_of(kind) {
            Some(pane) => {
                let _ = self.close(pane);
            }
            None => self.reopen(kind, grid_size),
        }
    }

    /// FR-PANE-07: the default arrangement and ratios, nothing maximized.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The pane rectangles inside a grid of `size`.
    pub fn regions(&self, size: Size) -> Vec<(PaneKind, iced::Rectangle)> {
        let regions = match self.state.maximized() {
            Some(p) => [(p, iced::Rectangle::new(iced::Point::ORIGIN, size))]
                .into_iter()
                .collect(),
            None => self.state.layout().pane_regions(SPACING, MIN_PANE, size),
        };
        regions
            .into_iter()
            .filter_map(|(p, r)| self.state.get(p).map(|k| (*k, r)))
            .collect()
    }
}

/// The ratio a split actually gets when every pane must keep at least [`MIN_PANE`] pixels
/// (pane_grid's own min-size arithmetic, which accounts for nested splits).
pub fn clamp_ratio(
    state: &pane_grid::State<PaneKind>,
    split: pane_grid::Split,
    ratio: f32,
    grid_size: Size,
) -> f32 {
    let ratio = ratio.clamp(0.0, 1.0);
    let mut probe = state.clone();
    probe.resize(split, ratio);
    probe
        .layout()
        .split_regions(SPACING, MIN_PANE, grid_size)
        .get(&split)
        .map(|(_, _, effective)| *effective)
        .filter(|r| r.is_finite())
        .unwrap_or(ratio)
}
