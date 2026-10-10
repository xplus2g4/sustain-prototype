//! ArchAlign: native viewer and generator for Archify architecture diagrams.
//! Unofficial; not affiliated with Archify. Includes material derived from Archify (MIT).
//!
//! The iced application lives here so tests can drive it; `main.rs` only starts it.

pub mod agent_run;
pub mod app;
pub mod diagram;
pub mod fonts;
pub mod ids;
pub mod keys;
pub mod ops;
pub mod palette;
pub mod panes;
pub mod settings;
pub mod source;
pub mod tree;
pub mod view;
pub mod virtual_list;

/// Maximum Focus length in characters (Appendix C, Tunable).
pub const FOCUS_MAX_CHARS: usize = 500;

/// Starts the application.
pub fn run(boot: app::Boot) -> iced::Result {
    iced::application(
        move || app::App::new(boot.clone()),
        app::App::update,
        view::view,
    )
    .title(app::App::title)
    .theme(app::App::iced_theme)
    .subscription(app::App::subscription)
    .font(fonts::REGULAR_BYTES)
    .font(fonts::BOLD_BYTES)
    .default_font(fonts::MONO)
    .window_size(app::DEFAULT_WINDOW)
    .run()
}
