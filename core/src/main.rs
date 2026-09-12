//! `embeder-demo` — headless proof of the verification loop.
//!
//!   (default)  fixture oracles, no toolchains needed (P1a).
//!   --real     real arm-none-eabi-gcc + Renode (P1b): intent -> REAL compile ->
//!              (self-heal) -> REAL Renode simulation -> VERIFIED.
//!
//! Exit code is non-zero if the loop fails to reach VERIFIED — runnable proof.

use embeder_core::*;
use std::path::{Path, PathBuf};

const BLINK_UART_OK: &str = r#"#include <stdint.h>
/* STM32 blink + UART (illustrative) */
int main(void) {
    for (;;) {
        toggle_led();
        uart_puts("Hello from Embeder");
    }
    return 0;
}
"#;

/// Fixture-mode model stand-in: ships a bug, then self-heals.
struct ScriptedCodegen;
impl Codegen for ScriptedCodegen {
    fn generate(&mut self, ctx: &LoopContext) -> FirmwareDraft {
        let src = if ctx.compile_diagnostics.is_empty() {
            BLINK_UART_OK.replace("toggle_led();", &format!("toggle_led()  {}", BUG_MARKER))
        } else {
            BLINK_UART_OK.to_string()
        };
        FirmwareDraft::new("stm32f4-discovery", "main.c", vec![("main.c".to_string(), src)])
    }
}

/// Real-mode model stand-in: reads the actual firmware, ships a broken first draft
/// (a `#error`), then self-heals to the real source once the diagnostic is in context.
struct RealCodegen {
    good: String,
}
impl RealCodegen {
    fn new(fw_dir: &Path) -> Self {
        let good = std::fs::read_to_string(fw_dir.join("src").join("main.c"))
            .expect("read firmware main.c");
        Self { good }
    }
}
impl Codegen for RealCodegen {
    fn generate(&mut self, ctx: &LoopContext) -> FirmwareDraft {
        let src = if ctx.compile_diagnostics.is_empty() {
            format!("#error EMBEDER_FIRST_DRAFT_BUG\n{}", self.good)
        } else {
            self.good.clone()
        };
        FirmwareDraft::new("stm32f4-discovery", "main.c", vec![("main.c".to_string(), src)])
    }
}

fn print_outcome(title: &str, outcome: &LoopOutcome) {
    println!("=== {} ===", title);
    println!("state      : {:?}", outcome.state);
    if let Some(t) = &outcome.tier {
        println!("tier       : {} {}", t.symbol(), t.as_str());
    }
    println!(
        "attempts   : {}  (self-healed: {})",
        outcome.attempts,
        outcome.attempts > 1
    );
    if let Some(b) = &outcome.boundary {
        println!("verified     : {}", b.verified.join(", "));
        println!("stubbed  [!] : {}", b.stubbed.join(", "));
        println!("not modeled  [X]: {}", b.not_modeled.join(", "));
    }
    if let Some(a) = &outcome.artifact_path {
        println!("artifact   : {}", a);
    }
    for (k, v) in &outcome.observations {
        println!("observed   : {} = {}", k, v);
    }
    if !outcome.citations.is_empty() {
        println!("grounded on: {} cited facts", outcome.citations.len());
        for c in &outcome.citations {
            println!("   - {}", c.as_line());
        }
    }
    println!(
        "provenance : {} entries",
        outcome.provenance_ids.len()
    );
}

fn run_fixture() {
    let mut ledger = ProvenanceLedger::new(".embeder/provenance.jsonl").expect("open ledger");
    let mut codegen = ScriptedCodegen;
    let outcome = run_loop(
        "Blink an LED and print over UART on STM32",
        &mut codegen,
        &FixtureCompileOracle,
        &FixtureSimOracle,
        &mut ledger,
        &LoopConfig::default(),
    );
    print_outcome("Embeder P1a verification loop (fixture oracles)", &outcome);

    assert!(matches!(outcome.state, LoopState::Verified), "loop did not reach VERIFIED");
    assert_eq!(outcome.tier, Some(Tier::Verified), "final tier must be Verified");
    assert_eq!(outcome.attempts, 2, "expected exactly one self-heal iteration");
    println!("\nOK: intent -> compiled -> (self-heal) -> simulated -> VERIFIED, with provenance.");
}

fn run_real() {
    let fw = PathBuf::from("firmware/stm32-blink-uart");
    let renode = std::env::var("EMBEDER_RENODE")
        .unwrap_or_else(|_| r"C:\Program Files\Renode\bin\Renode.exe".to_string());

    let cc = ArmGccOracle::stm32f4(&fw);
    let sim = RenodeOracle::stm32f4(renode);

    if !cc.available() {
        eprintln!("arm-none-eabi-gcc not on PATH — add the Arm toolchain bin to PATH.");
        std::process::exit(2);
    }
    if !sim.available() {
        eprintln!("Renode not found — set EMBEDER_RENODE to Renode.exe.");
        std::process::exit(2);
    }

    let mut ledger = ProvenanceLedger::new(".embeder/provenance-real.jsonl").expect("open ledger");
    let mut codegen = RealCodegen::new(&fw);
    let outcome = run_loop(
        "Blink an LED and print over UART on STM32 (REAL toolchain)",
        &mut codegen,
        &cc,
        &sim,
        &mut ledger,
        &LoopConfig::default(),
    );
    print_outcome(
        "Embeder P1b verification loop (REAL arm-none-eabi-gcc + Renode)",
        &outcome,
    );

    assert!(matches!(outcome.state, LoopState::Verified), "real loop did not reach VERIFIED");
    assert_eq!(outcome.tier, Some(Tier::Verified), "final tier must be Verified");
    println!("\nOK: intent -> REAL compile -> (self-heal) -> REAL Renode sim -> VERIFIED.");
}

fn main() {
    if std::env::args().any(|a| a == "--real") {
        run_real();
    } else {
        run_fixture();
    }
}
