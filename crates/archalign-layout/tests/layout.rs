//! FR-LAY-01, 02, 03, 05, 06, 08.

mod common;

use archalign_layout::measure::auto_node_size;
use archalign_layout::route::{MARGIN, natural_sides};
use archalign_layout::{
    FixedAdvance, Mode, Point, Rect, Scene, SceneMode, layout_document, select_mode, write_back,
};
use archalign_model::{IssueCode, Side};
use common::{check_invariants, doc, fixture_root, load_fixture, on_border};
use serde_json::{Value, json};

fn comp(id: &str, extra: Value) -> Value {
    let mut c = json!({ "id": id, "type": "backend", "label": id.to_uppercase() });
    if let (Value::Object(c), Value::Object(e)) = (&mut c, extra) {
        c.extend(e);
    }
    c
}

fn arch(components: Vec<Value>, rest: Value) -> Value {
    let mut v = json!({
        "schema_version": 1, "diagram_type": "architecture",
        "meta": { "title": "t", "output": "t.html" },
        "components": components
    });
    if let (Value::Object(v), Value::Object(r)) = (&mut v, rest) {
        v.extend(r);
    }
    v
}

fn lay(v: Value) -> Scene {
    layout_document(&doc(v), &FixedAdvance::default())
}

// ---------- FR-LAY-01 ----------

