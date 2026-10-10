//! `settings.json` values and consents (spec Appendix B, FR-STO-05, FR-GEN-13, FR-SEC-03).

use std::path::PathBuf;

use archalign_agent::Budgets;
use serde::{Deserialize, Serialize};

/// The default model (Appendix C, Contract).
pub const DEFAULT_MODEL: &str = "openai/gpt-6.1-sol";

/// User settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// File format version.
    pub version: u32,
    /// litellm `provider/model`.
    pub model: String,
    /// Passed through unchanged when set; unset by default.
    pub reasoning_effort: Option<String>,
    /// Run caps.
    pub budgets: Budgets,
    /// Show dotfiles in the tree.
    pub show_hidden_files: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            model: DEFAULT_MODEL.to_string(),
            reasoning_effort: None,
            budgets: Budgets::default(),
            show_hidden_files: false,
        }
    }
}

impl Settings {
    /// The provider prefix of the model (`openai` when none).
    pub fn provider(&self) -> String {
        self.model
            .split_once('/')
            .map(|(p, _)| p)
            .unwrap_or("openai")
            .to_string()
    }

    /// The model name without the provider prefix.
    pub fn model_name(&self) -> String {
        self.model
            .split_once('/')
            .map(|(_, m)| m)
            .unwrap_or(&self.model)
            .to_string()
    }
}

/// Consent to send repository data to a provider (FR-SEC-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consent {
    /// Canonical repository root.
    pub repo: PathBuf,
    /// Provider prefix.
    pub provider: String,
    /// Model name at the time of consent.
    pub model: String,
    /// RFC 3339 time.
    pub granted_at: String,
}

/// The Settings modal's editable copy (FR-STO-06): text fields as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsDraft {
    /// litellm `provider/model`.
    pub model: String,
    /// Empty means unset.
    pub reasoning_effort: String,
    /// Budget fields in [`BUDGET_FIELDS`] order.
    pub budgets: [String; 6],
    /// Show dotfiles in the tree.
    pub show_hidden_files: bool,
}

/// Labels of the budget fields, in [`SettingsDraft::budgets`] order.
pub const BUDGET_FIELDS: [&str; 6] = [
    "Max turns",
    "Max input tokens (whole run)",
    "Max completion tokens per turn",
    "Wall clock (s)",
    "Per-request timeout (s)",
    "Repairs",
];

impl SettingsDraft {
    /// A draft of `s`.
    pub fn of(s: &Settings) -> Self {
        let b = &s.budgets;
        Self {
            model: s.model.clone(),
            reasoning_effort: s.reasoning_effort.clone().unwrap_or_default(),
            budgets: [
                b.max_turns.to_string(),
                b.max_input_tokens.to_string(),
                b.max_completion_tokens.to_string(),
                b.wall_clock_s.to_string(),
                b.request_timeout_s.to_string(),
                b.max_repairs.to_string(),
            ],
            show_hidden_files: s.show_hidden_files,
        }
    }

    /// The settings this draft describes, or the first problem.
    pub fn apply(&self, base: &Settings) -> Result<Settings, String> {
        let model = self.model.trim();
        if model.is_empty() {
            return Err("Model can't be empty".into());
        }
        if archalign_agent::litellm::provider_of(model).is_none() {
            return Err(format!("Unknown provider in model `{model}`"));
        }
        fn num<T: std::str::FromStr + PartialOrd + Default>(
            label: &str,
            s: &str,
            allow_zero: bool,
        ) -> Result<T, String> {
            let v: T = s
                .trim()
                .parse()
                .map_err(|_| format!("{label} must be a whole number"))?;
            if !allow_zero && v <= T::default() {
                return Err(format!("{label} must be greater than 0"));
            }
            Ok(v)
        }
        let [t, i, c, w, r, rep] = &self.budgets;
        let budgets = Budgets {
            max_turns: num(BUDGET_FIELDS[0], t, false)?,
            max_input_tokens: num(BUDGET_FIELDS[1], i, false)?,
            max_completion_tokens: num(BUDGET_FIELDS[2], c, false)?,
            wall_clock_s: num(BUDGET_FIELDS[3], w, false)?,
            request_timeout_s: num(BUDGET_FIELDS[4], r, false)?,
            max_repairs: num(BUDGET_FIELDS[5], rep, true)?,
        };
        let effort = self.reasoning_effort.trim();
        Ok(Settings {
            model: model.to_string(),
            reasoning_effort: (!effort.is_empty()).then(|| effort.to_string()),
            budgets,
            show_hidden_files: self.show_hidden_files,
            ..base.clone()
        })
    }
}

#[cfg(test)]
mod draft_tests {
    use super::*;

    #[test]
    fn a_draft_round_trips_and_validates() {
        let s = Settings::default();
        assert_eq!(SettingsDraft::of(&s).apply(&s).unwrap(), s);
        let mut d = SettingsDraft::of(&s);
        d.reasoning_effort = " high ".into();
        d.budgets[0] = "12".into();
        d.budgets[5] = "0".into();
        let out = d.apply(&s).unwrap();
        assert_eq!(out.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(out.budgets.max_turns, 12);
        assert_eq!(out.budgets.max_repairs, 0);
        d.budgets[0] = "0".into();
        assert_eq!(d.apply(&s).unwrap_err(), "Max turns must be greater than 0");
        d.budgets[0] = "ten".into();
        assert_eq!(d.apply(&s).unwrap_err(), "Max turns must be a whole number");
        d.budgets[0] = "10".into();
        d.model = "nope/x".into();
        assert!(d.apply(&s).unwrap_err().starts_with("Unknown provider"));
    }
}
