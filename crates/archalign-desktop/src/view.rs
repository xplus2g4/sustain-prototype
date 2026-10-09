//! The window chrome: toolbar, dropdowns, welcome screen, pane grid and status bar.

use std::sync::LazyLock;

use archalign_repo::git::GitStatus;
use iced::widget::{button, column, container, pane_grid, row, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme};

use crate::app::{
    Action, App, Level, Menu, Message, RunState, STATUS_H, TITLE_H, TOOLBAR_H, WatchStatus,
};
use crate::diagram;
use crate::fonts::{MONO, MONO_BOLD};
use crate::ids;
use crate::palette::{DARK, LIGHT, Palette};
use crate::panes::{MIN_PANE, PaneKind, SPACING};
use crate::{source, tree};

/// The dark iced theme, built once.
pub static DARK_THEME: LazyLock<Theme> = LazyLock::new(|| DARK.iced_theme("ArchAlign Dark"));
/// The light iced theme, built once.
pub static LIGHT_THEME: LazyLock<Theme> = LazyLock::new(|| LIGHT.iced_theme("ArchAlign Light"));

fn tool_button<'a>(
    label: impl Into<String>,
    msg: Option<Message>,
    id: iced::widget::Id,
) -> Element<'a, Message> {
    container(
        button(text(label.into()).size(12))
            .on_press_maybe(msg)
            .padding(Padding::from([5, 10]))
            .style(button::secondary),
    )
    .id(id)
    .into()
}

