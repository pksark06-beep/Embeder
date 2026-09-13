//! A minimal, blocking MCP stdio client: spawn a server, perform the initialize
//! handshake, and call tools. Newline-delimited JSON-RPC 2.0 — the MCP wire protocol.

use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const PROTOCOL_VERSION: &str = "2024-11-05";

pub struct McpClient {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    next_id: i64,
}

impl McpClient {
    /// Spawn a server process and complete the MCP handshake.
    pub fn spawn(program: &str, args: &[String]) -> io::Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let reader = BufReader::new(stdout);

        let mut client = McpClient { child, stdin, reader, next_id: 0 };
        client.initialize()?;
        Ok(client)
    }

    fn send(&mut self, v: &Value) -> io::Result<()> {
        writeln!(self.stdin, "{}", v)?;
        self.stdin.flush()
    }

    fn read_message(&mut self) -> io::Result<Value> {
        loop {
            let mut line = String::new();
            let n = self.reader.read_line(&mut line)?;
            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "MCP server closed the pipe"));
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            return serde_json::from_str(trimmed)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
        }
    }

    fn request(&mut self, method: &str, params: Value) -> io::Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        loop {
            let msg = self.read_message()?;
            if msg.get("id").and_then(|v| v.as_i64()) == Some(id) {
                return Ok(msg);
            }
            // Notifications or out-of-band messages: ignore and keep reading.
        }
    }

    fn notify(&mut self, method: &str) -> io::Result<()> {
        self.send(&json!({"jsonrpc": "2.0", "method": method}))
    }

    fn initialize(&mut self) -> io::Result<()> {
        self.request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "embeder", "version": "0.1"}
            }),
        )?;
        self.notify("notifications/initialized")
    }

    /// Call a tool and return its (JSON) result payload, unwrapped from the MCP
    /// content envelope.
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> io::Result<Value> {
        let resp = self.request("tools/call", json!({"name": name, "arguments": arguments}))?;
        if let Some(err) = resp.get("error") {
            return Err(io::Error::new(io::ErrorKind::Other, format!("tool error: {}", err)));
        }
        let text = resp
            .get("result")
            .and_then(|r| r.get("content"))
            .and_then(|c| c.get(0))
            .and_then(|c0| c0.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("{}");
        serde_json::from_str(text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
