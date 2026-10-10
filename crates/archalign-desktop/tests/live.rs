//! Opt-in live run (spec §10, ADR 0029). Never part of the default suite: it is `#[ignore]`d
//! and does nothing unless `ARCHALIGN_LIVE=1`. One invocation makes exactly one run, with no
//! retries of the run itself:
//!
//! ```text
//! ARCHALIGN_LIVE=1 ARCHALIGN_LIVE_REPO=.smoke_repo/HyperTrack ARCHALIGN_LIVE_MODEL=openai/gpt-6-luna \
//!   cargo test -p archalign-desktop --release --test live -- --ignored --nocapture
//! ```
//!
//! The key comes from the normal lookup (FR-SEC-04) with the workspace root as the launch
//! directory, as if the app had been started there. Screenshots go to `target/smoke/<repo>/`.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use archalign_desktop::app::{Boot, Message};
use archalign_desktop::ids;
use archalign_desktop::keys::KeyEnv;
use archalign_desktop::settings::{Consent, Settings};
use common::Harness;
use iced::Point;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

#[test]
#[ignore = "live LLM run; needs ARCHALIGN_LIVE=1 (see the module docs)"]
fn live_archify_one_repo() {
    if std::env::var("ARCHALIGN_LIVE").as_deref() != Ok("1") {
        eprintln!("skipped: set ARCHALIGN_LIVE=1 to run a live archify");
        return;
    }
    let repo_arg = std::env::var("ARCHALIGN_LIVE_REPO").expect("ARCHALIGN_LIVE_REPO");
    let repo = workspace().join(&repo_arg);
    if !repo.is_dir() {
        eprintln!("skipped: {} is missing", repo.display());
        return;
    }
    let repo = repo.canonicalize().unwrap();
    let model =
        std::env::var("ARCHALIGN_LIVE_MODEL").unwrap_or_else(|_| "openai/gpt-6-luna".into());
    let focus = std::env::var("ARCHALIGN_LIVE_FOCUS").unwrap_or_default();
    let shots = shots_dir(&repo);
    let provider = model
        .split_once('/')
        .map(|(p, _)| p)
        .unwrap_or("openai")
        .to_string();
    let mut h = Harness::new(Boot {
        settings: Settings {
            model: model.clone(),
            ..Settings::default()
        },
        consents: vec![Consent {
            repo: repo.clone(),
            provider,
            model: model.clone(),
            granted_at: archalign_repo::now_rfc3339(),
        }],
        key_env: Some(KeyEnv {
            launch_dir: Some(workspace()),
            ..KeyEnv::process(None)
        }),
        llm: Some(archalign_desktop::agent_run::provider_factory()),
        ..Boot::default()
    });
    h.patience = Duration::from_millis(500);
    h.send(Message::OpenRepo(repo.clone()));
    assert!(h.app.repo.is_some(), "{:?}", h.app.banners);
    h.send(Message::FocusChanged(focus));
    h.click(ids::TB_ARCHIFY);
    assert!(
        h.app.run.is_running(),
        "run did not start: {:?}",
        h.app.banners
    );
    let started = std::time::Instant::now();
    let finished = h.wait_until(Duration::from_secs(16 * 60), |a| !a.run.is_running());
    println!(
        "--- run log ({:?}) ---\n{}",
        started.elapsed(),
        h.app.run_log.join("\n")
    );
    assert!(finished, "run did not finish within the wall clock");
    let folder = h
        .app
        .run_log
        .first()
        .and_then(|l| l.split_whitespace().nth(3))
        .unwrap_or_default()
        .to_string();
    println!("folder: {folder}");
    assert!(
        h.app
            .run_log
            .last()
            .is_some_and(|l| l.starts_with("✓ success")),
        "{:?}",
        h.app.banners
    );
    let rel = h.app.diagram.loaded().expect("diagram opened").rel.clone();
    assert!(
        repo.join(rel.parent().unwrap().as_str())
            .join("run.jsonl")
            .is_file()
    );
    drop(h);
    verify_newest(&repo, &shots, Some(rel), false);
}

