//! P1b real-toolchain tests. Ignored by default because they require
//! `arm-none-eabi-gcc` on PATH. Run explicitly with:
//!   cargo test --test real_toolchain -- --ignored

use embeder_core::armgcc::ArmGccOracle;
use embeder_core::*;

fn firmware_dir() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR = core/ ; firmware/ is its sibling.
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("firmware")
        .join("stm32-blink-uart")
}

fn read_main() -> String {
    std::fs::read_to_string(firmware_dir().join("src").join("main.c")).expect("read main.c")
}

fn draft(src: String) -> FirmwareDraft {
    FirmwareDraft::new("stm32f4-discovery", "main.c", vec![("main.c".into(), src)])
}

#[test]
#[ignore]
fn real_compile_and_link_produces_elf() {
    let oracle = ArmGccOracle::stm32f4(firmware_dir());
    assert!(oracle.available(), "arm-none-eabi-gcc must be on PATH");

    let r = oracle.compile(&draft(read_main()));
    assert!(r.ok, "expected a clean build; stderr:\n{}", r.stderr);
    let elf = r.artifact_path.expect("elf path present on success");
    assert!(
        std::path::Path::new(&elf).exists(),
        "ELF should exist at {}",
        elf
    );
}

#[test]
#[ignore]
fn real_compile_error_is_parsed_from_gcc() {
    let oracle = ArmGccOracle::stm32f4(firmware_dir());
    assert!(oracle.available(), "arm-none-eabi-gcc must be on PATH");

    // Inject a guaranteed, unambiguous compile error at the top of the file.
    let broken = format!("#error EMBEDER_INTENTIONAL\n{}", read_main());
    let r = oracle.compile(&draft(broken));

    assert!(!r.ok, "broken source must not build");
    assert!(
        !r.errors().is_empty(),
        "real gcc stderr must yield parsed diagnostics; stderr:\n{}",
        r.stderr
    );
    assert!(
        r.errors()
            .iter()
            .any(|d| d.file.as_deref().map(|f| f.ends_with("main.c")).unwrap_or(false)),
        "a diagnostic should point at main.c; got: {:?}",
        r.errors().iter().map(|d| d.as_context()).collect::<Vec<_>>()
    );
}

fn renode_bin() -> String {
    std::env::var("EMBEDER_RENODE")
        .unwrap_or_else(|_| r"C:\Program Files\Renode\bin\Renode.exe".to_string())
}

/// End-to-end: real compile -> self-heal -> real Renode simulation -> VERIFIED.
struct HealCodegen {
    good: String,
}
impl Codegen for HealCodegen {
    fn generate(&mut self, ctx: &LoopContext) -> FirmwareDraft {
        let src = if ctx.compile_diagnostics.is_empty() {
            format!("#error EMBEDER_FIRST_DRAFT_BUG\n{}", self.good)
        } else {
            self.good.clone()
        };
        draft(src)
    }
}

#[test]
#[ignore]
fn real_full_loop_compile_sim_verifies() {
    let cc = ArmGccOracle::stm32f4(firmware_dir());
    let sim = RenodeOracle::stm32f4(renode_bin());
    if !cc.available() || !sim.available() {
        eprintln!("skip: arm-gcc and/or renode unavailable");
        return;
    }
    let mut ledger =
        ProvenanceLedger::new(std::env::temp_dir().join("embeder-e2e-prov.jsonl")).unwrap();
    let mut cg = HealCodegen { good: read_main() };
    let o = run_loop("blink+uart", &mut cg, &cc, &sim, &mut ledger, &LoopConfig::default());

    assert!(matches!(o.state, LoopState::Verified), "e2e must reach VERIFIED");
    assert_eq!(o.attempts, 2, "one failed build, then a healed + simulated one");
    assert_eq!(o.tier, Some(Tier::Verified));
    let uart = o
        .observations
        .iter()
        .find(|(k, _)| k == "uart")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    assert!(
        uart.contains("Hello from Embeder"),
        "USART2 capture should contain the banner; got {:?}",
        uart
    );
}
