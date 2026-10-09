//! JSON Schema validation against the vendored Archify schemas (ADR 0014).
//!
//! The schemas are embedded with `include_str!` and compiled once. The common schema is
//! registered as an in-memory resource, so `common.schema.json#/$defs/…` references
//! resolve offline; `jsonschema` is built without its HTTP and file resolvers.

use std::sync::LazyLock;

use serde_json::Value;

use crate::issue::{Issue, IssueCode};

/// Vendored `architecture.schema.json` (Archify `27f78a2`, MIT).
pub const ARCHITECTURE_SCHEMA: &str = include_str!("../schemas/architecture.schema.json");
/// Vendored `common.schema.json` (Archify `27f78a2`, MIT).
pub const COMMON_SCHEMA: &str = include_str!("../schemas/common.schema.json");

const COMMON_ID: &str = "https://github.com/tt-a1i/archify/schemas/common.schema.json";

/// `portableOutputPath` forbids UTF-16 surrogates with `not: {pattern: "[\uD800-\uDFFF]"}`.
/// Rust's `regex` cannot compile surrogate code points, and a Rust `String` can never contain
/// one (serde_json rejects lone surrogates), so that `not` always passes. The pattern is
/// swapped for an equivalent never-matching class in memory; the vendored file stays
/// byte-identical to upstream.
const SURROGATE_PATTERN: &str = r#""[\\uD800-\\uDFFF]""#;
const NEVER_MATCHES: &str = r#""[^\\s\\S]""#;

static VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    // The schemas are compile-time constants; failing here is a build defect, not user data.
    let common_text = COMMON_SCHEMA.replace(SURROGATE_PATTERN, NEVER_MATCHES);
    let common: Value =
        serde_json::from_str(&common_text).expect("vendored common schema is valid JSON");
    let architecture: Value = serde_json::from_str(ARCHITECTURE_SCHEMA)
        .expect("vendored architecture schema is valid JSON");
    let registry = jsonschema::Registry::new()
        .add(COMMON_ID, common)
        .expect("common schema registers")
        .prepare()
        .expect("registry prepares");
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .with_registry(&registry)
        .build(&architecture)
        .expect("vendored architecture schema compiles")
});

/// Validates a JSON value against the architecture schema. Returns one error issue per
/// violation, each with the JSON pointer of the offending instance location.
pub fn validate_architecture(value: &Value) -> Vec<Issue> {
    VALIDATOR
        .iter_errors(value)
        .map(|err| {
            Issue::error(
                IssueCode::Schema,
                err.instance_path().to_string(),
                err.to_string(),
            )
        })
        .collect()
}
