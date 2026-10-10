//! The iced application: state, messages and `update` (Elm architecture).

use std::path::PathBuf;
use std::sync::Arc;

use archalign_repo::git::GitStatus;
use archalign_repo::storage::{self, HistoryEntry};
use archalign_repo::text::TextLoad;
use archalign_repo::tree::DirEntry;
use archalign_repo::{RelPath, Sandbox};
use iced::event;
use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::widget::{Id, operation, pane_grid, scrollable};
use iced::{Size, Subscription, Task};

use crate::agent_run::{self, LlmFactory, LlmSpec, PreparedRun, RunMsg};
use crate::diagram::canvas::{CanvasEvent, Hit};
use crate::diagram::logic::Direction;
use crate::diagram::{DiagramContent, DiagramMsg, DiagramState, Hover, Selection};
use crate::ids;
use crate::keys::{self, KeyEnv};
use crate::ops::{self, DiagramFailed, DiagramLoaded, RepoOpened};
use crate::palette::ThemeChoice;
use crate::panes::{PaneKind, PaneLayout};
use crate::settings::{Consent, Settings};
use crate::source::{
    self, Cite, CiteOutcome, OpenText, PlaceholderReason, SourceContent, SourceState,
};
use crate::tree::{self, TreeState};
use crate::virtual_list;
use archalign_repo::SelfWrites;

/// Maximum number of recent repositories.
pub const MAX_RECENTS: usize = 10;
/// Height of the toolbar.
pub const TOOLBAR_H: f32 = 44.0;
/// Height of the status bar.
pub const STATUS_H: f32 = 26.0;
/// Height of a pane title bar.
pub const TITLE_H: f32 = 30.0;
/// Height reserved for the diagram detail strip when estimating the canvas size.
pub const DETAIL_STRIP_H: f32 = 64.0;
/// Hover delay before a node tooltip shows (Appendix C, Tunable).
pub const HOVER_DELAY: std::time::Duration = std::time::Duration::from_millis(400);

fn camera_step() -> f32 {
    crate::diagram::camera::ZOOM_STEP
}

/// Height of the source header row.
pub const SOURCE_HEADER_H: f32 = 30.0;
/// Default window size (Appendix C, Tunable).
pub const DEFAULT_WINDOW: Size = Size::new(1440.0, 900.0);

/// User-level actions. Toolbar buttons and keyboard shortcuts produce the same action
/// (FR-PANE-09, §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// ⌘O
    OpenFolder,
    /// ⌘G
    Archify,
    /// ⌘.
    CancelRun,
    /// ⌘Y
    History,
    /// ⌘1 / ⌘2 / ⌘3
    TogglePane(PaneKind),
    /// ⌘⇧0
    ResetLayout,
    /// ⌘⇧D
    ToggleTheme,
    /// ⌘,
    Settings,
    /// ⌘R
    Refresh,
    /// ⌘⇧L
    Relayout,
}

/// Maps a key press to a global shortcut (§9). Diagram-scoped keys are handled by the canvas.
pub fn shortcut_action(key: &Key, modifiers: Modifiers) -> Option<Action> {
    if !modifiers.command() {
        return None;
    }
    let shift = modifiers.shift();
    let Key::Character(c) = key.as_ref() else {
        return None;
    };
    let c = c.to_lowercase();
    Some(match (c.as_str(), shift) {
        ("o", false) => Action::OpenFolder,
        ("g", false) => Action::Archify,
        (".", false) => Action::CancelRun,
        ("y", false) => Action::History,
        ("1", false) => Action::TogglePane(PaneKind::Tree),
        ("2", false) => Action::TogglePane(PaneKind::Diagram),
        ("3", false) => Action::TogglePane(PaneKind::Source),
        ("0" | ")", true) => Action::ResetLayout,
        ("d", true) => Action::ToggleTheme,
        (",", false) => Action::Settings,
        ("r", false) => Action::Refresh,
        ("l", true) => Action::Relayout,
        _ => return None,
    })
}

/// A toolbar dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    /// Panels ▾
    Panels,
    /// History ▾
    History,
}

/// Banner severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Informational.
    Info,
    /// Something degraded.
    Warning,
    /// Something failed.
    Error,
}

/// A dismissible message in a pane (FR-PANE-11).
#[derive(Debug, Clone, PartialEq)]
pub struct Banner {
    /// Stable ID for dismissal.
    pub id: u64,
    /// Pane it belongs to; `None` shows above the panes.
    pub pane: Option<PaneKind>,
    /// Severity.
    pub level: Level,
    /// Text.
    pub text: String,
}

/// A recent repository on the welcome screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Recent {
    /// Absolute path.
    pub path: PathBuf,
    /// The folder no longer exists.
    pub missing: bool,
}

/// The open repository.
#[derive(Debug, Clone)]
pub struct RepoSession {
    /// Canonical root.
    pub root: PathBuf,
    /// Confinement for every read.
    pub sandbox: Sandbox,
    /// Git metadata.
    pub git: GitStatus,
    /// `.archify/` history, newest first.
    pub history: Vec<HistoryEntry>,
}

impl RepoSession {
    /// Dirty paths, when git is available.
    pub fn dirty(&self) -> std::collections::HashSet<String> {
        match &self.git {
            GitStatus::Repo(info) => info.dirty_set(),
            _ => Default::default(),
        }
    }
}

/// Agent run progress (FR-PANE-10, FR-GEN-09).
#[derive(Debug, Clone, Default, PartialEq)]
pub enum RunState {
    /// No run.
    #[default]
    Idle,
    /// A run is active.
    Running {
        /// Current turn.
        turn: u32,
        /// Turn budget.
        max_turns: u32,
        /// Input tokens used so far.
        input_tokens: u64,
        /// Elapsed seconds.
        elapsed_s: u64,
    },
}

impl RunState {
    /// Whether a run is active.
    pub fn is_running(&self) -> bool {
        matches!(self, RunState::Running { .. })
    }
}

/// Watcher state shown in the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WatchStatus {
    /// Events arrive from the file watcher.
    Watching,
    /// No watcher; ⌘R refreshes.
    #[default]
    ManualOnly,
}

