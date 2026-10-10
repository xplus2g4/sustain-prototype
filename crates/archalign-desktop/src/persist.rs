//! `state.json` and `settings.json` in the OS config dir (FR-STO-05, FR-PANE-08, ADR 0015).
//!
//! Both files are written atomically. A missing, unreadable or invalid file falls back to
//! defaults and yields a banner message. An invalid pane layout inside an otherwise good
//! `state.json` falls back to the default layout alone, with a warning.

use std::path::{Path, PathBuf};

use archalign_repo::{RelPath, Sandbox};
use iced::Size;
use iced::widget::pane_grid::{self, Axis, Configuration, Node};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::app::MAX_RECENTS;
use crate::palette::ThemeChoice;
use crate::panes::{PaneKind, PaneLayout};
use crate::settings::{Consent, Settings};

/// File names inside the config dir.
pub const STATE_FILE: &str = "state.json";
/// See [`STATE_FILE`].
pub const SETTINGS_FILE: &str = "settings.json";

/// Minimum time between two state saves (Appendix C).
pub const SAVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// The OS config dir for `ArchAlign` (`~/Library/Application Support/ArchAlign` on macOS).
pub fn config_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "ArchAlign").map(|d| d.config_dir().to_path_buf())
}

/// Window size in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowSize {
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Default for WindowSize {
    fn default() -> Self {
        Self {
            width: crate::app::DEFAULT_WINDOW.width,
            height: crate::app::DEFAULT_WINDOW.height,
        }
    }
}

/// Split axis in the serde mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitAxis {
    /// Side by side (a vertical divider).
    Vertical,
    /// Stacked (a horizontal divider).
    Horizontal,
}

/// A serde mirror of `pane_grid::Configuration<PaneKind>`, which has no serde support
/// (ADR 0006).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LayoutNode {
    /// A split into `a` and `b`.
    Split {
        /// Axis.
        axis: SplitAxis,
        /// Share of `a`, in (0, 1).
        ratio: f32,
        /// First child.
        a: Box<LayoutNode>,
        /// Second child.
        b: Box<LayoutNode>,
    },
    /// One pane.
    Pane {
        /// Which pane.
        pane: PaneKind,
    },
}

impl LayoutNode {
    /// Mirrors the current pane grid.
    pub fn from_state(state: &pane_grid::State<PaneKind>) -> Self {
        fn walk(node: &Node, state: &pane_grid::State<PaneKind>) -> LayoutNode {
            match node {
                Node::Split {
                    axis, ratio, a, b, ..
                } => LayoutNode::Split {
                    axis: match axis {
                        Axis::Vertical => SplitAxis::Vertical,
                        Axis::Horizontal => SplitAxis::Horizontal,
                    },
                    ratio: *ratio,
                    a: Box::new(walk(a, state)),
                    b: Box::new(walk(b, state)),
                },
                Node::Pane(p) => LayoutNode::Pane {
                    pane: state.get(*p).copied().unwrap_or(PaneKind::Diagram),
                },
            }
        }
        walk(state.layout(), state)
    }

    /// The pane-grid configuration.
    pub fn to_configuration(&self) -> Configuration<PaneKind> {
        match self {
            LayoutNode::Split { axis, ratio, a, b } => Configuration::Split {
                axis: match axis {
                    SplitAxis::Vertical => Axis::Vertical,
                    SplitAxis::Horizontal => Axis::Horizontal,
                },
                ratio: *ratio,
                a: Box::new(a.to_configuration()),
                b: Box::new(b.to_configuration()),
            },
            LayoutNode::Pane { pane } => Configuration::Pane(*pane),
        }
    }

    /// Kinds in layout order.
    pub fn kinds(&self) -> Vec<PaneKind> {
        match self {
            LayoutNode::Split { a, b, .. } => {
                let mut v = a.kinds();
                v.extend(b.kinds());
                v
            }
            LayoutNode::Pane { pane } => vec![*pane],
        }
    }

