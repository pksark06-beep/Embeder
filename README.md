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

## Status: Phase 1 COMPLETE (verification loop core, headless)

- **P1a — mechanics: DONE.** Closed-loop state machine, real GCC-diagnostic parser,
  provenance ledger, and tier assignment, driven by *fixture* oracles.
- **P1b — real exit criterion: DONE.** The real STM32F4 blink+UART firmware compiles
  and links with `arm-none-eabi-gcc`, boots on Renode's `stm32f4_discovery` platform,
  and its USART2 banner is captured in-simulation — `intent → REAL compile →
  self-heal → REAL Renode sim → VERIFIED`.

Verified on this machine: **11/11 tests** (8 default + 3 real-toolchain), rustc 1.98.1
(x86_64-pc-windows-gnu), Arm GNU Toolchain 12.2.1, Renode 1.16.0.

### Build & run

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"   # if cargo isn't on PATH yet
cargo test --manifest-path core\Cargo.toml             # 8 default tests

# Fixture demo (no toolchains needed):
cargo run  --manifest-path core\Cargo.toml --bin embeder-demo

# Real pipeline (needs arm-none-eabi-gcc on PATH + Renode):
$env:Path = "C:\Program Files (x86)\Arm GNU Toolchain arm-none-eabi\12.2 mpacbti-rel1\bin;$env:Path"
cargo run  --manifest-path core\Cargo.toml --bin embeder-demo -- --real
cargo test --manifest-path core\Cargo.toml --test real_toolchain -- --ignored   # 3 real tests
```

Expected (`--real`): VERIFIED after one self-heal, `observed uart = Hello from Embeder`,
with provenance in `.embeder/provenance-real.jsonl`. `EMBEDER_RENODE` overrides the
Renode path.

## Design integrity

Every result is labeled with a confidence tier — Verified / Assisted / Advisory /
Out-of-scope (SYSTEM_SPEC §2) — and the Core never promotes a tier. A VERIFIED
firmware carries a *verifiability boundary* stating exactly what simulation exercised
(and, honestly, what it did not: RF, wireless, and real analog are out of scope).
