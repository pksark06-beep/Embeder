# Embeder — System Specification (v0.1, design pass)

> Status: **Core decisions ratified 2026-09-12** (OD-1..OD-4 closed; OD-5 open,
> non-blocking). Phase 1 may begin. This document locks scope and boundaries before
> implementation. It supersedes the vision described in `core.md` and the three
> architecture PDFs wherever they conflict. Decisions here are formalized in the
> ADRs under `docs/adr/`.
>
> Date: 2026-09-12

---

## 0. How to read this document

Embeder is a **system**, not an application. The difference that matters for design:
a system makes a small number of promises and keeps *all* of them, rather than a
large number of features that mostly work. This spec is organized around that
principle. Its center of gravity is **Section 3 (the Verifiability Matrix)** — the
honest statement of what Embeder *verifies* versus what it *advises*. Everything
else exists to serve that matrix.

---

## 1. Thesis

> **Embeder grounds an LLM's embedded-engineering output in a closed verification
> loop — native compilation and instruction-accurate simulation — so the model
> cannot ship hardware code it hasn't proven.**

The intellectual contribution is not "AI writes firmware." It is the *loop*: every
model claim about firmware is subjected to a deterministic oracle (a real compiler,
a real instruction-set simulator) before it reaches the user or the hardware. The
model proposes; the toolchain disposes. This is what separates Embeder from a chat
wrapper, and it is the axis on which the whole system should be judged.

Corollary: **the system's credibility is bounded by the honesty of its oracles.**
Where we have a deterministic oracle (a compiler, an ISS peripheral model) we may
say *verified*. Where we do not (RF, analog, real-world timing, board electrical
correctness) we must say *advisory* — loudly, in the UI, on every artifact. A system
that blurs this line is worse than useless; it is confidently wrong about hardware.

---

## 2. Confidence tiers (the vocabulary of the whole system)

Every capability, every tool result, and every UI surface is labeled with exactly
one of these. This vocabulary is load-bearing.

| Tier | Symbol | Meaning | Example |
|---|---|---|---|
| **Verified** | ✅ | A deterministic oracle proved it. Reproducible. | Firmware compiled clean; Renode executed the boot + register init without fault. |
| **Assisted** | 🟡 | The system did the mechanical work correctly, but a human must confirm intent/fitness. | Netlist generated from a component graph; flashing a binary to attached hardware. |
| **Advisory** | ⚠️ | Best-effort model output with no oracle behind it. Explicitly *not* verification. | "This decoupling network should be adequate"; simulated ADC value; RF range estimate. |
| **Out of scope** | ⛔ | Not supported. The system refuses rather than guesses. | Simulating an ESP32 BLE link; autonomous routing of a manufacturable board. |

**Hard rule:** the system never silently promotes a tier. An ⚠️ result may never be
presented with the affordances of a ✅ result. If a stage cannot reach at least 🟡,
it says ⛔ and stops.

---

## 3. Verifiability Matrix (the crown jewel)

What Embeder can actually stand behind, per MCU family. This table is the contract.

Legend: ✅ Verified · 🟡 Assisted · ⚠️ Advisory · ⛔ Out of scope

| Capability | STM32 (Cortex-M) | RP2040 (Cortex-M0+) | ESP32 (Xtensa) | AVR / Arduino (ATmega) |
|---|---|---|---|---|
| **Datasheet: register map** | ✅ CMSIS-SVD | ✅ SVD (RPi) | 🟡 SVD (Espressif) | 🟡 ATDF/pack |
| **Datasheet: pinmux / pin capability** | 🟡 curated + CubeMX DB | 🟡 fixed fn + PIO caveats | 🟡 strapping/input-only rules | 🟡 well-known |
| **Firmware: native compile** | ✅ arm-none-eabi-gcc | ✅ arm-none-eabi-gcc | ✅ esp-idf / xtensa-gcc | ✅ avr-gcc |
| **Firmware: self-healing loop** | ✅ | ✅ | ✅ | ✅ |
| **Firmware: flash to hardware** | 🟡 OpenOCD/ST-Link | 🟡 UF2 | 🟡 esptool | 🟡 avrdude |
| **Sim: CPU boot / register init** | ✅ Renode | ✅ Renode | 🟡 QEMU-Espressif | 🟡 simavr / QEMU-AVR |
| **Sim: digital peripherals (GPIO/UART/I²C/SPI/timers)** | ✅ Renode | 🟡 (PIO unmodeled) | 🟡 | 🟡 simavr |
| **Sim: analog (ADC/DAC/sensors)** | ⚠️ stubbed values | ⚠️ | ⚠️ | ⚠️ |
| **Sim: wireless (Wi-Fi/BLE/RF)** | ⛔ | ⛔ (Pico W) | ⛔ | ⛔ |
| **PCB: netlist from validated graph** | 🟡 (family-independent) | 🟡 | 🟡 | 🟡 |
| **PCB: autorouting → manufacturable board** | ⛔ (Freerouting/human = ⚠️) | ⛔ | ⛔ | ⛔ |
| **PCB: DRC + Gerber/BOM/CPL export** | ✅ pcbnew (family-independent) | ✅ | ✅ | ✅ |

