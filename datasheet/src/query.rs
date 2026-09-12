//! Grounded queries over a CMSIS-SVD device. Every answer carries a confidence tier
//! and a citation; a miss is REFUSED (OutOfScope), never fabricated (ADR-0002).

use crate::model::{Device, Field, Peripheral};
use crate::svd::parse_svd;
use embeder_core::Tier;
use std::path::Path;

/// Where a fact came from — the auditable pointer that makes grounding real.
#[derive(Debug, Clone)]
pub struct Citation {
    pub source: String, // the SVD file
    pub path: String,   // node path within it, e.g. device/peripherals/USART2/registers/CR1
}

/// A fact plus its confidence tier and provenance. `value` is `None` on a refusal.
#[derive(Debug, Clone)]
pub struct Grounded<T> {
    pub value: Option<T>,
    pub tier: Tier,
    pub citation: Option<Citation>,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct RegisterView {
    pub name: String,
    pub offset: u64,
    pub absolute_address: u64,
    pub description: Option<String>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct PeripheralView {
    pub name: String,
    pub base_address: u64,
    pub registers: Vec<RegisterView>,
}

pub struct DatasheetIndex {
    pub device: Device,
    pub source: String,
}

impl DatasheetIndex {
    pub fn from_svd_str(xml: &str, source: impl Into<String>) -> Result<Self, String> {
        Ok(Self {
            device: parse_svd(xml)?,
            source: source.into(),
        })
    }

    pub fn from_svd_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let p = path.as_ref();
        let xml = std::fs::read_to_string(p).map_err(|e| format!("read {}: {}", p.display(), e))?;
        let source = p
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| p.display().to_string());
        Self::from_svd_str(&xml, source)
    }

    fn view_peripheral(&self, p: &Peripheral) -> PeripheralView {
        let registers = p
            .registers
            .iter()
            .map(|r| RegisterView {
                name: r.name.clone(),
                offset: r.address_offset,
                absolute_address: p.base_address + r.address_offset,
                description: r.description.clone(),
                fields: r.fields.clone(),
            })
            .collect();
        PeripheralView {
            name: p.name.clone(),
            base_address: p.base_address,
            registers,
        }
    }

    fn cite(&self, path: String) -> Citation {
        Citation {
            source: self.source.clone(),
            path,
        }
    }

    fn refuse<T>(&self, note: String) -> Grounded<T> {
        Grounded {
            value: None,
            tier: Tier::OutOfScope,
            citation: None,
            note,
        }
    }

    /// Authoritative register map for a peripheral: Verified on a hit, refused on a miss.
    pub fn register_map(&self, peripheral: &str) -> Grounded<PeripheralView> {
        match self.device.peripheral(peripheral) {
            Some(p) => Grounded {
                value: Some(self.view_peripheral(p)),
                tier: Tier::Verified,
                citation: Some(self.cite(format!("device/peripherals/{}", p.name))),
                note: "CMSIS-SVD structured source".to_string(),
            },
            None => self.refuse(format!(
                "peripheral '{}' not in structured source; not fabricated",
                peripheral
            )),
        }
    }

    /// A single register (absolute address + fields), grounded the same way.
    pub fn lookup_register(&self, peripheral: &str, register: &str) -> Grounded<RegisterView> {
        let Some(p) = self.device.peripheral(peripheral) else {
            return self.refuse(format!(
                "peripheral '{}' not in structured source; not fabricated",
                peripheral
            ));
        };
        match p.register(register) {
            Some(r) => Grounded {
                value: Some(RegisterView {
                    name: r.name.clone(),
                    offset: r.address_offset,
                    absolute_address: p.base_address + r.address_offset,
                    description: r.description.clone(),
                    fields: r.fields.clone(),
                }),
                tier: Tier::Verified,
                citation: Some(
                    self.cite(format!("device/peripherals/{}/registers/{}", p.name, r.name)),
                ),
                note: "CMSIS-SVD structured source".to_string(),
            },
            None => self.refuse(format!(
                "register '{}.{}' not in structured source; not fabricated",
                peripheral, register
            )),
        }
    }
}
