// SPDX-License-Identifier: MPL-2.0
//! Confidence tiers and the verifiability boundary — the vocabulary of the whole
//! system (SYSTEM_SPEC §2, §3). The Core is the sole authority for assigning these.

use crate::json;

/// A single confidence tier. The system never silently promotes a tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// A deterministic oracle proved it. Reproducible.
    Verified,
    /// Mechanically correct, but a human must confirm intent/fitness.
    Assisted,
    /// Best-effort model output with no oracle behind it.
    Advisory,
    /// Not supported; the system refuses rather than guesses.
    OutOfScope,
}

impl Tier {
    pub fn symbol(&self) -> &'static str {
        match self {
            Tier::Verified => "[OK]",
            Tier::Assisted => "[~]",
            Tier::Advisory => "[!]",
            Tier::OutOfScope => "[X]",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Tier::Verified => "verified",
            Tier::Assisted => "assisted",
            Tier::Advisory => "advisory",
            Tier::OutOfScope => "out_of_scope",
        }
    }
}

/// What a simulation actually exercised — attached to every VERIFIED result so an
/// unmodeled aspect (RF, analog) can never be presented as verified (ADR-0003).
#[derive(Debug, Clone, Default)]
pub struct VerifiabilityBoundary {
    pub verified: Vec<String>,
    pub stubbed: Vec<String>,
    pub not_modeled: Vec<String>,
}

impl VerifiabilityBoundary {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"verified\":{},\"stubbed\":{},\"not_modeled\":{}}}",
            json::str_array(&self.verified),
            json::str_array(&self.stubbed),
            json::str_array(&self.not_modeled),
        )
    }
}
