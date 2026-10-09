//! FR-VAL-01, FR-VAL-02, FR-VAL-04, FR-VAL-05 and FR-DIA-12.

use archalign_model::document::parse_architecture;
use archalign_model::issue::{MAX_SHOWN_ERRORS, first_errors};
use archalign_model::submission::{apply_app_owned_fields, drop_model_supplied_fields};
use archalign_model::{
    AppOwned, DiagramType, IssueCode, Repository, Severity, check_submission, load_architecture,
    load_architecture_value,
};
use serde_json::{Value, json};

fn base() -> Value {
    json!({
        "schema_version": 1,
        "diagram_type": "architecture",
        "meta": { "title": "T", "output": "out/t.html" },
        "components": [
            { "id": "a", "type": "frontend", "label": "A", "sources": [{ "path": "src/a.rs", "line": 1 }] },
            { "id": "b", "type": "backend", "label": "B", "sources": [{ "path": "src/b.rs" }] },
            { "id": "c", "type": "database", "label": "C", "sources": [{ "path": "src/c.rs" }] }
        ],
        "connections": [ { "id": "ab", "from": "a", "to": "b" } ],
        "boundaries": [ { "kind": "region", "label": "R", "wraps": ["a", "b"] } ]
    })
}

fn load(v: Value) -> Result<archalign_model::Loaded, archalign_model::LoadFailure> {
    load_architecture(&serde_json::to_vec(&v).unwrap())
}

fn codes(issues: &[archalign_model::Issue]) -> Vec<IssueCode> {
    issues.iter().map(|i| i.code).collect()
}

// ---------- FR-VAL-01 ----------

#[test]
fn malformed_json_is_an_error_with_position() {
    let err = load_architecture(b"{ \"schema_version\": 1, ").unwrap_err();
    assert_eq!(codes(&err.errors), vec![IssueCode::Json]);
    assert!(err.errors[0].message.contains("line 1"), "{:?}", err.errors);
}

#[test]
fn schema_errors_carry_pointer_and_message() {
    let mut v = base();
    v["components"][1]["type"] = json!("mainframe");
    let err = load(v).unwrap_err();
    assert_eq!(err.errors[0].code, IssueCode::Schema);
    assert_eq!(err.errors[0].pointer, "/components/1/type");
    assert!(!err.errors[0].message.is_empty());
}

#[test]
fn ui_shows_at_most_twenty_errors() {
    let mut v = base();
    let comps: Vec<Value> = (0..30)
        .map(|i| json!({ "id": format!("n{i}"), "type": "nope", "label": "x" }))
        .collect();
    v["components"] = Value::Array(comps);
    let err = load(v).unwrap_err();
    assert!(err.errors.len() >= 30);
    let shown: Vec<_> = first_errors(&err.errors).collect();
    assert_eq!(shown.len(), MAX_SHOWN_ERRORS);
    assert_eq!(shown[0], &err.errors[0]);
}

// ---------- FR-VAL-02 ----------

#[test]
fn extra_field_fails_and_names_the_field() {
    let mut v = base();
    v["components"][0]["colour"] = json!("red");
    let err = load(v.clone()).unwrap_err();
    assert!(
        err.errors.iter().any(|e| e.message.contains("colour")),
        "{:?}",
        err.errors
    );
    // The strict serde types reject it on their own as well.
    let parse = parse_architecture(v).unwrap_err();
    assert_eq!(parse.code, IssueCode::Parse);
    assert!(parse.message.contains("colour"), "{}", parse.message);
}

#[test]
fn extra_nested_meta_field_fails() {
    let mut v = base();
    v["meta"]["legend"] = json!({ "mode": "auto", "entries": { "robot": { "label": "R" } } });
    let err = load(v).unwrap_err();
    assert!(err.errors.iter().any(|e| e.message.contains("robot")));
}

// ---------- FR-VAL-04: Import profile, one case per rule ----------

#[test]
fn import_valid_base_has_no_issues() {
    let loaded = load(base()).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
}

#[test]
fn import_unknown_from_is_error() {
    let mut v = base();
    v["connections"][0]["from"] = json!("ghost");
    let err = load(v).unwrap_err();
    assert_eq!(codes(&err.errors), vec![IssueCode::UnknownReference]);
    assert_eq!(err.errors[0].pointer, "/connections/0/from");
}

#[test]
fn import_unknown_to_is_error() {
    let mut v = base();
    v["connections"][0]["to"] = json!("ghost");
    let err = load(v).unwrap_err();
    assert_eq!(err.errors[0].pointer, "/connections/0/to");
}