/// What the app needs to start (from `state.json` / `settings.json`).
#[derive(Clone, Default)]
pub struct Boot {
    /// Theme.
    pub theme: ThemeChoice,
    /// Last repository, reopened if it still exists (FR-IMP-05).
    pub last_repo: Option<PathBuf>,
    /// Recent repositories, most recent first.
    pub recent_repos: Vec<PathBuf>,
    /// User settings.
    pub settings: Settings,
    /// Consents granted so far (FR-SEC-03).
    pub consents: Vec<Consent>,
    /// The git executable (`git` on PATH unless overridden, e.g. by tests).
    pub git_program: Option<std::ffi::OsString>,
    /// Where API keys are looked up (the process environment unless overridden).
    pub key_env: Option<KeyEnv>,
    /// LLM client factory (litellm-rust unless overridden, e.g. by tests).
    pub llm: Option<LlmFactory>,
}

/// Every message the app handles.
#[derive(Debug, Clone)]
pub enum Message {
    /// A user action (button or shortcut).
    Action(Action),
    /// A key press; `captured` is true when a widget (e.g. a focused text field) used it.
    KeyPressed(Key, Modifiers, bool),
    /// An event from the diagram canvas.
    Canvas(CanvasEvent),
    /// Agent run messages.
    Run(RunMsg),
    /// Revoke a consent (Settings).
    RevokeConsent(usize),
    /// A diagram-pane message.
    Diagram(DiagramMsg),
    /// A shortcut after checking whether a text field has focus.
    ShortcutResolved(Action, bool),
    /// The folder picker closed.
    FolderPicked(Option<PathBuf>),
    /// Open this folder as the repository.
    OpenRepo(PathBuf),
    /// Repository opening finished.
    RepoOpened(u64, Result<Box<RepoOpened>, String>),
    /// Welcome screen: open a recent repository.
    OpenRecent(usize),
    /// Welcome screen: forget a recent repository.
    RemoveRecent(usize),
    /// The Focus field changed.
    FocusChanged(String),
    /// Open or close a toolbar dropdown.
    ToggleMenu(Menu),
    /// A pane was clicked.
    PaneClicked(pane_grid::Pane),
    /// A pane drag event.
    PaneDragged(pane_grid::DragEvent),
    /// A divider drag.
    PaneResized(pane_grid::ResizeEvent),
    /// Maximize a pane.
    PaneMaximize(pane_grid::Pane),
    /// Restore from maximized.
    PaneRestore,
    /// Close a pane.
    PaneClose(pane_grid::Pane),
    /// Expand or collapse a folder.
    TreeToggle(RelPath),
    /// Click a file row.
    TreeActivate(RelPath),
    /// A folder listing finished.
    TreeListed(u64, RelPath, Result<Vec<DirEntry>, String>),
    /// The tree scrolled.
    TreeScrolled(scrollable::Viewport),
    /// Open a file, optionally at a citation.
    OpenFile(RelPath, Option<Cite>),
    /// A file load finished.
    FileLoaded(u64, RelPath, Option<Cite>, Result<Arc<TextLoad>, String>),
    /// The source viewer scrolled.
    SourceScrolled(scrollable::Viewport),
    /// Copy `path:line`.
    CopyPathLine,
    /// Copy the banded lines.
    CopyCitedLines,
    /// Open a diagram file.
    DiagramOpen(RelPath),
    /// A diagram load finished.
    DiagramLoaded(u64, Result<Box<DiagramLoaded>, Box<DiagramFailed>>),
    /// Dismiss a banner.
    DismissBanner(u64),
    /// The window was resized.
    WindowResized(Size),
    /// Background warm-up finished (syntax sets).
    Warmed,
}

/// The application state.
pub struct App {
    /// Light or dark.
    pub theme: ThemeChoice,
    /// Pane arrangement.
    pub panes: PaneLayout,
    /// Window size.
    pub window: Size,
    /// The open repository.
    pub repo: Option<RepoSession>,
    /// Incremented on every repository switch; stale async results are dropped.
    pub generation: u64,
    /// Recent repositories, most recent first.
    pub recents: Vec<Recent>,
    /// Tree pane.
    pub tree: TreeState,
    /// Source pane.
    pub source: SourceState,
    /// Diagram pane.
    pub diagram: DiagramState,
    /// Agent run.
    pub run: RunState,
    /// Focus text.
    pub focus: String,
    /// Banners, oldest first.
    pub banners: Vec<Banner>,
    next_banner: u64,
    /// Latest status-bar message.
    pub status_message: Option<String>,
    /// Open toolbar dropdown.
    pub menu: Option<Menu>,
    /// User settings.
    pub settings: Settings,
    /// Consents (FR-SEC-03).
    pub consents: Vec<Consent>,
    /// Key lookup.
    pub key_env: KeyEnv,
    /// LLM client factory.
    pub llm: LlmFactory,
    /// The consent modal, when shown: (provider, model).
    pub consent_prompt: Option<(String, String)>,
    /// Abort handle of the active run.
    pub run_handle: Option<iced::task::Handle>,
    /// When the active run started.
    pub run_started_at: Option<std::time::Instant>,
    /// Run-log drawer lines (FR-GEN-09).
    pub run_log: Vec<String>,
    /// Run-log drawer expanded.
    pub run_drawer_open: bool,
    /// The app's own writes, for the watcher (FR-WAT-08).
    pub self_writes: SelfWrites,
    /// Watcher state.
    pub watch: WatchStatus,
    /// The git executable.
    pub git_program: std::ffi::OsString,
}

