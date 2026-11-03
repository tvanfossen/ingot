//! Helpers shared by the contract-mode integration tests: the generic model, running ingot,
//! editing a model copy and reading contract.json.
#![allow(dead_code)] // each test crate uses a subset

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// @brief Fresh, uniquely-named temp directory for one test.
/// @version 1
pub fn unique_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ingot_contract_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// @brief Path of a version of the generic model, e.g. `v1_0`.
/// @version 1
pub fn model(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/contract")
        .join(version)
}

/// @brief Copy directory `from` into `to`, recursively.
/// @version 1
pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// @brief Run contract mode on `contract` into `out` (variant sku_a unless `extra` names one);
/// returns (success, stderr).
/// @version 1
pub fn generate(contract: &Path, out: &Path, extra: &[&str]) -> (bool, String) {
    let mut args = vec![
        "--contract",
        contract.to_str().unwrap(),
        "--output",
        out.to_str().unwrap(),
        "--allow-dirty-ingot",
    ];
    if !extra.contains(&"--variant") {
        args.extend(["--variant", "sku_a"]);
    }
    args.extend_from_slice(extra);
    let result = Command::new(env!("CARGO_BIN_EXE_ingot"))
        .args(&args)
        .output()
        .expect("spawn ingot");
    (
        result.status.success(),
        String::from_utf8_lossy(&result.stderr).into_owned(),
    )
}

/// @brief Generate `version` of the generic model into `out`, asserting success.
/// @version 1
pub fn generate_ok(version: &str, out: &Path, extra: &[&str]) {
    let (ok, stderr) = generate(&model(version).join("contract.toml"), out, extra);
    assert!(ok, "{version} must generate:\n{stderr}");
}

/// @brief The parsed contract.json in `gen`.
/// @version 1
pub fn contract_json(gen: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(gen.join("contract.json")).unwrap()).unwrap()
}

/// @brief The contract.json entry of principal `name`.
/// @version 1
pub fn principal(gen: &Path, name: &str) -> serde_json::Value {
    contract_json(gen)["principals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("principal {name}"))
        .clone()
}

/// @brief The digests of principal `name` in a generated contract.json.
/// @version 1
pub fn digests(gen: &Path, name: &str) -> Vec<String> {
    principal(gen, name)["digests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d.as_str().unwrap().to_string())
        .collect()
}

/// @brief Replace `from` with `to` in `file`, asserting `from` occurs exactly once.
/// @version 1
pub fn edit(file: &Path, from: &str, to: &str) {
    let text = fs::read_to_string(file).unwrap();
    assert_eq!(
        text.matches(from).count(),
        1,
        "{}: '{from}' must occur once",
        file.display()
    );
    fs::write(file, text.replacen(from, to, 1)).unwrap();
}

/// @brief Assert `contract` is refused against the contract.json in `previous_out`, with a log
/// containing `expected`.
/// @version 1
pub fn refused_against(contract: &Path, previous_out: &Path, expected: &str) {
    let previous = previous_out.join("contract.json");
    let out = previous_out.with_extension("out");
    let args = ["--previous-contract", previous.to_str().unwrap()];
    let (ok, stderr) = generate(contract, &out, &args);
    assert!(!ok, "'{expected}' must be refused");
    assert!(
        stderr.contains(expected),
        "expected '{expected}' in:\n{stderr}"
    );
}

/// @brief The rights cell of principal `name` on key `path` in a contract.json ("" for none).
/// @version 1
pub fn right(gen: &Path, name: &str, path: &str) -> String {
    contract_json(gen)["rights"][name][path]
        .as_str()
        .unwrap_or("")
        .to_string()
}

/// @brief The contract.json entry of the key at `path`.
/// @version 1
pub fn key_json(gen: &Path, path: &str) -> serde_json::Value {
    contract_json(gen)["keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["path"] == path)
        .unwrap_or_else(|| panic!("key {path}"))
        .clone()
}

/// @brief Copy `version` of the generic model into `dir` and apply `edits` (file, from, to).
/// @version 1
pub fn edited_copy(version: &str, dir: &Path, edits: &[(&str, &str, &str)]) {
    copy_dir(&model(version), dir);
    for (file, from, to) in edits {
        edit(&dir.join(file), from, to);
    }
}
