//! M5 through the UI: preconditions, consent, runs, cancel, success, storage (FR-GEN-01..12,
//! FR-SEC-03/04, FR-STO-01..03).

mod common;

use std::time::Duration;

use archalign_desktop::app::{Action, Boot, Message, RunState};
use archalign_desktop::ids;
use archalign_desktop::settings::Consent;
use common::Harness;
use common::fakes::{Scripted, Step, key_env, submit, tools, topology};
use common::fixtures::{TempRepo, sample_repo};
use serde_json::json;

const KEY: &str = "sk-test-KEY-DO-NOT-LOG-7f3a";

fn consent(repo: &TempRepo) -> Consent {
    Consent {
        repo: repo.root.clone(),
        provider: "openai".into(),
        model: "gpt-6.1-sol".into(),
        granted_at: "2026-10-10T10:00:00+08:00".into(),
    }
}

fn boot(repo: &TempRepo, fake: &Scripted, consented: bool) -> Boot {
    Boot {
        key_env: Some(key_env("OPENAI_API_KEY", KEY)),
        llm: Some(fake.factory()),
        consents: if consented {
            vec![consent(repo)]
        } else {
            vec![]
        },
        ..Boot::default()
    }
}

fn opened(repo: &TempRepo, boot: Boot) -> Harness {
    let mut h = Harness::new(boot);
    h.send(Message::OpenRepo(repo.root.clone()));
    assert!(h.app.repo.is_some());
    h
}

