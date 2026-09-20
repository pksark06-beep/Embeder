// SPDX-License-Identifier: MPL-2.0
//! The closed verification loop (ADR-0007) — the thesis in code.
//!
//! DRAFT --compile--> BUILT --simulate--> VERIFIED, with bounded retry and honest
//! degradation. Only the oracles move the state forward; the model never
//! self-certifies. On exhausted budget the loop HALTs and returns the structured
//! failure rather than guessing.

use crate::json;
use crate::oracle::{
    CompileOracle, CompileResult, Diagnostic, FirmwareDraft, SimOracle, SimResult, SourceRef,
};
use crate::provenance::ProvenanceLedger;
use crate::tiers::{Tier, VerifiabilityBoundary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopState {
    Draft,
    Built,
    Verified,
    Halted,
}

/// Context handed to codegen on each attempt. Prior compiler diagnostics are the
/// self-healing signal.
#[derive(Debug, Clone, Default)]
pub struct LoopContext {
    pub goal: String,
    pub attempt: u32,
    pub compile_diagnostics: Vec<Diagnostic>,
    pub sim_fault: Option<String>,
}

/// The model stand-in: given context, produce a firmware draft.
pub trait Codegen {
    fn generate(&mut self, ctx: &LoopContext) -> FirmwareDraft;
}

pub struct LoopConfig {
    pub max_attempts: u32,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self { max_attempts: 3 }
    }
}

#[derive(Debug, Clone)]
pub struct LoopOutcome {
    pub state: LoopState,
    pub tier: Option<Tier>,
    pub boundary: Option<VerifiabilityBoundary>,
    pub attempts: u32,
    pub diagnostics: Vec<Diagnostic>,
    pub provenance_ids: Vec<String>,
    pub artifact_path: Option<String>,
    pub observations: Vec<(String, String)>,
    pub citations: Vec<SourceRef>,
}

/// A firmware is VERIFIED only when a deterministic oracle proved modeled behavior:
/// the build is clean AND simulation passed AND it actually exercised something.
pub fn assign_tier(c: &CompileResult, s: &SimResult) -> Tier {
    if c.ok && s.ok && !s.boundary.verified.is_empty() {
        Tier::Verified
    } else {
        Tier::Advisory
    }
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub fn run_loop(
    goal: &str,
    codegen: &mut dyn Codegen,
    compile_oracle: &dyn CompileOracle,
    sim_oracle: &dyn SimOracle,
    ledger: &mut ProvenanceLedger,
    cfg: &LoopConfig,
) -> LoopOutcome {
    let mut prov_ids: Vec<String> = Vec::new();
    let mut ctx = LoopContext {
        goal: goal.to_string(),
        attempt: 0,
        compile_diagnostics: Vec::new(),
        sim_fault: None,
    };
    let mut last_diags: Vec<Diagnostic> = Vec::new();

    for attempt in 1..=cfg.max_attempts {
        ctx.attempt = attempt;
        let draft = codegen.generate(&ctx);

        // --- ground: record the authoritative facts this draft was built on ---
        if !draft.citations.is_empty() {
            let cites: Vec<String> = draft.citations.iter().map(|c| c.as_line()).collect();
            let gid = ledger.record(
                "model",
                "ground",
                &format!("{{\"goal\":{},\"attempt\":{}}}", json::quote(goal), attempt),
                &format!("{{\"citations\":{}}}", json::str_array(&cites)),
                None,
                "datasheet",
                "{}",
            );
            prov_ids.push(gid);
        }

        // --- compile ---
        let cres = compile_oracle.compile(&draft);
        let files_list: Vec<String> = draft.files.iter().map(|(p, _)| p.clone()).collect();
        let inputs = format!(
            "{{\"goal\":{},\"attempt\":{},\"files\":{}}}",
            json::quote(goal),
            attempt,
            json::str_array(&files_list)
        );
        let err_ctx: Vec<String> = cres.errors().iter().map(|d| d.as_context()).collect();
        let outputs = format!(
            "{{\"ok\":{},\"errors\":{}}}",
            cres.ok,
            json::str_array(&err_ctx)
        );
        let detail = format!("{{\"stderr\":{}}}", json::quote(&truncate(&cres.stderr, 2000)));
        let id = ledger.record("oracle", "compile", &inputs, &outputs, None, &cres.toolchain, &detail);
        prov_ids.push(id);

        last_diags = cres.diagnostics.clone();

        if !cres.ok {
            ctx.compile_diagnostics = cres.errors().into_iter().cloned().collect();
            if attempt == cfg.max_attempts {
                return LoopOutcome {
                    state: LoopState::Halted,
                    tier: None,
                    boundary: None,
                    attempts: attempt,
                    diagnostics: cres.errors().into_iter().cloned().collect(),
                    provenance_ids: prov_ids,
                    artifact_path: None,
                    observations: Vec::new(),
                    citations: draft.citations.clone(),
                };
            }
            continue; // self-heal: regenerate with the diagnostics in context
        }

        // --- BUILT -> simulate ---
        let artifact = cres.artifact_path.clone().unwrap_or_default();
        let sres = sim_oracle.simulate(&artifact, &draft.target);
        let sim_inputs = format!(
            "{{\"artifact\":{},\"target\":{}}}",
            json::quote(&artifact),
            json::quote(&draft.target)
        );
        let fault_json = match &sres.fault {
            Some(f) => json::quote(f),
            None => "null".to_string(),
        };
        let sim_outputs = format!(
            "{{\"ok\":{},\"boundary\":{},\"fault\":{}}}",
            sres.ok,
            sres.boundary.to_json(),
            fault_json
        );
        let sid = ledger.record("oracle", "simulate", &sim_inputs, &sim_outputs, None, &sres.engine, "{}");
        prov_ids.push(sid);

        if sres.ok {
            let tier = assign_tier(&cres, &sres);
            let vinputs = format!("{{\"goal\":{}}}", json::quote(goal));
            let voutputs = format!(
                "{{\"tier\":{},\"boundary\":{}}}",
                json::quote(tier.as_str()),
                sres.boundary.to_json()
            );
            let overs = format!("{} + {}", cres.toolchain, sres.engine);
            let vid = ledger.record("user", "verify", &vinputs, &voutputs, Some(tier.as_str()), &overs, "{}");
            prov_ids.push(vid);
            return LoopOutcome {
                state: LoopState::Verified,
                tier: Some(tier),
                boundary: Some(sres.boundary.clone()),
                attempts: attempt,
                diagnostics: Vec::new(),
                provenance_ids: prov_ids,
                artifact_path: cres.artifact_path.clone(),
                observations: sres.observations.clone(),
                citations: draft.citations.clone(),
            };
        } else {
            ctx.sim_fault = sres.fault.clone();
            if attempt == cfg.max_attempts {
                return LoopOutcome {
                    state: LoopState::Halted,
                    tier: None,
                    boundary: Some(sres.boundary.clone()),
                    attempts: attempt,
                    diagnostics: last_diags.clone(),
                    provenance_ids: prov_ids,
                    artifact_path: None,
                    observations: Vec::new(),
                    citations: draft.citations.clone(),
                };
            }
            continue;
        }
    }

    LoopOutcome {
        state: LoopState::Halted,
        tier: None,
        boundary: None,
        attempts: cfg.max_attempts,
        diagnostics: last_diags,
        provenance_ids: prov_ids,
        artifact_path: None,
        observations: Vec::new(),
        citations: Vec::new(),
    }
}
