//! Explicit integration checks: real desktop executable, MCP, GCC and Renode.
//! cargo test -p embeder-desktop --test real_desktop -- --ignored
use embeder_mcp::McpClient;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); }
}

fn request(port: u16, method: &str, path: &str) -> Value {
    request_body(port, method, path, "")
}

fn request_body(port: u16, method: &str, path: &str, body: &str) -> Value {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (_, body) = response.split_once("\r\n\r\n").unwrap();
    serde_json::from_str(body).unwrap()
}

#[test]
#[ignore = "requires real ARM GCC and Renode"]
fn desktop_task_builds_saved_files_and_verifies_real_uart() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let _server = Server(Command::new(env!("CARGO_BIN_EXE_embeder-desktop"))
        .env("EMBEDER_ADDR", format!("127.0.0.1:{port}"))
        .stdout(Stdio::null()).spawn().unwrap());
    let started = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(50));
    }
    let task = request(port, "POST", "/api/tasks?simulate=true");
    let id = task["id"].as_str().expect("scheduled task");
    let mut events = Vec::new();
    let result = loop {
        let task = request(port, "GET", &format!("/api/task?id={id}"));
        if let Some(items) = task["events"].as_array() { events = items.clone(); }
        if task["status"] == "completed" { break task["result"].clone(); }
        assert!(started.elapsed() < Duration::from_secs(240), "task timed out");
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(result["mode"], "real", "{result}");
    assert_eq!(result["state"], "verified", "{result}");
    assert_eq!(result["attempts"], 1);
    assert_eq!(result["self_healed"], false);
    assert!(std::path::Path::new(result["artifact"].as_str().unwrap()).is_file());
    assert!(result["observations"].to_string().contains("Hello from Embeder"));
    assert!(events.iter().any(|event| event["stage"] == "compile" && event["status"] == "done"));
    assert!(events.iter().any(|event| event["stage"] == "simulate" && event["status"] == "done"));
    assert_eq!(result["boundary"]["verified"], json!(["cpu_boot", "uart_tx"]));
    assert!(result["provenance"].as_array().unwrap().len() >= 2);
}

#[test]
#[ignore = "requires real ARM GCC and Renode"]
fn desktop_generate_task_drafts_and_verifies() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    // provider=none forces the deterministic grounded fallback: this exercises the full
    // generate -> compile -> simulate -> verify task without a network or API key.
    let _server = Server(Command::new(env!("CARGO_BIN_EXE_embeder-desktop"))
        .env("EMBEDER_ADDR", format!("127.0.0.1:{port}"))
        .env("EMBEDER_LLM_PROVIDER", "none")
        .stdout(Stdio::null()).spawn().unwrap());
    let started = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(50));
    }
    let task = request_body(port, "POST", "/api/generate", "Blink PA5 and print 'Hello from Embeder' over USART2");
    let id = task["id"].as_str().expect("scheduled task");
    let mut events = Vec::new();
    let result = loop {
        let task = request(port, "GET", &format!("/api/task?id={id}"));
        if let Some(items) = task["events"].as_array() { events = items.clone(); }
        if task["status"] == "completed" { break task["result"].clone(); }
        assert!(started.elapsed() < Duration::from_secs(240), "task timed out");
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(result["state"], "verified", "{result}");
    assert_eq!(result["drafted_by"]["used_llm"], false, "provider=none must use the fallback: {result}");
    assert!(result["drafted_source"].as_str().unwrap().contains("int main"), "a draft must be returned");
    assert!(result["observations"].to_string().contains("Hello from Embeder"));
    assert!(events.iter().any(|event| event["stage"] == "ground" && event["status"] == "done"));
    assert!(
        result["provenance"].as_array().unwrap().iter().any(|record| record["tool"] == "draft"),
        "provenance must record which model drafted the code: {result}"
    );
}

#[test]
#[ignore = "requires real ARM GCC"]
fn mcp_compiles_a_snapshot_and_rejects_real_source_errors() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../firmware/stm32-blink-uart");
    let mut files = serde_json::Map::new();
    for path in ["src/main.c", "src/startup.c", "link/stm32f4.ld"] {
        files.insert(path.into(), json!(std::fs::read_to_string(root.join(path)).unwrap()));
    }
    let args = vec!["--mcp-server".into(), "firmware".into()];
    let mut client = McpClient::spawn(env!("CARGO_BIN_EXE_embeder-desktop"), &args).unwrap();
    let valid = client.call_tool("compile_firmware", json!({"files": files, "workspace_snapshot": true})).unwrap();
    assert_eq!(valid["ok"], true, "{valid}");
    files.insert("src/main.c".into(), json!("#error EMBEDER_REAL_FAILURE_PROBE\n"));
    let broken = client.call_tool("compile_firmware", json!({"files": files, "workspace_snapshot": true})).unwrap();
    assert_eq!(broken["ok"], false, "{broken}");
    assert!(broken["artifact_path"].is_null());
    assert!(broken["diagnostics"].to_string().contains("EMBEDER_REAL_FAILURE_PROBE"));
    // Linker script edits are part of the same snapshot, too.
    files.insert("src/main.c".into(), json!(std::fs::read_to_string(root.join("src/main.c")).unwrap()));
    files.insert("link/stm32f4.ld".into(), json!("THIS IS NOT A LINKER SCRIPT"));
    let broken_link = client.call_tool("compile_firmware", json!({"files": files, "workspace_snapshot": true})).unwrap();
    assert_eq!(broken_link["ok"], false, "{broken_link}");
}