    /// Every pane at most once; every ratio finite and strictly inside (0, 1).
    pub fn validate(&self) -> Result<(), String> {
        fn ratios(n: &LayoutNode) -> Result<(), String> {
            if let LayoutNode::Split { ratio, a, b, .. } = n {
                if !(ratio.is_finite() && *ratio > 0.0 && *ratio < 1.0) {
                    return Err(format!("split ratio {ratio} is outside (0, 1)"));
                }
                ratios(a)?;
                ratios(b)?;
            }
            Ok(())
        }
        let kinds = self.kinds();
        let mut seen = kinds.clone();
        seen.sort();
        seen.dedup();
        if seen.len() != kinds.len() {
            return Err("a pane appears more than once".into());
        }
        ratios(self)
    }
}

/// The saved pane layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanesState {
    /// Arrangement and ratios of the visible panes.
    pub layout: LayoutNode,
    /// Hidden panes (those not in `layout`).
    pub hidden: Vec<PaneKind>,
    /// The maximized pane, if any.
    pub maximized: Option<PaneKind>,
}

impl PanesState {
    /// Captures a live layout.
    pub fn capture(layout: &PaneLayout) -> Self {
        let state = &layout.state;
        Self {
            layout: LayoutNode::from_state(state),
            hidden: layout.hidden(),
            maximized: state.maximized().and_then(|p| state.get(p).copied()),
        }
    }

    /// Checks the layout and that `hidden`/`maximized` agree with it.
    pub fn validate(&self) -> Result<(), String> {
        self.layout.validate()?;
        let visible = self.layout.kinds();
        if self.hidden.iter().any(|k| visible.contains(k)) {
            return Err("a pane is both visible and hidden".into());
        }
        if self.maximized.is_some_and(|m| !visible.contains(&m)) {
            return Err("the maximized pane is not visible".into());
        }
        Ok(())
    }

    /// Rebuilds the live layout.
    pub fn restore(&self) -> PaneLayout {
        let mut layout = PaneLayout {
            state: pane_grid::State::with_configuration(self.layout.to_configuration()),
        };
        if let Some(m) = self.maximized
            && let Some(p) = layout.pane_of(m)
        {
            layout.maximize(p);
        }
        layout
    }
}

/// Everything in `state.json`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppState {
    /// File format version.
    pub version: u32,
    /// Window size.
    pub window: WindowSize,
    /// Pane layout; `None` means the default.
    pub panes: Option<PanesState>,
    /// Theme.
    pub theme: ThemeChoice,
    /// Last repository.
    pub last_repo: Option<PathBuf>,
    /// Recent repositories, most recent first (at most 10).
    pub recent_repos: Vec<PathBuf>,
    /// Consents (FR-SEC-03).
    pub consents: Vec<Consent>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            version: 1,
            window: WindowSize::default(),
            panes: None,
            theme: ThemeChoice::Dark,
            last_repo: None,
            recent_repos: Vec::new(),
            consents: Vec::new(),
        }
    }
}

/// `state.json` as read, before the pane layout is checked.
#[derive(Deserialize)]
struct RawState {
    #[serde(default = "one")]
    version: u32,
    #[serde(default)]
    window: Option<WindowSize>,
    #[serde(default)]
    panes: Option<Value>,
    #[serde(default)]
    theme: ThemeChoice,
    #[serde(default)]
    last_repo: Option<PathBuf>,
    #[serde(default)]
    recent_repos: Vec<PathBuf>,
    #[serde(default)]
    consents: Vec<Consent>,
}

fn one() -> u32 {
    1
}

/// The result of loading a file: the value and, when defaults were used, why.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded<T> {
    /// The loaded or default value.
    pub value: T,
    /// A user-facing message when the file was missing, unreadable or invalid.
    pub problem: Option<String>,
    /// A warning that didn't discard the file (an invalid pane layout).
    pub warning: Option<String>,
}