/// Re-lays out the newest diagram of an already-smoked repo (no LLM call), then repeats the
/// reopen-and-click check. Used after a layout change, so the saved smoke diagrams pick it up:
///
/// ```text
/// ARCHALIGN_SMOKE_RELAYOUT_REPO=.smoke_repo/HyperTrack \
///   cargo test -p archalign-desktop --release --test live -- --ignored --nocapture smoke_relayout
/// ```
#[test]
#[ignore = "rewrites a smoke repo's newest diagram; needs ARCHALIGN_SMOKE_RELAYOUT_REPO"]
fn smoke_relayout_newest() {
    let Ok(repo_arg) = std::env::var("ARCHALIGN_SMOKE_RELAYOUT_REPO") else {
        eprintln!("skipped: set ARCHALIGN_SMOKE_RELAYOUT_REPO");
        return;
    };
    let repo = workspace().join(repo_arg).canonicalize().unwrap();
    let shots = shots_dir(&repo);
    verify_newest(&repo, &shots, None, true);
}

fn shots_dir(repo: &Path) -> PathBuf {
    let name = repo.file_name().unwrap().to_string_lossy().to_string();
    // `target/smoke/archify-src` is itself the repo copy: keep screenshots out of its tree.
    let shots = common::smoke_dir(&name);
    if shots.canonicalize().ok().as_deref() == Some(repo) {
        shots.join(".archalign-smoke")
    } else {
        shots
    }
}

/// Opens `repo` in a fresh app (the newest valid diagram opens on import), optionally
/// re-lays it out, screenshots it, and clicks 3 nodes with sources, checking path and band.
fn verify_newest(
    repo: &Path,
    shots: &Path,
    expect: Option<archalign_repo::RelPath>,
    relayout: bool,
) {
    let mut h = Harness::new(Boot::default());
    h.send(Message::OpenRepo(repo.to_path_buf()));
    let rel = h
        .app
        .diagram
        .loaded()
        .expect("diagram reopened")
        .rel
        .clone();
    if let Some(e) = expect {
        assert_eq!(rel, e, "the new diagram is the newest valid one");
    }
    if relayout {
        h.click(ids::RELAYOUT);
        assert_eq!(
            h.app.diagram.loaded().map(|d| d.rel.clone()),
            Some(rel.clone())
        );
        println!("re-laid out {rel}");
    }
    let d = h.app.diagram.loaded().expect("diagram open").clone();
    assert!(
        d.doc
            .components
            .iter()
            .all(|c| c.pos.is_some() && c.size.is_some())
    );
    // Let the canvas report its size, then screenshot the whole diagram.
    let c = h.bounds(ids::DIAGRAM_CANVAS).unwrap();
    h.move_to(Point::new(c.x + 1.0, c.y + 1.0));
    h.screenshot(&shots.join("diagram.png"));
    println!("screenshots: {}", shots.display());
    // Click 3 nodes with sources and check the path and band.
    let mut clicked = 0;
    for n in d.scene.nodes.iter() {
        let comp = &d.doc.components[n.index];
        let Some(src) = comp.sources().first() else {
            continue;
        };
        let center = n.rect.center();
        let p = h
            .app
            .diagram
            .camera
            .to_canvas(Point::new(center.x as f32, center.y as f32));
        let canvas = h.bounds(ids::DIAGRAM_CANVAS).unwrap();
        h.click_at(Point::new(canvas.x + p.x, canvas.y + p.y));
        assert_eq!(
            h.app.source.rel().map(|r| r.to_string()).as_deref(),
            Some(src.path.as_str()),
            "{}",
            comp.id
        );
        let expected = src
            .line
            .map(|l| (l as usize, src.end_line.unwrap_or(l) as usize));
        assert_eq!(h.app.source.band, expected, "{}", comp.id);
        println!("clicked {} → {}:{:?}", comp.id, src.path, expected);
        h.screenshot(&shots.join(format!("node-{}.png", clicked + 1)));
        clicked += 1;
        if clicked == 3 {
            break;
        }
    }
    assert_eq!(clicked, 3, "need 3 nodes with sources");
}
