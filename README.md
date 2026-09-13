# Embeder

A local-first, BYOK AI workspace for embedded systems engineering. Its thesis: an LLM's
firmware output is **grounded in a closed verification loop** — native compilation and
instruction-accurate simulation as deterministic oracles — so the model cannot ship
hardware code it hasn't proven.

> This is a **system**, not an app. Read [`docs/SYSTEM_SPEC.md`](docs/SYSTEM_SPEC.md)
> first; it is the contract. Decisions are recorded in [`docs/adr/`](docs/adr/).

## Repository layout (Cargo workspace)

```
docs/                  Ratified design: system spec + ADRs + phase notes
core/                  Rust Core (ADR-0006): the verification-loop engine  [P1]
  src/                 loop engine, oracle traits, gcc parser, provenance, tiering
  tests/               loop invariants (ADR-0007) + real-toolchain e2e (ignored)
datasheet/             Rust crate (ADR-0002): CMSIS-SVD grounding  [P2]
  data/                bundled STM32F4 SVD excerpt; drop full vendor .svd here too
mcp/                   Rust crate (ADR-0001): MCP client bridge + McpCompileOracle
firmware/              Example targets (STM32 blink+UART is the P1 exit criterion)
servers/               Python MCP tool servers (ADR-0001): firmware_server.py + mcp_lib.py
```

## Status: Phase 1 COMPLETE (verification loop core, headless)

- **P1a — mechanics: DONE.** Closed-loop state machine, real GCC-diagnostic parser,
  provenance ledger, and tier assignment, driven by *fixture* oracles.
- **P1b — real exit criterion: DONE.** The real STM32F4 blink+UART firmware compiles
  and links with `arm-none-eabi-gcc`, boots on Renode's `stm32f4_discovery` platform,
  and its USART2 banner is captured in-simulation — `intent → REAL compile →
  self-heal → REAL Renode sim → VERIFIED`.

## Status: Phase 2 (datasheet grounding) — registers DONE + loop grounded

- **P2 registers: DONE.** CMSIS-SVD-grounded register maps with absolute addresses,
  source citations, and confidence tiers; **refuses rather than fabricates** on a miss
  (ADR-0002). Addresses cross-checked against the P1 firmware.
- **Grounding the loop (P2→P1): DONE.** `GroundedCodegen` synthesizes firmware whose
  register addresses come *straight from the SVD* (each cited); that firmware then goes
  through the real compile+Renode loop to VERIFIED — `grounded-loop --real`.
  See [`docs/P2-datasheet-grounding.md`](docs/P2-datasheet-grounding.md). Pinmux: next.

## MCP backbone (ADR-0001) — compile now runs over MCP

The Firmware (compile) tool runs across the MCP protocol boundary: the Rust Core is an
MCP **client** driving a local Python MCP server over stdio JSON-RPC, which executes
`arm-none-eabi-gcc`. Diagnostics survive the round trip and drive self-heal. Renode is
now bounded by a wall-clock timeout. See [`docs/MCP-backbone.md`](docs/MCP-backbone.md).
Simulation + Datasheet MCP servers are next.

```powershell
cargo run -p embeder-mcp --bin mcp_loop -- --real   # intent -> (MCP) compile -> Renode -> VERIFIED
```

Verified on this machine: **20 tests** (15 default + 5 ignored real-toolchain/MCP e2e),
rustc 1.98.1 (x86_64-pc-windows-gnu), Arm GNU Toolchain 12.2.1, Renode 1.16.0, Python 3.12.
Run e2e tests single-threaded: `cargo test -- --ignored --test-threads=1`.

```powershell
# The full picture: datasheet -> grounded synthesis -> real compile -> real sim -> VERIFIED
cargo run -p embeder-datasheet --bin grounded-loop -- --real
```

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
