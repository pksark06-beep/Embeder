// SPDX-License-Identifier: MPL-2.0
//! P2 grounding tests. The addresses asserted here are cross-checked against the
//! exact registers the P1 firmware uses (firmware/stm32-blink-uart/src/main.c) and
//! that Renode verified — so "the datasheet layer agrees with reality".

use embeder_core::Tier;
use embeder_datasheet::*;
use std::path::PathBuf;

fn index() -> DatasheetIndex {
    let svd = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("stm32f4-mini.svd");
    DatasheetIndex::from_svd_file(&svd).expect("load svd")
}

#[test]
fn usart2_addresses_match_hardware() {
    let idx = index();
    let g = idx.register_map("USART2");
    assert_eq!(g.tier, Tier::Verified);
    assert!(g.citation.is_some(), "a grounded fact must carry a citation");

    let v = g.value.expect("USART2 present");
    assert_eq!(v.base_address, 0x4000_4400);
    let addr = |n: &str| {
        v.registers
            .iter()
            .find(|r| r.name == n)
            .map(|r| r.absolute_address)
    };
    assert_eq!(addr("SR"), Some(0x4000_4400));
    assert_eq!(addr("DR"), Some(0x4000_4404));
    assert_eq!(addr("BRR"), Some(0x4000_4408));
    assert_eq!(addr("CR1"), Some(0x4000_440C));
}

#[test]
fn usart2_cr1_fields_are_correct() {
    let idx = index();
    let r = idx
        .lookup_register("USART2", "CR1")
        .value
        .expect("CR1 present");
    let bit = |n: &str| r.fields.iter().find(|f| f.name == n).map(|f| f.bit_offset);
    assert_eq!(bit("UE"), Some(13));
    assert_eq!(bit("TE"), Some(3));
}

#[test]
fn gpioa_and_rcc_cross_check_firmware() {
    let idx = index();

    let gpioa = idx.register_map("GPIOA").value.unwrap();
    let ga = |n: &str| {
        gpioa
            .registers
            .iter()
            .find(|r| r.name == n)
            .map(|r| r.absolute_address)
    };
    assert_eq!(ga("MODER"), Some(0x4002_0000));
    assert_eq!(ga("ODR"), Some(0x4002_0014));
    assert_eq!(ga("AFRL"), Some(0x4002_0020));

    let rcc = idx.register_map("RCC").value.unwrap();
    let ra = |n: &str| {
        rcc.registers
            .iter()
            .find(|r| r.name == n)
            .map(|r| r.absolute_address)
    };
    assert_eq!(ra("AHB1ENR"), Some(0x4002_3830));
    assert_eq!(ra("APB1ENR"), Some(0x4002_3840));
}

#[test]
fn unknown_peripheral_is_refused_not_fabricated() {
    let idx = index();
    let g = idx.register_map("DEFINITELY_NOT_A_PERIPHERAL");
    assert!(g.value.is_none(), "must not invent data");
    assert_eq!(g.tier, Tier::OutOfScope, "a miss is refused, not guessed");
    assert!(g.citation.is_none());
}

#[test]
fn lookups_are_case_insensitive() {
    let idx = index();
    assert!(idx.register_map("usart2").value.is_some());
    assert!(idx.lookup_register("Usart2", "cr1").value.is_some());
}
