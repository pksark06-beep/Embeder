# SPDX-License-Identifier: MPL-2.0
"""Minimal MCP stdio server scaffold (ADR-0001).

Speaks the MCP wire protocol: newline-delimited JSON-RPC 2.0 over stdin/stdout,
with the initialize / notifications/initialized / tools/list / tools/call methods.
Pure stdlib and protocol-compatible with any MCP client (FastMCP is a drop-in
replacement that emits the same wire messages).

A tool is registered as: name -> {"fn": callable(**args)->dict, "schema": {...}, "desc": str}
"""
import sys
import json

PROTOCOL_VERSION = "2024-11-05"
MAX_REQUEST_BYTES = 2 * 1024 * 1024


def _send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def serve(server_name, tools):
    # NOTE: readline() (not `for line in sys.stdin`) to avoid read-ahead buffering
    # that would deadlock a pipe.
    while True:
        line = sys.stdin.readline()
        if not line:
            break  # EOF: client closed
        if len(line.encode("utf-8", errors="ignore")) > MAX_REQUEST_BYTES:
            _send({"jsonrpc": "2.0", "id": None,
                   "error": {"code": -32600, "message": "request exceeds MCP sandbox limit"}})
            continue
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue

        mid = msg.get("id")
        method = msg.get("method")
        params = msg.get("params") or {}

        if method == "initialize":
            _send({
                "jsonrpc": "2.0", "id": mid,
                "result": {
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": server_name, "version": "0.1"},
                },
            })
        elif method == "notifications/initialized":
            pass  # notification: no response
        elif method == "tools/list":
            _send({
                "jsonrpc": "2.0", "id": mid,
                "result": {"tools": [
                    {"name": n, "description": t.get("desc", ""), "inputSchema": t.get("schema", {"type": "object"})}
                    for n, t in tools.items()
                ]},
            })
        elif method == "tools/call":
            name = params.get("name")
            args = params.get("arguments") or {}
            tool = tools.get(name)
            if tool is None:
                _send({"jsonrpc": "2.0", "id": mid,
                       "error": {"code": -32601, "message": f"unknown tool: {name}"}})
                continue
            try:
                result = tool["fn"](**args)
                _send({"jsonrpc": "2.0", "id": mid,
                       "result": {"content": [{"type": "text", "text": json.dumps(result)}],
                                  "isError": False}})
            except Exception as e:  # noqa: BLE001 - surface any tool failure as an error result
                _send({"jsonrpc": "2.0", "id": mid,
                       "result": {"content": [{"type": "text", "text": json.dumps({"error": str(e)})}],
                                  "isError": True}})
        elif method in ("shutdown", "exit"):
            break
        # other notifications are ignored