**Three consequences you can read straight off this table:**

1. **The verification loop is strongest on STM32/RP2040 (Cortex-M).** That is where
   we have authoritative register data (SVD) *and* the best ISS (Renode). This
   inverts the vision docs' tier ordering — see §4.
2. **The docs' canonical demo (ESP32 BLE temperature logger, "verified in
   simulation") is the system's worst case:** wrong simulator family for Renode *and*
   the wireless link is ⛔. Embeder must present the wireless portion of any such
   project as ⚠️/⛔ and verify only the compile + non-RF peripheral logic.
3. **"Idea-to-PCB" is 🟡/⚠️, never autonomous.** Netlist + DRC + export are reliable;
   *routing a correct, manufacturable board from intent* is not, and is descoped to
   template/reference-design-assisted with a human in the loop. See ADR-0004.

---

## 4. MCU tier sequencing (inverting the vision docs)

The vision docs order tiers by *human learning difficulty* (Arduino = "basic" first,
STM32 = "industrial" last). Embeder should order by **verification-stack strength**,
because the system's value is verification, not pedagogy. "Easy for a beginner" ≠
"easy for the system to prove."

| Build order | Family | Why first | Whole-loop status |
|---|---|---|---|
| **1** | **STM32 (Cortex-M)** | SVD register data + Renode ISS + mature GCC = the *only* family where every non-RF stage reaches ✅. | Strongest |
| **2** | **RP2040** | Also Cortex-M/Renode; adds the PIO caveat as a known 🟡 edge. | Strong |
| **3** | **ESP32** | Compile ✅, boot/peripherals 🟡 (QEMU-Espressif), wireless ⛔. High user demand; honest about limits. | Partial |
| **4** | **AVR / Arduino** | Different sim engine (simavr, not Renode). Good "prototyping lane," weakest verification. | Prototyping only |

See ADR-0005.

---

## 5. Architecture (locked shape)

```
┌────────────────────────────────────────────────────────┐
│  UI Shell — Tauri (Rust) + Web frontend                 │  presentation only;
│  panels: Reasoning · Firmware Editor · HW Canvas · PCB   │  never runs toolchains
└───────────────────────────┬────────────────────────────┘
                            │  typed IPC (Tauri commands / events)
┌───────────────────────────▼────────────────────────────┐
│  Core / Desktop Engine (Rust)                           │  orchestration, BYOK
│  · session & workspace state   · provenance ledger       │  secrets, LLM routing,
│  · MCP orchestrator (client)   · capability broker       │  tier-labeling authority
└───────────────────────────┬────────────────────────────┘
                            │  MCP (local stdio), schema-enforced tool contracts
      ┌────────────┬─────────┴────────┬─────────────────┐
      ▼            ▼                  ▼                 ▼
┌───────────┐ ┌───────────┐   ┌──────────────┐  ┌──────────────┐
│ Datasheet │ │ Firmware  │   │ Simulation   │  │ PCB / EDA    │  MCP servers
│  MCP      │ │  MCP      │   │  MCP         │  │  MCP         │  (Python/FastMCP)
└───────────┘ └─────┬─────┘   └──────┬───────┘  └──────┬───────┘
                    ▼                ▼                 ▼
             ┌──────────────────────────────────────────────┐
             │  Toolchain Workers — SANDBOXED subprocesses   │  compilers, Renode/
             │  fs-jailed to workspace · no ambient network   │  QEMU/simavr, KiCad
             │  · hardware/flash behind explicit user consent │  pcbnew, Freerouting
             └──────────────────────────────────────────────┘
```

Key invariants:
- **The UI never executes a toolchain.** All execution is behind the Core → MCP →
  sandboxed-worker path, so every action is loggable, cancelable, and permissioned.
- **The Core is the sole authority for tier labels.** MCP servers report raw oracle
  results + provenance; the Core assigns ✅/🟡/⚠️/⛔ from that. Tiering logic lives in
  one place, not scattered across servers.
- **Every state-changing action writes to the provenance ledger** (§6).

See ADR-0001 (MCP backbone), ADR-0006 (shell/process model), ADR-0008 (sandboxing).

## 6. Provenance ledger

Because the model drives real tools on the user's machine and (optionally) real
hardware, every consequential action is recorded as an append-only entry:
`{timestamp, actor(model/user), tool, inputs_hash, outputs_hash, tier_assigned,
oracle_version}`. This is what makes "transparent AI guidance" real rather than a
slogan, and it is the audit trail for the closed loop. Firmware artifacts carry the
hash of the exact toolchain + sim run that verified them.

