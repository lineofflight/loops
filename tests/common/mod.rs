//! A `loops` binary run with its own home, no Herdr or agent session of
//! the user's, and a mock `hc` in place of Herdr's: the prototype's, which
//! keeps every terminal's labels in a JSON file.

#![allow(dead_code)]

use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

/// The variables that would move or reveal the user's own files.
const XDG: [&str; 5] = [
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "XDG_STATE_HOME",
    "XDG_RUNTIME_DIR",
];

const MOCK_HC: &str = r#"#!/usr/bin/env python3
import json, os, sys

state_file = os.environ["STATE_FILE"]
with open(state_file, "r", encoding="utf-8") as f:
    state = json.load(f)

args = sys.argv[1:]
if args and args[0] == "get":
    args = [a for a in args[1:] if a != "--json"]
    term = args[0]
    if term not in state:
        sys.stderr.write(f'error: no terminal "{term}"\n')
        sys.exit(3)
    print(json.dumps({"address": term, "state": "live", "labels": state[term]}))
    sys.exit(0)

if args and args[0] == "label":
    args = [a for a in args[1:] if a != "--json"]
    term = args[0]
    if term not in state:
        sys.stderr.write(f'error: no terminal "{term}"\n')
        sys.exit(3)
    for kv in args[1:]:
        k, v = kv.split("=", 1)
        if v == "":
            state[term].pop(k, None)
        else:
            state[term][k] = v
    with open(state_file, "w", encoding="utf-8") as f:
        json.dump(state, f)
    print(json.dumps({"address": term, "state": "live", "labels": state[term]}))
    sys.exit(0)

sys.stderr.write(f"unexpected hc args: {args}\n")
sys.exit(1)
"#;

pub struct Env {
    pub dir: TempDir,
}

impl Env {
    /// Terminals `t8k` and `rig/t3`, and a workspace, `work`, a
    /// repository with a `sub` folder.
    pub fn new() -> Env {
        let dir = tempfile::tempdir().unwrap();
        let env = Env { dir };
        fs::create_dir_all(env.home()).unwrap();
        fs::create_dir_all(env.work().join("sub")).unwrap();
        fs::create_dir_all(env.work().join(".git")).unwrap();
        env.set_state(&json!({ "t8k": { "herdr.space": "sp1" }, "rig/t3": {} }));
        let hc = env.hc();
        fs::write(&hc, MOCK_HC).unwrap();
        fs::set_permissions(&hc, fs::Permissions::from_mode(0o755)).unwrap();
        env
    }

    pub fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    pub fn work(&self) -> PathBuf {
        self.dir.path().join("work")
    }

    pub fn hc(&self) -> PathBuf {
        self.dir.path().join("hc")
    }

    pub fn state_file(&self) -> PathBuf {
        self.dir.path().join("state.json")
    }

    pub fn state(&self) -> Value {
        serde_json::from_str(&fs::read_to_string(self.state_file()).unwrap()).unwrap()
    }

    pub fn set_state(&self, state: &Value) {
        fs::write(self.state_file(), state.to_string()).unwrap();
    }

    /// The `herdr.loops` label of `terminal`, if set.
    pub fn label(&self, terminal: &str) -> Option<String> {
        self.state()[terminal]["herdr.loops"]
            .as_str()
            .map(String::from)
    }

    /// The `herdr.loops.mirror` label of `terminal`, if set: the session
    /// that last mirrored its ledger there, and the loops as it left them.
    pub fn mirror(&self, terminal: &str) -> Option<Value> {
        let mirror = self.state()[terminal]["herdr.loops.mirror"]
            .as_str()
            .map(String::from)?;
        Some(serde_json::from_str(&mirror).unwrap())
    }

    pub fn set_label(&self, terminal: &str, value: &str) {
        let mut state = self.state();
        state[terminal]["herdr.loops"] = Value::String(value.into());
        self.set_state(&state);
    }

    /// The texts on the `herdr.loops` label of `terminal`.
    pub fn label_texts(&self, terminal: &str) -> Vec<String> {
        let label = self.label(terminal).unwrap_or_else(|| "[]".into());
        let entries: Vec<Value> = serde_json::from_str(&label).unwrap();
        entries
            .iter()
            .map(|e| e["t"].as_str().unwrap().to_string())
            .collect()
    }

    /// Dismisses from `terminal`'s label every loop but those in `keep`,
    /// as Herdr's drawer does: the last one gone removes the label.
    pub fn dismiss_all_but(&self, terminal: &str, keep: &[&str]) {
        let label = self.label(terminal).unwrap_or_else(|| "[]".into());
        let entries: Vec<Value> = serde_json::from_str(&label).unwrap();
        let left: Vec<_> = entries
            .into_iter()
            .filter(|e| keep.contains(&e["t"].as_str().unwrap()))
            .collect();
        let mut state = self.state();
        if left.is_empty() {
            state[terminal]
                .as_object_mut()
                .unwrap()
                .remove("herdr.loops");
        } else {
            state[terminal]["herdr.loops"] = Value::String(Value::Array(left).to_string());
        }
        self.set_state(&state);
    }

    pub fn state_dir(&self) -> PathBuf {
        self.home().join(".local/state/loops")
    }

    /// The workspace ledgers on disk.
    pub fn ledgers(&self) -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(self.state_dir()) else {
            return Vec::new();
        };
        entries
            .map(|e| e.unwrap().path())
            .filter(|path| path.is_file())
            .collect()
    }

    /// Where the agent session `id` keeps its ledger.
    pub fn session_path(&self, id: &str) -> PathBuf {
        self.state_dir().join("sessions").join(format!("{id}.json"))
    }

    /// What the agent session `id`'s file holds, if there is one.
    pub fn session_file(&self, id: &str) -> Option<Value> {
        let text = fs::read_to_string(self.session_path(id)).ok()?;
        Some(serde_json::from_str(&text).unwrap())
    }

    /// `loops`, in `cwd`, inside Herdr's `terminal` when given.
    pub fn command(&self, terminal: Option<&str>, cwd: &Path) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_loops"));
        cmd.current_dir(cwd)
            .env("HOME", self.home())
            .env("HC_BIN", self.hc())
            .env("STATE_FILE", self.state_file())
            .env_remove("HC_TERMINAL_ID");
        for var in XDG.iter().chain(&loops::store::SESSION_VARS) {
            cmd.env_remove(var);
        }
        if let Some(terminal) = terminal {
            cmd.env("HC_TERMINAL_ID", terminal);
        }
        cmd
    }

    pub fn run(&self, terminal: Option<&str>, args: &[&str]) -> Output {
        self.command(terminal, &self.work())
            .args(args)
            .output()
            .unwrap()
    }

    /// stdout of a run that must succeed, as the agent session `id`
    /// makes it.
    pub fn session(&self, id: &str, terminal: Option<&str>, args: &[&str]) -> String {
        let out = self
            .command(terminal, &self.work())
            .env("CLAUDE_CODE_SESSION_ID", id)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "loops {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    /// stdout of a run that must succeed.
    pub fn ok(&self, terminal: Option<&str>, args: &[&str]) -> String {
        let out = self.run(terminal, args);
        assert!(
            out.status.success(),
            "loops {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }
}
