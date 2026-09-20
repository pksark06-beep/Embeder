// SPDX-License-Identifier: MPL-2.0
//! Minimal, dependency-free JSON string helpers — enough to emit provenance JSONL
//! and boundary objects without pulling in serde. Not a parser; output only.

/// Escape a string for embedding inside a JSON double-quoted string.
pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// Quote and escape a string as a JSON string literal.
pub fn quote(s: &str) -> String {
    format!("\"{}\"", escape(s))
}

/// Render a slice of strings as a JSON array of strings.
pub fn str_array(items: &[String]) -> String {
    let inner: Vec<String> = items.iter().map(|s| quote(s)).collect();
    format!("[{}]", inner.join(","))
}
