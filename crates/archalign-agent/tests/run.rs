//! FR-GEN-05..10, FR-STO-02, FR-SEC-04/06 with a scripted fake `LlmClient`.

mod common;

use std::time::Duration;

use archalign_agent::budgets::Budgets;
use archalign_agent::llm::ChatMessage;
use archalign_agent::llm::LlmError;
use archalign_agent::prompt::REMINDER;
use archalign_agent::run::{AgentEvent, RunOutcome, run_agent};
use common::*;
use serde_json::json;

fn run(
    repo: &Repo,
    fake: &Fake,
    budgets: Budgets,
    tweak: impl FnOnce(&mut Setup),
) -> (RunOutcome, Setup, Vec<AgentEvent>) {
    let mut s = setup(repo, budgets);
    tweak(&mut s);
    let sink = Sink::default();
    let mut sink2 = sink.clone();
    let sleeper = FastSleeper::default();
    let log = s.log.take().expect("log");
    let outcome = block_on(run_agent(
        s.cfg.clone(),
        fake,
        s.tools.clone(),
        &s.finalize,
        &mut sink2,
        log,
        &sleeper,
    ));
    let evs = sink.0.lock().unwrap().clone();
    (outcome, s, evs)
}

fn candidate(repo: &Repo, s: &Setup) -> std::path::PathBuf {
    repo.root.join(format!("{}/candidate.json", s.folder))
}

#[test]
fn success_saves_the_candidate_and_logs_every_event_in_order() {
    let repo = sample();
    let fake = Fake::new(vec![
        Step::Reply(tools_reply(
            vec![call("1", "list_dir", json!({ "path": "" }))],
            1000,
        )),
        Step::Reply(tools_reply(
            vec![call(
                "2",
                "read_file",
                json!({ "path": "src/main.rs", "start_line": 1, "end_line": 10 }),
            )],
            2000,
        )),
        Step::Reply(submit(good_diagram())),
    ]);
    let (outcome, s, evs) = run(&repo, &fake, Budgets::default(), |_| {});
    let RunOutcome::Success { candidate: rel } = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(rel.to_string(), format!("{}/candidate.json", s.folder));
    assert!(candidate(&repo, &s).exists());
    let log = events(&repo, &s.folder);
    assert_eq!(
        event_names(&log),
        [
            "run_started",
            "turn",
            "tool_call",
            "turn",
            "tool_call",
            "turn",
            "tool_call",
            "run_finished"
        ]
    );
    assert_eq!(log[0]["model"], "openai/gpt-6-luna");
    assert_eq!(log[0]["focus"], "payments");
    assert_eq!(log[0]["budgets"]["max_turns"], 40);
    assert_eq!(log[4]["args"]["path"], "src/main.rs");
    assert!(log[4]["result_bytes"].as_u64().unwrap() > 0);
    let last = log.last().unwrap();
    assert_eq!(last["outcome"], "success");
    assert_eq!(last["turns"], 3);
    assert_eq!(last["input_tokens"], 3100);
    assert_eq!(last["candidate"], rel.to_string());
    for e in &log {
        assert!(
            e["ts"].as_str().unwrap().len() >= 25,
            "RFC 3339 with offset"
        );
    }
    // Progress events arrive in order (FR-GEN-09).
    let kinds: Vec<&str> = evs
        .iter()
        .map(|e| match e {
            AgentEvent::TurnStarted { .. } => "turn",
            AgentEvent::Usage { .. } => "usage",
            AgentEvent::ToolCalled { .. } => "call",
            AgentEvent::ToolResult { .. } => "result",
            AgentEvent::Rejected { .. } => "rejected",
            AgentEvent::Retry { .. } => "retry",
            AgentEvent::Finished(_) => "finished",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "turn", "usage", "call", "result", "turn", "usage", "call", "result", "turn", "usage",
            "call", "result", "finished"
        ]
    );
    // The saved file has the app-owned fields.
    let saved: serde_json::Value = serde_json::from_str(&repo.read(&rel.to_string())).unwrap();
    assert_eq!(saved["schema_version"], 1);
    assert_eq!(saved["meta"]["locale"], "en");
}