#[test]
fn import_unknown_wraps_is_error() {
    let mut v = base();
    v["boundaries"][0]["wraps"] = json!(["a", "ghost"]);
    let err = load(v).unwrap_err();
    assert_eq!(codes(&err.errors), vec![IssueCode::UnknownReference]);
    assert_eq!(err.errors[0].pointer, "/boundaries/0/wraps/1");
}

#[test]
fn import_duplicate_component_id_is_error() {
    let mut v = base();
    v["components"][2]["id"] = json!("a");
    let err = load(v).unwrap_err();
    assert!(codes(&err.errors).contains(&IssueCode::DuplicateId));
    assert_eq!(err.errors[0].pointer, "/components/2/id");
}

#[test]
fn import_duplicate_connection_id_is_error() {
    let mut v = base();
    v["connections"] = json!([
        { "id": "x", "from": "a", "to": "b" },
        { "id": "x", "from": "b", "to": "c" }
    ]);
    let err = load(v).unwrap_err();
    assert_eq!(codes(&err.errors), vec![IssueCode::DuplicateId]);
    assert_eq!(err.errors[0].pointer, "/connections/1/id");
}

#[test]
fn import_missing_sources_is_warning() {
    let mut v = base();
    v["components"][2]
        .as_object_mut()
        .unwrap()
        .remove("sources");
    let loaded = load(v).unwrap();
    assert_eq!(codes(&loaded.warnings), vec![IssueCode::MissingSources]);
    assert_eq!(loaded.warnings[0].severity, Severity::Warning);
}

#[test]
fn import_partial_boundary_overlap_is_warning() {
    let mut v = base();
    v["boundaries"] = json!([
        { "kind": "region", "label": "R1", "wraps": ["a", "b"] },
        { "kind": "security-group", "label": "R2", "wraps": ["b", "c"] }
    ]);
    let loaded = load(v).unwrap();
    assert_eq!(codes(&loaded.warnings), vec![IssueCode::BoundaryOverlap]);
}

#[test]
fn import_nested_and_separate_boundaries_are_fine() {
    let mut v = base();
    v["boundaries"] = json!([
        { "kind": "region", "label": "Outer", "wraps": ["a", "b"] },
        { "kind": "security-group", "label": "Inner", "wraps": ["b"] },
        { "kind": "region", "label": "Other", "wraps": ["c"] }
    ]);
    assert!(load(v).unwrap().warnings.is_empty());
}

// ---------- FR-DIA-12 ----------

#[test]
fn non_architecture_type_is_unsupported() {
    for ty in ["workflow", "sequence", "dataflow", "lifecycle", "mindmap"] {
        let err = load(json!({ "schema_version": 1, "diagram_type": ty, "meta": {} })).unwrap_err();
        assert_eq!(codes(&err.errors), vec![IssueCode::UnsupportedType]);
        assert_eq!(
            err.errors[0].message,
            format!("Unsupported diagram type: `{ty}`")
        );
        assert!(!err.diagram_type.is_supported());
        assert_eq!(err.diagram_type.to_string(), ty);
    }
}

#[test]
fn detects_types() {
    assert_eq!(
        DiagramType::detect(&json!({"diagram_type": "workflow"})),
        DiagramType::Workflow
    );
    assert_eq!(DiagramType::detect(&json!([1, 2])), DiagramType::Unknown);
    assert_eq!(DiagramType::detect(&base()), DiagramType::Architecture);
}

#[test]
fn non_object_json_fails_without_panicking() {
    for text in ["[]", "null", "42", "\"x\"", "{}"] {
        assert!(load_architecture(text.as_bytes()).is_err(), "{text}");
    }
}

// ---------- FR-VAL-05: Submission profile ----------

fn app() -> AppOwned {
    AppOwned {
        output: ".archify/architecture-x-20261010-101010/x.html".into(),
        repository: Some(Repository {
            url: "https://github.com/o/r.git".into(),
            provider: None,
            link_mode: None,
            revision: "0123456789abcdef0123456789abcdef01234567".into(),
        }),
    }
}

fn submission() -> Value {
    let mut v = base();
    let obj = v.as_object_mut().unwrap();
    obj.remove("schema_version");
    obj.remove("diagram_type");
    v["meta"] = json!({ "title": "T" });
    v
}

#[test]
fn submission_accepts_valid_topology_and_sets_app_owned_fields() {
    let check = check_submission(submission(), &app());
    let doc = check.outcome.unwrap();
    assert!(check.dropped.is_empty());
    assert_eq!(doc.schema_version, 1);
    assert_eq!(doc.diagram_type, "architecture");
    assert_eq!(doc.meta.output, app().output);
    assert_eq!(doc.meta.locale.as_deref(), Some("en"));
    assert_eq!(
        doc.meta.quality_profile,
        Some(archalign_model::QualityProfile::Showcase)
    );
    assert_eq!(doc.meta.repository, app().repository);
}

