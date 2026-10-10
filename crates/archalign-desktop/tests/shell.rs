//! M3 app shell through the UI harness: panes, toolbar, status bar, welcome and import.

mod common;

use archalign_desktop::app::{Action, Boot, Message, RunState};
use archalign_desktop::diagram::DiagramContent;
use archalign_desktop::ids;
use archalign_desktop::panes::PaneKind;
use common::Harness;
use common::fixtures::{TempRepo, sample_doc, sample_repo};
use iced::keyboard::Modifiers;

fn opened(repo: &TempRepo) -> Harness {
    let mut h = Harness::new(Boot::default());
    h.send(Message::OpenRepo(repo.root.clone()));
    assert!(h.app.repo.is_some(), "{:?}", h.app.banners);
    h
}

/// A UI interaction.
type Step = Box<dyn Fn(&mut Harness)>;

fn open_panels(h: &mut Harness) {
    if !h.has(ids::MENU_RESET) {
        h.click(ids::TB_PANELS);
    }
}

fn actions(h: &Harness) -> Vec<String> {
    h.log
        .iter()
        .filter(|l| l.starts_with("Action("))
        .cloned()
        .collect()
}

// ---------- FR-PANE-01 ----------

#[test]
fn default_layout_is_tree_diagram_source_at_20_50_30() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    let t = h.bounds(ids::pane_title(PaneKind::Tree)).unwrap();
    let d = h.bounds(ids::pane_title(PaneKind::Diagram)).unwrap();
    let s = h.bounds(ids::pane_title(PaneKind::Source)).unwrap();
    assert!(t.x < d.x && d.x < s.x, "left to right: {t:?} {d:?} {s:?}");
    let regions = h.app.panes.regions(h.app.grid_size());
    let width = |k: PaneKind| regions.iter().find(|(kk, _)| *kk == k).unwrap().1.width;
    let total = h.app.grid_size().width;
    for (k, want) in [
        (PaneKind::Tree, 0.20),
        (PaneKind::Diagram, 0.50),
        (PaneKind::Source, 0.30),
    ] {
        let got = width(k) / total;
        assert!((got - want).abs() < 0.01, "{k:?}: {got}");
    }
    h.screenshot(&common::smoke_dir("m3").join("default-layout.png"));
}

// ---------- FR-PANE-02 ----------

#[test]
fn title_bars_show_name_and_context_and_buttons_emit_messages() {
    let repo = sample_repo();
    repo.diagram(
        "architecture-a-20261010-101010",
        &sample_doc("Payments overview"),
    );
    let mut h = opened(&repo);
    assert!(h.has_text("Tree") && h.has_text("Diagram") && h.has_text("Source"));
    assert!(
        h.has_text("Payments overview"),
        "diagram title as context label"
    );
    h.send(Message::OpenFile("src/api.rs".parse().unwrap(), None));
    assert!(
        h.find("src/api.rs").is_some(),
        "source path as context label"
    );
    h.click(ids::pane_maximize(PaneKind::Diagram));
    assert!(h.log.iter().any(|l| l.starts_with("PaneMaximize")));
    h.click(ids::pane_maximize(PaneKind::Diagram)); // now "Restore"
    assert!(h.log.iter().any(|l| l == "PaneRestore"));
    h.click(ids::pane_close(PaneKind::Source));
    assert!(h.log.iter().any(|l| l.starts_with("PaneClose")));
    assert!(!h.app.panes.is_visible(PaneKind::Source));
}

// ---------- FR-PANE-05 ----------

#[test]
fn maximize_fills_the_grid_and_restore_returns() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    let before = h.app.panes.regions(h.app.grid_size());
    h.click(ids::pane_maximize(PaneKind::Source));
    let max = h.app.panes.regions(h.app.grid_size());
    assert_eq!(max.len(), 1);
    assert_eq!(max[0].0, PaneKind::Source);
    assert_eq!(max[0].1.width, h.app.grid_size().width);
    assert!(
        !h.has(ids::pane_title(PaneKind::Tree)),
        "only the maximized pane renders"
    );
    h.click(ids::pane_maximize(PaneKind::Source));
    assert_eq!(h.app.panes.regions(h.app.grid_size()), before);
}

// ---------- FR-PANE-06 ----------

