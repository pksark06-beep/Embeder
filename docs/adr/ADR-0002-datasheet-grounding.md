# ADR-0002 — Structured-first datasheet grounding (SVD over RAG)

- **Status:** Accepted (ratified 2026-09-12; closes OD-3)
- **Date:** 2026-09-12

## Context
The Datasheet server must answer register-map and pinmux questions. The vision docs
describe a "local vector index of datasheet PDFs" (RAG). But the safety-critical
facts here — register addresses, bitfield offsets, pin capabilities — are exactly the
facts where a hallucination is *actively dangerous*: a wrong address silently
compiles and bricks or misconfigures hardware. RAG over PDF prose is lossy and
non-authoritative for structured facts.

## Decision
**Structured sources are the primary oracle; RAG is secondary and prose-only.**
- Register maps: **CMSIS-SVD** files (ARM: STM32, RP2040), Espressif SVD (ESP32),
  ATDF/pack files (AVR). These are authoritative and machine-readable.
- Pinmux / pin capability: curated structured tables (STM32 CubeMX DB where
  license permits; ESP32 strapping/input-only rules; RP2040 fixed-function + PIO
  caveats).
- RAG over datasheet PDFs is used **only** for explanatory prose ("what does this
  peripheral do"), never to source an address, offset, or pin capability.
- Every factual answer carries a **source citation** (file + node path). Answers
  without a structured source are returned as ⚠️ advisory, not as fact.

## Consequences
- (+) Near-zero fabricated addresses; answers are citable and reproducible.
- (+) Enables a hard test gate (P2 exit: zero fabricated addresses on the test set).
- (−) Upfront work curating/normalizing SVD/pinmux data per family; SVD coverage
  quality varies (ESP32 < STM32). Reflected as 🟡 in the matrix.

## Alternatives rejected
- **RAG-first:** simpler to build, but stakes the system's credibility on a lossy
  retrieval of prose for facts that must be exact. Unacceptable for hardware.
