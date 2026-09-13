//! Embeder desktop dev server.
//!
//! Serves the static UI (`desktop/dist`) and exposes `GET /api/run`, which drives the
//! grounded verification loop with fixture oracles (no toolchains needed) and returns
//! the outcome + provenance as JSON. This is the verifiable stand-in for the Tauri
//! command layer: the same frontend runs inside the Tauri shell via `invoke`.

use embeder_core::{
    run_loop, FixtureCompileOracle, FixtureSimOracle, LoopConfig, LoopState, ProvenanceLedger,
};
use embeder_datasheet::{DatasheetIndex, GroundedCodegen};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <root>/desktop/server
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn dist_dir() -> PathBuf {
    repo_root().join("desktop").join("dist")
}

fn svd_path() -> PathBuf {
    repo_root().join("datasheet").join("data").join("stm32f4-mini.svd")
}

fn main() {
    let addr = std::env::var("EMBEDER_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("cannot bind {addr}: {e}");
        std::process::exit(1);
    });
    println!("Embeder desktop server → http://{addr}   (serving {})", dist_dir().display());
    for stream in listener.incoming().flatten() {
        handle(stream);
    }
}

fn handle(mut stream: TcpStream) {
    let peer = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut reader = BufReader::new(peer);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }
    // Drain headers.
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).unwrap_or(0);
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    let path = parts.get(1).copied().unwrap_or("/");

    if path.starts_with("/api/run") {
        let body = run_verification().to_string();
        respond(&mut stream, "200 OK", "application/json; charset=utf-8", body.as_bytes());
    } else {
        serve_static(path, &mut stream);
    }
}

fn serve_static(path: &str, stream: &mut TcpStream) {
    let rel = if path == "/" { "index.html" } else { path.trim_start_matches('/') };
    // Prevent path traversal.
    if rel.contains("..") {
        respond(stream, "400 Bad Request", "text/plain", b"bad path");
        return;
    }
    let file = dist_dir().join(rel);
    match std::fs::read(&file) {
        Ok(bytes) => respond(stream, "200 OK", content_type(&file), &bytes),
        Err(_) => respond(stream, "404 Not Found", "text/plain", b"not found"),
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

/// Run the grounded verification loop with fixture oracles and serialize the result.
fn run_verification() -> Value {
    let idx = match DatasheetIndex::from_svd_file(svd_path()) {
        Ok(i) => i,
        Err(e) => return json!({"error": format!("cannot load SVD: {e}")}),
    };
    let mut codegen = GroundedCodegen::new(&idx);
    let ledger_path = std::env::temp_dir().join("embeder-desktop-prov.jsonl");
    let _ = std::fs::remove_file(&ledger_path);
    let mut ledger = match ProvenanceLedger::new(&ledger_path) {
        Ok(l) => l,
        Err(e) => return json!({"error": format!("ledger: {e}")}),
    };

    let outcome = run_loop(
        "Blink an LED and print over UART on STM32",
        &mut codegen,
        &FixtureCompileOracle,
        &FixtureSimOracle,
        &mut ledger,
        &LoopConfig::default(),
    );

    let tier = outcome.tier.map(|t| json!({"symbol": t.symbol(), "name": t.as_str()}));
    let boundary = outcome.boundary.as_ref().map(|b| {
        json!({"verified": b.verified, "stubbed": b.stubbed, "not_modeled": b.not_modeled})
    });
    let citations: Vec<Value> = outcome
        .citations
        .iter()
        .map(|c| json!({"source": c.source, "locator": c.locator, "summary": c.summary}))
        .collect();
    let observations: Vec<Value> = outcome
        .observations
        .iter()
        .map(|(k, v)| json!({"key": k, "value": v}))
        .collect();
    let provenance: Vec<Value> = ledger
        .records
        .iter()
        .map(|r| {
            json!({
                "id": r.id, "ts": r.ts, "actor": r.actor, "tool": r.tool,
                "tier": r.tier, "oracle": r.oracle_version,
                "inputs_hash": r.inputs_hash, "outputs_hash": r.outputs_hash
            })
        })
        .collect();

    json!({
        "goal": "Blink an LED and print over UART on STM32",
        "state": match outcome.state {
            LoopState::Verified => "verified",
            LoopState::Halted => "halted",
            LoopState::Built => "built",
            LoopState::Draft => "draft",
        },
        "tier": tier,
        "attempts": outcome.attempts,
        "self_healed": outcome.attempts > 1,
        "boundary": boundary,
        "citations": citations,
        "observations": observations,
        "artifact": outcome.artifact_path,
        "provenance": provenance,
    })
}
