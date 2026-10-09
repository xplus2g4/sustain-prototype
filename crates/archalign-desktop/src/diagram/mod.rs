//! The diagram pane: state, canvas and detail strip (FR-DIA).

pub mod camera;
pub mod canvas;
pub mod logic;

use std::collections::BTreeSet;

use archalign_repo::RelPath;
use archalign_repo::citations::CitationIndex;
use iced::widget::{
    Space, button, canvas as iced_canvas, column, container, pin, row, scrollable, stack, text,
    text_input,
};
use iced::{Background, Border, Element, Length, Padding, Point, Size};

use crate::app::{Action, Message};
use crate::fonts::{MONO, MONO_BOLD};
use crate::ids;
use crate::ops::{DiagramFailed, DiagramLoaded};
use crate::palette::{Palette, ThemeChoice};
use camera::{Bounds, Camera};
use canvas::DiagramCanvas;
use logic::{Badges, Direction, LegendRow, ReachSet};

/// An open diagram with everything derived from it.
#[derive(Debug, Clone)]
pub struct LoadedView {
    /// Document, scene and warnings.
    pub d: DiagramLoaded,
    /// Per-node badges.
    pub badges: Vec<Badges>,
    /// Legend rows.
    pub legend: Vec<LegendRow>,
    /// File ↔ node index for the tree.
    pub index: CitationIndex,
    /// Bumped on every load (canvas cache key).
    pub version: u64,
}

/// What the diagram pane shows.
#[derive(Debug, Clone, Default)]
pub enum DiagramContent {
    /// No diagram: the Archify empty state.
    #[default]
    Empty,
    /// A diagram is loading.
    Loading(RelPath),
    /// A diagram is open.
    Loaded(Box<LoadedView>),
    /// The diagram couldn't be opened.
    Failed(DiagramFailed),
}

/// What is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Selection {
    /// Nothing.
    #[default]
    None,
    /// A component.
    Node(usize),
    /// A connection.
    Edge(usize),
}

/// Hover state for the tooltip (FR-DIA-04).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hover {
    /// Hovered node.
    pub node: usize,
    /// Cursor position in canvas coordinates.
    pub at: Point,
    /// The 400 ms delay has passed.
    pub shown: bool,
}

/// The node finder (FR-DIA-07).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Finder {
    /// Search text.
    pub query: String,
    /// Matching nodes (at most 10).
    pub results: Vec<usize>,
    /// Highlighted result.
    pub cursor: usize,
}

/// State of the diagram pane.
#[derive(Debug, Clone, Default)]
pub struct DiagramState {
    /// What is shown.
    pub content: DiagramContent,
    /// Camera.
    pub camera: Camera,
    /// Last known canvas size.
    pub canvas_size: Option<Size>,
    /// The camera still shows the fitted view (refit on resize).
    pub fitted: bool,
    /// Selection.
    pub selection: Selection,
    /// Reach highlight.
    pub reach: Option<(Direction, ReachSet)>,
    /// Nodes citing the file selected in the tree (FR-TREE-07).
    pub reverse: BTreeSet<usize>,
    /// Hover.
    pub hover: Option<Hover>,
    /// Finder.
    pub finder: Option<Finder>,
    /// The canvas has keyboard focus.
    pub focused: bool,
    /// Cards strip expanded.
    pub cards_open: bool,
    /// Problem list expanded.
    pub problems_open: bool,
    next_version: u64,
}

impl DiagramState {
    /// The open diagram, if any.
    pub fn view(&self) -> Option<&LoadedView> {
        match &self.content {
            DiagramContent::Loaded(v) => Some(v),
            _ => None,
        }
    }

    /// The open diagram's files, if any.
    pub fn loaded(&self) -> Option<&DiagramLoaded> {
        self.view().map(|v| &v.d)
    }

    /// Title for the pane's context label.
    pub fn title(&self) -> Option<String> {
        self.loaded().map(|d| d.doc.meta.title.clone())
    }

