//! Orthogonal edge routing (FR-LAY-05, ADR 0018).
//!
//! Every connection becomes axis-aligned segments between ports on node sides:
//!
//! 1. Sides come from authored `fromSide`/`toSide`, else from the relative position of the
//!    two boxes.
//! 2. Ports are spread evenly along each side, ordered by the far end, so connections that
//!    share a side (including parallel connections) get distinct, ordered ports.
//! 3. Each route runs A* over a sparse grid made of the inflated node edges, the port lines
//!    and the corridor midlines. Every node is an obstacle inflated by [`MARGIN`], so a route
//!    never enters a node other than at its own ports. The cost is length plus a bend
//!    penalty plus a penalty for reusing a grid segment another route already uses.
//! 4. Authored `via` points are visited in order.
//! 5. A self-loop is a small rectangle on the node's right side.
//!
//! The label anchor is the midpoint of the longest segment; the arrowhead sits at the target.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use archalign_model::Side;
use serde::Serialize;

use crate::geom::{Point, Rect};

/// Clearance kept between a route and any node.
pub const MARGIN: f64 = 10.0;
/// Extent of a self-loop to the right of its node.
pub const SELF_LOOP_EXTENT: f64 = 24.0;
/// Extra extent for each further self-loop on the same node.
pub const SELF_LOOP_STEP: f64 = 8.0;
/// Further self-loops beyond this many reuse the outermost loop, so a loop never reaches
/// into the next rank (whose gap is far wider than 24 + 3·8 px).
pub const SELF_LOOP_MAX_NESTING: usize = 3;
/// Cost of one bend, in pixels of length.
const BEND_COST: f64 = 40.0;
/// Extra cost per pixel of a grid segment already used by another route.
const OVERLAP_COST: f64 = 3.0;
/// Cost scale for integer priorities.
const SCALE: f64 = 16.0;

/// A connection to route, in node indices.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeRequest {
    /// Source node.
    pub from: usize,
    /// Target node.
    pub to: usize,
    /// Authored source side.
    pub from_side: Option<Side>,
    /// Authored target side.
    pub to_side: Option<Side>,
    /// Authored waypoints.
    pub via: Vec<Point>,
}

/// The arrowhead at the target: its tip and the direction it points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Arrow {
    /// Tip, on the target's side.
    pub tip: Point,
    /// Unit direction of travel at the tip.
    pub dir: Point,
}

/// A routed connection.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Route {
    /// Polyline from the source port to the target port; consecutive points differ in
    /// exactly one coordinate.
    pub points: Vec<Point>,
    /// Midpoint of the longest segment.
    pub label_anchor: Point,
    /// Arrowhead at the target.
    pub arrow: Arrow,
}

fn normal(side: Side) -> Point {
    match side {
        Side::Left => Point::new(-1.0, 0.0),
        Side::Right => Point::new(1.0, 0.0),
        Side::Top => Point::new(0.0, -1.0),
        Side::Bottom => Point::new(0.0, 1.0),
    }
}

fn is_horizontal_side(side: Side) -> bool {
    matches!(side, Side::Left | Side::Right)
}

fn opposite(side: Side) -> Side {
    match side {
        Side::Left => Side::Right,
        Side::Right => Side::Left,
        Side::Top => Side::Bottom,
        Side::Bottom => Side::Top,
    }
}

/// The natural sides for a connection from `a` to `b`.
pub fn natural_sides(a: &Rect, b: &Rect) -> (Side, Side) {
    if b.x >= a.right() {
        (Side::Right, Side::Left)
    } else if b.right() <= a.x {
        (Side::Left, Side::Right)
    } else if b.y >= a.bottom() {
        (Side::Bottom, Side::Top)
    } else if b.bottom() <= a.y {
        (Side::Top, Side::Bottom)
    } else {
        let (ca, cb) = (a.center(), b.center());
        let (dx, dy) = (cb.x - ca.x, cb.y - ca.y);
        if dx.abs() >= dy.abs() {
            if dx >= 0.0 {
                (Side::Right, Side::Left)
            } else {
                (Side::Left, Side::Right)
            }
        } else if dy >= 0.0 {
            (Side::Bottom, Side::Top)
        } else {
            (Side::Top, Side::Bottom)
        }
    }
}

fn sides_for(req: &EdgeRequest, rects: &[Rect]) -> (Side, Side) {
    let (a, b) = (&rects[req.from], &rects[req.to]);
    match (req.from_side, req.to_side) {
        (Some(f), Some(t)) => (f, t),
        (Some(f), None) => (f, natural_sides(a, b).1),
        (None, Some(t)) => (natural_sides(a, b).0, t),
        (None, None) => natural_sides(a, b),
    }
}

