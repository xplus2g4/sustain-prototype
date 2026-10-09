//! The diagram canvas: a cached static layer plus a cheap overlay layer (FR-DIA-01..06).

use std::cell::Cell;
use std::collections::BTreeSet;

use archalign_layout::{Scene, SceneMode};
use archalign_model::{ArchitectureDoc, BoundaryKind};
use iced::keyboard::Modifiers;
use iced::mouse::{self, ScrollDelta};
use iced::widget::canvas::{self, Action, Event, Frame, Geometry, LineDash, Path, Stroke, Text};
use iced::{Color, Pixels, Point, Rectangle, Renderer, Size, Theme, Vector, alignment};

use super::camera::Camera;
use super::logic::{Badges, LegendRow, ReachSet};
use crate::fonts::{MONO, MONO_BOLD};
use crate::palette::{Palette, ThemeChoice};

/// What a press on the canvas hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// A component (index).
    Node(usize),
    /// A connection (index into `connections`).
    Edge(usize),
    /// Empty canvas (starts a pan drag).
    Empty,
}

/// Messages the canvas publishes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanvasEvent {
    /// Left button pressed inside the canvas; the canvas now has focus.
    Pressed(Hit),
    /// A press happened outside the canvas; it lost focus.
    Unfocused,
    /// Drag or scroll pan, in canvas pixels.
    Pan(Vector),
    /// ⌘/Ctrl+scroll zoom about a canvas point.
    ZoomAt {
        /// Zoom factor.
        factor: f32,
        /// Anchor in canvas coordinates.
        anchor: Point,
    },
    /// The node under the cursor changed (with the cursor position in canvas coordinates).
    Hover(Option<usize>, Point),
    /// The canvas size is known or changed.
    Resized(Size),
}

/// Everything the canvas draws, borrowed from the app state.
pub struct DiagramCanvas<'a> {
    /// The document.
    pub doc: &'a ArchitectureDoc,
    /// Its geometry.
    pub scene: &'a Scene,
    /// Per-node badges.
    pub badges: &'a [Badges],
    /// Legend rows.
    pub legend: &'a [LegendRow],
    /// Camera.
    pub camera: Camera,
    /// Palette.
    pub palette: &'a Palette,
    /// Theme (part of the cache key).
    pub theme: ThemeChoice,
    /// Bumped whenever the scene changes (part of the cache key).
    pub version: u64,
    /// Selected node.
    pub selected_node: Option<usize>,
    /// Selected connection.
    pub selected_edge: Option<usize>,
    /// Reach highlight.
    pub reach: Option<&'a ReachSet>,
    /// Nodes citing the file selected in the tree.
    pub reverse: &'a BTreeSet<usize>,
    /// Hovered node.
    pub hover: Option<usize>,
}

/// What the static layer was last drawn for: scene version, theme, zoom and pan bits.
type CacheKey = (u64, ThemeChoice, u32, u32, u32);

/// Widget state kept by iced across frames.
#[derive(Default)]
pub struct CanvasState {
    cache: canvas::Cache,
    key: Cell<Option<CacheKey>>,
    focused: bool,
    drag_from: Option<Point>,
    modifiers: Modifiers,
    hover: Option<usize>,
    size: Option<Size>,
}

/// Pixels of tolerance when clicking a connection.
const EDGE_HIT_PX: f32 = 5.0;

impl DiagramCanvas<'_> {
    /// Hit test at a canvas point (topmost node first, then connections).
    pub fn hit(&self, p: Point) -> Hit {
        hit_test(self.scene, self.camera, p)
    }
}

/// Hit test at a canvas point: nodes (topmost first), then connections within 5 px.
pub fn hit_test(scene: &Scene, camera: Camera, p: Point) -> Hit {
    let d = camera.to_diagram(p);
    let (x, y) = (f64::from(d.x), f64::from(d.y));
    for n in scene.nodes.iter().rev() {
        let r = n.rect;
        if x >= r.x && x <= r.right() && y >= r.y && y <= r.bottom() {
            return Hit::Node(n.index);
        }
    }
    let tol = f64::from(EDGE_HIT_PX / camera.zoom.max(1e-3));
    for e in &scene.edges {
        for w in e.route.points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (x0, x1) = (a.x.min(b.x) - tol, a.x.max(b.x) + tol);
            let (y0, y1) = (a.y.min(b.y) - tol, a.y.max(b.y) + tol);
            if x >= x0 && x <= x1 && y >= y0 && y <= y1 {
                return Hit::Edge(e.index);
            }
        }
    }
    Hit::Empty
}

