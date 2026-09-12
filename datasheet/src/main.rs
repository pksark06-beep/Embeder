//! `datasheet-demo` — print a grounded register map for a peripheral.
//!   datasheet-demo [PERIPHERAL]     (default: USART2)

use embeder_datasheet::DatasheetIndex;
use std::path::PathBuf;

fn main() {
    let svd = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("stm32f4-mini.svd");
    let idx = DatasheetIndex::from_svd_file(&svd).expect("load svd");

    println!("=== Embeder P2 datasheet grounding ===");
    println!("device: {}   (source: {})", idx.device.name, idx.source);

    let peripheral = std::env::args().nth(1).unwrap_or_else(|| "USART2".to_string());
    let g = idx.register_map(&peripheral);

    match &g.value {
        Some(v) => {
            println!(
                "\n{} @ base {:#010x}   [{} {}]",
                v.name,
                v.base_address,
                g.tier.symbol(),
                g.tier.as_str()
            );
            if let Some(c) = &g.citation {
                println!("cited: {} :: {}", c.source, c.path);
            }
            for r in &v.registers {
                println!(
                    "  {:<8} off {:#06x}  abs {:#010x}  {}",
                    r.name,
                    r.offset,
                    r.absolute_address,
                    r.description.clone().unwrap_or_default()
                );
                for f in &r.fields {
                    let bits = if f.bit_width > 1 {
                        format!("bits {}..{}", f.bit_offset, f.bit_offset + f.bit_width - 1)
                    } else {
                        format!("bit {}", f.bit_offset)
                    };
                    println!(
                        "      .{:<10} {:<10} {}",
                        f.name,
                        bits,
                        f.description.clone().unwrap_or_default()
                    );
                }
            }
        }
        None => println!("\n[{} {}] {}", g.tier.symbol(), g.tier.as_str(), g.note),
    }
}