fn port_point(rect: &Rect, side: Side, t: f64) -> Point {
    match side {
        Side::Left => Point::new(rect.x, rect.y + rect.h * t),
        Side::Right => Point::new(rect.right(), rect.y + rect.h * t),
        Side::Top => Point::new(rect.x + rect.w * t, rect.y),
        Side::Bottom => Point::new(rect.x + rect.w * t, rect.bottom()),
    }
}

/// Endpoint record used for port assignment.
struct End {
    edge: usize,
    is_source: bool,
    node: usize,
    side: Side,
    key: f64,
}

/// Ports for every non-self-loop edge: `(source port, source side, target port, target side)`.
fn assign_ports(
    rects: &[Rect],
    edges: &[EdgeRequest],
    sides: &[(Side, Side)],
) -> Vec<(Point, Side, Point, Side)> {
    let mut ends: Vec<End> = Vec::new();
    for (k, e) in edges.iter().enumerate() {
        if e.from == e.to {
            continue;
        }
        let (fs, ts) = sides[k];
        let target_c = e.via.first().copied().unwrap_or(rects[e.to].center());
        let source_c = e.via.last().copied().unwrap_or(rects[e.from].center());
        let key = |side: Side, p: Point| if is_horizontal_side(side) { p.y } else { p.x };
        ends.push(End {
            edge: k,
            is_source: true,
            node: e.from,
            side: fs,
            key: key(fs, target_c),
        });
        ends.push(End {
            edge: k,
            is_source: false,
            node: e.to,
            side: ts,
            key: key(ts, source_c),
        });
    }
    ends.sort_by(|a, b| {
        (a.node, side_rank(a.side))
            .cmp(&(b.node, side_rank(b.side)))
            .then(a.key.total_cmp(&b.key))
            .then(a.edge.cmp(&b.edge))
            .then(b.is_source.cmp(&a.is_source))
    });
    let mut out = vec![
        (
            Point::new(0.0, 0.0),
            Side::Right,
            Point::new(0.0, 0.0),
            Side::Left
        );
        edges.len()
    ];
    let mut i = 0;
    while i < ends.len() {
        let mut j = i;
        while j < ends.len() && ends[j].node == ends[i].node && ends[j].side == ends[i].side {
            j += 1;
        }
        let n = (j - i) as f64;
        for (slot, end) in ends[i..j].iter().enumerate() {
            let t = (slot as f64 + 1.0) / (n + 1.0);
            let p = port_point(&rects[end.node], end.side, t);
            if end.is_source {
                out[end.edge].0 = p;
                out[end.edge].1 = end.side;
            } else {
                out[end.edge].2 = p;
                out[end.edge].3 = end.side;
            }
        }
        i = j;
    }
    out
}

fn side_rank(side: Side) -> u8 {
    match side {
        Side::Left => 0,
        Side::Right => 1,
        Side::Top => 2,
        Side::Bottom => 3,
    }
}

/// Sorted, deduplicated coordinates.
fn dedup_sorted(mut v: Vec<f64>) -> Vec<f64> {
    v.retain(|x| x.is_finite());
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    v
}

fn with_midlines(base: Vec<f64>, extra: Vec<f64>) -> Vec<f64> {
    let base = dedup_sorted(base);
    let mut all = base.clone();
    for w in base.windows(2) {
        let gap = w[1] - w[0];
        if gap > 4.0 {
            all.push((w[0] + w[1]) / 2.0);
        }
        if gap > 64.0 {
            all.push(w[0] + gap / 4.0);
            all.push(w[0] + 3.0 * gap / 4.0);
        }
    }
    all.extend(extra);
    dedup_sorted(all)
}

/// The sparse routing grid with obstacle blocking and per-segment usage.
struct Grid {
    xs: Vec<f64>,
    ys: Vec<f64>,
    /// Horizontal segment (i..i+1) on row j is blocked: index `j * (nx-1) + i`.
    hblock: Vec<bool>,
    /// Vertical segment (j..j+1) on column i is blocked: index `i * (ny-1) + j`.
    vblock: Vec<bool>,
    husage: Vec<u16>,
    vusage: Vec<u16>,
    // A* scratch, reused across searches with a generation stamp.
    best: Vec<u32>,
    stamp: Vec<u32>,
    prev: Vec<u32>,
    generation: u32,
}

const NO_PREV: u32 = u32::MAX;

