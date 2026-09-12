//! In-process fixture oracles. Deterministic stand-ins for `arm-none-eabi-gcc` and
//! Renode so the loop is provable today without toolchains installed. The fixture
//! compiler emits authentic gcc-style stderr and parses it with the *real* parser,
//! so the loop consumes genuinely structured diagnostics.

use crate::gcc::parse_gcc_stderr;
use crate::oracle::{CompileOracle, CompileResult, FirmwareDraft, SimOracle, SimResult};
use crate::tiers::VerifiabilityBoundary;

/// A source containing this marker "fails to compile" deterministically.
pub const BUG_MARKER: &str = "/* BUG */";

pub struct FixtureCompileOracle;

impl CompileOracle for FixtureCompileOracle {
    fn name(&self) -> &str {
        "fixture-gcc"
    }

    fn compile(&self, draft: &FirmwareDraft) -> CompileResult {
        let src = draft
            .files
            .iter()
            .find(|(p, _)| p == &draft.entry)
            .map(|(_, s)| s.clone())
            .unwrap_or_default();

        if src.contains(BUG_MARKER) {
            let line_no = src
                .lines()
                .position(|l| l.contains(BUG_MARKER))
                .map(|i| i + 1)
                .unwrap_or(1);
            let stderr = format!(
                "{f}:{n}:5: error: expected ';' before '}}' token\n{f}:{n}:5: note: to match this '{{'\n",
                f = draft.entry,
                n = line_no
            );
            let diagnostics = parse_gcc_stderr(&stderr);
            return CompileResult {
                ok: false,
                diagnostics,
                stdout: String::new(),
                stderr,
                artifact_path: None,
                toolchain: "fixture-gcc 0.0".to_string(),
                toolchain_available: true,
            };
        }

        CompileResult {
            ok: true,
            diagnostics: Vec::new(),
            stdout: String::new(),
            stderr: String::new(),
            artifact_path: Some(format!("/virtual/{}.elf", draft.target)),
            toolchain: "fixture-gcc 0.0".to_string(),
            toolchain_available: true,
        }
    }
}

pub struct FixtureSimOracle;

impl SimOracle for FixtureSimOracle {
    fn name(&self) -> &str {
        "fixture-renode"
    }

    fn simulate(&self, _artifact_path: &str, _target: &str) -> SimResult {
        // Honest boundary for an STM32 blink+UART: digital behavior is modeled;
        // analog is stubbed; wireless/RF is out of scope entirely.
        let boundary = VerifiabilityBoundary {
            verified: vec![
                "cpu_boot".to_string(),
                "register_init".to_string(),
                "gpio_toggle".to_string(),
                "uart_tx".to_string(),
            ],
            stubbed: vec!["adc_values".to_string()],
            not_modeled: vec![
                "wifi".to_string(),
                "ble".to_string(),
                "rf".to_string(),
                "real_analog".to_string(),
            ],
        };
        SimResult {
            ok: true,
            boundary,
            observations: vec![
                ("gpio_toggles".to_string(), "5".to_string()),
                ("uart".to_string(), "Hello from Embeder".to_string()),
            ],
            log: "fixture: booted ELF; PA5 toggled 5x; USART2 TX captured".to_string(),
            engine: "fixture-renode 0.0".to_string(),
            engine_available: true,
            fault: None,
        }
    }
}
