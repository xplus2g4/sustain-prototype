//! Shared virtualization for the tree and the source viewer (ADR 0009).
//!
//! Rows have a fixed height. Only rows inside the viewport plus [`OVERSCAN`] rows on each
//! side are built; spacers stand in for the rest so the scrollbar stays correct.

use std::ops::Range;

/// Extra rows built above and below the viewport (Appendix C, Tunable).
pub const OVERSCAN: usize = 20;

/// The scroll state a virtual list needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollState {
    /// Vertical offset of the content, in pixels.
    pub offset_y: f32,
    /// Height of the viewport, in pixels.
    pub viewport_h: f32,
}

impl Default for ScrollState {
    fn default() -> Self {
        // Until the first scroll event the viewport height is unknown (0); callers supply the
        // pane's height as a fallback through [`ScrollState::or_height`].
        Self {
            offset_y: 0.0,
            viewport_h: 0.0,
        }
    }
}

impl ScrollState {
    /// This state, with `fallback_h` as the viewport height if none was measured yet.
    pub fn or_height(self, fallback_h: f32) -> Self {
        if self.viewport_h > 0.0 {
            self
        } else {
            Self {
                viewport_h: fallback_h.max(0.0),
                ..self
            }
        }
    }
}

/// Rows that intersect the viewport.
pub fn visible_rows(scroll: ScrollState, row_h: f32, total: usize) -> Range<usize> {
    if total == 0 || row_h <= 0.0 {
        return 0..0;
    }
    let first = (scroll.offset_y.max(0.0) / row_h).floor() as usize;
    let count = (scroll.viewport_h.max(0.0) / row_h).ceil() as usize + 1;
    let first = first.min(total);
    first..(first + count).min(total)
}

/// Rows to build: the visible rows plus overscan on each side.
pub fn window(scroll: ScrollState, row_h: f32, total: usize) -> Range<usize> {
    let v = visible_rows(scroll, row_h, total);
    v.start.saturating_sub(OVERSCAN)..(v.end + OVERSCAN).min(total)
}

/// Offset that puts `row` at a third of the viewport height (clamped at the top).
pub fn offset_for_top_third(row: usize, row_h: f32, viewport_h: f32) -> f32 {
    (row as f32 * row_h - viewport_h / 3.0).max(0.0)
}

/// Offset that makes `row` visible with minimal scrolling.
pub fn offset_to_reveal(row: usize, row_h: f32, scroll: ScrollState) -> f32 {
    let top = row as f32 * row_h;
    let bottom = top + row_h;
    if top < scroll.offset_y {
        top
    } else if bottom > scroll.offset_y + scroll.viewport_h {
        (bottom - scroll.viewport_h).max(0.0)
    } else {
        scroll.offset_y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_is_visible_plus_overscan_whatever_the_size() {
        for total in [0usize, 5, 100, 10_000, 1_000_000] {
            for offset in [0.0f32, 500.0, 99_999.0, 1e9] {
                let s = ScrollState {
                    offset_y: offset,
                    viewport_h: 600.0,
                };
                let visible = visible_rows(s, 20.0, total);
                let w = window(s, 20.0, total);
                assert!(w.len() <= visible.len() + 2 * OVERSCAN, "{total} {offset}");
                assert!(w.start <= visible.start && w.end >= visible.end);
                assert!(w.end <= total);
            }
        }
    }

    #[test]
    fn top_third_and_reveal() {
        assert_eq!(offset_for_top_third(0, 20.0, 600.0), 0.0);
        assert_eq!(offset_for_top_third(100, 20.0, 600.0), 1800.0);
        let s = ScrollState {
            offset_y: 0.0,
            viewport_h: 100.0,
        };
        assert_eq!(offset_to_reveal(2, 20.0, s), 0.0);
        assert_eq!(offset_to_reveal(10, 20.0, s), 120.0);
    }
}
