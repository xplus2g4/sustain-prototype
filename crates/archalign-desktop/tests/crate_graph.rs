//! NFR-QUAL-03: the crate dependency direction follows ADR 0003, and the pure crates
//! (`archalign-model`, `archalign-layout`) depend on no I/O crate.

use std::collections::BTreeSet;
use std::path::Path;

/// Names of `[dependencies]` entries in a crate manifest (dev-dependencies excluded).
fn normal_deps(crate_name: &str) -> BTreeSet<String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(crate_name)
        .join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).expect("manifest readable");
    let mut deps = BTreeSet::new();
    let mut in_deps = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_deps = line == "[dependencies]";
            continue;
        }
        if in_deps
            && !line.is_empty()
            && !line.starts_with('#')
            && let Some((name, _)) = line.split_once('=')
        {
            let name = name.trim().trim_matches('"');
            // `foo.workspace = true` and `foo = { … }` both name the crate `foo`.
            let name = name.split('.').next().unwrap_or(name);
            deps.insert(name.to_string());
        }
    }
    deps
}

fn internal(deps: &BTreeSet<String>) -> BTreeSet<&str> {
    deps.iter()
        .filter(|d| d.starts_with("archalign-"))
        .map(String::as_str)
        .collect()
}

#[test]
fn dependency_direction_matches_adr_0003() {
    let allowed: [(&str, &[&str]); 5] = [
        ("archalign-model", &[]),
        ("archalign-layout", &["archalign-model"]),
        ("archalign-repo", &["archalign-model"]),
        ("archalign-agent", &["archalign-model", "archalign-repo"]),
        (
            "archalign-desktop",
            &[
                "archalign-agent",
                "archalign-layout",
                "archalign-model",
                "archalign-repo",
            ],
        ),
    ];
    for (krate, ok) in allowed {
        let deps = normal_deps(krate);
        for dep in internal(&deps) {
            assert!(ok.contains(&dep), "{krate} must not depend on {dep}");
        }
    }
}

#[test]
fn pure_crates_have_no_io_dependencies() {
    const IO_CRATES: &[&str] = &[
        "tokio",
        "reqwest",
        "litellm-rust",
        "ignore",
        "notify-debouncer-mini",
        "rfd",
        "directories",
        "dotenvy",
        "iced",
        "syntect",
        "jiff",
        "regex",
        "tempfile",
    ];
    for krate in ["archalign-model", "archalign-layout"] {
        let deps = normal_deps(krate);
        for io in IO_CRATES {
            assert!(!deps.contains(*io), "{krate} must not depend on {io}");
        }
    }
}

#[test]
fn only_desktop_depends_on_iced() {
    for krate in [
        "archalign-model",
        "archalign-layout",
        "archalign-repo",
        "archalign-agent",
    ] {
        assert!(!normal_deps(krate).contains("iced"), "{krate} uses iced");
    }
}
