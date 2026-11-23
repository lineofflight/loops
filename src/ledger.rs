//! The ledger: its entries, as Herdr's `herdr.loops` label holds them, and
//! what adding and closing do to them.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

/// One open loop: its text, on one line, and when it was added, in Unix
/// seconds, where known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub t: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<u64>,
}

/// `text` on one line, every run of whitespace one space.
pub fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Text compared whatever its case.
fn fold(text: &str) -> String {
    text.to_lowercase()
}

/// The entries in `raw`: a JSON array of plain strings or `{"t", "at"}`
/// objects. Anything else reads as none, so a corrupt ledger starts over.
pub fn parse(raw: &str) -> Vec<Entry> {
    serde_json::from_str(raw)
        .map(|value| from_value(&value))
        .unwrap_or_default()
}

/// The entries in `value`, as `parse` reads them.
pub fn from_value(value: &Value) -> Vec<Entry> {
    let Value::Array(items) = value else {
        return Vec::new();
    };
    items.iter().filter_map(entry).collect()
}

fn entry(item: &Value) -> Option<Entry> {
    let (text, at) = match item {
        Value::String(text) => (text.as_str(), None),
        Value::Object(map) => {
            let at = map
                .get("at")
                .and_then(Value::as_f64)
                .filter(|at| *at > 0.0)
                .map(|at| at as u64);
            (map.get("t")?.as_str()?, at)
        }
        _ => return None,
    };
    let t = normalize(text);
    (!t.is_empty()).then_some(Entry { t, at })
}

/// The entries as the label holds them: compact JSON.
pub fn serialize(entries: &[Entry]) -> String {
    serde_json::to_string(entries).expect("entries serialize")
}

/// The texts as a JSON array, spaced as the prototype printed them.
pub fn json(entries: &[Entry]) -> String {
    let texts: Vec<_> = entries
        .iter()
        .map(|e| serde_json::to_string(&e.t).expect("a string serializes"))
        .collect();
    format!("[{}]", texts.join(", "))
}

/// The texts numbered from 1, a line each; nothing for none.
pub fn numbered(entries: &[Entry]) -> String {
    entries
        .iter()
        .enumerate()
        .map(|(i, e)| format!("{}. {}\n", i + 1, e.t))
        .collect()
}

/// Adds each of `items`, normalized and stamped `now`, unless it is blank
/// or already there whatever its case. Whether any was added.
pub fn add(entries: &mut Vec<Entry>, items: &[String], now: u64) -> bool {
    let mut seen: HashSet<_> = entries.iter().map(|e| fold(&e.t)).collect();
    let before = entries.len();
    for item in items {
        let t = normalize(item);
        if !t.is_empty() && seen.insert(fold(&t)) {
            entries.push(Entry { t, at: Some(now) });
        }
    }
    entries.len() > before
}

/// The entries left once `tokens` are closed. A token of digits is a
/// 1-based number, as `numbered` shows them, ignored when out of range;
/// any other closes every entry it is found in, whatever its case. Every
/// number counts from the entries as they were.
pub fn done(entries: &[Entry], tokens: &[String]) -> Vec<Entry> {
    let mut closed = vec![false; entries.len()];
    for token in tokens {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if token.bytes().all(|b| b.is_ascii_digit()) {
            let index = token.parse::<usize>().ok().and_then(|n| n.checked_sub(1));
            if let Some(slot) = index.and_then(|i| closed.get_mut(i)) {
                *slot = true;
            }
            continue;
        }
        let needle = fold(token);
        for (slot, entry) in closed.iter_mut().zip(entries) {
            if fold(&entry.t).contains(&needle) {
                *slot = true;
            }
        }
    }
    entries
        .iter()
        .zip(closed)
        .filter(|(_, closed)| !closed)
        .map(|(entry, _)| entry.clone())
        .collect()
}

