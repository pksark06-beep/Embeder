# core/ — the Embeder Rust Core

Per **ADR-0006**, the Core (loop orchestration, provenance ledger, tier assignment,
and — later — the MCP orchestrator and BYOK secret access) is a long-lived **Rust**
process. Built natively from Phase 1; the Tauri shell wraps it at Phase 3.

## Modules (`src/`)

| Module | Responsibility |
|---|---|
| `engine.rs` | The closed verification loop state machine (ADR-0007) + tier assignment |
| `oracle.rs` | `CompileOracle` / `SimOracle` traits + result types (the contract) |
| `gcc.rs` | Structured GCC/Clang stderr → `Diagnostic` parser (pure, unit-tested) |
| `provenance.rs` | Append-only JSONL ledger (SYSTEM_SPEC §6) |
| `tiers.rs` | `Tier` + `VerifiabilityBoundary` (SYSTEM_SPEC §2/§3) |
| `fixtures.rs` | In-process fixture oracles — prove the loop with no toolchains |
| `armgcc.rs` | Real `arm-none-eabi-gcc` oracle (P1b; compiled, not yet exercised) |
| `renode.rs` | Real Renode oracle (P1b; availability probe live, boot wiring TODO) |

The loop depends only on the two oracle **traits**, so fixtures and real toolchains
are interchangeable. Real oracles will reach their toolchains via the Python MCP
servers (ADR-0001) added under `../servers/` at P1b.

## Run

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
cargo test --manifest-path core\Cargo.toml
cargo run  --manifest-path core\Cargo.toml --bin embeder-demo
```

Zero external crate dependencies — the Core builds offline on pure `std`.
