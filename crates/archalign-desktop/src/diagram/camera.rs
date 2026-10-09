//! Diagram camera: zoom and pan between diagram space and canvas space (FR-DIA-02).

use iced::{Point, Size, Vector};

/// Smallest zoom (10%, Appendix C, Contract).
pub const MIN_ZOOM: f32 = 0.10;
/// Largest zoom (400%, Appendix C, Contract).
pub const MAX_ZOOM: f32 = 4.0;
/// Zoom factor of `+` / `-` (Appendix C, Tunable).
pub const ZOOM_STEP: f32 = 1.25;
/// Padding around a fitted diagram, in canvas pixels (Appendix C, Tunable).
pub const FIT_PADDING: f32 = 40.0;

/// `canvas = diagram × zoom + pan`, with canvas coordinates relative to the canvas origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Scale factor.
    pub zoom: f32,
    /// Translation in canvas pixels.
    pub pan: Vector,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vector::ZERO,
        }
    }
}

/// An axis-aligned diagram-space rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// Left.
    pub x: f32,
    /// Top.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Camera {
    /// Diagram → canvas.
    pub fn to_canvas(&self, p: Point) -> Point {
        Point::new(p.x * self.zoom + self.pan.x, p.y * self.zoom + self.pan.y)
    }

    /// Canvas → diagram.
    pub fn to_diagram(&self, p: Point) -> Point {
        Point::new(
            (p.x - self.pan.x) / self.zoom,
            (p.y - self.pan.y) / self.zoom,
        )
    }

    /// Multiplies the zoom by `factor` (clamped to 10%–400%), keeping the diagram point
    /// under `anchor` (canvas coordinates) where it is.
    pub fn zoom_about(&mut self, factor: f32, anchor: Point) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let before = self.to_diagram(anchor);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = Vector::new(
            anchor.x - before.x * self.zoom,
            anchor.y - before.y * self.zoom,
        );
    }

    /// Sets an absolute zoom about `anchor`.
    pub fn set_zoom_about(&mut self, zoom: f32, anchor: Point) {
        let factor = zoom / self.zoom;
        self.zoom_about(factor, anchor);
    }

    /// Moves the view by `delta` canvas pixels.
    pub fn pan_by(&mut self, delta: Vector) {
        if delta.x.is_finite() && delta.y.is_finite() {
            self.pan += delta;
        }
    }

    /// Centers a diagram point in a canvas of `viewport` size.
    pub fn center_on(&mut self, p: Point, viewport: Size) {
        self.pan = Vector::new(
            viewport.width / 2.0 - p.x * self.zoom,
            viewport.height / 2.0 - p.y * self.zoom,
        );
    }

    /// The camera that shows `content` centered in `viewport` with [`FIT_PADDING`] on every
    /// side, at the largest zoom that fits (clamped to 10%–400%).
    pub fn fit(content: Bounds, viewport: Size) -> Self {
        let avail_w = (viewport.width - 2.0 * FIT_PADDING).max(1.0);
        let avail_h = (viewport.height - 2.0 * FIT_PADDING).max(1.0);
        let zoom = if content.w <= 0.0 || content.h <= 0.0 {
            1.0
        } else {
            (avail_w / content.w).min(avail_h / content.h)
        }
        .clamp(MIN_ZOOM, MAX_ZOOM);
        let mut cam = Camera {
            zoom,
            pan: Vector::ZERO,
        };
        cam.center_on(
            Point::new(content.x + content.w / 2.0, content.y + content.h / 2.0),
            viewport,
        );
        cam
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn zoom_about_keeps_the_anchor_fixed() {
        let mut c = Camera {
            zoom: 1.0,
            pan: Vector::new(10.0, 20.0),
        };
        let anchor = Point::new(300.0, 200.0);
        let under = c.to_diagram(anchor);
        c.zoom_about(1.25, anchor);
        let after = c.to_canvas(under);
        assert!(close(after.x, anchor.x) && close(after.y, anchor.y));
        assert!(close(c.zoom, 1.25));
    }

    #[test]
    fn zoom_is_clamped_to_10_and_400_percent() {
        let mut c = Camera::default();
        for _ in 0..50 {
            c.zoom_about(1.25, Point::ORIGIN);
        }
        assert!(close(c.zoom, MAX_ZOOM));
        for _ in 0..100 {
            c.zoom_about(0.8, Point::new(50.0, 50.0));
        }
        assert!(close(c.zoom, MIN_ZOOM));
        c.zoom_about(f32::NAN, Point::ORIGIN);
        c.zoom_about(0.0, Point::ORIGIN);
        assert!(close(c.zoom, MIN_ZOOM));
    }

    #[test]
    fn fit_centers_with_40px_padding() {
        let content = Bounds {
            x: 100.0,
            y: 50.0,
            w: 800.0,
            h: 200.0,
        };
        let c = Camera::fit(content, Size::new(1000.0, 600.0));
        // Width limits: (1000 − 80) / 800 = 1.15.
        assert!(close(c.zoom, 1.15));
        let tl = c.to_canvas(Point::new(content.x, content.y));
        let br = c.to_canvas(Point::new(content.x + content.w, content.y + content.h));
        assert!(close(tl.x, 40.0) && close(br.x, 960.0));
        assert!(close((tl.y + br.y) / 2.0, 300.0), "vertically centered");
        // Huge content clamps at 10%.
        let c = Camera::fit(
            Bounds {
                x: 0.0,
                y: 0.0,
                w: 1e6,
                h: 10.0,
            },
            Size::new(1000.0, 600.0),
        );
        assert!(close(c.zoom, MIN_ZOOM));
    }

    #[test]
    fn round_trip() {
        let c = Camera {
            zoom: 0.37,
            pan: Vector::new(-12.0, 99.0),
        };
        let p = Point::new(123.0, -45.0);
        let q = c.to_diagram(c.to_canvas(p));
        assert!(close(p.x, q.x) && close(p.y, q.y));
    }
}