fn min_max<'a>(vals: impl Iterator<Item = &'a f64>) -> (f64, f64) {
    vals.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| {
        (a.min(x), b.max(x))
    })
}

impl Grid {
    fn new(obstacles: &[Rect], extra_x: Vec<f64>, extra_y: Vec<f64>) -> Self {
        let mut xs = Vec::with_capacity(obstacles.len() * 2 + 2);
        let mut ys = Vec::with_capacity(obstacles.len() * 2 + 2);
        for r in obstacles {
            xs.push(r.x);
            xs.push(r.right());
            ys.push(r.y);
            ys.push(r.bottom());
        }
        let (minx, maxx) = min_max(xs.iter().chain(extra_x.iter()));
        let (miny, maxy) = min_max(ys.iter().chain(extra_y.iter()));
        if minx.is_finite() && miny.is_finite() {
            xs.push(minx - 2.0 * MARGIN);
            xs.push(maxx + 2.0 * MARGIN);
            ys.push(miny - 2.0 * MARGIN);
            ys.push(maxy + 2.0 * MARGIN);
        }
        let xs = with_midlines(xs, extra_x);
        let ys = with_midlines(ys, extra_y);
        let (nx, ny) = (xs.len(), ys.len());
        let mut hblock = vec![false; ny * nx.saturating_sub(1)];
        let mut vblock = vec![false; nx * ny.saturating_sub(1)];
        let idx = |v: &[f64], x: f64| v.partition_point(|&a| a < x - 1e-6);
        for r in obstacles {
            let (x0, x1) = (idx(&xs, r.x), idx(&xs, r.right()));
            let (y0, y1) = (idx(&ys, r.y), idx(&ys, r.bottom()));
            // Rows strictly inside, columns spanning.
            for j in (y0 + 1)..y1 {
                for i in x0..x1 {
                    hblock[j * (nx - 1) + i] = true;
                }
            }
            for i in (x0 + 1)..x1 {
                for j in y0..y1 {
                    vblock[i * (ny - 1) + j] = true;
                }
            }
        }
        let states = nx * ny * 5;
        Self {
            hblock,
            vblock,
            husage: vec![0; ny * nx.saturating_sub(1)],
            vusage: vec![0; nx * ny.saturating_sub(1)],
            best: vec![0; states],
            stamp: vec![0; states],
            prev: vec![NO_PREV; states],
            generation: 0,
            xs,
            ys,
        }
    }

    fn xi(&self, x: f64) -> Option<usize> {
        let i = self.xs.partition_point(|&a| a < x - 1e-6);
        (i < self.xs.len() && (self.xs[i] - x).abs() < 1e-6).then_some(i)
    }

    fn yi(&self, y: f64) -> Option<usize> {
        let j = self.ys.partition_point(|&a| a < y - 1e-6);
        (j < self.ys.len() && (self.ys[j] - y).abs() < 1e-6).then_some(j)
    }

