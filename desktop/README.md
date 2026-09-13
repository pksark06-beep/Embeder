# desktop/ — the Embeder UI shell (P3, ADR-0006)

An instrument-panel UI over the working Core: a signal-flow pipeline
(INTENT → GROUND → COMPILE → SIMULATE → VERIFIED), a tier verdict, the verifiability
boundary, grounded citations, and the provenance ledger.

```
dist/         Static frontend (HTML/CSS/JS). Loaded by BOTH runtimes below.
server/       Rust dev server (workspace member): serves dist/ + GET /api/run,
              which drives the grounded loop (fixture oracles) and returns JSON.
src-tauri/    Tauri native shell (ADR-0006). Excluded from the workspace.
```

The frontend auto-detects its runtime: inside Tauri it calls `invoke("run_verification")`;
in the browser it `fetch("/api/run")`. Same UI, same Core, two hosts.

## Run now (dev server — no extra toolchains)

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
cargo run -p embeder-desktop          # then open http://127.0.0.1:8787
```

## Run as the native app (Tauri)

Tauri's native build on Windows needs the **MSVC** toolchain + WebView2, but this
machine's Rust host is `x86_64-pc-windows-gnu`. To build the native shell:

```powershell
# install VS Build Tools (C++), then:
rustup toolchain install stable-x86_64-pc-windows-msvc
cargo install tauri-cli --version "^2"
cd desktop/src-tauri
cargo +stable-x86_64-pc-windows-msvc tauri dev
```

The Tauri command layer (`src-tauri/src/main.rs`) mirrors the dev server's `/api/run`,
so the UI behaves identically in both. Until MSVC is available, use the dev server —
it exercises the exact same frontend and Core path.
