//! Shared desktop API — the Core rendered as JSON views. The dev server and the Tauri
//! command layer both call these, so the UI behaves identically in either host.

use embeder_core::{ArmGccOracle, CompileOracle, FirmwareDraft, RenodeOracle, SimOracle};
use embeder_datasheet::DatasheetIndex;
use embeder_mcp::McpClient;
use serde_json::{json, Value};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const MAX_EDIT_BYTES: usize = 512 * 1024;

mod tasks;
pub use tasks::{start_generate_task, start_task, task_status};

fn repo_root() -> PathBuf {
    // Dev fallback only: CARGO_MANIFEST_DIR = <root>/desktop/api. In a packaged app
    // this path does not exist on the user's disk, so production hosts set the
    // EMBEDER_* env vars below and this branch is never reached.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Read-only bundled CMSIS-SVD. A packaged host (Tauri) sets `EMBEDER_SVD` to the
/// file inside its resource directory; dev falls back to the repo layout.
fn svd_path() -> PathBuf {
    if let Some(path) = std::env::var_os("EMBEDER_SVD") {
        return PathBuf::from(path);
    }
    repo_root().join("datasheet").join("data").join("stm32f4-mini.svd")
}

/// Directory holding the Python MCP tool scripts (the BYOK codegen server). A packaged
/// host sets `EMBEDER_SERVERS_DIR` to the scripts inside its resource directory; dev
/// falls back to the repo `servers/` layout. Without this, an installed app can't find
/// the codegen server and silently degrades to non-AI synthesis.
fn servers_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("EMBEDER_SERVERS_DIR") {
        return PathBuf::from(path);
    }
    repo_root().join("servers")
}

/// Writable per-user firmware workspace. A packaged host sets `EMBEDER_WORKSPACE_DIR`
/// (seeded once from the bundled template via [`ensure_workspace_seeded`]); dev edits
/// the repo checkout directly.
fn workspace_root() -> PathBuf {
    if let Some(path) = std::env::var_os("EMBEDER_WORKSPACE_DIR") {
        return PathBuf::from(path);
    }
    repo_root().join("firmware").join("stm32-blink-uart")
}

/// Seed the per-user workspace from a read-only template if it has no sources yet.
/// Hosts call this once at startup after `EMBEDER_WORKSPACE_DIR` is set. It is a no-op
/// once the workspace contains `src/main.c`, so user edits are never overwritten.
pub fn ensure_workspace_seeded(template: &Path) -> io::Result<()> {
    let dest = workspace_root();
    if dest.join("src").join("main.c").is_file() {
        return Ok(());
    }
    copy_dir_recursive(template, &dest)
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> io::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn probe(cmd: &str, arg: &str) -> Option<String> {
    let out = Command::new(cmd).arg(arg).output().ok()?;
    if !out.status.success() { return None; }
    String::from_utf8_lossy(&out.stdout).lines().next().map(|s| s.trim().to_string())
}
fn renode_present() -> Option<String> {
    let candidate = std::env::var("EMBEDER_RENODE")
        .unwrap_or_else(|_| r"C:\Program Files\Renode\bin\Renode.exe".to_string());
    probe(&candidate, "--version")
}
fn tool(name: &str, version: Option<String>) -> Value {
    json!({"name": name, "available": version.is_some(), "version": version})
}

fn language_for(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("c") | Some("h") => "c",
        Some("s") | Some("S") => "asm",
        Some("ld") => "linker",
        Some("md") => "markdown",
        _ => "text",
    }
}

