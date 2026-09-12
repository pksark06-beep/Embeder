# ADR-0001 — MCP as the tool backbone

- **Status:** Accepted
- **Date:** 2026-09-12

## Context
The model must drive four categories of local tools (datasheet, compile, simulate,
PCB). We need a boundary between "model intent" and "tool execution" that is
uniform, schema-enforced, loggable, and extensible by third parties — the
"transparent AI guidance" value proposition depends on this boundary being real.

## Decision
Use the **Model Context Protocol** over local **stdio** transport as the tool
backbone. Each server exposes tools with **JSON-schema-enforced** inputs/outputs and
returns **structured errors** (never free-text failures). The Core acts as MCP client
/ orchestrator. Every tool call and result is written to the provenance ledger.

## Consequences
- (+) One uniform, inspectable contract for every tool; third parties can add
  servers without touching the Core.
- (+) Schema enforcement gives the self-healing loop clean, parseable failures.
- (−) An extra serialization hop vs. calling libraries in-process. Accepted: tool
  latency is dominated by compilers/simulators (seconds), not IPC (ms).
- The Core, not the servers, assigns confidence tiers (SYSTEM_SPEC §2) — servers
  report raw oracle output + provenance only.

## Alternatives rejected
- **Direct in-process tool calls (no MCP):** faster, but loses the uniform
  contract, the audit boundary, and third-party extensibility — i.e. the value prop.
- **Remote/HTTP MCP:** contradicts local-first and widens the attack surface.
