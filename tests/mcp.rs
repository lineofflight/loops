//! `loops mcp` over real stdio, line by line as an agent speaks to it.

mod common;

use common::Env;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
    /// What `initialize` answered.
    info: Value,
}

impl Server {
    fn start(env: &Env, terminal: Option<&str>) -> Server {
        Server::start_as(env, terminal, None)
    }

    /// Started by the agent session `session`, when given.
    fn start_as(env: &Env, terminal: Option<&str>, session: Option<&str>) -> Server {
        let mut command = env.command(terminal, &env.work().join("sub"));
        if let Some(session) = session {
            command.env("CLAUDE_CODE_SESSION_ID", session);
        }
        let mut child = command
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut server = Server {
            child,
            stdin,
            stdout,
            next: 1,
            info: Value::Null,
        };
        server.info = server.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "0" },
            }),
        );
        server.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        server
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// The result of `method`, skipping any notification before it.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            assert!(
                self.stdout.read_line(&mut line).unwrap() > 0,
                "server hung up"
            );
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == id {
                assert!(message.get("error").is_none(), "{message}");
                return message["result"].clone();
            }
        }
    }

    /// A tool's text, and whether it was an error.
    fn call(&mut self, tool: &str, arguments: Value) -> (String, bool) {
        self.call_with(tool, arguments, json!({}))
    }

    /// `call`, with the `_meta` an agent sends beside it.
    fn call_with(&mut self, tool: &str, arguments: Value, meta: Value) -> (String, bool) {
        let result = self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments, "_meta": meta }),
        );
        let text = result["content"][0]["text"].as_str().unwrap().to_string();
        (text, result["isError"] == true)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn initialize_ships_the_loops_instructions() {
    let env = Env::new();
    let server = Server::start(&env, None);
    assert_eq!(server.info["serverInfo"]["name"], "loops");
    let instructions = server.info["instructions"].as_str().unwrap();
    for rule in [
        "Never add a newly received ask in the turn it was requested",
        "At the start of each new turn, if the user's message skipped past an unanswered question, pending choice, or parked idea from the previous turn, record it immediately before starting on the new ask",
        "Check current loops with `list` before updating, so items dismissed in the UI stay dropped",
        "\"Still open\"",
    ] {
        assert!(
            instructions.contains(rule),
            "missing {rule:?} in {instructions}"
        );
    }
    assert_eq!(instructions, loops::mcp::INSTRUCTIONS);
}

#[test]
fn the_tools_are_the_command_line_s() {
    let env = Env::new();
    let mut server = Server::start(&env, None);
    let tools = server.request("tools/list", json!({}));
    let mut names: Vec<_> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(names, ["add", "clear", "done", "list"]);
}

#[test]
fn outside_herdr_it_keeps_the_workspace_s_ledger() {
    let env = Env::new();
    let mut server = Server::start(&env, None);
    assert_eq!(
        server.call("list", json!({})),
        ("No open loops.".into(), false)
    );
    assert_eq!(
        server.call(
            "add",
            json!({ "items": ["Pick A or B", "pick a or b", "Ship it"] })
        ),
        ("1. Pick A or B\n2. Ship it\n".into(), false)
    );
    // The command line sees the same ledger, from the workspace's root.
    assert_eq!(env.ok(None, &["list"]), "1. Pick A or B\n2. Ship it\n");
    assert_eq!(
        server.call("done", json!({ "items": ["ship"] })),
        ("1. Pick A or B\n".into(), false)
    );
    assert_eq!(
        server.call("clear", json!({})),
        ("No open loops.".into(), false)
    );
    assert!(env.ledgers().is_empty());
}

#[test]
fn inside_herdr_it_keeps_the_terminal_s_label_and_honours_dismissals() {
    let env = Env::new();
    let mut server = Server::start(&env, Some("t8k"));
    server.call("add", json!({ "items": ["Pick A or B", "Parked: evals"] }));
    let label: Value = serde_json::from_str(&env.label("t8k").unwrap()).unwrap();
    assert_eq!(label[0]["t"], "Pick A or B");
    assert!(env.ledgers().is_empty());

    // Dismissed in Herdr's drawer: the next read sees it gone.
    env.set_label("t8k", &json!([label[1]]).to_string());
    assert_eq!(
        server.call("list", json!({})),
        ("1. Parked: evals\n".into(), false)
    );
    assert_eq!(
        server.call("done", json!({ "items": ["1"] })),
        ("No open loops.".into(), false)
    );
    assert_eq!(env.label("t8k"), None);
}

