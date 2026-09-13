//! Embeder MCP client bridge (ADR-0001).
//!
//! The Rust Core is an MCP *client*: it spawns local MCP tool servers and drives them
//! over stdio JSON-RPC. Oracle implementations here ([`McpCompileOracle`]) satisfy the
//! same `core` traits as the in-process ones, so they drop into the verification loop
//! with no loop changes.

pub mod client;
pub mod datasheet;
pub mod oracle;
pub mod sim;

pub use client::McpClient;
pub use datasheet::{GroundedRegisterMap, McpDatasheet};
pub use oracle::McpCompileOracle;
pub use sim::McpSimOracle;