fn allowed_workspace_file(relative: &str, for_write: bool) -> Result<PathBuf, String> {
    let relative_path = Path::new(relative);
    if relative_path.as_os_str().is_empty()
        || relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("workspace path must be a relative file path".to_string());
    }

    let extension = relative_path.extension().and_then(|value| value.to_str()).unwrap_or("");
    let allowed = matches!(extension, "c" | "h" | "s" | "S" | "ld" | "md");
    if !allowed || (for_write && extension == "md") {
        return Err("file type is not editable in the firmware sandbox".to_string());
    }

    let root = workspace_root()
        .canonicalize()
        .map_err(|error| format!("workspace unavailable: {error}"))?;
    let candidate = root.join(relative_path);
    let parent = candidate
        .parent()
        .ok_or_else(|| "workspace file has no parent".to_string())?
        .canonicalize()
        .map_err(|error| format!("workspace directory unavailable: {error}"))?;
    if !parent.starts_with(&root) {
        return Err("workspace path escaped the sandbox".to_string());
    }
    if candidate.exists() && !candidate.canonicalize().map_err(|e| e.to_string())?.starts_with(&root) {
        return Err("workspace file escaped the sandbox".to_string());
    }
    Ok(candidate)
}

fn collect_workspace_files(root: &Path, directory: &Path, output: &mut Vec<Value>) {
    let Ok(entries) = fs::read_dir(directory) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if entry.file_type().map(|kind| kind.is_symlink()).unwrap_or(true) { continue; }
        let path = entry.path();
        if path.is_dir() {
            collect_workspace_files(root, &path, output);
            continue;
        }
        let Some(relative) = path.strip_prefix(root).ok().and_then(Path::to_str) else { continue };
        let relative = relative.replace('\\', "/");
        if allowed_workspace_file(&relative, false).is_err() {
            continue;
        }
        let size = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        output.push(json!({
            "path": relative,
            "name": path.file_name().and_then(|name| name.to_str()).unwrap_or("file"),
            "language": language_for(&path),
            "size": size,
            "editable": path.extension().and_then(|value| value.to_str()) != Some("md"),
        }));
    }
}

fn spawn_mcp(name: &str) -> Result<McpClient, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let args = vec!["--mcp-server".to_string(), name.to_string()];
    McpClient::spawn(&executable.to_string_lossy(), &args).map_err(|error| error.to_string())
}

fn mcp_server_status(id: &str, label: &str, dependency: Value) -> Value {
    let started = Instant::now();
    match spawn_mcp(id).and_then(|mut client| client.list_tools().map_err(|error| error.to_string())) {
        Ok(tools) => json!({
            "id": id,
            "name": label,
            "transport": "stdio JSON-RPC",
            "isolation": "native worker · path jailed",
            "status": "online",
            "latency_ms": started.elapsed().as_millis(),
            "dependency": dependency,
            "tools": tools,
        }),
        Err(error) => json!({
            "id": id,
            "name": label,
            "transport": "stdio JSON-RPC",
            "isolation": "native worker · path jailed",
            "status": "offline",
            "latency_ms": started.elapsed().as_millis(),
            "dependency": dependency,
            "tools": [],
            "error": error,
        }),
    }
}

fn mcp_tools(kind: &str) -> Vec<Value> {
    match kind {
        "datasheet" => vec![
            json!({
                "name": "register_map",
                "description": "Read a grounded peripheral register map from the bundled CMSIS-SVD.",
                "inputSchema": {"type": "object", "properties": {"peripheral": {"type": "string"}}, "required": ["peripheral"]}
            }),
            json!({
                "name": "lookup_register",
                "description": "Read one grounded register from the bundled CMSIS-SVD.",
                "inputSchema": {"type": "object", "properties": {"peripheral": {"type": "string"}, "register": {"type": "string"}}, "required": ["peripheral", "register"]}
            }),
        ],
        "firmware" => vec![json!({
            "name": "compile_firmware",
            "description": "Compile firmware inside an isolated temporary build directory with the fixed STM32F4 profile.",
            "inputSchema": {"type": "object", "properties": {"files": {"type": "object"}}, "required": ["files"]}
        })],
        "simulation" => vec![json!({
            "name": "run_simulation",
            "description": "Run a sandboxed STM32F4 ELF in Renode and capture USART2.",
            "inputSchema": {"type": "object", "properties": {"elf_path": {"type": "string"}}, "required": ["elf_path"]}
        })],
        _ => Vec::new(),
    }
}

