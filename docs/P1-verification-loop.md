# P1 — Verification Loop Core (component design)

> Implements the thesis (SYSTEM_SPEC §1, §7) as the first runnable component.
> Status: **P1 COMPLETE.** P1a (fixture oracles) and P1b (real `arm-none-eabi-gcc`
> 12.2.1 + Renode 1.16.0) both verified on this machine — 11/11 tests pass, and the
> real STM32F4 firmware compiles, links, boots in Renode, and emits its USART2 banner
> in-simulation → VERIFIED. Date: 2026-09-12.

## 1. Scope

P1 delivers the closed verification loop **headless** for one target (STM32
blink+UART): `intent → compile → (self-heal) → simulate → VERIFIED`, with provenance.
No UI (that is P3), no PCB, no datasheet RAG (that is P2).

Split by what the current machine can run:
- **P1a (Rust Core, fixture oracles):** loop state machine, real GCC-diagnostic
  parser, provenance ledger, tier assignment — proven with **fixture oracles** +
  `cargo test`. No hardware/toolchains required.
- **P1b (heavyweight installs):** real `arm-none-eabi-gcc` and `Renode` adapters
  behind the same oracle traits; meets the true exit criterion.

## 2. Component shape

```
run_loop(goal, codegen, compile_oracle, sim_oracle, ledger, config)
        │
        ├── codegen(ctx)            model stand-in → FirmwareDraft   (P1b: real LLM)
        ├── compile_oracle.compile  ── CompileOracle ──┬─ ArmGccOracle   (real, guarded)
        │                                              └─ FixtureCompileOracle
        ├── sim_oracle.simulate     ── SimOracle ──────┬─ RenodeOracle   (real, guarded)
        │                                              └─ FixtureSimOracle
        ├── assign_tier(...)        deterministic oracle result → Tier
        └── ledger.record(...)      append-only provenance (JSONL)
```

The loop depends only on the **CompileOracle / SimOracle** protocols. Fixtures and
real toolchains are interchangeable implementations — this is what makes the thesis
testable without hardware, and what lets the Rust Core (P3) reuse the same contract.

## 3. Tool contracts

### 3.1 `compile` → `CompileResult`
```jsonc
{
  "ok": false,
  "diagnostics": [                       // parsed from real compiler stderr
    {"severity": "error", "file": "main.c", "line": 12, "col": 5,
     "message": "expected ';' before '}' token", "raw": "main.c:12:5: error: ..."}
  ],
  "artifact_path": null,                 // set (ELF) only when ok
  "toolchain": "arm-none-eabi-gcc 13.2.1",
  "toolchain_available": true
}
```
Structured diagnostics (never free-text) are what make the self-heal loop possible:
`diagnostics` are fed back into `codegen` context on the next attempt (ADR-0002 rigor,
applied to compilers).

### 3.2 `simulate` → `SimResult`
```jsonc
{
  "ok": true,
  "boundary": {                          // ADR-0003: the honesty object, always present
    "verified":    ["cpu_boot", "register_init", "gpio_toggle", "uart_tx"],
    "stubbed":     ["adc_values"],
    "not_modeled": ["wifi", "ble", "rf", "real_analog"]
  },
  "observations": {"gpio_toggles": 5, "uart": "Hello from Embeder\r\n"},
  "engine": "renode 1.15",
  "engine_available": true
}
```

## 4. State machine (ADR-0007)

```
DRAFT ─compile─▶ ok? ──no──▶ [attempt<max] regenerate(with diagnostics) ─▶ DRAFT
                  │                     └─[attempt==max]─▶ HALTED (return errors)
                 yes
                  ▼
                BUILT ─simulate─▶ pass-in-scope? ──yes──▶ VERIFIED (+boundary, +provenance)
                                        └──no──▶ [attempt<max] regenerate ─▶ DRAFT
                                                 [attempt==max]─▶ HALTED
```
Invariants: **bounded retry** (default 3); **honest degradation** (HALTED returns the
structured failure, never a guess); **only the oracles** move DRAFT→BUILT→VERIFIED;
VERIFIED means "within the modeled scope" and always carries the boundary object.

