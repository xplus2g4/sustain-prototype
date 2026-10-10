//! M3 tree and source viewer through the UI harness (FR-TREE-01..05, 08; FR-SRC-01..08).

mod common;

use archalign_desktop::app::{App, Boot, Message};
use archalign_desktop::ids;
use archalign_desktop::source::{self, Cite, SourceContent};
use archalign_desktop::virtual_list::OVERSCAN;
use archalign_repo::RelPath;
use common::Harness;
use common::fixtures::{TempRepo, lines, sample_repo};

fn rel(s: &str) -> RelPath {
    s.parse().unwrap()
}

fn opened(repo: &TempRepo) -> Harness {
    let mut h = Harness::new(Boot::default());
    h.send(Message::OpenRepo(repo.root.clone()));
    assert!(h.app.repo.is_some());
    h
}

fn names(h: &Harness) -> Vec<String> {
    h.app.tree.rows().into_iter().map(|r| r.name).collect()
}

// ---------- FR-TREE-01 ----------

#[test]
fn folders_come_first_then_files_case_insensitively() {
    let r = TempRepo::new();
    r.write("b.txt", "").write("A.txt", "").write("c.txt", "");
    r.mkdir("zeta").mkdir("Alpha").write("beta/x", "");
    let h = opened(&r);
    assert_eq!(
        names(&h),
        ["Alpha", "beta", "zeta", "A.txt", "b.txt", "c.txt"]
    );
}

#[test]
fn a_folder_is_listed_on_first_expansion_off_the_ui_thread() {
    let r = sample_repo();
    let h = opened(&r);
    let mut app: App = h.app;
    assert!(
        !app.tree.children.contains_key(&rel("src")),
        "not listed before expansion"
    );
    let task = app.update(Message::TreeToggle(rel("src")));
    // The listing runs as a task; nothing is listed synchronously in `update`.
    assert!(!app.tree.children.contains_key(&rel("src")));
    assert!(app.tree.loading.contains(&rel("src")));
    assert!(iced_test::runtime::task::into_stream(task).is_some());
}

// ---------- FR-TREE-02 ----------

#[test]
fn ignored_files_git_and_dotfiles_are_hidden_unless_enabled() {
    let r = TempRepo::new();
    r.write(".gitignore", "target/\n*.log\n")
        .write("target/out", "")
        .write("debug.log", "")
        .write("main.rs", "")
        .write(".env.example", "")
        .write("sub/.gitignore", "secret.txt\n")
        .write("sub/secret.txt", "")
        .write("sub/ok.txt", "");
    r.git_commit_all();
    let mut h = opened(&r);
    assert_eq!(names(&h), ["sub", "main.rs"]);
    h.send(Message::TreeToggle(rel("sub")));
    assert_eq!(names(&h), ["sub", "ok.txt", "main.rs"]);
    let mut h = Harness::new(Boot {
        settings: archalign_desktop::settings::Settings {
            show_hidden_files: true,
            ..Default::default()
        },
        ..Boot::default()
    });
    h.send(Message::OpenRepo(r.root.clone()));
    let n = names(&h);
    assert!(n.contains(&".gitignore".to_string()) && n.contains(&".env.example".to_string()));
    assert!(!n.contains(&".git".to_string()), "{n:?}");
}

// ---------- FR-TREE-03 ----------

#[test]
fn only_visible_rows_plus_overscan_are_built() {
    let r = TempRepo::new();
    for i in 0..5000 {
        r.write(&format!("big/f{i:05}.txt"), "");
    }
    let mut h = opened(&r);
    h.send(Message::TreeToggle(rel("big")));
    assert_eq!(h.app.tree.rows().len(), 5001);
    let built = h.ids_with_prefix("tree-row:");
    let pane_h = h
        .app
        .pane_body_height(archalign_desktop::panes::PaneKind::Tree);
    let visible = (pane_h / archalign_desktop::tree::ROW_H).ceil() as usize + 1;
    assert!(
        built.len() <= visible + 2 * OVERSCAN,
        "{} built, {visible} visible",
        built.len()
    );
    // Scrolling far down builds a window around the new position only.
    let at = h.bounds(ids::tree_row(&rel("big"))).unwrap().center();
    h.move_to(at);
    h.wheel(-40_000.0);
    let built = h.ids_with_prefix("tree-row:");
    assert!(
        built.len() <= visible + 2 * OVERSCAN,
        "{} built, {visible} visible, {:?}",
        built.len(),
        h.app.tree.scroll
    );
    assert!(
        built
            .iter()
            .any(|(id, _)| id.starts_with("tree-row:big/f018")),
        "window moved"
    );
}

