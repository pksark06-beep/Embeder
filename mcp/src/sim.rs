// SPDX-License-Identifier: MPL-2.0
//! `McpSimOracle` — a `SimOracle` that runs Renode via the Simulation MCP server.
//! The server executes Renode and captures the UART; the Core assigns the tier and
//! the verifiability boundary.

use crate::client::McpClient;
use embeder_core::{SimOracle, SimResult, VerifiabilityBoundary};
use serde_json::json;

pub struct McpSimOracle {
    pub program: String,
    pub args: Vec<String>,
    pub renode_bin: String,
    pub platform: String,
    pub uart: String,
    pub run_for_secs: String,
    pub expect: String,
    pub timeout_secs: u64,
}

impl McpSimOracle {
    pub fn stm32f4(
        program: impl Into<String>,
        server_script: impl Into<String>,
        renode_bin: impl Into<String>,
    ) -> Self {
        Self {
            program: program.into(),
            args: vec![server_script.into()],
            renode_bin: renode_bin.into(),
            platform: "@platforms/boards/stm32f4_discovery.repl".to_string(),
            uart: "sysbus.usart2".to_string(),
            run_for_secs: "0.5".to_string(),
            expect: "Hello from Embeder".to_string(),
            timeout_secs: 180,
        }
    }

    pub fn available(&self) -> bool {
        McpClient::spawn_without_model_secrets(&self.program, &self.args).is_ok()
    }

    fn boundary() -> VerifiabilityBoundary {
        VerifiabilityBoundary {
            verified: vec![
                "cpu_boot".into(),
                "register_init".into(),
                "gpio_toggle".into(),
                "uart_tx".into(),
            ],
            stubbed: vec!["adc_values".into()],
            not_modeled: vec!["wifi".into(), "ble".into(), "rf".into(), "real_analog".into()],
        }
    }
}

impl SimOracle for McpSimOracle {
    fn name(&self) -> &str {
        "mcp:simulation"
    }

    fn simulate(&self, artifact_path: &str, _target: &str) -> SimResult {
        let mut client = match McpClient::spawn_without_model_secrets(&self.program, &self.args) {
            Ok(c) => c,
            Err(e) => return sim_fault(&format!("cannot start MCP simulation server: {}", e), "engine_error", false),
        };
        let args = json!({
            "renode_bin": self.renode_bin,
            "platform": self.platform,
            "uart": self.uart,
            "elf_path": artifact_path,
            "run_for_secs": self.run_for_secs,
            "expect": self.expect,
            "timeout_secs": self.timeout_secs,
        });
        match client.call_tool("run_simulation", args) {
            Ok(res) => {
                if !res.get("available").and_then(|v| v.as_bool()).unwrap_or(true) {
                    return sim_fault("renode not available", "engine_absent", false);
                }
                let ok = res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                let timed_out = res.get("timed_out").and_then(|v| v.as_bool()).unwrap_or(false);
                let uart = res.get("uart").and_then(|v| v.as_str()).unwrap_or("");
                let bytes = res.get("uart_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                let first = uart.lines().next().unwrap_or("").trim().to_string();
                let fault = if timed_out {
                    Some("timeout".to_string())
                } else if ok {
                    None
                } else {
                    Some("expected_uart_not_found".to_string())
                };
                SimResult {
                    ok,
                    boundary: Self::boundary(),
                    observations: vec![("uart".into(), first), ("uart_bytes".into(), bytes.to_string())],
                    log: res.get("log").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    engine: "renode (mcp)".into(),
                    engine_available: true,
                    fault,
                }
            }
            Err(e) => sim_fault(&format!("MCP run_simulation failed: {}", e), "engine_error", false),
        }
    }
}

fn sim_fault(msg: &str, kind: &str, available: bool) -> SimResult {
    SimResult {
        ok: false,
        boundary: VerifiabilityBoundary::default(),
        observations: Vec::new(),
        log: msg.to_string(),
        engine: "renode (mcp, error)".into(),
        engine_available: available,
        fault: Some(kind.to_string()),
    }
}