#[test]
fn rejections_return_errors_to_the_model_and_repairs_can_succeed() {
    let repo = sample();
    let mut bad = good_diagram();
    bad["components"][1]["sources"][0]["end_line"] = json!(999);
    bad["components"][0]["pos"] = json!([1, 2]);
    let fake = Fake::new(vec![
        Step::Reply(submit(bad)),
        Step::Reply(submit(good_diagram())),
    ]);
    let (outcome, s, evs) = run(&repo, &fake, Budgets::default(), |_| {});
    assert!(matches!(outcome, RunOutcome::Success { .. }), "{outcome:?}");
    // The second request carries the rejection as the tool result.
    let reqs = fake.requests.lock().unwrap();
    let ChatMessage::Tool { content, .. } = reqs[1].messages.last().unwrap() else {
        panic!()
    };
    assert!(
        content.contains("/components/1/sources/0/end_line"),
        "{content}"
    );
    assert!(content.contains("\"repairs_left\":3"), "{content}");
    let log = events(&repo, &s.folder);
    let names = event_names(&log);
    assert!(names.contains(&"fields_dropped".to_string()));
    let rej = log
        .iter()
        .find(|e| e["event"] == "submission_rejected")
        .unwrap();
    assert_eq!(rej["attempt"], 1);
    assert_eq!(
        rej["errors"][0]["pointer"],
        "/components/1/sources/0/end_line"
    );
    assert!(
        evs.iter()
            .any(|e| matches!(e, AgentEvent::Rejected { attempt: 1, .. }))
    );
}

#[test]
fn the_fourth_rejection_fails_the_run() {
    let repo = sample();
    let mut bad = good_diagram();
    bad["connections"][0]["to"] = json!("ghost");
    let fake = Fake::new((0..4).map(|_| Step::Reply(submit(bad.clone()))).collect());
    let (outcome, s, _) = run(&repo, &fake, Budgets::default(), |_| {});
    let RunOutcome::Failed { reason } = outcome else {
        panic!()
    };
    assert_eq!(reason, "Submission rejected 4 times (repair budget 3)");
    assert_eq!(fake.calls(), 4);
    assert!(!candidate(&repo, &s).exists(), "only run.jsonl is written");
    let log = events(&repo, &s.folder);
    assert_eq!(log.last().unwrap()["outcome"], "failed");
    assert_eq!(
        log.iter()
            .filter(|e| e["event"] == "submission_rejected")
            .count(),
        4
    );
}

#[test]
fn a_text_only_reply_gets_a_reminder_and_counts_as_a_turn() {
    let repo = sample();
    let fake = Fake::new(vec![
        Step::Reply(text_reply("Let me think…")),
        Step::Reply(submit(good_diagram())),
    ]);
    let budgets = Budgets {
        max_turns: 2,
        ..Budgets::default()
    };
    let (outcome, s, _) = run(&repo, &fake, budgets, |_| {});
    assert!(matches!(outcome, RunOutcome::Success { .. }));
    let reqs = fake.requests.lock().unwrap();
    assert_eq!(
        reqs[1].messages.last(),
        Some(&ChatMessage::User(REMINDER.to_string()))
    );
    assert_eq!(events(&repo, &s.folder).last().unwrap()["turns"], 2);
}