impl App {
    /// Creates the app from boot data, reopening the last repo if it still exists.
    pub fn new(boot: Boot) -> (Self, Task<Message>) {
        let recents = boot
            .recent_repos
            .iter()
            .take(MAX_RECENTS)
            .map(|p| Recent {
                missing: !p.is_dir(),
                path: p.clone(),
            })
            .collect();
        let app = Self {
            theme: boot.theme,
            panes: PaneLayout::default(),
            window: DEFAULT_WINDOW,
            repo: None,
            generation: 0,
            recents,
            tree: TreeState::default(),
            source: SourceState::default(),
            diagram: DiagramState::default(),
            run: RunState::Idle,
            focus: String::new(),
            banners: Vec::new(),
            next_banner: 1,
            status_message: None,
            menu: None,
            settings: boot.settings.clone(),
            consents: boot.consents.clone(),
            key_env: boot
                .key_env
                .clone()
                .unwrap_or_else(|| KeyEnv::process(None)),
            llm: boot.llm.clone().unwrap_or_else(agent_run::provider_factory),
            consent_prompt: None,
            run_handle: None,
            run_started_at: None,
            run_log: Vec::new(),
            run_drawer_open: false,
            self_writes: SelfWrites::default(),
            watch: WatchStatus::ManualOnly,
            git_program: boot.git_program.clone().unwrap_or_else(|| "git".into()),
        };
        let warm = Task::perform(
            ops::blocking(|| {
                std::sync::LazyLock::force(&source::SYNTAXES);
                std::sync::LazyLock::force(&source::THEMES);
                crate::fonts::ensure_loaded();
            }),
            |_| Message::Warmed,
        );
        let reopen = match boot.last_repo {
            Some(p) if p.is_dir() => Task::done(Message::OpenRepo(p)),
            _ => Task::none(),
        };
        (app, Task::batch([warm, reopen]))
    }

    /// The window title.
    pub fn title(&self) -> String {
        match &self.repo {
            Some(r) => format!(
                "ArchAlign — {}",
                r.root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("repository")
            ),
            None => "ArchAlign".to_string(),
        }
    }

    /// Size of the pane grid area.
    pub fn grid_size(&self) -> Size {
        Size::new(
            self.window.width,
            (self.window.height - TOOLBAR_H - STATUS_H).max(0.0),
        )
    }

    /// Height available to a pane's scrolling body (below its title bar and header), used as
    /// the viewport height until the scrollable reports one.
    pub fn pane_body_height(&self, kind: PaneKind) -> f32 {
        self.pane_body_size(kind).height
    }

