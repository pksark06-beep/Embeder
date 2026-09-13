//! MCP round-trip tests (ignored by default: need `python` on PATH + the firmware
//! server + arm-none-eabi-gcc). Run with:
//!   cargo test -p embeder-mcp --test roundtrip -- --ignored

use embeder_core::*;
use embeder_mcp::{McpCompileOracle, McpDatasheet, McpSimOracle};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = mcp/ ; firmware/, servers/, datasheet/ are its siblings.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn firmware_dir() -> PathBuf {
    repo_root()
}

fn oracle() -> McpCompileOracle {
    let root = firmware_dir();
    let server = root.join("servers").join("firmware_server.py");
    McpCompileOracle::stm32f4(
        "python",
        server.to_string_lossy().to_string(),
        root.join("firmware").join("stm32-blink-uart"),
    )
}

fn read_main() -> String {
    std::fs::read_to_string(
        firmware_dir()
            .join("firmware")
            .join("stm32-blink-uart")
            .join("src")
            .join("main.c"),
    )
    .expect("read main.c")
}

#[test]
#[ignore]
fn mcp_compile_builds_real_elf() {
    let cc = oracle();
    if !cc.available() {
        eprintln!("skip: MCP firmware server / python unavailable");
        return;
    }
    let draft = FirmwareDraft::new(
        "stm32f4-discovery",
        "main.c",
        vec![("main.c".to_string(), read_main())],
    );
    let r = cc.compile(&draft);
    assert!(r.ok, "expected clean build over MCP; stderr:\n{}", r.stderr);
    let elf = r.artifact_path.expect("elf path");
    assert!(std::path::Path::new(&elf).exists(), "server should have produced {}", elf);
}

#[test]
#[ignore]
fn mcp_compile_error_round_trips() {
    let cc = oracle();
    if !cc.available() {
        eprintln!("skip: MCP firmware server / python unavailable");
        return;
    }
    let broken = format!("#error EMBEDER_INTENTIONAL\n{}", read_main());
    let draft = FirmwareDraft::new(
        "stm32f4-discovery",
        "main.c",
        vec![("main.c".to_string(), broken)],
    );
    let r = cc.compile(&draft);
    assert!(!r.ok, "broken source must not build");
    assert!(
        !r.errors().is_empty(),
        "diagnostics must survive the MCP round trip; stderr:\n{}",
        r.stderr
    );
}

fn datasheet() -> McpDatasheet {
    let root = repo_root();
    McpDatasheet::new(
        "python",
        root.join("servers").join("datasheet_server.py").to_string_lossy().to_string(),
        root.join("datasheet").join("data").join("stm32f4-mini.svd").to_string_lossy().to_string(),
    )
}

#[test]
#[ignore]
fn mcp_datasheet_agrees_with_hardware() {
    let ds = datasheet();
    if !ds.available() {
        eprintln!("skip: python / datasheet server unavailable");
        return;
    }
    let g = ds.register_map("USART2");
    assert!(g.found, "USART2 must be found: {}", g.note);
    assert_eq!(g.tier, Tier::Verified);
    assert!(g.citation.is_some());
    assert_eq!(g.base_address, 0x4000_4400);
    let cr1 = g.registers.iter().find(|(n, _)| n == "CR1").map(|(_, a)| *a);
    assert_eq!(cr1, Some(0x4000_440C), "CR1 abs addr over MCP must match the Rust crate + firmware");

    // A miss is refused, not fabricated.
    let miss = ds.register_map("SPI5");
    assert!(!miss.found);
    assert_eq!(miss.tier, Tier::OutOfScope);
}

#[test]
#[ignore]
fn mcp_full_loop_compile_and_sim_over_mcp() {
    let cc = oracle();
    let renode = std::env::var("EMBEDER_RENODE")
        .unwrap_or_else(|_| r"C:\Program Files\Renode\bin\Renode.exe".to_string());
    let sim = McpSimOracle::stm32f4(
        "python",
        repo_root().join("servers").join("simulation_server.py").to_string_lossy().to_string(),
        renode,
    );
    if !cc.available() || !sim.available() {
        eprintln!("skip: MCP servers / toolchains unavailable");
        return;
    }

    struct Heal {
        good: String,
    }
    impl Codegen for Heal {
        fn generate(&mut self, ctx: &LoopContext) -> FirmwareDraft {
            let src = if ctx.compile_diagnostics.is_empty() {
                format!("#error EMBEDER_FIRST_DRAFT_BUG\n{}", self.good)
            } else {
                self.good.clone()
            };
            FirmwareDraft::new("stm32f4-discovery", "main.c", vec![("main.c".to_string(), src)])
        }
    }

    let mut ledger =
        ProvenanceLedger::new(std::env::temp_dir().join("embeder-mcp-e2e.jsonl")).unwrap();
    let mut cg = Heal { good: read_main() };
    let o = run_loop("blink+uart over MCP", &mut cg, &cc, &sim, &mut ledger, &LoopConfig::default());

    assert!(matches!(o.state, LoopState::Verified), "full MCP loop must VERIFY");
    assert_eq!(o.attempts, 2, "one self-heal, then compiled + simulated over MCP");
}
