# desktop/ — the Embeder UI shell (P3, ADR-0006)

A VS Code-inspired embedded workbench over the Core: sandboxed workspace editing,
file tabs, a command palette, multi-view navigation, MCP extension discovery, the
closed verification loop, grounded citations, and the provenance ledger.

```
dist/         Static frontend (HTML/CSS/JS). Loaded by BOTH runtimes below.
server/       Rust dev server (workspace member): serves dist/ + the shared API.
src-tauri/    Tauri native shell (ADR-0006). Excluded from the workspace.
```

The frontend auto-detects its runtime. Inside Tauri it uses typed commands; in the
browser it uses matching HTTP routes for project state, workspace files, datasheets,
MCP discovery/probes, and verification. Both hosts call the same `desktop/api`
functions, so loading, success, and error states stay aligned.

The desktop executable is also its own MCP worker. The Core starts a fresh process
with `--mcp-server datasheet|firmware|simulation`, completes the MCP handshake over
stdio, discovers tools, calls them, and tears the process down. This removes the
Python runtime requirement for the MVP. File access is restricted to
`firmware/stm32-blink-uart`; generated build files are restricted to temporary
directories; unknown servers, path traversal, and oversized editor payloads are
refused. This is the MVP's application-level sandbox: the exposed tools have no
network operation, but OS-level process/network isolation remains a later hardening
step.

## Run now (dev server)

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
cargo run -p embeder-desktop          # then open http://127.0.0.1:8787
```

The workbench Build command snapshots the saved workspace, invokes the native
firmware MCP worker, and runs `arm-none-eabi-gcc`. Build & Simulate also invokes
Renode and checks the USART2 banner. Open a new terminal after installing the
toolchain so its user PATH is visible to the dev server.

## Run as the native app (Tauri)

Tauri's native build on Windows uses the **MSVC** toolchain + WebView2. This
checkout has the MSVC toolchain installed. To build and run the native shell:

```powershell
cargo +stable-x86_64-pc-windows-msvc build --manifest-path desktop/src-tauri/Cargo.toml
cargo +stable-x86_64-pc-windows-msvc run --manifest-path desktop/src-tauri/Cargo.toml
```

The Tauri command layer mirrors every dev-server route. The default desktop task
is real. The Core fixture loop remains available for unit tests and deterministic
development checks; it is not used by the workbench Build or Build & Simulate
actions. See [ARCHITECTURE.md](ARCHITECTURE.md) for service boundaries and the
remaining IDE work.
