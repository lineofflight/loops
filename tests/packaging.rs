//! The installers and the Homebrew formula, through `tests/packaging.sh`,
//! offline. The script moves HOME to a temporary directory itself, once
//! it has found Ruby, and clears the install settings; this clears Herdr
//! and the agent sessions.

use std::process::Command;

#[test]
fn the_packaging_scripts_pass() {
    let tmp = tempfile::tempdir().unwrap();
    let mut command = Command::new("sh");
    command
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/packaging.sh"))
        .env("TMPDIR", tmp.path())
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
    assert_eq!(String::from_utf8_lossy(&out.stdout), "PASS: packaging\n");
}