fn pt(p: archalign_layout::Point) -> Point {
    Point::new(p.x as f32, p.y as f32)
}

fn rect_on(camera: Camera, r: &archalign_layout::Rect) -> Rectangle {
    let tl = camera.to_canvas(Point::new(r.x as f32, r.y as f32));
    Rectangle::new(
        tl,
        Size::new(r.w as f32 * camera.zoom, r.h as f32 * camera.zoom),
    )
}

fn with_alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

fn text(content: impl Into<String>, at: Point, size: f32, color: Color, bold: bool) -> Text {
    Text {
        content: content.into(),
        position: at,
        max_width: f32::INFINITY,
        color,
        size: Pixels(size),
        line_height: iced::widget::text::LineHeight::default(),
        font: if bold { MONO_BOLD } else { MONO },
        align_x: iced::widget::text::Alignment::Center,
        align_y: alignment::Vertical::Center,
        shaping: iced::widget::text::Shaping::Advanced,
    }
}

/// Smallest on-screen font size that is still drawn.
const MIN_TEXT_PX: f32 = 3.0;

fn polyline(points: &[Point]) -> Path {
    Path::new(|b| {
        if let Some(first) = points.first() {
            b.move_to(*first);
            for p in &points[1..] {
                b.line_to(*p);
            }
        }
    })
}

/// Draws the static layer: boundaries, connections, nodes, labels, badges and the legend.
pub fn draw_static(frame: &mut Frame, c: &DiagramCanvas<'_>) {
    let cam = c.camera;
    let z = cam.zoom;
    let p = c.palette;
    // Boundaries, outermost first.
    for b in &c.scene.boundaries {
        let r = rect_on(cam, &b.rect);
        let (fill, stroke, dash) = p.boundary(b.kind);
        let radius = if b.kind == BoundaryKind::SecurityGroup {
            8.0
        } else {
            12.0
        } * z;
        let path = Path::rounded_rectangle(r.position(), r.size(), radius.into());
        frame.fill(&path, fill);
        let segs = [dash[0] * z.max(0.3), dash[1] * z.max(0.3)];
        frame.stroke(
            &path,
            Stroke {
                line_dash: LineDash {
                    segments: &segs,
                    offset: 0,
                },
                ..Stroke::default().with_color(stroke).with_width(1.0)
            },
        );
        let size = 11.0 * z;
        if size >= MIN_TEXT_PX {
            let label = c
                .doc
                .boundaries()
                .get(b.index)
                .map(|x| x.label.as_str())
                .unwrap_or("");
            let mut t = text(label, cam.to_canvas(pt(b.label_anchor)), size, stroke, true);
            t.align_x = iced::widget::text::Alignment::Left;
            t.align_y = alignment::Vertical::Top;
            frame.fill_text(t);
        }
    }
    // Connections.
    for e in &c.scene.edges {
        let pts: Vec<Point> = e
            .route
            .points
            .iter()
            .map(|q| cam.to_canvas(pt(*q)))
            .collect();
        let (color, dash) = p.connection(e.variant);
        draw_edge(frame, &pts, e.route.arrow, color, dash, 1.5, cam);
        let label = c
            .doc
            .connections()
            .get(e.index)
            .and_then(|x| x.label.as_deref());
        if let Some(label) = label.filter(|l| !l.is_empty()) {
            let size = 10.0 * z;
            if size >= MIN_TEXT_PX {
                let at = cam.to_canvas(pt(e.route.label_anchor));
                let w = label.chars().count() as f32 * size * 0.6 + 6.0 * z;
                let h = size * 1.4;
                frame.fill_rectangle(
                    Point::new(at.x - w / 2.0, at.y - h / 2.0),
                    Size::new(w, h),
                    with_alpha(p.mask, 0.85),
                );
                frame.fill_text(text(label, at, size, p.text_muted, false));
            }
        }
    }
    // Nodes.
    for n in &c.scene.nodes {
        draw_node(frame, c, n.index, &n.rect, None);
    }
    // Legend (fixed in canvas space, bottom-left).
    draw_legend(frame, c);
}

