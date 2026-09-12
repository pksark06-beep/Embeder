//! Real `arm-none-eabi-gcc` compile oracle (P1b). Compiled but not yet exercised —
//! the toolchain isn't installed on this machine. When it is, this drops in behind
//! [`CompileOracle`] with no changes to the loop.
//!
//! NOTE: the current cflags produce a `-c` object, enough to exercise the parser end
//! to end. A Renode-bootable ELF additionally needs a startup file + linker script
//! (tracked in docs/P1-verification-loop.md §8).
#![allow(dead_code)]

use crate::gcc::parse_gcc_stderr;
use crate::oracle::{CompileOracle, CompileResult, Diagnostic, FirmwareDraft};
use std::fs;
use std::process::Command;

pub struct ArmGccOracle {
    pub cc: String,
    pub cflags: Vec<String>,
}

impl Default for ArmGccOracle {
    fn default() -> Self {
        Self {
            cc: "arm-none-eabi-gcc".to_string(),
            cflags: vec![
                "-mcpu=cortex-m4".to_string(),
                "-mthumb".to_string(),
                "-nostdlib".to_string(),
                "-c".to_string(),
            ],
        }
    }
}

impl ArmGccOracle {
    pub fn available(&self) -> bool {
        Command::new(&self.cc).arg("--version").output().is_ok()
    }

    fn version(&self) -> String {
        match Command::new(&self.cc).arg("--version").output() {
            Ok(o) => String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or(&self.cc)
                .to_string(),
            Err(_) => format!("{} (absent)", self.cc),
        }
    }
}

impl CompileOracle for ArmGccOracle {
    fn name(&self) -> &str {
        "arm-none-eabi-gcc"
    }

    fn compile(&self, draft: &FirmwareDraft) -> CompileResult {
        if !self.available() {
            return CompileResult {
                ok: false,
                diagnostics: vec![Diagnostic {
                    severity: "error".to_string(),
                    message: format!("{} not installed", self.cc),
                    file: None,
                    line: None,
                    col: None,
                    raw: String::new(),
                }],
                stdout: String::new(),
                stderr: String::new(),
                artifact_path: None,
                toolchain: format!("{} (absent)", self.cc),
                toolchain_available: false,
            };
        }

        let dir = std::env::temp_dir().join(format!("embeder-build-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        for (rel, src) in &draft.files {
            let p = dir.join(rel);
            if let Some(parent) = p.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&p, src);
        }
        let out_obj = dir.join("out.o");

        let mut cmd = Command::new(&self.cc);
        cmd.args(&self.cflags)
            .arg(dir.join(&draft.entry))
            .arg("-o")
            .arg(&out_obj);

        match cmd.output() {
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let diagnostics = parse_gcc_stderr(&stderr);
                let ok = o.status.success();
                CompileResult {
                    ok,
                    diagnostics,
                    stdout,
                    stderr,
                    artifact_path: if ok {
                        Some(out_obj.to_string_lossy().to_string())
                    } else {
                        None
                    },
                    toolchain: self.version(),
                    toolchain_available: true,
                }
            }
            Err(e) => CompileResult {
                ok: false,
                diagnostics: vec![Diagnostic {
                    severity: "error".to_string(),
                    message: format!("failed to run {}: {}", self.cc, e),
                    file: None,
                    line: None,
                    col: None,
                    raw: String::new(),
                }],
                stdout: String::new(),
                stderr: String::new(),
                artifact_path: None,
                toolchain: format!("{} (error)", self.cc),
                toolchain_available: false,
            },
        }
    }
}
