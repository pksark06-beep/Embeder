# P2 — Datasheet Grounding (component design)

> Implements ADR-0002 (structured-first datasheet grounding). Status: **register-map
> grounding DONE** (`datasheet/` crate, 5/5 tests). **Pinmux grounding: next.**
> Date: 2026-09-12.

## 1. Why this exists
A hallucinated register address silently compiles and misconfigures — or bricks —
hardware. So the safety-critical facts (addresses, bitfields) must come from an
**authoritative structured source**, carry a **citation**, and be **refused rather
than guessed** on a miss. RAG over datasheet PDFs is reserved for prose only.

## 2. Source of truth
**CMSIS-SVD** — the vendor-published, machine-readable register description format.
The `datasheet` crate parses it into `Device → Peripheral → Register → Field`. The
parser handles the features real vendor SVDs use, not just the bundled excerpt:
- `derivedFrom` peripherals (e.g. `USART2` inheriting `USART1`'s registers),
- all three field bit encodings (`bitOffset`+`bitWidth`, `lsb`+`msb`, `bitRange`).

A minimal, hand-authored excerpt lives at `datasheet/data/stm32f4-mini.svd` (RCC,
GPIOA, USART2 — exactly the registers the P1 firmware touches). **Drop a full vendor
`.svd` beside it and the same parser handles it** — no code change.

## 3. The grounding contract
```rust
struct Grounded<T> { value: Option<T>, tier: Tier, citation: Option<Citation>, note: String }
struct Citation   { source: String /* svd file */, path: String /* node path */ }
```
- **Hit** → `value: Some(..)`, `tier: Verified`, `citation: Some(..)`.
- **Miss** → `value: None`, `tier: OutOfScope`, `citation: None`, note explains the
  refusal. **The layer never fabricates.**

Queries: `register_map(peripheral)` and `lookup_register(peripheral, register)` — both
return absolute addresses (`base + offset`) and, for `lookup_register`, the bitfields.

## 4. Proof — the datasheet layer agrees with reality
The P2 tests assert the very addresses the P1 firmware used and Renode verified:

| Fact | Grounded answer | Cross-check |
|---|---|---|
| USART2 base | `0x40004400` | firmware `USART2_BASE` |
| USART2.CR1 | `0x4000440C` | firmware `USART2_CR1` |
| CR1.UE / CR1.TE | bit 13 / bit 3 | firmware `USART_CR1_UE/TE` |
| GPIOA.MODER / ODR / AFRL | `0x40020000 / …14 / …20` | firmware GPIOA writes |
| RCC.AHB1ENR / APB1ENR | `0x40023830 / …40` | firmware clock enables |

`datasheet-demo USART2` prints the grounded map with its citation and tier;
`datasheet-demo SPI5` returns `[X] out_of_scope … not fabricated`.

## 5. Run
```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
cargo test -p embeder-datasheet
cargo run  -p embeder-datasheet --bin datasheet-demo -- USART2
```

## 6. Next
- **Pinmux grounding:** pin/alternate-function tables (ESP32 strapping/input-only
  rules, STM32 AF maps, RP2040 PIO caveats) — same Grounded/citation/refuse contract.
- **Loop integration:** feed grounded register facts into codegen context so firmware
  synthesis cites its sources (ties P2 into the P1 loop).
- **MCP-ification (ADR-0001):** expose this behind the Python Datasheet MCP server;
  the crate is the reference implementation the server mirrors.
- **RAG (prose only):** optional, clearly tiered Advisory, never a source for addresses.