fn read(dir: &Path, name: &str) -> Result<Vec<u8>, String> {
    let path = dir.join(name);
    std::fs::read(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("{} not found; using defaults", path.display())
        } else {
            format!("Can't read {}: {e}; using defaults", path.display())
        }
    })
}

/// Parses `state.json` bytes.
pub fn parse_state(bytes: &[u8]) -> Result<Loaded<AppState>, String> {
    let raw: RawState = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let mut warning = None;
    let panes = match raw.panes {
        None | Some(Value::Null) => None,
        Some(v) => match serde_json::from_value::<PanesState>(v)
            .map_err(|e| e.to_string())
            .and_then(|p| p.validate().map(|()| p))
        {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!(error = %e, "invalid saved pane layout; using the default");
                warning = Some(format!(
                    "The saved pane layout is invalid ({e}); using the default layout"
                ));
                None
            }
        },
    };
    let window = raw
        .window
        .filter(|w| {
            w.width.is_finite() && w.height.is_finite() && w.width >= 400.0 && w.height >= 300.0
        })
        .unwrap_or_default();
    let mut recent_repos = raw.recent_repos;
    recent_repos.truncate(MAX_RECENTS);
    Ok(Loaded {
        value: AppState {
            version: raw.version,
            window,
            panes,
            theme: raw.theme,
            last_repo: raw.last_repo,
            recent_repos,
            consents: raw.consents,
        },
        problem: None,
        warning,
    })
}

/// Loads `state.json` from `dir`.
pub fn load_state(dir: &Path) -> Loaded<AppState> {
    let invalid = |e: String| Loaded {
        value: AppState::default(),
        problem: Some(e),
        warning: None,
    };
    match read(dir, STATE_FILE) {
        Ok(bytes) => parse_state(&bytes).unwrap_or_else(|e| {
            invalid(format!(
                "{} is invalid ({e}); using defaults",
                dir.join(STATE_FILE).display()
            ))
        }),
        Err(e) => invalid(e),
    }
}

/// Loads `settings.json` from `dir`. Missing fields take their defaults.
pub fn load_settings(dir: &Path) -> Loaded<Settings> {
    let invalid = |e: String| Loaded {
        value: Settings::default(),
        problem: Some(e),
        warning: None,
    };
    match read(dir, SETTINGS_FILE) {
        Ok(bytes) => match serde_json::from_slice::<Settings>(&bytes) {
            Ok(s) => Loaded {
                value: s,
                problem: None,
                warning: None,
            },
            Err(e) => invalid(format!(
                "{} is invalid ({e}); using defaults",
                dir.join(SETTINGS_FILE).display()
            )),
        },
        Err(e) => invalid(e),
    }
}

fn write_json(dir: &Path, name: &str, value: &impl Serialize) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Can't create {}: {e}", dir.display()))?;
    let sandbox = Sandbox::new(dir).map_err(|e| e.to_string())?;
    let rel = RelPath::parse(name).map_err(|e| e.to_string())?;
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    archalign_repo::storage::atomic_write(&sandbox, &rel, &bytes).map_err(|e| e.to_string())
}

/// Writes `state.json` atomically.
pub fn save_state(dir: &Path, state: &AppState) -> Result<(), String> {
    write_json(dir, STATE_FILE, state)
}

/// Writes `settings.json` atomically.
pub fn save_settings(dir: &Path, settings: &Settings) -> Result<(), String> {
    write_json(dir, SETTINGS_FILE, settings)
}

/// Builds the launch [`crate::app::Boot`] from the config dir (FR-STO-05). Problems become
/// launch banners: Info when a file doesn't exist yet, Warning when it can't be used.
pub fn boot_from_disk() -> crate::app::Boot {
    boot_from_dir(config_dir())
}