    /// Shows a freshly loaded diagram: derived data, cleared interaction state, fitted camera.
    pub fn set_loaded(&mut self, d: DiagramLoaded, fallback_size: Size) {
        self.next_version += 1;
        let badges = logic::badges(&d.doc, &d.warnings);
        let legend = logic::legend(&d.doc);
        let index = CitationIndex::build(&d.doc, &d.warnings);
        self.content = DiagramContent::Loaded(Box::new(LoadedView {
            d,
            badges,
            legend,
            index,
            version: self.next_version,
        }));
        self.selection = Selection::None;
        self.reach = None;
        self.reverse.clear();
        self.hover = None;
        self.finder = None;
        self.fit(fallback_size);
    }

    /// The canvas size, or `fallback` before the canvas reported one.
    pub fn size_or(&self, fallback: Size) -> Size {
        self.canvas_size.unwrap_or(fallback)
    }

    /// Fits the diagram into the canvas with 40 px padding (FR-DIA-02).
    pub fn fit(&mut self, fallback: Size) {
        let size = self.size_or(fallback);
        if let Some(v) = self.view() {
            let b = v.d.scene.bounds.inflate(-archalign_layout::scene::ORIGIN);
            self.camera = Camera::fit(
                Bounds {
                    x: b.x as f32,
                    y: b.y as f32,
                    w: b.w as f32,
                    h: b.h as f32,
                },
                size,
            );
            self.fitted = true;
        }
    }

    /// Zooms by `factor` about the canvas center.
    pub fn zoom_center(&mut self, factor: f32, fallback: Size) {
        let s = self.size_or(fallback);
        self.camera
            .zoom_about(factor, Point::new(s.width / 2.0, s.height / 2.0));
        self.fitted = false;
    }

    /// Sets 100% about the canvas center.
    pub fn zoom_reset(&mut self, fallback: Size) {
        let s = self.size_or(fallback);
        self.camera
            .set_zoom_about(1.0, Point::new(s.width / 2.0, s.height / 2.0));
        self.fitted = false;
    }

    /// Centers a node in the canvas.
    pub fn center_node(&mut self, i: usize, fallback: Size) {
        let s = self.size_or(fallback);
        if let Some(n) = self.view().and_then(|v| v.d.scene.nodes.get(i)) {
            let c = n.rect.center();
            self.camera.center_on(Point::new(c.x as f32, c.y as f32), s);
            self.fitted = false;
        }
    }

    /// Toggles a reach highlight from the selected node (FR-DIA-05).
    pub fn toggle_reach(&mut self, dir: Direction) {
        let Selection::Node(start) = self.selection else {
            return;
        };
        if self.reach.as_ref().is_some_and(|(d, _)| *d == dir) {
            self.reach = None;
            return;
        }
        if let Some(v) = self.view() {
            self.reach = Some((dir, logic::reach(&v.d.doc, start, dir)));
        }
    }

    /// Esc (FR-DIA-08): closes the finder, or clears reach, selection and reverse highlight.
    pub fn escape(&mut self) {
        if self.finder.take().is_some() {
            return;
        }
        self.reach = None;
        self.selection = Selection::None;
        self.reverse.clear();
    }

    /// Updates the finder results for a query.
    pub fn set_query(&mut self, query: String) {
        let results = self
            .view()
            .map(|v| logic::find_nodes(&v.d.doc, &query))
            .unwrap_or_default();
        self.finder = Some(Finder {
            query,
            results,
            cursor: 0,
        });
    }

    /// Moves the finder highlight.
    pub fn move_cursor(&mut self, delta: i32) {
        if let Some(f) = &mut self.finder
            && !f.results.is_empty()
        {
            let n = f.results.len() as i32;
            f.cursor = (f.cursor as i32 + delta).rem_euclid(n) as usize;
        }
    }
}

/// Inputs the view needs from the rest of the app.
pub struct ViewCtx<'a> {
    /// Palette.
    pub palette: &'a Palette,
    /// Theme.
    pub theme: ThemeChoice,
    /// Focus text.
    pub focus: &'a str,
    /// Whether Archify can start now.
    pub can_archify: bool,
}

