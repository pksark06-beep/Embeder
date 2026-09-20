# ADR-0008 — Sandboxing LLM-driven toolchain workers

- **Status:** Accepted target design; not yet implemented
- **Date:** 2026-09-12

## Context
The real threat model is not a remote attacker — it is **an LLM issuing tool calls
that execute code and touch hardware on the user's machine.** Compilers run
arbitrary build scripts; KiCad automation runs arbitrary Python (`pcbnew`);
`flash_target` writes to physical devices. An unconstrained model here is a local
code-execution and hardware-damage risk, even with no malice — just a bad tool call.

## Decision
This section records the intended architecture, not current protections. The
current implementation has application-level path checks and removes model
API key environment variables from non-model child processes, but it has no
OS-level filesystem jail, network egress block, or keychain integration. There
is no flash tool exposed at present. See `SECURITY.md` for the operational
security boundary.

All toolchain workers run under least privilege:
- **Filesystem jail:** a worker can read/write only within the active
  `embeder-workspace/` (plus read-only access to installed toolchains). No access to
  the broader home directory.
- **No ambient network:** workers get no network egress by default. Package/board
  fetches happen through an explicit, audited channel, not from inside a build.
- **Explicit consent for the physical world:** `flash_target` and any destructive or
  irreversible action require **per-action user confirmation** in the UI. The model
  may propose; only the user authorizes hardware writes.
- **Secrets never reach workers:** BYOK API keys live in the OS keychain, are used
  only by the Core for LLM calls, and are never passed into a worker's environment.
- **Everything is ledgered:** every worker invocation → provenance entry (SYSTEM_SPEC
  §6), enabling audit and reproduction.

## Consequences
- (+) A bad/hallucinated tool call is contained to the workspace and cannot flash
  hardware or exfiltrate secrets without the user in the loop.
- (−) Some friction (consent prompts for flashing; explicit fetch step). Accepted —
  this is exactly where friction belongs.

## Alternatives rejected
- **Trust the model / run workers with user privileges:** unacceptable for a system
  that executes model-authored code and drives hardware.
- **Full container/VM isolation for every worker (v1):** stronger but heavy on a
  local-first desktop app; revisit if the threat model warrants (e.g. sharing
  untrusted projects). fs-jail + no-net + consent is the right v1 balance.