// ---------- FR-TREE-04 ----------

#[test]
fn clicking_a_file_opens_it_and_selects_it_and_folders_toggle() {
    let r = sample_repo();
    let mut h = opened(&r);
    h.click(ids::tree_row(&rel("README.md")));
    assert_eq!(h.app.tree.selected, Some(rel("README.md")));
    assert_eq!(h.app.source.rel(), Some(&rel("README.md")));
    assert!(matches!(h.app.source.content, SourceContent::Text(_)));
    h.click(ids::tree_row(&rel("src")));
    assert!(h.app.tree.expanded.contains(&rel("src")));
    assert!(names(&h).contains(&"api.rs".to_string()));
    h.click(ids::tree_row(&rel("src")));
    assert!(!h.app.tree.expanded.contains(&rel("src")));
    assert!(!names(&h).contains(&"api.rs".to_string()));
}

// ---------- FR-TREE-05 ----------

#[test]
fn reveal_expands_ancestors_selects_and_scrolls_into_view() {
    let r = TempRepo::new();
    for i in 0..150 {
        r.write(&format!("a{i:03}.txt"), "");
    }
    r.write("zz/deep/er/target.rs", "fn main() {}\n");
    let mut h = opened(&r);
    let reveal = h.app.reveal(rel("zz/deep/er/target.rs"));
    h.run_task(reveal);
    for a in ["zz", "zz/deep", "zz/deep/er"] {
        assert!(h.app.tree.expanded.contains(&rel(a)), "{a}");
    }
    assert_eq!(h.app.tree.selected, Some(rel("zz/deep/er/target.rs")));
    let row = h
        .bounds(ids::tree_row(&rel("zz/deep/er/target.rs")))
        .expect("row built");
    let pane = h.bounds(ids::TREE_SCROLL).unwrap();
    assert!(
        row.y >= pane.y && row.y + row.height <= pane.y + pane.height + 0.5,
        "{row:?} in {pane:?}"
    );
}

// ---------- FR-TREE-08 ----------

#[test]
fn symlinks_show_a_marker_and_outside_links_cannot_be_opened() {
    let outside = TempRepo::new();
    outside.write("secret.txt", "TOP SECRET\n");
    let r = sample_repo();
    std::os::unix::fs::symlink(outside.path("secret.txt"), r.path("leak.txt")).unwrap();
    std::os::unix::fs::symlink(outside.root.clone(), r.path("leakdir")).unwrap();
    std::os::unix::fs::symlink(r.path("README.md"), r.path("readme-link.md")).unwrap();
    let mut h = opened(&r);
    assert!(h.has_text(" ↗"), "link marker");
    let n = h.log.len();
    h.click(ids::tree_row(&rel("leak.txt")));
    h.click(ids::tree_row(&rel("leakdir")));
    assert!(
        h.log[n..].iter().all(|l| l.starts_with("PaneClicked")),
        "outside links are disabled: {:?}",
        &h.log[n..]
    );
    assert!(h.app.source.rel().is_none());
    // The tooltip explains why (overlay text isn't reachable by selectors; the row is marked
    // blocked and the screenshot shows the tooltip).
    let row = h
        .app
        .tree
        .rows()
        .into_iter()
        .find(|r| r.name == "leak.txt")
        .unwrap();
    assert!(row.is_blocked());
    let b = h.bounds(ids::tree_row(&rel("leak.txt"))).unwrap();
    h.move_to(b.center());
    h.screenshot(&common::smoke_dir("m3").join("outside-link-tooltip.png"));
    // A link inside the root opens normally.
    h.click(ids::tree_row(&rel("readme-link.md")));
    assert!(matches!(h.app.source.content, SourceContent::Text(_)));
}

// ---------- FR-SRC-01 / FR-SRC-02 / FR-SRC-03 ----------

#[test]
fn the_viewer_is_read_only() {
    let r = sample_repo();
    let mut h = opened(&r);
    h.send(Message::OpenFile(rel("README.md"), None));
    let before = match &h.app.source.content {
        SourceContent::Text(t) => t.text.clone(),
        _ => panic!(),
    };
    h.click(ids::source_line(1));
    h.type_text("typed text");
    let after = match &h.app.source.content {
        SourceContent::Text(t) => t.text.clone(),
        _ => panic!(),
    };
    assert_eq!(before, after);
    assert!(
        h.app.focus.is_empty(),
        "keys don't leak into the Focus field"
    );
}