/// [`boot_from_disk`] for an explicit config dir (`None`: not persisted).
pub fn boot_from_dir(dir: Option<PathBuf>) -> crate::app::Boot {
    use crate::app::{Boot, Level};
    let Some(dir) = dir else {
        return Boot {
            startup_notes: vec![(
                Level::Warning,
                "No config directory is available; state and settings won't be saved".into(),
            )],
            ..Boot::default()
        };
    };
    let mut notes = Vec::new();
    let level = |name: &str| {
        if dir.join(name).exists() {
            Level::Warning
        } else {
            Level::Info
        }
    };
    let state = load_state(&dir);
    if let Some(p) = state.problem {
        notes.push((level(STATE_FILE), p));
    }
    if let Some(w) = state.warning {
        notes.push((Level::Warning, w));
    }
    let settings = load_settings(&dir);
    if let Some(p) = settings.problem {
        let missing = !dir.join(SETTINGS_FILE).exists();
        notes.push((level(SETTINGS_FILE), p));
        // Create the file on first launch so it can be edited; never overwrite a bad one.
        if missing && let Err(e) = save_settings(&dir, &settings.value) {
            notes.push((Level::Warning, format!("Can't save settings: {e}")));
        }
    }
    let s = state.value;
    Boot {
        theme: s.theme,
        last_repo: s.last_repo,
        recent_repos: s.recent_repos,
        settings: settings.value,
        consents: s.consents,
        key_env: Some(crate::keys::KeyEnv::process(Some(dir.clone()))),
        config_dir: Some(dir),
        window: Some(Size::new(s.window.width, s.window.height)),
        panes: s.panes,
        startup_notes: notes,
        ..Boot::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panes::PaneLayout;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn state_round_trips_with_a_customized_layout() {
        let mut layout = PaneLayout::default();
        let tree = layout.pane_of(PaneKind::Tree).unwrap();
        layout.close(tree);
        layout.maximize(layout.pane_of(PaneKind::Source).unwrap());
        let state = AppState {
            window: WindowSize {
                width: 1200.0,
                height: 800.0,
            },
            panes: Some(PanesState::capture(&layout)),
            theme: ThemeChoice::Light,
            last_repo: Some("/r/a".into()),
            recent_repos: vec!["/r/a".into(), "/r/b".into()],
            consents: vec![Consent {
                repo: "/r/a".into(),
                provider: "openai".into(),
                model: "gpt-6.1-sol".into(),
                granted_at: "2026-10-10T10:00:00+08:00".into(),
            }],
            ..AppState::default()
        };
        let dir = tmp();
        save_state(dir.path(), &state).unwrap();
        let loaded = load_state(dir.path());
        assert_eq!(loaded.problem, None);
        assert_eq!(loaded.warning, None);
        assert_eq!(loaded.value, state);
        // The restored grid has the same arrangement, ratios, hidden set and maximized pane.
        let restored = loaded.value.panes.unwrap().restore();
        assert_eq!(PanesState::capture(&restored), PanesState::capture(&layout));
        assert_eq!(restored.hidden(), vec![PaneKind::Tree]);
    }

    #[test]
    fn the_saved_json_has_the_documented_shape() {
        let state = AppState {
            panes: Some(PanesState::capture(&PaneLayout::default())),
            ..AppState::default()
        };
        let v: Value = serde_json::to_value(&state).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(
            v["window"],
            serde_json::json!({ "width": 1440.0, "height": 900.0 })
        );
        assert_eq!(v["theme"], "dark");
        assert_eq!(v["panes"]["hidden"], serde_json::json!([]));
        assert_eq!(v["panes"]["maximized"], Value::Null);
        assert_eq!(v["panes"]["layout"]["kind"], "split");
        assert_eq!(v["panes"]["layout"]["axis"], "vertical");
        assert_eq!(
            v["panes"]["layout"]["a"],
            serde_json::json!({ "kind": "pane", "pane": "tree" })
        );
    }

    #[test]
    fn an_invalid_layout_falls_back_to_the_default_layout_only() {
        for panes in [
            serde_json::json!({ "layout": { "kind": "split", "axis": "vertical", "ratio": 1.5,
                "a": { "kind": "pane", "pane": "tree" }, "b": { "kind": "pane", "pane": "source" } }, "hidden": [], "maximized": null }),
            serde_json::json!({ "layout": { "kind": "split", "axis": "vertical", "ratio": 0.5,
                "a": { "kind": "pane", "pane": "tree" }, "b": { "kind": "pane", "pane": "tree" } }, "hidden": [], "maximized": null }),
            serde_json::json!({ "layout": { "kind": "pane", "pane": "tree" }, "hidden": ["tree"], "maximized": null }),
            serde_json::json!({ "layout": { "kind": "pane", "pane": "tree" }, "hidden": [], "maximized": "source" }),
            serde_json::json!({ "layout": "garbage" }),
        ] {
            let bytes = serde_json::to_vec(
                &serde_json::json!({ "version": 1, "theme": "light", "panes": panes }),
            )
            .unwrap();
            let loaded = parse_state(&bytes).unwrap();
            assert_eq!(loaded.value.panes, None, "{panes}");
            assert!(
                loaded
                    .warning
                    .unwrap()
                    .starts_with("The saved pane layout is invalid")
            );
            assert_eq!(
                loaded.value.theme,
                ThemeChoice::Light,
                "the rest of the file is kept"
            );
        }
    }

    #[test]
    fn missing_unreadable_and_invalid_files_fall_back_with_a_problem() {
        let dir = tmp();
        let s = load_state(dir.path());
        assert_eq!(s.value, AppState::default());
        assert!(s.problem.unwrap().contains("not found"));
        let st = load_settings(dir.path());
        assert_eq!(st.value, Settings::default());
        assert!(st.problem.unwrap().contains("not found"));

        std::fs::write(dir.path().join(STATE_FILE), b"{ not json").unwrap();
        std::fs::write(
            dir.path().join(SETTINGS_FILE),
            br#"{ "budgets": { "max_turns": "many" } }"#,
        )
        .unwrap();
        let s = load_state(dir.path());
        assert_eq!(s.value, AppState::default());
        assert!(s.problem.unwrap().contains("state.json is invalid"));
        let st = load_settings(dir.path());
        assert_eq!(st.value, Settings::default());
        assert!(st.problem.unwrap().contains("settings.json is invalid"));

        // A folder in place of the file can't be read.
        let dir2 = tmp();
        std::fs::create_dir(dir2.path().join(STATE_FILE)).unwrap();
        assert!(
            load_state(dir2.path())
                .problem
                .unwrap()
                .starts_with("Can't read")
        );
    }

    #[test]
    fn settings_round_trip_and_missing_fields_take_defaults() {
        let dir = tmp();
        let mut s = Settings {
            model: "anthropic/claude-x".into(),
            reasoning_effort: Some("high".into()),
            show_hidden_files: true,
            ..Settings::default()
        };
        s.budgets.max_turns = 12;
        save_settings(dir.path(), &s).unwrap();
        let loaded = load_settings(dir.path());
        assert_eq!(loaded.problem, None);
        assert_eq!(loaded.value, s);

        std::fs::write(
            dir.path().join(SETTINGS_FILE),
            br#"{ "version": 1, "show_hidden_files": true }"#,
        )
        .unwrap();
        let loaded = load_settings(dir.path()).value;
        assert!(loaded.show_hidden_files);
        assert_eq!(loaded.model, crate::settings::DEFAULT_MODEL);
    }

    #[test]
    fn recents_are_capped_and_tiny_windows_are_ignored() {
        let recent: Vec<String> = (0..15).map(|i| format!("/r/{i}")).collect();
        let bytes = serde_json::to_vec(&serde_json::json!({ "recent_repos": recent, "window": { "width": 10.0, "height": 10.0 } })).unwrap();
        let s = parse_state(&bytes).unwrap().value;
        assert_eq!(s.recent_repos.len(), MAX_RECENTS);
        assert_eq!(s.window, WindowSize::default());
    }
}
