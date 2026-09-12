//! The oracle contracts. A firmware draft is only ever advanced by a deterministic
//! oracle: a compiler (CompileOracle) and a simulator (SimOracle). Fixtures and real
//! toolchains are interchangeable implementations of these traits.

use crate::tiers::VerifiabilityBoundary;

/// One structured compiler diagnostic (never free text — this is what makes the
/// self-healing loop possible; the diagnostics feed the next codegen attempt).
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: String, // "error" | "warning" | "note"
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub col: Option<u32>,
    pub raw: String,
}

impl Diagnostic {
    /// Compact single-line form fed back into the model's context on retry.
    pub fn as_context(&self) -> String {
        let mut loc = String::new();
        if let Some(f) = &self.file {
            loc.push_str(f);
            if let Some(l) = self.line {
                loc.push_str(&format!(":{}", l));
            }
            if let Some(c) = self.col {
                loc.push_str(&format!(":{}", c));
            }
            loc.push_str(": ");
        }
        format!("{}{}: {}", loc, self.severity, self.message)
    }
}

/// A candidate firmware, produced by codegen (the model stand-in).
#[derive(Debug, Clone)]
pub struct FirmwareDraft {
    pub target: String,             // e.g. "stm32f4-discovery"
    pub entry: String,              // e.g. "main.c"
    pub files: Vec<(String, String)>, // (relative path, source)
}

/// Result of a compile attempt.
#[derive(Debug, Clone)]
pub struct CompileResult {
    pub ok: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub stdout: String,
    pub stderr: String,
    pub artifact_path: Option<String>, // Some(ELF) only when ok
    pub toolchain: String,             // named version, for provenance
    pub toolchain_available: bool,
}

impl CompileResult {
    pub fn errors(&self) -> Vec<&Diagnostic> {
        self.diagnostics.iter().filter(|d| d.severity == "error").collect()
    }
}

/// Result of a simulation run.
#[derive(Debug, Clone)]
pub struct SimResult {
    pub ok: bool,
    pub boundary: VerifiabilityBoundary,
    pub observations: Vec<(String, String)>, // e.g. ("uart", "Hello...")
    pub log: String,
    pub engine: String, // named version, for provenance
    pub engine_available: bool,
    pub fault: Option<String>,
}

/// A deterministic build oracle.
pub trait CompileOracle {
    fn name(&self) -> &str;
    fn compile(&self, draft: &FirmwareDraft) -> CompileResult;
}

/// A deterministic simulation oracle.
pub trait SimOracle {
    fn name(&self) -> &str;
    fn simulate(&self, artifact_path: &str, target: &str) -> SimResult;
}
