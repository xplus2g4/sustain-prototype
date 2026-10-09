//! FR-PANE-03 (move and split) and FR-PANE-04 (resize).

use archalign_desktop::panes::{MIN_PANE, PaneKind, PaneLayout};
use iced::Size;
use iced::widget::pane_grid::{Axis, DragEvent, Edge, Node, Region, ResizeEvent, Target};

const GRID: Size = Size::new(1440.0, 830.0);

/// A compact description of the layout tree, e.g. `V(Tree,V(Diagram,Source))`.
fn shape(layout: &PaneLayout) -> String {
    fn walk(layout: &PaneLayout, node: &Node) -> String {
        match node {
            Node::Split { axis, a, b, .. } => format!(
                "{}({},{})",
                if *axis == Axis::Vertical { "V" } else { "H" },
                walk(layout, a),
                walk(layout, b)
            ),
            Node::Pane(p) => layout
                .state
                .get(*p)
                .map(|k| k.name().to_string())
                .unwrap_or_default(),
        }
    }
    walk(layout, layout.state.layout())
}

fn pane(layout: &PaneLayout, kind: PaneKind) -> iced::widget::pane_grid::Pane {
    layout.pane_of(kind).unwrap()
}

fn drop_on(layout: &mut PaneLayout, kind: PaneKind, target: Target) {
    let p = pane(layout, kind);
    // The same operation through pane_grid's own semantics, for comparison.
    let mut expected = layout.state.clone();
    expected.drop(p, target);
    layout.drag(DragEvent::Picked { pane: p });
    layout.drag(DragEvent::Dropped { pane: p, target });
    let exp = PaneLayout { state: expected };
    assert_eq!(shape(layout), shape(&exp), "matches pane_grid::State::drop");
}

#[test]
fn default_layout_shape() {
    assert_eq!(shape(&PaneLayout::default()), "V(Tree,V(Diagram,Source))");
}

#[test]
fn dropping_on_an_edge_region_splits_that_pane_on_that_side() {
    let mut l = PaneLayout::default();
    let tree = pane(&l, PaneKind::Tree);
    drop_on(
        &mut l,
        PaneKind::Source,
        Target::Pane(tree, Region::Edge(Edge::Bottom)),
    );
    assert_eq!(shape(&l), "V(H(Tree,Source),Diagram)");

    let mut l = PaneLayout::default();
    let diagram = pane(&l, PaneKind::Diagram);
    drop_on(
        &mut l,
        PaneKind::Tree,
        Target::Pane(diagram, Region::Edge(Edge::Right)),
    );
    assert_eq!(shape(&l), "V(V(Diagram,Tree),Source)");
}

#[test]
fn dropping_on_a_pane_center_swaps_the_two() {
    let mut l = PaneLayout::default();
    let diagram = pane(&l, PaneKind::Diagram);
    drop_on(
        &mut l,
        PaneKind::Tree,
        Target::Pane(diagram, Region::Center),
    );
    assert_eq!(shape(&l), "V(Diagram,V(Tree,Source))");
}

#[test]
fn dropping_on_the_outer_edge_moves_the_pane_to_that_edge() {
    let mut l = PaneLayout::default();
    drop_on(&mut l, PaneKind::Source, Target::Edge(Edge::Top));
    assert_eq!(shape(&l), "H(Source,V(Tree,Diagram))");
    let mut l = PaneLayout::default();
    drop_on(&mut l, PaneKind::Tree, Target::Edge(Edge::Right));
    assert_eq!(shape(&l), "V(V(Diagram,Source),Tree)");
}

#[test]
fn dragging_is_ignored_while_maximized() {
    let mut l = PaneLayout::default();
    let tree = pane(&l, PaneKind::Tree);
    l.maximize(pane(&l, PaneKind::Source));
    l.drag(DragEvent::Dropped {
        pane: pane(&l, PaneKind::Source),
        target: Target::Pane(tree, Region::Center),
    });
    assert_eq!(shape(&l), "V(Tree,V(Diagram,Source))");
}

fn split_of(l: &PaneLayout) -> iced::widget::pane_grid::Split {
    match l.state.layout() {
        Node::Split { id, .. } => *id,
        Node::Pane(_) => panic!("no split"),
    }
}

fn ratio_of(l: &PaneLayout) -> f32 {
    match l.state.layout() {
        Node::Split { ratio, .. } => *ratio,
        Node::Pane(_) => panic!("no split"),
    }
}

#[test]
fn resize_updates_the_ratio() {
    let mut l = PaneLayout::default();
    let split = split_of(&l);
    l.resize(ResizeEvent { split, ratio: 0.3 }, GRID);
    assert!((ratio_of(&l) - 0.3).abs() < 0.01, "{}", ratio_of(&l));
}

#[test]
fn no_pane_goes_below_160px_in_either_dimension() {
    for ratio in [0.0f32, 0.01, 0.05, 0.5, 0.95, 0.99, 1.0] {
        let mut l = PaneLayout::default();
        let split = split_of(&l);
        l.resize(ResizeEvent { split, ratio }, GRID);
        // Also stack two panes vertically and squeeze that split.
        let tree = pane(&l, PaneKind::Tree);
        l.drag(DragEvent::Dropped {
            pane: pane(&l, PaneKind::Source),
            target: Target::Pane(tree, Region::Edge(Edge::Bottom)),
        });
        let inner = l.state.layout().splits().copied().collect::<Vec<_>>();
        for s in inner {
            l.resize(ResizeEvent { split: s, ratio }, GRID);
        }
        for (kind, r) in l.regions(GRID) {
            assert!(
                r.width >= MIN_PANE && r.height >= MIN_PANE,
                "ratio {ratio}: {kind:?} is {}×{}",
                r.width,
                r.height
            );
        }
    }
}
