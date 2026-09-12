# stm32-blink-uart — the P1 exit-criterion target

The smallest project that exercises the whole loop end-to-end on a Cortex-M target:

- **Blink:** toggle the user LED (PA5 on an STM32F4-Discovery-class board).
- **UART:** print a banner over USART2 TX.

Why this target: it touches `cpu_boot`, `register_init`, `gpio_toggle`, and `uart_tx`
— all of which Renode models deterministically for STM32 (see the Verifiability
Matrix in `docs/SYSTEM_SPEC.md`). It touches **no** analog or RF, so a VERIFIED result
here is honest with no ⚠️ caveats on the exercised behavior.

## Status
- P1a: represented by fixture sources in `reference-impl/embeder/demo.py`.
- P1b (pending `arm-none-eabi-gcc` + `Renode`): real `main.c`, startup file, linker
  script, and a `.resc` Renode platform script to boot the ELF and capture USART2.
