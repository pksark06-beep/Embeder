// SPDX-License-Identifier: MPL-2.0
//! CMSIS-SVD parser (the subset Embeder needs). Read-only, built on roxmltree.
//! Handles `derivedFrom` peripherals and the three common field bit encodings so a
//! full vendor .svd works, not just the bundled excerpt.

use crate::model::{Device, Field, Peripheral, Register};

pub fn parse_svd(xml: &str) -> Result<Device, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("XML parse error: {}", e))?;
    let root = doc.root_element();
    if !root.has_tag_name("device") {
        return Err(format!(
            "expected <device> root, found <{}>",
            root.tag_name().name()
        ));
    }

    let name = child_text(root, "name").unwrap_or_default();
    let description = child_text(root, "description");

    let mut peripherals: Vec<Peripheral> = Vec::new();
    if let Some(ps) = find_child(root, "peripherals") {
        for p in ps
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("peripheral"))
        {
            let pname = child_text(p, "name").ok_or("peripheral without <name>")?;
            let derived_from = p.attribute("derivedFrom").map(|s| s.to_string());
            let base_address = child_text(p, "baseAddress")
                .and_then(|s| parse_u64(&s))
                .unwrap_or(0);
            let description = child_text(p, "description");

            let mut registers = Vec::new();
            if let Some(rs) = find_child(p, "registers") {
                for r in rs
                    .children()
                    .filter(|n| n.is_element() && n.has_tag_name("register"))
                {
                    registers.push(parse_register(r)?);
                }
            }

            peripherals.push(Peripheral {
                name: pname,
                description,
                base_address,
                derived_from,
                registers,
            });
        }
    }

    resolve_derived(&mut peripherals);
    Ok(Device {
        name,
        description,
        peripherals,
    })
}

fn parse_register(r: roxmltree::Node) -> Result<Register, String> {
    let name = child_text(r, "name").ok_or("register without <name>")?;
    let address_offset = child_text(r, "addressOffset")
        .and_then(|s| parse_u64(&s))
        .unwrap_or(0);
    let description = child_text(r, "description");

    let mut fields = Vec::new();
    if let Some(fs) = find_child(r, "fields") {
        for f in fs
            .children()
            .filter(|n| n.is_element() && n.has_tag_name("field"))
        {
            if let Some(field) = parse_field(f) {
                fields.push(field);
            }
        }
    }
    Ok(Register {
        name,
        description,
        address_offset,
        fields,
    })
}

fn parse_field(f: roxmltree::Node) -> Option<Field> {
    let name = child_text(f, "name")?;
    let description = child_text(f, "description");
    let (bit_offset, bit_width) = field_bits(f)?;
    Some(Field {
        name,
        description,
        bit_offset,
        bit_width,
    })
}

/// SVD allows three bit encodings: bitOffset+bitWidth, lsb+msb, or bitRange "[msb:lsb]".
fn field_bits(f: roxmltree::Node) -> Option<(u32, u32)> {
    if let (Some(o), Some(w)) = (
        child_text(f, "bitOffset").and_then(|s| parse_u32(&s)),
        child_text(f, "bitWidth").and_then(|s| parse_u32(&s)),
    ) {
        return Some((o, w));
    }
    if let (Some(lsb), Some(msb)) = (
        child_text(f, "lsb").and_then(|s| parse_u32(&s)),
        child_text(f, "msb").and_then(|s| parse_u32(&s)),
    ) {
        if msb >= lsb {
            return Some((lsb, msb - lsb + 1));
        }
    }
    if let Some(br) = child_text(f, "bitRange") {
        let t = br.trim_matches(|c| c == '[' || c == ']');
        let parts: Vec<&str> = t.split(':').collect();
        if parts.len() == 2 {
            if let (Some(msb), Some(lsb)) = (parse_u32(parts[0]), parse_u32(parts[1])) {
                if msb >= lsb {
                    return Some((lsb, msb - lsb + 1));
                }
            }
        }
    }
    None
}

/// Peripherals declared with `derivedFrom` and no registers of their own inherit the
/// register set of the referenced peripheral (common in vendor SVDs, e.g. USART2 <- USART1).
fn resolve_derived(peripherals: &mut [Peripheral]) {
    let snapshot: Vec<(String, Vec<Register>)> = peripherals
        .iter()
        .map(|p| (p.name.clone(), p.registers.clone()))
        .collect();
    for p in peripherals.iter_mut() {
        if p.registers.is_empty() {
            if let Some(src) = &p.derived_from {
                if let Some((_, regs)) = snapshot.iter().find(|(n, _)| n.eq_ignore_ascii_case(src)) {
                    p.registers = regs.clone();
                }
            }
        }
    }
}

fn find_child<'a, 'input>(
    n: roxmltree::Node<'a, 'input>,
    tag: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    n.children().find(|c| c.is_element() && c.has_tag_name(tag))
}

fn child_text(n: roxmltree::Node, tag: &str) -> Option<String> {
    find_child(n, tag)
        .and_then(|c| c.text())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn parse_u64(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}

fn parse_u32(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}
