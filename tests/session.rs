//! An agent session's own ledger: a file found by the session's id, so
//! the loops follow the session from one Herdr terminal to another and
//! out of Herdr, mirrored to the label of the terminal it runs in.

mod common;

use common::Env;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

const S: &str = "5e55-10";
const T: Option<&str> = Some("t8k");
const T3: Option<&str> = Some("rig/t3");

/// Makes `path` look last used `days` ago.
fn age(path: &Path, days: u64) {
    let used = SystemTime::now() - Duration::from_secs(days * 24 * 60 * 60);
    let file = fs::File::options().append(true).open(path).unwrap();
    file.set_modified(used).unwrap();
}

#[test]
fn resumed_in_the_same_terminal_it_has_the_same_loops() {
    let env = Env::new();
    env.session(S, T, &["add", "Pick A or B", "Parked: evals"]);
    assert_eq!(
        env.session(S, T, &["list"]),
        "1. Pick A or B\n2. Parked: evals\n"
    );

    // The file is the ledger, the label its mirror, and the terminal
    // says whose it mirrors and what that session left.
    let file = env.session_file(S).unwrap();
    assert_eq!(file["session"], S);
    assert_eq!(file["terminal"], "t8k");
    assert_eq!(file["loops"][0]["t"], "Pick A or B");
    assert!(file["loops"][0]["at"].is_u64());
    assert_eq!(env.label_texts("t8k"), ["Pick A or B", "Parked: evals"]);
    assert_eq!(
        env.mirror("t8k").unwrap(),
        json!({ "session": S, "loops": file["loops"] })
    );
    // No workspace's ledger was made, and another session has none of it.
    assert!(env.ledgers().is_empty());
    assert_eq!(env.session("another", T3, &["list"]), "");
}

#[test]
fn resumed_in_another_terminal_its_loops_follow_and_leave_the_old_label() {
    let env = Env::new();
    env.session(S, T, &["add", "Pick A or B", "Parked: evals"]);
    assert_eq!(
        env.session(S, T3, &["list"]),
        "1. Pick A or B\n2. Parked: evals\n"
    );
    assert_eq!(env.label_texts("rig/t3"), ["Pick A or B", "Parked: evals"]);
    assert_eq!(env.session_file(S).unwrap()["terminal"], "rig/t3");
    // Nothing of the session is left on the old terminal.
    assert_eq!(env.label("t8k"), None);
    assert_eq!(env.mirror("t8k"), None);
    assert_eq!(env.session("next", T, &["list"]), "");
}

