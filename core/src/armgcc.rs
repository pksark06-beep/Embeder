//! Real `arm-none-eabi-gcc` compile oracle (P1b).
//!
//! With a board build profile (CPU flags + startup source + linker script) it
//! compiles AND links a bootable ELF; without a linker script it falls back to a
//! `-c` object build. Slots behind [`CompileOracle`] with no changes to the loop.

use crate::gcc::parse_gcc_stderr;
use crate::oracle::{CompileOracle, CompileResult, Diagnostic, FirmwareDraft};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// Per-process sequence so concurrent compiles never share a build directory.
static BUILD_SEQ: AtomicU64 = AtomicU64::new(0);

/// Explicit configuration wins; also find the per-user Windows installation when
/// a desktop launched before PATH changed still has the old environment.
pub fn compiler_path() -> String {
    if let Ok(path) = std::env::var("EMBEDER_GCC") {
        return path;
    }
    if Command::new("arm-none-eabi-gcc").arg("--version").output()
        .map(|out| out.status.success()).unwrap_or(false) {
        return "arm-none-eabi-gcc".into();
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let path = PathBuf::from(local).join("Programs/ArmGNU/bin/arm-none-eabi-gcc.exe");
        if path.is_file() { return path.to_string_lossy().into_owned(); }
    }
    "arm-none-eabi-gcc".into()
}

pub struct ArmGccOracle {
    pub cc: String,
    pub cpu_flags: Vec<String>,
    pub support_sources: Vec<PathBuf>, // board support (startup.c, ...) compiled with the app
    pub linker_script: Option<PathBuf>,
    pub extra_flags: Vec<String>,
}

impl Default for ArmGccOracle {
    fn default() -> Self {
        Self {
            cc: compiler_path(),
            cpu_flags: vec!["-mcpu=cortex-m4".into(), "-mthumb".into()],
            support_sources: Vec::new(),
            linker_script: None,
            extra_flags: vec!["-nostdlib".into(), "-c".into()],
        }
    }
}

impl ArmGccOracle {
    /// Board profile for the STM32F4 blink+UART target. `firmware_dir` points at
    /// `firmware/stm32-blink-uart`.
    pub fn stm32f4(firmware_dir: impl Into<PathBuf>) -> Self {
        let dir = firmware_dir.into();
        Self {
            cc: compiler_path(),
            cpu_flags: vec!["-mcpu=cortex-m4".into(), "-mthumb".into()],
            support_sources: vec![dir.join("src").join("startup.c")],
            linker_script: Some(dir.join("link").join("stm32f4.ld")),
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
        }
    }

    pub fn available(&self) -> bool {
        Command::new(&self.cc).arg("--version").output()
            .map(|out| out.status.success()).unwrap_or(false)
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
            return absent(&self.cc);
        }

        let seq = BUILD_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "embeder-build-{}-{}-{}",
            std::process::id(),
            seq,
            draft.target
        ));
        let _ = fs::remove_dir_all(&dir);
        if fs::create_dir_all(&dir).is_err() {
            return err_result(&self.cc, "cannot create build dir");
        }

        // Write model-authored sources; collect the compilable ones.
        let mut sources: Vec<PathBuf> = Vec::new();
        for (rel, src) in &draft.files {
            let p = dir.join(rel);
            if let Some(parent) = p.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if fs::write(&p, src).is_err() {
                return err_result(&self.cc, &format!("cannot write {}", p.display()));
            }
            if rel.ends_with(".c") || rel.ends_with(".s") || rel.ends_with(".S") {
                sources.push(p);
            }
        }
        for s in &self.support_sources {
            sources.push(s.clone());
        }

        let mut cmd = Command::new(&self.cc);
        cmd.current_dir(&dir);
        cmd.args(&self.cpu_flags).args(&self.extra_flags);

        let artifact = if let Some(ld) = &self.linker_script {
            let out = dir.join("firmware.elf");
            cmd.arg("-T").arg(ld);
            for s in &sources {
                cmd.arg(s);
            }
            cmd.arg("-o").arg(&out);
            out
        } else {
            let out = dir.join("out.o");
            for s in &sources {
                cmd.arg(s);
            }
            cmd.arg("-o").arg(&out);
            out
        };

        match cmd.output() {
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let diagnostics = parse_gcc_stderr(&stderr);
                let ok = o.status.success() && artifact.exists();
                CompileResult {
                    ok,
                    diagnostics,
                    stdout,
                    stderr,
                    artifact_path: if ok {
                        Some(artifact.to_string_lossy().to_string())
                    } else {
                        None
                    },
                    toolchain: self.version(),
                    toolchain_available: true,
                }
            }
            Err(e) => err_result(&self.cc, &format!("failed to run {}: {}", self.cc, e)),
        }
    }
}

fn absent(cc: &str) -> CompileResult {
    CompileResult {
        ok: false,
        diagnostics: vec![Diagnostic {
            severity: "error".into(),
            message: format!("{} not installed", cc),
            file: None,
            line: None,
            col: None,
            raw: String::new(),
        }],
        stdout: String::new(),
        stderr: String::new(),
        artifact_path: None,
        toolchain: format!("{} (absent)", cc),
        toolchain_available: false,
    }
}

fn err_result(cc: &str, msg: &str) -> CompileResult {
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
        toolchain: format!("{} (error)", cc),
        toolchain_available: false,
    }
}