fn sandboxed_artifact(path: &str) -> Result<PathBuf, String> {
    let artifact = PathBuf::from(path)
        .canonicalize()
        .map_err(|error| format!("artifact unavailable: {error}"))?;
    let temp = std::env::temp_dir()
        .canonicalize()
        .map_err(|error| format!("temporary directory unavailable: {error}"))?;
    let workspace = workspace_root()
        .canonicalize()
        .map_err(|error| format!("workspace unavailable: {error}"))?;
    if !artifact.starts_with(&temp) && !artifact.starts_with(&workspace) {
        return Err("artifact is outside the MCP sandbox".to_string());
    }
    Ok(artifact)
}

fn call_local_mcp_tool(kind: &str, name: &str, arguments: &Value) -> Result<Value, String> {
    match (kind, name) {
        ("datasheet", "register_map") | ("datasheet", "lookup_register") => {
            let peripheral = arguments
                .get("peripheral")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing peripheral".to_string())?;
            let result = register_map(Some(peripheral));
            if result.get("found").and_then(Value::as_bool) != Some(true) {
                return Ok(result);
            }
            let peripheral_value = result.get("peripheral").cloned().unwrap_or(Value::Null);
            if name == "lookup_register" {
                let register_name = arguments
                    .get("register")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing register".to_string())?;
                let register = peripheral_value
                    .get("registers")
                    .and_then(Value::as_array)
                    .and_then(|registers| {
                        registers.iter().find(|register| {
                            register.get("name").and_then(Value::as_str).map(|value| value.eq_ignore_ascii_case(register_name)).unwrap_or(false)
                        })
                    })
                    .cloned();
                return Ok(match register {
                    Some(register) => json!({"found": true, "name": peripheral, "register": register, "source": "stm32f4-mini.svd"}),
                    None => json!({"found": false, "note": format!("register '{peripheral}.{register_name}' is not in the structured source")}),
                });
            }
            Ok(json!({
                "found": true,
                "name": peripheral_value.get("name").cloned().unwrap_or(Value::Null),
                "base_address": peripheral_value.get("base_address").cloned().unwrap_or(Value::Null),
                "registers": peripheral_value.get("registers").cloned().unwrap_or_else(|| json!([])),
                "source": "stm32f4-mini.svd",
            }))
        }
        ("firmware", "compile_firmware") => {
            let files = arguments
                .get("files")
                .and_then(Value::as_object)
                .ok_or_else(|| "missing files".to_string())?;
            let mut draft_files = Vec::new();
            for (path, contents) in files {
                let contents = contents.as_str().ok_or_else(|| "source content must be text".to_string())?;
                if contents.len() > MAX_EDIT_BYTES {
                    return Err("source file exceeds the MCP sandbox limit".to_string());
                }
                if Path::new(path).is_absolute()
                    || Path::new(path).components().any(|component| !matches!(component, Component::Normal(_)))
                {
                    return Err("source path escaped the MCP sandbox".to_string());
                }
                draft_files.push((path.clone(), contents.to_string()));
            }
            let draft = FirmwareDraft::new("stm32f4-discovery", "main.c", draft_files);
            let mut compiler = ArmGccOracle::stm32f4(workspace_root());
            if arguments.get("workspace_snapshot").and_then(Value::as_bool) == Some(true) {
                if !files.contains_key("src/main.c") || !files.contains_key("src/startup.c") || !files.contains_key("link/stm32f4.ld") {
                    return Err("workspace snapshot requires src/main.c, src/startup.c and link/stm32f4.ld".into());
                }
                compiler.support_sources.clear();
                compiler.linker_script = Some(PathBuf::from("link/stm32f4.ld"));
                compiler.extra_flags.extend(["-Isrc".into(), "-Iinclude".into()]);
            }
            let result = compiler.compile(&draft);
            Ok(json!({
                "ok": result.ok,
                "available": result.toolchain_available,
                "stdout": result.stdout,
                "stderr": result.stderr,
                "artifact_path": result.artifact_path,
                "toolchain": result.toolchain,
                "diagnostics": result.diagnostics.iter().map(|diagnostic| json!({
                    "severity": diagnostic.severity,
                    "message": diagnostic.message,
                    "file": diagnostic.file,
                    "line": diagnostic.line,
                    "col": diagnostic.col,
                })).collect::<Vec<_>>(),
            }))
        }
        ("simulation", "run_simulation") => {
            let artifact = arguments
                .get("elf_path")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing elf_path".to_string())?;
            let artifact = sandboxed_artifact(artifact)?;
            let renode = std::env::var("EMBEDER_RENODE")
                .unwrap_or_else(|_| r"C:\Program Files\Renode\bin\Renode.exe".to_string());
            let result = RenodeOracle::stm32f4(renode)
                .simulate(&artifact.to_string_lossy(), "stm32f4-discovery");
            Ok(json!({
                "ok": result.ok,
                "available": result.engine_available,
                "engine": result.engine,
                "observations": result.observations,
                "log": result.log,
                "fault": result.fault,
                "boundary": {
                    "verified": result.boundary.verified,
                    "stubbed": result.boundary.stubbed,
                    "not_modeled": result.boundary.not_modeled,
                }
            }))
        }
        _ => Err(format!("tool '{name}' is not available on MCP server '{kind}'")),
    }
}

