/*
 * STM32F4 blink + UART — bare-metal, register level (no HAL, no libc).
 * Toggles PA5 and transmits a banner over USART2 (PA2 = TX, AF7).
 *
 * This is the P1 exit-criterion application. It touches only digital peripherals
 * that Renode models deterministically for STM32 (see SYSTEM_SPEC Verifiability
 * Matrix): GPIO + USART. No analog, no RF.
 */
#include <stdint.h>

/* --- RCC --- */
#define RCC_BASE      0x40023800u
#define RCC_AHB1ENR   (*(volatile uint32_t *)(RCC_BASE + 0x30))
#define RCC_APB1ENR   (*(volatile uint32_t *)(RCC_BASE + 0x40))

/* --- GPIOA --- */
#define GPIOA_BASE    0x40020000u
#define GPIOA_MODER   (*(volatile uint32_t *)(GPIOA_BASE + 0x00))
#define GPIOA_AFRL    (*(volatile uint32_t *)(GPIOA_BASE + 0x20))
#define GPIOA_ODR     (*(volatile uint32_t *)(GPIOA_BASE + 0x14))

/* --- USART2 --- */
#define USART2_BASE   0x40004400u
#define USART2_SR     (*(volatile uint32_t *)(USART2_BASE + 0x00))
#define USART2_DR     (*(volatile uint32_t *)(USART2_BASE + 0x04))
#define USART2_BRR    (*(volatile uint32_t *)(USART2_BASE + 0x08))
#define USART2_CR1    (*(volatile uint32_t *)(USART2_BASE + 0x0C))

#define USART_SR_TXE  (1u << 7)
#define USART_CR1_UE  (1u << 13)
#define USART_CR1_TE  (1u << 3)

static void clocks_init(void)
{
    RCC_AHB1ENR |= (1u << 0);   /* GPIOA clock */
    RCC_APB1ENR |= (1u << 17);  /* USART2 clock */
}

static void led_init(void)
{
    GPIOA_MODER &= ~(3u << (5 * 2));
    GPIOA_MODER |=  (1u << (5 * 2));  /* PA5 = general purpose output */
}

static void led_toggle(void)
{
    GPIOA_ODR ^= (1u << 5);
}

static void uart_init(void)
{
    /* PA2 -> alternate function AF7 (USART2_TX) */
    GPIOA_MODER &= ~(3u << (2 * 2));
    GPIOA_MODER |=  (2u << (2 * 2));   /* AF mode */
    GPIOA_AFRL  &= ~(0xFu << (2 * 4));
    GPIOA_AFRL  |=  (7u << (2 * 4));   /* AF7 */

    USART2_BRR = 0x8Bu;                /* ~115200 @ 16 MHz HSI */
    USART2_CR1 = USART_CR1_UE | USART_CR1_TE;
}

static void uart_putc(char c)
{
    while (!(USART2_SR & USART_SR_TXE)) { }
    USART2_DR = (uint32_t)(uint8_t)c;
}

static void uart_puts(const char *s)
{
    while (*s) {
        uart_putc(*s++);
    }
}

int main(void)
{
    clocks_init();
    led_init();
    uart_init();

    for (int i = 0; i < 5; i++) {
        led_toggle();
        uart_puts("Hello from Embeder\r\n");
        for (volatile int d = 0; d < 20000; d++) { }
    }

    while (1) { }
    return 0;
}
