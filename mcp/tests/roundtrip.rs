//! MCP round-trip tests (ignored by default: need `python` on PATH + the firmware
//! server + arm-none-eabi-gcc). Run with:
//!   cargo test -p embeder-mcp --test roundtrip -- --ignored

use embeder_core::*;
use embeder_mcp::McpCompileOracle;
use std::path::PathBuf;

fn firmware_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = mcp/ ; firmware/ and servers/ are its siblings.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
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
