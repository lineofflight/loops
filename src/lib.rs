//! loops: an open loops ledger for coding agents. The `loops` command
//! line and `loops mcp`, an MCP server over stdio, keep the same ledger:
//! a file of the agent session's own, mirrored to the Herdr terminal's
//! `herdr.loops` label inside Herdr.

pub mod ledger;
pub mod mcp;
pub mod store;
