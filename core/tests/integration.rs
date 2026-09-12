//! Integration tests for the verification loop (ADR-0007 invariants).

use embeder_core::*;

struct AlwaysOk;
impl Codegen for AlwaysOk {
    fn generate(&mut self, _ctx: &LoopContext) -> FirmwareDraft {
        FirmwareDraft {
            target: "stm32f4-discovery".to_string(),
            entry: "main.c".to_string(),
            files: vec![("main.c".to_string(), "int main(){return 0;}".to_string())],
        }
    }
}

struct AlwaysBug;
impl Codegen for AlwaysBug {
    fn generate(&mut self, _ctx: &LoopContext) -> FirmwareDraft {
        FirmwareDraft {
            target: "stm32f4-discovery".to_string(),
            entry: "main.c".to_string(),
            files: vec![("main.c".to_string(), format!("int main(){{ {} return 0; }}", BUG_MARKER))],
        }
    }
}

struct BugThenHeal;
impl Codegen for BugThenHeal {
    fn generate(&mut self, ctx: &LoopContext) -> FirmwareDraft {
        let src = if ctx.compile_diagnostics.is_empty() {
            format!("int main(){{ {} return 0; }}", BUG_MARKER)
        } else {
            "int main(){return 0;}".to_string()
        };
        FirmwareDraft {
            target: "stm32f4-discovery".to_string(),
            entry: "main.c".to_string(),
            files: vec![("main.c".to_string(), src)],
        }
    }
}

fn tmp_ledger(tag: &str) -> ProvenanceLedger {
    let p = std::env::temp_dir().join(format!("embeder-test-{}-{}.jsonl", tag, std::process::id()));
    let _ = std::fs::remove_file(&p);
    ProvenanceLedger::new(p).unwrap()
}

#[test]
fn clean_build_verifies_in_one_attempt() {
    let mut l = tmp_ledger("clean");
    let mut cg = AlwaysOk;
    let o = run_loop("g", &mut cg, &FixtureCompileOracle, &FixtureSimOracle, &mut l, &LoopConfig::default());
    assert!(matches!(o.state, LoopState::Verified));
    assert_eq!(o.attempts, 1);
    assert_eq!(o.tier, Some(Tier::Verified));
    let b = o.boundary.expect("boundary present on verified");
    assert!(b.not_modeled.contains(&"rf".to_string()), "RF must be out of scope");
}

#[test]
fn self_heals_then_verifies() {
    let mut l = tmp_ledger("heal");
    let mut cg = BugThenHeal;
    let o = run_loop("g", &mut cg, &FixtureCompileOracle, &FixtureSimOracle, &mut l, &LoopConfig::default());
    assert!(matches!(o.state, LoopState::Verified));
    assert_eq!(o.attempts, 2, "one failed build, then a healed one");
}

#[test]
fn persistent_bug_halts_honestly() {
    let mut l = tmp_ledger("bug");
    let mut cg = AlwaysBug;
    let o = run_loop("g", &mut cg, &FixtureCompileOracle, &FixtureSimOracle, &mut l, &LoopConfig { max_attempts: 3 });
    assert!(matches!(o.state, LoopState::Halted));
    assert_eq!(o.attempts, 3);
    assert!(o.tier.is_none(), "a halted loop is never assigned a tier");
    assert!(!o.diagnostics.is_empty(), "halt returns the structured failure");
}

#[test]
fn provenance_is_appended_per_action() {
    let mut l = tmp_ledger("prov");
    let mut cg = BugThenHeal;
    let o = run_loop("g", &mut cg, &FixtureCompileOracle, &FixtureSimOracle, &mut l, &LoopConfig::default());
    // attempt 1: compile(fail); attempt 2: compile(ok) + simulate + verify = 4 records.
    assert_eq!(o.provenance_ids.len(), 4);
    assert_eq!(l.records.len(), 4);
}
