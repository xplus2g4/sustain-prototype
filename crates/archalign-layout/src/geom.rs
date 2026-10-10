//! Geometry primitives in diagram space (pixels, y down).

use serde::Serialize;

/// A point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Point {
    /// x
    pub x: f64,
    /// y
    pub y: f64,
}

impl Point {
    /// A point.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A width and height.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Size {
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

impl Size {
    /// A size.
    pub const fn new(w: f64, h: f64) -> Self {
        Self { w, h }
    }
}

/// An axis-aligned rectangle (top-left origin).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Rect {
    /// Left.
    pub x: f64,
    /// Top.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

impl Rect {
    /// A rectangle.
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    /// Right edge.
    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    /// Center point.
    pub fn center(&self) -> Point {
        Point::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Grows the rectangle by `d` on every side.
    pub fn inflate(&self, d: f64) -> Rect {
        Rect::new(self.x - d, self.y - d, self.w + 2.0 * d, self.h + 2.0 * d)
    }

    /// Whether the interiors intersect (touching edges do not count).
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }

    /// Whether `o` lies inside (edges may touch).
    pub fn contains_rect(&self, o: &Rect) -> bool {
        const EPS: f64 = 1e-6;
        self.x <= o.x + EPS
            && self.y <= o.y + EPS
            && o.right() <= self.right() + EPS
            && o.bottom() <= self.bottom() + EPS
    }

    /// Whether `p` lies inside (edges count).
    pub fn contains_point(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    /// The smallest rectangle containing both.
    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect::new(
            x,
            y,
            self.right().max(o.right()) - x,
            self.bottom().max(o.bottom()) - y,
        )
    }

    /// Moves the rectangle.
    pub fn translate(&self, dx: f64, dy: f64) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    /// Whether the open segment `a`–`b` (axis-aligned) passes through the interior.
    pub fn segment_crosses_interior(&self, a: Point, b: Point) -> bool {
        const EPS: f64 = 1e-6;
        let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        x0 < self.right() - EPS
            && x1 > self.x + EPS
            && y0 < self.bottom() - EPS
            && y1 > self.y + EPS
    }
}

/// Bounding box of rectangles; `None` for an empty input.
pub fn bounding_box<'a>(rects: impl IntoIterator<Item = &'a Rect>) -> Option<Rect> {
    rects.into_iter().copied().reduce(|a, b| a.union(&b))
}

/// Largest coordinate magnitude taken from a file (NFR-QUAL-04). Larger or non-finite
/// values are treated as missing, so geometry stays finite in `f32` at any zoom.
pub const MAX_COORD: f64 = 1_000_000.0;

/// Whether a coordinate read from a file is usable (see [`MAX_COORD`]).
pub fn sane(v: f64) -> bool {
    v.is_finite() && v.abs() <= MAX_COORD
}