## 7. The closed verification loop (formal core)

```
draft ──▶ compile ──(errors)──▶ structure errors ──▶ regenerate ──┐
   ▲                                                              │
   └──────────────────── bounded retry (N) ───────────────────────┘
             │ clean build
             ▼
        simulate (per-arch engine) ──▶ oracle report + verifiability boundary
             │ pass (within modeled scope)
             ▼
        mark firmware ✅(non-RF) / ⚠️(unmodeled aspects) + provenance
```

The loop has a **bounded retry budget** and must degrade honestly: if it cannot
reach a clean build within budget it returns the structured failure, not a guess.
The simulation step always emits a *verifiability boundary* object listing what was
and was not exercised (e.g., "UART TX verified; ADC values stubbed; no RF"). See
ADR-0003, ADR-0007.

## 8. Security & trust model (summary)

The threat is not a remote attacker — it is **an LLM issuing tool calls that run
code and touch hardware on the user's machine.** Mitigations (ADR-0008): workspace
filesystem jail for all workers; no ambient network egress from workers; hardware
flashing and any destructive action gated behind explicit, per-action user consent;
BYOK secrets stored in the OS keychain, never passed to workers.

## 9. Non-goals (v1)

- ❌ Autonomous routing of manufacturable PCBs.
- ❌ Simulating wireless/RF or claiming any RF behavior as verified.
- ❌ Cloud execution of toolchains (Embeder is local-first by definition).
- ❌ Guaranteeing electrical correctness of generated schematics (⚠️ only).
- ❌ Being a general IDE. It is a *verification-loop workspace*.

## 10. Phased roadmap (revised from `core.md`)

| Phase | Deliverable | Exit criterion (measurable) |
|---|---|---|
| **P0** | This spec + ADRs ratified | Decisions accepted; open register (§11) closed. |
| **P1** ✅ DONE | Verification loop core, headless (Rust) | MET 2026-09-12: STM32 blink+UART goes intent→✅ real compile (arm-none-eabi-gcc)→✅ Renode-verified (USART2 captured) with provenance, no UI. |
| **P2** ◑ registers done | Datasheet grounding (structured) | MET for registers 2026-09-12: SVD-grounded register maps with citations + tiers, refuses (no fabrication) on a miss, addresses cross-checked against P1 firmware. Pinmux grounding still to do. |
| **P3** | Tauri shell wraps P1–P2 | Same P1 flow driven from the UI with live compile/sim logs + tier badges. |
| **P4** | Simulation breadth | RP2040 + ESP32 engines behind one MCP interface; verifiability-boundary objects surfaced in UI. |
| **P5** | PCB assist | netlist→DRC→Gerber/BOM export (🟡/✅) from a validated graph; routing via Freerouting/human (⚠️). |

## 11. Open decisions register

| # | Decision | Resolution (2026-09-12) | ADR |
|---|---|---|---|
| OD-1 | Tier sequencing (STM32-first inversion) | ✅ Accepted | ADR-0005 |
| OD-2 | PCB ambition: assisted vs. autonomous routing | ✅ Accepted — assisted for v1 | ADR-0004 |
| OD-3 | Datasheet: structured-first vs. RAG-first | ✅ Accepted — structured-first | ADR-0002 |
| OD-4 | Primary user | ✅ **Both, pro-first** — see below | ADR-0009 (pending) |
| OD-5 | License / governance for "open source" | ⏳ Open — non-blocking for P1 | — |

### 11.1 OD-4 resolution — "Both, pro-first"

v1 builds the **pro-grade verification core** and treats the learner as a later,
additive layer — never a compromise to the core:

- **One engine, two presentation modes.** The verification loop, its oracles, the
  provenance ledger, and the tier labels are *identical* in both modes. Only framing
  and verbosity differ. A "Learning mode" must never relax a tier or hide a ⚠️.
- **Default (Pro):** register-level, terse, log- and provenance-heavy; STM32-first;
  the loop as a productivity multiplier. This is what P1–P3 build.
- **Learning mode (P4+):** explanation-forward reasoning panel, guardrails, gentler
  defaults, more prominent Arduino/ESP32 entry points — layered on the *same*
  verified core.
- **Consequence:** no beginner-oriented shortcut may enter the core. If a feature
  only makes sense for learners and would weaken verification, it lives in the mode
  layer or not at all.

*(This resolution should be captured as ADR-0009 before Learning-mode work starts.)*

---

*OD-1..OD-4 are closed. Only OD-5 (license/governance) remains and it does not block
Phase 1. The verification loop core (P1 — STM32 intent→✅compiled→✅Renode-verified,
headless) can begin.*
