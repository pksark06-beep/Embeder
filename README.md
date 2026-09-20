# Embeder

A local-first, BYOK AI workspace for embedded systems engineering. Its thesis: an LLM's
firmware output is **grounded in a closed verification loop** — native compilation and
instruction-accurate simulation as deterministic oracles — so the model cannot ship
hardware code it hasn't proven.

> This is a **system**, not an app. Read [`docs/SYSTEM_SPEC.md`](docs/SYSTEM_SPEC.md)
> first; it is the contract. Decisions are recorded in [`docs/adr/`](docs/adr/).

**[Download for Windows](https://github.com/pksark06-beep/Embeder/releases/latest)** ·
**[Documentation](https://embeder-docs.vercel.app)** ·
**[Installation guide](docs/INSTALLATION.md)** ·
**[Support the project](https://buymeacoffee.com/sekweb)**

[![M8ven Score](https://m8ven.ai/badge/mcp/pksark06-beep/embeder)](https://m8ven.ai/mcp/pksark06-beep/embeder)

The Windows installer contains the Embeder workbench, bundled datasheet data and the
starter workspace. Real firmware builds require Arm GNU `arm-none-eabi-gcc`; simulation
requires Renode. Python and an API key are optional and only needed for model-backed
generation. See the [installation guide](docs/INSTALLATION.md) for the complete setup.

## Repository layout (Cargo workspace)

```
docs/                  Ratified design: system spec + ADRs + phase notes
core/                  Rust Core (ADR-0006): the verification-loop engine  [P1]
  src/                 loop engine, oracle traits, gcc parser, provenance, tiering
  tests/               loop invariants (ADR-0007) + real-toolchain e2e (ignored)
datasheet/             Rust crate (ADR-0002): CMSIS-SVD grounding  [P2]
  data/                bundled STM32F4 SVD excerpt; drop full vendor .svd here too
mcp/                   Rust crate (ADR-0001): MCP client bridge (compile/sim/datasheet)
desktop/               UI shell (ADR-0006): dist/ frontend + dev server + src-tauri  [P3]
website/               Public landing page and documentation source
firmware/              Example targets (STM32 blink+UART is the P1 exit criterion)
servers/               Python MCP tool servers: firmware / simulation / datasheet
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

## MCP backbone (ADR-0001) — DONE (compile + simulate + datasheet over MCP)

All three tools run across the MCP protocol boundary: the Rust Core is an MCP **client**
driving local MCP servers over stdio JSON-RPC — `firmware_server.py` (arm-none-eabi-gcc),
`simulation_server.py` (Renode, headless + timeout), and `datasheet_server.py`
(CMSIS-SVD). Diagnostics survive the round trip and drive self-heal; the datasheet path
returns the same addresses as the Rust crate (cross-checked). See
[`docs/MCP-backbone.md`](docs/MCP-backbone.md).

```powershell
cargo run -p embeder-mcp --bin mcp_loop -- --real   # intent -> (MCP) compile + simulate -> VERIFIED
```

Verified on this machine: **32 tests** (21 default + 11 ignored real-toolchain/MCP e2e),
rustc 1.98.1 (x86_64-pc-windows-gnu), Arm GNU Toolchain 12.2.1, Renode 1.16.0, Python 3.12.
Run e2e tests single-threaded: `cargo test -- --ignored --test-threads=1`.

## The model in the loop (BYOK) — the other half of the thesis

A real large language model now drafts the firmware, but it never certifies itself:
its draft is handed to the *same* compile + simulation oracles, and only they advance
it to VERIFIED. The model is **grounded** — its prompt carries the exact register
addresses from the CMSIS-SVD, and the citations attached to the draft are those SVD
facts, not anything the model claimed. On a compile failure the diagnostics are fed
back and the model **self-heals**; if it can never satisfy the oracles the loop HALTs
honestly. The LLM runs behind a Codegen MCP server (`servers/codegen_server.py`), so
the selected provider receives the prompt and API key over the network.
Toolchain workers currently have application-level path checks, not OS-level isolation.

Bring your own key — set one of these (or drop it in a git-ignored `.env`; see
[`.env.example`](.env.example)). With **no** key set, the loop falls back to
deterministic grounded synthesis, so it always runs offline:

```powershell
$env:GEMINI_API_KEY     = "..."   # Google Gemini
# or:  $env:AI_GATEWAY_API_KEY = "..."   # Vercel AI Gateway (OpenAI-compatible)
# or:  $env:OPENAI_API_KEY     = "..."   # OpenAI / any OpenAI-compatible endpoint

cargo run -p embeder-mcp --bin llm_loop            # model draft -> fixture oracles (anywhere)
cargo run -p embeder-mcp --bin llm_loop -- --real  # model draft -> real arm-gcc + Renode -> VERIFIED
```

Provider/model auto-detect from whichever key is present; override with
`EMBEDER_LLM_PROVIDER` (`gemini` | `vercel` | `openai` | `none`) and `EMBEDER_LLM_MODEL`.

## Phase 3: UI shell (ADR-0006)

An instrument-panel desktop UI over the working Core — signal-flow pipeline, tier
verdict, verifiability boundary, grounded citations, and the provenance ledger. The
frontend runs inside the Tauri native shell (`invoke`) or the dev server (`fetch`);
same UI, same Core. See [`desktop/README.md`](desktop/README.md).

```powershell
cargo run -p embeder-desktop        # http://127.0.0.1:8787  (no extra toolchains)
```

The toolbar's **✦ Generate** button (or `POST /api/generate` with the goal as the body)
runs the BYOK model in the app: describe the firmware, the model drafts it grounded on
the SVD, and the same compile + Renode oracles verify it — self-healing from compiler
diagnostics, non-destructive to your workspace, and recording which model drafted the
code in provenance. Set a key (see above) or it falls back to grounded synthesis.

> Tauri's native build on Windows needs the MSVC toolchain; this host is `windows-gnu`,
> so the dev server is the runnable path here and `src-tauri/` is the native packaging
> (build it under MSVC). The Tauri command mirrors the dev server's `/api/run`.

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

## Support the project

If Embeder is useful to your work, you can support continued development at
[buymeacoffee.com/sekweb](https://buymeacoffee.com/sekweb). Contributions, bug reports,
and reproducible hardware cases are equally welcome.

## License and official releases

Embeder source code is licensed under the [Mozilla Public License 2.0](LICENSE).
The source for each official Windows release is available in this repository at its
release tag. Contributions are welcome under the same license; see
[CONTRIBUTING.md](CONTRIBUTING.md). The Embeder name and logo are covered by the
[trademark policy](TRADEMARKS.md), which keeps unofficial forks distinguishable.
Please report security issues through the process in [SECURITY.md](SECURITY.md).