#[allow(clippy::too_many_arguments)]
fn draw_edge(
    frame: &mut Frame,
    pts: &[Point],
    arrow: archalign_layout::Arrow,
    color: Color,
    dash: Option<[f32; 2]>,
    width: f32,
    cam: Camera,
) {
    let path = polyline(pts);
    let segs = dash.map(|d| [d[0] * cam.zoom.max(0.3), d[1] * cam.zoom.max(0.3)]);
    let mut stroke = Stroke::default().with_color(color).with_width(width);
    if let Some(s) = &segs {
        stroke.line_dash = LineDash {
            segments: s,
            offset: 0,
        };
    }
    frame.stroke(&path, stroke);
    // Arrowhead at the target, pointing along the last segment.
    let tip = cam.to_canvas(pt(arrow.tip));
    let (dx, dy) = (arrow.dir.x as f32, arrow.dir.y as f32);
    let len = (8.0 * cam.zoom).clamp(3.0, 16.0);
    let half = len * 0.45;
    let base = Point::new(tip.x - dx * len, tip.y - dy * len);
    let head = Path::new(|b| {
        b.move_to(tip);
        b.line_to(Point::new(base.x - dy * half, base.y + dx * half));
        b.line_to(Point::new(base.x + dy * half, base.y - dx * half));
        b.close();
    });
    frame.fill(&head, color);
}

fn draw_node(
    frame: &mut Frame,
    c: &DiagramCanvas<'_>,
    i: usize,
    rect: &archalign_layout::Rect,
    outline: Option<(Color, f32)>,
) {
    let cam = c.camera;
    let z = cam.zoom;
    let p = c.palette;
    let Some(comp) = c.doc.components.get(i) else {
        return;
    };
    let r = rect_on(cam, rect);
    let colors = p.component(comp.ty);
    let path = Path::rounded_rectangle(r.position(), r.size(), (6.0 * z).into());
    frame.fill(&path, p.mask);
    frame.fill(&path, colors.fill);
    frame.stroke(
        &path,
        Stroke::default().with_color(colors.stroke).with_width(1.2),
    );
    if let Some((color, width)) = outline {
        let o = Path::rounded_rectangle(
            Point::new(r.x - 3.0, r.y - 3.0),
            Size::new(r.width + 6.0, r.height + 6.0),
            (8.0 * z).into(),
        );
        frame.stroke(&o, Stroke::default().with_color(color).with_width(width));
    }
    // Text block, vertically centered.
    let mut lines: Vec<(String, f32, Color, bool)> = vec![(comp.label.clone(), 14.0, p.text, true)];
    if let Some(s) = comp.sublabel.as_deref().filter(|s| !s.is_empty()) {
        lines.push((s.to_string(), 12.0, p.text_muted, false));
    }
    if let Some(t) = comp.tag.as_deref().filter(|s| !s.is_empty()) {
        lines.push((t.to_string(), 11.0, colors.stroke, false));
    }
    if 11.0 * z >= MIN_TEXT_PX {
        let total: f32 = lines.iter().map(|l| l.1 * 1.32 * z).sum();
        let mut y = r.center_y() - total / 2.0;
        for (s, size, color, bold) in lines {
            let h = size * 1.32 * z;
            frame.fill_text(text(
                s,
                Point::new(r.center_x(), y + h / 2.0),
                size * z,
                color,
                bold,
            ));
            y += h;
        }
    }
    // Badges (FR-DIA-09).
    let b = c.badges.get(i).copied().unwrap_or_default();
    let mut bx = r.x + r.width - 4.0;
    let size = (10.0 * z).max(7.0);
    if b.broken {
        let w = size * 1.6;
        bx -= w;
        frame.fill_rectangle(
            Point::new(bx, r.y - size * 0.6),
            Size::new(w, size * 1.3),
            p.warning(),
        );
        frame.fill_text(text("!", Point::new(bx + w / 2.0, r.y), size, p.mask, true));
    }
    if b.modified {
        let w = size * 1.6;
        bx -= w + 2.0;
        frame.fill_rectangle(
            Point::new(bx, r.y - size * 0.6),
            Size::new(w, size * 1.3),
            p.accent(),
        );
        frame.fill_text(text("M", Point::new(bx + w / 2.0, r.y), size, p.mask, true));
    }
}