#[test]
fn close_and_reopen_through_the_panels_menu() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    h.click(ids::pane_close(PaneKind::Tree));
    assert!(!h.app.panes.is_visible(PaneKind::Tree));
    h.click(ids::TB_PANELS);
    assert!(h.has_text("☐ Tree") && h.has_text("☑ Diagram") && h.has_text("☑ Source"));
    h.click(ids::menu_pane(PaneKind::Tree));
    assert!(h.app.panes.is_visible(PaneKind::Tree));
    // Tree comes back on the left of the pane it split.
    let t = h.bounds(ids::pane_title(PaneKind::Tree)).unwrap();
    let d = h.bounds(ids::pane_title(PaneKind::Diagram)).unwrap();
    assert!(t.x < d.x);
    // Diagram and Source are inserted on the right.
    h.click(ids::pane_close(PaneKind::Source));
    h.click(ids::menu_pane(PaneKind::Source));
    let s = h.bounds(ids::pane_title(PaneKind::Source)).unwrap();
    let largest_left = h.bounds(ids::pane_title(PaneKind::Diagram)).unwrap();
    assert!(
        s.x > largest_left.x,
        "Source right of the split (largest) pane"
    );
}

#[test]
fn the_last_visible_pane_cannot_be_closed() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    h.click(ids::pane_close(PaneKind::Tree));
    h.click(ids::pane_close(PaneKind::Source));
    assert_eq!(h.app.panes.visible_count(), 1);
    let before = h.log.len();
    h.click(ids::pane_close(PaneKind::Diagram));
    assert!(
        h.log[before..].iter().all(|l| !l.starts_with("PaneClose")),
        "disabled button"
    );
    assert!(h.app.panes.is_visible(PaneKind::Diagram));
    // Each pane exists at most once: toggling a visible pane hides it instead of duplicating.
    h.send(Message::Action(Action::TogglePane(PaneKind::Tree)));
    h.send(Message::Action(Action::TogglePane(PaneKind::Tree)));
    h.send(Message::Action(Action::TogglePane(PaneKind::Source)));
    let kinds: Vec<_> = h.app.panes.state.iter().map(|(_, k)| *k).collect();
    for k in PaneKind::ALL {
        assert!(kinds.iter().filter(|x| **x == k).count() <= 1);
    }
}

// ---------- FR-PANE-07 ----------

#[test]
fn reset_restores_the_default_arrangement_and_clears_maximize() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    let default = h.app.panes.regions(h.app.grid_size());
    h.click(ids::pane_close(PaneKind::Tree));
    h.click(ids::pane_maximize(PaneKind::Source));
    h.click(ids::TB_PANELS);
    h.click(ids::MENU_RESET);
    assert_eq!(h.app.panes.state.maximized(), None);
    assert_eq!(h.app.panes.regions(h.app.grid_size()), default);
    // ⌘⇧0 does the same.
    h.click(ids::pane_close(PaneKind::Diagram));
    h.cmd_shift("0");
    assert_eq!(h.app.panes.regions(h.app.grid_size()), default);
}

// ---------- FR-PANE-09 ----------

#[test]
fn archify_is_disabled_without_a_repo() {
    let mut h = Harness::new(Boot::default());
    h.click(ids::TB_ARCHIFY);
    assert!(actions(&h).is_empty(), "{:?}", h.log);
    h.cmd("g");
    assert!(
        !h.log
            .iter()
            .any(|l| l.contains("Archify") && l.starts_with("Action"))
    );
}

#[test]
fn open_repo_is_disabled_during_a_run() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    h.app.run = RunState::Running {
        turn: 1,
        max_turns: 40,
        input_tokens: 10,
        elapsed_s: 3,
    };
    let before = h.log.len();
    h.click(ids::TB_OPEN);
    assert!(h.log[before..].is_empty(), "{:?}", &h.log[before..]);
    assert!(h.has_text("Cancel"), "Archify becomes Cancel");
}

