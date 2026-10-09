//! Text measurement and node sizing (FR-LAY-03, ADR 0010).

use archalign_model::Component;

use crate::geom::Size;

/// Measures one line of text in the bundled font.
pub trait TextMeasure {
    /// The size of `text` drawn at `size_px`.
    fn measure(&self, text: &str, size_px: f64) -> Size;
}

/// A deterministic monospace measurer for tests: every character advances
/// `advance × size_px`, and a line is `line_height × size_px` tall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedAdvance {
    /// Advance per character, as a fraction of the font size.
    pub advance: f64,
    /// Line height, as a fraction of the font size.
    pub line_height: f64,
}

impl Default for FixedAdvance {
    /// JetBrains Mono's metrics: 600/1000 em advance, 1.32 em line height.
    fn default() -> Self {
        Self {
            advance: 0.6,
            line_height: 1.32,
        }
    }
}

impl TextMeasure for FixedAdvance {
    fn measure(&self, text: &str, size_px: f64) -> Size {
        let chars = text.chars().count() as f64;
        Size::new(chars * self.advance * size_px, self.line_height * size_px)
    }
}

/// Label font size (px).
pub const LABEL_PX: f64 = 14.0;
/// Sublabel font size (px).
pub const SUBLABEL_PX: f64 = 12.0;
/// Tag font size (px).
pub const TAG_PX: f64 = 11.0;
/// Horizontal padding on each side of the text block.
pub const PAD_X: f64 = 12.0;
/// Vertical padding above and below the text block.
pub const PAD_Y: f64 = 10.0;
/// Minimum auto-layout node width.
pub const MIN_W: f64 = 120.0;
/// Minimum auto-layout node height.
pub const MIN_H: f64 = 60.0;
/// Size of an authored component without `size` (Archify's default).
pub const DEFAULT_AUTHORED: Size = Size::new(120.0, 60.0);

/// The auto-layout size of a component: its stacked label, sublabel and tag lines plus
/// padding, at least 120×60, rounded up to whole pixels.
pub fn auto_node_size(component: &Component, measure: &dyn TextMeasure) -> Size {
    let mut lines = vec![measure.measure(&component.label, LABEL_PX)];
    if let Some(s) = component.sublabel.as_deref().filter(|s| !s.is_empty()) {
        lines.push(measure.measure(s, SUBLABEL_PX));
    }
    if let Some(t) = component.tag.as_deref().filter(|s| !s.is_empty()) {
        lines.push(measure.measure(t, TAG_PX));
    }
    let text_w = lines.iter().map(|s| s.w).fold(0.0, f64::max);
    let text_h: f64 = lines.iter().map(|s| s.h).sum();
    Size::new(
        (text_w + 2.0 * PAD_X).max(MIN_W).ceil(),
        (text_h + 2.0 * PAD_Y).max(MIN_H).ceil(),
    )
}
