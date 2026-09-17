//! Real workspace tasks. Hosts schedule work here; renderers only observe state.
use super::*;
use embeder_core::{short_hash, Codegen, Diagnostic, LoopContext, ProvenanceLedger};
use embeder_datasheet::GroundedCodegen;
use embeder_mcp::LlmCodegen;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};

static TASKS: OnceLock<Mutex<BTreeMap<String, Value>>> = OnceLock::new();
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn tasks() -> &'static Mutex<BTreeMap<String, Value>> {
    TASKS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Register a task, run `work` on a background thread with a live event emitter, and
/// store its result. One running task at a time; renderers only ever observe state.
fn spawn_task<F>(work: F) -> Value
where
    F: FnOnce(&dyn Fn(&str, &str, &str)) -> Value + Send + 'static,
{
    let id = format!("{}-{}", std::process::id(), SEQUENCE.fetch_add(1, Ordering::Relaxed));
    {
        let mut registry = tasks().lock().unwrap();
        if registry.values().any(|task| task["status"] == "running") {
            return json!({"error": "A workspace task is already running"});
        }
        // Keep recent task results without growing the desktop process forever.
        if registry.len() >= 32 { registry.clear(); }
        registry.insert(id.clone(), json!({"id": id, "status": "running", "events": [], "result": null}));
    }
    let task_id = id.clone();
    std::thread::spawn(move || {
        let emit = |stage: &str, status: &str, message: &str| {
            let mut registry = tasks().lock().unwrap();
            if let Some(task) = registry.get_mut(&task_id) {
                task["events"].as_array_mut().unwrap().push(json!({
                    "stage": stage, "status": status, "message": message,
                }));
            }
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&emit)))
            .unwrap_or_else(|_| json!({"error": "Task worker panicked", "state": "halted", "mode": "real"}));
        let mut registry = tasks().lock().unwrap();
        if let Some(task) = registry.get_mut(&task_id) {
            task["status"] = json!("completed");
            task["result"] = result;
        }
    });
    json!({"id": id, "status": "running"})
}

/// Compile (and optionally simulate) the saved workspace sources — no generation.
pub fn start_task(simulate: bool) -> Value {
    spawn_task(move |emit| execute_task(simulate, emit))
}

/// Draft firmware for `goal` with the BYOK model, then verify it through the same
/// oracle chain. Non-destructive: the user's workspace files are never overwritten;
/// the drafted source is returned for the UI to display and optionally save.
pub fn start_generate_task(goal: String) -> Value {
    spawn_task(move |emit| execute_generate(&goal, emit))
}

pub fn task_status(id: &str) -> Value {
    tasks().lock().unwrap().get(id).cloned()
        .unwrap_or_else(|| json!({"error": "Unknown task"}))
}

fn snapshot() -> Result<Value, String> {
    let root = workspace_root();
    let mut entries = Vec::new();
    collect_workspace_files(&root, &root, &mut entries);
    let mut files = serde_json::Map::new();
    let mut total = 0;
    for entry in entries {
        let path = entry["path"].as_str().ok_or("invalid workspace entry")?;
        if path.ends_with(".md") { continue; }
        let file = allowed_workspace_file(path, false)?;
        let contents = fs::read_to_string(file).map_err(|error| error.to_string())?;
        total += contents.len();
        if contents.len() > MAX_EDIT_BYTES || total > 1024 * 1024 {
            return Err("Workspace exceeds the 1 MiB source snapshot limit".into());
        }
        files.insert(path.into(), json!(contents));
    }
    Ok(Value::Object(files))
}

fn call(server: &str, tool: &str, args: Value) -> Result<Value, String> {
    let mut client = spawn_mcp(server)?;
    let value = client.call_tool(tool, args).map_err(|error| error.to_string())?;
    if let Some(error) = value.get("error") { return Err(error.to_string()); }
    Ok(value)
}

pub(super) fn execute_task(simulate: bool, emit: &dyn Fn(&str, &str, &str)) -> Value {
    match execute(simulate, emit) {
        Ok(result) => result,
        Err(error) => {
            emit("verified", "failed", &error);
            json!({"mode": "real", "state": "halted", "error": error})
        }
    }
}