#[test]
fn submission_overrides_model_supplied_app_owned_fields() {
    let mut v = submission();
    v["schema_version"] = json!(7);
    v["diagram_type"] = json!("workflow");
    v["meta"]["output"] = json!("../../etc/x.html");
    v["meta"]["locale"] = json!("fr");
    v["meta"]["quality_profile"] = json!("standard");
    let doc = check_submission(v, &app()).outcome.unwrap();
    assert_eq!(doc.meta.output, app().output);
    assert_eq!(doc.meta.locale.as_deref(), Some("en"));
}

#[test]
fn submission_drops_and_reports_geometry_and_presentation_fields() {
    let mut v = submission();
    v["layout"] = json!({ "mode": "grid" });
    v["meta"]["viewBox"] = json!([800, 600]);
    v["meta"]["views"] = json!([]);
    v["meta"]["animation"] = json!("trace");
    v["meta"]["translations"] = json!({});
    v["meta"]["visual_preset"] = json!("classic");
    v["meta"]["engineering_profile"] = json!("deployment-ownership");
    v["components"][0]["pos"] = json!([1, 2]);
    v["components"][0]["size"] = json!([100, 50]);
    v["components"][0]["row"] = json!(0);
    v["components"][0]["col"] = json!(1);
    v["components"][0]["icon"] = json!("clock");
    v["components"][0]["brand"] = json!("rust");
    v["boundaries"][0]["pad"] = json!(10);
    for k in [
        "fromSide",
        "toSide",
        "route",
        "via",
        "labelAt",
        "labelDx",
        "labelDy",
        "labelSegment",
        "width",
    ] {
        v["connections"][0][k] = json!(null);
    }
    let check = check_submission(v, &app());
    let doc = check.outcome.unwrap();
    assert_eq!(check.dropped.len(), 7 + 6 + 1 + 9, "{:?}", check.dropped);
    for p in [
        "/layout",
        "/meta/viewBox",
        "/components/0/pos",
        "/components/0/brand",
        "/boundaries/0/pad",
        "/connections/0/via",
        "/connections/0/labelSegment",
    ] {
        assert!(check.dropped.contains(&p.to_string()), "{p}");
    }
    assert!(doc.components[0].pos.is_none() && doc.layout.is_none());
}

#[test]
fn submission_drops_model_repository_without_git() {
    let mut v = submission();
    v["meta"]["repository"] = json!({ "url": "x", "revision": "0".repeat(40) });
    let app = AppOwned {
        repository: None,
        ..app()
    };
    let check = check_submission(v, &app);
    assert_eq!(check.dropped, vec!["/meta/repository".to_string()]);
    assert!(check.outcome.unwrap().meta.repository.is_none());
}

#[test]
fn submission_rejects_schema_violation() {
    let mut v = submission();
    v["components"][0]["type"] = json!("robot");
    let errors = check_submission(v, &app()).outcome.unwrap_err();
    assert_eq!(codes(&errors), vec![IssueCode::Schema]);
}

#[test]
fn submission_rejects_unknown_field() {
    let mut v = submission();
    v["components"][0]["notes"] = json!("x");
    let errors = check_submission(v, &app()).outcome.unwrap_err();
    assert!(errors[0].message.contains("notes"));
}

#[test]
fn submission_rejects_duplicate_ids() {
    let mut v = submission();
    v["components"][1]["id"] = json!("a");
    let errors = check_submission(v, &app()).outcome.unwrap_err();
    assert!(codes(&errors).contains(&IssueCode::DuplicateId));
}

#[test]
fn submission_rejects_unresolved_reference() {
    let mut v = submission();
    v["connections"][0]["to"] = json!("zz");
    let errors = check_submission(v, &app()).outcome.unwrap_err();
    assert_eq!(codes(&errors), vec![IssueCode::UnknownReference]);
}

#[test]
fn submission_rejects_missing_sources() {
    let mut v = submission();
    v["components"][1]
        .as_object_mut()
        .unwrap()
        .remove("sources");
    let errors = check_submission(v, &app()).outcome.unwrap_err();
    assert_eq!(codes(&errors), vec![IssueCode::MissingSources]);
    assert_eq!(errors[0].severity, Severity::Error);
}

#[test]
fn submission_rejects_too_many_sources() {
    let mut v = submission();
    v["components"][0]["sources"] = json!([
        { "path": "a" }, { "path": "b" }, { "path": "c" }, { "path": "d" }
    ]);
    let errors = check_submission(v, &app()).outcome.unwrap_err();
    assert_eq!(errors[0].pointer, "/components/0/sources");
}