#[test]
fn each_shortcut_triggers_the_same_action_as_its_button() {
    let repo = sample_repo();
    repo.diagram("architecture-a-20261010-101010", &sample_doc("S"));
    let mut h = opened(&repo);
    let pairs: Vec<(Step, Step, &str)> = vec![
        (
            Box::new(|h| h.click(ids::TB_THEME)),
            Box::new(|h| h.cmd_shift("d")),
            "ToggleTheme",
        ),
        (
            Box::new(|h| h.click(ids::TB_HISTORY)),
            Box::new(|h| h.cmd("y")),
            "History",
        ),
        (
            Box::new(|h| h.click(ids::TB_SETTINGS)),
            Box::new(|h| h.cmd(",")),
            "Settings",
        ),
        (
            Box::new(|h| h.click(ids::TB_ARCHIFY)),
            Box::new(|h| h.cmd("g")),
            "Archify",
        ),
        (
            Box::new(|h| {
                open_panels(h);
                h.click(ids::menu_pane(PaneKind::Tree))
            }),
            Box::new(|h| h.cmd("1")),
            "TogglePane(Tree)",
        ),
        (
            Box::new(|h| {
                open_panels(h);
                h.click(ids::MENU_RESET)
            }),
            Box::new(|h| h.cmd_shift("0")),
            "ResetLayout",
        ),
    ];
    for (button, shortcut, name) in pairs {
        let n = h.log.len();
        button(&mut h);
        let by_button: Vec<_> = h.log[n..]
            .iter()
            .filter(|l| l.starts_with("Action("))
            .cloned()
            .collect();
        let n = h.log.len();
        shortcut(&mut h);
        let by_key: Vec<_> = h.log[n..]
            .iter()
            .filter(|l| l.starts_with("Action("))
            .cloned()
            .collect();
        assert_eq!(by_button, vec![format!("Action({name})")], "button");
        assert_eq!(by_key, by_button, "shortcut for {name}");
        // Close the Settings modal so the next button is reachable.
        h.send(Message::Settings(
            archalign_desktop::app::SettingsMsg::Cancel,
        ));
    }
    // ⌘2 / ⌘3, ⌘R, ⌘⇧L, ⌘. map to their actions too.
    for (c, shift, name) in [
        ("2", false, "TogglePane(Diagram)"),
        ("3", false, "TogglePane(Source)"),
        ("r", false, "Refresh"),
        ("l", true, "Relayout"),
    ] {
        let n = h.log.len();
        if shift {
            h.cmd_shift(c);
        } else {
            h.cmd(c);
        }
        assert!(
            h.log[n..].contains(&format!("Action({name})")),
            "{c}: {:?}",
            &h.log[n..]
        );
    }
}

#[test]
fn global_shortcuts_do_not_fire_inside_a_focused_text_field() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    h.click(ids::FOCUS_INPUT);
    h.type_text("payments");
    assert_eq!(h.app.focus, "payments");
    let n = h.log.len();
    h.cmd_shift("d");
    assert!(
        !h.log[n..].iter().any(|l| l.starts_with("Action(")),
        "{:?}",
        &h.log[n..]
    );
    // Click elsewhere (the tree) to leave the field; the shortcut works again.
    h.click(ids::tree_row(&"README.md".parse().unwrap()));
    let n = h.log.len();
    h.cmd_shift("d");
    assert!(h.log[n..].contains(&"Action(ToggleTheme)".to_string()));
}

// ---------- FR-PANE-10 ----------

#[test]
fn status_bar_shows_repo_git_run_and_watcher() {
    let repo = sample_repo();
    repo.git_commit_all();
    repo.write("src/api.rs", "changed\n");
    let mut h = opened(&repo);
    assert!(h.has_text(&repo.root.display().to_string()));
    let head = match &h.app.repo.as_ref().unwrap().git {
        archalign_repo::git::GitStatus::Repo(info) => info.short_head().to_string(),
        other => panic!("{other:?}"),
    };
    assert!(h.has_text(&format!("git {head} · 1 dirty")));
    assert!(h.has_text("Idle"));
    assert!(h.has_text("Manual refresh only"));
    // A run's progress replaces "Idle" on the next frame.
    h.app.run = RunState::Running {
        turn: 3,
        max_turns: 40,
        input_tokens: 12_345,
        elapsed_s: 75,
    };
    assert!(h.has_text("Turn 3/40 · 12345 input tokens · 1:15"));
    h.screenshot(&common::smoke_dir("m3").join("status-bar.png"));
}

// ---------- FR-PANE-11 ----------

