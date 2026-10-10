//! The `submit_diagram` gate (FR-GEN-06, FR-VAL-05/06, ADR 0027).

use std::collections::HashSet;

use archalign_model::citations::check_citations;
use archalign_model::{AppOwned, ArchitectureDoc, Issue, Profile, check_submission};
use archalign_repo::Sandbox;
use archalign_repo::citations::RepoProbe;
use serde_json::{Value, json};

/// The gate's verdict.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Accepted: the document plus non-blocking findings (e.g. modified files).
    Accepted(Box<ArchitectureDoc>, Vec<Issue>),
    /// Rejected with errors, in check order.
    Rejected(Vec<Issue>),
}

/// The gate's result, with the dropped-field pointers for `fields_dropped`.
#[derive(Debug, Clone, PartialEq)]
pub struct GateResult {
    /// Dropped model-supplied fields.
    pub dropped: Vec<String>,
    /// Verdict.
    pub verdict: Verdict,
}

/// Runs the Submission profile: app-owned and dropped fields, schema, serde, semantic
/// checks, then citations through the sandbox (deny-listed paths are errors).
pub fn check(
    value: Value,
    app: &AppOwned,
    sandbox: &Sandbox,
    dirty: &HashSet<String>,
) -> GateResult {
    let check = check_submission(value, app);
    let verdict = match check.outcome {
        Err(errors) => Verdict::Rejected(errors),
        Ok(doc) => {
            let probe = RepoProbe::new(sandbox);
            let issues = check_citations(&doc, &probe, Profile::Submission, dirty);
            let (errors, warnings): (Vec<_>, Vec<_>) =
                issues.into_iter().partition(Issue::is_error);
            if errors.is_empty() {
                Verdict::Accepted(Box::new(doc), warnings)
            } else {
                Verdict::Rejected(errors)
            }
        }
    };
    GateResult {
        dropped: check.dropped,
        verdict,
    }
}

/// The tool result text for a rejection: every error with its pointer.
pub fn rejection_text(errors: &[Issue], attempt: u32, max_repairs: u32) -> String {
    let list: Vec<Value> = errors
        .iter()
        .map(|e| json!({ "pointer": e.pointer, "message": e.message }))
        .collect();
    // After rejection `attempt` (1-based), this many further submissions may still be
    // rejected before the run fails.
    let left = max_repairs.saturating_add(1).saturating_sub(attempt);
    json!({
        "status": "rejected",
        "errors": list,
        "repairs_left": left,
        "hint": "Fix exactly these errors and submit the complete diagram again."
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejection_text_saturates_at_the_largest_repair_budget() {
        let text = super::rejection_text(&[], 1, u32::MAX);
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["repairs_left"], u32::MAX - 1);
    }
}
