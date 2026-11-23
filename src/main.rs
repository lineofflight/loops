//! The `loops` command line, as the Python prototype had it, plus an
//! agent session's own ledger, a workspace's outside Herdr, and `loops
//! mcp`.

use loops::ledger;
use loops::store::{self, Error, Store};
use std::io::Write;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

const USAGE: &str = "Usage: loops [--terminal <id>] [list [--json] | add <text>... | done <idx-or-match>... | clear | mcp]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    ExitCode::from(run(&args))
}

fn usage() -> u8 {
    eprintln!("{USAGE}");
    2
}

fn failed(e: Error) -> u8 {
    eprintln!("{}", e.message);
    u8::try_from(e.code)
        .ok()
        .filter(|code| *code != 0)
        .unwrap_or(1)
}

/// Prints to stdout, a reader that went away aside.
fn print(text: &str) {
    let _ = std::io::stdout().lock().write_all(text.as_bytes());
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

fn run(argv: &[String]) -> u8 {
    let mut terminal = None;
    let mut i = 0;
    while let Some(arg) = argv.get(i) {
        if arg == "--terminal" {
            let Some(id) = argv.get(i + 1) else {
                return usage();
            };
            terminal = Some(id.trim().to_string());
            i += 2;
        } else if let Some(id) = arg.strip_prefix("--terminal=") {
            terminal = Some(id.trim().to_string());
            i += 1;
        } else if arg == "-h" || arg == "--help" {
            return usage();
        } else {
            break;
        }
    }
    let (cmd, args) = match argv[i..].split_first() {
        Some((cmd, args)) => (cmd.as_str(), args),
        None => ("list", &[][..]),
    };

    // A terminal given outright is that terminal's label, whoever asks:
    // how one session hands a loop to another. Else the ledger of the
    // session the command runs in, which a call to the MCP server may
    // name itself; with no session, the terminal's label inside Herdr
    // and the workspace's file outside it.
    let terminal = terminal.filter(|id| !id.is_empty());
    let open = move |session: Option<String>| match &terminal {
        Some(terminal) => Ok(Store::Herdr {
            terminal: terminal.clone(),
        }),
        None => Store::open(
            session.or_else(store::session_from_env),
            store::terminal_from_env(),
        ),
    };
    if cmd == "mcp" {
        return match loops::mcp::serve(open) {
            Ok(()) => 0,
            Err(e) => failed(Error {
                message: format!("loops: {e}"),
                code: 1,
            }),
        };
    }
    let store = match open(None) {
        Ok(store) => store,
        Err(e) => return failed(e),
    };

    let result = match cmd {
        "list" => store.load().map(|entries| {
            if args.iter().any(|arg| arg == "--json") {
                print(&format!("{}\n", ledger::json(&entries)));
            } else {
                print(&ledger::numbered(&entries));
            }
        }),
        "add" if !args.is_empty() => store.load().and_then(|mut entries| {
            if ledger::add(&mut entries, args, now()) {
                store.save(&entries)?;
            }
            print(&ledger::numbered(&entries));
            Ok(())
        }),
        "done" if !args.is_empty() => store.load().and_then(|entries| {
            let left = ledger::done(&entries, args);
            if left != entries {
                store.save(&left)?;
            }
            print(&ledger::numbered(&left));
            Ok(())
        }),
        "clear" => store.save(&[]),
        _ => return usage(),
    };
    match result {
        Ok(()) => 0,
        Err(e) => failed(e),
    }
}
