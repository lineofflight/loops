//! Where a ledger lives. An agent session's is a small file of its own,
//! under `$XDG_STATE_HOME/loops/sessions`, else
//! `~/.local/state/loops/sessions`, found by the session id the agent
//! gives its commands, so it follows the session wherever it is resumed.
//! Inside Herdr the file is mirrored to the terminal's `herdr.loops`
//! label, through `hc`: Herdr's Loops drawer shows the label and
//! dismisses loops from it, and another session hands a loop over by
//! adding to it, so every read applies what changed on the label since
//! the session last mirrored to it. A second label, `herdr.loops.mirror`,
//! says which session that was and what it left, so the next session in
//! the terminal takes what was handed over and none of the last one's.
//!
//! With no session id, a person in a shell or an agent that gives none,
//! the ledger is the terminal's label itself inside Herdr, and a local
//! file for the workspace outside it, beside the `sessions` folder.

use crate::ledger::{self, Entry};
use serde_json::{Value, json};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime};

/// The label a Herdr terminal keeps its ledger on.
pub const LABEL: &str = "herdr.loops";

/// The label beside it that says whose ledger that is: the session that
/// last mirrored its own there, and the loops as it left them, as
/// `{"session":"<id>","loops":[...]}`. Only a session writes it, so
/// where the label differs from it, someone else added or dismissed.
pub const MIRROR: &str = "herdr.loops.mirror";

/// The variables agents give their commands a session id in: Claude
/// Code's, Antigravity's and Codex's.
pub const SESSION_VARS: [&str; 3] = [
    "CLAUDE_CODE_SESSION_ID",
    "ANTIGRAVITY_CONVERSATION_ID",
    "CODEX_THREAD_ID",
];

/// How long a session's file outlives the session's last use of it.
const KEPT: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Why a ledger couldn't be read or written, and the exit code that
/// means.
#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub code: i32,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Error {
            message: message.into(),
            code: 1,
        }
    }
}

/// A ledger's home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Store {
    /// An agent session's file, mirrored to the label of the Herdr
    /// terminal the session runs in, when it runs in one.
    Session {
        id: String,
        file: PathBuf,
        terminal: Option<String>,
    },
    /// A Herdr terminal's `herdr.loops` label, by the terminal's address.
    Herdr { terminal: String },
    /// A workspace's file: `root` is the workspace, `file` its ledger.
    Workspace { root: PathBuf, file: PathBuf },
}

/// `value`, trimmed, unless blank.
fn stated(value: String) -> Option<String> {
    Some(value.trim().to_string()).filter(|value| !value.is_empty())
}

/// The agent session a command runs in: the first of `SESSION_VARS` set
/// and not blank.
pub fn session_from_env() -> Option<String> {
    SESSION_VARS
        .iter()
        .find_map(|var| std::env::var(var).ok().and_then(stated))
}

/// The terminal `HC_TERMINAL_ID` names, when set and not blank.
pub fn terminal_from_env() -> Option<String> {
    std::env::var("HC_TERMINAL_ID").ok().and_then(stated)
}

/// The workspace `dir` is in: the nearest directory up from it holding
/// `.git`, a repository's or a worktree's, else `dir` itself.
pub fn workspace_root(dir: &Path) -> PathBuf {
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    dir.ancestors()
        .find(|d| d.join(".git").exists())
        .map(Path::to_path_buf)
        .unwrap_or(dir)
}

/// Where local ledgers are kept: `$XDG_STATE_HOME/loops`, else
/// `~/.local/state/loops`. A relative `XDG_STATE_HOME` is ignored, as the
/// XDG specification says.
pub fn state_dir() -> Option<PathBuf> {
    let xdg = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute());
    let base =
        xdg.or_else(|| std::env::home_dir().map(|home| home.join(".local").join("state")))?;
    Some(base.join("loops"))
}

/// FNV-1a, a hash that stays the same from one build to the next.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// `name` as part of a file's name.
fn safe(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_.".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The file a workspace's ledger is kept in: its folder's name, for a
/// person looking, and a hash of its full path.
fn workspace_file(dir: &Path, root: &Path) -> PathBuf {
    let name = root
        .file_name()
        .map(|n| safe(&n.to_string_lossy()))
        .unwrap_or_else(|| "root".into());
    let hash = fnv1a(root.as_os_str().as_encoded_bytes());
    dir.join(format!("{name}-{hash:016x}.json"))
}

fn hc_bin() -> String {
    std::env::var("HC_BIN")
        .ok()
        .filter(|bin| !bin.is_empty())
        .unwrap_or_else(|| "hc".into())
}

fn hc(args: &[&str]) -> Result<Output, Error> {
    let bin = hc_bin();
    Command::new(&bin)
        .args(args)
        .output()
        .map_err(|e| Error::new(format!("loops: failed to run {bin}: {e}")))
}

/// `hc`'s own error where it gave one, else `fallback`, with its exit
/// code.
fn hc_failed(out: &Output, fallback: String) -> Error {
    let stderr = String::from_utf8_lossy(&out.stderr).trim_end().to_string();
    Error {
        message: if stderr.is_empty() { fallback } else { stderr },
        code: out.status.code().filter(|code| *code != 0).unwrap_or(1),
    }
}

/// A terminal's label, and the mirror beside it: the session that last
/// mirrored its ledger there, if any, and the loops as it left them.
#[derive(Debug, Default)]
struct Label {
    loops: Vec<Entry>,
    session: Option<String>,
    mirrored: Vec<Entry>,
}

/// `terminal`'s label and its mirror.
fn read_label(terminal: &str) -> Result<Label, Error> {
    let out = hc(&["get", "--json", terminal])?;
    if !out.status.success() {
        return Err(hc_failed(
            &out,
            format!("loops: failed to read terminal {terminal}"),
        ));
    }
    let data: Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| Error::new(format!("loops: invalid JSON from hc get: {e}")))?;
    let label = |name| data["labels"][name].as_str().unwrap_or_default();
    let mirror: Value = serde_json::from_str(label(MIRROR)).unwrap_or_default();
    Ok(Label {
        loops: ledger::parse(label(LABEL)),
        session: mirror["session"].as_str().map(String::from),
        mirrored: ledger::from_value(&mirror["loops"]),
    })
}