fn write_mcp_message(output: &mut impl Write, value: Value) -> io::Result<()> {
    writeln!(output, "{value}")?;
    output.flush()
}

/// Run one isolated MCP worker over stdio. Desktop hosts enter this mode when they
/// are re-launched with `--mcp-server <kind>` by the local MCP client.
pub fn serve_mcp_stdio(kind: &str) -> io::Result<()> {
    if !matches!(kind, "datasheet" | "firmware" | "simulation") {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "unknown MCP server kind"));
    }
    let stdin = io::stdin();
    let mut output = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.len() > 2 * 1024 * 1024 {
            continue;
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        match method {
            "initialize" => write_mcp_message(&mut output, json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": format!("embeder-{kind}"), "version": "0.1.0"}
                }
            }))?,
            "notifications/initialized" => {}
            "tools/list" => write_mcp_message(&mut output, json!({
                "jsonrpc": "2.0", "id": id, "result": {"tools": mcp_tools(kind)}
            }))?,
            "tools/call" => {
                let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                match call_local_mcp_tool(kind, name, &arguments) {
                    Ok(result) => write_mcp_message(&mut output, json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": {"content": [{"type": "text", "text": result.to_string()}], "isError": false}
                    }))?,
                    Err(error) => write_mcp_message(&mut output, json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": {"content": [{"type": "text", "text": json!({"error": error}).to_string()}], "isError": true}
                    }))?,
                }
            }
            "shutdown" | "exit" => break,
            _ => write_mcp_message(&mut output, json!({
                "jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "method not found"}
            }))?,
        }
    }
    Ok(())
}

pub fn project_info() -> Value {
    json!({
        "name": "embeder-workspace",
        "target": "STM32F4-Discovery",
        "mcu": "STM32F407 · Cortex-M4",
        "verification_mode": "real",
        "tiers": [
            {"tier": 1, "family": "Arduino / AVR",    "status": "prototyping"},
            {"tier": 2, "family": "ESP32 / ESP8266",  "status": "partial"},
            {"tier": 3, "family": "RP2040 / RPi",     "status": "strong"},
            {"tier": 4, "family": "STM32 (Cortex-M)", "status": "primary"}
        ],
        "toolchains": [
            tool("arm-none-eabi-gcc", probe(&embeder_core::armgcc::compiler_path(), "--version")),
            tool("renode", renode_present()),
            tool("python", probe("python", "--version")),
        ],
        "phases": [
            {"id": "P0",  "label": "Design",              "done": true},
            {"id": "P1",  "label": "Verification loop",   "done": true},
            {"id": "P2",  "label": "Datasheet grounding", "done": true},
            {"id": "MCP", "label": "MCP backbone",        "done": true},
            {"id": "P3",  "label": "UI shell",            "done": true}
        ]
    })
}

