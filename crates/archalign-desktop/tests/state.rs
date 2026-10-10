//! M6: History, app state, settings (FR-STO-04..06, FR-PANE-08, FR-DIA-10, FR-GEN-13).

mod common;

use std::path::Path;
use std::time::Duration;

use archalign_desktop::app::{Boot, Message};
use archalign_desktop::ids;
use archalign_desktop::palette::ThemeChoice;
use archalign_desktop::panes::PaneKind;
use archalign_desktop::persist::{self, PanesState};
use archalign_desktop::settings::Consent;
use common::Harness;
use common::fakes::{Scripted, Step, key_env, submit, topology};
use common::fixtures::{TempRepo, sample_doc, sample_repo};

fn boot_in(dir: &Path) -> Boot {
    let mut b = persist::boot_from_dir(Some(dir.to_path_buf()));
    b.key_env = None; // the harness substitutes an empty environment
    b
}

fn saved_state(dir: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(dir.join(persist::STATE_FILE)).unwrap()).unwrap()
}

fn saves(h: &Harness) -> usize {
    h.log.iter().filter(|l| l.starts_with("Saved(")).count()
}

// ---------- FR-STO-04 ----------

#[test]
fn history_lists_newest_first_with_title_time_and_status_and_opens_entries() {
    let repo = sample_repo();
    repo.diagram("architecture-old-20240101-090000", &sample_doc("Old one"));
    repo.diagram("architecture-new-20250202-100000", &sample_doc("New one"));
    let mut bad = sample_doc("Broken");
    bad["components"] = serde_json::json!("nope");
    repo.diagram("architecture-bad-20250101-080000", &bad);
    let mut seq = sample_doc("Sequence");
    seq["diagram_type"] = serde_json::json!("sequence");
    repo.diagram("sequence-x-20230101-000000", &seq);
    repo.mkdir(".archify/architecture-empty-20260101-000000");
    let mut h = Harness::new(Boot::default());
    h.send(Message::OpenRepo(repo.root.clone()));
    h.click(ids::TB_HISTORY);
    let rows = h.ids_with_prefix("history:");
    assert_eq!(
        rows.len(),
        4,
        "the folder without candidate.json is hidden: {rows:?}"
    );
    assert!(h.has_text("● New one  ·  2025-02-02 10:00:00  ·  valid"));
    assert!(h.has_text("  Broken  ·  2025-01-01 08:00:00  ·  invalid"));
    assert!(h.has_text("  Old one  ·  2024-01-01 09:00:00  ·  valid"));
    assert!(h.has_text("  Sequence  ·  2023-01-01 00:00:00  ·  unsupported type (sequence)"));
    let b = |h: &mut Harness, i: usize| h.bounds(ids::history_entry(i)).unwrap().y;
    assert!(
        b(&mut h, 0) < b(&mut h, 1) && b(&mut h, 1) < b(&mut h, 2) && b(&mut h, 2) < b(&mut h, 3)
    );
    h.screenshot(&common::smoke_dir("m6").join("history.png"));
    h.click(ids::history_entry(2));
    assert_eq!(h.app.diagram.title().as_deref(), Some("Old one"));
    assert!(!h.has(ids::history_entry(0)), "the menu closes");
}

// ---------- FR-STO-05 / FR-PANE-08 / FR-DIA-10 ----------

#[test]
fn state_survives_a_restart_layout_theme_window_recents_and_consents() {
    let cfg = tempfile::tempdir().unwrap();
    let repo = sample_repo();
    {
        let mut h = Harness::new(boot_in(cfg.path()));
        // First launch: both files are missing → info banners; settings.json is created.
        assert!(
            h.app
                .banners
                .iter()
                .any(|b| b.text.contains("state.json not found"))
        );
        assert!(cfg.path().join(persist::SETTINGS_FILE).exists());
        h.send(Message::OpenRepo(repo.root.clone()));
        h.click(ids::TB_THEME);
        let source = h.app.panes.pane_of(PaneKind::Source).unwrap();
        h.send(Message::PaneClose(source));
        h.send(Message::WindowResized(iced::Size::new(1300.0, 820.0)));
        h.app.consents.push(Consent {
            repo: repo.root.clone(),
            provider: "openai".into(),
            model: "gpt-6.1-sol".into(),
            granted_at: "2026-10-10T10:00:00+08:00".into(),
        });
        h.send(Message::DismissBanner(0));
        assert!(h.wait_until(Duration::from_secs(5), |a| {
            persist::load_state(&a.config_dir.clone().unwrap()).value == a.app_state()
        }));
        let s = saved_state(cfg.path());
        assert_eq!(s["theme"], "light");
        assert_eq!(s["panes"]["hidden"], serde_json::json!(["source"]));
    }
    let before = persist::load_state(cfg.path()).value;
    let h = Harness::new(boot_in(cfg.path()));
    assert!(h.app.banners.is_empty(), "{:?}", h.app.banners);
    assert_eq!(h.app.theme, ThemeChoice::Light);
    assert_eq!(h.app.window, iced::Size::new(1300.0, 820.0));
    assert!(!h.app.panes.is_visible(PaneKind::Source));
    assert_eq!(
        PanesState::capture(&h.app.panes),
        before.panes.clone().unwrap()
    );
    assert_eq!(h.app.consents.len(), 1);
    assert_eq!(
        h.app.repo.as_ref().map(|r| r.root.clone()),
        Some(repo.root.clone()),
        "last repo reopened"
    );
    assert_eq!(h.app.app_state(), before);
}

