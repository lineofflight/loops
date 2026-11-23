//! The prototype's own test script, run against this binary: a copy of
//! it beside a `loops` that links to the binary, where the script looks
//! for the prototype. It is unchanged but for its first case: outside
//! Herdr the prototype exited 2, where this keeps the workspace's ledger.
//! Run with its own home and as no agent session, so no ledger of the
//! user's is read or written.

use std::process::Command;

#[test]
fn the_prototype_s_test_script_passes() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("test_loops.sh");
    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/prototype/test_loops.sh"),
        &script,
    )
    .unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_loops"), dir.path().join("loops")).unwrap();
    let mut command = Command::new("bash");
    command
        .arg(&script)
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .env_remove("XDG_STATE_HOME")
        .env_remove("HC_TERMINAL_ID");
    for var in loops::store::SESSION_VARS {
        command.env_remove(var);
    }
    let out = command.output().unwrap();
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "PASS: all loops CLI tests passed\n"
    );
}
