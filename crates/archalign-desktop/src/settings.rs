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