/// The empty state: Focus field and Archify button (FR-IMP-03).
pub fn empty_state<'a>(ctx: &ViewCtx<'a>) -> Element<'a, Message> {
    let p = ctx.palette;
    container(
        column![
            text("No diagram yet").size(16).color(p.text),
            text("Describe what to focus on (optional), then press Archify to generate an architecture diagram of this repository.")
                .size(12)
                .color(p.text_muted),
            row![
                text_input("Focus (optional)", ctx.focus)
                    .id(ids::FOCUS_INPUT_EMPTY)
                    .on_input(Message::FocusChanged)
                    .on_submit(Message::Action(Action::Archify))
                    .width(360),
                button(text("Archify")).on_press_maybe(ctx.can_archify.then_some(Message::Action(Action::Archify))),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .max_width(560),
    )
    .center(Length::Fill)
    .into()
}

fn chip<'a>(
    label: impl Into<String>,
    msg: Option<Message>,
    id: iced::widget::Id,
) -> Element<'a, Message> {
    container(
        button(text(label.into()).size(11))
            .on_press_maybe(msg)
            .padding(Padding::from([2, 8]))
            .style(button::secondary),
    )
    .id(id)
    .into()
}

fn finder_bar<'a>(
    state: &'a DiagramState,
    v: &'a LoadedView,
    p: &'a Palette,
) -> Option<Element<'a, Message>> {
    let f = state.finder.as_ref()?;
    let mut col = column![
        text_input("Find node by label or ID…", &f.query)
            .id(ids::FINDER_INPUT)
            .on_input(|q| Message::Diagram(DiagramMsg::FinderQuery(q)))
            .on_submit(Message::Diagram(DiagramMsg::FinderSubmit))
            .size(12)
    ]
    .spacing(2)
    .padding(6);
    for (k, &i) in f.results.iter().enumerate() {
        let Some(c) = v.d.doc.components.get(i) else {
            continue;
        };
        let active = k == f.cursor;
        col = col.push(
            container(
                button(
                    row![
                        text(c.label.clone()).size(12).color(p.text),
                        text(c.id.clone()).size(11).font(MONO).color(p.text_muted)
                    ]
                    .spacing(8),
                )
                .on_press(Message::Diagram(DiagramMsg::FinderPick(i)))
                .width(Length::Fill)
                .padding(Padding::from([2, 6]))
                .style(move |_, _| button::Style {
                    background: active.then_some(Background::Color(p.panel_border)),
                    text_color: p.text,
                    ..button::Style::default()
                }),
            )
            .id(ids::finder_result(k)),
        );
    }
    Some(
        container(col)
            .style(move |_| container::Style {
                background: Some(Background::Color(p.panel)),
                border: Border {
                    color: p.panel_border,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            })
            .into(),
    )
}

