# ADR-0003 — Per-architecture simulation behind one interface

- **Status:** Accepted
- **Date:** 2026-09-12

## Context
The vision docs say "Renode/QEMU" as if one engine covers all targets. It does not.
Renode covers ARM Cortex-M well (STM32, RP2040) but **not AVR** and **not Xtensa**.
ESP32 (Xtensa) needs the **Espressif QEMU fork**; AVR (ATmega/Arduino) needs
**simavr** or QEMU-AVR. And no engine meaningfully simulates Wi-Fi/BLE/RF.

## Decision
Define **one Simulation MCP interface** (`run_simulation`, `capture_signals`,
`check_timing`) backed by a **per-architecture engine adapter**:

| Family | Engine |
|---|---|
| STM32, RP2040 (Cortex-M) | Renode |
| ESP32 (Xtensa) | QEMU-Espressif |
| AVR (ATmega) | simavr (fallback QEMU-AVR) |

Every simulation result MUST include a **verifiability-boundary object** enumerating
what was and was not exercised, e.g.:
```json
{ "verified": ["cpu_boot","register_init","uart_tx","timer_isr"],
  "stubbed":  ["adc_values"],
  "not_modeled": ["wifi","ble","rf","real_analog"] }
```
The Core turns this into tier labels: modeled → contributes to ✅; stubbed → ⚠️;
not_modeled → ⛔ for those aspects. **Wireless/RF is always ⛔.**

## Consequences
- (+) Honest, machine-readable boundary on every run; the UI can never present an
  unmodeled aspect as verified.
- (+) New engines slot in behind the same interface.
- (−) Three engines to integrate/maintain with differing capabilities (matrix §3).

## Alternatives rejected
- **"Renode for everything":** factually wrong for AVR/Xtensa; would produce
  false ⛔-as-✅ results — the single worst failure mode for the system.