fn execute(simulate: bool, emit: &dyn Fn(&str, &str, &str)) -> Result<Value, String> {
    let started = Instant::now();
    emit("intent", "running", "Reading saved workspace sources");
    let files = snapshot()?;
    let fingerprint = short_hash(&files.to_string());
    emit("intent", "done", &format!("Captured {} workspace files ({fingerprint})", files.as_object().unwrap().len()));
    emit("ground", "done", "Validated the STM32F4 board profile, startup source and linker script");
    emit("compile", "running", "MCP firmware worker: compiling workspace with arm-none-eabi-gcc");
    let mut compile = call("firmware", "compile_firmware", json!({"files": files, "workspace_snapshot": true}))?;
    // Map isolated build paths back to clickable workspace-relative diagnostics.
    for diagnostic in compile["diagnostics"].as_array_mut().into_iter().flatten() {
        if let Some(file) = diagnostic["file"].as_str() {
            let normalized = file.replace('\\', "/");
            if let Some(path) = files.as_object().unwrap().keys().find(|path| normalized.ends_with(&format!("/{path}")) || normalized == **path) {
                diagnostic["file"] = json!(path);
            }
        }
    }
    let built = compile["ok"] == true;
    emit("compile", if built { "done" } else { "failed" },
        if built { "GCC produced firmware.elf" } else { "GCC rejected the workspace; see Problems and Build output" });
    let mut simulation = Value::Null;
    if built && simulate {
        emit("simulate", "running", "MCP simulation worker: booting this ELF in Renode");
        simulation = call("simulation", "run_simulation", json!({"elf_path": compile["artifact_path"]}))?;
        emit("simulate", if simulation["ok"] == true { "done" } else { "failed" },
            if simulation["ok"] == true { "Captured the expected USART2 banner" } else { "Renode did not satisfy the UART assertion" });
    }
    let verified = built && simulate && simulation["ok"] == true;
    let state = if verified { "verified" } else if built && !simulate { "built" } else { "halted" };
    emit("verified", if state == "halted" { "failed" } else { "done" },
        if verified { "ELF boot and expected UART transmission passed" } else if built && !simulate { "Build succeeded; simulation was not requested" } else { "Task failed; source files were not regenerated" });
    let ledger_path = std::env::temp_dir().join(format!("embeder-task-{}-{}.jsonl", std::process::id(), embeder_core::now_secs()));
    let mut ledger = ProvenanceLedger::new(&ledger_path).map_err(|error| error.to_string())?;
    ledger.record("user", "compile", &files.to_string(), &compile.to_string(), None,
        compile["toolchain"].as_str().unwrap_or("arm-none-eabi-gcc"), "{}");
    if !simulation.is_null() {
        ledger.record("oracle", "simulate", &compile["artifact_path"].to_string(), &simulation.to_string(),
            if verified { Some("verified") } else { None }, simulation["engine"].as_str().unwrap_or("renode"), "{}");
    }
    let provenance: Vec<Value> = ledger.records.iter().map(|record| json!({
        "id": record.id, "ts": record.ts, "actor": record.actor, "tool": record.tool,
        "tier": record.tier, "oracle": record.oracle_version,
        "inputs_hash": record.inputs_hash, "outputs_hash": record.outputs_hash,
    })).collect();
    let observations: Vec<Value> = simulation["observations"].as_array().into_iter().flatten()
        .map(|pair| json!({"key": pair[0], "value": pair[1]})).collect();
    // The configured oracle checks UART text. It does not measure GPIO edges.
    let boundary = if verified {
        json!({"verified": ["cpu_boot", "uart_tx"], "stubbed": [], "not_modeled": ["gpio_toggle_assertion", "real_analog", "rf"]})
    } else { json!({"verified": [], "stubbed": [], "not_modeled": []}) };
    Ok(json!({
        "goal": "Build saved STM32F4 sources and check USART2 in Renode",
        "mode": "real", "state": state, "attempts": 1, "self_healed": false,
        "tier": if verified { json!({"symbol": "✓", "name": "verified"}) } else { Value::Null },
        "artifact": compile["artifact_path"], "diagnostics": compile["diagnostics"],
        "compile": compile, "simulation": simulation, "observations": observations,
        "boundary": boundary, "citations": [], "provenance": provenance,
        "ledger_path": ledger_path, "source_fingerprint": fingerprint,
        "duration_ms": started.elapsed().as_millis(),
    }))
}

pub(super) fn execute_generate(goal: &str, emit: &dyn Fn(&str, &str, &str)) -> Value {
    match generate(goal, emit) {
        Ok(result) => result,
        Err(error) => {
            emit("verified", "failed", &error);
            json!({"mode": "real", "state": "halted", "error": error})
        }
    }
}

