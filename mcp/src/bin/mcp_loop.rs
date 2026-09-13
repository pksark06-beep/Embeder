//! `mcp_loop` — drive the verification loop with the compile step running over MCP
//! (Rust Core -> stdio JSON-RPC -> Python firmware server -> arm-none-eabi-gcc).
//!   (default)  MCP compile + fixture simulation
//!   --real     MCP compile + real Renode simulation

use embeder_core::*;
use embeder_mcp::McpCompileOracle;
use std::path::PathBuf;

struct RealCodegen {
    good: String,
}
impl RealCodegen {
    fn new(fw_dir: &std::path::Path) -> Self {
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

fn main() {
    let real = std::env::args().any(|a| a == "--real");
    let fw = PathBuf::from("firmware/stm32-blink-uart");
    let server = "servers/firmware_server.py".to_string();

    let cc = McpCompileOracle::stm32f4("python", server, &fw);
    if !cc.available() {
        eprintln!("MCP firmware server unavailable (need python on PATH + servers/firmware_server.py).");
        std::process::exit(2);
    }

    let mut ledger = ProvenanceLedger::new(".embeder/provenance-mcp.jsonl").expect("open ledger");
    let mut codegen = RealCodegen::new(&fw);

    let outcome = if real {
        let renode = std::env::var("EMBEDER_RENODE")
            .unwrap_or_else(|_| r"C:\Program Files\Renode\bin\Renode.exe".to_string());
        let sim = RenodeOracle::stm32f4(renode);
        if !sim.available() {
            eprintln!("Renode not found — set EMBEDER_RENODE");
            std::process::exit(2);
        }
        run_loop(
            "Blink+UART via MCP compile + real Renode",
            &mut codegen,
            &cc,
            &sim,
            &mut ledger,
            &LoopConfig::default(),
        )
    } else {
        run_loop(
            "Blink+UART via MCP compile + fixture sim",
            &mut codegen,
            &cc,
            &FixtureSimOracle,
            &mut ledger,
            &LoopConfig::default(),
        )
    };

    println!("=== Embeder MCP loop (compile over MCP, ADR-0001) ===");
    println!("state     : {:?}", outcome.state);
    if let Some(t) = &outcome.tier {
        println!("tier      : {} {}", t.symbol(), t.as_str());
    }
    println!("attempts  : {}  (self-healed: {})", outcome.attempts, outcome.attempts > 1);
    if let Some(a) = &outcome.artifact_path {
        println!("artifact  : {}  (built by the MCP firmware server)", a);
    }
    for (k, v) in &outcome.observations {
        println!("observed  : {} = {}", k, v);
    }
    println!("provenance: {} entries -> .embeder/provenance-mcp.jsonl", outcome.provenance_ids.len());

    assert!(matches!(outcome.state, LoopState::Verified), "MCP loop did not reach VERIFIED");
    assert!(outcome.artifact_path.is_some(), "the MCP server should have produced an ELF");
    println!("\nOK: intent -> (MCP) compile -> self-heal -> simulate -> VERIFIED.");
}
