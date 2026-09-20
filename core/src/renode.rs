// SPDX-License-Identifier: MPL-2.0
//! Real Renode simulation oracle (P1b): boots the ELF on an STM32F4 platform
//! headlessly, captures USART2 to a file, and passes iff the expected banner is
//! actually transmitted. Always attaches an honest verifiability boundary — the
//! STM32_UART model is deterministic, but analog is stubbed and RF is out of scope.

use crate::oracle::{SimOracle, SimResult};
use crate::process_env::remove_model_secrets;
use crate::tiers::VerifiabilityBoundary;
use std::fs;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Per-process sequence so concurrent simulations never share a directory.
static SIM_SEQ: AtomicU64 = AtomicU64::new(0);

pub struct RenodeOracle {
    pub bin: String,
    pub platform: String,      // Renode @-path to the .repl
    pub uart: String,          // e.g. "sysbus.usart2"
    pub run_for_secs: String,  // virtual time to execute
    pub expect: String,        // banner that must appear on the UART
    pub timeout_secs: u64,     // wall-clock guard on the Renode subprocess
}

impl Default for RenodeOracle {
    fn default() -> Self {
        Self {
            bin: "renode".to_string(),
            platform: "@platforms/boards/stm32f4_discovery.repl".to_string(),
            uart: "sysbus.usart2".to_string(),
            run_for_secs: "0.5".to_string(),
            expect: "Hello from Embeder".to_string(),
            // Generous: a safety net against a true hang, not a performance bound.
            // Renode cold-start under load can take tens of seconds; never false-fire.
            timeout_secs: 180,
        }
    }
}

impl RenodeOracle {
    pub fn stm32f4(bin: impl Into<String>) -> Self {
        Self { bin: bin.into(), ..Default::default() }
    }

    pub fn available(&self) -> bool {
        let mut probe = Command::new(&self.bin);
        remove_model_secrets(&mut probe);
        probe.arg("--version").output().is_ok()
    }

    fn full_boundary() -> VerifiabilityBoundary {
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

fn fwd(p: &str) -> String {
    p.replace('\\', "/")
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

struct RunResult {
    code: Option<i32>,
    output: String,
    timed_out: bool,
}

/// Run a command with a wall-clock timeout, draining stdout/stderr on background
/// threads so a chatty child can't deadlock on a full pipe. Kills the child on timeout.
fn run_with_timeout(mut cmd: Command, timeout: Duration) -> std::io::Result<RunResult> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let mut out_pipe = child.stdout.take().expect("piped stdout");
    let mut err_pipe = child.stderr.take().expect("piped stderr");

    let (tx_o, rx_o) = mpsc::channel();
    let (tx_e, rx_e) = mpsc::channel();
    thread::spawn(move || {
        let mut s = String::new();
        let _ = out_pipe.read_to_string(&mut s);
        let _ = tx_o.send(s);
    });
    thread::spawn(move || {
        let mut s = String::new();
        let _ = err_pipe.read_to_string(&mut s);
        let _ = tx_e.send(s);
    });

    let start = Instant::now();
    let code;
    let mut timed_out = false;
    loop {
        match child.try_wait()? {
            Some(status) => {
                code = status.code();
                break;
            }
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    code = None;
                    timed_out = true;
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    let out = rx_o.recv().unwrap_or_default();
    let err = rx_e.recv().unwrap_or_default();
    Ok(RunResult { code, output: format!("{}{}", out, err), timed_out })
}

impl SimOracle for RenodeOracle {
    fn name(&self) -> &str {
        "renode"
    }

    fn simulate(&self, artifact_path: &str, _target: &str) -> SimResult {
        if !self.available() {
            return fault(&self.bin, "engine_absent", &format!("{} not available", self.bin), false);
        }

        let seq = SIM_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("embeder-sim-{}-{}", std::process::id(), seq));
        let _ = fs::remove_dir_all(&dir);
        if fs::create_dir_all(&dir).is_err() {
            return fault(&self.bin, "engine_error", "cannot create sim dir", true);
        }
        let uart_out = dir.join("uart.txt");
        let resc = dir.join("run.resc");

        let script = format!(
            "mach create \"embeder\"\n\
             machine LoadPlatformDescription {platform}\n\
             sysbus LoadELF @{elf}\n\
             {uart} CreateFileBackend @{uartfile}\n\
             emulation RunFor \"{secs}\"\n\
             {uart} CloseFileBackend @{uartfile}\n\
             quit\n",
            platform = self.platform,
            elf = fwd(artifact_path),
            uart = self.uart,
            uartfile = fwd(&uart_out.to_string_lossy()),
            secs = self.run_for_secs,
        );
        if fs::write(&resc, &script).is_err() {
            return fault(&self.bin, "engine_error", "cannot write resc", true);
        }

        let mut cmd = Command::new(&self.bin);
        remove_model_secrets(&mut cmd);
        cmd.args(["--console", "--disable-gui", "--plain"]).arg(&resc);

        match run_with_timeout(cmd, Duration::from_secs(self.timeout_secs)) {
            Ok(run) => {
                let uart_text = fs::read_to_string(&uart_out).unwrap_or_default();
                let ok = !run.timed_out && uart_text.contains(&self.expect);
                let first_line = uart_text.lines().next().unwrap_or("").trim().to_string();
                let fault = if run.timed_out {
                    Some("timeout".to_string())
                } else if ok {
                    None
                } else {
                    Some("expected_uart_not_found".to_string())
                };
                SimResult {
                    ok,
                    boundary: Self::full_boundary(),
                    observations: vec![
                        ("uart".into(), first_line),
                        ("uart_bytes".into(), uart_text.len().to_string()),
                    ],
                    log: format!(
                        "renode exit={:?} timed_out={}; uart_captured={} bytes; expect={:?} -> pass={}\n--- renode output (truncated) ---\n{}",
                        run.code,
                        run.timed_out,
                        uart_text.len(),
                        self.expect,
                        ok,
                        truncate(&run.output, 900)
                    ),
                    engine: "renode 1.16.0".into(),
                    engine_available: true,
                    fault,
                }
            }
            Err(e) => fault(&self.bin, "engine_error", &format!("failed to run renode: {}", e), false),
        }
    }
}

fn fault(bin: &str, kind: &str, msg: &str, available: bool) -> SimResult {
    SimResult {
        ok: false,
        boundary: VerifiabilityBoundary::default(),
        observations: Vec::new(),
        log: msg.to_string(),
        engine: format!("{} ({})", bin, if available { "error" } else { "absent" }),
        engine_available: available,
        fault: Some(kind.to_string()),
    }
}
