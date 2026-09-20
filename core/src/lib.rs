// SPDX-License-Identifier: MPL-2.0
//! Embeder Core — the headless verification-loop engine (ADR-0006, ADR-0007).
//!
//! The Core proposes nothing itself: it drives deterministic oracles (a compiler, a
//! simulator) and only lets their results move a firmware draft through
//! DRAFT -> BUILT -> VERIFIED. Everything is recorded to a provenance ledger, and
//! every VERIFIED result carries an honest verifiability boundary.
//!
//! Oracles sit behind traits ([`oracle::CompileOracle`], [`oracle::SimOracle`]) so the
//! loop is testable with in-process fixtures today and wired to real toolchains
//! (`arm-none-eabi-gcc`, Renode) — via the Python MCP servers (ADR-0001) — later.

pub mod json;
pub mod tiers;
pub mod oracle;
pub mod gcc;
pub mod provenance;
pub mod engine;
pub mod fixtures;
pub mod armgcc;
pub mod renode;
pub mod process_env;

pub use armgcc::ArmGccOracle;
pub use engine::*;
pub use fixtures::*;
pub use gcc::parse_gcc_stderr;
pub use oracle::*;
pub use provenance::*;
pub use renode::RenodeOracle;
pub use tiers::*;
