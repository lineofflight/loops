//! The prototype's `test_loops.sh`, case by case, then what the prototype
//! never had: a bare `loops`, and the workspace's own ledger outside
//! Herdr. No run here is an agent session's; `session.rs` has those.

mod common;

use common::Env;
use serde_json::Value;
use std::fs;

const T: Option<&str> = Some("t8k");

fn texts(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap()
}

#[test]
fn outside_herdr_without_a_terminal_it_keeps_the_workspace_s_ledger() {
    let env = Env::new();
    assert_eq!(env.ok(None, &["list"]), "");
    assert_eq!(env.ok(None, &[]), "");
    env.ok(None, &["add", "Pick a name"]);
    assert_eq!(env.ok(None, &[]), "1. Pick a name\n");
    assert_eq!(env.ledgers().len(), 1);
}

#[test]
fn an_empty_ledger_prints_nothing_or_an_empty_array() {
    let env = Env::new();
    assert_eq!(env.ok(T, &["list"]), "");
    assert_eq!(env.ok(T, &["list", "--json"]), "[]\n");
}

#[test]
fn add_keeps_quotes_equals_and_unicode_joins_lines_and_skips_duplicates() {
    let env = Env::new();
    env.ok(T, &["add", "Download Hans's homework?"]);
    env.ok(
        T,
        &[
            "add",
            r#"Positioning: tier=paid, "pro", Raphaëlstraat #1265"#,
        ],
    );
    env.ok(T, &["add", "download hans's homework?"]);
    let printed = env.ok(T, &["add", "Line one\n  line two  "]);
    let expected = [
        "Download Hans's homework?",
        r#"Positioning: tier=paid, "pro", Raphaëlstraat #1265"#,
        "Line one line two",
    ];
    assert_eq!(texts(&env.ok(T, &["list", "--json"])), expected);
    let list = env.ok(T, &["list"]);
    assert_eq!(list, printed);
    assert_eq!(
        list,
        format!(
            "1. {}\n2. {}\n3. {}\n",
            expected[0], expected[1], expected[2]
        )
    );
    assert_eq!(
        env.ok(T, &["list", "--json"]),
        "[\"Download Hans's homework?\", \"Positioning: tier=paid, \\\"pro\\\", Raphaëlstraat #1265\", \"Line one line two\"]\n"
    );
}

#[test]
fn done_by_number_then_by_text_whatever_its_case_and_the_last_removes_the_label() {
    let env = Env::new();
    env.ok(
        T,
        &[
            "add",
            "Download Hans's homework?",
            "Positioning: Raphaëlstraat",
            "Line one",
        ],
    );
    env.ok(T, &["done", "1"]);
    let left = texts(&env.ok(T, &["list", "--json"]));
    assert_eq!(left.len(), 2);
    assert!(left[0].starts_with("Positioning:"));

    assert_eq!(env.ok(T, &["done", "raphaëlstraat"]), "1. Line one\n");
    assert_eq!(env.ok(T, &["list", "--json"]), "[\"Line one\"]\n");

    env.ok(T, &["done", "1"]);
    assert_eq!(env.label("t8k"), None);
    assert_eq!(env.state()["t8k"]["herdr.space"], "sp1");
}

#[test]
fn a_corrupt_label_reads_as_empty_and_starts_over() {
    let env = Env::new();
    env.set_label("t8k", "{not-valid-json");
    assert_eq!(env.ok(T, &["list"]), "");
    env.ok(T, &["add", "Fresh item"]);
    assert_eq!(env.ok(T, &["list", "--json"]), "[\"Fresh item\"]\n");
}

