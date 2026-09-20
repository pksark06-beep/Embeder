// SPDX-License-Identifier: MPL-2.0
//! `LlmCodegen` — the BYOK model in the loop (the missing half of the thesis).
//!
//! It drafts firmware with a real large language model (over the Codegen MCP server),
//! but the model never certifies itself: the draft is handed to the same compile +
//! simulation oracles as any other `Codegen`, and only they can advance it to
//! VERIFIED. The model is *grounded* — the prompt carries the exact register
//! addresses from the CMSIS-SVD, and the citations attached to the draft are those
//! SVD facts, not anything the model asserted. On retry, the prior compiler
//! diagnostics (or a simulation fault) are fed back in, which is the self-heal signal.
//!
//! If the model is unavailable for any reason — no API key, server missing, network
//! or provider error — `LlmCodegen` falls back to a deterministic `Codegen`, so the
//! loop always closes and offline/CI runs never depend on a network.

use crate::client::McpClient;
use embeder_core::{Codegen, FirmwareDraft, LoopContext, SourceRef};
use serde_json::json;

/// What actually produced the last draft — surfaced so callers (and provenance) can
/// tell an LLM draft from a fallback, and name the provider/model that wrote it.
#[derive(Debug, Clone)]
pub struct LlmMeta {
    pub used_llm: bool,
    pub provider: String,
    pub model: String,
    pub note: String,
}

pub struct LlmCodegen<'a> {
    program: String,
    args: Vec<String>,
    target: String,
    entry: String,
    grounding_brief: String,
    citations: Vec<SourceRef>,
    provider: Option<String>,
    model: Option<String>,
    timeout_secs: u64,
    fallback: Box<dyn Codegen + 'a>,
    /// Metadata about the most recent `generate` call.
    pub last: Option<LlmMeta>,
}

impl<'a> LlmCodegen<'a> {
    /// `program`/`server_script` spawn the Codegen MCP server (e.g. "python",
    /// "servers/codegen_server.py"). `grounding_brief` + `citations` come from the
    /// datasheet layer (`GroundedCodegen::grounding_brief`). `fallback` is used
    /// whenever the model can't be reached — pass the deterministic grounded codegen.
    pub fn new(
        program: impl Into<String>,
        server_script: impl Into<String>,
        grounding_brief: impl Into<String>,
        citations: Vec<SourceRef>,
        fallback: Box<dyn Codegen + 'a>,
    ) -> Self {
        Self {
            program: program.into(),
            args: vec![server_script.into()],
            target: "stm32f4-discovery".to_string(),
            entry: "main.c".to_string(),
            grounding_brief: grounding_brief.into(),
            citations,
            provider: None,
            model: None,
            timeout_secs: 90,
            fallback,
            last: None,
        }
    }

    pub fn target(mut self, t: impl Into<String>) -> Self {
        self.target = t.into();
        self
    }
    pub fn entry(mut self, e: impl Into<String>) -> Self {
        self.entry = e.into();
        self
    }
    /// Force a provider ("gemini" | "vercel" | "openai" | "none"); otherwise the
    /// server auto-detects from whichever key is present.
    pub fn provider(mut self, p: impl Into<String>) -> Self {
        self.provider = Some(p.into());
        self
    }
    pub fn model(mut self, m: impl Into<String>) -> Self {
        self.model = Some(m.into());
        self
    }

    /// True if the Codegen MCP server can be started and completes the handshake.
    /// (This says nothing about whether a *key* is configured — that is reported
    /// per-call so the loop can fall back gracefully.)
    pub fn available(&self) -> bool {
        McpClient::spawn(&self.program, &self.args).is_ok()
    }

