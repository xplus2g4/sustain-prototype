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
use crate::settings::{Consent, Settings, SettingsDraft};
use crate::source::{
    self, Cite, CiteOutcome, OpenText, PlaceholderReason, SourceContent, SourceState,
};
use crate::tree::{self, TreeState};
use crate::virtual_list;
use archalign_repo::{GitRefreshLimiter, SelfWrites, WatchBatch};

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

/// Settings modal messages (FR-STO-06).
#[derive(Debug, Clone)]
pub enum SettingsMsg {
    /// Model field.
    Model(String),
    /// `reasoning_effort` field.
    Effort(String),
    /// Budget field `i` (see `settings::BUDGET_FIELDS`).
    Budget(usize, String),
    /// Show hidden files.
    ShowHidden(bool),
    /// Save and close.
    Save,
    /// Close without saving.
    Cancel,
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
    /// Where `state.json`/`settings.json` are saved; `None` disables persistence (tests).
    pub config_dir: Option<PathBuf>,
    /// The saved pane layout.
    pub panes: Option<crate::persist::PanesState>,
    /// The saved window size.
    pub window: Option<Size>,
    /// Problems found while loading the saved files, shown as banners at launch.
    pub startup_notes: Vec<(Level, String)>,
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
    /// Settings modal messages.
    Settings(SettingsMsg),
    /// The watcher for repo generation `.0` started.
    WatchStarted(u64),
    /// The watcher for repo generation `.0` couldn't start or failed.
    WatchFailed(u64, String),
    /// A filtered, debounced batch from the watcher (FR-WAT).
    Watch(u64, WatchBatch),
    /// The open file was re-read after a change; `None` means it was deleted.
    FileReloaded(u64, RelPath, Result<Option<Arc<TextLoad>>, String>),
    /// Citations of the open diagram were re-checked (diagram path, citation issues).
    CitationsRechecked(u64, RelPath, Vec<archalign_model::Issue>),
    /// The open diagram was re-read after an external change.
    DiagramReloaded(u64, Result<Box<DiagramLoaded>, Box<DiagramFailed>>),
    /// History was rescanned.
    HistoryScanned(u64, Vec<HistoryEntry>),
    /// A rate-limited git refresh is due.
    GitRefreshDue(u64),
    /// Git metadata was collected.
    GitCollected(u64, GitStatus),
    /// Open a History entry.
    HistoryOpen(usize),
    /// The state-save delay passed.
    SaveStateDue,
    /// A state or settings save finished.
    Saved(Result<(), String>),
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
    /// Where state and settings are saved (`None`: not persisted).
    pub config_dir: Option<PathBuf>,
    /// The last repository opened (kept while none is open).
    pub last_repo: Option<PathBuf>,
    /// The state last written (or being written).
    saved_state: Option<crate::persist::AppState>,
    /// When the state was last written.
    last_state_save: Option<std::time::Instant>,
    /// A delayed save is scheduled.
    save_scheduled: bool,
    /// Banners from launch, kept when a repo opens.
    startup_banners: Vec<u64>,
    /// The Settings modal, when open.
    pub settings_draft: Option<SettingsDraft>,
    /// FR-WAT-09 rate limit.
    git_limiter: GitRefreshLimiter,
    /// The last Settings validation error.
    pub settings_error: Option<String>,
    /// Where the API key for the current model comes from (computed when Settings opens).
    pub settings_key_source: String,
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
            panes: boot
                .panes
                .as_ref()
                .map(crate::persist::PanesState::restore)
                .unwrap_or_default(),
            window: boot.window.unwrap_or(DEFAULT_WINDOW),
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
            config_dir: boot.config_dir.clone(),
            last_repo: boot.last_repo.clone(),
            saved_state: None,
            last_state_save: None,
            save_scheduled: false,
            startup_banners: Vec::new(),
            settings_draft: None,
            git_limiter: GitRefreshLimiter::default(),
            settings_error: None,
            settings_key_source: String::new(),
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
        let mut app = app;
        for (level, text) in &boot.startup_notes {
            app.banner(None, *level, text.clone());
            app.startup_banners.push(app.next_banner - 1);
        }
        // What's on disk now, so the first save happens only after a change.
        if boot.startup_notes.is_empty() {
            app.saved_state = Some(app.app_state());
        }
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
        let task = self.handle(message);
        match self.persist_state() {
            Some(save) => Task::batch([task, save]),
            None => task,
        }
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
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
                        // A reveal through a folder that can't be listed stops here instead
                        // of retrying forever (NFR-QUAL-04).
                        if self
                            .tree
                            .pending_reveal
                            .as_ref()
                            .is_some_and(|t| TreeState::ancestors(t).contains(&dir))
                        {
                            self.tree.pending_reveal = None;
                        }
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
            Message::Settings(msg) => self.settings_msg(msg),
            Message::WatchStarted(g) => {
                if g == self.generation {
                    self.watch = WatchStatus::Watching;
                }
                Task::none()
            }
            Message::WatchFailed(g, e) => {
                if g == self.generation {
                    tracing::warn!(error = %e, "file watcher unavailable");
                    self.watch = WatchStatus::ManualOnly;
                }
                Task::none()
            }
            Message::Watch(g, batch) => {
                if g != self.generation {
                    return Task::none();
                }
                self.watch_batch(batch)
            }
            Message::FileReloaded(g, rel, result) => {
                if g != self.generation || self.source.rel() != Some(&rel) {
                    return Task::none();
                }
                match result {
                    Ok(Some(load)) => self.file_reloaded(rel, load),
                    Ok(None) => {
                        self.banner(
                            Some(PaneKind::Source),
                            Level::Warning,
                            format!("File deleted: {rel}"),
                        );
                        Task::none()
                    }
                    Err(e) => {
                        self.banner(
                            Some(PaneKind::Source),
                            Level::Error,
                            format!("Can't read {rel}: {e}"),
                        );
                        Task::none()
                    }
                }
            }
            Message::CitationsRechecked(g, rel, issues) => {
                if g != self.generation || self.diagram.loaded().map(|d| &d.rel) != Some(&rel) {
                    return Task::none();
                }
                self.diagram.replace_citation_issues(issues);
                self.sync_tree_markers();
                Task::none()
            }
            Message::DiagramReloaded(g, result) => {
                if g != self.generation {
                    return Task::none();
                }
                match result {
                    Ok(d) => {
                        let fallback = self.canvas_estimate();
                        self.diagram.replace_loaded(*d, fallback);
                        self.sync_tree_markers();
                    }
                    Err(f) => {
                        // FR-WAT-06: keep the last good version.
                        let first = f
                            .errors
                            .first()
                            .map(|i| i.message.clone())
                            .unwrap_or_default();
                        self.banner(
                            Some(PaneKind::Diagram),
                            Level::Warning,
                            format!("{} changed on disk and is now invalid ({first}); showing the last good version", f.rel),
                        );
                    }
                }
                Task::none()
            }
            Message::HistoryScanned(g, entries) => {
                if g == self.generation
                    && let Some(repo) = &mut self.repo
                {
                    repo.history = entries;
                }
                Task::none()
            }
            Message::GitRefreshDue(g) => {
                if g != self.generation {
                    return Task::none();
                }
                self.git_refresh_now()
            }
            Message::GitCollected(g, status) => {
                if g != self.generation {
                    return Task::none();
                }
                self.git_limiter.done(std::time::Instant::now());
                let changed = match &mut self.repo {
                    Some(repo) if repo.git != status => {
                        repo.git = status;
                        true
                    }
                    _ => false,
                };
                // Dirty paths drive the "modified" badges (FR-DIA-09).
                if changed {
                    self.recheck_citations()
                } else {
                    Task::none()
                }
            }
            Message::HistoryOpen(i) => {
                let rel = self
                    .repo
                    .as_ref()
                    .and_then(|r| r.history.get(i))
                    .map(|e| e.rel.clone());
                self.menu = None;
                match rel {
                    Some(rel) => self.open_diagram(rel),
                    None => Task::none(),
                }
            }
            Message::SaveStateDue => {
                self.save_scheduled = false;
                self.persist_state().unwrap_or_else(Task::none)
            }
            Message::Saved(result) => {
                if let Err(e) = result {
                    tracing::warn!(error = %e, "saving app state failed");
                    self.banner(None, Level::Warning, format!("Can't save app state: {e}"));
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
            Action::Settings => {
                self.menu = None;
                self.settings_error = None;
                self.settings_draft = Some(SettingsDraft::of(&self.settings));
                self.settings_key_source =
                    match archalign_agent::litellm::provider_of(&self.settings.model) {
                        Some(p) => {
                            // Only the source is kept; the key itself is dropped here (FR-STO-06).
                            let (source, _) = keys::resolve(&self.key_env, p.key_env);
                            format!("{} — {source}", p.key_env)
                        }
                        None => "unknown provider".into(),
                    };
                Task::none()
            }
            Action::Refresh => self.refresh_all(),
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
        let keep = self.startup_banners.clone();
        self.banners.retain(|b| keep.contains(&b.id));
        self.status_message = None;
        self.menu = None;
        self.last_repo = Some(opened.root.clone());
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
        let writes = self.self_writes.clone();
        Task::perform(
            ops::blocking(move || ops::relayout(&sandbox, &rel, doc, &dirty, &writes)),
            move |r| {
                let r = r.unwrap_or_else(|| Err(ops::interrupted_failure(rel2.clone())));
                Message::DiagramLoaded(generation, r.map(Box::new).map_err(Box::new))
            },
        )
    }

    /// The persisted part of the app state.
    pub fn app_state(&self) -> crate::persist::AppState {
        crate::persist::AppState {
            version: 1,
            window: crate::persist::WindowSize {
                width: self.window.width,
                height: self.window.height,
            },
            panes: Some(crate::persist::PanesState::capture(&self.panes)),
            theme: self.theme,
            last_repo: self.last_repo.clone(),
            recent_repos: self.recents.iter().map(|r| r.path.clone()).collect(),
            consents: self.consents.clone(),
        }
    }

    /// Saves `state.json` when the state changed, at most once per second (FR-PANE-08,
    /// FR-STO-05). Returns the save (or the delayed retry) task.
    fn persist_state(&mut self) -> Option<Task<Message>> {
        let dir = self.config_dir.clone()?;
        if self.save_scheduled {
            return None;
        }
        let state = self.app_state();
        if self.saved_state.as_ref() == Some(&state) {
            return None;
        }
        let now = std::time::Instant::now();
        if let Some(last) = self.last_state_save {
            let since = now.duration_since(last);
            if since < crate::persist::SAVE_INTERVAL {
                self.save_scheduled = true;
                let wait = crate::persist::SAVE_INTERVAL - since;
                return Some(Task::perform(
                    async move { tokio::time::sleep(wait).await },
                    |_| Message::SaveStateDue,
                ));
            }
        }
        self.last_state_save = Some(now);
        self.saved_state = Some(state.clone());
        Some(Task::perform(
            ops::blocking(move || crate::persist::save_state(&dir, &state)),
            saved,
        ))
    }

    fn settings_msg(&mut self, msg: SettingsMsg) -> Task<Message> {
        let Some(draft) = self.settings_draft.as_mut() else {
            return Task::none();
        };
        match msg {
            SettingsMsg::Model(s) => draft.model = s,
            SettingsMsg::Effort(s) => draft.reasoning_effort = s,
            SettingsMsg::Budget(i, s) => {
                if let Some(f) = draft.budgets.get_mut(i) {
                    *f = s;
                }
            }
            SettingsMsg::ShowHidden(b) => draft.show_hidden_files = b,
            SettingsMsg::Cancel => {
                self.settings_draft = None;
                self.settings_error = None;
            }
            SettingsMsg::Save => {
                let next = match draft.apply(&self.settings) {
                    Ok(s) => s,
                    Err(e) => {
                        self.settings_error = Some(e);
                        return Task::none();
                    }
                };
                self.settings_draft = None;
                self.settings_error = None;
                let hidden_changed = next.show_hidden_files != self.settings.show_hidden_files;
                self.settings = next.clone();
                let save = match self.config_dir.clone() {
                    Some(dir) => Task::perform(
                        ops::blocking(move || crate::persist::save_settings(&dir, &next)),
                        saved,
                    ),
                    None => Task::none(),
                };
                let relist = if hidden_changed {
                    self.refresh_tree()
                } else {
                    Task::none()
                };
                return Task::batch([save, relist]);
            }
        }
        Task::none()
    }

    /// Re-lists the root and every expanded folder, keeping expansion and selection.
    pub fn refresh_tree(&mut self) -> Task<Message> {
        if self.repo.is_none() {
            return Task::none();
        }
        let mut dirs = vec![RelPath::root()];
        let mut expanded: Vec<_> = self.tree.expanded.iter().cloned().collect();
        expanded.sort();
        dirs.extend(expanded);
        let tasks: Vec<_> = dirs.into_iter().map(|dir| self.list(dir)).collect();
        Task::batch(tasks)
    }

    /// Handles one watcher batch (FR-WAT-03..09).
    fn watch_batch(&mut self, batch: WatchBatch) -> Task<Message> {
        let changes = match batch {
            WatchBatch::Full => return self.refresh_all(),
            WatchBatch::Error(e) => {
                tracing::warn!(error = %e, "file watcher error");
                return Task::none();
            }
            WatchBatch::Changes(c) => c,
        };
        let mut tasks = Vec::new();
        // FR-WAT-03: re-list only affected folders that are listed and expanded (or the root).
        for dir in &changes.dirs {
            if dir.is_root()
                || (self.tree.expanded.contains(dir) && self.tree.children.contains_key(dir))
            {
                tasks.push(self.list(dir.clone()));
            }
        }
        // FR-WAT-04: the open file.
        if let Some(rel) = self.source.rel().cloned()
            && changes.paths.contains(&rel)
        {
            tasks.push(self.reload_file(rel));
        }
        // FR-WAT-06: History, and the open candidate.json.
        if changes.archify {
            tasks.push(self.rescan_history());
        }
        let open_diagram = self.diagram.loaded().map(|d| d.rel.clone());
        if let Some(rel) = &open_diagram
            && changes.paths.contains(rel)
            && !self.self_writes_own(rel)
        {
            tasks.push(self.reload_diagram(rel.clone()));
        } else if let Some(d) = self.diagram.loaded()
            && changes.paths.iter().any(|p| {
                d.doc
                    .components
                    .iter()
                    .any(|c| c.sources().iter().any(|s| cites(&s.path, p)))
            })
        {
            // FR-WAT-05: re-check citations of changed paths.
            tasks.push(self.recheck_citations());
        }
        // FR-WAT-09: git metadata, rate-limited.
        if changes.git {
            tasks.push(self.request_git_refresh());
        }
        Task::batch(tasks)
    }

    fn self_writes_own(&self, rel: &RelPath) -> bool {
        self.repo
            .as_ref()
            .and_then(|r| r.sandbox.resolve(rel).ok())
            .is_some_and(|abs| self.self_writes.is_own(&abs))
    }

    /// FR-WAT-10: ⌘R refreshes the tree, the open file, citations, History and git metadata.
    pub fn refresh_all(&mut self) -> Task<Message> {
        if self.repo.is_none() {
            return Task::none();
        }
        let mut tasks = vec![
            self.refresh_tree(),
            self.rescan_history(),
            self.git_refresh_now(),
        ];
        if let Some(rel) = self.source.rel().cloned() {
            tasks.push(self.reload_file(rel));
        }
        tasks.push(self.recheck_citations());
        self.status_message = Some("Refreshed".into());
        Task::batch(tasks)
    }

    fn reload_file(&mut self, rel: RelPath) -> Task<Message> {
        let Some(sandbox) = self.repo.as_ref().map(|r| r.sandbox.clone()) else {
            return Task::none();
        };
        let generation = self.generation;
        Task::perform(
            ops::blocking({
                let rel = rel.clone();
                move || match sandbox.resolve(&rel) {
                    Ok(abs) if abs.exists() => {
                        ops::read_file(&sandbox, &rel).map(|l| Some(Arc::new(l)))
                    }
                    Ok(_) => Ok(None),
                    Err(_) if !sandbox.root().join(rel.as_str()).exists() => Ok(None),
                    Err(e) => Err(e.to_string()),
                }
            }),
            move |r| {
                Message::FileReloaded(
                    generation,
                    rel.clone(),
                    r.unwrap_or_else(|| Err("reading was interrupted".to_string())),
                )
            },
        )
    }

    /// Installs a re-read file, keeping the scroll position and the band (FR-WAT-04).
    fn file_reloaded(&mut self, rel: RelPath, load: Arc<TextLoad>) -> Task<Message> {
        let band = self.source.band;
        let offset = self.source.scroll.offset_y;
        self.clear_pane_banners(PaneKind::Source);
        let _ = self.file_loaded(rel, None, Ok(load));
        if let Some(n) = self.source.line_count() {
            self.source.band = band.filter(|(a, _)| *a <= n).map(|(a, b)| (a, b.min(n)));
        }
        self.source.scroll.offset_y = offset;
        let h = self.pane_body_height(PaneKind::Source);
        self.source.ensure_highlighted(h);
        operation::scroll_to(
            ids::SOURCE_SCROLL,
            scrollable::AbsoluteOffset { x: 0.0, y: offset },
        )
    }

    fn rescan_history(&mut self) -> Task<Message> {
        let Some(sandbox) = self.repo.as_ref().map(|r| r.sandbox.clone()) else {
            return Task::none();
        };
        let generation = self.generation;
        Task::perform(
            ops::blocking(move || archalign_repo::storage::scan_history(&sandbox)),
            move |r| Message::HistoryScanned(generation, r.unwrap_or_default()),
        )
    }

    fn reload_diagram(&mut self, rel: RelPath) -> Task<Message> {
        let Some((sandbox, dirty)) = self.repo.as_ref().map(|r| (r.sandbox.clone(), r.dirty()))
        else {
            return Task::none();
        };
        let generation = self.generation;
        Task::perform(
            ops::blocking({
                let rel = rel.clone();
                move || ops::load_diagram(&sandbox, &rel, &dirty)
            }),
            move |r| match r {
                Some(r) => Message::DiagramReloaded(generation, r.map(Box::new).map_err(Box::new)),
                None => Message::DiagramReloaded(
                    generation,
                    Err(Box::new(DiagramFailed {
                        rel: rel.clone(),
                        errors: vec![],
                        unsupported: None,
                    })),
                ),
            },
        )
    }

    fn recheck_citations(&mut self) -> Task<Message> {
        let Some((sandbox, dirty)) = self.repo.as_ref().map(|r| (r.sandbox.clone(), r.dirty()))
        else {
            return Task::none();
        };
        let Some(d) = self.diagram.loaded() else {
            return Task::none();
        };
        let doc = d.doc.clone();
        let rel = d.rel.clone();
        let generation = self.generation;
        Task::perform(
            ops::blocking(move || ops::citation_issues(&sandbox, &doc, &dirty)),
            move |r| Message::CitationsRechecked(generation, rel.clone(), r.unwrap_or_default()),
        )
    }

    fn request_git_refresh(&mut self) -> Task<Message> {
        match self.git_limiter.request(std::time::Instant::now()) {
            Some(d) if d.is_zero() => self.git_refresh_now(),
            Some(d) => {
                let generation = self.generation;
                Task::perform(async move { tokio::time::sleep(d).await }, move |_| {
                    Message::GitRefreshDue(generation)
                })
            }
            None => Task::none(),
        }
    }

    fn git_refresh_now(&mut self) -> Task<Message> {
        let Some(root) = self.repo.as_ref().map(|r| r.root.clone()) else {
            return Task::none();
        };
        let program = self.git_program.clone();
        let generation = self.generation;
        Task::perform(
            ops::blocking(move || archalign_repo::git::collect_with(&program, &root)),
            move |r| Message::GitCollected(generation, r.unwrap_or(GitStatus::Unavailable)),
        )
    }

    fn sync_tree_markers(&mut self) {
        if let Some(v) = self.diagram.view() {
            self.tree.markers = tree::Markers {
                cited: v.index.by_file.keys().cloned().collect(),
                folder_counts: v.index.folder_counts.clone().into_iter().collect(),
                broken: v.index.broken_files.iter().cloned().collect(),
            };
        }
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
            match &self.repo {
                Some(r) => crate::watch::subscription(crate::watch::WatchKey {
                    generation: self.generation,
                    sandbox: r.sandbox.clone(),
                    writes: self.self_writes.clone(),
                    show_hidden: self.settings.show_hidden_files,
                }),
                None => Subscription::none(),
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

fn saved(r: Option<Result<(), String>>) -> Message {
    Message::Saved(r.unwrap_or_else(|| Err("the save was interrupted".to_string())))
}

/// Whether a citation of `cited` is affected by a change at `changed` (the file itself or
/// a folder above it).
fn cites(cited: &str, changed: &RelPath) -> bool {
    let c = changed.as_str();
    cited == c || (cited.starts_with(c) && cited.as_bytes().get(c.len()) == Some(&b'/'))
}
