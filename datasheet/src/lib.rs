//! Embeder datasheet grounding (ADR-0002).
//!
//! Register/field facts come from an authoritative *structured* source (CMSIS-SVD),
//! carry a citation, and are refused rather than fabricated on a miss. This is the
//! P2 layer that keeps firmware synthesis from hallucinating register addresses.

pub mod grounded;
pub mod model;
pub mod query;
pub mod svd;

pub use grounded::GroundedCodegen;
pub use model::*;
pub use query::*;
pub use svd::parse_svd;
