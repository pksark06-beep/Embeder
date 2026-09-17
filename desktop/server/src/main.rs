//! Embeder desktop dev server — a thin std-only HTTP layer over the shared desktop
//! API. Serves `desktop/dist` and exposes:
//!   GET /api/project · /api/peripherals · /api/register_map?peripheral=NAME
//!   GET /api/firmware · /api/run · /api/workspace · /api/file
//!   GET /api/mcp · /api/mcp_probe · POST /api/file
//!   POST /api/tasks (build/simulate saved sources) · POST /api/generate (model drafts + verifies)
//!
//! The Tauri command layer calls the same `embeder_desktop_api` functions, so the UI
//! is identical in either host.

use embeder_desktop_api as api;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

fn dist_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.join("dist"))
        .unwrap_or_else(|| PathBuf::from("dist"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(index) = args.iter().position(|argument| argument == "--mcp-server") {
        let kind = args.get(index + 1).map(String::as_str).unwrap_or("");
        if let Err(error) = api::serve_mcp_stdio(kind) {
            eprintln!("MCP worker failed: {error}");
            std::process::exit(2);
        }
        return;
    }
    let addr = std::env::var("EMBEDER_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("cannot bind {addr}: {e}");
        std::process::exit(1);
    });
    println!("Embeder desktop → http://{addr}   (serving {})", dist_dir().display());
    for stream in listener.incoming().flatten() {
        std::thread::spawn(move || handle(stream));
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
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).unwrap_or(0);
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }

    if content_length > 512 * 1024 {
        respond(&mut stream, "413 Payload Too Large", "text/plain", b"request too large");
        return;
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 && reader.read_exact(&mut body).is_err() {
        respond(&mut stream, "400 Bad Request", "text/plain", b"incomplete body");
        return;
    }

    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or("GET");
    let path = request_parts.next().unwrap_or("/");
    let (route, query) = path.split_once('?').unwrap_or((path, ""));
    let path_param = query_param(query, "path");
    let server_param = query_param(query, "server");
    let peripheral_param = query_param(query, "peripheral");

    match (method, route) {
        ("GET", "/api/project") => json_ok(&mut stream, api::project_info()),
        ("GET", "/api/peripherals") => json_ok(&mut stream, api::peripherals()),
        ("GET", "/api/register_map") => {
            json_ok(&mut stream, api::register_map(peripheral_param.as_deref()))
        }
        ("GET", "/api/firmware") => json_ok(&mut stream, api::firmware()),
        ("GET", "/api/workspace") => json_ok(&mut stream, api::workspace_files()),
        ("GET", "/api/file") => json_ok(&mut stream, api::read_workspace_file(path_param.as_deref())),
        ("POST", "/api/file") => json_ok(
            &mut stream,
            api::save_workspace_file(path_param.as_deref(), &String::from_utf8_lossy(&body)),
        ),
        ("GET", "/api/mcp") => json_ok(&mut stream, api::mcp_status()),
        ("GET", "/api/mcp_probe") => json_ok(&mut stream, api::probe_mcp(server_param.as_deref())),
        ("POST", "/api/tasks") => json_ok(&mut stream, api::start_task(query_param(query, "simulate").as_deref() != Some("false"))),
        ("POST", "/api/generate") => json_ok(&mut stream, api::start_generate_task(String::from_utf8_lossy(&body).into_owned())),
        ("GET", "/api/task") => json_ok(&mut stream, api::task_status(&query_param(query, "id").unwrap_or_default())),
        ("POST", "/api/run") => json_ok(&mut stream, api::run_verification()),
        ("GET", _) => serve_static(route, &mut stream),
        _ => respond(&mut stream, "405 Method Not Allowed", "text/plain", b"method not allowed"),
    }
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| percent_decode(v))
    })
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = &value[index + 1..index + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                output.push(byte);
                index += 3;
                continue;
            }
        }
        output.push(if bytes[index] == b'+' { b' ' } else { bytes[index] });
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn serve_static(path: &str, stream: &mut TcpStream) {
    let rel = if path == "/" { "index.html" } else { path.trim_start_matches('/') };
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
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn json_ok(stream: &mut TcpStream, v: Value) {
    respond(stream, "200 OK", "application/json; charset=utf-8", v.to_string().as_bytes());
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
