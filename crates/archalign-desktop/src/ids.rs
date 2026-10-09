//! Widget IDs. Tests find and click widgets by these (text selectors can't see rich text or
//! canvas content).

use archalign_repo::RelPath;
use iced::widget::Id;

use crate::panes::PaneKind;

/// The tree's scrollable.
pub const TREE_SCROLL: Id = Id::new("tree-scroll");
/// The source viewer's scrollable.
pub const SOURCE_SCROLL: Id = Id::new("source-scroll");
/// The Focus text field.
pub const FOCUS_INPUT: Id = Id::new("focus-input");
/// The Focus field in the empty diagram state.
pub const FOCUS_INPUT_EMPTY: Id = Id::new("focus-input-empty");
/// The node finder text field.
pub const FINDER_INPUT: Id = Id::new("finder-input");
/// The diagram canvas container.
pub const DIAGRAM_CANVAS: Id = Id::new("diagram-canvas");

/// Toolbar: Open repo.
pub const TB_OPEN: Id = Id::new("tb-open");
/// Toolbar: Archify / Cancel.
pub const TB_ARCHIFY: Id = Id::new("tb-archify");
/// Toolbar: History menu.
pub const TB_HISTORY: Id = Id::new("tb-history");
/// Toolbar: Panels menu.
pub const TB_PANELS: Id = Id::new("tb-panels");
/// Toolbar: Theme toggle.
pub const TB_THEME: Id = Id::new("tb-theme");
/// Toolbar: Settings.
pub const TB_SETTINGS: Id = Id::new("tb-settings");
/// Panels menu: reset layout.
pub const MENU_RESET: Id = Id::new("menu-reset-layout");

/// Status bar: repo path.
pub const STATUS_REPO: Id = Id::new("status-repo");
/// Status bar: git.
pub const STATUS_GIT: Id = Id::new("status-git");
/// Status bar: run progress.
pub const STATUS_RUN: Id = Id::new("status-run");
/// Status bar: watcher.
pub const STATUS_WATCH: Id = Id::new("status-watch");
/// Status bar: last message.
pub const STATUS_MESSAGE: Id = Id::new("status-message");

/// Source header: Copy path:line.
pub const SRC_COPY_PATH: Id = Id::new("src-copy-path");
/// Source header: Copy cited lines.
pub const SRC_COPY_LINES: Id = Id::new("src-copy-lines");
/// Source header: path label.
pub const SRC_PATH: Id = Id::new("src-path");

/// Welcome: Open folder.
pub const WELCOME_OPEN: Id = Id::new("welcome-open");

/// A pane's title bar.
pub fn pane_title(kind: PaneKind) -> Id {
    Id::from(format!("pane-title:{}", kind.name()))
}

/// A pane's Maximize/Restore button.
pub fn pane_maximize(kind: PaneKind) -> Id {
    Id::from(format!("pane-max:{}", kind.name()))
}

/// A pane's Close button.
pub fn pane_close(kind: PaneKind) -> Id {
    Id::from(format!("pane-close:{}", kind.name()))
}

/// A Panels-menu entry.
pub fn menu_pane(kind: PaneKind) -> Id {
    Id::from(format!("menu-pane:{}", kind.name()))
}

/// A tree row.
pub fn tree_row(rel: &RelPath) -> Id {
    Id::from(format!("tree-row:{rel}"))
}

/// A source line row (1-based line number).
pub fn source_line(n: usize) -> Id {
    Id::from(format!("src-line:{n}"))
}

/// A recent-repo entry on the welcome screen.
pub fn recent(i: usize) -> Id {
    Id::from(format!("recent:{i}"))
}

/// The remove control of a recent-repo entry.
pub fn recent_remove(i: usize) -> Id {
    Id::from(format!("recent-remove:{i}"))
}

/// A banner's dismiss button.
pub fn banner_dismiss(id: u64) -> Id {
    Id::from(format!("banner-dismiss:{id}"))
}
