// SPDX-License-Identifier: MPL-2.0
//! `llm_loop` — the product path: a real BYOK model drafts the firmware, grounded on
//! CMSIS-SVD facts, and the same oracle chain verifies it. The model only proposes;
//! compile + simulation decide VERIFIED. With no API key it falls back to determin-
//! istic grounded synthesis, so this always runs.
//!   (default)  LLM draft + fixture oracles (runs anywhere, no toolchains)
//!   --real     LLM draft + MCP compile (arm-none-eabi-gcc) + MCP Renode simulation
//!
//! Bring your key: set GEMINI_API_KEY, AI_GATEWAY_API_KEY (Vercel), or OPENAI_API_KEY
//! (or drop it in a repo-root `.env`). Override with EMBEDER_LLM_PROVIDER / _MODEL.

use embeder_core::*;
use embeder_datasheet::{DatasheetIndex, GroundedCodegen};
use embeder_mcp::{LlmCodegen, McpCompileOracle, McpSimOracle};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn main() {
    let real = std::env::args().any(|a| a == "--real");
    let root = repo_root();

    let svd = root.join("datasheet").join("data").join("stm32f4-mini.svd");
    let idx = DatasheetIndex::from_svd_file(&svd).expect("load svd");

    // The datasheet layer supplies both the grounding brief (handed to the model) and
    // the deterministic fallback (used when no key is configured).
    let grounded = GroundedCodegen::new(&idx);
    let (brief, cites) = grounded.grounding_brief();
    let n_cites = cites.len();

    let codegen_server = root.join("servers").join("codegen_server.py");
    let mut codegen = LlmCodegen::new(
        "python",
        codegen_server.to_string_lossy().to_string(),
        brief,
        cites,
        Box::new(grounded),
    );
    if let Ok(p) = std::env::var("EMBEDER_LLM_PROVIDER") {
        codegen = codegen.provider(p);
    }
    if let Ok(m) = std::env::var("EMBEDER_LLM_MODEL") {
        codegen = codegen.model(m);
    }
    if !codegen.available() {
        eprintln!("Codegen MCP server unavailable (need python on PATH + servers/codegen_server.py).");
        std::process::exit(2);
    }

    let mut ledger = ProvenanceLedger::new(".embeder/provenance-llm.jsonl").expect("open ledger");

    let goal = "Blink PA5 and print 'Hello from Embeder' over USART2 on STM32F4";
    let outcome = if real {
        let fw = root.join("firmware").join("stm32-blink-uart");
        let cc = McpCompileOracle::stm32f4(
            "python",
            root.join("servers").join("firmware_server.py").to_string_lossy().to_string(),
            &fw,
        );
        let renode = std::env::var("EMBEDER_RENODE")
            .unwrap_or_else(|_| r"C:\Program Files\Renode\bin\Renode.exe".to_string());
        let sim = McpSimOracle::stm32f4(
            "python",
            root.join("servers").join("simulation_server.py").to_string_lossy().to_string(),
            renode,
        );
        if !cc.available() || !sim.available() {
            eprintln!("MCP compile/sim servers unavailable (need python + arm-none-eabi-gcc + Renode).");
            std::process::exit(2);
        }
        run_loop(goal, &mut codegen, &cc, &sim, &mut ledger, &LoopConfig::default())
    } else {
        run_loop(goal, &mut codegen, &FixtureCompileOracle, &FixtureSimOracle, &mut ledger, &LoopConfig::default())
    };

    println!("=== Embeder LLM loop (BYOK model in the verification loop) ===");
    if let Some(m) = &codegen.last {
        if m.used_llm {
            println!("drafted by: {} / {}  (model proposed; oracles verified)", m.provider, m.model);
        } else {
            println!("drafted by: deterministic grounded synthesis  ({})", m.note);
        }
    }
    println!("mode      : {}", if real { "REAL (arm-gcc + Renode over MCP)" } else { "fixtures" });
    println!("state     : {:?}", outcome.state);
    if let Some(t) = &outcome.tier {
        println!("tier      : {} {}", t.symbol(), t.as_str());
    }
    println!("attempts  : {}  (self-healed: {})", outcome.attempts, outcome.attempts > 1);
    println!("grounded on {} cited SVD facts", n_cites);
    if let Some(b) = &outcome.boundary {
        println!("verified  : {}", b.verified.join(", "));
    }
    for (k, v) in &outcome.observations {
        println!("observed  : {} = {}", k, v);
    }
    println!("provenance: {} entries -> .embeder/provenance-llm.jsonl", outcome.provenance_ids.len());

    match outcome.state {
        LoopState::Verified => {
            println!("\nOK: intent -> (model) draft -> grounded -> compiled -> simulated -> VERIFIED.");
        }
        _ => {
            eprintln!("\nHALTED: the model's draft was not verified (this is the loop working, not failing).");
            std::process::exit(1);
        }
    }
}