fn detail_strip<'a>(
    state: &'a DiagramState,
    v: &'a LoadedView,
    p: &'a Palette,
) -> Element<'a, Message> {
    let doc = &v.d.doc;
    let selection: Element<'a, Message> = match state.selection {
        Selection::Node(i) => match doc.components.get(i) {
            Some(c) => {
                let mut r = row![text(c.label.clone()).size(13).font(MONO_BOLD).color(p.text)]
                    .spacing(8)
                    .align_y(iced::Alignment::Center);
                if let Some(s) = c.sublabel.as_deref().filter(|s| !s.is_empty()) {
                    r = r.push(text(s.to_string()).size(12).color(p.text_muted));
                }
                if let Some(t) = c.tag.as_deref().filter(|s| !s.is_empty()) {
                    r = r.push(
                        text(format!("[{t}]"))
                            .size(11)
                            .color(p.component(c.ty).stroke),
                    );
                }
                let mut sources = row![].spacing(6).align_y(iced::Alignment::Center);
                if c.sources().is_empty() {
                    sources = sources.push(
                        container(text("No source").size(11).color(p.warning()))
                            .id(ids::DETAIL_NO_SOURCE),
                    );
                }
                for (j, s) in c.sources().iter().enumerate() {
                    sources = sources.push(chip(
                        logic::source_label(s),
                        Some(Message::Diagram(DiagramMsg::OpenSource(i, j))),
                        ids::detail_source(j),
                    ));
                }
                let up = state
                    .reach
                    .as_ref()
                    .is_some_and(|(d, _)| *d == Direction::Upstream);
                let down = state
                    .reach
                    .as_ref()
                    .is_some_and(|(d, _)| *d == Direction::Downstream);
                sources = sources
                    .push(Space::new().width(Length::Fill))
                    .push(chip(
                        if up {
                            "● Upstream (U)"
                        } else {
                            "Upstream (U)"
                        },
                        Some(Message::Diagram(DiagramMsg::Reach(Direction::Upstream))),
                        ids::REACH_UP,
                    ))
                    .push(chip(
                        if down {
                            "● Downstream (D)"
                        } else {
                            "Downstream (D)"
                        },
                        Some(Message::Diagram(DiagramMsg::Reach(Direction::Downstream))),
                        ids::REACH_DOWN,
                    ));
                column![r, sources].spacing(4).into()
            }
            None => Space::new().into(),
        },
        Selection::Edge(k) => match doc.connections().get(k) {
            Some(c) => container(
                text(format!(
                    "{} → {}{} · {}",
                    c.from,
                    c.to,
                    c.label
                        .as_deref()
                        .map(|l| format!(" · {l}"))
                        .unwrap_or_default(),
                    c.variant().as_str()
                ))
                .size(12)
                .font(MONO)
                .color(p.text),
            )
            .id(ids::DETAIL_EDGE)
            .into(),
            None => Space::new().into(),
        },
        Selection::None => {
            text("Click a node to open the code it cites; click a connection to see its endpoints.")
                .size(11)
                .color(p.text_muted)
                .into()
        }
    };
    let n_problems = v.d.warnings.len();
    let n_cards = doc.cards().len();
    let mut toggles = row![
        chip(
            format!("Problems ({n_problems})"),
            Some(Message::Diagram(DiagramMsg::ToggleProblems)),
            ids::PROBLEMS_TOGGLE
        ),
        chip(
            format!("Cards ({n_cards})"),
            Some(Message::Diagram(DiagramMsg::ToggleCards)),
            ids::CARDS_TOGGLE
        ),
    ]
    .spacing(6);
    if canvas::is_auto(&v.d.scene) {
        toggles = toggles.push(text("auto layout").size(10).color(p.text_dim));
    }
    let mut col = column![selection, toggles]
        .spacing(6)
        .padding(Padding::from([6, 8]));
    if state.problems_open {
        let mut list = column![].spacing(2);
        if v.d.warnings.is_empty() {
            list = list.push(text("No problems").size(11).color(p.text_muted));
        }
        for w in &v.d.warnings {
            list = list.push(
                text(format!(
                    "{}  {}",
                    if w.pointer.is_empty() {
                        "/"
                    } else {
                        &w.pointer
                    },
                    w.message
                ))
                .size(11)
                .font(MONO)
                .color(p.warning()),
            );
        }
        col = col.push(scrollable(list).height(Length::Shrink).height(120));
    }
    if state.cards_open && n_cards > 0 {
        let mut cards = row![].spacing(10);
        for card in doc.cards() {
            let mut c = column![
                row![
                    text("●").size(11).color(p.card_dot(card.dot)),
                    text(card.title.clone())
                        .size(12)
                        .font(MONO_BOLD)
                        .color(p.text)
                ]
                .spacing(4)
            ]
            .spacing(2)
            .width(220);
            for item in &card.items {
                c = c.push(text(format!("· {item}")).size(11).color(p.text_muted));
            }
            cards = cards.push(c);
        }
        col = col.push(
            scrollable(cards).direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::default(),
            )),
        );
    }
    container(col)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.panel)),
            border: Border {
                color: p.panel_border,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn tooltip_view<'a>(
    state: &'a DiagramState,
    v: &'a LoadedView,
    p: &'a Palette,
) -> Option<Element<'a, Message>> {
    let h = state.hover.filter(|h| h.shown)?;
    let lines = logic::tooltip_lines(&v.d.doc, h.node);
    let mut col = column![].spacing(2);
    for (k, l) in lines.into_iter().enumerate() {
        col = col.push(
            text(l)
                .size(if k == 0 { 12 } else { 11 })
                .font(if k == 0 { MONO_BOLD } else { MONO })
                .color(if k == 0 { p.text } else { p.text_muted }),
        );
    }
    Some(
        pin(container(col)
            .id(ids::DIAGRAM_TOOLTIP)
            .padding(6)
            .style(move |_| container::Style {
                background: Some(Background::Color(p.mask)),
                border: Border {
                    color: p.panel_border,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            }))
        .x(h.at.x + 14.0)
        .y(h.at.y + 14.0)
        .into(),
    )
}

