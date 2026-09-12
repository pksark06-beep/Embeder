# Embeder

A local-first, BYOK AI workspace for embedded systems engineering. Its thesis: an LLM's
firmware output is **grounded in a closed verification loop** — native compilation and
instruction-accurate simulation as deterministic oracles — so the model cannot ship
hardware code it hasn't proven.

> This is a **system**, not an app. Read [`docs/SYSTEM_SPEC.md`](docs/SYSTEM_SPEC.md)
> first; it is the contract. Decisions are recorded in [`docs/adr/`](docs/adr/).

## Repository layout

```
docs/                  Ratified design: system spec + ADRs + phase notes
core/                  The Rust Core (ADR-0006): the verification-loop engine  [ACTIVE]
  src/                 loop engine, oracle traits, gcc parser, provenance, tiering
  tests/               integration tests for the loop invariants (ADR-0007)
firmware/              Example targets (STM32 blink+UART is the P1 exit criterion)
servers/               Python MCP tool servers (ADR-0001) — arrives at P1b
```

## Status: Phase 1 (verification loop core, headless)

- **P1a — mechanics: DONE, verified on this machine.** The closed-loop state machine,
  the real GCC-diagnostic parser, the provenance ledger, and tier assignment, driven
  by *fixture* oracles. 8/8 tests pass; the demo reaches VERIFIED end to end.
- **P1b — real exit criterion: pending toolchains.** Wire `arm-none-eabi-gcc` +
  `Renode` behind the same oracle traits (`ArmGccOracle`, `RenodeOracle` are already
  stubbed in `core/src/`). Requires installing those toolchains.

### Build & run (Rust toolchain installed via rustup)

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"   # if cargo isn't on PATH yet
cargo test --manifest-path core\Cargo.toml             # 8 tests
cargo run  --manifest-path core\Cargo.toml --bin embeder-demo
```

Expected: `intent → compiled → (self-heal) → simulated → VERIFIED`, reached after one
self-heal iteration, with a provenance trail written to `.embeder/provenance.jsonl`.

## Design integrity

Every result is labeled with a confidence tier — Verified / Assisted / Advisory /
Out-of-scope (SYSTEM_SPEC §2) — and the Core never promotes a tier. A VERIFIED
firmware carries a *verifiability boundary* stating exactly what simulation exercised
(and, honestly, what it did not: RF, wireless, and real analog are out of scope).