#[test]
fn errors_show_as_dismissible_banners_and_in_the_status_bar() {
    let repo = sample_repo();
    repo.write(
        ".archify/architecture-bad-20261010-101010/candidate.json",
        "{ not json",
    );
    let mut h = opened(&repo);
    assert!(
        matches!(h.app.diagram.content, DiagramContent::Empty),
        "invalid diagrams don't auto-open"
    );
    h.send(Message::DiagramOpen(
        ".archify/architecture-bad-20261010-101010/candidate.json"
            .parse()
            .unwrap(),
    ));
    let banner = h.app.banners.last().cloned().expect("banner");
    assert_eq!(banner.pane, Some(PaneKind::Diagram));
    assert!(h.has_text(&banner.text));
    assert_eq!(h.app.status_message.as_deref(), Some(banner.text.as_str()));
    h.click(ids::banner_dismiss(banner.id));
    assert!(h.app.banners.iter().all(|b| b.id != banner.id));
}

// ---------- FR-IMP-01 ----------

#[test]
fn welcome_lists_recents_and_marks_missing_ones() {
    let a = TempRepo::new();
    let missing = a.root.join("gone");
    let mut h = Harness::new(Boot {
        recent_repos: vec![a.root.clone(), missing.clone()],
        ..Boot::default()
    });
    assert!(h.has(ids::WELCOME_OPEN));
    assert!(h.has_text(&a.root.display().to_string()));
    assert!(h.has_text("Missing"));
    // The missing entry is disabled.
    let n = h.log.len();
    h.click(ids::recent(1));
    assert!(h.log[n..].is_empty());
    // And can be removed.
    h.click(ids::recent_remove(1));
    assert_eq!(h.app.recents.len(), 1);
    assert!(!h.has_text("Missing"));
    // The other opens.
    h.click(ids::recent(0));
    assert_eq!(h.app.repo.as_ref().unwrap().root, a.root);
    h.screenshot(&common::smoke_dir("m3").join("after-recent.png"));
}

#[test]
fn welcome_keeps_at_most_ten_recents_most_recent_first() {
    let repos: Vec<TempRepo> = (0..12).map(|_| TempRepo::new()).collect();
    let mut h = Harness::new(Boot::default());
    for r in &repos {
        h.send(Message::OpenRepo(r.root.clone()));
    }
    assert_eq!(h.app.recents.len(), 10);
    assert_eq!(h.app.recents[0].path, repos[11].root);
    assert_eq!(h.app.recents[9].path, repos[2].root);
}

// ---------- FR-IMP-02 ----------

#[test]
fn opening_a_repo_sets_the_root_and_resets_state() {
    let first = sample_repo();
    first.diagram("architecture-a-20261010-101010", &sample_doc("First"));
    let second = TempRepo::new();
    second.write("only.txt", "x\n");
    let mut h = opened(&first);
    h.send(Message::TreeActivate("src".parse().unwrap()));
    h.send(Message::OpenFile("README.md".parse().unwrap(), None));
    assert!(h.app.diagram.loaded().is_some());
    h.send(Message::OpenRepo(second.root.clone()));
    let repo = h.app.repo.as_ref().unwrap();
    assert_eq!(repo.root, second.root);
    assert!(h.app.source.rel().is_none(), "source reset");
    assert!(
        matches!(h.app.diagram.content, DiagramContent::Empty),
        "diagram reset"
    );
    assert!(
        h.app.tree.selected.is_none() && h.app.tree.expanded.is_empty(),
        "tree reset"
    );
    assert_eq!(h.app.tree.rows().len(), 1);
    assert_eq!(h.app.recents[0].path, second.root);
    assert_eq!(h.app.recents[1].path, first.root);
}

// ---------- FR-IMP-03 ----------

#[test]
fn the_newer_valid_diagram_opens_on_import() {
    let repo = sample_repo();
    repo.diagram("architecture-old-20261001-090000", &sample_doc("Older"));
    repo.diagram("architecture-new-20261009-090000", &sample_doc("Newer"));
    // An even newer but invalid one is skipped.
    repo.write(
        ".archify/architecture-broken-20261010-090000/candidate.json",
        "[]",
    );
    let mut h = opened(&repo);
    assert_eq!(h.app.diagram.title().as_deref(), Some("Newer"));
    assert!(h.has_text("Newer"));
    assert!(matches!(
        h.app.source.content,
        archalign_desktop::source::SourceContent::Empty
    ));
    assert!(h.app.tree.rows().iter().any(|r| r.name == "src"));
}

