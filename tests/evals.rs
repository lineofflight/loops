//! The eval cases, checked without running them: a mistake here costs a
//! whole eval run to find. A count grader with a maximum states its
//! minimum, which otherwise defaults to 1, so a "never called" grader
//! fails when nothing was called.

use std::fs;
use std::path::{Path, PathBuf};

const TOOLS: [&str; 4] = ["list", "add", "done", "clear"];

fn cases() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("evals/cases");
    let mut cases: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    cases.sort();
    cases
}

/// The `key: value` lines of a file's YAML front matter, or of all of a
/// plain YAML file.
fn fields(text: &str) -> Vec<(String, String)> {
    let body = match text.strip_prefix("---\n") {
        Some(rest) => rest.split("\n---").next().unwrap_or_default(),
        None => text,
    };
    body.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter_map(|line| line.split_once(": "))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn field<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

#[test]
fn every_case_is_complete_and_named_for_its_folder() {
    let cases = cases();
    assert_eq!(cases.len(), 6);
    for case in cases {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let yaml = fields(&fs::read_to_string(case.join("case.yaml")).unwrap());
        assert_eq!(field(&yaml, "name"), Some(name.as_str()));
        for key in ["history_file", "scaffold_script"] {
            if let Some(path) = field(&yaml, key) {
                assert!(case.join(path).is_file(), "{name}: no {path}");
            }
        }
        let prompt = fs::read_to_string(case.join("prompt.md")).unwrap();
        assert!(
            prompt
                .split("\n---\n")
                .nth(1)
                .is_some_and(|p| !p.trim().is_empty())
        );
        assert!(
            fs::read_dir(case.join("graders")).unwrap().count() > 0,
            "{name}"
        );
    }
}

#[test]
fn count_graders_name_a_loops_tool_and_state_their_minimum() {
    for case in cases() {
        for grader in fs::read_dir(case.join("graders")).unwrap() {
            let path = grader.unwrap().path();
            let fields = fields(&fs::read_to_string(&path).unwrap());
            let mut tools: Vec<_> = ["tool", "before", "after"]
                .iter()
                .filter_map(|key| field(&fields, key))
                .collect();
            if field(&fields, "type") == Some("tool_used") {
                assert!(
                    field(&fields, "max").is_none() || field(&fields, "min").is_some(),
                    "{}: a maximum without a minimum",
                    path.display()
                );
            } else if field(&fields, "type") != Some("tool_order") {
                tools.clear();
            }
            for tool in tools {
                let name = tool.strip_prefix("mcp__plugin_loops_loops__");
                assert!(
                    name.is_some_and(|n| TOOLS.contains(&n)),
                    "{}: unknown tool {tool}",
                    path.display()
                );
            }
        }
    }
}