fn archify_folders(repo: &TempRepo) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(repo.path(".archify"))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn run_events(repo: &TempRepo, folder: &str) -> Vec<serde_json::Value> {
    std::fs::read_to_string(repo.path(&format!(".archify/{folder}/run.jsonl")))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

// ---------- FR-GEN-01 ----------

#[test]
fn a_missing_key_shows_a_banner_and_makes_no_call() {
    let repo = sample_repo();
    let fake = Scripted::new(vec![]);
    let mut h = opened(
        &repo,
        Boot {
            llm: Some(fake.factory()),
            consents: vec![consent(&repo)],
            ..Boot::default()
        },
    );
    h.click(ids::TB_ARCHIFY);
    assert_eq!(fake.calls(), 0);
    assert!(
        fake.keys.lock().unwrap().is_empty(),
        "no client was even built"
    );
    let b = h.app.banners.last().unwrap();
    assert!(
        b.text.starts_with("No API key found: set OPENAI_API_KEY"),
        "{}",
        b.text
    );
    assert!(h.has_text(&b.text.clone()));
    assert!(archify_folders(&repo).is_empty(), "no folder created");
}

#[test]
fn missing_consent_shows_the_modal_and_makes_no_call() {
    let repo = sample_repo();
    let fake = Scripted::new(vec![]);
    let mut h = opened(&repo, boot(&repo, &fake, false));
    h.click(ids::TB_ARCHIFY);
    assert!(h.has(ids::CONSENT_MODAL));
    assert!(h.has_text("Provider: openai · Model: gpt-6.1-sol"));
    assert!(h.has_text("• contents of the files the agent reads"));
    assert!(h.has_text("• .env and .env.* (except .env.example and .env.sample)"));
    assert_eq!(fake.calls(), 0);
    h.screenshot(&common::smoke_dir("m5").join("consent.png"));
}

#[test]
fn an_uncreatable_diagram_folder_shows_a_banner_and_makes_no_call() {
    let repo = sample_repo();
    repo.write(".archify", "not a folder");
    let fake = Scripted::new(vec![]);
    let mut h = opened(&repo, boot(&repo, &fake, true));
    h.click(ids::TB_ARCHIFY);
    assert_eq!(fake.calls(), 0);
    assert!(
        h.app
            .banners
            .last()
            .unwrap()
            .text
            .starts_with("Can't create the diagram folder")
    );
    assert!(!h.app.run.is_running());
}

// ---------- FR-SEC-03 ----------

#[test]
fn consent_accept_decline_and_revoke() {
    let repo = sample_repo();
    let fake = Scripted::new(vec![Step::Reply(submit(topology()))]);
    let mut h = opened(&repo, boot(&repo, &fake, false));
    // Decline: nothing runs, viewing stays available.
    h.click(ids::TB_ARCHIFY);
    h.click(ids::CONSENT_DECLINE);
    assert!(!h.has(ids::CONSENT_MODAL));
    assert_eq!(fake.calls(), 0);
    assert!(h.app.consents.is_empty());
    assert!(
        h.app
            .banners
            .last()
            .unwrap()
            .text
            .contains("nothing was sent")
    );
    h.send(Message::OpenFile("README.md".parse().unwrap(), None));
    assert!(h.app.source.rel().is_some());
    // Accept: the consent is stored and the run starts.
    h.click(ids::TB_ARCHIFY);
    h.click(ids::CONSENT_ACCEPT);
    assert_eq!(h.app.consents.len(), 1);
    assert_eq!(h.app.consents[0].repo, repo.root);
    assert_eq!(h.app.consents[0].provider, "openai");
    assert!(h.wait_until(Duration::from_secs(10), |a| !a.run.is_running()));
    assert_eq!(fake.calls(), 1);
    // Revoke: the next Archify asks again.
    h.send(Message::RevokeConsent(0));
    assert!(h.app.consents.is_empty());
    h.click(ids::TB_ARCHIFY);
    assert!(h.has(ids::CONSENT_MODAL));
    assert_eq!(fake.calls(), 1);
}

// ---------- FR-GEN-02 / 03 / 05 / 06 / 09 / 12, FR-STO-01..03 ----------

#[test]
fn a_successful_run_saves_logs_and_opens_the_diagram() {
    let repo = sample_repo();
    repo.diagram(
        "architecture-old-20200101-000000",
        &common::fixtures::sample_doc("Old"),
    );
    let fake = Scripted::new(vec![
        Step::Reply(tools(vec![("list_dir", json!({ "path": "src" }))])),
        Step::Reply(tools(vec![(
            "read_file",
            json!({ "path": "src/api.rs", "start_line": 1, "end_line": 30 }),
        )])),
        Step::Reply(submit(topology())),
    ]);
    let mut h = opened(&repo, boot(&repo, &fake, true));
    assert_eq!(h.app.diagram.title().as_deref(), Some("Old"));
    h.click(ids::FOCUS_INPUT);
    h.type_text("Payment capture flow!");
    h.click(ids::TB_ARCHIFY);
    assert!(
        h.wait_until(Duration::from_secs(10), |a| !a.run.is_running()),
        "{:?}",
        h.app.run_log
    );
    // FR-GEN-02/03: the Focus is in the seed message, with tree and README.
    let reqs = fake.requests.lock().unwrap().clone();
    let seed = match &reqs[0].messages[1] {
        archalign_agent::ChatMessage::User(s) => s.clone(),
        other => panic!("{other:?}"),
    };
    assert!(seed.contains("Focus (from the user): Payment capture flow!"));
    assert!(seed.contains("### README.md") && seed.contains("### Cargo.toml"));
    assert!(
        matches!(&reqs[0].messages[0], archalign_agent::ChatMessage::System(s) if s.contains("submit_diagram"))
    );
    assert_eq!(fake.keys.lock().unwrap().as_slice(), [KEY]);
    // FR-STO-01: the folder name.
    let folders = archify_folders(&repo);
    let new = folders
        .iter()
        .find(|f| f.starts_with("architecture-payment-capture-flow-"))
        .expect("run folder");
    let ts = new.trim_start_matches("architecture-payment-capture-flow-");
    assert_eq!(ts.len(), 15, "{new}");
    assert!(ts.chars().enumerate().all(|(i, c)| if i == 8 {
        c == '-'
    } else {
        c.is_ascii_digit()
    }));
    // FR-STO-02: candidate.json + run.jsonl; meta.output points at the never-written HTML.
    let cand: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repo.path(&format!(".archify/{new}/candidate.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(
        cand["meta"]["output"],
        format!(".archify/{new}/payment-capture-flow.html")
    );
    assert!(
        !repo
            .path(&format!(".archify/{new}/payment-capture-flow.html"))
            .exists()
    );
    let evs = run_events(&repo, new);
    assert_eq!(evs.first().unwrap()["event"], "run_started");
    assert_eq!(evs.first().unwrap()["focus"], "Payment capture flow!");
    assert_eq!(evs.last().unwrap()["outcome"], "success");
    // FR-STO-03 [unit]: the file passes the vendored schema and has pos/size everywhere.
    let bytes = std::fs::read(repo.path(&format!(".archify/{new}/candidate.json"))).unwrap();
    let loaded = archalign_model::load_architecture(&bytes).expect("schema-valid");
    assert!(
        loaded
            .doc
            .components
            .iter()
            .all(|c| c.pos.is_some() && c.size.is_some())
    );
    // FR-GEN-12: the new diagram opened and is at the top of History.
    assert_eq!(h.app.diagram.title().as_deref(), Some("Generated"));
    let history = &h.app.repo.as_ref().unwrap().history;
    assert_eq!(history[0].folder, *new);
    // FR-GEN-09: the drawer shows the events in order.
    let log = h.app.run_log.join("\n");
    let order = [
        "Turn 1/40",
        "→ list_dir path=src",
        "← list_dir:",
        "Turn 2/40",
        "→ read_file",
        "Turn 3/40",
        "→ submit_diagram 3 components",
        "✓ success",
    ];
    let mut at = 0;
    for needle in order {
        let found = log[at..]
            .find(needle)
            .unwrap_or_else(|| panic!("{needle} after {at} in\n{log}"));
        at += found + needle.len();
    }
    assert!(h.has(ids::RUN_DRAWER));
    h.screenshot(&common::smoke_dir("m5").join("run-success.png"));
}

#[test]
fn a_failed_run_shows_the_reason_and_saves_only_the_log() {
    let repo = sample_repo();
    let mut bad = topology();
    bad["connections"][0]["to"] = json!("ghost");
    let fake = Scripted::new((0..4).map(|_| Step::Reply(submit(bad.clone()))).collect());
    let mut h = opened(&repo, boot(&repo, &fake, true));
    h.click(ids::TB_ARCHIFY);
    assert!(h.wait_until(Duration::from_secs(10), |a| !a.run.is_running()));
    let b = h.app.banners.last().unwrap();
    assert_eq!(
        b.text,
        "Archify failed: Submission rejected 4 times (repair budget 3)"
    );
    let folder = archify_folders(&repo).pop().unwrap();
    assert!(
        !repo
            .path(&format!(".archify/{folder}/candidate.json"))
            .exists()
    );
    assert!(
        h.app
            .run_log
            .iter()
            .any(|l| l.contains("✗ submission rejected (#4"))
    );
}

// ---------- FR-GEN-10 / FR-GEN-11 ----------

#[test]
fn cancel_stops_the_run_and_the_app_stays_usable_meanwhile() {
    let repo = sample_repo();
    let fake = Scripted::new(vec![
        Step::Reply(tools(vec![("list_dir", json!({ "path": "" }))])),
        Step::Hang,
    ]);
    let mut h = opened(&repo, boot(&repo, &fake, true));
    h.patience = Duration::from_millis(300);
    h.click(ids::TB_ARCHIFY);
    assert!(h.wait_until(Duration::from_secs(5), |_| fake.calls() == 2));
    assert!(h.app.run.is_running());
    assert_eq!(h.pending_tasks(), 1, "the run is waiting on the model");
    // FR-GEN-11: tree and source work; Open repo is disabled; a second run can't start.
    h.click(ids::tree_row(&"README.md".parse().unwrap()));
    assert_eq!(
        h.app.source.rel().map(|r| r.to_string()).as_deref(),
        Some("README.md")
    );
    let n = h.log.len();
    h.click(ids::TB_OPEN);
    assert!(h.log[n..].iter().all(|l| !l.starts_with("Action(")));
    assert!(!h.app.can_archify());
    assert!(h.has_text("Cancel"));
    assert!(
        h.has_text("Turn 2/40 · 1200 input tokens · 0:00")
            || h.has_text("Turn 2/40 · 1200 input tokens · 0:01")
    );
    h.screenshot(&common::smoke_dir("m5").join("run-in-progress.png"));
    // FR-GEN-10: Cancel ends it within a second; run.jsonl says cancelled; nothing saved.
    let t = std::time::Instant::now();
    h.click(ids::TB_ARCHIFY);
    h.pump(Duration::from_millis(100));
    assert!(t.elapsed() < Duration::from_secs(1));
    assert_eq!(h.app.run, RunState::Idle);
    assert_eq!(h.pending_tasks(), 0, "the run stream was dropped");
    let folder = archify_folders(&repo).pop().unwrap();
    let evs = run_events(&repo, &folder);
    assert_eq!(evs.last().unwrap()["outcome"], "cancelled");
    assert!(
        !repo
            .path(&format!(".archify/{folder}/candidate.json"))
            .exists()
    );
    // ⌘. also cancels.
    let fake2 = Scripted::new(vec![Step::Hang]);
    h.app.llm = fake2.factory();
    h.click(ids::TB_ARCHIFY);
    assert!(h.app.run.is_running());
    h.cmd(".");
    h.pump(Duration::from_millis(100));
    assert_eq!(h.app.run, RunState::Idle);
    let _ = Action::CancelRun;
}

// ---------- FR-SEC-04 / FR-SEC-06 ----------

#[test]
fn the_key_never_appears_in_the_run_log_or_tracing_output() {
    let repo = sample_repo();
    let fake = Scripted::new(vec![
        Step::Reply(tools(vec![("repo_meta", json!({}))])),
        Step::Reply(submit(topology())),
    ]);
    let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let writer = {
        let buf = buf.clone();
        move || BufWriter(buf.clone())
    };
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(writer)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let mut h = opened(&repo, boot(&repo, &fake, true));
    h.click(ids::TB_ARCHIFY);
    assert!(h.wait_until(Duration::from_secs(10), |a| !a.run.is_running()));
    let folder = archify_folders(&repo).pop().unwrap();
    let raw = std::fs::read_to_string(repo.path(&format!(".archify/{folder}/run.jsonl"))).unwrap();
    assert!(!raw.contains(KEY));
    let logs = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
    assert!(!logs.contains(KEY), "{logs}");
    assert!(!format!("{:?}", h.app.key_env).contains(KEY));
}

#[derive(Clone)]
struct BufWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for BufWriter {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