    /// A* from `start` to `goal`. `start_dir`/`goal_dir` are preferred travel directions
    /// (0 = +x, 1 = -x, 2 = +y, 3 = -y); turning away from them costs a bend.
    fn search(
        &mut self,
        start: Point,
        start_dir: Option<u8>,
        goal: Point,
        goal_dir: Option<u8>,
    ) -> Option<Vec<Point>> {
        let (si, sj) = (self.xi(start.x)?, self.yi(start.y)?);
        let (gi, gj) = (self.xi(goal.x)?, self.yi(goal.y)?);
        let (nx, ny) = (self.xs.len(), self.ys.len());
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.stamp.iter_mut().for_each(|s| *s = 0);
            self.generation = 1;
        }
        let state = |i: usize, j: usize, d: u8| (j * nx + i) * 5 + d as usize;
        // Manhattan distance plus a lower bound on the bends still needed.
        let h = |i: usize, j: usize, d: u8, xs: &[f64], ys: &[f64]| {
            let dx = xs[gi] - xs[i];
            let dy = ys[gj] - ys[j];
            let away = |dir: u8| match dir {
                0 => dx < 0.0,
                1 => dx > 0.0,
                2 => dy < 0.0,
                3 => dy > 0.0,
                _ => false,
            };
            let horizontal = d < 2;
            let bends = if dx == 0.0 && dy == 0.0 || d == 4 {
                0.0
            } else if dx != 0.0 && dy != 0.0 {
                if away(d) { 2.0 } else { 1.0 }
            } else if (dx != 0.0) == horizontal {
                // Moving along the remaining axis: straight on, or a U-turn.
                if away(d) { 2.0 } else { 0.0 }
            } else {
                1.0
            };
            (dx.abs() + dy.abs() + bends * BEND_COST) * SCALE
        };
        let mut heap = BinaryHeap::new();
        let d0 = start_dir.unwrap_or(4);
        let s0 = state(si, sj, d0);
        self.best[s0] = 0;
        self.stamp[s0] = self.generation;
        self.prev[s0] = NO_PREV;
        // Ties on f prefer the larger g (deeper states), which explores far fewer states.
        heap.push(Reverse((
            h(si, sj, d0, &self.xs, &self.ys) as u32,
            Reverse(0u32),
            s0 as u32,
        )));
        let mut found: Option<usize> = None;
        while let Some(Reverse((_, Reverse(g), s))) = heap.pop() {
            let s = s as usize;
            if self.stamp[s] == self.generation && g > self.best[s] {
                continue;
            }
            let d = (s % 5) as u8;
            let cell = s / 5;
            let (i, j) = (cell % nx, cell / nx);
            if i == gi && j == gj {
                // The goal-direction bend is charged on the move into the goal, so the first
                // goal state popped is the cheapest.
                found = Some(s);
                break;
            }
            for nd in 0u8..4 {
                let (ni, nj) = match nd {
                    0 if i + 1 < nx && !self.hblock[j * (nx - 1) + i] => (i + 1, j),
                    1 if i > 0 && !self.hblock[j * (nx - 1) + i - 1] => (i - 1, j),
                    2 if j + 1 < ny && !self.vblock[i * (ny - 1) + j] => (i, j + 1),
                    3 if j > 0 && !self.vblock[i * (ny - 1) + j - 1] => (i, j - 1),
                    _ => continue,
                };
                // No immediate reversal.
                if d < 4 && (d ^ 1) == nd {
                    continue;
                }
                let (len, usage) = match nd {
                    0 => (self.xs[i + 1] - self.xs[i], self.husage[j * (nx - 1) + i]),
                    1 => (
                        self.xs[i] - self.xs[i - 1],
                        self.husage[j * (nx - 1) + i - 1],
                    ),
                    2 => (self.ys[j + 1] - self.ys[j], self.vusage[i * (ny - 1) + j]),
                    _ => (
                        self.ys[j] - self.ys[j - 1],
                        self.vusage[i * (ny - 1) + j - 1],
                    ),
                };
                let mut cost = len + OVERLAP_COST * len * f64::from(usage);
                if d < 4 && d != nd {
                    cost += BEND_COST;
                }
                if ni == gi && nj == gj && goal_dir.is_some_and(|gd| gd != nd) {
                    cost += BEND_COST;
                }
                let ng = g.saturating_add((cost * SCALE) as u32);
                let ns = state(ni, nj, nd);
                if self.stamp[ns] != self.generation || ng < self.best[ns] {
                    self.stamp[ns] = self.generation;
                    self.best[ns] = ng;
                    self.prev[ns] = s as u32;
                    let f = ng.saturating_add(h(ni, nj, nd, &self.xs, &self.ys) as u32);
                    heap.push(Reverse((f, Reverse(ng), ns as u32)));
                }
            }
        }
        let mut s = found?;
        let mut cells = Vec::new();
        loop {
            let cell = s / 5;
            cells.push((cell % nx, cell / nx));
            let p = self.prev[s];
            if p == NO_PREV {
                break;
            }
            s = p as usize;
        }
        cells.reverse();
        // Record usage.
        for w in cells.windows(2) {
            let ((i0, j0), (i1, j1)) = (w[0], w[1]);
            if j0 == j1 {
                let i = i0.min(i1);
                self.husage[j0 * (nx - 1) + i] = self.husage[j0 * (nx - 1) + i].saturating_add(1);
            } else {
                let j = j0.min(j1);
                self.vusage[i0 * (ny - 1) + j] = self.vusage[i0 * (ny - 1) + j].saturating_add(1);
            }
        }
        Some(
            cells
                .into_iter()
                .map(|(i, j)| Point::new(self.xs[i], self.ys[j]))
                .collect(),
        )
    }
}

fn dir_code(v: Point) -> u8 {
    if v.x > 0.0 {
        0
    } else if v.x < 0.0 {
        1
    } else if v.y > 0.0 {
        2
    } else {
        3
    }
}

