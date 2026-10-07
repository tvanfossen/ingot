//! Integration tests for `dm_key_event_list.hpp`, the neutral (key id, event type) typelist
//! emitted under `--emit-tinyfsm`.
//!
//! The golden test pins the exact bytes for a small fixture model. The agreement test pins the
//! invariant on a richer model: the list holds exactly the event structs `dm_key_events.hpp`
//! declares, in the same order, each paired with its own key define. The compile proof against
//! real tinyfsm lives in `tests/fixtures/cxx`, driven by `invoke test`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const LIST: &str = "dm_key_event_list.hpp";

/// Create a fresh, uniquely-named temp directory for one test.
fn unique_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ingot_evlist_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Run ingot on `model` into `out`, asserting success. CWD is the crate root under `cargo test`.
fn run_ingot(model: &str, out: &Path, extra: &[&str]) {
    let mut args = vec!["--model", model, "--output", out.to_str().unwrap()];
    args.extend_from_slice(extra);
    let status = Command::new(env!("CARGO_BIN_EXE_ingot"))
        .args(&args)
        .status()
        .expect("spawn ingot");
    assert!(status.success(), "ingot failed for args {args:?}");
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// `(key define, event struct)` pairs, in order, from the emitted list's `KE<...>` rows.
fn list_pairs(hpp: &str) -> Vec<(String, String)> {
    hpp.lines()
        .filter_map(|l| l.trim().strip_prefix("KE<"))
        .map(|rest| {
            let inner = rest.trim_end_matches(',').trim_end_matches('>');
            let (k, e) = inner.split_once(", ").expect("KE<key, event> row");
            (k.to_string(), e.to_string())
        })
        .collect()
}

/// Event struct names, in order, from `dm_key_events.hpp`.
fn event_structs(hpp: &str) -> Vec<String> {
    hpp.lines()
        .filter_map(|l| l.strip_prefix("struct "))
        .filter_map(|l| l.strip_suffix(" : tinyfsm::Event { };"))
        .map(str::to_string)
        .collect()
}

#[test]
fn event_list_matches_golden() {
    let out = unique_dir("golden");
    run_ingot(
        "tests/fixtures/event_list/model.toml",
        &out,
        &["--emit-tinyfsm"],
    );

    let golden = read(Path::new(
        "tests/fixtures/event_list/dm_key_event_list.hpp.golden",
    ))
    .replace("@VERSION@", env!("CARGO_PKG_VERSION"));
    let actual = read(&out.join(LIST));
    assert_eq!(actual, golden, "dm_key_event_list.hpp drifted from golden");

    let _ = fs::remove_dir_all(&out);
}

#[test]
fn event_list_agrees_with_emitted_event_structs() {
    let out = unique_dir("agree");
    run_ingot("examples/full.toml", &out, &["--emit-tinyfsm"]);

    let pairs = list_pairs(&read(&out.join(LIST)));
    let structs = event_structs(&read(&out.join("dm_key_events.hpp")));
    assert!(!structs.is_empty(), "full.toml must carry event keys");

    let listed: Vec<String> = pairs.iter().map(|(_, e)| e.clone()).collect();
    assert_eq!(
        listed, structs,
        "list must hold exactly the emitted event structs, in order"
    );
    for (key, ev) in &pairs {
        assert_eq!(
            key.replacen("DM_KEY_", "FSM_EVENT_", 1),
            *ev,
            "key {key} paired with the wrong event"
        );
    }
    let count = format!("dm_event_count = {}U;", pairs.len());
    assert!(
        read(&out.join(LIST)).contains(&count),
        "dm_event_count must equal the row count ({})",
        pairs.len()
    );

    let _ = fs::remove_dir_all(&out);
}

#[test]
fn event_list_not_emitted_without_flag() {
    let out = unique_dir("off");
    run_ingot("examples/full.toml", &out, &[]);
    assert!(
        !out.join(LIST).exists(),
        "{LIST} must not be emitted without --emit-tinyfsm"
    );
    let _ = fs::remove_dir_all(&out);
}

#[test]
fn event_list_not_emitted_for_model_without_event_keys() {
    let dir = unique_dir("noevent");
    let model = dir.join("noevent.toml");
    fs::write(
        &model,
        "[meta]\nid = \"plain\"\nversion = \"1.0.0\"\n\n\
         [[classes]]\nid = \"status\"\n\
         [[classes.keys]]\nid = \"voltage\"\ntype = \"uint16\"\ndefault = 0\n",
    )
    .unwrap();
    let out = dir.join("gen");
    run_ingot(model.to_str().unwrap(), &out, &["--emit-tinyfsm"]);
    assert!(
        out.join("key_definitions.h").exists(),
        "generation itself must have run"
    );
    assert!(
        !out.join(LIST).exists(),
        "a model with zero event keys must emit no {LIST}: L<> would name no event"
    );
    let _ = fs::remove_dir_all(&dir);
}
