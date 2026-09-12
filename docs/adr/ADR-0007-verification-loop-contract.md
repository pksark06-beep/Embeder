# ADR-0007 — The closed verification-loop contract

- **Status:** Accepted
- **Date:** 2026-09-12

## Context
The loop is the thesis (SYSTEM_SPEC §1, §7). If it is under-specified it will drift
into "model says it's fine." It needs a formal contract: states, the oracle at each
edge, retry bounds, and honest degradation.

## Decision
The loop is a state machine with **deterministic oracles on every transition**:

```
DRAFT ─compile─▶ {clean → BUILT} | {errors → structure_errors → REGENERATE}
REGENERATE ─▶ DRAFT      (retry budget N; default N=3, configurable)
BUILT ─simulate─▶ {pass-in-scope → VERIFIED(+boundary)} | {fault → REGENERATE | HALT}
budget exhausted ─▶ HALT(return structured failure, never a guess)
```

Rules:
- **Compiler and simulator are the only sources of truth** for BUILT/VERIFIED. The
  model never self-certifies a transition.
- Every VERIFIED state carries the **verifiability-boundary object** (ADR-0003) and a
  **provenance record** (toolchain version, sim engine version, input/output hashes).
- **Bounded retry:** on budget exhaustion the loop HALTs and returns the last
  structured error set. No silent success.
- **Honest degradation:** VERIFIED means "verified *within the modeled scope*," never
  "correct on real hardware." Unmodeled aspects (RF, analog, timing-on-silicon)
  remain ⚠️/⛔ on the artifact.

## Consequences
- (+) "Verified" has a precise, reproducible meaning tied to named oracle versions.
- (+) The loop cannot loop forever or fake success.
- (−) Requires disciplined structured-error parsing per toolchain (ADR-0002 style
   rigor for compilers).

## Alternatives rejected
- **Unbounded self-healing / model self-assessment:** turns the thesis back into a
  chat wrapper and can burn tokens/time indefinitely.
