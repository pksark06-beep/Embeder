// SPDX-License-Identifier: MPL-2.0
//! `McpDatasheet` — grounded register lookups via the Datasheet MCP server. Mirrors
//! the `datasheet` crate's contract over the wire: a hit is Verified + cited, a miss
//! is OutOfScope and never fabricated.

use crate::client::McpClient;
use embeder_core::Tier;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct GroundedRegisterMap {
    pub found: bool,
    pub peripheral: String,
    pub base_address: u64,
    pub registers: Vec<(String, u64)>, // (name, absolute_address)
    pub tier: Tier,
    pub citation: Option<String>,
    pub note: String,
}

pub struct McpDatasheet {
    pub program: String,
    pub args: Vec<String>,
    pub svd_path: String,
}

impl McpDatasheet {
    pub fn new(
        program: impl Into<String>,
        server_script: impl Into<String>,
        svd_path: impl Into<String>,
    ) -> Self {
        Self {
            program: program.into(),
            args: vec![server_script.into()],
            svd_path: svd_path.into(),
        }
    }

    pub fn available(&self) -> bool {
        McpClient::spawn_without_model_secrets(&self.program, &self.args).is_ok()
    }

    pub fn register_map(&self, peripheral: &str) -> GroundedRegisterMap {
        let refuse = |note: String| GroundedRegisterMap {
            found: false,
            peripheral: peripheral.to_string(),
            base_address: 0,
            registers: Vec::new(),
            tier: Tier::OutOfScope,
            citation: None,
            note,
        };

        let mut client = match McpClient::spawn_without_model_secrets(&self.program, &self.args) {
            Ok(c) => c,
            Err(e) => return refuse(format!("cannot start MCP datasheet server: {}", e)),
        };
        let res = match client.call_tool(
            "register_map",
            json!({"svd_path": self.svd_path, "peripheral": peripheral}),
        ) {
            Ok(r) => r,
            Err(e) => return refuse(format!("MCP register_map failed: {}", e)),
        };

        if !res.get("found").and_then(|v| v.as_bool()).unwrap_or(false) {
            return refuse(
                res.get("note").and_then(|v| v.as_str()).unwrap_or("not found").to_string(),
            );
        }

        let base_address = res.get("base_address").and_then(|v| v.as_u64()).unwrap_or(0);
        let registers = res
            .get("registers")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        Some((
                            r.get("name")?.as_str()?.to_string(),
                            r.get("absolute_address")?.as_u64()?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let source = res.get("source").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let name = res.get("name").and_then(|v| v.as_str()).unwrap_or(peripheral).to_string();

        GroundedRegisterMap {
            found: true,
            peripheral: name.clone(),
            base_address,
            registers,
            tier: Tier::Verified,
            citation: Some(format!("{}::device/peripherals/{}", source, name)),
            note: "CMSIS-SVD structured source (via MCP)".to_string(),
        }
    }
}