#[test]
fn submission_rejects_partial_boundary_overlap() {
    let mut v = submission();
    v["boundaries"] = json!([
        { "kind": "region", "label": "R1", "wraps": ["a", "b"] },
        { "kind": "region", "label": "R2", "wraps": ["b", "c"] }
    ]);
    let errors = check_submission(v, &app()).outcome.unwrap_err();
    assert_eq!(codes(&errors), vec![IssueCode::BoundaryOverlap]);
}

#[test]
fn submission_rejects_equal_boundaries() {
    let mut v = submission();
    v["boundaries"] = json!([
        { "kind": "region", "label": "R1", "wraps": ["a", "b"] },
        { "kind": "region", "label": "R2", "wraps": ["b", "a"] }
    ]);
    let errors = check_submission(v, &app()).outcome.unwrap_err();
    assert_eq!(codes(&errors), vec![IssueCode::BoundaryOverlap]);
}

#[test]
fn drop_and_app_owned_ignore_non_objects() {
    let mut v = json!([1, 2]);
    assert!(drop_model_supplied_fields(&mut v).is_empty());
    assert!(apply_app_owned_fields(&mut v, &app()).is_empty());
    assert_eq!(v, json!([1, 2]));
    let check = check_submission(json!("nope"), &app());
    assert!(check.outcome.is_err());
}

// ---------- portableOutputPath still enforced after the surrogate-pattern swap ----------

#[test]
fn output_path_rules_are_enforced() {
    for bad in [
        "../x.html",
        "/abs/x.html",
        "a/b.txt",
        "a\\b.html",
        "con/x.html",
        "a./b.html",
    ] {
        let mut v = base();
        v["meta"]["output"] = json!(bad);
        let err = load_architecture_value(v).unwrap_err();
        assert!(
            err.errors.iter().any(|e| e.pointer == "/meta/output"),
            "{bad}: {:?}",
            err.errors
        );
    }
    let mut v = base();
    v["meta"]["output"] = json!(".archify/architecture-x-20261010-101010/x.html");
    assert!(load_architecture_value(v).is_ok());
}

/// Every property in the architecture and common schemas, set at once (FR-VAL-02).
#[test]
fn kitchen_sink_document_covers_every_schema_property() {
    let v = json!({
        "schema_version": 1,
        "diagram_type": "architecture",
        "meta": {
            "title": "All", "locale": "en", "translations": { "node.a": "A" },
            "subtitle": "s", "output": "x/all.html", "animation": "trace",
            "visual_preset": "signal-flow", "quality_profile": "standard",
            "engineering_profile": "deployment-ownership",
            "repository": { "url": "https://github.com/o/r", "provider": "github",
                            "link_mode": "local-only", "revision": "a".repeat(40) },
            "views": [{ "id": "v1", "label": "V", "focus": ["a"], "note": "n" }],
            "legend": { "mode": "all", "entries": {
                "frontend": { "label": "F", "visible": true }, "backend": { "label": "B" },
                "database": { "visible": false }, "cloud": { "label": "C" },
                "security": { "label": "S" }, "messagebus": { "label": "M" },
                "external": { "label": "E" } } },
            "viewBox": [1200.5, 800]
        },
        "layout": { "mode": "grid", "origin": [10, 20], "cols": 3, "gapX": 5,
                    "gapY": 6.5, "cellW": 140, "cellH": 70 },
        "components": [
            { "id": "a", "type": "cloud", "label": "A", "sublabel": "sa", "tag": "t",
              "icon": "moon", "brand": "rust",
              "sources": [{ "path": "p", "line": 1, "end_line": 2, "label": "l" }],
              "row": 0, "col": 1, "pos": [1.5, -2], "size": [120, 60] },
            { "id": "b", "type": "messagebus", "label": "B",
              "brand": { "url": "https://x.example/b.svg", "sha256": "f".repeat(64) } }
        ],
        "boundaries": [{ "kind": "security-group", "label": "SG", "wraps": ["a"], "pad": 12 }],
        "connections": [{
            "id": "ab", "from": "a", "to": "b", "label": "L", "variant": "emphasis",
            "fromSide": "right", "toSide": "left", "route": "orthogonal-h",
            "via": [[1, 2], [3, 4.5]], "labelAt": [5, 6], "labelDx": -1, "labelDy": 2.5,
            "labelSegment": 1, "width": 1.5
        }],
        "cards": [{ "dot": "violet", "title": "C", "items": ["i"] }]
    });
    let loaded = load_architecture_value(v.clone()).unwrap();
    let out: Value = serde_json::from_slice(&archalign_model::to_pretty_json(&loaded.doc)).unwrap();
    assert_eq!(out, v);
}