/// The diagram pane body.
pub fn view<'a>(state: &'a DiagramState, ctx: ViewCtx<'a>) -> Element<'a, Message> {
    let p = ctx.palette;
    match &state.content {
        DiagramContent::Empty => empty_state(&ctx),
        DiagramContent::Loading(rel) => {
            container(text(format!("Opening {rel}…")).size(12).color(p.text_muted))
                .center(Length::Fill)
                .into()
        }
        DiagramContent::Failed(f) => {
            let mut col = column![
                text(match &f.unsupported {
                    Some(ty) => archalign_model::document::unsupported_message(
                        &archalign_model::DiagramType::Other(ty.clone())
                    ),
                    None => format!("{} can't be opened:", f.rel),
                })
                .size(13)
                .color(p.danger())
            ]
            .spacing(4);
            if f.unsupported.is_none() {
                for e in &f.errors {
                    col = col.push(
                        text(format!(
                            "{}  {}",
                            if e.pointer.is_empty() {
                                "/"
                            } else {
                                &e.pointer
                            },
                            e.message
                        ))
                        .size(11)
                        .font(MONO)
                        .color(p.text_muted),
                    );
                }
            }
            scrollable(container(col).padding(12)).into()
        }
        DiagramContent::Loaded(v) => {
            let program = DiagramCanvas {
                doc: &v.d.doc,
                scene: &v.d.scene,
                badges: &v.badges,
                legend: &v.legend,
                camera: state.camera,
                palette: p,
                theme: ctx.theme,
                version: v.version,
                selected_node: match state.selection {
                    Selection::Node(i) => Some(i),
                    _ => None,
                },
                selected_edge: match state.selection {
                    Selection::Edge(k) => Some(k),
                    _ => None,
                },
                reach: state.reach.as_ref().map(|(_, r)| r),
                reverse: &state.reverse,
                hover: state.hover.map(|h| h.node),
            };
            let canvas: Element<'a, Message> = container(
                iced_canvas(program)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .id(ids::DIAGRAM_CANVAS)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
            let mut layers = stack![canvas];
            if let Some(t) = tooltip_view(state, v, p) {
                layers = layers.push(t);
            }
            let mut col = column![];
            if let Some(f) = finder_bar(state, v, p) {
                col = col.push(f);
            }
            col.push(container(layers).height(Length::Fill).width(Length::Fill))
                .push(detail_strip(state, v, p))
                .into()
        }
    }
}

/// Diagram-pane messages.
#[derive(Debug, Clone)]
pub enum DiagramMsg {
    /// Open source `j` of node `i`.
    OpenSource(usize, usize),
    /// Toggle a reach highlight.
    Reach(Direction),
    /// Finder text changed.
    FinderQuery(String),
    /// Enter in the finder.
    FinderSubmit,
    /// A finder result was clicked.
    FinderPick(usize),
    /// Expand/collapse cards.
    ToggleCards,
    /// Expand/collapse the problem list.
    ToggleProblems,
    /// The hover delay passed for a node.
    HoverElapsed(usize),
}
