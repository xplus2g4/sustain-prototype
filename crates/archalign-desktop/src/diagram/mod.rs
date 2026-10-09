//! The diagram pane: state and view (FR-DIA).

use archalign_repo::RelPath;
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Element, Length};

use crate::app::{Action, Message};
use crate::fonts::MONO;
use crate::ids;
use crate::ops::{DiagramFailed, DiagramLoaded};
use crate::palette::Palette;

/// What the diagram pane shows.
#[derive(Debug, Clone, Default)]
pub enum DiagramContent {
    /// No diagram: the Archify empty state.
    #[default]
    Empty,
    /// A diagram is loading.
    Loading(RelPath),
    /// A diagram is open.
    Loaded(DiagramLoaded),
    /// The diagram couldn't be opened.
    Failed(DiagramFailed),
}

/// State of the diagram pane.
#[derive(Debug, Clone, Default)]
pub struct DiagramState {
    /// What is shown.
    pub content: DiagramContent,
}

impl DiagramState {
    /// The open diagram, if any.
    pub fn loaded(&self) -> Option<&DiagramLoaded> {
        match &self.content {
            DiagramContent::Loaded(d) => Some(d),
            _ => None,
        }
    }

    /// Title for the pane's context label.
    pub fn title(&self) -> Option<String> {
        self.loaded().map(|d| d.doc.meta.title.clone())
    }
}

/// Inputs the view needs from the rest of the app.
pub struct ViewCtx<'a> {
    /// Palette.
    pub palette: &'a Palette,
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
                button(text("Archify"))
                    .on_press_maybe(ctx.can_archify.then_some(Message::Action(Action::Archify))),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .max_width(560),
    )
    .center(Length::Fill)
    .into()
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
        DiagramContent::Loaded(d) => container(
            column![
                text(d.doc.meta.title.clone()).size(16).color(p.text),
                text(format!(
                    "{} components · {} connections · {} boundaries",
                    d.scene.nodes.len(),
                    d.scene.edges.len(),
                    d.scene.boundaries.len()
                ))
                .size(12)
                .color(p.text_muted),
            ]
            .spacing(6),
        )
        .id(ids::DIAGRAM_CANVAS)
        .center(Length::Fill)
        .into(),
    }
}