#[test]
fn invalid_tool_arguments_cost_a_turn_but_not_a_repair() {
    let repo = sample();
    let mut bad_args = call("x", "read_file", json!({}));
    bad_args.arguments = "{not json".into();
    let mut no_diagram = call("y", "submit_diagram", json!({}));
    no_diagram.arguments = "{\"wrong\": 1}".into();
    let fake = Fake::new(vec![
        Step::Reply(tools_reply(vec![bad_args], 10)),
        Step::Reply(tools_reply(vec![no_diagram], 10)),
        Step::Reply(submit(good_diagram())),
    ]);
    let (outcome, s, _) = run(
        &repo,
        &fake,
        Budgets {
            max_turns: 3,
            max_repairs: 0,
            ..Budgets::default()
        },
        |_| {},
    );
    assert!(matches!(outcome, RunOutcome::Success { .. }), "{outcome:?}");
    let reqs = fake.requests.lock().unwrap();
    let ChatMessage::Tool { content, .. } = reqs[1].messages.last().unwrap() else {
        panic!()
    };
    assert!(
        content.starts_with("error: invalid arguments for read_file"),
        "{content}"
    );
    let log = events(&repo, &s.folder);
    assert!(log.iter().all(|e| e["event"] != "submission_rejected"));
}

#[test]
fn the_turn_cap_fails_the_run() {
    let repo = sample();
    let fake = Fake::new(vec![]).with_fallback(tools_reply(
        vec![call("1", "list_dir", json!({ "path": "" }))],
        10,
    ));
    let (outcome, s, _) = run(
        &repo,
        &fake,
        Budgets {
            max_turns: 3,
            ..Budgets::default()
        },
        |_| {},
    );
    assert_eq!(
        outcome,
        RunOutcome::Failed {
            reason: "Turn budget exhausted (3 turns)".into()
        }
    );
    assert_eq!(fake.calls(), 3);
    let last = events(&repo, &s.folder).last().cloned().unwrap();
    assert_eq!(last["reason"], "Turn budget exhausted (3 turns)");
    assert!(!candidate(&repo, &s).exists());
}

#[test]
fn the_input_token_cap_fails_the_run() {
    let repo = sample();
    let fake = Fake::new(vec![]).with_fallback(tools_reply(
        vec![call("1", "list_dir", json!({ "path": "" }))],
        2_000_000,
    ));
    let (outcome, _, _) = run(&repo, &fake, Budgets::default(), |_| {});
    assert_eq!(
        outcome,
        RunOutcome::Failed {
            reason: "Input-token budget exhausted (3000000 tokens)".into()
        }
    );
    assert_eq!(fake.calls(), 2);
}

#[test]
fn the_wall_clock_cap_fails_the_run() {
    let repo = sample();
    let step = || {
        Step::Slow(
            Duration::from_millis(80),
            tools_reply(vec![call("1", "list_dir", json!({ "path": "" }))], 10),
        )
    };
    let fake = Fake::new((0..10).map(|_| step()).collect());
    let (outcome, s, _) = run(
        &repo,
        &fake,
        Budgets {
            wall_clock_s: 900,
            ..Budgets::default()
        },
        |s| {
            s.cfg.wall_clock = Duration::from_millis(200);
        },
    );
    assert_eq!(
        outcome,
        RunOutcome::Failed {
            reason: "Wall-clock budget exhausted (900 s)".into()
        }
    );
    assert!(fake.calls() <= 4);
    assert_eq!(
        events(&repo, &s.folder).last().unwrap()["outcome"],
        "failed"
    );
}

