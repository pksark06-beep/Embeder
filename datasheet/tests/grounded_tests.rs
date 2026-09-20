// SPDX-License-Identifier: MPL-2.0
//! Grounding-the-loop tests (P2 -> P1): synthesis really uses SVD addresses, and the
//! citations flow through to the loop outcome.

use embeder_core::*;
use embeder_datasheet::{DatasheetIndex, GroundedCodegen};
use std::path::PathBuf;

fn index() -> DatasheetIndex {
    let svd = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("stm32f4-mini.svd");
    DatasheetIndex::from_svd_file(&svd).expect("load svd")
}

#[test]
fn synthesis_uses_svd_addresses_and_cites_them() {
    let idx = index();
    let cg = GroundedCodegen::new(&idx);
    let (src, cites) = cg.synthesize();

    // The CR1 absolute address (0x4000440c) came from the SVD, not a literal.
    assert!(src.contains("0x4000440c"), "generated code must use the grounded CR1 address");
    assert!(src.contains("Hello from Embeder"));
    assert!(cites.len() >= 9, "each grounded register should be cited");
    assert!(cites.iter().all(|c| c.source.ends_with(".svd")));
    assert!(cites.iter().any(|c| c.locator.contains("USART2/registers/CR1")));
}

#[test]
fn grounded_loop_verifies_and_carries_citations() {
    let idx = index();
    let mut cg = GroundedCodegen::new(&idx);
    let mut ledger =
        ProvenanceLedger::new(std::env::temp_dir().join("embeder-grounded-test.jsonl")).unwrap();

    let o = run_loop(
        "grounded blink+uart",
        &mut cg,
        &FixtureCompileOracle,
        &FixtureSimOracle,
        &mut ledger,
        &LoopConfig::default(),
    );

    assert!(matches!(o.state, LoopState::Verified));
    assert!(
        !o.citations.is_empty(),
        "a verified grounded outcome must carry its citations"
    );
    // The ledger got a dedicated `ground` record in addition to compile/sim/verify.
    assert!(o.provenance_ids.len() >= 4);
}
