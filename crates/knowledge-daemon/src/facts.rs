//! Transport observations forwarded unchanged to the native handler.
//!
//! Nothing here treats loopback, stdio, a header, or an identifier as a person,
//! a root grant, or an exposure revision.

use std::net::SocketAddr;

/// Which bounded listener accepted the bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportKind {
    /// `omni serve --stdio` or another direct MCP stdio session.
    McpStdio,
    /// Newline-delimited service envelopes on the loopback IPC listener.
    Ipc,
    /// Opt-in loopback HTTP.
    Http,
}

/// Numeric process-account snapshot supplied by the single account owner.
///
/// The transport does not measure RAM, invent a 50% default, or open a second pool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountFacts {
    /// Effective memory budget, in bytes, as reported by that account.
    pub memory_budget_bytes: u64,
    /// Bytes that account currently reserves.
    pub memory_reserved_bytes: u64,
    /// Effective generated-storage budget, in bytes.
    pub storage_budget_bytes: u64,
    /// Bytes that account currently reserves for generated storage.
    pub storage_reserved_bytes: u64,
    /// Jobs that account reports as waiting for memory.
    pub waiting_for_memory: u64,
}

/// Facts available at one delivery, without an authorization conclusion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportFacts {
    /// Listener class.
    pub kind: TransportKind,
    /// Raw Host header, when the listener is HTTP.
    pub host: Option<String>,
    /// Raw Origin header, when the client sent one.
    pub origin: Option<String>,
    /// Local socket, when the listener is a socket.
    pub local: Option<SocketAddr>,
    /// Peer socket, when the listener is a socket.
    pub peer: Option<SocketAddr>,
    /// Account snapshot, present only when the process account is bound.
    pub account: Option<AccountFacts>,
}

impl TransportFacts {
    /// Stdio facts with no peer and no account.
    pub fn mcp_stdio(account: Option<AccountFacts>) -> Self {
        Self {
            kind: TransportKind::McpStdio,
            host: None,
            origin: None,
            local: None,
            peer: None,
            account,
        }
    }
}
