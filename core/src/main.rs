//! `embeder-demo` — headless proof of the verification loop (P1a).
//!
//! A scripted codegen (LLM stand-in) first ships a firmware with a compile bug, then
//! self-heals once the structured diagnostic is in context. Uses fixture oracles so
//! it runs with no toolchains installed. Exit code is non-zero if the loop fails to
//! reach VERIFIED — this is runnable proof, not a print statement.

use embeder_core::*;

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

struct ScriptedCodegen;

impl Codegen for ScriptedCodegen {
    fn generate(&mut self, ctx: &LoopContext) -> FirmwareDraft {
        let src = if ctx.compile_diagnostics.is_empty() {
            // First attempt: ship a bug.
            BLINK_UART_OK.replace("toggle_led();", &format!("toggle_led()  {}", BUG_MARKER))
        } else {
            // Self-heal using the compiler diagnostic that is now in context.
            BLINK_UART_OK.to_string()
        };
        FirmwareDraft {
            target: "stm32f4-discovery".to_string(),
            entry: "main.c".to_string(),
            files: vec![("main.c".to_string(), src)],
        }
    }
}

fn main() {
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

    println!("=== Embeder P1a verification loop (fixture oracles) ===");
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
    println!(
        "provenance : {} entries -> .embeder/provenance.jsonl",
        outcome.provenance_ids.len()
    );

    assert!(
        matches!(outcome.state, LoopState::Verified),
        "loop did not reach VERIFIED"
    );
    assert_eq!(outcome.tier, Some(Tier::Verified), "final tier must be Verified");
    assert_eq!(outcome.attempts, 2, "expected exactly one self-heal iteration");

    println!("\nOK: intent -> compiled -> (self-heal) -> simulated -> VERIFIED, with provenance.");
}