    /// Size of a pane's scrolling body (see [`App::pane_body_height`]).
    pub fn pane_body_size(&self, kind: PaneKind) -> Size {
        self.panes
            .regions(self.grid_size())
            .into_iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, r)| {
                let header = if kind == PaneKind::Source {
                    SOURCE_HEADER_H
                } else {
                    0.0
                };
                Size::new(r.width.max(0.0), (r.height - TITLE_H - header).max(0.0))
            })
            .unwrap_or(Size::ZERO)
    }

    /// Adds a banner and mirrors it in the status bar.
    pub fn banner(&mut self, pane: Option<PaneKind>, level: Level, text: impl Into<String>) {
        let text = text.into();
        tracing::info!(target: "archalign::banner", ?level, "{text}");
        self.status_message = Some(text.clone());
        let id = self.next_banner;
        self.next_banner += 1;
        self.banners.push(Banner {
            id,
            pane,
            level,
            text,
        });
    }

    fn clear_pane_banners(&mut self, pane: PaneKind) {
        self.banners.retain(|b| b.pane != Some(pane));
    }

    /// Text fields currently in the view (for shortcut gating).
    fn text_inputs(&self) -> Vec<Id> {
        let mut ids = vec![ids::FOCUS_INPUT];
        if self.repo.is_some()
            && matches!(self.diagram.content, DiagramContent::Empty)
            && self.panes.is_visible(PaneKind::Diagram)
        {
            ids.push(ids::FOCUS_INPUT_EMPTY);
        }
        ids
    }

    /// Whether Archify can start (FR-PANE-09: disabled without a repo).
    pub fn can_archify(&self) -> bool {
        self.repo.is_some() && !self.run.is_running()
    }

    /// Whether an action's button is enabled; a disabled button's shortcut does nothing.
    pub fn action_enabled(&self, action: Action) -> bool {
        match action {
            Action::OpenFolder => self.can_open_repo(),
            Action::Archify => self.can_archify(),
            Action::CancelRun => self.run.is_running(),
            Action::Relayout => self.diagram.loaded().is_some() && !self.run.is_running(),
            Action::History
            | Action::TogglePane(_)
            | Action::ResetLayout
            | Action::ToggleTheme
            | Action::Settings
            | Action::Refresh => true,
        }
    }

    /// Whether Open repo is enabled (FR-IMP-06).
    pub fn can_open_repo(&self) -> bool {
        !self.run.is_running()
    }

    /// The `update` function.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Action(action) => self.perform(action),
            Message::KeyPressed(key, modifiers, captured) => {
                if let Some(task) = self.finder_key(&key, captured) {
                    return task;
                }
                if captured {
                    return Task::none();
                }
                if key == Key::Named(Named::Escape) && self.menu.is_some() {
                    self.menu = None;
                    return Task::none();
                }
                if self.diagram.focused
                    && let Some(task) = self.diagram_key(&key, modifiers)
                {
                    return task;
                }
                let Some(action) =
                    shortcut_action(&key, modifiers).filter(|a| self.action_enabled(*a))
                else {
                    return Task::none();
                };
                let check = self
                    .text_inputs()
                    .into_iter()
                    .fold(Task::done(false), |acc, id| {
                        acc.then(move |focused| {
                            if focused {
                                Task::done(true)
                            } else {
                                operation::is_focused(id.clone())
                            }
                        })
                    });
                check.map(move |focused| Message::ShortcutResolved(action, focused))
            }
            Message::ShortcutResolved(action, focused) => {
                if focused {
                    Task::none()
                } else {
                    // The same message its toolbar button sends (FR-PANE-09).
                    Task::done(Message::Action(action))
                }
            }
            Message::FolderPicked(Some(path)) => self.update(Message::OpenRepo(path)),
            Message::FolderPicked(None) => Task::none(),
            Message::OpenRepo(path) => self.open_repo(path),
            Message::RepoOpened(generation, result) => {
                if generation != self.generation {
                    return Task::none();
                }
                self.repo_opened(result)
            }
            Message::OpenRecent(i) => match self.recents.get(i) {
                Some(r) if !r.missing && self.can_open_repo() => {
                    let p = r.path.clone();
                    self.update(Message::OpenRepo(p))
                }
                _ => Task::none(),
            },
            Message::RemoveRecent(i) => {
                if i < self.recents.len() {
                    self.recents.remove(i);
                }
                Task::none()
            }
            Message::FocusChanged(s) => {
                self.focus = s.chars().take(crate::FOCUS_MAX_CHARS).collect();
                Task::none()
            }
            Message::ToggleMenu(menu) => {
                self.menu = if self.menu == Some(menu) {
                    None
                } else {
                    Some(menu)
                };
                Task::none()
            }
            Message::PaneClicked(_) => Task::none(),
            Message::PaneDragged(event) => {
                self.panes.drag(event);
                Task::none()
            }
            Message::PaneResized(event) => {
                let size = self.grid_size();
                self.panes.resize(event, size);
                Task::none()
            }
            Message::PaneMaximize(pane) => {
                self.panes.maximize(pane);
                Task::none()
            }
            Message::PaneRestore => {
                self.panes.restore();
                Task::none()
            }
            Message::PaneClose(pane) => {
                self.panes.close(pane);
                Task::none()
            }
            Message::TreeToggle(rel) => self.tree_toggle(rel),
            Message::TreeActivate(rel) => {
                self.tree.selected = Some(rel.clone());
                // FR-TREE-07: highlight the nodes citing this file (or clear).
                self.diagram.reverse = self
                    .diagram
                    .view()
                    .map(|v| v.index.nodes_citing(rel.as_str()).iter().copied().collect())
                    .unwrap_or_default();
                self.update(Message::OpenFile(rel, None))
            }
            Message::TreeListed(generation, dir, result) => {
                if generation != self.generation {
                    return Task::none();
                }
                self.tree.loading.remove(&dir);
                match result {
                    Ok(entries) => {
                        self.tree.errors.remove(&dir);
                        self.tree.children.insert(dir, entries);
                    }
                    Err(e) => {
                        self.tree.expanded.remove(&dir);
                        self.tree.errors.insert(dir.clone(), e.clone());
                        self.banner(
                            Some(PaneKind::Tree),
                            Level::Error,
                            format!("Can't list {dir}: {e}"),
                        );
                    }
                }
                self.continue_reveal()
            }
            Message::TreeScrolled(viewport) => {
                self.tree.scroll = virtual_list::ScrollState {
                    offset_y: viewport.absolute_offset().y,
                    viewport_h: viewport.bounds().height,
                };
                Task::none()
            }
            Message::OpenFile(rel, cite) => self.open_file(rel, cite),
            Message::FileLoaded(generation, rel, cite, result) => {
                if generation != self.generation || self.source.rel() != Some(&rel) {
                    return Task::none();
                }
                self.file_loaded(rel, cite, result)
            }
            Message::SourceScrolled(viewport) => {
                self.source.scroll = virtual_list::ScrollState {
                    offset_y: viewport.absolute_offset().y,
                    viewport_h: viewport.bounds().height,
                };
                self.source.viewport_w = viewport.bounds().width;
                let h = self.pane_body_height(PaneKind::Source);
                self.source.ensure_highlighted(h);
                Task::none()
            }
            Message::CopyPathLine => match self.source.copy_path_text() {
                Some(t) => iced::clipboard::write(t),
                None => Task::none(),
            },
            Message::CopyCitedLines => match self.source.cited_text() {
                Some(t) => iced::clipboard::write(t),
                None => Task::none(),
            },
            Message::DiagramOpen(rel) => self.open_diagram(rel),
            Message::DiagramLoaded(generation, result) => {
                if generation != self.generation {
                    return Task::none();
                }
                self.diagram_loaded(result);
                Task::none()
            }
            Message::Canvas(event) => self.canvas_event(event),
            Message::Run(msg) => self.run_msg(msg),
            Message::RevokeConsent(i) => {
                if i < self.consents.len() {
                    self.consents.remove(i);
                }
                Task::none()
            }
            Message::Diagram(msg) => self.diagram_msg(msg),
            Message::DismissBanner(id) => {
                self.banners.retain(|b| b.id != id);
                if self.banners.is_empty() {
                    self.status_message = None;
                }
                Task::none()
            }
            Message::WindowResized(size) => {
                self.window = size;
                Task::none()
            }
            Message::Warmed => {
                let h = self.pane_body_height(PaneKind::Source);
                self.source.ensure_highlighted(h);
                Task::none()
            }
        }
    }

    /// Runs a user action.
    pub fn perform(&mut self, action: Action) -> Task<Message> {
        match action {
            Action::OpenFolder => {
                if !self.can_open_repo() {
                    return Task::none();
                }
                Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Open repository folder")
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    Message::FolderPicked,
                )
            }
            Action::TogglePane(kind) => {
                let size = self.grid_size();
                self.panes.toggle(kind, size);
                Task::none()
            }
            Action::ResetLayout => {
                self.panes.reset();
                self.menu = None;
                Task::none()
            }
            Action::ToggleTheme => {
                self.theme = self.theme.toggled();
                let h = self.pane_body_height(PaneKind::Source);
                self.source.set_theme(self.theme, h);
                Task::none()
            }
            Action::History => {
                self.menu = if self.menu == Some(Menu::History) {
                    None
                } else {
                    Some(Menu::History)
                };
                Task::none()
            }
            Action::Relayout => self.relayout(),
            Action::Archify => self.archify(),
            Action::CancelRun => self.cancel_run(),
            // Wired in M6: Settings and Refresh.
            Action::Settings | Action::Refresh => Task::none(),
        }
    }

    fn open_repo(&mut self, path: PathBuf) -> Task<Message> {
        if !self.can_open_repo() {
            return Task::none();
        }
        self.generation += 1;
        let generation = self.generation;
        let show_hidden = self.settings.show_hidden_files;
        let git = self.git_program.clone();
        Task::perform(
            ops::blocking(move || ops::open_repo(path, show_hidden, &git)),
            move |r| {
                let r = r.unwrap_or_else(|| Err("Opening the folder was interrupted".to_string()));
                Message::RepoOpened(generation, r.map(Box::new))
            },
        )
    }

    fn repo_opened(&mut self, result: Result<Box<RepoOpened>, String>) -> Task<Message> {
        let opened = match result {
            Ok(o) => *o,
            Err(e) => {
                self.banner(None, Level::Error, e);
                return Task::none();
            }
        };
        // Reset the per-repo state (FR-IMP-02).
        self.tree = TreeState::default();
        self.source = SourceState::default();
        self.diagram = DiagramState::default();
        self.banners.clear();
        self.status_message = None;
        self.menu = None;
        // Recents: most recent first, unique, capped.
        self.recents.retain(|r| r.path != opened.root);
        self.recents.insert(
            0,
            Recent {
                path: opened.root.clone(),
                missing: false,
            },
        );
        self.recents.truncate(MAX_RECENTS);
        match opened.top {
            Ok(entries) => {
                self.tree.children.insert(RelPath::root(), entries);
            }
            Err(e) => self.banner(
                Some(PaneKind::Tree),
                Level::Error,
                format!("Can't list the repository: {e}"),
            ),
        }
        match &opened.git {
            GitStatus::Unavailable => self.banner(
                Some(PaneKind::Tree),
                Level::Info,
                "git was not found on PATH; repository metadata and modified badges are off",
            ),
            GitStatus::Subfolder { toplevel } => self.banner(
                Some(PaneKind::Tree),
                Level::Info,
                format!(
                    "This folder is inside the git work tree at {} but isn't its top level; git metadata is off",
                    toplevel.display()
                ),
            ),
            GitStatus::NotARepo | GitStatus::Repo(_) => {}
        }
        let newest = storage::newest_valid(&opened.history).map(|h| h.rel.clone());
        self.repo = Some(RepoSession {
            root: opened.root,
            sandbox: opened.sandbox,
            git: opened.git,
            history: opened.history,
        });
        match newest {
            Some(rel) => self.open_diagram(rel),
            None => Task::none(),
        }
    }

    fn tree_toggle(&mut self, rel: RelPath) -> Task<Message> {
        self.tree.selected = Some(rel.clone());
        if self.tree.expanded.remove(&rel) {
            return Task::none();
        }
        self.tree.expanded.insert(rel.clone());
        if self.tree.children.contains_key(&rel) {
            return Task::none();
        }
        self.list(rel)
    }

    fn list(&mut self, dir: RelPath) -> Task<Message> {
        let Some(repo) = &self.repo else {
            return Task::none();
        };
        if !self.tree.loading.insert(dir.clone()) {
            return Task::none();
        }
        let sandbox = repo.sandbox.clone();
        let generation = self.generation;
        let show_hidden = self.settings.show_hidden_files;
        Task::perform(
            ops::blocking({
                let dir = dir.clone();
                move || ops::list(&sandbox, &dir, show_hidden)
            }),
            move |r| {
                Message::TreeListed(
                    generation,
                    dir.clone(),
                    r.unwrap_or_else(|| Err("listing was interrupted".to_string())),
                )
            },
        )
    }

    /// FR-TREE-05: expands ancestors (listing them as needed), selects and scrolls to `rel`.
    pub fn reveal(&mut self, rel: RelPath) -> Task<Message> {
        self.tree.pending_reveal = Some(rel);
        self.continue_reveal()
    }

    fn continue_reveal(&mut self) -> Task<Message> {
        if self.tree.pending_reveal.is_none() {
            return Task::none();
        }
        if let Some(next) = self.tree.advance_reveal() {
            return self.list(next);
        }
        if !self.tree.reveal_ready() {
            return Task::none();
        }
        let Some(target) = self.tree.pending_reveal.take() else {
            return Task::none();
        };
        self.tree.selected = Some(target.clone());
        let Some(row) = self.tree.row_index(&target) else {
            return Task::none();
        };
        let scroll = self
            .tree
            .scroll
            .or_height(self.pane_body_height(PaneKind::Tree));
        let y = virtual_list::offset_to_reveal(row, tree::ROW_H, scroll);
        self.tree.scroll.offset_y = y;
        operation::scroll_to(ids::TREE_SCROLL, scrollable::AbsoluteOffset { x: 0.0, y })
    }

    fn open_file(&mut self, rel: RelPath, cite: Option<Cite>) -> Task<Message> {
        let Some(sandbox) = self.repo.as_ref().map(|r| r.sandbox.clone()) else {
            return Task::none();
        };
        self.clear_pane_banners(PaneKind::Source);
        self.source.content = SourceContent::Loading(rel.clone());
        self.source.band = None;
        let generation = self.generation;
        Task::perform(
            ops::blocking({
                let rel = rel.clone();
                move || ops::read_file(&sandbox, &rel)
            }),
            move |r| {
                Message::FileLoaded(
                    generation,
                    rel.clone(),
                    cite,
                    r.unwrap_or_else(|| Err("reading was interrupted".to_string()))
                        .map(Arc::new),
                )
            },
        )
    }

    fn file_loaded(
        &mut self,
        rel: RelPath,
        cite: Option<Cite>,
        result: Result<Arc<TextLoad>, String>,
    ) -> Task<Message> {
        let load = match result {
            Ok(l) => l,
            Err(e) => {
                let message = format!("Can't read {rel}: {e}");
                self.banner(Some(PaneKind::Source), Level::Error, message.clone());
                self.source.content = SourceContent::Error { rel, message };
                return Task::none();
            }
        };
        // The load is uniquely owned here; unwrap it instead of copying up to 2 MB of text.
        let load = Arc::try_unwrap(load).unwrap_or_else(|shared| (*shared).clone());
        match load {
            TextLoad::Binary { size } => {
                self.source.content = SourceContent::Placeholder {
                    rel,
                    size,
                    reason: PlaceholderReason::Binary,
                };
                Task::none()
            }
            TextLoad::TooLarge { size } => {
                self.source.content = SourceContent::Placeholder {
                    rel,
                    size,
                    reason: PlaceholderReason::TooLarge,
                };
                Task::none()
            }
            TextLoad::Text {
                text,
                lossy,
                index,
                size,
            } => {
                if lossy {
                    self.banner(
                        Some(PaneKind::Source),
                        Level::Warning,
                        format!("{rel} isn't valid UTF-8; invalid bytes are shown as \u{FFFD}"),
                    );
                }
                let first = if index.line_count() > 0 {
                    index.line(&text, 0)
                } else {
                    ""
                };
                let highlighter = source::select_syntax(rel.as_str(), first)
                    .map(|s| source::FileHighlighter::new(s, self.theme));
                let max_cols = (0..index.line_count())
                    .map(|n| index.line(&text, n).chars().count())
                    .max()
                    .unwrap_or(0);
                let line_count = index.line_count();
                self.source.content = SourceContent::Text(OpenText {
                    rel: rel.clone(),
                    text: Arc::new(text),
                    index: Arc::new(index),
                    lossy,
                    size,
                    max_cols,
                    highlighter,
                });
                let target_line = match apply_cite_with_banner(self, cite, line_count) {
                    Some((a, b)) => {
                        self.source.band = Some((a, b));
                        a
                    }
                    None => {
                        self.source.band = None;
                        1
                    }
                };
                let pane_h = self.pane_body_height(PaneKind::Source);
                let y = virtual_list::offset_for_top_third(
                    target_line.saturating_sub(1),
                    source::LINE_H,
                    self.source.scroll.or_height(pane_h).viewport_h,
                );
                self.source.scroll.offset_y = y;
                self.source.ensure_highlighted(pane_h);
                operation::scroll_to(ids::SOURCE_SCROLL, scrollable::AbsoluteOffset { x: 0.0, y })
            }
        }
    }

    fn open_diagram(&mut self, rel: RelPath) -> Task<Message> {
        let Some((sandbox, dirty)) = self.repo.as_ref().map(|r| (r.sandbox.clone(), r.dirty()))
        else {
            return Task::none();
        };
        self.clear_pane_banners(PaneKind::Diagram);
        self.diagram.content = DiagramContent::Loading(rel.clone());
        let generation = self.generation;
        Task::perform(
            ops::blocking({
                let rel = rel.clone();
                move || ops::load_diagram(&sandbox, &rel, &dirty)
            }),
            move |r| {
                let r = r.unwrap_or_else(|| {
                    Err(DiagramFailed {
                        rel: rel.clone(),
                        errors: vec![archalign_model::Issue::error(
                            archalign_model::IssueCode::Json,
                            "",
                            "loading was interrupted",
                        )],
                        unsupported: None,
                    })
                });
                Message::DiagramLoaded(generation, r.map(Box::new).map_err(Box::new))
            },
        )
    }

    fn diagram_loaded(&mut self, result: Result<Box<DiagramLoaded>, Box<DiagramFailed>>) {
        match result {
            Ok(d) => {
                let fallback = self.canvas_estimate();
                self.diagram.set_loaded(*d, fallback);
                if let Some(v) = self.diagram.view() {
                    self.tree.markers = tree::Markers {
                        cited: v.index.by_file.keys().cloned().collect(),
                        folder_counts: v.index.folder_counts.clone().into_iter().collect(),
                        broken: v.index.broken_files.iter().cloned().collect(),
                    };
                }
            }
            Err(f) => {
                let msg = match &f.unsupported {
                    Some(ty) => archalign_model::document::unsupported_message(
                        &archalign_model::DiagramType::Other(ty.clone()),
                    ),
                    None => format!(
                        "{} is not a valid Archify architecture diagram ({} error{})",
                        f.rel,
                        f.errors.len(),
                        if f.errors.len() == 1 { "" } else { "s" }
                    ),
                };
                self.banner(Some(PaneKind::Diagram), Level::Error, msg);
                self.diagram.content = DiagramContent::Failed(*f);
            }
        }
    }

    /// Estimated canvas size before the canvas reports one.
    pub fn canvas_estimate(&self) -> Size {
        let s = self.pane_body_size(PaneKind::Diagram);
        Size::new(s.width, (s.height - DETAIL_STRIP_H).max(1.0))
    }

    /// Keys while the finder is open: ↑/↓ move, Esc closes (even when the field used it).
    fn finder_key(&mut self, key: &Key, captured: bool) -> Option<Task<Message>> {
        self.diagram.finder.as_ref()?;
        match key {
            Key::Named(Named::Escape) => {
                self.diagram.finder = None;
                Some(Task::none())
            }
            Key::Named(Named::ArrowDown) if !captured => {
                self.diagram.move_cursor(1);
                Some(Task::none())
            }
            Key::Named(Named::ArrowUp) if !captured => {
                self.diagram.move_cursor(-1);
                Some(Task::none())
            }
            _ => None,
        }
    }

    /// Diagram-scoped shortcuts (§9); only while the canvas has focus.
    fn diagram_key(&mut self, key: &Key, modifiers: Modifiers) -> Option<Task<Message>> {
        self.diagram.view()?;
        let fallback = self.canvas_estimate();
        if let Key::Named(Named::Escape) = key {
            self.diagram.escape();
            return Some(Task::none());
        }
        let Key::Character(c) = key.as_ref() else {
            return None;
        };
        let c = c.to_lowercase();
        match (c.as_str(), modifiers.command()) {
            ("f", true) | ("/", false) => {
                self.diagram.set_query(String::new());
                return Some(operation::focus(ids::FINDER_INPUT));
            }
            (_, true) => return None,
            ("+" | "=", false) => self.diagram.zoom_center(camera_step(), fallback),
            ("-" | "_", false) => self.diagram.zoom_center(1.0 / camera_step(), fallback),
            ("0", false) => self.diagram.zoom_reset(fallback),
            ("f", false) => self.diagram.fit(fallback),
            ("u", false) => self.diagram.toggle_reach(Direction::Upstream),
            ("d", false) => self.diagram.toggle_reach(Direction::Downstream),
            _ => return None,
        }
        Some(Task::none())
    }

    fn canvas_event(&mut self, event: CanvasEvent) -> Task<Message> {
        let fallback = self.canvas_estimate();
        match event {
            CanvasEvent::Pressed(hit) => {
                self.diagram.focused = true;
                match hit {
                    Hit::Node(i) => return self.click_node(i),
                    Hit::Edge(k) => {
                        self.diagram.selection = Selection::Edge(k);
                        self.diagram.reach = None;
                    }
                    Hit::Empty => {}
                }
                Task::none()
            }
            CanvasEvent::Unfocused => {
                self.diagram.focused = false;
                Task::none()
            }
            CanvasEvent::Pan(d) => {
                self.diagram.camera.pan_by(d);
                self.diagram.fitted = false;
                Task::none()
            }
            CanvasEvent::ZoomAt { factor, anchor } => {
                self.diagram.camera.zoom_about(factor, anchor);
                self.diagram.fitted = false;
                Task::none()
            }
            CanvasEvent::Hover(Some(node), at) => {
                self.diagram.hover = Some(Hover {
                    node,
                    at,
                    shown: false,
                });
                Task::perform(async { tokio::time::sleep(HOVER_DELAY).await }, move |_| {
                    Message::Diagram(DiagramMsg::HoverElapsed(node))
                })
            }
            CanvasEvent::Hover(None, _) => {
                self.diagram.hover = None;
                Task::none()
            }
            CanvasEvent::Resized(size) => {
                self.diagram.canvas_size = Some(size);
                if self.diagram.fitted {
                    self.diagram.fit(fallback);
                }
                Task::none()
            }
        }
    }

    /// FR-DIA-03: select, open `sources[0]`, reveal the file in the tree.
    pub fn click_node(&mut self, i: usize) -> Task<Message> {
        self.diagram.selection = Selection::Node(i);
        if self
            .diagram
            .reach
            .as_ref()
            .is_some_and(|(_, r)| !r.nodes.contains(&i))
        {
            self.diagram.reach = None;
        }
        self.open_node_source(i, 0)
    }

    fn open_node_source(&mut self, i: usize, j: usize) -> Task<Message> {
        let Some(src) = self
            .diagram
            .loaded()
            .and_then(|d| d.doc.components.get(i))
            .and_then(|c| c.sources().get(j))
            .cloned()
        else {
            return Task::none();
        };
        let Ok(rel) = RelPath::parse(&src.path) else {
            self.banner(
                Some(PaneKind::Source),
                Level::Warning,
                format!("{} is not a valid repository path", src.path),
            );
            return Task::none();
        };
        let cite = Cite {
            line: src.line,
            end_line: src.end_line,
        };
        let open = self.open_file(rel.clone(), Some(cite));
        let reveal = self.reveal(rel);
        Task::batch([open, reveal])
    }

    fn diagram_msg(&mut self, msg: DiagramMsg) -> Task<Message> {
        let fallback = self.canvas_estimate();
        match msg {
            DiagramMsg::OpenSource(i, j) => self.open_node_source(i, j),
            DiagramMsg::Reach(dir) => {
                self.diagram.toggle_reach(dir);
                Task::none()
            }
            DiagramMsg::FinderQuery(q) => {
                self.diagram.set_query(q);
                Task::none()
            }
            DiagramMsg::FinderSubmit => {
                let pick = self
                    .diagram
                    .finder
                    .as_ref()
                    .and_then(|f| f.results.get(f.cursor).copied());
                match pick {
                    Some(i) => self.diagram_msg(DiagramMsg::FinderPick(i)),
                    None => Task::none(),
                }
            }
            DiagramMsg::FinderPick(i) => {
                self.diagram.finder = None;
                self.diagram.focused = true;
                self.diagram.center_node(i, fallback);
                self.click_node(i)
            }
            DiagramMsg::ToggleCards => {
                self.diagram.cards_open = !self.diagram.cards_open;
                Task::none()
            }
            DiagramMsg::ToggleProblems => {
                self.diagram.problems_open = !self.diagram.problems_open;
                Task::none()
            }
            DiagramMsg::HoverElapsed(node) => {
                if let Some(h) = &mut self.diagram.hover
                    && h.node == node
                {
                    h.shown = true;
                }
                Task::none()
            }
        }
    }

    /// FR-DIA-11: strip geometry, lay out again, save atomically, redraw.
    fn relayout(&mut self) -> Task<Message> {
        if !self.action_enabled(Action::Relayout) {
            return Task::none();
        }
        let (Some(repo), Some(d)) = (self.repo.as_ref(), self.diagram.loaded()) else {
            return Task::none();
        };
        let sandbox = repo.sandbox.clone();
        let dirty = repo.dirty();
        let rel = d.rel.clone();
        let doc = (*d.doc).clone();
        let generation = self.generation;
        let rel2 = rel.clone();
        Task::perform(
            ops::blocking(move || ops::relayout(&sandbox, &rel, doc, &dirty)),
            move |r| {
                let r = r.unwrap_or_else(|| Err(ops::interrupted_failure(rel2.clone())));
                Message::DiagramLoaded(generation, r.map(Box::new).map_err(Box::new))
            },
        )
    }

    /// FR-GEN-01: checks the preconditions, asks for consent, creates the diagram folder and
    /// starts the run. Every failure shows a banner and makes no model call.
    fn archify(&mut self) -> Task<Message> {
        if !self.can_archify() {
            return Task::none();
        }
        let Some(repo) = self.repo.clone() else {
            return Task::none();
        };
        let model = self.settings.model.clone();
        let Some(provider) = archalign_agent::litellm::provider_of(&model) else {
            self.banner(
                Some(PaneKind::Diagram),
                Level::Error,
                format!(
                    "Unknown provider in model `{model}`; use a litellm `provider/model` string"
                ),
            );
            return Task::none();
        };
        let (_, key) = keys::resolve(&self.key_env, provider.key_env);
        let Some(key) = key else {
            let mut places = vec!["the environment".to_string()];
            places.extend(
                self.key_env
                    .launch_dir
                    .iter()
                    .map(|d| format!("{}", d.join(".env").display())),
            );
            places.extend(
                self.key_env
                    .config_dir
                    .iter()
                    .map(|d| format!("{}", d.join(".env").display())),
            );
            self.banner(
                Some(PaneKind::Diagram),
                Level::Error,
                format!(
                    "No API key found: set {} in {}",
                    provider.key_env,
                    places.join(", or in ")
                ),
            );
            return Task::none();
        };
        if !self
            .consents
            .iter()
            .any(|c| c.repo == repo.root && c.provider == provider.name)
        {
            self.consent_prompt = Some((provider.name.to_string(), self.settings.model_name()));
            return Task::none();
        }
        let client = match (self.llm)(&LlmSpec {
            model: model.clone(),
            key,
            request_timeout: self.settings.budgets.request_timeout(),
        }) {
            Ok(c) => c,
            Err(e) => {
                self.banner(
                    Some(PaneKind::Diagram),
                    Level::Error,
                    format!("Can't start the model client: {e}"),
                );
                return Task::none();
            }
        };
        let repo_name = repo
            .root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repo")
            .to_string();
        let slug = archalign_repo::slugify(&self.focus, &repo_name);
        let folder = match archalign_repo::create_run_folder(
            &repo.sandbox,
            &slug,
            &archalign_repo::now_run_timestamp(),
        ) {
            Ok(f) => f,
            Err(e) => {
                self.banner(
                    Some(PaneKind::Diagram),
                    Level::Error,
                    format!("Can't create the diagram folder: {e}"),
                );
                return Task::none();
            }
        };
        let (summary, meta, dirty) = agent_run::repo_facts(&repo.git, &repo_name);
        let repository = match &repo.git {
            GitStatus::Repo(info) => info.repository(),
            _ => None,
        };
        let cfg = archalign_agent::RunConfig::new(
            model,
            self.settings
                .reasoning_effort
                .clone()
                .filter(|s| !s.trim().is_empty()),
            self.settings.budgets,
            self.focus.clone(),
            String::new(),
            summary.clone(),
            archalign_model::AppOwned {
                output: format!(".archify/{}/{}.html", folder.name, folder.slug),
                repository,
            },
            dirty,
        );
        let prepared = PreparedRun {
            cfg,
            client,
            sandbox: repo.sandbox.clone(),
            meta,
            folder: folder.rel.clone(),
            writes: self.self_writes.clone(),
            seed: archalign_agent::prompt::SeedInput {
                focus: self.focus.clone(),
                repo_name,
                short_head: summary.head.as_ref().map(|h| h.chars().take(7).collect()),
                dirty_count: summary.dirty_count,
            },
        };
        self.run = RunState::Running {
            turn: 0,
            max_turns: self.settings.budgets.max_turns,
            input_tokens: 0,
            elapsed_s: 0,
        };
        self.run_started_at = Some(std::time::Instant::now());
        self.run_log = vec![format!(
            "Run started in .archify/{} with {}",
            folder.name, self.settings.model
        )];
        self.run_drawer_open = true;
        self.menu = None;
        let (task, handle) = Task::run(agent_run::stream(prepared), |m| m).abortable();
        self.run_handle = Some(handle);
        task
    }

    /// FR-GEN-10: ends the run; the dropped run records `cancelled` and saves nothing.
    fn cancel_run(&mut self) -> Task<Message> {
        if let Some(h) = self.run_handle.take() {
            h.abort();
        }
        if self.run.is_running() {
            self.run = RunState::Idle;
            self.run_started_at = None;
            self.run_log.push("✗ cancelled".into());
            self.banner(
                Some(PaneKind::Diagram),
                Level::Info,
                "Run cancelled; no diagram was saved",
            );
        }
        Task::none()
    }

    fn run_msg(&mut self, msg: RunMsg) -> Task<Message> {
        match msg {
            RunMsg::Tick => {
                if let (RunState::Running { elapsed_s, .. }, Some(t)) =
                    (&mut self.run, self.run_started_at)
                {
                    *elapsed_s = t.elapsed().as_secs();
                }
                Task::none()
            }
            RunMsg::ToggleDrawer => {
                self.run_drawer_open = !self.run_drawer_open;
                Task::none()
            }
            RunMsg::ConsentAccept => {
                let Some((provider, model)) = self.consent_prompt.take() else {
                    return Task::none();
                };
                if let Some(repo) = &self.repo {
                    self.consents.push(Consent {
                        repo: repo.root.clone(),
                        provider,
                        model,
                        granted_at: archalign_repo::now_rfc3339(),
                    });
                }
                self.archify()
            }
            RunMsg::ConsentDecline => {
                self.consent_prompt = None;
                self.banner(
                    Some(PaneKind::Diagram),
                    Level::Info,
                    "Archify cancelled: nothing was sent. Existing diagrams stay available.",
                );
                Task::none()
            }
            RunMsg::Event(event) => {
                if !self.run.is_running() {
                    return Task::none();
                }
                self.run_log.push(agent_run::event_line(&event));
                match event {
                    archalign_agent::AgentEvent::TurnStarted { n, .. } => {
                        if let RunState::Running { turn, .. } = &mut self.run {
                            *turn = n;
                        }
                        Task::none()
                    }
                    archalign_agent::AgentEvent::Usage {
                        input_tokens: n, ..
                    } => {
                        if let RunState::Running { input_tokens, .. } = &mut self.run {
                            *input_tokens = n;
                        }
                        Task::none()
                    }
                    archalign_agent::AgentEvent::Finished(outcome) => self.run_finished(outcome),
                    _ => Task::none(),
                }
            }
        }
    }

    fn run_finished(&mut self, outcome: archalign_agent::RunOutcome) -> Task<Message> {
        self.run = RunState::Idle;
        self.run_handle = None;
        self.run_started_at = None;
        match outcome {
            archalign_agent::RunOutcome::Success { candidate } => {
                // FR-GEN-12: refresh History and open the new diagram.
                if let Some(repo) = &mut self.repo {
                    repo.history = archalign_repo::storage::scan_history(&repo.sandbox);
                }
                self.open_diagram(candidate)
            }
            archalign_agent::RunOutcome::Failed { reason } => {
                self.banner(
                    Some(PaneKind::Diagram),
                    Level::Error,
                    format!("Archify failed: {reason}"),
                );
                Task::none()
            }
        }
    }

    /// Subscriptions: keyboard shortcuts and window resizes.
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            event::listen_with(|event, status, _window| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => Some(
                    Message::KeyPressed(key, modifiers, status == event::Status::Captured),
                ),
                _ => None,
            }),
            iced::window::resize_events().map(|(_, size)| Message::WindowResized(size)),
            if self.run.is_running() {
                iced::time::every(std::time::Duration::from_secs(1))
                    .map(|_| Message::Run(RunMsg::Tick))
            } else {
                Subscription::none()
            },
        ])
    }

    /// The iced theme.
    pub fn iced_theme(&self) -> iced::Theme {
        match self.theme {
            ThemeChoice::Dark => crate::view::DARK_THEME.clone(),
            ThemeChoice::Light => crate::view::LIGHT_THEME.clone(),
        }
    }
}

/// Applies a citation and raises the out-of-range banner (FR-SRC-05). Returns the band.
fn apply_cite_with_banner(
    app: &mut App,
    cite: Option<Cite>,
    line_count: usize,
) -> Option<(usize, usize)> {
    match source::apply_cite(cite.unwrap_or_default(), line_count) {
        CiteOutcome::Top => None,
        CiteOutcome::Band(a, b) => Some((a, b)),
        CiteOutcome::OutOfRange { line, end, lines } => {
            app.banner(
                Some(PaneKind::Source),
                Level::Warning,
                format!("Cited lines {line}–{end} are outside this file ({lines} lines)"),
            );
            None
        }
    }
}