/// Removes repeated and collinear interior points.
pub fn simplify(points: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        if out
            .last()
            .is_some_and(|q| (q.x - p.x).abs() < 1e-9 && (q.y - p.y).abs() < 1e-9)
        {
            continue;
        }
        if out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            let collinear = ((a.x - b.x).abs() < 1e-9 && (b.x - p.x).abs() < 1e-9)
                || ((a.y - b.y).abs() < 1e-9 && (b.y - p.y).abs() < 1e-9);
            if collinear {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

/// An L-shaped orthogonal leg, used only when the grid search finds nothing.
fn l_leg(a: Point, b: Point) -> Vec<Point> {
    vec![a, Point::new(b.x, a.y), b]
}

fn finish(points: Vec<Point>) -> Route {
    let points = simplify(points);
    let mut best = (0usize, -1.0f64);
    for (k, w) in points.windows(2).enumerate() {
        let len = (w[1].x - w[0].x).abs() + (w[1].y - w[0].y).abs();
        if len > best.1 + 1e-9 {
            best = (k, len);
        }
    }
    let (a, b) = if points.len() >= 2 {
        (points[best.0], points[best.0 + 1])
    } else {
        (points[0], points[0])
    };
    let label_anchor = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let n = points.len();
    let (p, q) = if n >= 2 {
        (points[n - 2], points[n - 1])
    } else {
        (points[0], points[0])
    };
    let len = ((q.x - p.x).powi(2) + (q.y - p.y).powi(2)).sqrt();
    let dir = if len > 0.0 {
        Point::new((q.x - p.x) / len, (q.y - p.y) / len)
    } else {
        Point::new(1.0, 0.0)
    };
    Route {
        label_anchor,
        arrow: Arrow { tip: q, dir },
        points,
    }
}

/// The self-loop on the right side of `rect`; `nth` counts earlier loops on the same node.
pub fn self_loop(rect: &Rect, nth: usize) -> Route {
    let k = nth.min(SELF_LOOP_MAX_NESTING) as f64;
    let e = SELF_LOOP_EXTENT + SELF_LOOP_STEP * k;
    // Nested loops also spread vertically, staying inside the node's height.
    let spread = (rect.h * 0.2 + 4.0 * k).min(rect.h * 0.45);
    let cy = rect.y + rect.h / 2.0;
    let (y1, y2) = (cy - spread, cy + spread);
    let x = rect.right();
    finish(vec![
        Point::new(x, y1),
        Point::new(x + e, y1),
        Point::new(x + e, y2),
        Point::new(x, y2),
    ])
}

/// Routes every edge over the node rectangles.
pub fn route_all(rects: &[Rect], edges: &[EdgeRequest]) -> Vec<Route> {
    let sides: Vec<(Side, Side)> = edges
        .iter()
        .map(|e| {
            if e.from == e.to {
                (Side::Right, Side::Right)
            } else {
                sides_for(e, rects)
            }
        })
        .collect();
    let ports = assign_ports(rects, edges, &sides);
    let obstacles: Vec<Rect> = rects.iter().map(|r| r.inflate(MARGIN)).collect();
    let stub = |p: Point, side: Side| {
        let n = normal(side);
        Point::new(p.x + n.x * MARGIN, p.y + n.y * MARGIN)
    };
    let mut extra_x = Vec::new();
    let mut extra_y = Vec::new();
    for (k, e) in edges.iter().enumerate() {
        if e.from == e.to {
            continue;
        }
        let (sp, ss, tp, ts) = ports[k];
        for p in [stub(sp, ss), stub(tp, ts)]
            .into_iter()
            .chain(e.via.iter().copied())
        {
            extra_x.push(p.x);
            extra_y.push(p.y);
        }
    }
    let mut grid = Grid::new(&obstacles, extra_x, extra_y);
    let mut loops_seen = vec![0usize; rects.len()];
    edges
        .iter()
        .enumerate()
        .map(|(k, e)| {
            if e.from == e.to {
                let nth = loops_seen[e.from];
                loops_seen[e.from] += 1;
                return self_loop(&rects[e.from], nth);
            }
            let (sp, ss, tp, ts) = ports[k];
            let (s0, t0) = (stub(sp, ss), stub(tp, ts));
            let mut waypoints = vec![s0];
            waypoints.extend(e.via.iter().copied());
            waypoints.push(t0);
            let mut points = vec![sp];
            let last = waypoints.len() - 1;
            for w in 0..last {
                let (a, b) = (waypoints[w], waypoints[w + 1]);
                let start_dir = (w == 0).then(|| dir_code(normal(ss)));
                let goal_dir = (w + 1 == last).then(|| dir_code(normal(opposite(ts))));
                match grid.search(a, start_dir, b, goal_dir) {
                    Some(leg) => points.extend(leg),
                    None => points.extend(l_leg(a, b)),
                }
            }
            points.push(tp);
            finish(points)
        })
        .collect()
}