## 5. Provenance record (SYSTEM_SPEC §6)
```jsonc
{"id":"...", "ts":..., "actor":"oracle|user|model", "tool":"compile|simulate|verify",
 "inputs_hash":"sha256/16", "outputs_hash":"sha256/16", "tier":"verified|null",
 "oracle_version":"arm-none-eabi-gcc 13.2.1 + renode 1.15", "detail":{...}}
```
Append-only JSONL at `.embeder/provenance.jsonl`. A VERIFIED artifact is reproducible
from its input hash + named oracle versions.

## 6. Tier assignment
`assign_tier` returns **VERIFIED** only when a deterministic oracle proved the modeled
behavior (`compile.ok ∧ simulate.ok ∧ boundary.verified ≠ ∅`); otherwise **ADVISORY**.
Aspects in `boundary.stubbed` are ⚠️ and in `boundary.not_modeled` are ⛔ — the Core
never promotes them. (SYSTEM_SPEC §2.)

## 7. Test strategy (P1a, `cargo test`, zero external crates) — PASSING

8/8 tests green as of 2026-09-12 (rustc 1.98.1, x86_64-pc-windows-gnu):
- **Parser** (`core/src/gcc.rs`, 4 tests): error w/ line:col; warning + note + linker
  "undefined reference"; Windows drive-letter path preserved; `fatal error` → error.
- **Loop** (`core/tests/integration.rs`, 3 tests): (a) clean build → VERIFIED in 1
  attempt; (b) bug-then-heal → VERIFIED in 2; (c) persistent bug → HALTED at max
  attempts with diagnostics and no tier.
- **Provenance** (1 test): bug-then-heal writes exactly 4 records (compile-fail,
  compile-ok, simulate, verify) to the ledger.

## 8. P1b — real exit criterion (DONE)
Achieved 2026-09-12, no changes to the loop (only new oracle implementations):
1. Installed Arm GNU Toolchain 12.2.1 + Renode 1.16.0 (both via winget).
2. Real STM32F4 target under `firmware/stm32-blink-uart/`: register-level `main.c`,
   C `startup.c` (vector table + .data/.bss init), and `link/stm32f4.ld`.
3. `ArmGccOracle::stm32f4` compiles+links a real ELF (per-invocation build dir, so
   concurrent compiles are isolated).
4. `RenodeOracle::stm32f4` templates a `.resc` (`LoadPlatformDescription
   stm32f4_discovery` → `LoadELF` → `usart2 CreateFileBackend` → `RunFor "0.5"` →
   `quit`), runs Renode headless (`--console --disable-gui --plain`), and passes iff
   the captured USART2 text contains the banner.
5. `cargo run -- --real` drives the loop with these oracles: 100 bytes captured
   (`"Hello from Embeder\r\n"` × 5) → VERIFIED. Regression test:
   `real_full_loop_compile_sim_verifies` (`--ignored`).

### Known limitations / next hardening
- No wall-clock timeout around the Renode subprocess yet (relies on the `quit` in the
  `.resc`); add `wait_timeout` before this runs unattended in CI.
- Renode is invoked directly; per ADR-0001 this should move behind a Python MCP
  Simulation server. Same for compile (Firmware MCP server).

## 9. Sequencing note — RESOLVED
The Core is built natively in **Rust from Phase 1** (user decision, 2026-09-12),
honoring ADR-0006 directly rather than deferring to P3. Rust toolchain installed via
rustup (stable 1.98.1, `x86_64-pc-windows-gnu`, self-contained linker — no MSVC/VS
required). **Python is reserved for the MCP tool servers** (ADR-0001), which the Rust
Core will call as an MCP client at P1b.