/// Replaces the entries on `terminal`'s label. None removes the label.
/// For `session`, the mirror beside it is replaced too; else it is left
/// as it is, so the entries read as changed by someone else.
fn write_label(terminal: &str, entries: &[Entry], session: Option<&str>) -> Result<(), Error> {
    let (mut label, mut mirror) = (format!("{LABEL}="), format!("{MIRROR}="));
    if !entries.is_empty() {
        label += &ledger::serialize(entries);
        mirror += &json!({ "session": session, "loops": entries }).to_string();
    }
    let mut args = vec!["label", terminal, &label];
    if session.is_some() {
        args.push(&mirror);
    }
    let out = hc(&args)?;
    if !out.status.success() {
        return Err(hc_failed(
            &out,
            format!("loops: failed to update terminal {terminal}"),
        ));
    }
    Ok(())
}

/// The JSON in `file`. A missing or corrupt file reads as null, so its
/// ledger starts over.
fn read_json(file: &Path) -> Result<Value, Error> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(serde_json::from_str(&text).unwrap_or_default()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Value::Null),
        Err(e) => Err(Error::new(format!(
            "loops: failed to read {}: {e}",
            file.display()
        ))),
    }
}

fn write_failed(file: &Path, e: io::Error) -> Error {
    Error::new(format!("loops: failed to write {}: {e}", file.display()))
}

/// Writes `data` to `file`: beside it, then moved over it, so a reader
/// never sees half a file.
fn write_json(file: &Path, data: &Value) -> Result<(), Error> {
    let failed = |e| write_failed(file, e);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(failed)?;
    }
    let partial = file.with_extension(format!("{}.partial", std::process::id()));
    fs::write(&partial, format!("{data:#}\n")).map_err(failed)?;
    fs::rename(&partial, file).map_err(failed)
}

/// Removes `file`, if it is there.
fn remove(file: &Path) -> Result<(), Error> {
    match fs::remove_file(file) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(write_failed(file, e)),
        _ => Ok(()),
    }
}

/// What a session's file holds: its loops, and the terminal they were
/// last mirrored to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Saved {
    loops: Vec<Entry>,
    terminal: Option<String>,
}

impl Saved {
    fn read(file: &Path) -> Result<Saved, Error> {
        let data = read_json(file)?;
        Ok(Saved {
            loops: ledger::from_value(&data["loops"]),
            terminal: data["terminal"].as_str().map(String::from),
        })
    }

    /// Writes the file, and removes the files of sessions long unused.
    /// With no loops, no file is left behind.
    fn write(&self, file: &Path, id: &str) -> Result<(), Error> {
        if self.loops.is_empty() {
            return remove(file);
        }
        let mut data = json!({ "session": id, "loops": self.loops });
        if let Some(terminal) = &self.terminal {
            data["terminal"] = json!(terminal);
        }
        write_json(file, &data)?;
        if let Some(dir) = file.parent() {
            sweep(dir);
        }
        Ok(())
    }

    /// The session `id` now runs in `terminal`: clears the label it left
    /// on another, if that is still its own and as it left it. A terminal
    /// that is gone, or whose label changed or went to another session
    /// since, is left alone.
    fn leave_for(&self, terminal: &str, id: &str) {
        let Some(old) = self.terminal.as_deref().filter(|old| *old != terminal) else {
            return;
        };
        let untouched =
            |label: Label| label.session.as_deref() == Some(id) && label.loops == label.mirrored;
        if read_label(old).is_ok_and(untouched) {
            let _ = write_label(old, &[], Some(id));
        }
    }
}

/// Marks `file` as used now, so `sweep` keeps it.
fn touch(file: &Path) {
    if let Ok(file) = fs::File::options().append(true).open(file) {
        let _ = file.set_modified(SystemTime::now());
    }
}

