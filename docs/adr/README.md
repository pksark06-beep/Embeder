# Architecture Decision Records — Embeder

Each ADR records one load-bearing decision: its context, the decision, the
consequences, and the alternatives that were rejected and *why*. ADRs are immutable
once **Accepted** — to change a decision, write a new ADR that supersedes the old one.

**Status values:** `Proposed` (awaiting your ratification) · `Accepted` ·
`Superseded by ADR-XXXX` · `Rejected`.

| ADR | Title | Status | Your call? |
|---|---|---|---|
| [0001](ADR-0001-mcp-tool-backbone.md) | MCP as the tool backbone | Accepted | — |
| [0002](ADR-0002-datasheet-grounding.md) | Structured-first datasheet grounding (SVD over RAG) | Accepted | OD-3 ✓ |
| [0003](ADR-0003-simulation-strategy.md) | Per-architecture simulation behind one interface | Accepted | — |
| [0004](ADR-0004-pcb-scope.md) | PCB is assisted, not autonomous | Accepted | OD-2 ✓ |
| [0005](ADR-0005-mcu-tier-sequencing.md) | MCU sequencing by verification strength (STM32 first) | Accepted | OD-1 ✓ |
| [0006](ADR-0006-desktop-shell-process-model.md) | Tauri shell + client–daemon process model | Accepted | — |
| [0007](ADR-0007-verification-loop-contract.md) | The closed verification-loop contract | Accepted | — |
| [0008](ADR-0008-toolchain-sandboxing.md) | Sandboxing LLM-driven toolchain workers | Accepted | — |

Read alongside [`../SYSTEM_SPEC.md`](../SYSTEM_SPEC.md).