    fn system_prompt(&self) -> String {
        format!(
            "You are a meticulous bare-metal embedded firmware engineer. Write a single \
             freestanding C translation unit for an STM32F4 (Cortex-M4), register-level, \
             no libc, no CMSIS headers, no external includes except <stdint.h>.\n\n\
             Hard rules:\n\
             - Output ONLY the C source for `{entry}` in one ```c fenced block. No prose.\n\
             - Provide `int main(void)`. Startup code and the linker script are supplied \
             separately; do not write a vector table or reset handler.\n\
             - Use ONLY the register addresses and bit positions given below. Never \
             invent an address.\n\
             - Acceptance is decided by a simulator, not by you: the firmware is VERIFIED \
             only if the exact text `Hello from Embeder\\r\\n` is transmitted on USART2, \
             and the PA5 LED is toggled. Configure GPIOA clock, USART2 clock, PA5 as \
             output, PA2 as alternate-function 7 (USART2 TX), set USART2_BRR, enable \
             USART2 (UE|TE), then poll TXE and write each byte of the banner to USART2_DR \
             in a loop, toggling PA5 each iteration.",
            entry = self.entry
        )
    }

    fn user_prompt(&self, ctx: &LoopContext) -> String {
        let mut p = format!(
            "Goal: {goal}\n\nAuthoritative grounded register facts:\n{brief}\n",
            goal = ctx.goal,
            brief = self.grounding_brief
        );
        if !ctx.compile_diagnostics.is_empty() {
            p.push_str(
                "\nYour previous draft FAILED TO COMPILE. Fix exactly these compiler \
                 diagnostics and return the corrected full file:\n",
            );
            for d in &ctx.compile_diagnostics {
                p.push_str("  - ");
                p.push_str(&d.as_context());
                p.push('\n');
            }
        }
        if let Some(f) = &ctx.sim_fault {
            p.push_str(&format!(
                "\nYour previous draft compiled but simulation did not observe the banner \
                 (fault: {f}). Make sure USART2 is fully initialized and every byte of \
                 `Hello from Embeder\\r\\n` is written to USART2_DR after TXE is set.\n"
            ));
        }
        p
    }

    /// Attempt an LLM draft; `None` means "fall back" (unavailable/no key/error).
    fn try_llm(&self, ctx: &LoopContext) -> (Option<FirmwareDraft>, LlmMeta) {
        let mut meta = LlmMeta {
            used_llm: false,
            provider: self.provider.clone().unwrap_or_default(),
            model: self.model.clone().unwrap_or_default(),
            note: String::new(),
        };

        let mut client = match McpClient::spawn(&self.program, &self.args) {
            Ok(c) => c,
            Err(e) => {
                meta.note = format!("codegen server unavailable: {e}");
                return (None, meta);
            }
        };

        let args = json!({
            "system": self.system_prompt(),
            "user": self.user_prompt(ctx),
            "provider": self.provider,
            "model": self.model,
            "timeout_secs": self.timeout_secs,
        });

        let res = match client.call_tool("generate_code", args) {
            Ok(v) => v,
            Err(e) => {
                meta.note = format!("generate_code call failed: {e}");
                return (None, meta);
            }
        };

        if let Some(p) = res.get("provider").and_then(|v| v.as_str()) {
            meta.provider = p.to_string();
        }
        if let Some(m) = res.get("model").and_then(|v| v.as_str()) {
            meta.model = m.to_string();
        }
        let available = res.get("available").and_then(|v| v.as_bool()).unwrap_or(false);
        let code = res.get("code").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();

        if !available || code.is_empty() {
            let err = res.get("error").and_then(|v| v.as_str()).unwrap_or("no code returned");
            meta.note = format!("model unavailable/empty ({err})");
            return (None, meta);
        }

        meta.used_llm = true;
        meta.note = "drafted by model".to_string();
        let draft = FirmwareDraft::new(
            self.target.clone(),
            self.entry.clone(),
            vec![(self.entry.clone(), code)],
        )
        .with_citations(self.citations.clone());
        (Some(draft), meta)
    }
}

impl Codegen for LlmCodegen<'_> {
    fn generate(&mut self, ctx: &LoopContext) -> FirmwareDraft {
        let (draft, meta) = self.try_llm(ctx);
        match draft {
            Some(d) => {
                self.last = Some(meta);
                d
            }
            None => {
                let mut meta = meta;
                meta.note = format!("{} -> deterministic fallback", meta.note);
                self.last = Some(meta);
                // The fallback is grounded too; it attaches its own citations.
                self.fallback.generate(ctx)
            }
        }
    }
}
