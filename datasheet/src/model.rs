//! The parsed CMSIS-SVD data model (the subset Embeder grounds against).

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub description: Option<String>,
    pub bit_offset: u32,
    pub bit_width: u32,
}

#[derive(Debug, Clone)]
pub struct Register {
    pub name: String,
    pub description: Option<String>,
    pub address_offset: u64,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Peripheral {
    pub name: String,
    pub description: Option<String>,
    pub base_address: u64,
    pub derived_from: Option<String>,
    pub registers: Vec<Register>,
}

#[derive(Debug, Clone)]
pub struct Device {
    pub name: String,
    pub description: Option<String>,
    pub peripherals: Vec<Peripheral>,
}

impl Device {
    pub fn peripheral(&self, name: &str) -> Option<&Peripheral> {
        self.peripherals
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }
}

impl Peripheral {
    pub fn register(&self, name: &str) -> Option<&Register> {
        self.registers
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(name))
    }
}
