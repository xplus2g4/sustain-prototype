//! The Submission profile for agent output (FR-VAL-05, ADR 0027).
//!
//! Order: app-owned fields and dropped fields → schema → serde → semantic checks. Citation
//! checks (step 5) need file access and are run by the caller through
//! [`crate::citations::check_citations`].

use serde_json::{Map, Value};

use crate::checks::{self, Profile};
use crate::document::parse_architecture;
use crate::issue::{Issue, pointer_token};
use crate::schema;
use crate::types::{ArchitectureDoc, Repository};

/// Values the app owns and writes over whatever the model supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppOwned {
    /// `meta.output`: `.archify/<folder>/<slug>.html` (never written).
    pub output: String,
    /// `meta.repository`, when git metadata is available.
    pub repository: Option<Repository>,
}

/// Component fields the model may not author.
const COMPONENT_DROPS: &[&str] = &["pos", "size", "row", "col", "icon", "brand"];
/// Connection fields the model may not author.
const CONNECTION_DROPS: &[&str] = &[
    "fromSide",
    "toSide",
    "route",
    "via",
    "labelAt",
    "labelDx",
    "labelDy",
    "labelSegment",
    "width",
];
/// Boundary fields the model may not author.
const BOUNDARY_DROPS: &[&str] = &["pad"];
/// `meta` fields the model may not author.
const META_DROPS: &[&str] = &[
    "viewBox",
    "views",
    "animation",
    "translations",
    "visual_preset",
    "engineering_profile",
];

fn drop_keys(obj: &mut Map<String, Value>, keys: &[&str], prefix: &str, out: &mut Vec<String>) {
    for key in keys {
        if obj.remove(*key).is_some() {
            out.push(format!("{prefix}/{}", pointer_token(key)));
        }
    }
}

fn drop_in_array(root: &mut Map<String, Value>, field: &str, keys: &[&str], out: &mut Vec<String>) {
    if let Some(Value::Array(items)) = root.get_mut(field) {
        for (i, item) in items.iter_mut().enumerate() {
            if let Value::Object(obj) = item {
                drop_keys(obj, keys, &format!("/{field}/{i}"), out);
            }
        }
    }
}

/// Removes the geometry and presentation fields the model may not supply. Returns the
/// JSON pointers of the removed fields, in document order by section.
pub fn drop_model_supplied_fields(value: &mut Value) -> Vec<String> {
    let mut out = Vec::new();
    let Value::Object(root) = value else {
        return out;
    };
    if root.remove("layout").is_some() {
        out.push("/layout".to_string());
    }
    if let Some(Value::Object(meta)) = root.get_mut("meta") {
        drop_keys(meta, META_DROPS, "/meta", &mut out);
    }
    drop_in_array(root, "components", COMPONENT_DROPS, &mut out);
    drop_in_array(root, "boundaries", BOUNDARY_DROPS, &mut out);
    drop_in_array(root, "connections", CONNECTION_DROPS, &mut out);
    out
}

/// Sets `schema_version`, `diagram_type`, `meta.output`, `meta.locale`,
/// `meta.quality_profile` and `meta.repository`. A model-supplied `meta.repository` is
/// removed when the app has no git metadata; its pointer is returned.
pub fn apply_app_owned_fields(value: &mut Value, app: &AppOwned) -> Vec<String> {
    let mut dropped = Vec::new();
    let Value::Object(root) = value else {
        return dropped;
    };
    root.insert("schema_version".into(), Value::from(1));
    root.insert("diagram_type".into(), Value::from("architecture"));
    let meta = root
        .entry("meta")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(meta) = meta {
        meta.insert("output".into(), Value::from(app.output.clone()));
        meta.insert("locale".into(), Value::from("en"));
        meta.insert("quality_profile".into(), Value::from("showcase"));
        match &app.repository {
            Some(repo) => {
                // Serializing a plain struct of strings cannot fail.
                let repo = serde_json::to_value(repo).unwrap_or(Value::Null);
                meta.insert("repository".into(), repo);
            }
            None => {
                if meta.remove("repository").is_some() {
                    dropped.push("/meta/repository".to_string());
                }
            }
        }
    }
    dropped
}

/// Result of the Submission profile, steps 1–4.
#[derive(Debug, Clone, PartialEq)]
pub struct SubmissionCheck {
    /// Pointers of the fields removed in step 1 (logged as `fields_dropped`).
    pub dropped: Vec<String>,
    /// The parsed document, or every error found by the first failing step.
    pub outcome: Result<ArchitectureDoc, Vec<Issue>>,
}

/// Runs Submission steps 1–4 on a model-supplied diagram value.
pub fn check_submission(mut value: Value, app: &AppOwned) -> SubmissionCheck {
    let mut dropped = drop_model_supplied_fields(&mut value);
    dropped.extend(apply_app_owned_fields(&mut value, app));
    let schema_errors = schema::validate_architecture(&value);
    if !schema_errors.is_empty() {
        return SubmissionCheck {
            dropped,
            outcome: Err(schema_errors),
        };
    }
    let doc = match parse_architecture(value) {
        Ok(doc) => doc,
        Err(issue) => {
            return SubmissionCheck {
                dropped,
                outcome: Err(vec![issue]),
            };
        }
    };
    let errors = checks::semantic(&doc, Profile::Submission);
    SubmissionCheck {
        dropped,
        outcome: if errors.is_empty() {
            Ok(doc)
        } else {
            Err(errors)
        },
    }
}
