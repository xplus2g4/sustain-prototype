//! NFR-PERF-01 and NFR-PERF-02 timing tests. Run with
//! `cargo test -p archalign-repo --release -- --ignored perf_ --nocapture`.

use std::fs;
use std::time::{Duration, Instant};

use archalign_repo::{RelPath, Sandbox, TextLoad, TreeOptions, list_dir, load_text};

const BUDGET: Duration = Duration::from_millis(300);

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

#[test]
#[ignore = "timing test; run in release with --ignored"]
fn perf_top_level_listing_50k_files() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    for d in 0..50 {
        let dir = root.join(format!("dir{d:02}"));
        fs::create_dir(&dir).unwrap();
        for f in 0..1_000 {
            fs::write(dir.join(format!("file{f:04}.txt")), b"x").unwrap();
        }
    }
    for f in 0..20 {
        fs::write(root.join(format!("top{f:02}.txt")), b"x").unwrap();
    }
    fs::write(root.join(".gitignore"), "*.log\n").unwrap();
    let sandbox = Sandbox::new(root).unwrap();
    let opts = TreeOptions::default();
    let samples: Vec<Duration> = (0..5)
        .map(|_| {
            let start = Instant::now();
            let entries = list_dir(&sandbox, &RelPath::root(), &opts).unwrap();
            let took = start.elapsed();
            assert_eq!(entries.len(), 70);
            took
        })
        .collect();
    let med = median(samples.clone());
    println!("perf_top_level_listing_50k_files: median {med:?} of {samples:?}");
    assert!(med < BUDGET, "median {med:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "timing test; run in release with --ignored"]
fn perf_open_2mb_file_read_and_index() {
    let tmp = tempfile::tempdir().unwrap();
    let line = "    let value = compute(alpha, beta, gamma); // a typical source line\n";
    let count = (2 * 1024 * 1024 - 1) / line.len();
    fs::write(tmp.path().join("big.rs"), line.repeat(count)).unwrap();
    let sandbox = Sandbox::new(tmp.path()).unwrap();
    let rel = RelPath::parse("big.rs").unwrap();
    let samples: Vec<Duration> = (0..5)
        .map(|_| {
            let start = Instant::now();
            let loaded = load_text(&sandbox, &rel).unwrap();
            let took = start.elapsed();
            match loaded {
                TextLoad::Text { index, .. } => assert_eq!(index.line_count(), count),
                other => panic!("{other:?}"),
            }
            took
        })
        .collect();
    let med = median(samples.clone());
    println!("perf_open_2mb_file_read_and_index: median {med:?} of {samples:?}");
    assert!(med < BUDGET, "median {med:?} exceeds {BUDGET:?}");
}
