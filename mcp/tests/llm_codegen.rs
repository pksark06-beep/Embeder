//! Tests for the BYOK model-in-the-loop. The default test needs neither python nor a
//! network: it proves the *fallback contract* — if the model can't be reached, the
//! loop still gets a grounded draft. The ignored test additionally exercises the real
//! Codegen MCP server (python), still without a network, by forcing `provider=none`.
//!   cargo test -p embeder-mcp --test llm_codegen
//!   cargo test -p embeder-mcp --test llm_codegen -- --ignored

use embeder_core::{Codegen, FirmwareDraft, LoopContext, SourceRef};
use embeder_mcp::LlmCodegen;
use std::path::PathBuf;

/// A stand-in deterministic fallback with a recognizable target + citation.
struct Stub;
impl Codegen for Stub {
    fn generate(&mut self, _ctx: &LoopContext) -> FirmwareDraft {
        FirmwareDraft::new(
            "stub-target",
            "main.c",
            vec![("main.c".to_string(), "int main(void){return 0;}".to_string())],
        )
        .with_citations(vec![SourceRef::new("test.svd", "device/x", "X @ 0x0")])
    }
}

#[test]
fn falls_back_when_the_model_cannot_be_reached() {
    // A program that cannot be spawned forces the "unavailable" path with no network.
    let mut cg = LlmCodegen::new(
        "embeder-nonexistent-binary-xyz",
        "unused.py",
        "grounding brief",
        vec![SourceRef::new("stm32f4-mini.svd", "loc", "summary")],
        Box::new(Stub),
    );
    let ctx = LoopContext { goal: "blink".to_string(), attempt: 1, ..Default::default() };
    let draft = cg.generate(&ctx);

    assert_eq!(draft.target, "stub-target", "must fall back to the deterministic codegen");
    assert_eq!(draft.citations.len(), 1, "fallback attaches its own grounded citations");
    assert_eq!(draft.citations[0].source, "test.svd");

    let meta = cg.last.as_ref().expect("metadata recorded");
    assert!(!meta.used_llm, "no model was reachable");
    assert!(meta.note.contains("fallback"), "note should record the fallback: {}", meta.note);
}

#[test]
#[ignore]
fn codegen_server_stays_offline_when_disabled() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let server = root.join("servers").join("codegen_server.py");

    // provider=none: the server must report unavailable WITHOUT touching the network,
    // regardless of any keys in the developer's environment.
    let mut cg = LlmCodegen::new(
        "python",
        server.to_string_lossy().to_string(),
        "grounding brief",
        vec![SourceRef::new("stm32f4-mini.svd", "loc", "summary")],
        Box::new(Stub),
    )
    .provider("none");

    if !cg.available() {
        eprintln!("skip: python / codegen server unavailable");
        return;
    }

    let ctx = LoopContext { goal: "blink".to_string(), attempt: 1, ..Default::default() };
    let draft = cg.generate(&ctx);

    let meta = cg.last.as_ref().expect("metadata recorded");
    assert!(!meta.used_llm, "provider=none must never call a model");
    assert_eq!(draft.target, "stub-target", "must fall back to the deterministic codegen");
}