#[test]
fn syntax_is_chosen_by_extension_then_first_line() {
    let name = |p: &str, first: &str| source::select_syntax(p, first).map(|s| s.name.clone());
    assert_eq!(name("src/main.rs", "").as_deref(), Some("Rust"));
    assert_eq!(name("app.py", "").as_deref(), Some("Python"));
    assert_eq!(name("Makefile", "").as_deref(), Some("Makefile"));
    assert_eq!(
        name("scripts/run", "#!/usr/bin/env python3").as_deref(),
        Some("Python")
    );
    assert_eq!(
        name("scripts/run", "#!/bin/bash").as_deref(),
        Some("Bourne Again Shell (bash)")
    );
    assert_eq!(name("notes.unknownext", "hello"), None, "plain text");
    assert_eq!(name("Main.java", "").as_deref(), Some("Java"));
}

#[test]
fn long_lines_scroll_horizontally_without_wrapping() {
    let r = TempRepo::new();
    r.write("wide.txt", format!("{}\nshort\n", "x".repeat(2000)));
    let mut h = opened(&r);
    h.send(Message::OpenFile(rel("wide.txt"), None));
    let line1 = h.bounds(ids::source_line(1));
    let row = h.find(ids::source_line(1)).unwrap();
    let full = match row {
        iced_test::selector::Target::Container { bounds, .. } => bounds,
        other => panic!("{other:?}"),
    };
    assert!(
        full.width > 2000.0 * source::CHAR_W,
        "one unwrapped line: {full:?}"
    );
    assert_eq!(full.height, source::LINE_H);
    assert!(line1.is_some());
}

#[test]
fn only_visible_lines_plus_overscan_are_built() {
    let r = TempRepo::new();
    r.write("big.txt", lines(50_000));
    let mut h = opened(&r);
    h.send(Message::OpenFile(
        rel("big.txt"),
        Some(Cite {
            line: Some(30_000),
            end_line: None,
        }),
    ));
    let built = h.ids_with_prefix("src-line:");
    let pane_h = h
        .app
        .pane_body_height(archalign_desktop::panes::PaneKind::Source);
    let visible = (pane_h / source::LINE_H).ceil() as usize + 1;
    assert!(
        built.len() <= visible + 2 * OVERSCAN,
        "{} built",
        built.len()
    );
    assert!(built.iter().any(|(id, _)| id == "src-line:30000"));
}

// ---------- FR-SRC-04 ----------

#[test]
fn opening_a_citation_bands_the_range_and_puts_it_in_the_top_third() {
    let r = sample_repo();
    let mut h = opened(&r);
    h.send(Message::OpenFile(
        rel("src/api.rs"),
        Some(Cite {
            line: Some(50),
            end_line: Some(60),
        }),
    ));
    assert_eq!(h.app.source.band, Some((50, 60)));
    let view = h.bounds(ids::SOURCE_SCROLL).unwrap();
    let l50 = h.bounds(ids::source_line(50)).expect("line 50 visible");
    assert!(
        l50.y >= view.y && l50.y < view.y + view.height / 3.0 + source::LINE_H,
        "{l50:?} {view:?}"
    );
    h.screenshot(&common::smoke_dir("m3").join("cited-range.png"));
    // The band stays while scrolling, until another file or citation opens.
    h.move_to(view.center());
    h.wheel(-200.0);
    assert!(h.log.iter().any(|l| l.starts_with("SourceScrolled")));
    assert_eq!(h.app.source.band, Some((50, 60)));
    h.send(Message::OpenFile(
        rel("src/api.rs"),
        Some(Cite {
            line: Some(5),
            end_line: None,
        }),
    ));
    assert_eq!(
        h.app.source.band,
        Some((5, 5)),
        "just `line` without end_line"
    );
    h.send(Message::OpenFile(rel("README.md"), None));
    assert_eq!(h.app.source.band, None);
    // A citation without a line opens at the top with no band.
    h.send(Message::OpenFile(rel("src/api.rs"), Some(Cite::default())));
    assert_eq!(h.app.source.band, None);
    assert_eq!(h.app.source.scroll.offset_y, 0.0);
}