#[test]
fn terminal_flag_names_another_terminal_and_clear_removes_the_label() {
    let env = Env::new();
    env.ok(T, &["add", "Local loop"]);
    env.ok(T, &["--terminal", "rig/t3", "add", "Remote loop"]);
    assert_eq!(
        env.ok(T, &["--terminal=rig/t3", "list", "--json"]),
        "[\"Remote loop\"]\n"
    );
    assert_eq!(
        env.ok(None, &["--terminal", "rig/t3", "list"]),
        "1. Remote loop\n"
    );
    env.ok(T, &["clear"]);
    assert_eq!(env.label("t8k"), None);
    let remote: Value = serde_json::from_str(&env.label("rig/t3").unwrap()).unwrap();
    assert_eq!(remote[0]["t"], "Remote loop");
    assert!(remote[0]["at"].is_u64());
}

#[test]
fn hc_failures_pass_on_its_error_and_exit_code() {
    let env = Env::new();
    let out = env.run(Some("gone"), &["list"]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "error: no terminal \"gone\"\n"
    );
}

#[test]
fn legacy_plain_strings_keep_their_place_and_gain_no_time() {
    let env = Env::new();
    env.set_label("t8k", r#"["Old one"]"#);
    env.ok(T, &["add", "New one"]);
    let label: Value = serde_json::from_str(&env.label("t8k").unwrap()).unwrap();
    assert_eq!(label[0], serde_json::json!({ "t": "Old one" }));
    assert_eq!(label[1]["t"], "New one");
}

#[test]
fn usage_errors_exit_2() {
    let env = Env::new();
    for args in [
        &["add"][..],
        &["done"],
        &["nonsense"],
        &["--terminal"],
        &["-h"],
        &["--workspace", "list"],
    ] {
        let out = env.run(T, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(String::from_utf8_lossy(&out.stderr).starts_with("Usage: loops"));
    }
}

#[test]
fn a_bare_loops_lists() {
    let env = Env::new();
    env.ok(T, &["add", "One"]);
    assert_eq!(env.ok(T, &[]), "1. One\n");
}

#[test]
fn the_workspace_ledger_is_kept_locally_and_shared_across_its_folders() {
    let env = Env::new();
    assert_eq!(env.ok(None, &["list"]), "");
    env.ok(None, &["add", "Pick a name"]);
    let sub = env
        .command(None, &env.work().join("sub"))
        .args(["list"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&sub.stdout), "1. Pick a name\n");

    let ledgers = env.ledgers();
    assert_eq!(ledgers.len(), 1);
    let file: Value = serde_json::from_str(&fs::read_to_string(&ledgers[0]).unwrap()).unwrap();
    let work = env.work().canonicalize().unwrap();
    assert_eq!(file["workspace"], work.to_string_lossy().as_ref());
    assert_eq!(file["loops"][0]["t"], "Pick a name");
    assert!(file["loops"][0]["at"].is_u64());
    // Herdr's terminal is untouched.
    assert_eq!(env.label("t8k"), None);
}

#[test]
fn inside_herdr_the_terminal_s_label_is_the_ledger_and_no_file_is_kept() {
    let env = Env::new();
    env.ok(T, &["add", "On the label"]);
    assert_eq!(env.label_texts("t8k"), ["On the label"]);
    assert!(!env.state_dir().exists());
    assert_eq!(env.ok(None, &["list"]), "");
}

#[test]
fn the_workspace_ledger_goes_once_empty_and_recovers_from_corruption() {
    let env = Env::new();
    env.ok(None, &["add", "One", "Two"]);
    env.ok(None, &["done", "one"]);
    assert_eq!(env.ok(None, &["list", "--json"]), "[\"Two\"]\n");
    env.ok(None, &["clear"]);
    assert!(env.ledgers().is_empty());

    env.ok(None, &["add", "Three"]);
    fs::write(&env.ledgers()[0], "{not-valid-json").unwrap();
    assert_eq!(env.ok(None, &["list"]), "");
    env.ok(None, &["add", "Four"]);
    assert_eq!(env.ok(None, &["list"]), "1. Four\n");
}

#[test]
fn other_workspaces_keep_their_own() {
    let env = Env::new();
    let other = env.dir.path().join("other");
    fs::create_dir_all(&other).unwrap();
    env.ok(None, &["add", "Here"]);
    let out = env.command(None, &other).args(["list"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "");
}