#[test]
fn a_failing_hc_is_a_tool_error_the_agent_reads() {
    let env = Env::new();
    let mut server = Server::start(&env, Some("gone"));
    assert_eq!(
        server.call("list", json!({})),
        ("error: no terminal \"gone\"".into(), true)
    );
}

#[test]
fn a_session_s_ledger_follows_it_to_a_server_started_elsewhere() {
    let env = Env::new();
    let mut server = Server::start_as(&env, Some("t8k"), Some("5e55-10"));
    server.call("add", json!({ "items": ["Pick A or B", "Parked: evals"] }));
    assert_eq!(env.label_texts("t8k"), ["Pick A or B", "Parked: evals"]);
    assert_eq!(env.session_file("5e55-10").unwrap()["terminal"], "t8k");
    // Dismissed in Herdr's drawer: the next read sees it gone.
    env.dismiss_all_but("t8k", &["Parked: evals"]);
    assert_eq!(
        server.call("list", json!({})),
        ("1. Parked: evals\n".into(), false)
    );
    drop(server);

    // Resumed in another terminal, then outside Herdr.
    let mut server = Server::start_as(&env, Some("rig/t3"), Some("5e55-10"));
    assert_eq!(
        server.call("list", json!({})),
        ("1. Parked: evals\n".into(), false)
    );
    assert_eq!(env.label_texts("rig/t3"), ["Parked: evals"]);
    assert_eq!(env.label("t8k"), None);
    drop(server);
    let mut server = Server::start_as(&env, None, Some("5e55-10"));
    assert_eq!(
        server.call("done", json!({ "items": ["parked"] })),
        ("No open loops.".into(), false)
    );
    // The command line, run by the same session, keeps the same ledger.
    assert_eq!(env.session("5e55-10", None, &["list"]), "");
}

#[test]
fn a_call_that_names_its_session_gets_that_session_s_ledger() {
    let env = Env::new();
    // As Codex runs it: no session id or terminal in the environment, the
    // thread's id beside every call.
    let mut server = Server::start(&env, None);
    let meta = json!({ "threadId": "codex-1", "progressToken": 1 });
    assert_eq!(
        server.call_with("add", json!({ "items": ["Theirs"] }), meta.clone()),
        ("1. Theirs\n".into(), false)
    );
    assert_eq!(
        server.call_with("list", json!({}), json!({ "threadId": "codex-2" })),
        ("No open loops.".into(), false)
    );
    assert_eq!(
        server.call("list", json!({})),
        ("No open loops.".into(), false)
    );
    assert_eq!(
        env.session_file("codex-1").unwrap()["loops"][0]["t"],
        "Theirs"
    );
    assert!(env.ledgers().is_empty());
    assert_eq!(
        server.call_with("clear", json!({}), meta),
        ("No open loops.".into(), false)
    );
    assert_eq!(env.session_file("codex-1"), None);
}

#[test]
fn antigravity_names_the_conversation_beside_each_call() {
    let env = Env::new();
    // As Antigravity runs it: the terminal in the environment, no session
    // id, and beside every call the conversation the user started, which
    // a subagent's own is not.
    let mut server = Server::start(&env, Some("t8k"));
    let meta = json!({
        "antigravity.google/conversation_id": "agy-sub",
        "antigravity.google/root_conversation_id": "agy-1",
    });
    assert_eq!(
        server.call_with("add", json!({ "items": ["Theirs"] }), meta),
        ("1. Theirs\n".into(), false)
    );
    assert_eq!(
        env.session_file("agy-1").unwrap()["loops"][0]["t"],
        "Theirs"
    );
    assert_eq!(env.session_file("agy-sub"), None);
    assert_eq!(env.mirror("t8k").unwrap()["session"], "agy-1");
}