// ---------- FR-SRC-05 ----------

#[test]
fn an_out_of_range_citation_opens_at_line_1_with_a_banner() {
    let r = sample_repo();
    let mut h = opened(&r);
    h.send(Message::OpenFile(
        rel("src/db/store.rs"),
        Some(Cite {
            line: Some(100),
            end_line: Some(120),
        }),
    ));
    assert_eq!(h.app.source.band, None);
    assert_eq!(h.app.source.scroll.offset_y, 0.0);
    assert!(h.has_text("Cited lines 100–120 are outside this file (40 lines)"));
}

// ---------- FR-SRC-06 ----------

#[test]
fn large_and_binary_files_show_a_placeholder_and_invalid_utf8_a_banner() {
    let r = TempRepo::new();
    r.write("big.bin", vec![b'a'; 2 * 1024 * 1024 + 1]);
    r.write("img.png", [0x89u8, b'P', b'N', b'G', 0, 0, 1, 2]);
    r.write("latin1.txt", [b'c', b'a', b'f', 0xe9, b'\n']);
    let mut h = opened(&r);
    h.send(Message::OpenFile(rel("big.bin"), None));
    assert!(h.has_text("This file is larger than 2 MB and isn't displayed."));
    assert!(h.has_text("Size: 2.0 MB"));
    assert!(h.has(ids::SRC_COPY_PATH));
    h.send(Message::OpenFile(rel("img.png"), None));
    assert!(h.has_text("This looks like a binary file and isn't displayed."));
    h.click(ids::SRC_COPY_PATH);
    assert_eq!(h.clipboard.last().map(String::as_str), Some("img.png"));
    h.send(Message::OpenFile(rel("latin1.txt"), None));
    assert!(matches!(h.app.source.content, SourceContent::Text(_)));
    assert!(h.has_text("latin1.txt isn't valid UTF-8; invalid bytes are shown as \u{FFFD}"));
}

// ---------- FR-SRC-07 ----------

#[test]
fn copy_path_line_and_cited_lines() {
    let r = sample_repo();
    let mut h = opened(&r);
    h.send(Message::OpenFile(rel("src/api.rs"), None));
    let n = h.log.len();
    h.click(ids::SRC_COPY_LINES);
    assert!(
        h.log[n..].iter().all(|l| l.starts_with("PaneClicked")),
        "disabled without a band"
    );
    h.click(ids::SRC_COPY_PATH);
    assert_eq!(h.clipboard.last().map(String::as_str), Some("src/api.rs"));
    h.send(Message::OpenFile(
        rel("src/api.rs"),
        Some(Cite {
            line: Some(10),
            end_line: Some(12),
        }),
    ));
    h.click(ids::SRC_COPY_PATH);
    assert_eq!(
        h.clipboard.last().map(String::as_str),
        Some("src/api.rs:10")
    );
    h.click(ids::SRC_COPY_LINES);
    assert_eq!(
        h.clipboard.last().map(String::as_str),
        Some("line 10\nline 11\nline 12")
    );
}

// ---------- FR-SRC-08 ----------

#[test]
fn reads_are_confined_to_the_root() {
    let outside = TempRepo::new();
    outside.write("secret.txt", "TOP SECRET\n");
    let r = sample_repo();
    std::os::unix::fs::symlink(outside.path("secret.txt"), r.path("leak.txt")).unwrap();
    let mut h = opened(&r);
    h.send(Message::OpenFile(rel("leak.txt"), None));
    match &h.app.source.content {
        SourceContent::Error { message, .. } => assert!(message.contains("outside"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert!(!h.has_text("TOP SECRET"));
}

/// NFR-QUAL-04: revealing a path under a folder that can't be listed (a stale citation)
/// stops after one error instead of re-listing forever.
#[test]
fn revealing_under_a_missing_folder_stops_after_one_error() {
    let repo = common::fixtures::sample_repo();
    let mut h = Harness::new(Boot::default());
    h.send(Message::OpenRepo(repo.root.clone()));
    let t = h.app.reveal("README.md/inner/x.rs".parse().unwrap());
    h.run_task(t);
    assert_eq!(h.app.tree.pending_reveal, None);
    let errors = h
        .app
        .banners
        .iter()
        .filter(|b| b.text.starts_with("Can't list"))
        .count();
    assert_eq!(errors, 1, "{:?}", h.app.banners);
}
