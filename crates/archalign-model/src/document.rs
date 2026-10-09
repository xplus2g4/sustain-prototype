//! Diagram-type detection and the Import-profile load pipeline (FR-VAL-01, FR-VAL-04,
//! FR-DIA-12).
//!
//! Pipeline: bytes → JSON → schema → strict serde → semantic checks. Citation checks need
//! file access and are applied separately through [`crate::citations`].

use std::fmt;

use serde_json::Value;

use crate::checks::{self, Profile};
use crate::issue::{Issue, IssueCode};
use crate::schema;
use crate::types::ArchitectureDoc;

/// The Archify diagram types (ADR 0013).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DiagramType {
    /// Supported in v1.
    Architecture,
    /// Archify workflow diagram (unsupported).
    Workflow,
    /// Archify sequence diagram (unsupported).
    Sequence,
    /// Archify dataflow diagram (unsupported).
    Dataflow,
    /// Archify lifecycle diagram (unsupported).
    Lifecycle,
    /// Any other `diagram_type` string.
    Other(String),
    /// No string `diagram_type`, or not a JSON object.
    Unknown,
}

impl DiagramType {
    /// Reads `diagram_type` from a parsed JSON value.
    pub fn detect(value: &Value) -> Self {
        match value.get("diagram_type").and_then(Value::as_str) {
            Some("architecture") => DiagramType::Architecture,
            Some("workflow") => DiagramType::Workflow,
            Some("sequence") => DiagramType::Sequence,
            Some("dataflow") => DiagramType::Dataflow,
            Some("lifecycle") => DiagramType::Lifecycle,
            Some(other) => DiagramType::Other(other.to_string()),
            None => DiagramType::Unknown,
        }
    }

    /// Whether v1 can open this type.
    pub fn is_supported(&self) -> bool {
        matches!(self, DiagramType::Architecture)
    }
}

impl fmt::Display for DiagramType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            DiagramType::Architecture => "architecture",
            DiagramType::Workflow => "workflow",
            DiagramType::Sequence => "sequence",
            DiagramType::Dataflow => "dataflow",
            DiagramType::Lifecycle => "lifecycle",
            DiagramType::Other(s) => s,
            DiagramType::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

/// A loaded diagram. The per-type seam for later types (ADR 0013).
#[derive(Debug, Clone, PartialEq)]
pub enum DiagramDocument {
    /// An architecture diagram.
    Architecture(ArchitectureDoc),
}

/// A document that passed the Import profile, with its warnings.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    /// The parsed document.
    pub doc: ArchitectureDoc,
    /// Non-blocking findings (missing sources, boundary overlap, …).
    pub warnings: Vec<Issue>,
}

/// Why a document could not be opened.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadFailure {
    /// The detected type (`Unknown` when the bytes were not a JSON object).
    pub diagram_type: DiagramType,
    /// All errors, in pipeline order. The UI shows the first 20.
    pub errors: Vec<Issue>,
}

impl LoadFailure {
    fn single(diagram_type: DiagramType, issue: Issue) -> Self {
        Self {
            diagram_type,
            errors: vec![issue],
        }
    }
}

/// Parses bytes as JSON. Errors carry the line and column.
pub fn parse_json(bytes: &[u8]) -> Result<Value, Issue> {
    serde_json::from_slice(bytes)
        .map_err(|e| Issue::error(IssueCode::Json, "", format!("Malformed JSON: {e}")))
}

/// Strict serde parse of a JSON value that already passed the schema.
pub fn parse_architecture(value: Value) -> Result<ArchitectureDoc, Issue> {
    serde_json::from_value(value).map_err(|e| Issue::error(IssueCode::Parse, "", e.to_string()))
}

/// The message shown for a non-architecture document (FR-DIA-12).
pub fn unsupported_message(ty: &DiagramType) -> String {
    format!("Unsupported diagram type: `{ty}`")
}

/// Runs the Import-profile pipeline on raw file bytes (FR-VAL-01, FR-VAL-04).
pub fn load_architecture(bytes: &[u8]) -> Result<Loaded, LoadFailure> {
    let value = parse_json(bytes).map_err(|i| LoadFailure::single(DiagramType::Unknown, i))?;
    load_architecture_value(value)
}

/// Runs the Import-profile pipeline on an already-parsed JSON value.
pub fn load_architecture_value(value: Value) -> Result<Loaded, LoadFailure> {
    let ty = DiagramType::detect(&value);
    if !ty.is_supported() && ty != DiagramType::Unknown {
        let issue = Issue::error(
            IssueCode::UnsupportedType,
            "/diagram_type",
            unsupported_message(&ty),
        );
        return Err(LoadFailure::single(ty, issue));
    }
    let schema_errors = schema::validate_architecture(&value);
    if !schema_errors.is_empty() {
        return Err(LoadFailure {
            diagram_type: ty,
            errors: schema_errors,
        });
    }
    let doc = parse_architecture(value).map_err(|i| LoadFailure::single(ty.clone(), i))?;
    let issues = checks::semantic(&doc, Profile::Import);
    let (errors, warnings): (Vec<_>, Vec<_>) = issues.into_iter().partition(Issue::is_error);
    if errors.is_empty() {
        Ok(Loaded { doc, warnings })
    } else {
        Err(LoadFailure {
            diagram_type: ty,
            errors,
        })
    }
}
