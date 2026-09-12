# ADR-0006 — Tauri shell + client–daemon process model

- **Status:** Accepted
- **Date:** 2026-09-12

## Context
Compilation and simulation are long, heavy, cancelable background jobs. The UI must
stay responsive and must **never** execute a toolchain itself (that would make
actions unloggable and unpermissioned). We want native performance and a small
footprint, with a web frontend for the rich panels (editor, canvas, PCB preview).

## Decision
- **UI Shell:** Tauri (Rust core + web frontend). Presentation only.
- **Core / Desktop Engine (Rust):** long-lived process that owns session/workspace
  state, the MCP orchestrator, BYOK secret access, the provenance ledger, and tier
  assignment. It is the only component that starts toolchain workers.
- **Toolchain workers:** short-lived sandboxed subprocesses spawned via MCP servers
  (see ADR-0008).
- UI ↔ Core communication is typed (Tauri commands + event streams for live
  compile/sim logs).

## Consequences
- (+) UI responsive during heavy jobs; jobs are cancelable and streamed.
- (+) Single choke point (Core) for logging, permissions, and tiering.
- (+) Small footprint vs. Electron; native Rust for the orchestration hot path.
- (−) Rust + web split has a steeper learning curve than an all-JS stack. Accepted
  for footprint, safety, and process-control fidelity.

## Alternatives rejected
- **Electron:** heavier footprint; weaker native process/sandbox control.
- **UI runs tools directly (no daemon):** breaks the audit/permission boundary that
  the whole trust model (ADR-0008) depends on.
