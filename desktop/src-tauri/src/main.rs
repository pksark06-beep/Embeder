//! Embeder native desktop shell (ADR-0006). The window loads `desktop/dist`; the
//! frontend calls `invoke("run_verification")`, which drives the Core's grounded
//! verification loop and returns the outcome + provenance as JSON.
//!
//! Presentation only lives in the webview — all execution stays in the Core.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use embeder_core::{
    run_loop, FixtureCompileOracle, FixtureSimOracle, LoopConfig, LoopState, ProvenanceLedger,
};
use embeder_datasheet::{DatasheetIndex, GroundedCodegen};
use serde_json::{json, Value};
use std::path::PathBuf;

fn svd_path() -> PathBuf {
    // <root>/desktop/src-tauri -> <root>/datasheet/data/stm32f4-mini.svd
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join("datasheet").join("data").join("stm32f4-mini.svd"))
        .unwrap_or_default()
}

#[tauri::command]
fn run_verification() -> Value {
    let idx = match DatasheetIndex::from_svd_file(svd_path()) {
        Ok(i) => i,
        Err(e) => return json!({"error": format!("cannot load SVD: {e}")}),
    };
    let mut codegen = GroundedCodegen::new(&idx);
    let ledger_path = std::env::temp_dir().join("embeder-tauri-prov.jsonl");
    let _ = std::fs::remove_file(&ledger_path);
    let mut ledger = match ProvenanceLedger::new(&ledger_path) {
        Ok(l) => l,
        Err(e) => return json!({"error": format!("ledger: {e}")}),
    };

    let outcome = run_loop(
        "Blink an LED and print over UART on STM32",
        &mut codegen,
        &FixtureCompileOracle,
        &FixtureSimOracle,
        &mut ledger,
        &LoopConfig::default(),
    );

    let tier = outcome.tier.map(|t| json!({"symbol": t.symbol(), "name": t.as_str()}));
    let boundary = outcome.boundary.as_ref().map(|b| {
        json!({"verified": b.verified, "stubbed": b.stubbed, "not_modeled": b.not_modeled})
    });
    let citations: Vec<Value> = outcome
        .citations
        .iter()
        .map(|c| json!({"source": c.source, "locator": c.locator, "summary": c.summary}))
        .collect();
    let observations: Vec<Value> = outcome
        .observations
        .iter()
        .map(|(k, v)| json!({"key": k, "value": v}))
        .collect();
    let provenance: Vec<Value> = ledger
        .records
        .iter()
        .map(|r| {
            json!({"id": r.id, "ts": r.ts, "actor": r.actor, "tool": r.tool,
                   "tier": r.tier, "oracle": r.oracle_version})
        })
        .collect();

    json!({
        "goal": "Blink an LED and print over UART on STM32",
        "state": match outcome.state {
            LoopState::Verified => "verified",
            LoopState::Halted => "halted",
            LoopState::Built => "built",
            LoopState::Draft => "draft",
        },
        "tier": tier,
        "attempts": outcome.attempts,
        "self_healed": outcome.attempts > 1,
        "boundary": boundary,
        "citations": citations,
        "observations": observations,
        "artifact": outcome.artifact_path,
        "provenance": provenance,
    })
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![run_verification])
        .run(tauri::generate_context!())
        .expect("error while running Embeder");
}
