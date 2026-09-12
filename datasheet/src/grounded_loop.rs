//! `grounded-loop` — the P2→P1 join: synthesize firmware from grounded SVD facts,
//! then drive the verification loop over it.
//!   (default)  fixture oracles (no toolchains needed)
//!   --real     real arm-none-eabi-gcc + Renode

use embeder_core::*;
use embeder_datasheet::{DatasheetIndex, GroundedCodegen};
use std::path::PathBuf;

fn main() {
    let real = std::env::args().any(|a| a == "--real");

    let svd = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("stm32f4-mini.svd");
    let idx = DatasheetIndex::from_svd_file(&svd).expect("load svd");
    let mut codegen = GroundedCodegen::new(&idx);

    let mut ledger =
        ProvenanceLedger::new(".embeder/provenance-grounded.jsonl").expect("open ledger");

    let outcome = if real {
        let fw = PathBuf::from("firmware/stm32-blink-uart");
        let renode = std::env::var("EMBEDER_RENODE")
            .unwrap_or_else(|_| r"C:\Program Files\Renode\bin\Renode.exe".to_string());
        let cc = ArmGccOracle::stm32f4(&fw);
        let sim = RenodeOracle::stm32f4(renode);
        if !cc.available() {
            eprintln!("arm-none-eabi-gcc not on PATH");
            std::process::exit(2);
        }
        if !sim.available() {
            eprintln!("Renode not found — set EMBEDER_RENODE");
            std::process::exit(2);
        }
        run_loop(
            "Grounded blink+UART on STM32 (REAL toolchain)",
            &mut codegen,
            &cc,
            &sim,
            &mut ledger,
            &LoopConfig::default(),
        )
    } else {
        run_loop(
            "Grounded blink+UART on STM32 (fixtures)",
            &mut codegen,
            &FixtureCompileOracle,
            &FixtureSimOracle,
            &mut ledger,
            &LoopConfig::default(),
        )
    };

    println!("=== Embeder grounded loop (P2 -> P1) ===");
    println!("state     : {:?}", outcome.state);
    if let Some(t) = &outcome.tier {
        println!("tier      : {} {}", t.symbol(), t.as_str());
    }
    println!("attempts  : {}", outcome.attempts);
    println!("grounded on {} cited facts:", outcome.citations.len());
    for c in &outcome.citations {
        println!("   - {}", c.as_line());
    }
    if let Some(b) = &outcome.boundary {
        println!("verified  : {}", b.verified.join(", "));
    }
    for (k, v) in &outcome.observations {
        println!("observed  : {} = {}", k, v);
    }
    println!(
        "provenance: {} entries -> .embeder/provenance-grounded.jsonl",
        outcome.provenance_ids.len()
    );

    assert!(matches!(outcome.state, LoopState::Verified), "grounded loop did not VERIFY");
    println!("\nOK: grounded synthesis -> VERIFIED (register addresses sourced from the datasheet).");
}