#[test]
fn the_per_request_timeout_is_retried_then_fails_the_run() {
    let repo = sample();
    let fake = Fake::new(vec![Step::Hang, Step::Hang, Step::Hang, Step::Hang]);
    let (outcome, s, evs) = run(&repo, &fake, Budgets::default(), |s| {
        s.cfg.request_timeout = Duration::from_millis(30);
    });
    assert_eq!(
        outcome,
        RunOutcome::Failed {
            reason: "Per-request timeout (300 s) exceeded on every retry".into()
        }
    );
    assert_eq!(fake.calls(), 4, "one call plus 3 retries");
    let log = events(&repo, &s.folder);
    let retries: Vec<_> = log.iter().filter(|e| e["event"] == "retry").collect();
    assert_eq!(retries.len(), 3);
    assert_eq!(retries[0]["timeout"], true);
    assert_eq!(
        retries
            .iter()
            .map(|r| r["wait_ms"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [1000, 2000, 4000]
    );
    assert_eq!(retries[2]["attempt"], 3);
    assert_eq!(
        evs.iter()
            .filter(|e| matches!(e, AgentEvent::Retry { .. }))
            .count(),
        3
    );
    // Retries don't count as turns.
    assert_eq!(log.last().unwrap()["turns"], 0);
}

#[test]
fn http_429_with_retry_after_waits_that_long_and_continues() {
    let repo = sample();
    let fake = Fake::new(vec![
        Step::Fail(LlmError::Status {
            status: 429,
            retry_after: Some(Duration::from_secs(9)),
            message: "slow down".into(),
        }),
        Step::Fail(LlmError::Status {
            status: 502,
            retry_after: None,
            message: "bad gateway".into(),
        }),
        Step::Reply(submit(good_diagram())),
    ]);
    let (outcome, s, _) = run(&repo, &fake, Budgets::default(), |_| {});
    assert!(matches!(outcome, RunOutcome::Success { .. }));
    let log = events(&repo, &s.folder);
    let retries: Vec<_> = log.iter().filter(|e| e["event"] == "retry").collect();
    assert_eq!(retries[0]["status"], 429);
    assert_eq!(retries[0]["wait_ms"], 9000);
    assert_eq!(retries[1]["status"], 502);
    assert_eq!(retries[1]["wait_ms"], 2000);
}

#[test]
fn cancelling_stops_within_a_second_and_saves_nothing() {
    let repo = sample();
    let fake = Fake::new(vec![
        Step::Reply(tools_reply(
            vec![call("1", "list_dir", json!({ "path": "" }))],
            10,
        )),
        Step::Hang,
    ]);
    let mut s = setup(&repo, Budgets::default());
    let log = s.log.take().unwrap();
    let mut sink = Sink::default();
    let sleeper = FastSleeper::default();
    let started = std::time::Instant::now();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let cancelled = rt.block_on(async {
        let fut = run_agent(
            s.cfg.clone(),
            &fake,
            s.tools.clone(),
            &s.finalize,
            &mut sink,
            log,
            &sleeper,
        );
        // The run is waiting on the model when the user cancels; dropping the future is the cancel.
        tokio::time::timeout(Duration::from_millis(100), fut)
            .await
            .is_err()
    });
    assert!(cancelled);
    assert!(started.elapsed() < Duration::from_secs(1));
    let log = events(&repo, &s.folder);
    let last = log.last().unwrap();
    assert_eq!(last["event"], "run_finished");
    assert_eq!(last["outcome"], "cancelled");
    assert!(
        !repo
            .root
            .join(format!("{}/candidate.json", s.folder))
            .exists()
    );
}

#[test]
fn the_key_and_file_contents_never_reach_the_run_log() {
    let repo = sample();
    repo.write(
        "src/secret_note.rs",
        "const TOKEN: &str = \"FILE-CONTENT-MARKER-42\";\n",
    );
    let fake = Fake::new(vec![
        Step::Reply(tools_reply(
            vec![call(
                "1",
                "read_file",
                json!({ "path": "src/secret_note.rs" }),
            )],
            10,
        )),
        Step::Reply(tools_reply(
            vec![call("2", "grep", json!({ "pattern": "MARKER" }))],
            10,
        )),
        Step::Reply(submit(good_diagram())),
    ]);
    let (outcome, s, _) = run(&repo, &fake, Budgets::default(), |_| {});
    assert!(matches!(outcome, RunOutcome::Success { .. }));
    // The tool results did carry the content to the model…
    let reqs = fake.requests.lock().unwrap();
    assert!(format!("{:?}", reqs[1].messages).contains("FILE-CONTENT-MARKER-42"));
    // …but run.jsonl only has paths, patterns and sizes.
    let raw = repo.read(&format!("{}/run.jsonl", s.folder));
    assert!(!raw.contains("FILE-CONTENT-MARKER-42"), "{raw}");
    assert!(!raw.contains("sk-"), "no key-like strings");
    assert!(raw.contains("\"pattern\":\"MARKER\""));
}