/// Removes the session files in `dir` unused for longer than `KEPT`:
/// those of sessions that ended with loops open and never came back.
fn sweep(dir: &Path) {
    let Ok(files) = fs::read_dir(dir) else {
        return;
    };
    for file in files.flatten() {
        let used = file.metadata().and_then(|meta| meta.modified());
        if used.is_ok_and(|at| at.elapsed().is_ok_and(|age| age > KEPT)) {
            let _ = fs::remove_file(file.path());
        }
    }
}

impl Store {
    /// The ledger of `session` in `terminal`, either of which there may
    /// be none of: the session's own file, else the terminal's label,
    /// else the file of the workspace the current directory is in.
    pub fn open(session: Option<String>, terminal: Option<String>) -> Result<Store, Error> {
        match (session.and_then(stated), terminal.and_then(stated)) {
            (Some(id), terminal) => {
                let state = state_dir().ok_or_else(|| {
                    Error::new("loops: no home directory to keep this session's open loops in")
                })?;
                let file = state.join("sessions").join(format!("{}.json", safe(&id)));
                Ok(Store::Session { id, file, terminal })
            }
            (None, Some(terminal)) => Ok(Store::Herdr { terminal }),
            (None, None) => {
                let dir = std::env::current_dir()
                    .map_err(|e| Error::new(format!("loops: no working directory: {e}")))?;
                Store::workspace(&dir)
            }
        }
    }

    /// The ledger of the workspace `dir` is in.
    pub fn workspace(dir: &Path) -> Result<Store, Error> {
        let state = state_dir().ok_or_else(|| {
            Error::new("loops: no home directory to keep this workspace's open loops in")
        })?;
        let root = workspace_root(dir);
        let file = workspace_file(&state, &root);
        Ok(Store::Workspace { root, file })
    }

    /// The entries as they are now.
    pub fn load(&self) -> Result<Vec<Entry>, Error> {
        match self {
            Store::Session { id, file, terminal } => {
                let saved = Saved::read(file)?;
                let Some(terminal) = terminal else {
                    touch(file);
                    return Ok(saved.loops);
                };
                // What changed on the label since this session mirrored
                // to it applies. Under another session's mirror, what
                // that one left is its own and stays out of this ledger:
                // only what was added since, a hand-off, joins. With no
                // mirror, that is all of it.
                let label = read_label(terminal)?;
                let loops = if label.session.as_deref() == Some(id) {
                    ledger::merge(&saved.loops, &label.mirrored, &label.loops)
                } else {
                    let handed = ledger::merge(&[], &label.mirrored, &label.loops);
                    ledger::merge(&saved.loops, &[], &handed)
                };
                let session = (!loops.is_empty()).then_some(id.as_str());
                if label.loops != loops
                    || label.mirrored != loops
                    || label.session.as_deref() != session
                {
                    write_label(terminal, &loops, Some(id))?;
                }
                saved.leave_for(terminal, id);
                let now = Saved {
                    loops: loops.clone(),
                    terminal: Some(terminal.clone()),
                };
                if now == saved {
                    touch(file);
                } else {
                    now.write(file, id)?;
                }
                Ok(loops)
            }
            Store::Herdr { terminal } => Ok(read_label(terminal)?.loops),
            Store::Workspace { file, .. } => Ok(ledger::from_value(&read_json(file)?["loops"])),
        }
    }

    /// Replaces the entries with `entries`. None leaves no label, and no
    /// file where there is nothing else to remember.
    pub fn save(&self, entries: &[Entry]) -> Result<(), Error> {
        match self {
            Store::Session { id, file, terminal } => {
                let saved = Saved::read(file)?;
                if let Some(terminal) = terminal {
                    write_label(terminal, entries, Some(id))?;
                    saved.leave_for(terminal, id);
                }
                let now = Saved {
                    loops: entries.to_vec(),
                    terminal: terminal.clone().or(saved.terminal),
                };
                now.write(file, id)
            }
            Store::Herdr { terminal } => write_label(terminal, entries, None),
            Store::Workspace { root, file } => {
                if entries.is_empty() {
                    return remove(file);
                }
                let data = json!({ "workspace": root.to_string_lossy(), "loops": entries });
                write_json(file, &data)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workspace_is_the_nearest_folder_with_git() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        let deep = repo.join("src").join("deep");
        fs::create_dir_all(&deep).unwrap();
        fs::write(repo.join(".git"), "gitdir: elsewhere\n").unwrap();
        let repo = repo.canonicalize().unwrap();
        assert_eq!(workspace_root(&deep), repo);
        assert_eq!(workspace_root(&repo), repo);

        let loose = tmp.path().join("loose");
        fs::create_dir_all(&loose).unwrap();
        assert_eq!(workspace_root(&loose), loose.canonicalize().unwrap());
    }

    #[test]
    fn a_workspace_file_is_named_for_people_and_hashed_for_uniqueness() {
        let file = workspace_file(Path::new("/s"), Path::new("/code/my repo"));
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with("my_repo-") && name.ends_with(".json"),
            "{name}"
        );
        assert_ne!(
            file,
            workspace_file(Path::new("/s"), Path::new("/other/my repo"))
        );
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