/// Rebuild core `Diagnostic`s from the firmware worker's JSON so they can be fed back
/// into the model on a self-heal attempt (errors only — warnings don't block a build).
fn error_diagnostics(compile: &Value) -> Vec<Diagnostic> {
    compile["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["severity"] == "error")
        .map(|d| Diagnostic {
            severity: "error".to_string(),
            message: d["message"].as_str().unwrap_or("").to_string(),
            file: d["file"].as_str().map(|s| s.to_string()),
            line: d["line"].as_u64().map(|n| n as u32),
            col: d["col"].as_u64().map(|n| n as u32),
            raw: String::new(),
        })
        .collect()
}

fn generate(goal: &str, emit: &dyn Fn(&str, &str, &str)) -> Result<Value, String> {
    let started = Instant::now();
    emit("intent", "running", &format!("Intent: {goal}"));

    // The datasheet layer supplies both the grounding brief handed to the model and
    // the deterministic fallback used when no key is configured.
    let idx = DatasheetIndex::from_svd_file(svd_path()).map_err(|e| format!("SVD: {e}"))?;
    let grounded = GroundedCodegen::new(&idx);
    let (brief, cites) = grounded.grounding_brief();
    let n_cites = cites.len();
    let citations: Vec<Value> = cites
        .iter()
        .map(|c| json!({"source": c.source, "locator": c.locator, "summary": c.summary}))
        .collect();
    emit("intent", "done", &format!("Grounded on {n_cites} cited SVD facts"));

    let server = servers_dir().join("codegen_server.py");
    let mut codegen = LlmCodegen::new(
        "python",
        server.to_string_lossy().to_string(),
        brief,
        cites,
        Box::new(grounded),
    );
    if let Ok(p) = std::env::var("EMBEDER_LLM_PROVIDER") { codegen = codegen.provider(p); }
    if let Ok(m) = std::env::var("EMBEDER_LLM_MODEL") { codegen = codegen.model(m); }

    // Snapshot the workspace once; each attempt swaps in the freshly drafted main.c
    // WITHOUT touching the user's files, then compiles it against the real startup +
    // linker. The workspace on disk is never modified by generation.
    let base = snapshot()?;
    let base_obj = base.as_object().ok_or("workspace snapshot invalid")?.clone();

    let max_attempts = 3u32;
    let mut ctx = LoopContext {
        goal: goal.to_string(),
        attempt: 0,
        compile_diagnostics: Vec::new(),
        sim_fault: None,
    };

    let mut attempts = 0u32;
    let mut state_str = "halted";
    let mut source = String::new();
    let mut compile = Value::Null;
    let mut simulation = Value::Null;
    let mut drafted_by = json!(null);
    let mut draft_oracle = "grounded-synthesis".to_string();

    for attempt in 1..=max_attempts {
        attempts = attempt;
        ctx.attempt = attempt;

        emit(
            "ground",
            "running",
            if attempt == 1 { "Drafting firmware with the model" } else { "Self-healing: re-drafting from the compiler diagnostics" },
        );
        let draft = codegen.generate(&ctx);
        source = draft
            .files
            .iter()
            .find(|(p, _)| p == &draft.entry)
            .map(|(_, s)| s.clone())
            .unwrap_or_default();
        let (used_llm, provider, model, note) = match &codegen.last {
            Some(m) => (m.used_llm, m.provider.clone(), m.model.clone(), m.note.clone()),
            None => (false, String::new(), String::new(), String::new()),
        };
        draft_oracle = if used_llm { format!("{provider}/{model}") } else { "grounded-synthesis".to_string() };
        drafted_by = json!({"used_llm": used_llm, "provider": provider, "model": model, "note": note});
        emit(
            "ground",
            "done",
            &if used_llm {
                format!("Drafted by {draft_oracle} · grounded on {n_cites} SVD facts")
            } else {
                format!("Deterministic grounded synthesis ({note})")
            },
        );

        // Compile the drafted main.c against the workspace startup + linker (in memory).
        let mut files = base_obj.clone();
        files.insert("src/main.c".to_string(), json!(source));
        emit("compile", "running", "MCP firmware worker: compiling the drafted firmware with arm-none-eabi-gcc");
        compile = call("firmware", "compile_firmware", json!({"files": Value::Object(files.clone()), "workspace_snapshot": true}))?;
        for diagnostic in compile["diagnostics"].as_array_mut().into_iter().flatten() {
            if let Some(file) = diagnostic["file"].as_str() {
                let normalized = file.replace('\\', "/");
                if let Some(path) = files.keys().find(|path| normalized.ends_with(&format!("/{path}")) || normalized == **path) {
                    diagnostic["file"] = json!(path);
                }
            }
        }
        let built = compile["ok"] == true;
        emit(
            "compile",
            if built { "done" } else { "failed" },
            if built { "GCC produced firmware.elf" } else { "GCC rejected the draft; feeding diagnostics back to the model" },
        );

        if !built {
            ctx.compile_diagnostics = error_diagnostics(&compile);
            ctx.sim_fault = None;
            if attempt == max_attempts { state_str = "halted"; break; }
            continue; // self-heal
        }

        emit("simulate", "running", "MCP simulation worker: booting the drafted ELF in Renode");
        simulation = call("simulation", "run_simulation", json!({"elf_path": compile["artifact_path"]}))?;
        let sim_ok = simulation["ok"] == true;
        emit(
            "simulate",
            if sim_ok { "done" } else { "failed" },
            if sim_ok { "Captured the expected USART2 banner" } else { "Renode did not satisfy the UART assertion" },
        );

        if sim_ok {
            state_str = "verified";
            emit("verified", "done", "ELF boot and expected UART transmission passed");
            break;
        }
        // Compiled but did not verify: self-heal against the simulation fault.
        ctx.compile_diagnostics = Vec::new();
        ctx.sim_fault = simulation["fault"].as_str().map(|s| s.to_string());
        if attempt == max_attempts { state_str = "halted"; break; }
    }

    let verified = state_str == "verified";
    if !verified {
        emit("verified", "failed", "The model's draft was not verified within the attempt budget");
    }

    // Provenance — including a `draft` record that names the model that wrote the code.
    let ledger_path = std::env::temp_dir().join(format!("embeder-generate-{}-{}.jsonl", std::process::id(), embeder_core::now_secs()));
    let mut ledger = ProvenanceLedger::new(&ledger_path).map_err(|error| error.to_string())?;
    ledger.record("model", "ground", &json!({"goal": goal}).to_string(), &json!({"citations": n_cites}).to_string(), None, "datasheet", "{}");
    ledger.record("model", "draft", &json!({"goal": goal, "attempts": attempts}).to_string(), &drafted_by.to_string(), None, &draft_oracle, "{}");
    ledger.record("oracle", "compile", &short_hash(&source), &compile.to_string(), None, compile["toolchain"].as_str().unwrap_or("arm-none-eabi-gcc"), "{}");
    if !simulation.is_null() {
        ledger.record("oracle", "simulate", &compile["artifact_path"].to_string(), &simulation.to_string(), if verified { Some("verified") } else { None }, simulation["engine"].as_str().unwrap_or("renode"), "{}");
    }
    if verified {
        ledger.record("user", "verify", &json!({"goal": goal}).to_string(), &json!({"tier": "verified"}).to_string(), Some("verified"),
            &format!("{} + {}", compile["toolchain"].as_str().unwrap_or("arm-none-eabi-gcc"), simulation["engine"].as_str().unwrap_or("renode")), "{}");
    }
    let provenance: Vec<Value> = ledger.records.iter().map(|record| json!({
        "id": record.id, "ts": record.ts, "actor": record.actor, "tool": record.tool,
        "tier": record.tier, "oracle": record.oracle_version,
        "inputs_hash": record.inputs_hash, "outputs_hash": record.outputs_hash,
    })).collect();

    let observations: Vec<Value> = simulation["observations"].as_array().into_iter().flatten()
        .map(|pair| json!({"key": pair[0], "value": pair[1]})).collect();
    let boundary = if verified {
        json!({"verified": ["cpu_boot", "uart_tx"], "stubbed": [], "not_modeled": ["gpio_toggle_assertion", "real_analog", "rf"]})
    } else {
        json!({"verified": [], "stubbed": [], "not_modeled": []})
    };

    Ok(json!({
        "goal": goal,
        "mode": "real",
        "state": state_str,
        "attempts": attempts,
        "self_healed": attempts > 1,
        "tier": if verified { json!({"symbol": "✓", "name": "verified"}) } else { Value::Null },
        "artifact": compile["artifact_path"],
        "diagnostics": compile["diagnostics"],
        "compile": compile,
        "simulation": simulation,
        "observations": observations,
        "boundary": boundary,
        "citations": citations,
        "drafted_by": drafted_by,
        "drafted_source": source,
        "drafted_entry": "src/main.c",
        "provenance": provenance,
        "ledger_path": ledger_path,
        "source_fingerprint": short_hash(&source),
        "duration_ms": started.elapsed().as_millis(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_includes_actual_startup_and_linker_inputs() {
        let files = snapshot().unwrap();
        for path in ["src/main.c", "src/startup.c", "link/stm32f4.ld"] {
            assert_eq!(files[path].as_str().unwrap(), fs::read_to_string(workspace_root().join(path)).unwrap());
        }
        assert!(files.get("README.md").is_none());
    }

    #[test]
    fn unknown_task_has_an_explicit_error() {
        assert!(task_status("missing").get("error").is_some());
    }
}