#[test]
fn without_diagrams_the_empty_state_shows_focus_and_archify() {
    let repo = sample_repo();
    let mut h = opened(&repo);
    assert!(matches!(h.app.diagram.content, DiagramContent::Empty));
    assert!(h.has(ids::FOCUS_INPUT_EMPTY));
    assert!(h.has_text("No diagram yet"));
    h.screenshot(&common::smoke_dir("m3").join("empty-state.png"));
}

// ---------- FR-IMP-05 ----------

#[test]
fn last_repo_reopens_at_launch_if_it_exists() {
    let repo = sample_repo();
    let h = Harness::new(Boot {
        last_repo: Some(repo.root.clone()),
        ..Boot::default()
    });
    assert_eq!(
        h.app.repo.as_ref().map(|r| r.root.clone()),
        Some(repo.root.clone())
    );
    let gone = repo.root.join("nope");
    let mut h = Harness::new(Boot {
        last_repo: Some(gone),
        ..Boot::default()
    });
    assert!(h.app.repo.is_none());
    assert!(h.has(ids::WELCOME_OPEN));
}

// ---------- FR-IMP-06 ----------

#[test]
fn another_repo_cannot_be_opened_while_a_run_is_active() {
    let a = sample_repo();
    let b = TempRepo::new();
    let mut h = opened(&a);
    h.app.run = RunState::Running {
        turn: 1,
        max_turns: 40,
        input_tokens: 0,
        elapsed_s: 0,
    };
    h.send(Message::OpenRepo(b.root.clone()));
    h.cmd("o");
    assert_eq!(h.app.repo.as_ref().unwrap().root, a.root);
    let _ = Modifiers::default();
}

#[test]
fn missing_git_unreadable_files_and_bad_utf8_give_banners_not_panics() {
    let repo = sample_repo();
    repo.write("bad.txt", [0xffu8, 0xfe, b'\n']);
    repo.write("locked.txt", "secret\n");
    let locked = repo.path("locked.txt");
    std::fs::set_permissions(&locked, std::os::unix::fs::PermissionsExt::from_mode(0o000)).unwrap();
    let mut h = Harness::new(Boot {
        git_program: Some("/nonexistent/git-binary".into()),
        ..Boot::default()
    });
    h.send(Message::OpenRepo(repo.root.clone()));
    assert!(
        h.has_text("git was not found on PATH; repository metadata and modified badges are off")
    );
    assert!(h.has_text("git not found"), "status bar");
    h.send(Message::OpenFile("bad.txt".parse().unwrap(), None));
    assert!(
        h.app
            .banners
            .iter()
            .any(|b| b.text.contains("isn't valid UTF-8"))
    );
    h.send(Message::OpenFile("locked.txt".parse().unwrap(), None));
    let b = h.app.banners.last().unwrap().clone();
    assert!(b.text.starts_with("Can't read locked.txt"), "{}", b.text);
    assert_eq!(b.pane, Some(PaneKind::Source));
    std::fs::set_permissions(&locked, std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();
}

// ---------- FR-VAL-06 / FR-VAL-07 in the app ----------

#[test]
fn opened_diagrams_carry_citation_and_dirty_warnings() {
    let repo = sample_repo();
    repo.git_commit_all();
    let mut doc = sample_doc("Cites");
    doc["components"][1]["sources"][0]["line"] = serde_json::json!(400); // store.rs has 40 lines
    repo.diagram("architecture-c-20261010-101010", &doc);
    repo.write("src/api.rs", "modified\n".repeat(30));
    let h = opened(&repo);
    let d = h.app.diagram.loaded().expect("opens despite warnings");
    let codes: Vec<_> = d
        .warnings
        .iter()
        .map(|w| (w.code, w.pointer.clone()))
        .collect();
    assert!(
        codes.contains(&(
            archalign_model::IssueCode::BrokenCitation,
            "/components/1/sources/0/line".into()
        )),
        "{codes:?}"
    );
    assert!(
        codes
            .iter()
            .any(|(c, p)| *c == archalign_model::IssueCode::ModifiedCitation
                && p.starts_with("/components/0"))
    );
    assert!(
        d.warnings
            .iter()
            .all(|w| w.severity == archalign_model::Severity::Warning)
    );
}
