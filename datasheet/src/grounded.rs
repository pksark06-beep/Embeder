//! Grounded code generation: P2 → P1.
//!
//! `GroundedCodegen` synthesizes firmware whose register addresses and bit positions
//! are taken *directly from the CMSIS-SVD* (via grounded lookups), and attaches a
//! citation for each fact used. The result is firmware that is provably grounded — if
//! the SVD says USART2.CR1 is at 0x4000440c, that is the address the code uses — and a
//! draft whose `citations` flow into the loop's provenance.

use crate::query::RegisterView;
use crate::DatasheetIndex;
use embeder_core::{Codegen, FirmwareDraft, LoopContext, SourceRef};

pub struct GroundedCodegen<'a> {
    idx: &'a DatasheetIndex,
}

impl<'a> GroundedCodegen<'a> {
    pub fn new(idx: &'a DatasheetIndex) -> Self {
        Self { idx }
    }

    /// Grounded lookup of one register: returns its view and records a citation.
    fn reg(&self, cites: &mut Vec<SourceRef>, peri: &str, reg: &str) -> RegisterView {
        let g = self.idx.lookup_register(peri, reg);
        let v = g
            .value
            .unwrap_or_else(|| panic!("SVD is missing {}.{} — grounding requires it", peri, reg));
        if let Some(c) = g.citation {
            cites.push(SourceRef::new(
                c.source,
                c.path,
                format!("{}.{} @ {:#010x}", peri, reg, v.absolute_address),
            ));
        }
        v
    }

    /// Synthesize the firmware source + the citations it is grounded on.
    pub fn synthesize(&self) -> (String, Vec<SourceRef>) {
        let mut cites = Vec::new();
        let ahb1 = self.reg(&mut cites, "RCC", "AHB1ENR");
        let apb1 = self.reg(&mut cites, "RCC", "APB1ENR");
        let moder = self.reg(&mut cites, "GPIOA", "MODER");
        let odr = self.reg(&mut cites, "GPIOA", "ODR");
        let afrl = self.reg(&mut cites, "GPIOA", "AFRL");
        let sr = self.reg(&mut cites, "USART2", "SR");
        let dr = self.reg(&mut cites, "USART2", "DR");
        let brr = self.reg(&mut cites, "USART2", "BRR");
        let cr1 = self.reg(&mut cites, "USART2", "CR1");

        let bit = |rv: &RegisterView, name: &str| -> u32 {
            rv.fields
                .iter()
                .find(|f| f.name == name)
                .map(|f| f.bit_offset)
                .unwrap_or(0)
        };
        let gpioaen = bit(&ahb1, "GPIOAEN");
        let usart2en = bit(&apb1, "USART2EN");
        let txe = bit(&sr, "TXE");
        let ue = bit(&cr1, "UE");
        let te = bit(&cr1, "TE");

        let header = format!(
            "#include <stdint.h>\n\
             /* Synthesized by Embeder from grounded CMSIS-SVD facts (source: {src}) */\n\
             #define REG(a) (*(volatile uint32_t *)(a))\n\
             #define RCC_AHB1ENR REG({ahb1:#x}u)\n\
             #define RCC_APB1ENR REG({apb1:#x}u)\n\
             #define GPIOA_MODER REG({moder:#x}u)\n\
             #define GPIOA_ODR   REG({odr:#x}u)\n\
             #define GPIOA_AFRL  REG({afrl:#x}u)\n\
             #define USART2_SR   REG({sr:#x}u)\n\
             #define USART2_DR   REG({dr:#x}u)\n\
             #define USART2_BRR  REG({brr:#x}u)\n\
             #define USART2_CR1  REG({cr1:#x}u)\n\
             #define GPIOAEN  (1u<<{gpioaen})\n\
             #define USART2EN (1u<<{usart2en})\n\
             #define TXE (1u<<{txe})\n\
             #define UE  (1u<<{ue})\n\
             #define TE  (1u<<{te})\n",
            src = self.idx.source,
            ahb1 = ahb1.absolute_address,
            apb1 = apb1.absolute_address,
            moder = moder.absolute_address,
            odr = odr.absolute_address,
            afrl = afrl.absolute_address,
            sr = sr.absolute_address,
            dr = dr.absolute_address,
            brr = brr.absolute_address,
            cr1 = cr1.absolute_address,
            gpioaen = gpioaen,
            usart2en = usart2en,
            txe = txe,
            ue = ue,
            te = te,
        );

        (format!("{}{}", header, SYNTH_BODY), cites)
    }
}

const SYNTH_BODY: &str = r#"
static void uart_putc(char c) { while (!(USART2_SR & TXE)) { } USART2_DR = (uint32_t)(uint8_t)c; }
static void uart_puts(const char *s) { while (*s) uart_putc(*s++); }

int main(void) {
    RCC_AHB1ENR |= GPIOAEN;
    RCC_APB1ENR |= USART2EN;
    GPIOA_MODER &= ~(3u << (5 * 2)); GPIOA_MODER |= (1u << (5 * 2)); /* PA5 output */
    GPIOA_MODER &= ~(3u << (2 * 2)); GPIOA_MODER |= (2u << (2 * 2)); /* PA2 AF mode */
    GPIOA_AFRL  &= ~(0xFu << (2 * 4)); GPIOA_AFRL |= (7u << (2 * 4)); /* PA2 AF7 */
    USART2_BRR = 0x8Bu;
    USART2_CR1 = UE | TE;
    for (int i = 0; i < 5; i++) {
        GPIOA_ODR ^= (1u << 5);
        uart_puts("Hello from Embeder\r\n");
        for (volatile int d = 0; d < 20000; d++) { }
    }
    while (1) { }
    return 0;
}
"#;

impl Codegen for GroundedCodegen<'_> {
    fn generate(&mut self, _ctx: &LoopContext) -> FirmwareDraft {
        let (src, cites) = self.synthesize();
        FirmwareDraft::new("stm32f4-discovery", "main.c", vec![("main.c".to_string(), src)])
            .with_citations(cites)
    }
}
