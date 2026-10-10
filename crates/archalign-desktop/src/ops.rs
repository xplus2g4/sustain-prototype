//! Blocking work run off the UI thread (`Task::perform` + `spawn_blocking`, ADR 0008).
//! Every function here takes owned inputs and returns owned, message-ready results.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use archalign_layout::{Scene, layout_document, write_back};
use archalign_model::citations::check_citations;
use archalign_model::{ArchitectureDoc, Issue, Profile};
use archalign_repo::citations::RepoProbe;
use archalign_repo::git::{self, GitStatus};
use archalign_repo::storage::{self, HistoryEntry};
use archalign_repo::text::{TextLoad, load_text};
use archalign_repo::tree::{DirEntry, TreeOptions, list_dir};
use archalign_repo::{RelPath, Sandbox};

use crate::fonts::IcedTextMeasure;

/// Runs blocking work on tokio's blocking pool.
pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tokio::task::spawn_blocking(f).await.ok()
}

/// Everything known right after opening a repository.
#[derive(Debug, Clone)]
pub struct RepoOpened {
    /// Canonical root.
    pub root: PathBuf,
    /// Root confinement.
    pub sandbox: Sandbox,
    /// Git metadata.
    pub git: GitStatus,
    /// Top-level listing.
    pub top: Result<Vec<DirEntry>, String>,
    /// `.archify/` history, newest first.
    pub history: Vec<HistoryEntry>,
}

/// Opens a folder as the repo root (FR-IMP-02..04).
pub fn open_repo(
    path: PathBuf,
    show_hidden: bool,
    git_program: &std::ffi::OsStr,
) -> Result<RepoOpened, String> {
    let sandbox = Sandbox::new(&path).map_err(|e| format!("Can't open {}: {e}", path.display()))?;
    let root = sandbox.root().to_path_buf();
    let top = list_dir(&sandbox, &RelPath::root(), &TreeOptions { show_hidden })
        .map_err(|e| e.to_string());
    let git = git::collect_with(git_program, &root);
    let history = storage::scan_history(&sandbox);
    Ok(RepoOpened {
        root,
        sandbox,
        git,
        top,
        history,
    })
}

/// Lists one folder (FR-TREE-01).
pub fn list(sandbox: &Sandbox, dir: &RelPath, show_hidden: bool) -> Result<Vec<DirEntry>, String> {
    list_dir(sandbox, dir, &TreeOptions { show_hidden }).map_err(|e| e.to_string())
}

/// Reads a file through the sandbox (FR-SRC-06, FR-SRC-08).
pub fn read_file(sandbox: &Sandbox, rel: &RelPath) -> Result<TextLoad, String> {
    load_text(sandbox, rel).map_err(|e| e.to_string())
}

/// A diagram ready to draw.
#[derive(Debug, Clone)]
pub struct DiagramLoaded {
    /// `.archify/<folder>/candidate.json`.
    pub rel: RelPath,
    /// The document.
    pub doc: Arc<ArchitectureDoc>,
    /// Its geometry.
    pub scene: Arc<Scene>,
    /// Import warnings, layout warnings and citation findings.
    pub warnings: Vec<Issue>,
}

/// Why a diagram didn't open.
#[derive(Debug, Clone)]
pub struct DiagramFailed {
    /// Path.
    pub rel: RelPath,
    /// First errors (pointer + message), at most 20.
    pub errors: Vec<Issue>,
    /// Set when the file is valid JSON of another Archify type.
    pub unsupported: Option<String>,
}

/// Loads, validates, lays out and citation-checks a diagram file. Never writes.
pub fn load_diagram(
    sandbox: &Sandbox,
    rel: &RelPath,
    dirty: &HashSet<String>,
) -> Result<DiagramLoaded, DiagramFailed> {
    let fail = |errors: Vec<Issue>, unsupported: Option<String>| DiagramFailed {
        rel: rel.clone(),
        errors,
        unsupported,
    };
    let bytes = sandbox
        .resolve(rel)
        .map_err(|e| e.to_string())
        .and_then(|p| std::fs::read(p).map_err(|e| e.to_string()))
        .map_err(|e| {
            fail(
                vec![Issue::error(
                    archalign_model::IssueCode::Json,
                    "",
                    format!("Can't read {rel}: {e}"),
                )],
                None,
            )
        })?;
    let loaded = archalign_model::load_architecture(&bytes).map_err(|f| {
        let unsupported = (!f.diagram_type.is_supported()
            && f.diagram_type != archalign_model::DiagramType::Unknown)
            .then(|| f.diagram_type.to_string());
        fail(
            archalign_model::issue::first_errors(&f.errors)
                .cloned()
                .collect(),
            unsupported,
        )
    })?;
    Ok(finish_diagram(
        sandbox,
        rel.clone(),
        loaded.doc,
        loaded.warnings,
        dirty,
    ))
}

/// The citation findings for `doc` now (FR-WAT-05).
pub fn citation_issues(
    sandbox: &Sandbox,
    doc: &ArchitectureDoc,
    dirty: &HashSet<String>,
) -> Vec<Issue> {
    check_citations(doc, &RepoProbe::new(sandbox), Profile::Import, dirty)
}

/// Lays out and citation-checks an already-validated document.
pub fn finish_diagram(
    sandbox: &Sandbox,
    rel: RelPath,
    doc: ArchitectureDoc,
    mut warnings: Vec<Issue>,
    dirty: &HashSet<String>,
) -> DiagramLoaded {
    let scene = layout_document(&doc, &IcedTextMeasure);
    warnings.extend(scene.warnings.iter().cloned());
    let probe = RepoProbe::new(sandbox);
    warnings.extend(check_citations(&doc, &probe, Profile::Import, dirty));
    DiagramLoaded {
        rel,
        doc: Arc::new(doc),
        scene: Arc::new(scene),
        warnings,
    }
}

/// The failure reported when a background job was dropped before it finished.
pub fn interrupted_failure(rel: RelPath) -> DiagramFailed {
    DiagramFailed {
        rel,
        errors: vec![Issue::error(
            archalign_model::IssueCode::Json,
            "",
            "the operation was interrupted",
        )],
        unsupported: None,
    }
}

/// Re-layout (FR-DIA-11, ADR 0019): strip geometry, lay out, write integer `pos`/`size`
/// back, save `candidate.json` atomically, then reload the saved bytes through the Import
/// profile so the view matches the file.
pub fn relayout(
    sandbox: &Sandbox,
    rel: &RelPath,
    mut doc: ArchitectureDoc,
    dirty: &HashSet<String>,
    writes: &archalign_repo::SelfWrites,
) -> Result<DiagramLoaded, DiagramFailed> {
    archalign_model::strip_geometry(&mut doc);
    let scene = layout_document(&doc, &IcedTextMeasure);
    write_back(&mut doc, &scene);
    let bytes = archalign_model::to_pretty_json(&doc);
    archalign_repo::storage::atomic_write_tracked(sandbox, rel, &bytes, writes).map_err(|e| {
        DiagramFailed {
            rel: rel.clone(),
            errors: vec![Issue::error(
                archalign_model::IssueCode::Json,
                "",
                format!("Can't save {rel}: {e}"),
            )],
            unsupported: None,
        }
    })?;
    let loaded = archalign_model::load_architecture(&bytes).map_err(|f| DiagramFailed {
        rel: rel.clone(),
        errors: f.errors,
        unsupported: None,
    })?;
    Ok(finish_diagram(
        sandbox,
        rel.clone(),
        loaded.doc,
        loaded.warnings,
        dirty,
    ))
}