fn draw_legend(frame: &mut Frame, c: &DiagramCanvas<'_>) {
    if c.legend.is_empty() {
        return;
    }
    let p = c.palette;
    let row_h = 16.0;
    let w = 12.0
        + c.legend
            .iter()
            .map(|r| r.label.chars().count())
            .max()
            .unwrap_or(0) as f32
            * 6.6
        + 28.0;
    let h = 10.0 + row_h * c.legend.len() as f32;
    let origin = Point::new(10.0, frame.height() - h - 10.0);
    frame.fill_rectangle(origin, Size::new(w, h), with_alpha(p.mask, 0.85));
    for (k, row) in c.legend.iter().enumerate() {
        let y = origin.y + 5.0 + row_h * k as f32;
        let colors = p.component(row.ty);
        frame.fill_rectangle(
            Point::new(origin.x + 8.0, y + 3.0),
            Size::new(10.0, 10.0),
            colors.stroke,
        );
        let mut t = text(
            row.label.clone(),
            Point::new(origin.x + 24.0, y + row_h / 2.0),
            11.0,
            p.text,
            false,
        );
        t.align_x = iced::widget::text::Alignment::Left;
        frame.fill_text(t);
    }
}

/// Draws the overlay: reach dimming, selection, reverse highlight and hover outlines.
pub fn draw_overlay(frame: &mut Frame, c: &DiagramCanvas<'_>) {
    let p = c.palette;
    let accent = p.accent();
    if let Some(reach) = c.reach {
        // Dim everything, then redraw what the reach covers.
        frame.fill_rectangle(Point::ORIGIN, frame.size(), with_alpha(p.bg, 0.7));
        for e in &c.scene.edges {
            if reach.edges.contains(&e.index) {
                let pts: Vec<Point> = e
                    .route
                    .points
                    .iter()
                    .map(|q| c.camera.to_canvas(pt(*q)))
                    .collect();
                draw_edge(frame, &pts, e.route.arrow, accent, None, 2.5, c.camera);
            }
        }
        for n in &c.scene.nodes {
            if reach.nodes.contains(&n.index) {
                draw_node(frame, c, n.index, &n.rect, None);
            }
        }
    }
    if let Some(k) = c.selected_edge
        && let Some(e) = c.scene.edges.iter().find(|e| e.index == k)
    {
        let pts: Vec<Point> = e
            .route
            .points
            .iter()
            .map(|q| c.camera.to_canvas(pt(*q)))
            .collect();
        draw_edge(frame, &pts, e.route.arrow, accent, None, 3.0, c.camera);
        for end in [e.from, e.to] {
            if let Some(n) = c.scene.nodes.get(end) {
                draw_node(frame, c, end, &n.rect, Some((accent, 2.5)));
            }
        }
    }
    for &i in c.reverse {
        if let Some(n) = c.scene.nodes.get(i) {
            draw_node(frame, c, i, &n.rect, Some((accent, 2.0)));
        }
    }
    if let Some(i) = c.selected_node
        && let Some(n) = c.scene.nodes.get(i)
    {
        draw_node(frame, c, i, &n.rect, Some((accent, 3.0)));
    }
    if let Some(i) = c.hover.filter(|h| Some(*h) != c.selected_node)
        && let Some(n) = c.scene.nodes.get(i)
    {
        let r = rect_on(c.camera, &n.rect);
        frame.stroke_rectangle(
            Point::new(r.x - 2.0, r.y - 2.0),
            Size::new(r.width + 4.0, r.height + 4.0),
            Stroke::default()
                .with_color(with_alpha(p.text, 0.5))
                .with_width(1.0),
        );
    }
}

