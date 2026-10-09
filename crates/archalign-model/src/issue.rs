//! Problems found while loading, validating or checking a diagram.
//!
//! The same `Issue` feeds UI banners, the problem list and the repair messages sent to the
//! model (architecture §3.1).

use serde::{Deserialize, Serialize};

/// How serious an issue is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Blocks loading (Import) or acceptance (Submission).
    Error,
    /// Shown as a badge or problem entry; never blocks.
    Warning,
}

/// Machine-readable issue category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueCode {
    /// The bytes are not JSON.
    Json,
    /// JSON Schema violation.
    Schema,
    /// Strict serde parse failure.
    Parse,
    /// `diagram_type` is not `architecture`.
    UnsupportedType,
    /// Two components (or two connections) share an ID.
    DuplicateId,
    /// `from`/`to`/`wraps` names a component that does not exist.
    UnknownReference,
    /// A component has no `sources`.
    MissingSources,
    /// Two boundaries share members without one strictly containing the other.
    BoundaryOverlap,
    /// A citation's file is missing, unreadable, binary, too large, or its lines are out of range.
    BrokenCitation,
    /// A citation's path is malformed or escapes the repository root.
    InvalidCitationPath,
    /// A citation points at a path on the agent deny-list (Submission only).
    DeniedCitation,
    /// A cited file has uncommitted changes.
    ModifiedCitation,
    /// Only some components had positions; all were auto-laid-out.
    PartialPositions,
}

/// One problem, with the JSON pointer of the offending location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    /// Error or warning.
    pub severity: Severity,
    /// Category.
    pub code: IssueCode,
    /// RFC 6901 JSON pointer into the document (`""` for the whole document).
    pub pointer: String,
    /// Human-readable message.
    pub message: String,
}

impl Issue {
    /// An error issue.
    pub fn error(code: IssueCode, pointer: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            pointer: pointer.into(),
            message: message.into(),
        }
    }

    /// A warning issue.
    pub fn warning(
        code: IssueCode,
        pointer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            pointer: pointer.into(),
            message: message.into(),
        }
    }

    /// Whether this is an error.
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// The UI shows at most this many errors of a failed load (FR-VAL-01).
pub const MAX_SHOWN_ERRORS: usize = 20;

/// The first [`MAX_SHOWN_ERRORS`] errors, in order.
pub fn first_errors(issues: &[Issue]) -> impl Iterator<Item = &Issue> {
    issues
        .iter()
        .filter(|i| i.is_error())
        .take(MAX_SHOWN_ERRORS)
}

/// Escapes one JSON pointer reference token (RFC 6901).
pub fn pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}
