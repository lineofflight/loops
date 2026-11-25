//! `loops mcp`: the ledger as an MCP server over stdio, its tools the
//! command line's own, and the instructions that teach an agent when to
//! use them, sent once, in `initialize`. Run by the agent, it inherits the
//! agent's session id, `HC_TERMINAL_ID` and working directory, so it finds
//! the same ledger the command line would there. An agent that names the
//! session in each call instead, as Codex and Antigravity do in `_meta`,
//! gets that session's.

use crate::ledger::{self, Entry};
use crate::store::{Error, Store};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, schemars, tool, tool_handler,
    tool_router,
};
use std::io;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// How to keep the ledger, sent to every agent as it connects.
pub const INSTRUCTIONS: &str = "\
Keep a ledger of open loops with this server's tools: questions the user hasn't answered, asks you parked or couldn't finish, and things waiting on the user or a background job. The ledger is where the user sees them, so never write an open loops footer or list (\"Still open\", \"Carried\" or the like) in your replies, even if earlier replies had one; move its items into the ledger instead.

- Never add a newly received ask in the turn it was requested, nor anything the user puts off in that same message (\"later\", \"I'll say when\").
- At the start of each new turn, if the user's message skipped past an unanswered question, pending choice, or parked idea from the previous turn, record it immediately before starting on the new ask: `list`, then `add`, before any other tool call or any reply.
- Treat a task you handed the user to do outside the chat (a message to send, a booking to fix) like an unanswered question, even with no question asked: if their next message moves on without saying they're on it, done with it, or dropping it, they may have missed it, so record it the same way.
- Check current loops with `list` before updating, so items dismissed in the UI stay dropped. Also call `list` first in every turn that follows one which left loops in the ledger, even if you don't mean to change it. A loop you recorded that `list` no longer shows was dismissed: never add it back or ask about it again.
- Name each item in a few words; don't re-ask it in full.
- Leave out work already in hand (a running job, another agent on it) until it has been going for more than two hours; then record it once.
- Leave out waits on a third party, however long (a support ticket, an email reply, a vendor's or partner's approval): nobody here can act on them yet.
- Close an item with `done` once it's answered, done, or the user says to drop it. Close or update a background job's item in the turn its result lands.
- Carry open items through compaction summaries and handoffs.";

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddArgs {
    /// Each open loop, in a few words.
    #[schemars(length(min = 1))]
    pub items: Vec<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DoneArgs {
    /// Each loop to close: its number in `list`, or text found in it,
    /// whatever its case.
    #[schemars(length(min = 1))]
    pub items: Vec<String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// The ledger as the agent reads it after a call, or what went wrong.
fn reply(result: Result<Vec<Entry>, Error>) -> CallToolResult {
    match result {
        Ok(entries) if entries.is_empty() => {
            CallToolResult::success(vec![ContentBlock::text("No open loops.")])
        }
        Ok(entries) => {
            CallToolResult::success(vec![ContentBlock::text(ledger::numbered(&entries))])
        }
        Err(e) => CallToolResult::error(vec![ContentBlock::text(e.message)]),
    }
}

/// Where agents that give their MCP servers no session id in the
/// environment name the session beside each call, in `_meta`: Codex its
/// thread, Antigravity the conversation the user started, which a
/// subagent's call names too.
const SESSION_META: [&str; 2] = ["threadId", "antigravity.google/root_conversation_id"];

/// Finds the ledger of a session, the server's own when none is named.
pub type Open = dyn Fn(Option<String>) -> Result<Store, Error> + Send + Sync;

#[derive(Clone)]
pub struct Loops {
    open: Arc<Open>,
    tool_router: ToolRouter<Loops>,
}

#[tool_router]
impl Loops {
    pub fn new(open: Arc<Open>) -> Self {
        Self {
            open,
            tool_router: Self::tool_router(),
        }
    }

    /// The ledger of the session a call is made in.
    fn store(&self, context: &RequestContext<RoleServer>) -> Result<Store, Error> {
        let session = SESSION_META
            .iter()
            .find_map(|name| context.meta.get(*name).and_then(|id| id.as_str()));
        (self.open)(session.map(String::from))
    }

    #[tool(
        name = "list",
        description = "This session's open loops, numbered. Call it first in a turn, before any other tool call or reply, whenever the user's message skipped past something or the ledger may hold loops, so loops the user dismissed stay dropped."
    )]
    async fn list(&self, context: RequestContext<RoleServer>) -> Result<CallToolResult, McpError> {
        Ok(reply(self.store(&context).and_then(|store| store.load())))
    }

    #[tool(
        name = "add",
        description = "Record open loops, a few words each: a question the user skipped past, a pending choice, a parked idea, a task you handed the user that they moved past unconfirmed, or something waiting on the user or a background job. Never one raised in this turn, anything the user put off in it, or a wait on a third party, such as a support ticket or an email reply. Ones already there are skipped. Returns the ledger."
    )]
    async fn add(
        &self,
        Parameters(AddArgs { items }): Parameters<AddArgs>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(reply(self.store(&context).and_then(|store| {
            let mut entries = store.load()?;
            if ledger::add(&mut entries, &items, now()) {
                store.save(&entries)?;
            }
            Ok(entries)
        })))
    }

    #[tool(
        name = "done",
        description = "Close open loops once answered, done or dropped, each by its number in `list` or by text found in it. Returns the ledger."
    )]
    async fn done(
        &self,
        Parameters(DoneArgs { items }): Parameters<DoneArgs>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(reply(self.store(&context).and_then(|store| {
            let entries = store.load()?;
            let left = ledger::done(&entries, &items);
            if left != entries {
                store.save(&left)?;
            }
            Ok(left)
        })))
    }

    #[tool(
        name = "clear",
        description = "Close every open loop. Only when the user asks to clear them all."
    )]
    async fn clear(&self, context: RequestContext<RoleServer>) -> Result<CallToolResult, McpError> {
        Ok(reply(
            self.store(&context)
                .and_then(|store| store.save(&[]))
                .map(|()| Vec::new()),
        ))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Loops {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("loops", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

/// Serves the ledgers `open` finds over stdin and stdout until the agent
/// hangs up.
pub fn serve(
    open: impl Fn(Option<String>) -> Result<Store, Error> + Send + Sync + 'static,
) -> io::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let service = Loops::new(Arc::new(open))
            .serve(rmcp::transport::stdio())
            .await
            .map_err(io::Error::other)?;
        service.waiting().await.map_err(io::Error::other)?;
        Ok(())
    })
}