/// Builds both layers into fresh frames (used by the perf test; the widget caches the
/// static one).
pub fn build_geometry(renderer: &Renderer, size: Size, c: &DiagramCanvas<'_>) -> Vec<Geometry> {
    let mut stat = Frame::new(renderer, size);
    draw_static(&mut stat, c);
    let mut over = Frame::new(renderer, size);
    draw_overlay(&mut over, c);
    vec![stat.into_geometry(), over.into_geometry()]
}

impl canvas::Program<crate::app::Message> for DiagramCanvas<'_> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<crate::app::Message>> {
        let publish = |e: CanvasEvent| Some(Action::publish(crate::app::Message::Canvas(e)));
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                match cursor.position_in(bounds) {
                    Some(p) => {
                        state.focused = true;
                        let hit = self.hit(p);
                        if hit == Hit::Empty {
                            state.drag_from = Some(p);
                        }
                        publish(CanvasEvent::Pressed(hit)).map(Action::and_capture)
                    }
                    None => {
                        if state.focused {
                            state.focused = false;
                            return publish(CanvasEvent::Unfocused);
                        }
                        None
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.drag_from = None;
                None
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let p = cursor.position_in(bounds);
                if let (Some(from), Some(p)) = (state.drag_from, p) {
                    state.drag_from = Some(p);
                    return publish(CanvasEvent::Pan(p - from));
                }
                let hover = p.and_then(|p| match self.hit(p) {
                    Hit::Node(i) => Some(i),
                    _ => None,
                });
                if hover != state.hover {
                    state.hover = hover;
                    return publish(CanvasEvent::Hover(hover, p.unwrap_or(Point::ORIGIN)));
                }
                self.resized(state, bounds)
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let p = cursor.position_in(bounds)?;
                let (dx, dy) = match delta {
                    ScrollDelta::Lines { x, y } => (x * 40.0, y * 40.0),
                    ScrollDelta::Pixels { x, y } => (*x, *y),
                };
                let zoom = state.modifiers.command() || state.modifiers.control();
                let e = if zoom {
                    CanvasEvent::ZoomAt {
                        factor: (dy / 200.0).exp(),
                        anchor: p,
                    }
                } else {
                    CanvasEvent::Pan(Vector::new(dx, dy))
                };
                publish(e).map(Action::and_capture)
            }
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(m)) => {
                state.modifiers = *m;
                None
            }
            _ => self.resized(state, bounds),
        }
    }

    fn draw(
        &self,
        state: &CanvasState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let key = (
            self.version,
            self.theme,
            self.camera.zoom.to_bits(),
            self.camera.pan.x.to_bits(),
            self.camera.pan.y.to_bits(),
        );
        if state.key.get() != Some(key) {
            state.cache.clear();
            state.key.set(Some(key));
        }
        let stat = state
            .cache
            .draw(renderer, bounds.size(), |frame| draw_static(frame, self));
        let mut over = Frame::new(renderer, bounds.size());
        draw_overlay(&mut over, self);
        vec![stat, over.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag_from.is_some() {
            return mouse::Interaction::Grabbing;
        }
        match cursor.position_in(bounds).map(|p| self.hit(p)) {
            Some(Hit::Node(_) | Hit::Edge(_)) => mouse::Interaction::Pointer,
            Some(Hit::Empty) => mouse::Interaction::Grab,
            None => mouse::Interaction::default(),
        }
    }
}

impl DiagramCanvas<'_> {
    fn resized(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
    ) -> Option<Action<crate::app::Message>> {
        let size = bounds.size();
        if state.size != Some(size) && size.width > 0.0 && size.height > 0.0 {
            state.size = Some(size);
            return Some(Action::publish(crate::app::Message::Canvas(
                CanvasEvent::Resized(size),
            )));
        }
        None
    }
}

/// Whether the scene was laid out automatically (used for the strip).
pub fn is_auto(scene: &Scene) -> bool {
    scene.mode == SceneMode::Auto
}
