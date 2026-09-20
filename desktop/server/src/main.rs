// SPDX-License-Identifier: MPL-2.0
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
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};

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
    let bind_addr: SocketAddr = addr.parse().unwrap_or_else(|_| {
        eprintln!("EMBEDER_ADDR must be a loopback IP address and port");
        std::process::exit(1);
    });
    if !bind_addr.ip().is_loopback() {
        eprintln!("Embeder's unauthenticated development server may only bind to loopback");
        std::process::exit(1);
    }
    let listener = TcpListener::bind(bind_addr).unwrap_or_else(|e| {
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
    if request_line.len() > 8192 {
        respond(&mut stream, "414 URI Too Long", "text/plain", b"request line too long");
        return;
    }
    let mut content_length = 0usize;
    let mut header_bytes = 0usize;
    let mut host = None;
    let mut origin = None;
    let mut fetch_site = None;
    let mut local_client = false;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).unwrap_or(0);
        header_bytes += n;
        if header_bytes > 32 * 1024 {
            respond(&mut stream, "431 Request Header Fields Too Large", "text/plain", b"headers too large");
            return;
        }
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            } else if name.eq_ignore_ascii_case("host") {
                host = Some(value.trim().to_string());
            } else if name.eq_ignore_ascii_case("origin") {
                origin = Some(value.trim().to_string());
            } else if name.eq_ignore_ascii_case("sec-fetch-site") {
                fetch_site = Some(value.trim().to_string());
            } else if name.eq_ignore_ascii_case("x-embeder-client") {
                local_client = value.trim() == "1";
            }
        }
    }

    let port = match stream.local_addr() {
        Ok(addr) => addr.port(),
        Err(_) => return,
    };
    if !host.as_deref().is_some_and(|value| is_local_host(value, port))
        || origin.as_deref().is_some_and(|value| {
            !value.strip_prefix("http://").is_some_and(|host| is_local_host(host, port))
        })
        || fetch_site.as_deref() == Some("cross-site")
    {
        respond(&mut stream, "403 Forbidden", "text/plain", b"local origin required");
        return;
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
    if method == "POST" && !local_client {
        respond(&mut stream, "403 Forbidden", "text/plain", b"local client header required");
        return;
    }
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

fn is_local_host(host: &str, port: u16) -> bool {
    host.eq_ignore_ascii_case(&format!("localhost:{port}"))
        || host == format!("127.0.0.1:{port}")
        || host == format!("[::1]:{port}")
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
    if !safe_static_relative_path(rel) {
        respond(stream, "400 Bad Request", "text/plain", b"bad path");
        return;
    }
    let Ok(base) = dist_dir().canonicalize() else {
        respond(stream, "404 Not Found", "text/plain", b"not found");
        return;
    };
    let Ok(file) = base.join(rel).canonicalize() else {
        respond(stream, "404 Not Found", "text/plain", b"not found");
        return;
    };
    if !file.starts_with(&base) || !file.is_file() {
        respond(stream, "400 Bad Request", "text/plain", b"bad path");
        return;
    }
    match std::fs::read(&file) {
        Ok(bytes) => respond(stream, "200 OK", content_type(&file), &bytes),
        Err(_) => respond(stream, "404 Not Found", "text/plain", b"not found"),
    }
}

fn safe_static_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && Path::new(path).components().all(|component| matches!(component, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::{is_local_host, safe_static_relative_path};

    #[test]
    fn only_loopback_hosts_are_accepted() {
        assert!(is_local_host("127.0.0.1:8787", 8787));
        assert!(is_local_host("localhost:8787", 8787));
        assert!(!is_local_host("example.com:8787", 8787));
        assert!(!is_local_host("127.0.0.1:9999", 8787));
    }

    #[test]
    fn static_paths_cannot_escape_the_ui_directory() {
        assert!(safe_static_relative_path("app.js"));
        assert!(!safe_static_relative_path("../README.md"));
        assert!(!safe_static_relative_path("C:\\Windows\\win.ini"));
        assert!(!safe_static_relative_path("\\\\server\\share"));
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
