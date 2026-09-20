// SPDX-License-Identifier: MPL-2.0
//! `McpCompileOracle` — a `CompileOracle` that runs the build via the Firmware MCP
//! server instead of shelling out directly. The server executes the toolchain; the
//! Core parses stderr with its own tested parser, keeping interpretation in one place.

use crate::client::McpClient;
use embeder_core::{parse_gcc_stderr, CompileOracle, CompileResult, Diagnostic, FirmwareDraft};
use serde_json::{json, Map, Value};
use std::path::Path;

pub struct McpCompileOracle {
    pub program: String,   // e.g. "python"
    pub args: Vec<String>, // e.g. ["servers/firmware_server.py"]
    pub cc: String,
    pub cpu_flags: Vec<String>,
    pub extra_flags: Vec<String>,
    pub support_sources: Vec<String>,
    pub linker_script: Option<String>,
}

impl McpCompileOracle {
    /// STM32F4 profile served over MCP. `firmware_dir` points at
    /// `firmware/stm32-blink-uart`.
    pub fn stm32f4(
        program: impl Into<String>,
        server_script: impl Into<String>,
        firmware_dir: impl AsRef<Path>,
    ) -> Self {
        let d = firmware_dir.as_ref();
        Self {
            program: program.into(),
            args: vec![server_script.into()],
            cc: "arm-none-eabi-gcc".to_string(),
            cpu_flags: vec!["-mcpu=cortex-m4".into(), "-mthumb".into()],
            extra_flags: vec![
                "-nostdlib".into(),
                "-ffreestanding".into(),
                "-ffunction-sections".into(),
                "-fdata-sections".into(),
                "-Wall".into(),
                "-O0".into(),
                "-g".into(),
                "-Wl,--gc-sections".into(),
            ],
            support_sources: vec![d.join("src").join("startup.c").to_string_lossy().into_owned()],
            linker_script: Some(d.join("link").join("stm32f4.ld").to_string_lossy().into_owned()),
        }
    }

    /// True if the server can be started and completes the handshake.
    pub fn available(&self) -> bool {
        McpClient::spawn_without_model_secrets(&self.program, &self.args).is_ok()
    }
}

impl CompileOracle for McpCompileOracle {
    fn name(&self) -> &str {
        "mcp:firmware"
    }

    fn compile(&self, draft: &FirmwareDraft) -> CompileResult {
        let mut files = Map::new();
        for (name, content) in &draft.files {
            files.insert(name.clone(), Value::String(content.clone()));
        }
        let arguments = json!({
            "cc": self.cc,
            "cpu_flags": self.cpu_flags,
            "extra_flags": self.extra_flags,
            "support_sources": self.support_sources,
            "linker_script": self.linker_script,
            "target": draft.target,
            "entry": draft.entry,
            "files": Value::Object(files),
        });

        let mut client = match McpClient::spawn_without_model_secrets(&self.program, &self.args) {
            Ok(c) => c,
            Err(e) => return err_result(&format!("cannot start MCP firmware server: {}", e)),
        };

        match client.call_tool("compile_firmware", arguments) {
            Ok(res) => {
                let stderr = res.get("stderr").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let stdout = res.get("stdout").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let ok = res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                let artifact_path = res
                    .get("artifact_path")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let toolchain = res
                    .get("toolchain")
                    .and_then(|v| v.as_str())
                    .unwrap_or("arm-none-eabi-gcc (mcp)")
                    .to_string();
                let toolchain_available = res.get("available").and_then(|v| v.as_bool()).unwrap_or(true);
                let diagnostics = parse_gcc_stderr(&stderr);
                CompileResult {
                    ok,
                    diagnostics,
                    stdout,
                    stderr,
                    artifact_path,
                    toolchain,
                    toolchain_available,
                }
            }
            Err(e) => err_result(&format!("MCP compile_firmware failed: {}", e)),
        }
    }
}

fn err_result(msg: &str) -> CompileResult {
    CompileResult {
        ok: false,
        diagnostics: vec![Diagnostic {
            severity: "error".into(),
            message: msg.to_string(),
            file: None,
            line: None,
            col: None,
            raw: String::new(),
        }],
        stdout: String::new(),
        stderr: String::new(),
        artifact_path: None,
        toolchain: "mcp:firmware (error)".into(),
        toolchain_available: false,
    }
}