#[test]
fn all_positioned_is_authored() {
    let d = doc(arch(
        vec![
            comp("a", json!({"pos": [0, 0]})),
            comp("b", json!({"pos": [300, 0], "size": [80, 40]})),
        ],
        json!({}),
    ));
    match select_mode(&d) {
        Mode::Authored(rects) => {
            assert_eq!(rects[0], Rect::new(0.0, 0.0, 120.0, 60.0));
            assert_eq!(rects[1], Rect::new(300.0, 0.0, 80.0, 40.0));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn all_on_grid_is_authored() {
    let d = doc(arch(
        vec![
            comp("a", json!({"row": 0, "col": 0})),
            comp("b", json!({"row": 1, "col": 2})),
        ],
        json!({"layout": {"mode": "grid"}}),
    ));
    let Mode::Authored(rects) = select_mode(&d) else {
        panic!()
    };
    assert_eq!(rects[0], Rect::new(40.0, 80.0, 120.0, 60.0));
    // origin + (2·(130+30), 1·(64+40))
    assert_eq!(rects[1], Rect::new(360.0, 184.0, 120.0, 60.0));
}

#[test]
fn mix_of_grid_and_pos_is_authored_and_pos_wins() {
    let d = doc(arch(
        vec![
            comp("a", json!({"row": 0, "col": 0})),
            comp("b", json!({"row": 3, "col": 3, "pos": [7, 9]})),
        ],
        json!({"layout": {"mode": "grid"}}),
    ));
    let Mode::Authored(rects) = select_mode(&d) else {
        panic!()
    };
    assert_eq!(rects[1].x, 7.0);
    assert_eq!(rects[1].y, 9.0);
}

#[test]
fn partially_positioned_is_auto_with_warning() {
    let v = arch(
        vec![comp("a", json!({"pos": [0, 0]})), comp("b", json!({}))],
        json!({}),
    );
    assert_eq!(select_mode(&doc(v.clone())), Mode::Auto { partial: true });
    let scene = lay(v);
    assert_eq!(scene.mode, SceneMode::Auto);
    assert_eq!(scene.warnings.len(), 1);
    assert_eq!(scene.warnings[0].code, IssueCode::PartialPositions);
    // A grid document where one component has neither pos nor a full cell.
    let g = arch(
        vec![
            comp("a", json!({"row": 0, "col": 0})),
            comp("b", json!({"row": 1})),
        ],
        json!({"layout": {"mode": "grid"}}),
    );
    assert_eq!(select_mode(&doc(g)), Mode::Auto { partial: true });
}

#[test]
fn none_positioned_is_auto_without_warning() {
    let v = arch(vec![comp("a", json!({})), comp("b", json!({}))], json!({}));
    assert_eq!(select_mode(&doc(v.clone())), Mode::Auto { partial: false });
    assert!(lay(v).warnings.is_empty());
    // row/col without a grid layout block are not positions.
    let g = arch(vec![comp("a", json!({"row": 0, "col": 0}))], json!({}));
    assert_eq!(select_mode(&doc(g)), Mode::Auto { partial: false });
}

// ---------- FR-LAY-02 ----------

#[test]
fn archify_repo_grid_fixture_resolves_to_expected_positions() {
    let d = load_fixture(&fixture_root().join("examples/archify-repo-grid.architecture.json"));
    let scene = layout_document(&d, &FixedAdvance::default());
    assert_eq!(scene.mode, SceneMode::Authored);
    // layout: origin [40,100], gapX 24, gapY 48, cellW 120, cellH 60 → step 144 × 108.
    let expect = [
        ("user", 40.0, 208.0),
        ("agents", 184.0, 208.0),
        ("skill", 184.0, 100.0),
        ("ir", 328.0, 208.0),
        ("schemas", 328.0, 100.0),
        ("renderers", 472.0, 208.0),
        ("template", 616.0, 208.0),
        ("checker", 760.0, 208.0),
        ("html", 904.0, 208.0),
    ];
    for (id, x, y) in expect {
        let r = scene.node(id).unwrap().rect;
        assert_eq!((r.x, r.y, r.w, r.h), (x, y, 120.0, 60.0), "{id}");
    }
}

// ---------- FR-LAY-03 ----------

#[test]
fn auto_sizes_are_exact_with_fixed_advance() {
    let m = FixedAdvance::default(); // 0.6 em advance, 1.32 em line height
    let d = doc(arch(
        vec![
            comp(
                "gw",
                json!({"label": "Gateway Service", "sublabel": "routes requests to backends", "tag": "v2"}),
            ),
            comp("a", json!({"label": "A"})),
            comp("w", json!({"label": "x".repeat(40)})),
        ],
        json!({}),
    ));
    // max(15·8.4, 27·7.2, 2·6.6) + 24 = 218.4 → 219; 18.48 + 15.84 + 14.52 + 20 = 68.84 → 69
    let s = auto_node_size(&d.components[0], &m);
    assert_eq!((s.w, s.h), (219.0, 69.0));
    // Minimum 120×60.
    let s = auto_node_size(&d.components[1], &m);
    assert_eq!((s.w, s.h), (120.0, 60.0));
    // 40·8.4 + 24 = 360
    let s = auto_node_size(&d.components[2], &m);
    assert_eq!((s.w, s.h), (360.0, 60.0));
    let scene = layout_document(&d, &m);
    assert_eq!(scene.nodes[0].rect.w, 219.0);
    assert_eq!(scene.nodes[0].rect.h, 69.0);
}

#[test]
fn authored_sizes_use_size_or_default() {
    let scene = lay(arch(
        vec![
            comp("a", json!({"pos": [0, 0], "size": [200, 90]})),
            comp("b", json!({"pos": [400, 0]})),
        ],
        json!({}),
    ));
    assert_eq!(scene.nodes[0].rect, Rect::new(0.0, 0.0, 200.0, 90.0));
    assert_eq!(scene.nodes[1].rect, Rect::new(400.0, 0.0, 120.0, 60.0));
}

// ---------- FR-LAY-05 ----------

fn two_nodes(conn: Value) -> Scene {
    lay(arch(
        vec![
            comp("a", json!({"pos": [0, 0]})),
            comp("b", json!({"pos": [400, 200]})),
        ],
        json!({ "connections": [conn] }),
    ))
}

#[test]
fn endpoints_sit_on_node_sides_and_arrow_is_at_target() {
    let scene = two_nodes(json!({"from": "a", "to": "b"}));
    let e = &scene.edges[0];
    let (a, b) = (scene.nodes[0].rect, scene.nodes[1].rect);
    let first = e.route.points[0];
    let last = *e.route.points.last().unwrap();
    assert_eq!(natural_sides(&a, &b), (Side::Right, Side::Left));
    assert_eq!(first.x, a.right());
    assert!(first.y > a.y && first.y < a.bottom());
    assert_eq!(last.x, b.x);
    assert!(last.y > b.y && last.y < b.bottom());
    assert_eq!(e.route.arrow.tip, last);
    // Pointing into the target's left side.
    assert_eq!(e.route.arrow.dir, Point::new(1.0, 0.0));
}

#[test]
fn authored_sides_are_honored() {
    let scene = two_nodes(json!({"from": "a", "to": "b", "fromSide": "bottom", "toSide": "top"}));
    let e = &scene.edges[0];
    let (a, b) = (scene.nodes[0].rect, scene.nodes[1].rect);
    let p = &e.route.points;
    assert_eq!(p[0].y, a.bottom());
    assert_eq!(p[1].x, p[0].x, "leaves the bottom side vertically");
    assert!(p[1].y > p[0].y);
    let last = *p.last().unwrap();
    assert_eq!(last.y, b.y);
    assert_eq!(e.route.arrow.dir, Point::new(0.0, 1.0));
}

#[test]
fn authored_via_points_are_visited_in_order() {
    let scene = two_nodes(json!({"from": "a", "to": "b", "via": [[200, -60], [200, 330]]}));
    let p = &scene.edges[0].route.points;
    let at = |x: f64, y: f64| p.iter().position(|q| q.x == x && q.y == y);
    let (i, j) = (at(200.0, -60.0).unwrap(), at(200.0, 330.0).unwrap());
    assert!(i < j);
}

#[test]
fn routes_avoid_nodes_that_are_not_endpoints() {
    let scene = lay(arch(
        vec![
            comp("a", json!({"pos": [0, 100]})),
            comp("blocker", json!({"pos": [250, 60], "size": [120, 140]})),
            comp("b", json!({"pos": [500, 100]})),
        ],
        json!({ "connections": [{"from": "a", "to": "b"}] }),
    ));
    check_invariants(&scene).unwrap();
    let blocker = scene.nodes[1].rect;
    for w in scene.edges[0].route.points.windows(2) {
        assert!(
            !blocker
                .inflate(MARGIN - 1.0)
                .segment_crosses_interior(w[0], w[1])
        );
    }
    assert!(
        scene.edges[0].route.points.len() > 2,
        "the route bends around"
    );
}

/// Smallest distance between parallel, overlapping segments of two routes.
fn min_parallel_gap(p: &[Point], q: &[Point]) -> f64 {
    let mut best = f64::INFINITY;
    for a in p.windows(2) {
        for b in q.windows(2) {
            let (ah, bh) = (a[0].y == a[1].y, b[0].y == b[1].y);
            if ah != bh {
                continue;
            }
            let (alo, ahi, blo, bhi, d) = if ah {
                (
                    a[0].x.min(a[1].x),
                    a[0].x.max(a[1].x),
                    b[0].x.min(b[1].x),
                    b[0].x.max(b[1].x),
                    (a[0].y - b[0].y).abs(),
                )
            } else {
                (
                    a[0].y.min(a[1].y),
                    a[0].y.max(a[1].y),
                    b[0].y.min(b[1].y),
                    b[0].y.max(b[1].y),
                    (a[0].x - b[0].x).abs(),
                )
            };
            if alo.max(blo) < ahi.min(bhi) {
                best = best.min(d);
            }
        }
    }
    best
}

#[test]
fn parallel_connections_are_offset_by_at_least_8px() {
    for (bx, by) in [(400, 0), (400, 200), (0, 300)] {
        let scene = lay(arch(
            vec![
                comp("a", json!({"pos": [0, 0]})),
                comp("b", json!({"pos": [bx, by]})),
            ],
            json!({ "connections": [
                {"from": "a", "to": "b"}, {"from": "a", "to": "b"}, {"from": "b", "to": "a"}
            ] }),
        ));
        let r: Vec<_> = scene.edges.iter().map(|e| e.route.points.clone()).collect();
        for i in 0..r.len() {
            for j in (i + 1)..r.len() {
                let gap = min_parallel_gap(&r[i], &r[j]);
                assert!(gap >= 8.0, "b at ({bx},{by}): edges {i},{j} gap {gap}");
            }
        }
    }
}

#[test]
fn self_loop_is_a_rectangle_on_the_right_side() {
    let scene = lay(arch(
        vec![comp("a", json!({"pos": [0, 0]}))],
        json!({ "connections": [{"from": "a", "to": "a"}] }),
    ));
    let r = scene.nodes[0].rect;
    let p = &scene.edges[0].route.points;
    assert_eq!(p.len(), 4);
    assert_eq!(p[0].x, r.right());
    assert_eq!(p[3].x, r.right());
    assert!(p[1].x > r.right() && p[2].x > r.right());
    assert!(p.iter().all(|q| q.y > r.y && q.y < r.bottom()));
    assert_eq!(scene.edges[0].route.arrow.dir, Point::new(-1.0, 0.0));
}

#[test]
fn label_is_centered_on_the_longest_segment() {
    let scene = two_nodes(json!({"from": "a", "to": "b", "label": "calls"}));
    let p = &scene.edges[0].route.points;
    let (k, _) = p
        .windows(2)
        .enumerate()
        .map(|(k, w)| (k, (w[1].x - w[0].x).abs() + (w[1].y - w[0].y).abs()))
        .fold(
            (0, -1.0),
            |best, cur| if cur.1 > best.1 { cur } else { best },
        );
    let mid = Point::new((p[k].x + p[k + 1].x) / 2.0, (p[k].y + p[k + 1].y) / 2.0);
    assert_eq!(scene.edges[0].route.label_anchor, mid);
}

#[test]
fn unknown_endpoints_are_skipped_without_panic() {
    let d = doc(arch(
        vec![comp("a", json!({"pos": [0, 0]}))],
        json!({ "connections": [{"from": "a", "to": "ghost"}] }),
    ));
    assert!(
        layout_document(&d, &FixedAdvance::default())
            .edges
            .is_empty()
    );
}

// ---------- FR-LAY-06 ----------

#[test]
fn boundary_uses_pad_on_every_side() {
    let scene = lay(arch(
        vec![
            comp("a", json!({"pos": [100, 100]})),
            comp("b", json!({"pos": [300, 200]})),
        ],
        json!({ "boundaries": [{"kind": "region", "label": "R", "wraps": ["a", "b"], "pad": 12}] }),
    ));
    let b = &scene.boundaries[0];
    assert_eq!(b.rect, Rect::new(88.0, 88.0, 344.0, 184.0));
    assert!(b.label_anchor.x > b.rect.x && b.label_anchor.y > b.rect.y);
    assert!(b.label_anchor.x < b.rect.x + 20.0 && b.label_anchor.y < b.rect.y + 20.0);
}

#[test]
fn boundary_default_padding_is_30_sides_50_bottom() {
    let scene = lay(arch(
        vec![comp("a", json!({"pos": [100, 100]}))],
        json!({ "boundaries": [{"kind": "security-group", "label": "S", "wraps": ["a"]}] }),
    ));
    assert_eq!(
        scene.boundaries[0].rect,
        Rect::new(70.0, 70.0, 180.0, 140.0)
    );
}

#[test]
fn nested_boundary_sits_inside_its_parent() {
    let scene = lay(arch(
        vec![
            comp("a", json!({"pos": [100, 100]})),
            comp("b", json!({"pos": [300, 100]})),
        ],
        json!({ "boundaries": [
            {"kind": "region", "label": "Outer", "wraps": ["a", "b"]},
            {"kind": "security-group", "label": "Inner", "wraps": ["b"]}
        ] }),
    ));
    let outer = scene.boundaries.iter().find(|b| b.index == 0).unwrap();
    let inner = scene.boundaries.iter().find(|b| b.index == 1).unwrap();
    assert_eq!(inner.parent, Some(0));
    assert_eq!(inner.depth, 1);
    assert_eq!(inner.rect, Rect::new(270.0, 70.0, 180.0, 140.0));
    // Outer wraps a and the inner rectangle, plus its own padding.
    assert_eq!(outer.rect, Rect::new(70.0, 40.0, 410.0, 220.0));
    assert!(outer.rect.contains_rect(&inner.rect));
}

// ---------- FR-LAY-08 ----------

#[test]
fn write_back_sets_integer_pos_and_size_and_nothing_else() {
    let mut d = load_fixture(&fixture_root().join("skill-examples/web-app.architecture.json"));
    archalign_model::strip_geometry(&mut d);
    let before = d.clone();
    let scene = layout_document(&d, &FixedAdvance::default());
    assert_eq!(d, before, "layout never mutates the document");
    assert_eq!(scene.mode, SceneMode::Auto);
    write_back(&mut d, &scene);
    let v = serde_json::to_value(&d).unwrap();
    for c in v["components"].as_array().unwrap() {
        for k in ["pos", "size"] {
            let pair = c[k].as_array().unwrap();
            assert!(pair.iter().all(|n| n.is_i64() || n.is_u64()), "{c}");
        }
    }
    // Clearing pos/size again gives back the input.
    let mut again = d.clone();
    for c in &mut again.components {
        c.pos = None;
        c.size = None;
    }
    assert_eq!(again, before);
    // The written geometry reopens as an authored layout with the same boxes.
    let reopened = layout_document(&d, &FixedAdvance::default());
    assert_eq!(reopened.mode, SceneMode::Authored);
    for (x, y) in reopened.nodes.iter().zip(&scene.nodes) {
        assert_eq!(x.rect, y.rect);
    }
}

#[test]
fn every_fixture_satisfies_invariants_when_auto_laid_out() {
    for path in common::fixtures() {
        let mut d = load_fixture(&path);
        archalign_model::strip_geometry(&mut d);
        let scene = layout_document(&d, &FixedAdvance::default());
        check_invariants(&scene).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for e in &scene.edges {
            let first = e.route.points[0];
            let last = *e.route.points.last().unwrap();
            assert!(on_border(&scene.nodes[e.from].rect, first.x, first.y));
            assert!(on_border(&scene.nodes[e.to].rect, last.x, last.y));
        }
    }
}

fn chain_or_fan(n: usize, fan: bool) -> archalign_layout::LayoutGraph {
    archalign_layout::LayoutGraph {
        nodes: vec![archalign_layout::Size { w: 160.0, h: 60.0 }; n],
        node_parent: vec![None; n],
        cluster_parent: vec![],
        edges: (1..n)
            .map(|i| if fan { (0, i) } else { (i - 1, i) })
            .collect(),
    }
}

fn placed(
    g: &archalign_layout::LayoutGraph,
    direction: archalign_layout::RankDirection,
) -> archalign_layout::Placement {
    use archalign_layout::LayoutEngine;
    archalign_layout::DugongEngine {
        direction,
        ..Default::default()
    }
    .place(g)
    .unwrap()
}

/// FR-LAY-04 (ADR 0035): the direction that fits a 16:10 viewport at the larger scale wins.
#[test]
fn auto_direction_picks_the_better_fitting_rank_direction() {
    use archalign_layout::{AutoDirectionEngine, LayoutEngine, fit_scale};
    let auto = AutoDirectionEngine::default();
    use archalign_layout::RankDirection::{LeftToRight, TopToBottom};
    // A 10-node chain is long along its ranks: top to bottom fits better (160×60 boxes).
    let chain = chain_or_fan(10, false);
    assert!(
        fit_scale(&placed(&chain, TopToBottom), &chain.nodes)
            > fit_scale(&placed(&chain, LeftToRight), &chain.nodes)
    );
    assert_eq!(auto.place(&chain).unwrap(), placed(&chain, TopToBottom));
    // A root with 8 children is wide across its ranks: left to right fits better.
    let fan = chain_or_fan(9, true);
    assert!(
        fit_scale(&placed(&fan, LeftToRight), &fan.nodes)
            > fit_scale(&placed(&fan, TopToBottom), &fan.nodes)
    );
    assert_eq!(auto.place(&fan).unwrap(), placed(&fan, LeftToRight));
    // A single node is a tie: left to right.
    let one = chain_or_fan(1, false);
    assert_eq!(
        auto.place(&one).unwrap(),
        archalign_layout::DugongEngine::default()
            .place(&one)
            .unwrap()
    );
}

/// NFR-QUAL-04: coordinates beyond `MAX_COORD` (or overflowing `f32`) are treated as
/// missing instead of reaching the router and the canvas.
#[test]
fn out_of_range_geometry_is_ignored_not_fatal() {
    for pos in [json!([1e39, 0]), json!([1e308, 0])] {
        let v = arch(
            vec![
                comp("a", json!({ "pos": pos, "size": [1e308, 100] })),
                comp("b", json!({ "pos": [300, 40] })),
            ],
            json!({
                "connections": [{ "from": "a", "to": "b", "via": [[1e300, 5]] }],
                "boundaries": [{ "kind": "region", "label": "G", "wraps": ["a", "b"], "pad": 1e39 }]
            }),
        );
        let s = lay(v);
        assert_eq!(s.mode, archalign_layout::scene::SceneMode::Auto);
        assert!(
            s.warnings
                .iter()
                .any(|w| w.code == IssueCode::PartialPositions)
        );
        for n in &s.nodes {
            assert!(
                [n.rect.x, n.rect.y, n.rect.w, n.rect.h]
                    .iter()
                    .all(|v| v.abs() < 1e6)
            );
        }
        for b in &s.boundaries {
            assert!(b.rect.w < 1e6, "{:?}", b.rect);
        }
        assert!(s.bounds.w < 1e6 && s.bounds.h < 1e6);
    }
}