#[test]
fn state_saves_are_rate_limited_to_once_per_second() {
    let cfg = tempfile::tempdir().unwrap();
    persist::save_state(cfg.path(), &persist::AppState::default()).unwrap();
    persist::save_settings(cfg.path(), &Default::default()).unwrap();
    let mut h = Harness::new(boot_in(cfg.path()));
    assert!(h.app.banners.is_empty());
    h.patience = Duration::from_millis(300);
    let start = saves(&h);
    let t = std::time::Instant::now();
    for _ in 0..4 {
        h.click(ids::TB_THEME);
    }
    assert_eq!(saves(&h) - start, 1, "one immediate save; the rest wait");
    assert_eq!(
        persist::load_state(cfg.path()).value.theme,
        ThemeChoice::Light,
        "the first change"
    );
    assert!(h.wait_until(Duration::from_secs(3), |a| {
        persist::load_state(&a.config_dir.clone().unwrap())
            .value
            .theme
            == ThemeChoice::Dark
    }));
    assert!(
        t.elapsed() >= persist::SAVE_INTERVAL,
        "the second save waited for the interval"
    );
    assert_eq!(
        saves(&h) - start,
        2,
        "then one delayed save with the final state"
    );
}

#[test]
fn an_invalid_state_file_falls_back_with_a_banner() {
    let cfg = tempfile::tempdir().unwrap();
    std::fs::write(cfg.path().join(persist::STATE_FILE), b"{ broken").unwrap();
    std::fs::write(cfg.path().join(persist::SETTINGS_FILE), br#"{"version":1}"#).unwrap();
    let mut h = Harness::new(boot_in(cfg.path()));
    let b = h
        .app
        .banners
        .iter()
        .find(|b| b.text.contains("state.json is invalid"))
        .expect("banner");
    assert_eq!(b.level, archalign_desktop::app::Level::Warning);
    assert_eq!(h.app.theme, ThemeChoice::Dark);
    assert!(h.has_text(&b.text.clone()));
}

// ---------- FR-STO-06 / FR-GEN-13 ----------

const KEY: &str = "sk-settings-secret-91";

#[test]
fn settings_edits_persist_never_show_the_key_and_apply_to_the_next_run() {
    let cfg = tempfile::tempdir().unwrap();
    let repo = sample_repo();
    let fake = Scripted::new(vec![Step::Reply(submit(topology()))]);
    let mut boot = boot_in(cfg.path());
    boot.key_env = Some(key_env("OPENAI_API_KEY", KEY));
    boot.llm = Some(fake.factory());
    boot.consents = vec![Consent {
        repo: repo.root.clone(),
        provider: "openai".into(),
        model: "gpt-6-luna".into(),
        granted_at: "2026-10-10T10:00:00+08:00".into(),
    }];
    let mut h = Harness::new(boot);
    h.send(Message::OpenRepo(repo.root.clone()));
    h.cmd(",");
    assert!(h.has(ids::SETTINGS_MODAL));
    assert!(h.has_text("API key: OPENAI_API_KEY — environment"));
    assert!(!h.has_text(KEY));
    assert!(h.ids_with_prefix("settings-revoke:").len() == 1);
    h.screenshot(&common::smoke_dir("m6").join("settings.png"));
    // Invalid input is reported and nothing is saved.
    h.click(ids::settings_budget(0));
    h.cmd("a");
    h.type_text("zero");
    h.click(ids::SETTINGS_SAVE);
    assert!(h.has_text("Max turns must be a whole number"));
    // Valid edits.
    h.click(ids::settings_budget(0));
    h.cmd("a");
    h.type_text("7");
    h.click(ids::SETTINGS_EFFORT);
    h.type_text("high");
    h.click(ids::SETTINGS_MODEL);
    h.cmd("a");
    h.type_text("openai/gpt-6-luna");
    h.click(ids::SETTINGS_SAVE);
    assert!(!h.has(ids::SETTINGS_MODAL));
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(cfg.path().join(persist::SETTINGS_FILE)).unwrap())
            .unwrap();
    assert_eq!(saved["model"], "openai/gpt-6-luna");
    assert_eq!(saved["reasoning_effort"], "high");
    assert_eq!(saved["budgets"]["max_turns"], 7);
    // FR-GEN-13: the next run uses them.
    h.click(ids::TB_ARCHIFY);
    assert!(h.wait_until(Duration::from_secs(10), |a| !a.run.is_running()));
    let req = fake.requests.lock().unwrap()[0].clone();
    assert_eq!(req.reasoning_effort.as_deref(), Some("high"));
    assert!(
        h.app.run_log.iter().any(|l| l == "Turn 1/7"),
        "{:?}",
        h.app.run_log
    );
    assert!(h.app.run_log[0].ends_with("with openai/gpt-6-luna"));
    // Revoke from Settings.
    h.cmd(",");
    h.click(ids::settings_revoke(0));
    assert!(h.app.consents.is_empty());
    assert!(h.has_text("None yet."));
    h.click(ids::SETTINGS_CANCEL);
    assert!(
        h.wait_until(Duration::from_secs(3), |_| saved_state_consents(cfg.path())
            == 0)
    );
}

fn saved_state_consents(dir: &Path) -> usize {
    std::fs::read(dir.join(persist::STATE_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v["consents"].as_array().map(Vec::len))
        .unwrap_or(usize::MAX)
}

#[test]
fn show_hidden_files_from_settings_relists_the_tree() {
    let repo = TempRepo::new();
    repo.write(".hidden.txt", "x").write("visible.txt", "y");
    let mut h = Harness::new(Boot::default());
    h.send(Message::OpenRepo(repo.root.clone()));
    assert!(!h.has_text(".hidden.txt"));
    h.click(ids::TB_SETTINGS);
    h.click(ids::SETTINGS_HIDDEN);
    h.click(ids::SETTINGS_SAVE);
    assert!(h.app.settings.show_hidden_files);
    assert!(h.has_text(".hidden.txt"));
}
