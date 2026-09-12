//! Real Renode simulation oracle (P1b). Availability probe is real; the boot/capture
//! wiring lands with the toolchain milestone (docs/P1-verification-loop.md §8). Until
//! then it degrades honestly rather than faking a pass.
#![allow(dead_code)]

use crate::oracle::{SimOracle, SimResult};
use crate::tiers::VerifiabilityBoundary;
use std::process::Command;

pub struct RenodeOracle {
    pub bin: String,
}

impl Default for RenodeOracle {
    fn default() -> Self {
        Self { bin: "renode".to_string() }
    }
}

impl RenodeOracle {
    pub fn available(&self) -> bool {
        Command::new(&self.bin).arg("--version").output().is_ok()
    }
}

impl SimOracle for RenodeOracle {
    fn name(&self) -> &str {
        "renode"
    }

    fn simulate(&self, _artifact_path: &str, _target: &str) -> SimResult {
        if !self.available() {
            return SimResult {
                ok: false,
                boundary: VerifiabilityBoundary::default(),
                observations: Vec::new(),
                log: format!("{} not installed", self.bin),
                engine: format!("{} (absent)", self.bin),
                engine_available: false,
                fault: Some("engine_absent".to_string()),
            };
        }

        // TODO(P1b): emit a .resc, boot the ELF on an STM32F4 platform, capture USART2
        // + GPIO, populate observations and the boundary, and assert on captured UART.
        SimResult {
            ok: false,
            boundary: VerifiabilityBoundary::default(),
            observations: Vec::new(),
            log: "renode adapter not yet wired".to_string(),
            engine: "renode (unwired)".to_string(),
            engine_available: true,
            fault: Some("not_implemented".to_string()),
        }
    }
}
