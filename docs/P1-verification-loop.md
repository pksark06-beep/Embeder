# P1 — Verification Loop Core (component design)

> Implements the thesis (SYSTEM_SPEC §1, §7) as the first runnable component.
> Status: **P1a implemented in the Rust Core** (`core/`, fixture oracles) — builds
> clean, 8/8 tests pass, demo reaches VERIFIED. **P1b pending** (real
> `arm-none-eabi-gcc` + `Renode`). Date: 2026-09-12.

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

## 8. P1b — path to the real exit criterion
1. Install `arm-none-eabi-gcc` (GNU Arm Embedded) and `Renode`.
2. Real STM32 build: `main.c` + startup + linker script → ELF (not just `-c` object).
3. `RenodeOracle`: emit a `.resc`, boot the ELF on an STM32F4 platform, capture USART2
   + GPIO, populate `observations` and the boundary object, assert on captured UART.
4. Swap `FixtureCompileOracle/FixtureSimOracle` → `ArmGccOracle/RenodeOracle`. No loop
   changes — that is the point of the interface.

## 9. Sequencing note — RESOLVED
The Core is built natively in **Rust from Phase 1** (user decision, 2026-09-12),
honoring ADR-0006 directly rather than deferring to P3. Rust toolchain installed via
rustup (stable 1.98.1, `x86_64-pc-windows-gnu`, self-contained linker — no MSVC/VS
required). **Python is reserved for the MCP tool servers** (ADR-0001), which the Rust
Core will call as an MCP client at P1b.
