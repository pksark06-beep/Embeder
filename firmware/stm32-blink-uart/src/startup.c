/* SPDX-License-Identifier: MPL-2.0 */
/*
 * Minimal Cortex-M startup + vector table for STM32F4 (C, no assembly).
 * Sets the initial stack, copies .data, zeroes .bss, then calls main().
 */
#include <stdint.h>

extern uint32_t _sidata;  /* .data init values in FLASH (LMA) */
extern uint32_t _sdata;   /* .data start in RAM (VMA)         */
extern uint32_t _edata;
extern uint32_t _sbss;
extern uint32_t _ebss;
extern uint32_t _estack;  /* top of stack (from linker script) */

int  main(void);
void Reset_Handler(void);
void Default_Handler(void);

/* The first 16 entries: initial SP + the Cortex-M system exception vectors. */
__attribute__((section(".isr_vector"), used))
void (*const g_pfnVectors[])(void) = {
    (void (*)(void))(&_estack), /* 0  Initial stack pointer */
    Reset_Handler,              /* 1  Reset                 */
    Default_Handler,            /* 2  NMI                   */
    Default_Handler,            /* 3  HardFault             */
    Default_Handler,            /* 4  MemManage             */
    Default_Handler,            /* 5  BusFault              */
    Default_Handler,            /* 6  UsageFault            */
    0, 0, 0, 0,                 /* 7-10 Reserved            */
    Default_Handler,            /* 11 SVCall                */
    Default_Handler,            /* 12 Debug Monitor         */
    0,                          /* 13 Reserved              */
    Default_Handler,            /* 14 PendSV                */
    Default_Handler,            /* 15 SysTick               */
};

void Reset_Handler(void)
{
    uint32_t *src = &_sidata;
    uint32_t *dst = &_sdata;
    while (dst < &_edata) {
        *dst++ = *src++;
    }
    for (dst = &_sbss; dst < &_ebss; ) {
        *dst++ = 0u;
    }

    main();

    while (1) { }
}

void Default_Handler(void)
{
    while (1) { }
}