pub fn workspace_files() -> Value {
    let root = workspace_root();
    let mut files = Vec::new();
    collect_workspace_files(&root, &root, &mut files);
    json!({
        "root": "firmware/stm32-blink-uart",
        "sandboxed": true,
        "files": files,
    })
}

pub fn read_workspace_file(path: Option<&str>) -> Value {
    let Some(path) = path else { return json!({"error": "missing 'path'"}) };
    let file = match allowed_workspace_file(path, false) {
        Ok(file) => file,
        Err(error) => return json!({"error": error}),
    };
    match fs::read_to_string(&file) {
        Ok(contents) => json!({
            "path": path.replace('\\', "/"),
            "name": file.file_name().and_then(|name| name.to_str()).unwrap_or("file"),
            "language": language_for(&file),
            "contents": contents,
            "bytes": contents.len(),
            "editable": file.extension().and_then(|value| value.to_str()) != Some("md"),
            "sandboxed": true,
        }),
        Err(error) => json!({"error": format!("cannot read workspace file: {error}")}),
    }
}

pub fn save_workspace_file(path: Option<&str>, contents: &str) -> Value {
    let Some(path) = path else { return json!({"error": "missing 'path'"}) };
    if contents.len() > MAX_EDIT_BYTES {
        return json!({"error": format!("file exceeds the {} KiB editor limit", MAX_EDIT_BYTES / 1024)});
    }
    let file = match allowed_workspace_file(path, true) {
        Ok(file) => file,
        Err(error) => return json!({"error": error}),
    };
    match fs::write(&file, contents.as_bytes()) {
        Ok(()) => json!({
            "ok": true,
            "path": path.replace('\\', "/"),
            "bytes": contents.len(),
            "sandboxed": true,
        }),
        Err(error) => json!({"error": format!("cannot save workspace file: {error}")}),
    }
}

pub fn mcp_status() -> Value {
    let gcc = probe(&embeder_core::armgcc::compiler_path(), "--version");
    let renode = renode_present();
    let runtime = std::env::current_exe().ok().map(|path| path.to_string_lossy().into_owned());
    json!({
        "protocol": "2024-11-05",
        "sandbox": {
            "workspace": workspace_root().display().to_string(),
            "network": "no network tools exposed",
            "process_model": "one isolated stdio process per request",
            "file_limit_kib": MAX_EDIT_BYTES / 1024,
        },
        "runtime": tool("embeder-mcp-worker", runtime),
        "servers": [
            mcp_server_status("datasheet", "Datasheet", json!({"name": "CMSIS-SVD", "available": svd_path().exists()})),
            mcp_server_status("firmware", "Firmware", json!({"name": "arm-none-eabi-gcc", "available": gcc.is_some(), "version": gcc})),
            mcp_server_status("simulation", "Simulation", json!({"name": "Renode", "available": renode.is_some(), "version": renode})),
        ]
    })
}