#[test]
fn resumed_in_another_terminal_it_joins_what_waits_there_without_duplicates() {
    let env = Env::new();
    env.session(S, T, &["add", "Pick A or B"]);
    env.set_label("rig/t3", r#"["pick a or b", "Handed over"]"#);
    assert_eq!(
        env.session(S, T3, &["list"]),
        "1. Pick A or B\n2. Handed over\n"
    );
    assert_eq!(env.label_texts("rig/t3"), ["Pick A or B", "Handed over"]);
}

#[test]
fn an_old_label_changed_since_the_session_left_is_left_alone() {
    let env = Env::new();
    env.session(S, T, &["add", "Pick A or B"]);
    // Someone added to it.
    env.ok(T, &["add", "Theirs"]);
    assert_eq!(env.session(S, T3, &["list"]), "1. Pick A or B\n");
    assert_eq!(env.label_texts("t8k"), ["Pick A or B", "Theirs"]);
    // The next session there takes what was added, and no more.
    assert_eq!(env.session("next", T, &["list"]), "1. Theirs\n");
}

#[test]
fn an_old_label_another_session_took_over_is_left_alone() {
    let env = Env::new();
    env.session(S, T, &["add", "Pick A or B"]);
    env.session("next", T, &["add", "Theirs"]);
    assert_eq!(env.session(S, T3, &["list"]), "1. Pick A or B\n");
    assert_eq!(env.label_texts("t8k"), ["Theirs"]);
    assert_eq!(env.mirror("t8k").unwrap()["session"], "next");
}

#[test]
fn a_terminal_that_is_gone_does_not_stop_a_move() {
    let env = Env::new();
    env.session(S, T, &["add", "Pick A or B"]);
    let mut state = env.state();
    state.as_object_mut().unwrap().remove("t8k");
    env.set_state(&state);
    assert_eq!(env.session(S, T3, &["list"]), "1. Pick A or B\n");
}

#[test]
fn resumed_outside_herdr_it_keeps_the_file_alone() {
    let env = Env::new();
    env.session(S, T, &["add", "Pick A or B", "Parked: evals"]);
    assert_eq!(
        env.session(S, None, &["list"]),
        "1. Pick A or B\n2. Parked: evals\n"
    );
    env.session(S, None, &["done", "pick"]);
    env.session(S, None, &["add", "Ship it"]);
    assert_eq!(
        env.session(S, None, &["list", "--json"]),
        "[\"Parked: evals\", \"Ship it\"]\n"
    );
    // Herdr was never asked: the label is as the session left it.
    assert_eq!(env.label_texts("t8k"), ["Pick A or B", "Parked: evals"]);

    // Back in the terminal, what was closed outside stays closed.
    assert_eq!(
        env.session(S, T, &["list"]),
        "1. Parked: evals\n2. Ship it\n"
    );
    assert_eq!(env.label_texts("t8k"), ["Parked: evals", "Ship it"]);
}

#[test]
fn everything_closed_outside_herdr_stays_closed_back_inside() {
    let env = Env::new();
    env.session(S, T, &["add", "Pick A or B"]);
    env.session(S, None, &["clear"]);
    assert_eq!(env.session(S, T, &["list"]), "");
    assert_eq!(env.label("t8k"), None);
    assert_eq!(env.session_file(S), None);
}

#[test]
fn started_outside_then_resumed_inside_it_is_mirrored_to_the_terminal() {
    let env = Env::new();
    env.session(S, None, &["add", "Pick A or B"]);
    let file = env.session_file(S).unwrap();
    assert_eq!(file["loops"][0]["t"], "Pick A or B");
    assert!(file.get("terminal").is_none() && file.get("mirrored").is_none());
    assert_eq!(env.label("t8k"), None);

    assert_eq!(env.session(S, T, &["list"]), "1. Pick A or B\n");
    assert_eq!(env.label_texts("t8k"), ["Pick A or B"]);
    assert_eq!(env.session_file(S).unwrap()["terminal"], "t8k");
}

#[test]
fn a_drawer_dismissal_is_picked_up_on_the_next_read() {
    let env = Env::new();
    env.session(S, T, &["add", "Pick A or B", "Parked: evals"]);
    env.dismiss_all_but("t8k", &["Parked: evals"]);
    assert_eq!(env.session(S, T, &["list"]), "1. Parked: evals\n");
    let file = env.session_file(S).unwrap();
    assert_eq!(file["loops"].as_array().unwrap().len(), 1);
    assert_eq!(file["loops"][0]["t"], "Parked: evals");
    // It stays dismissed wherever the session goes next.
    assert_eq!(env.session(S, None, &["list"]), "1. Parked: evals\n");

    // The last one dismissed leaves no label, and no file.
    env.dismiss_all_but("t8k", &[]);
    assert_eq!(env.session(S, T, &["add", "Ship it"]), "1. Ship it\n");
    env.dismiss_all_but("t8k", &[]);
    assert_eq!(env.session(S, T, &["list"]), "");
    assert_eq!(env.session_file(S), None);
}

#[test]
fn a_hand_off_from_another_session_is_picked_up_on_the_next_read() {
    let env = Env::new();
    env.session(S, T, &["add", "Pick A or B"]);
    // Another session, in another terminal, names this one's.
    env.session(
        "another",
        T3,
        &["--terminal", "t8k", "add", "Review the PR"],
    );
    assert_eq!(env.session_file("another"), None);
    assert_eq!(env.label("rig/t3"), None);

    assert_eq!(
        env.session(S, T, &["list"]),
        "1. Pick A or B\n2. Review the PR\n"
    );
    let file = env.session_file(S).unwrap();
    assert_eq!(file["loops"][1]["t"], "Review the PR");
    // And it follows the session out of Herdr.
    assert_eq!(
        env.session(S, None, &["list"]),
        "1. Pick A or B\n2. Review the PR\n"
    );
}

#[test]
fn a_new_session_adopts_a_label_no_session_left() {
    let env = Env::new();
    env.set_label("t8k", r#"["Old one", {"t":"Timed","at":1700}]"#);
    assert_eq!(env.session(S, T, &["list"]), "1. Old one\n2. Timed\n");
    let file = env.session_file(S).unwrap();
    assert_eq!(
        file["loops"],
        json!([{ "t": "Old one" }, { "t": "Timed", "at": 1700 }])
    );
    assert_eq!(env.mirror("t8k").unwrap()["session"], S);
}

#[test]
fn a_fresh_session_in_a_used_terminal_starts_empty() {
    let env = Env::new();
    env.session("earlier", T, &["add", "Pick A or B", "Parked: evals"]);
    let earlier = env.session_file("earlier");

    assert_eq!(env.session(S, T, &["list"]), "");
    assert_eq!(env.session_file(S), None);
    // The terminal shows the fresh session's ledger: nothing.
    assert_eq!(env.label("t8k"), None);
    assert_eq!(env.mirror("t8k"), None);
    // The earlier session's file is as it was.
    assert_eq!(env.session_file("earlier"), earlier);

    assert_eq!(env.session(S, T, &["add", "Mine"]), "1. Mine\n");
    assert_eq!(env.label_texts("t8k"), ["Mine"]);
}

#[test]
fn a_hand_off_before_a_session_s_first_call_survives() {
    let env = Env::new();
    // Handed over by another session, from another terminal, before
    // this one has asked for its ledger.
    env.session(
        "another",
        T3,
        &["--terminal", "t8k", "add", "Review the PR"],
    );
    assert_eq!(env.mirror("t8k"), None);
    assert_eq!(env.session(S, T, &["list"]), "1. Review the PR\n");
    assert_eq!(
        env.session_file(S).unwrap()["loops"][0]["t"],
        "Review the PR"
    );
    assert_eq!(env.mirror("t8k").unwrap()["session"], S);
}

#[test]
fn a_hand_off_onto_a_leftover_label_keeps_only_the_hand_off() {
    let env = Env::new();
    env.session("earlier", T, &["add", "Pick A or B", "Parked: evals"]);
    env.session(
        "another",
        T3,
        &["--terminal", "t8k", "add", "Review the PR"],
    );
    assert_eq!(
        env.label_texts("t8k"),
        ["Pick A or B", "Parked: evals", "Review the PR"]
    );

    assert_eq!(env.session(S, T, &["list"]), "1. Review the PR\n");
    assert_eq!(env.label_texts("t8k"), ["Review the PR"]);
    assert_eq!(
        env.session_file(S).unwrap()["loops"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // A person's, from a plain shell in the terminal, is one too.
    env.ok(T, &["add", "Call Hans"]);
    assert_eq!(
        env.session(S, T, &["list"]),
        "1. Review the PR\n2. Call Hans\n"
    );
}

#[test]
fn an_earlier_session_resumed_in_the_same_terminal_gets_its_loops_back() {
    let env = Env::new();
    env.session("earlier", T, &["add", "Pick A or B", "Parked: evals"]);
    env.session(S, T, &["add", "Mine"]);
    assert_eq!(env.label_texts("t8k"), ["Mine"]);

    assert_eq!(
        env.session("earlier", T, &["list"]),
        "1. Pick A or B\n2. Parked: evals\n"
    );
    assert_eq!(env.label_texts("t8k"), ["Pick A or B", "Parked: evals"]);
    assert_eq!(env.mirror("t8k").unwrap()["session"], "earlier");
    // And the one in between keeps its own, for when it is resumed.
    assert_eq!(env.session_file(S).unwrap()["loops"][0]["t"], "Mine");
    assert_eq!(env.session(S, T, &["list"]), "1. Mine\n");
}

#[test]
fn a_dismissal_while_away_holds_when_the_session_is_back() {
    let env = Env::new();
    env.session("earlier", T, &["add", "Pick A or B", "Parked: evals"]);
    // Dismissed in the drawer after the session ended, and handed one.
    env.dismiss_all_but("t8k", &["Parked: evals"]);
    env.ok(T, &["add", "Call Hans"]);
    assert_eq!(
        env.session("earlier", T, &["list"]),
        "1. Parked: evals\n2. Call Hans\n"
    );
}

#[test]
fn without_a_session_id_it_is_the_label_inside_herdr_and_the_workspace_s_file_outside() {
    let env = Env::new();
    env.ok(T, &["add", "On the label"]);
    assert_eq!(env.label_texts("t8k"), ["On the label"]);
    assert!(!env.state_dir().exists());

    env.ok(None, &["add", "In the workspace"]);
    assert_eq!(env.ok(None, &["list"]), "1. In the workspace\n");
    assert_eq!(env.ledgers().len(), 1);
    assert!(!env.state_dir().join("sessions").exists());
    assert_eq!(env.ok(T, &["list"]), "1. On the label\n");

    // A blank id is none.
    let out = env
        .command(None, &env.work())
        .env("CLAUDE_CODE_SESSION_ID", " ")
        .arg("list")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "1. In the workspace\n"
    );
}

#[test]
fn a_terminal_given_outright_is_its_label_whatever_the_session() {
    let env = Env::new();
    env.session(S, T, &["add", "Mine"]);
    assert_eq!(env.session(S, T, &["--terminal", "rig/t3", "list"]), "");
    env.session(S, T, &["--terminal=rig/t3", "add", "Theirs"]);
    assert_eq!(env.label_texts("rig/t3"), ["Theirs"]);
    assert_eq!(env.session(S, T, &["list"]), "1. Mine\n");
}

#[test]
fn clear_empties_the_file_and_the_label() {
    let env = Env::new();
    env.session(S, T, &["add", "One", "Two"]);
    env.session(S, T3, &["clear"]);
    assert_eq!(env.session_file(S), None);
    assert_eq!(env.label("rig/t3"), None);
    assert_eq!(env.label("t8k"), None);
    assert_eq!(env.session(S, None, &["list"]), "");
}

#[test]
fn antigravity_and_codex_name_their_sessions_too() {
    let env = Env::new();
    for (var, id) in [
        ("ANTIGRAVITY_CONVERSATION_ID", "agy-1"),
        ("CODEX_THREAD_ID", "codex-1"),
    ] {
        let run = |args: &[&str]| {
            let out = env
                .command(None, &env.work())
                .env(var, id)
                .args(args)
                .output()
                .unwrap();
            String::from_utf8(out.stdout).unwrap()
        };
        run(&["add", id]);
        assert_eq!(run(&["list"]), format!("1. {id}\n"));
        assert_eq!(env.session_file(id).unwrap()["loops"][0]["t"], id);
    }
}

#[test]
fn a_session_id_is_made_safe_for_a_file_name() {
    let env = Env::new();
    env.session("../up one", None, &["add", "Safe"]);
    assert_eq!(
        env.session_file(".._up_one").unwrap()["loops"][0]["t"],
        "Safe"
    );
    assert_eq!(env.session("../up one", None, &["list"]), "1. Safe\n");
}

#[test]
fn a_corrupt_file_reads_as_empty_and_starts_over() {
    let env = Env::new();
    env.session(S, None, &["add", "One"]);
    fs::write(env.session_path(S), "{not-valid-json").unwrap();
    assert_eq!(env.session(S, None, &["list"]), "");
    assert_eq!(env.session(S, None, &["add", "Two"]), "1. Two\n");
}

#[test]
fn files_of_sessions_unused_for_a_month_go_when_another_is_written() {
    let env = Env::new();
    env.session("ended", None, &["add", "Never closed"]);
    env.session("idle", None, &["add", "Still wanted"]);
    env.session("recent", None, &["add", "Last week's"]);
    age(&env.session_path("ended"), 31);
    age(&env.session_path("idle"), 31);
    age(&env.session_path("recent"), 29);

    // A read counts as use, and removes nothing.
    assert_eq!(env.session("idle", None, &["list"]), "1. Still wanted\n");
    assert!(env.session_file("ended").is_some());

    env.session(S, None, &["add", "New"]);
    assert_eq!(env.session_file("ended"), None);
    assert!(env.session_file("idle").is_some());
    assert!(env.session_file("recent").is_some());
    assert!(env.session_file(S).is_some());
}
