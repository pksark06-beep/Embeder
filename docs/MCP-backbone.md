# MCP Backbone (ADR-0001) — component design

> Status: **Firmware (compile) tool now runs over MCP**; Renode subprocess bounded by
> a wall-clock timeout. Simulation + Datasheet MCP servers: next. Date: 2026-09-13.

## 1. Why
ADR-0001 makes MCP the tool backbone: a uniform, schema-enforced, loggable boundary
between "model intent" and "tool execution," so tools are inspectable and third
parties can add servers without touching the Core. Until now the oracles shelled out
to toolchains directly (a tracked debt). This pays it back, starting with compile.

## 2. Shape
```
Rust Core (MCP client)  ──stdio JSON-RPC──►  Python MCP server  ──►  native toolchain
  embeder-mcp crate                           servers/*.py             arm-none-eabi-gcc
  McpCompileOracle: CompileOracle             mcp_lib.serve()
```
- **Transport:** newline-delimited JSON-RPC 2.0 over the server's stdin/stdout — the
  MCP wire protocol (`initialize` → `notifications/initialized` → `tools/call`).
- **Client:** `mcp/src/client.rs` — minimal blocking client (spawn, handshake, call).
- **Oracle:** `McpCompileOracle` implements the same `core::CompileOracle` trait, so it
  drops into the verification loop with **no loop changes**.
- **Servers:** pure-stdlib Python (`servers/mcp_lib.py` + `servers/firmware_server.py`).
  Protocol-compatible with FastMCP (a drop-in that emits the same wire messages); we
  keep stdlib to avoid dependency/version drift and to own the boundary.

## 3. Division of labor
The server is a thin, sandboxable **executor**: it runs the toolchain and returns raw
results (returncode, stdout, stderr, artifact path). The Core **interprets**: it parses
stderr into structured diagnostics with its own tested `parse_gcc_stderr`. Interpretation
stays in one place; the server stays trivial and language-agnostic.

## 4. Proof
```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;<arm bin>;$env:Path"
cargo run -p embeder-mcp --bin mcp_loop            # MCP compile + fixture sim
cargo run -p embeder-mcp --bin mcp_loop -- --real  # MCP compile + real Renode
cargo test -p embeder-mcp --test roundtrip -- --ignored   # 2 round-trip tests
```
`mcp_loop --real`: intent → **(MCP) compile** with real arm-none-eabi-gcc → self-heal
on diagnostics that survived the round trip → real Renode → **VERIFIED**. The ELF is
built by the server process; the diagnostic that drives self-heal crossed the protocol
boundary intact.

## 5. Renode timeout
`RenodeOracle` now runs the Renode subprocess under a wall-clock guard
(`timeout_secs`, default 180) with background pipe draining (no deadlock on a chatty
child); on timeout it kills the process and returns `fault: "timeout"`. Generous by
design — a safety net against a true hang, never a performance bound.

## 6. Testing note
Real-toolchain / MCP e2e tests are `#[ignore]` and best run **single-threaded**
(`-- --test-threads=1`): a Renode cold-start contended by parallel arm-gcc builds can
be slow, and we do not want test parallelism perturbing timing.

## 7. Next
- **Simulation MCP server:** wrap Renode behind MCP (`McpSimOracle`), same pattern.
- **Datasheet MCP server:** expose grounded lookups over MCP; note the logic already
  exists as the tested Rust `datasheet` crate, so decide server-wraps-crate vs. a
  Python re-implementation (leaning: thin server over the Rust crate to avoid a second
  SVD parser).
- **Sandboxing (ADR-0008):** fs-jail workers to the workspace; no ambient net.
