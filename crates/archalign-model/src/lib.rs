//! Archify v1 data model and validation (pure, no I/O).
//!
//! - [`types`]: strict serde types for every property of the vendored schemas (ADR 0012).
//! - [`schema`]: offline JSON Schema validation (ADR 0014).
//! - [`document`]: type detection and the Import-profile load pipeline.
//! - [`submission`]: the Submission profile for agent output (ADR 0027).
//! - [`edit`]: Re-layout geometry stripping and pretty serialization.
//! - [`citations`]: citation checks over a [`SourceProbe`] (FR-VAL-06, FR-VAL-07).

pub mod checks;
pub mod citations;
pub mod document;
pub mod edit;
pub mod issue;
pub mod num;
pub mod schema;
pub mod submission;
pub mod types;

pub use checks::Profile;
pub use citations::{FileProbe, SourceProbe, check_citations, citation_path_ok};
pub use document::{
    DiagramDocument, DiagramType, LoadFailure, Loaded, load_architecture, load_architecture_value,
};
pub use edit::{strip_geometry, to_pretty_json};
pub use issue::{Issue, IssueCode, Severity};
pub use num::{Num, Pair};
pub use submission::{AppOwned, SubmissionCheck, check_submission};
pub use types::*;