/// `ours` with what happened to a copy of it applied: `base` is the copy
/// as it was made, `theirs` the copy as it is now. What left the copy
/// leaves `ours`, and what joined it joins `ours`, unless already there.
/// Texts are compared whatever their case.
pub fn merge(ours: &[Entry], base: &[Entry], theirs: &[Entry]) -> Vec<Entry> {
    let texts = |entries: &[Entry]| entries.iter().map(|e| fold(&e.t)).collect::<HashSet<_>>();
    let (base, left) = (texts(base), texts(theirs));
    let mut merged: Vec<Entry> = ours
        .iter()
        .filter(|e| !base.contains(&fold(&e.t)) || left.contains(&fold(&e.t)))
        .cloned()
        .collect();
    let mut seen = texts(&merged);
    for entry in theirs {
        if !base.contains(&fold(&entry.t)) && seen.insert(fold(&entry.t)) {
            merged.push(entry.clone());
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|e| e.t.as_str()).collect()
    }

    #[test]
    fn merge_applies_what_left_and_joined_the_copy() {
        let ours = parse(r#"["Alpha", "Beta", "Mine"]"#);
        let base = parse(r#"["Alpha", "Beta"]"#);
        // Beta dismissed, Handed added.
        let theirs = parse(r#"["alpha", "Handed", "MINE"]"#);
        assert_eq!(
            texts(&merge(&ours, &base, &theirs)),
            ["Alpha", "Mine", "Handed"]
        );
        // An untouched copy changes nothing; no copy before adopts it all.
        assert_eq!(merge(&ours, &base, &base), ours);
        assert_eq!(merge(&[], &[], &theirs), theirs);
        // A copy from elsewhere is joined, without duplicates.
        assert_eq!(
            texts(&merge(&ours, &[], &parse(r#"["beta", "New"]"#))),
            ["Alpha", "Beta", "Mine", "New"]
        );
    }

    #[test]
    fn parse_reads_strings_and_objects_and_skips_the_rest() {
        let entries =
            parse(r#"[" First ", "", {"t":"Second","at":1700}, {"t":"Third","at":-1}, 42, null]"#);
        assert_eq!(
            entries,
            vec![
                Entry {
                    t: "First".into(),
                    at: None
                },
                Entry {
                    t: "Second".into(),
                    at: Some(1700)
                },
                Entry {
                    t: "Third".into(),
                    at: None
                },
            ]
        );
        assert!(parse("").is_empty());
        assert!(parse("{not-valid-json").is_empty());
        assert!(parse(r#"{"not":"an-array"}"#).is_empty());
    }

    #[test]
    fn serialize_is_compact_and_omits_unknown_times() {
        let entries = parse(r#"["Old", {"t":"Ünïcode \"q\"","at":5}]"#);
        assert_eq!(
            serialize(&entries),
            r#"[{"t":"Old"},{"t":"Ünïcode \"q\"","at":5}]"#
        );
        assert_eq!(json(&entries), r#"["Old", "Ünïcode \"q\""]"#);
        assert_eq!(json(&[]), "[]");
    }

    #[test]
    fn add_normalizes_and_skips_blanks_and_duplicates_whatever_their_case() {
        let mut entries = Vec::new();
        let items = [
            "Ask  Hans".into(),
            "  ".into(),
            "ask hans".into(),
            "Line\n two".into(),
        ];
        assert!(add(&mut entries, &items, 9));
        assert_eq!(texts(&entries), ["Ask Hans", "Line two"]);
        assert_eq!(entries[0].at, Some(9));
        assert!(!add(&mut entries, &["ASK HANS".into()], 10));
    }

    #[test]
    fn done_closes_by_number_from_the_original_list_and_by_text() {
        let entries = parse(r#"["Alpha", "Beta", "Gamma", "alphabet"]"#);
        assert_eq!(
            texts(&done(&entries, &["1".into(), "2".into()])),
            ["Gamma", "alphabet"]
        );
        assert_eq!(texts(&done(&entries, &["ALPHA".into()])), ["Beta", "Gamma"]);
        assert_eq!(
            texts(&done(&entries, &["0".into(), "9".into(), " ".into()])),
            texts(&entries)
        );
    }

    #[test]
    fn numbered_counts_from_one() {
        assert_eq!(numbered(&parse(r#"["a","b"]"#)), "1. a\n2. b\n");
        assert_eq!(numbered(&[]), "");
    }
}
