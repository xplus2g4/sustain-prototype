//! Run budgets (FR-GEN-07, ADR 0028; Appendix C, Contract defaults).

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Caps for one agent run. The defaults are contractual; Settings may change them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Budgets {
    /// Model turns (40).
    pub max_turns: u32,
    /// Input tokens summed over the run (3,000,000).
    pub max_input_tokens: u64,
    /// `max_completion_tokens` per turn (32,000).
    pub max_completion_tokens: u32,
    /// Overall run time in seconds (900 = 15 min).
    pub wall_clock_s: u64,
    /// Per-request timeout in seconds (300).
    pub request_timeout_s: u64,
    /// Rejected submissions the model may repair (3; the 4th rejection fails the run).
    pub max_repairs: u32,
}

impl Default for Budgets {
    fn default() -> Self {
        Self {
            max_turns: 40,
            max_input_tokens: 3_000_000,
            max_completion_tokens: 32_000,
            wall_clock_s: 900,
            request_timeout_s: 300,
            max_repairs: 3,
        }
    }
}

impl Budgets {
    /// Overall run time.
    pub fn wall_clock(&self) -> Duration {
        Duration::from_secs(self.wall_clock_s)
    }

    /// Per-request timeout.
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout_s)
    }
}

/// Which cap ended a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cap {
    /// Model turns.
    Turns,
    /// Input tokens.
    InputTokens,
    /// Overall run time.
    WallClock,
    /// Per-request timeout (after retries).
    RequestTimeout,
    /// Repairs.
    Repairs,
}

impl Cap {
    /// The message naming the cap (FR-GEN-07).
    pub fn message(self, b: &Budgets) -> String {
        match self {
            Cap::Turns => format!("Turn budget exhausted ({} turns)", b.max_turns),
            Cap::InputTokens => format!(
                "Input-token budget exhausted ({} tokens)",
                b.max_input_tokens
            ),
            Cap::WallClock => format!("Wall-clock budget exhausted ({} s)", b.wall_clock_s),
            Cap::RequestTimeout => format!(
                "Per-request timeout ({} s) exceeded on every retry",
                b.request_timeout_s
            ),
            Cap::Repairs => format!(
                "Submission rejected {} times (repair budget {})",
                b.max_repairs + 1,
                b.max_repairs
            ),
        }
    }
}
