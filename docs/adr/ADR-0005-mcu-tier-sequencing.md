# ADR-0005 — MCU sequencing by verification strength (STM32 first)

- **Status:** Accepted (ratified 2026-09-12; closes OD-1)
- **Date:** 2026-09-12

## Context
The vision docs order MCU tiers by *human learning difficulty*: Arduino ("basic")
first, STM32 ("industrial") last. But Embeder's value is **verification**, and
verification strength runs in the opposite direction. "Easy for a beginner" is not
"easy for the system to prove."

## Decision
Build in order of **whole-loop verification strength**:

1. **STM32 (Cortex-M)** — the only family where every non-RF stage reaches ✅
   (CMSIS-SVD register data + Renode ISS + mature arm-none-eabi-gcc).
2. **RP2040** — also Cortex-M/Renode; introduces the PIO-unmodeled 🟡 edge case.
3. **ESP32** — compile ✅, boot/peripherals 🟡 (QEMU-Espressif), wireless ⛔. High
   demand; ship with explicit limits.
4. **AVR / Arduino** — different engine (simavr), weakest verification; a
   "prototyping lane," not a verified path.

## Consequences
- (+) The first thing we ship is the thing we can most defend (P1 = STM32 blink+UART,
   fully ✅). Momentum built on solid ground.
- (+) Each subsequent family adds one honest caveat at a time (PIO, then Xtensa/RF,
   then AVR engine swap).
- (−) The most *popular beginner* board (Arduino Uno) ships last. Accepted: it is the
   weakest fit for a verification-first system, and its audience is served better
   once the loop is proven.

## Alternatives rejected
- **Arduino-first (per vision docs):** optimizes for onboarding a learner audience at
  the cost of building the system's core on its weakest verification target.
