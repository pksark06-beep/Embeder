// SPDX-License-Identifier: MPL-2.0
//! Append-only provenance ledger (SYSTEM_SPEC §6). Every consequential action the
//! Core takes is recorded so a VERIFIED artifact is auditable and reproducible from
//! its input hash + named oracle versions.
//!
//! NOTE: hashing here uses std's SipHash (non-cryptographic) — adequate as a change
//! fingerprint for P1. Swap to SHA-256 when the ledger becomes an integrity boundary.

use crate::json;
use std::collections::hash_map::DefaultHasher;
use std::fs::{create_dir_all, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn short_hash(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

pub fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[derive(Debug, Clone)]
pub struct ProvenanceRecord {
    pub id: String,
    pub ts: f64,
    pub actor: String, // "oracle" | "user" | "model"
    pub tool: String,  // "compile" | "simulate" | "verify"
    pub inputs_hash: String,
    pub outputs_hash: String,
    pub tier: Option<String>,
    pub oracle_version: String,
    pub detail: String, // a raw JSON fragment supplied by the caller
}

impl ProvenanceRecord {
    pub fn to_json(&self) -> String {
        let tier = match &self.tier {
            Some(t) => json::quote(t),
            None => "null".to_string(),
        };
        format!(
            "{{\"id\":{},\"ts\":{},\"actor\":{},\"tool\":{},\"inputs_hash\":{},\"outputs_hash\":{},\"tier\":{},\"oracle_version\":{},\"detail\":{}}}",
            json::quote(&self.id),
            self.ts,
            json::quote(&self.actor),
            json::quote(&self.tool),
            json::quote(&self.inputs_hash),
            json::quote(&self.outputs_hash),
            tier,
            json::quote(&self.oracle_version),
            self.detail,
        )
    }
}

pub struct ProvenanceLedger {
    path: PathBuf,
    counter: u64,
    pub records: Vec<ProvenanceRecord>,
}

impl ProvenanceLedger {
    pub fn new(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let p = path.as_ref().to_path_buf();
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                create_dir_all(parent)?;
            }
        }
        Ok(Self { path: p, counter: 0, records: Vec::new() })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        actor: &str,
        tool: &str,
        inputs: &str,
        outputs: &str,
        tier: Option<&str>,
        oracle_version: &str,
        detail: &str,
    ) -> String {
        self.counter += 1;
        let ts = now_secs();
        let id = short_hash(&format!("{}-{}-{}", ts, self.counter, tool));
        let rec = ProvenanceRecord {
            id: id.clone(),
            ts,
            actor: actor.to_string(),
            tool: tool.to_string(),
            inputs_hash: short_hash(inputs),
            outputs_hash: short_hash(outputs),
            tier: tier.map(|s| s.to_string()),
            oracle_version: oracle_version.to_string(),
            detail: detail.to_string(),
        };
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = writeln!(f, "{}", rec.to_json());
        }
        self.records.push(rec);
        id
    }
}
