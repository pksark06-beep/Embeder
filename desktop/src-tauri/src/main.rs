//! Embeder native desktop shell (ADR-0006). The window loads `desktop/dist`; the
//! frontend calls these commands via `invoke`, which delegate to the shared desktop
//! API (identical to the dev server's HTTP endpoints). Presentation lives in the
//! webview; all execution stays in the Core.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde_json::Value;

#[tauri::command]
async fn project() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(embeder_desktop_api::project_info).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn peripherals() -> Value {
    embeder_desktop_api::peripherals()
}

#[tauri::command]
fn register_map(peripheral: String) -> Value {
    embeder_desktop_api::register_map(Some(&peripheral))
}

#[tauri::command]
fn firmware() -> Value {
    embeder_desktop_api::firmware()
}

#[tauri::command]
fn workspace_files() -> Value {
    embeder_desktop_api::workspace_files()
}

#[tauri::command]
fn read_workspace_file(path: String) -> Value {
    embeder_desktop_api::read_workspace_file(Some(&path))
}

#[tauri::command]
fn save_workspace_file(path: String, contents: String) -> Value {
    embeder_desktop_api::save_workspace_file(Some(&path), &contents)
}

#[tauri::command]
async fn mcp_status() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(embeder_desktop_api::mcp_status).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn probe_mcp(server: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || embeder_desktop_api::probe_mcp(Some(&server))).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn run_verification() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(embeder_desktop_api::run_verification).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn start_task(simulate: bool) -> Value {
    embeder_desktop_api::start_task(simulate)
}

#[tauri::command]
fn generate(goal: String) -> Value {
    embeder_desktop_api::start_generate_task(goal)
}

#[tauri::command]
fn task_status(id: String) -> Value {
    embeder_desktop_api::task_status(&id)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(index) = args.iter().position(|argument| argument == "--mcp-server") {
        let kind = args.get(index + 1).map(String::as_str).unwrap_or("");
        if let Err(error) = embeder_desktop_api::serve_mcp_stdio(kind) {
            eprintln!("MCP worker failed: {error}");
            std::process::exit(2);
        }
        return;
    }
    tauri::Builder::default()
        .setup(|app| {
            // Make the Core's resource + workspace locations relocatable. The packaged
            // app carries the SVD and firmware template as bundle resources; the user's
            // editable workspace lives under the per-user app-data dir, seeded once from
            // that template. Child MCP workers (spawned via current_exe) inherit these.
            use tauri::Manager;
            let resource = app.path().resource_dir()?;
            let data = app.path().app_data_dir()?;
            let workspace = data.join("workspace").join("stm32-blink-uart");
            std::env::set_var("EMBEDER_SVD", resource.join("data").join("stm32f4-mini.svd"));
            std::env::set_var("EMBEDER_WORKSPACE_DIR", &workspace);
            // The BYOK codegen server is bundled as a resource; point the app at it so
            // an installed build runs the real model instead of falling back to synthesis.
            std::env::set_var("EMBEDER_SERVERS_DIR", resource.join("servers"));
            embeder_desktop_api::ensure_workspace_seeded(
                &resource.join("template").join("stm32-blink-uart"),
            )?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            project,
            peripherals,
            register_map,
            firmware,
            workspace_files,
            read_workspace_file,
            save_workspace_file,
            mcp_status,
            probe_mcp,
            start_task,
            generate,
            task_status,
            run_verification
        ])
        .run(tauri::generate_context!())
        .expect("error while running Embeder");
}