pub fn probe_mcp(server: Option<&str>) -> Value {
    let Some(server) = server else { return json!({"error": "missing 'server'"}) };
    if !matches!(server, "datasheet" | "firmware" | "simulation") {
        return json!({"error": "unknown MCP server"});
    }
    let started = Instant::now();
    let mut client = match spawn_mcp(server) {
        Ok(client) => client,
        Err(error) => return json!({"ok": false, "server": server, "error": error}),
    };
    let tools = match client.list_tools() {
        Ok(tools) => tools,
        Err(error) => return json!({"ok": false, "server": server, "error": error.to_string()}),
    };

    let result = if server == "datasheet" {
        match client.call_tool(
            "register_map",
            json!({"svd_path": svd_path(), "peripheral": "USART2"}),
        ) {
            Ok(value) => json!({
                "tool": "register_map",
                "found": value.get("found").and_then(Value::as_bool).unwrap_or(false),
                "registers": value.get("registers").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
                "source": value.get("source").cloned().unwrap_or(Value::Null),
            }),
            Err(error) => return json!({"ok": false, "server": server, "error": error.to_string()}),
        }
    } else {
        json!({"tool": "tools/list", "note": "transport and capability discovery passed"})
    };

    json!({
        "ok": true,
        "server": server,
        "latency_ms": started.elapsed().as_millis(),
        "tool_count": tools.len(),
        "result": result,
        "sandboxed": true,
    })
}

pub fn peripherals() -> Value {
    let idx = match DatasheetIndex::from_svd_file(svd_path()) {
        Ok(i) => i,
        Err(e) => return json!({"error": format!("SVD: {e}")}),
    };
    let list: Vec<Value> = idx
        .device
        .peripherals
        .iter()
        .map(|p| {
            json!({
                "name": p.name,
                "base_address": p.base_address,
                "register_count": p.registers.len(),
                "description": p.description,
            })
        })
        .collect();
    json!({"device": idx.device.name, "peripherals": list})
}

pub fn register_map(peripheral: Option<&str>) -> Value {
    let Some(peripheral) = peripheral else {
        return json!({"error": "missing 'peripheral'"});
    };
    let idx = match DatasheetIndex::from_svd_file(svd_path()) {
        Ok(i) => i,
        Err(e) => return json!({"error": format!("SVD: {e}")}),
    };
    let g = idx.register_map(peripheral);
    match g.value {
        Some(v) => json!({
            "found": true,
            "tier": {"symbol": g.tier.symbol(), "name": g.tier.as_str()},
            "citation": g.citation.map(|c| json!({"source": c.source, "path": c.path})),
            "peripheral": {
                "name": v.name,
                "base_address": v.base_address,
                "registers": v.registers.iter().map(|r| json!({
                    "name": r.name,
                    "offset": r.offset,
                    "absolute_address": r.absolute_address,
                    "description": r.description,
                    "fields": r.fields.iter().map(|f| json!({
                        "name": f.name, "bit_offset": f.bit_offset,
                        "bit_width": f.bit_width, "description": f.description,
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            }
        }),
        None => json!({
            "found": false,
            "tier": {"symbol": g.tier.symbol(), "name": g.tier.as_str()},
            "note": g.note
        }),
    }
}

pub fn firmware() -> Value {
    match fs::read_to_string(workspace_root().join("src/main.c")) {
        Ok(source) => json!({"entry": "src/main.c", "target": "stm32f4-discovery", "source": source, "citations": []}),
        Err(error) => json!({"error": error.to_string()}),
    }
}

/// Compatibility endpoint: executes the same real service as background tasks.
pub fn run_verification() -> Value {
    tasks::execute_task(true, &|_, _, _| {})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_jail_rejects_traversal_and_absolute_paths() {
        assert!(allowed_workspace_file("../../README.md", false).is_err());
        assert!(allowed_workspace_file(r"C:\\Windows\\system.ini", false).is_err());
    }

    #[test]
    fn workspace_jail_accepts_firmware_sources() {
        let path = allowed_workspace_file("src/main.c", true).expect("main.c in workspace");
        assert!(path.ends_with(Path::new("src").join("main.c")));
    }

    #[test]
    fn mcp_catalog_is_scoped_by_server_kind() {
        assert_eq!(mcp_tools("datasheet").len(), 2);
        assert_eq!(mcp_tools("firmware").len(), 1);
        assert_eq!(mcp_tools("simulation").len(), 1);
        assert!(mcp_tools("unknown").is_empty());
    }
}