fn toolbar<'a>(app: &'a App, p: &'a Palette) -> Element<'a, Message> {
    let running = app.run.is_running();
    let archify = if running {
        tool_button(
            "Cancel",
            Some(Message::Action(Action::CancelRun)),
            ids::TB_ARCHIFY,
        )
    } else {
        tool_button(
            "Archify",
            app.can_archify()
                .then_some(Message::Action(Action::Archify)),
            ids::TB_ARCHIFY,
        )
    };
    let bar = row![
        text("ArchAlign")
            .size(14)
            .font(MONO_BOLD)
            .color(p.toolbar_text),
        tool_button(
            "Open repo",
            app.can_open_repo()
                .then_some(Message::Action(Action::OpenFolder)),
            ids::TB_OPEN
        ),
        text_input("Focus (optional)", &app.focus)
            .id(ids::FOCUS_INPUT)
            .on_input(Message::FocusChanged)
            .on_submit_maybe(
                app.can_archify()
                    .then_some(Message::Action(Action::Archify))
            )
            .size(12)
            .width(Length::FillPortion(3)),
        archify,
        tool_button(
            "History ▾",
            Some(Message::Action(Action::History)),
            ids::TB_HISTORY
        ),
        tool_button(
            "Panels ▾",
            Some(Message::ToggleMenu(Menu::Panels)),
            ids::TB_PANELS
        ),
        tool_button(
            match app.theme {
                crate::palette::ThemeChoice::Dark => "Light theme",
                crate::palette::ThemeChoice::Light => "Dark theme",
            },
            Some(Message::Action(Action::ToggleTheme)),
            ids::TB_THEME
        ),
        tool_button(
            "Settings",
            Some(Message::Action(Action::Settings)),
            ids::TB_SETTINGS
        ),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    container(bar)
        .height(TOOLBAR_H)
        .width(Length::Fill)
        .padding(Padding::from([6, 10]))
        .style(move |_| container::Style {
            background: Some(Background::Color(p.toolbar_bg)),
            border: Border {
                color: p.toolbar_border,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn panels_menu<'a>(app: &'a App, p: &'a Palette) -> Element<'a, Message> {
    let mut r = row![text("Panels:").size(12).color(p.text_muted)]
        .spacing(8)
        .align_y(Alignment::Center);
    for kind in PaneKind::ALL {
        let visible = app.panes.is_visible(kind);
        let label = format!("{} {}", if visible { "☑" } else { "☐" }, kind.name());
        r = r.push(tool_button(
            label,
            Some(Message::Action(Action::TogglePane(kind))),
            ids::menu_pane(kind),
        ));
    }
    r = r.push(tool_button(
        "Reset layout",
        Some(Message::Action(Action::ResetLayout)),
        ids::MENU_RESET,
    ));
    menu_strip(r.into(), p)
}

fn menu_strip<'a>(content: Element<'a, Message>, p: &'a Palette) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .padding(Padding::from([4, 10]))
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

fn banner_view<'a>(b: &'a crate::app::Banner, p: &'a Palette) -> Element<'a, Message> {
    let color = match b.level {
        Level::Info => p.text_muted,
        Level::Warning => p.warning(),
        Level::Error => p.danger(),
    };
    container(
        row![
            text(b.text.clone())
                .size(12)
                .color(p.text)
                .width(Length::Fill),
            container(
                button(text("✕").size(11))
                    .on_press(Message::DismissBanner(b.id))
                    .padding(Padding::from([1, 6]))
                    .style(button::text)
            )
            .id(ids::banner_dismiss(b.id)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([4, 8]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(Color { a: 0.12, ..color })),
        border: Border {
            color,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

fn banners_for<'a>(
    app: &'a App,
    pane: Option<PaneKind>,
    p: &'a Palette,
) -> Option<Element<'a, Message>> {
    let list: Vec<_> = app.banners.iter().filter(|b| b.pane == pane).collect();
    if list.is_empty() {
        return None;
    }
    let mut col = column![].spacing(4).padding(4);
    for b in list {
        col = col.push(banner_view(b, p));
    }
    Some(col.into())
}

fn welcome<'a>(app: &'a App, p: &'a Palette) -> Element<'a, Message> {
    let mut col = column![
        text("ArchAlign").size(28).font(MONO_BOLD).color(p.text),
        text("Native viewer and generator for Archify architecture diagrams. Unofficial; not affiliated with Archify.")
            .size(12)
            .color(p.text_muted),
        container(
            button(text("Open folder…").size(13))
                .on_press_maybe(app.can_open_repo().then_some(Message::Action(Action::OpenFolder)))
                .padding(Padding::from([8, 16]))
        )
        .id(ids::WELCOME_OPEN),
    ]
    .spacing(14)
    .max_width(640);
    if !app.recents.is_empty() {
        col = col.push(text("Recent repositories").size(13).color(p.text));
        for (i, r) in app.recents.iter().enumerate() {
            let label = r.path.display().to_string();
            let open = container(
                button(text(label).size(12).font(MONO).color(if r.missing {
                    p.text_dim
                } else {
                    p.text
                }))
                .on_press_maybe((!r.missing).then_some(Message::OpenRecent(i)))
                .style(button::text)
                .width(Length::Fill),
            )
            .id(ids::recent(i))
            .width(Length::Fill);
            let mut line = row![open].spacing(8).align_y(Alignment::Center);
            if r.missing {
                line = line.push(text("Missing").size(11).color(p.warning()));
            }
            line = line.push(
                container(
                    button(text("Remove").size(11))
                        .on_press(Message::RemoveRecent(i))
                        .style(button::text),
                )
                .id(ids::recent_remove(i)),
            );
            col = col.push(line);
        }
    }
    container(col).center(Length::Fill).into()
}

fn title_bar<'a>(
    app: &'a App,
    pane: pane_grid::Pane,
    kind: PaneKind,
    p: &'a Palette,
) -> pane_grid::TitleBar<'a, Message> {
    let context = match kind {
        PaneKind::Diagram => app.diagram.title().unwrap_or_default(),
        PaneKind::Source => app.source.rel().map(|r| r.to_string()).unwrap_or_default(),
        PaneKind::Tree => app
            .repo
            .as_ref()
            .and_then(|r| {
                r.root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
            })
            .unwrap_or_default(),
    };
    let maximized = app.panes.state.maximized() == Some(pane);
    let max_btn = container(
        button(text(if maximized { "Restore" } else { "Maximize" }).size(11))
            .on_press(if maximized {
                Message::PaneRestore
            } else {
                Message::PaneMaximize(pane)
            })
            .padding(Padding::from([1, 6]))
            .style(button::text),
    )
    .id(ids::pane_maximize(kind));
    let can_close = app.panes.visible_count() > 1;
    let close_btn = container(
        button(text("Close").size(11))
            .on_press_maybe(can_close.then_some(Message::PaneClose(pane)))
            .padding(Padding::from([1, 6]))
            .style(button::text),
    )
    .id(ids::pane_close(kind));
    let label = row![
        text(kind.name()).size(12).font(MONO_BOLD).color(p.text),
        text(context).size(11).font(MONO).color(p.text_muted),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    pane_grid::TitleBar::new(
        container(label)
            .id(ids::pane_title(kind))
            .height(TITLE_H - 8.0),
    )
    .controls(pane_grid::Controls::new(
        row![max_btn, close_btn].spacing(2),
    ))
    .always_show_controls()
    .padding(Padding::from([4, 8]))
    .style(move |_| container::Style {
        background: Some(Background::Color(p.panel)),
        border: Border {
            color: p.panel_border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    })
}

fn pane_body<'a>(app: &'a App, kind: PaneKind, p: &'a Palette) -> Element<'a, Message> {
    let content: Element<'a, Message> = match kind {
        PaneKind::Tree => tree::view(&app.tree, p, app.pane_body_height(PaneKind::Tree)),
        PaneKind::Source => source::view(&app.source, p, app.pane_body_size(PaneKind::Source)),
        PaneKind::Diagram => diagram::view(
            &app.diagram,
            diagram::ViewCtx {
                palette: p,
                focus: &app.focus,
                can_archify: app.can_archify(),
            },
        ),
    };
    match banners_for(app, Some(kind), p) {
        Some(b) => column![b, content].into(),
        None => content,
    }
}

fn panes<'a>(app: &'a App, p: &'a Palette) -> Element<'a, Message> {
    let grid = pane_grid(&app.panes.state, |pane, kind, _maximized| {
        pane_grid::Content::new(pane_body(app, *kind, p))
            .title_bar(title_bar(app, pane, *kind, p))
            .style(move |_| container::Style {
                background: Some(Background::Color(p.bg)),
                border: Border {
                    color: p.panel_border,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            })
    })
    .spacing(SPACING)
    .min_size(MIN_PANE)
    .on_click(Message::PaneClicked)
    .on_drag(Message::PaneDragged)
    .on_resize(6, Message::PaneResized)
    .width(Length::Fill)
    .height(Length::Fill);
    grid.into()
}

/// Status-bar text for the git metadata.
pub fn git_label(app: &App) -> String {
    match app.repo.as_ref().map(|r| &r.git) {
        Some(GitStatus::Repo(info)) => {
            format!("git {} · {} dirty", info.short_head(), info.dirty.len())
        }
        Some(GitStatus::Unavailable) => "git not found".into(),
        Some(GitStatus::Subfolder { .. }) => "git: subfolder (off)".into(),
        Some(GitStatus::NotARepo) => "not a git repo".into(),
        None => String::new(),
    }
}

/// Status-bar text for the run.
pub fn run_label(app: &App) -> String {
    match &app.run {
        RunState::Idle => "Idle".into(),
        RunState::Running {
            turn,
            max_turns,
            input_tokens,
            elapsed_s,
        } => format!(
            "Turn {turn}/{max_turns} · {input_tokens} input tokens · {}:{:02}",
            elapsed_s / 60,
            elapsed_s % 60
        ),
    }
}

/// Status-bar text for the watcher.
pub fn watch_label(app: &App) -> &'static str {
    match app.watch {
        WatchStatus::Watching => "Watching",
        WatchStatus::ManualOnly => "Manual refresh only",
    }
}

fn status_bar<'a>(app: &'a App, p: &'a Palette) -> Element<'a, Message> {
    let item = |s: String, id: iced::widget::Id| {
        container(text(s).size(11).font(MONO).color(p.text_muted)).id(id)
    };
    let repo = app
        .repo
        .as_ref()
        .map(|r| r.root.display().to_string())
        .unwrap_or_else(|| "No repository".into());
    let bar = row![
        item(repo, ids::STATUS_REPO),
        item(git_label(app), ids::STATUS_GIT),
        container(
            text(app.status_message.clone().unwrap_or_default())
                .size(11)
                .color(p.text)
        )
        .id(ids::STATUS_MESSAGE)
        .width(Length::Fill),
        item(run_label(app), ids::STATUS_RUN),
        item(watch_label(app).to_string(), ids::STATUS_WATCH),
    ]
    .spacing(16)
    .align_y(Alignment::Center);
    container(bar)
        .height(STATUS_H)
        .width(Length::Fill)
        .padding(Padding::from([4, 10]))
        .style(move |_| container::Style {
            background: Some(Background::Color(p.toolbar_bg)),
            border: Border {
                color: p.toolbar_border,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// The whole window.
pub fn view(app: &App) -> Element<'_, Message> {
    let p = app.theme.palette();
    let mut col = column![toolbar(app, p)];
    if app.menu == Some(Menu::Panels) {
        col = col.push(panels_menu(app, p));
    }
    if let Some(b) = banners_for(app, None, p) {
        col = col.push(b);
    }
    let body: Element<'_, Message> = if app.repo.is_some() {
        panes(app, p)
    } else {
        welcome(app, p)
    };
    col = col.push(container(body).height(Length::Fill).width(Length::Fill));
    col = col.push(status_bar(app, p));
    container(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.bg)),
            text_color: Some(p.text),
            ..container::Style::default()
        })
        .into()
}
