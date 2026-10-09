//! NFR-PERF-02: opening a 2 MB text file (read, build the line index, highlight the first
//! visible window) in under 300 ms. Run with `cargo test --workspace --release -- --ignored perf_`.

use std::time::{Duration, Instant};

use archalign_desktop::source::{FileHighlighter, select_syntax};
use archalign_repo::text::{TextLoad, load_text};
use archalign_repo::{RelPath, Sandbox};

#[test]
#[ignore = "release-mode timing test; run with --release -- --ignored perf_"]
fn perf_open_2mb_file_read_index_and_highlight_first_window() {
    let dir = tempfile::tempdir().unwrap();
    let mut src = String::new();
    let mut i = 0;
    while src.len() < 2 * 1024 * 1024 - 200 {
        src.push_str(&format!(
            "pub fn function_{i}(x: u64) -> u64 {{ let y = x * {i}; // comment {i}\n    y + 1 }}\n"
        ));
        i += 1;
    }
    std::fs::write(dir.path().join("big.rs"), &src).unwrap();
    let sandbox = Sandbox::new(dir.path()).unwrap();
    let rel: RelPath = "big.rs".parse().unwrap();
    // Warm the syntax set once, as the app does at startup.
    let _ = select_syntax("x.rs", "");
    let mut times = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        let TextLoad::Text { text, index, .. } = load_text(&sandbox, &rel).unwrap() else {
            panic!("text")
        };
        let syntax = select_syntax("big.rs", index.line(&text, 0)).unwrap();
        let mut h = FileHighlighter::new(syntax, archalign_desktop::palette::ThemeChoice::Dark);
        h.ensure(&text, &index, 0..80);
        assert!(h.line(79).is_some());
        times.push(t.elapsed());
    }
    times.sort();
    let median = times[2];
    println!("open 2 MB file (read + index + highlight 80 lines): median {median:?}");
    assert!(median < Duration::from_millis(300), "{median:?}");
}
