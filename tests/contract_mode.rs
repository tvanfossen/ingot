//! Integration tests for contract mode (`--contract`): the generic test model at 1.0, 1.1 and
//! 2.0, the generated tables compiled and walked in C, determinism, variants, and one model edit
//! per refusal.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// @brief Fresh, uniquely-named temp directory for one test.
/// @version 1
fn unique_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ingot_contract_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// @brief Path of a version of the generic model, e.g. `v1_0`.
/// @version 1
fn model(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/contract")
        .join(version)
}

/// @brief Copy directory `from` into `to`, recursively.
/// @version 1
fn copy_dir(from: &Path, to: &Path) {
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
fn generate(contract: &Path, out: &Path, extra: &[&str]) -> (bool, String) {
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
fn generate_ok(version: &str, out: &Path, extra: &[&str]) {
    let (ok, stderr) = generate(&model(version).join("contract.toml"), out, extra);
    assert!(ok, "{version} must generate:\n{stderr}");
}

/// @brief Compile the generated tables with the C walker and run it.
/// @version 1
fn compile_and_walk(gen: &Path) {
    let check = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dmt/check_tables.c");
    let exe = gen.join("check_tables");
    let include = format!("-I{}", gen.display());
    let tables = gen.join("dmt_tables.c");
    let out = Command::new("cc")
        .args([
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-pedantic",
            "-Werror",
            &include,
        ])
        .args([check.as_path(), tables.as_path()])
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("spawn cc");
    assert!(
        out.status.success(),
        "cc failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        Command::new(&exe).status().unwrap().success(),
        "table walker failed"
    );
}

/// @brief The parsed contract.json in `gen`.
/// @version 1
fn contract_json(gen: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(gen.join("contract.json")).unwrap()).unwrap()
}

/// @brief The contract.json entry of principal `name`.
/// @version 1
fn principal(gen: &Path, name: &str) -> serde_json::Value {
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
fn digests(gen: &Path, name: &str) -> Vec<String> {
    principal(gen, name)["digests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn generic_model_versions_and_previous_checks() {
    let dir = unique_dir("versions");
    let (g10, g11, g20) = (dir.join("1.0"), dir.join("1.1"), dir.join("2.0"));
    generate_ok("v1_0", &g10, &[]);
    assert!(!g10.join("release-review.txt").exists());
    let prev10 = g10.join("contract.json");
    generate_ok(
        "v1_1",
        &g11,
        &["--previous-contract", prev10.to_str().unwrap()],
    );
    let review = fs::read_to_string(g11.join("release-review.txt")).unwrap();
    assert_eq!(
        review.trim(),
        "new principal iotd.ota: r on existing net.credentials.cloud_token"
    );
    for name in ["integrator", "iotd", "mcu", "inproc"] {
        assert_eq!(
            digests(&g10, name)[0],
            digests(&g11, name)[0],
            "{name}: 1.0 digest must reappear"
        );
        assert_ne!(
            digests(&g11, name)[0],
            digests(&g11, name)[1],
            "{name}: 1.1 adds keys"
        );
    }
    let prev11 = g11.join("contract.json");
    generate_ok(
        "v2_0",
        &g20,
        &["--previous-contract", prev11.to_str().unwrap()],
    );
    for gen in [&g10, &g11, &g20] {
        compile_and_walk(gen);
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_major_change_labelled_as_a_minor_is_refused() {
    let dir = unique_dir("minor12");
    let (m12, g11) = (dir.join("m12"), dir.join("g11"));
    generate_ok("v1_1", &g11, &[]);
    copy_dir(&model("v2_0"), &m12);
    edit(
        &m12.join("contract.toml"),
        "major = 2\nminor = 0",
        "major = 1\nminor = 2",
    );
    edit(
        &m12.join("ns/drive.toml"),
        "introduced_in = \"2.0\"",
        "introduced_in = \"1.2\"",
    );
    let prev = g11.join("contract.json");
    let (ok, stderr) = generate(
        &m12.join("contract.toml"),
        &dir.join("out"),
        &["--previous-contract", prev.to_str().unwrap()],
    );
    assert!(!ok);
    assert!(
        stderr.contains("drive.state.label was removed in a minor"),
        "{stderr}"
    );
    assert!(
        !dir.join("out/dmt_tables.c").exists(),
        "a refusal generates nothing"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn output_is_deterministic_and_variant_resolved() {
    let dir = unique_dir("determinism");
    let (a, b, c) = (dir.join("a"), dir.join("b"), dir.join("c"));
    generate_ok("v1_0", &a, &[]);
    generate_ok("v1_0", &b, &[]);
    for file in [
        "dmt_types.h",
        "dmt_tables.h",
        "dmt_tables.c",
        "contract.json",
    ] {
        assert_eq!(
            fs::read(a.join(file)).unwrap(),
            fs::read(b.join(file)).unwrap(),
            "{file}"
        );
    }
    generate_ok("v1_0", &c, &["--variant", "sku_b"]);
    let blob = |d: &Path| {
        let text = fs::read_to_string(d.join("dmt_tables.c")).unwrap();
        text.lines()
            .find(|l| l.starts_with("const uint8_t dmt_default_blob"))
            .unwrap()
            .to_string()
    };
    assert_ne!(blob(&a), blob(&c), "sku_b changes per-variant defaults");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn contract_mode_refuses_a_template_directory() {
    let dir = unique_dir("templates");
    let templates = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
    let (ok, stderr) = generate(
        &model("v1_0").join("contract.toml"),
        &dir,
        &["--templates", templates.to_str().unwrap()],
    );
    assert!(!ok && stderr.contains("built-in templates"), "{stderr}");
    let _ = fs::remove_dir_all(&dir);
}

/// @brief Replace `from` with `to` in `file`, asserting `from` occurs exactly once.
/// @version 1
fn edit(file: &Path, from: &str, to: &str) {
    let text = fs::read_to_string(file).unwrap();
    assert_eq!(
        text.matches(from).count(),
        1,
        "{}: '{from}' must occur once",
        file.display()
    );
    fs::write(file, text.replacen(from, to, 1)).unwrap();
}

/// One refusal case: model version, file, text to replace, replacement, expected message.
type Case = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

/// Edits of the generic model that each trigger one refusal.
const CASES: &[Case] = &[
    ("v1_0", "ns/core.toml", "key_index = 0\ntype = \"bool\"", "type = \"bool\"", "missing field `key_index`"),
    ("v1_0", "ns/drive.toml", "unit = \"mm/s\"", "units = \"mm/s\"", "unknown field"),
    ("v1_0", "ns/net.toml", "namespace_id = 4", "namespace_id = 3", "share namespace_id"),
    ("v1_0", "ns/drive.toml", "key_index = 4", "key_index = 2", "encode to the same ID"),
    ("v1_0", "ns/core.toml", "namespace_id = 1", "namespace_id = 0", "ID 0"),
    ("v1_0", "ns/drive.toml", "id = \"enabled\"", "id = \"enabled\"\nread_only = true", "read_only and thread_safe"),
    ("v1_0", "ns/drive.toml", "key_index = 4", "key_index = 9", "is reserved"),
    ("v1_0", "ns/drive.toml", "write_rate = \"1/min\"\ndefault = 100", "default = 100", "needs a write_rate"),
    ("v1_0", "ns/drive.toml", "deferred_ms = 5000", "deferred_ms = 0", "deferred_ms > 0"),
    ("v1_0", "ns/drive.toml", "max_size = 65536\n", "", "needs max_size"),
    ("v1_0", "ns/drive.toml", "id = \"enabled\"", "id = \"enabled\"\nalias_of = \"drive.state.nope\"", "alias_of must name"),
    ("v1_0", "ns/drive.toml", "introduced_in = \"1.0\"\nwrite_rate = \"10/s\"", "introduced_in = \"1.1\"\nwrite_rate = \"10/s\"", "later than the contract"),
    ("v1_0", "ns/drive.toml", "default = 100", "default = 70000", "out of range"),
    ("v1_0", "ns/core.toml", "wk = \"SECRETS_FALLBACK\"", "", "SECRETS_FALLBACK is not mapped"),
    ("v1_0", "ns/core.toml", "type = \"uint32\"\nintroduced_in = \"1.0\"\npersistence = \"deferred\"", "type = \"uint16\"\nintroduced_in = \"1.0\"\npersistence = \"deferred\"", "type and class role"),
    ("v1_0", "ns/mfg.toml", "mfg = true", "mfg = false", "MFG namespace"),
    ("v1_0", "ns/core.toml", "payload_max = 118", "payload_max = 0", "payload_max > 0"),
    ("v1_0", "ns/core.toml", "payload_max = 118", "payload_max = 117", "payload_max >= 118"),
    ("v1_0", "ns/core.toml", "max_size = 512", "max_size = 163", "DIAG_SUBSCRIBERS needs max_size >= 164"),
    ("v1_0", "ns/core.toml", "max_size = 32\nintroduced_in = \"1.0\"\nwk = \"DIAG_KEYSTORE_VERSION\"", "max_size = 29\nintroduced_in = \"1.0\"\nwk = \"DIAG_KEYSTORE_VERSION\"", "DIAG_KEYSTORE_VERSION needs max_size >= 30"),
    ("v1_0", "ns/core.toml", "max_size = 32\nintroduced_in = \"1.0\"\nwk = \"DIAG_CONTRACT_VERSION\"", "max_size = 16\nintroduced_in = \"1.0\"\nwk = \"DIAG_CONTRACT_VERSION\"", "DIAG_CONTRACT_VERSION needs max_size >= 17"),
    ("v1_0", "contract.toml", "uid = 2003\nkind", "uid = 2001\nkind", "(kind, uid, token)"),
    ("v1_0", "contract.toml", "heartbeat_window_ms = 2000", "heartbeat_window_ms = 0", "watchdog_required needs"),
    ("v1_0", "contract.toml", "staging_total_bytes = 262144", "staging_total_bytes = 1000", "staging_total_bytes"),
    ("v1_0", "contract.toml", "inproc_snapshot_bytes = 4096", "inproc_snapshot_bytes = 10", "inproc_snapshot_bytes"),
    ("v1_0", "contract.toml", "uid = 2003\ninflight_fd", "uid = 2999\ninflight_fd", "uid_caps must list"),
    ("v1_0", "contract.toml", "{ key = \"drive.state.speed\", access = \"rw\" },", "{ namespace = \"net\", class = \"credentials\", access = \"w\" },", "may not have rights on a secret"),
    ("v1_0", "contract.toml", "{ key = \"net.health.iotd_hb\", access = \"rw\" },", "{ key = \"net.health.iotd_hb\", access = \"rw\" },\n  { key = \"net.credentials.wifi_psk\", access = \"r\" },", "not on the secret's reader list"),
    ("v1_0", "contract.toml", "{ namespace = \"drive\", class = \"state\", access = \"r\" },\n]", "{ namespace = \"drive\", class = \"state\", access = \"r\" },\n  { namespace = \"drive\", class = \"map\", access = \"r\" },\n]", "may not have rights on a struct"),
    ("v1_1", "contract.toml", "  { key = \"drive.state.velocity\", access = \"rw\" },\n", "", "different rights on the alias"),
    ("v1_0", "contract.toml", "{ key = \"drive.state.speed\", access = \"rw\" },", "{ key = \"drive.state.speed\", access = \"rw\" },\n  { namespace = \"mfg\", access = \"w\" },", "UART principal mcu may not write"),
    ("v1_0", "contract.toml", "{ key = \"net.health.iotd_hb\", access = \"rw\" },", "{ key = \"net.health.iotd_hb\", access = \"rw\" },\n  { key = \"net.health.integrator_hb\", access = \"w\" },", "heartbeat of integrator"),
    ("v1_0", "contract.toml", "{ namespace = \"core\", class = \"diag\", access = \"r\" },\n]", "{ namespace = \"core\", class = \"diag\", access = \"rw\" },\n]", "written only by the core"),
    ("v1_0", "contract.toml", "{ key = \"net.health.iotd_hb\", access = \"rw\" },", "{ key = \"net.health.iotd_hb\", access = \"rw\" },\n  { key = \"core.command.trace_enable\", access = \"w\" },", "exactly one writer"),
    ("v1_0", "contract.toml", "{ key = \"core.command.pre_reset_result\", access = \"rw\" },", "{ key = \"core.command.pre_reset_result\", access = \"r\" },", "owner integrator needs"),
    ("v1_0", "ns/core.toml", "owner = \"core\"\ndedupe_ring = 4\nwrite_rate = \"1/day\"\nwk = \"RESET_REQUEST\"", "owner = \"integrator\"\ndedupe_ring = 4\nwrite_rate = \"1/day\"\nwk = \"RESET_REQUEST\"", "must be answered by the core"),
    ("v1_0", "ns/core.toml", "owner = \"integrator\"", "owner = \"core\"", "is only for RESET_REQUEST and TRACE_ENABLE"),
    ("v1_0", "contract.toml", "{ key = \"drive.command.map_edit_result\", access = \"rw\" },", "{ key = \"drive.command.map_edit_result\", access = \"rw\" },\n  { key = \"drive.command.map_edit_dedupe\", access = \"w\" },", "no principal may write a dedupe key"),
    ("v1_0", "contract.toml", "{ key = \"core.command.pre_reset_result\", access = \"rw\" },", "{ key = \"core.command.pre_reset_result\", access = \"rw\" },\n  { key = \"core.command.factory_reset_result\", access = \"w\" },", "answered by the core"),
    ("v1_0", "contract.toml", "name = \"tool\"", "name = \"core\"", "is reserved"),
    ("v1_0", "contract.toml", "{ key = \"net.health.iotd_hb\", access = \"rw\" },", "{ key = \"net.health.iotd_hb\", access = \"rw\" },\n  { key = \"drive.state.nope\", access = \"r\" },", "grant on unknown key"),
    ("v1_0", "ns/core.toml", "wk = \"SECRETS_FALLBACK\"", "wk = \"NOPE\"", "unknown well-known role"),
    ("v1_0", "ns/net.toml", "readers = [\"iotd.net\"]", "readers = [\"nobody\"]", "a reader is not a principal"),
    ("v1_0", "ns/drive.toml", "enum = \"mode\"", "enum = \"nope\"", "undefined enum"),
    ("v1_0", "ns/drive.toml", "write_rate = \"10/s\"", "write_rate = \"10/week\"", "write rate"),
    ("v1_0", "ns/drive.toml", "unit = \"mm/s\"", "unit = \"mm/s\"\nmax_size = 4", "max_size on a scalar"),
    ("v1_0", "ns/drive.toml", "key_index = 4", "key_index = 2000", "exceeds 1023"),
    ("v1_0", "ns/drive.toml", "class_index = 2", "class_index = 1", "out of range or reused"),
];

#[test]
fn every_refusal_has_a_triggering_model() {
    let dir = unique_dir("refusals");
    for (i, (version, file, from, to, expected)) in CASES.iter().enumerate() {
        let case = dir.join(format!("case{i}"));
        copy_dir(&model(version), &case);
        edit(&case.join(file), from, to);
        let (ok, stderr) = generate(&case.join("contract.toml"), &case.join("out"), &[]);
        assert!(!ok, "case {i} ({expected}) must be refused");
        assert!(
            stderr.contains(expected),
            "case {i}: expected '{expected}' in:\n{stderr}"
        );
        assert!(
            !case.join("out/dmt_tables.c").exists(),
            "case {i}: nothing is generated"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

/// Edits of 1.1 (checked against 1.0) or 2.0 (against 1.1) that the previous-contract checks refuse.
const PREVIOUS_CASES: &[(&str, &str, &str, &str, &str, &str)] = &[
    (
        "v1_1",
        "v1_0",
        "ns/drive.toml",
        "persistence = \"deferred\"\ndeferred_ms = 5000",
        "persistence = \"durable\"\ndeferred_ms = 5000",
        "changed in a minor",
    ),
    (
        "v1_1",
        "v1_0",
        "contract.toml",
        "flags = [\"trace_reader\"]",
        "flags = []",
        "principal tool was removed or its uid",
    ),
    (
        "v1_1",
        "v1_0",
        "contract.toml",
        "  { namespace = \"drive\", class = \"state\", access = \"r\" },\n]",
        "]",
        "rights on existing keys changed",
    ),
    (
        "v1_1",
        "v1_0",
        "ns/drive.toml",
        "key_index = 5\ntype = \"int16\"\nintroduced_in = \"1.1\"",
        "key_index = 5\ntype = \"int16\"\nintroduced_in = \"1.0\"",
        "a new key must have introduced_in 1.1",
    ),
    (
        "v1_1",
        "v1_0",
        "ns/drive.toml",
        "eco = 1\n",
        "",
        "changed in a minor",
    ),
    (
        "v2_0",
        "v1_1",
        "contract.toml",
        "[[reserved]]\nid = 0x00C201C0",
        "",
        "was removed but its ID",
    ),
];

/// @brief Assert `contract` is refused against the contract.json in `previous_out`, with a log
/// containing `expected`.
/// @version 1
fn refused_against(contract: &Path, previous_out: &Path, expected: &str) {
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

#[test]
fn previous_contract_refusals() {
    let dir = unique_dir("previous");
    for (i, (version, prev, file, from, to, expected)) in PREVIOUS_CASES.iter().enumerate() {
        let (case, prev_out) = (dir.join(format!("case{i}")), dir.join(format!("prev{i}")));
        generate_ok(prev, &prev_out, &[]);
        copy_dir(&model(version), &case);
        edit(&case.join(file), from, to);
        refused_against(&case.join("contract.toml"), &prev_out, expected);
    }
    let older = dir.join("older");
    generate_ok("v1_1", &older, &[]);
    refused_against(
        &model("v1_0").join("contract.toml"),
        &older,
        "older than the previous",
    );
    let _ = fs::remove_dir_all(&dir);
}

/// @brief The rights cell of principal `name` on key `path` in a contract.json ("" for none).
/// @version 1
fn right(gen: &Path, name: &str, path: &str) -> String {
    contract_json(gen)["rights"][name][path]
        .as_str()
        .unwrap_or("")
        .to_string()
}

#[test]
fn rights_are_resolved() {
    let dir = unique_dir("rights");
    let gen = dir.join("gen");
    generate_ok("v1_0", &gen, &[]);
    // read on a secret comes from its reader list only; write from grants
    assert_eq!(right(&gen, "iotd.net", "net.credentials.wifi_psk"), "rw");
    assert_eq!(right(&gen, "integrator", "net.credentials.wifi_psk"), "");
    assert_eq!(
        right(&gen, "integrator", "net.credentials.cloud_token"),
        "r"
    );
    // a key grant replaces the class grant; class grants combine
    assert_eq!(right(&gen, "mcu", "drive.state.speed"), "rw");
    assert_eq!(right(&gen, "mcu", "drive.state.odometer"), "");
    assert_eq!(right(&gen, "integrator", "net.health.iotd_hb"), "r");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn queue_bound_is_computed_from_rates_and_latency() {
    // ceil(200 * 1_127_550 writes/day * 50_000 ms / (100 * 86_400_000)) = 1306
    let dir = unique_dir("bound");
    let model_dir = dir.join("model");
    copy_dir(&model("v1_0"), &model_dir);
    edit(
        &model_dir.join("contract.toml"),
        "latency_budget_ms = 50\ncaps = { inflight_fd = 4",
        "latency_budget_ms = 50000\ncaps = { inflight_fd = 4",
    );
    let out = dir.join("bound");
    let (ok, stderr) = generate(&model_dir.join("contract.toml"), &out, &[]);
    assert!(ok, "{stderr}");
    assert_eq!(principal(&out, "integrator")["queue_bound"], 1306);
    let _ = fs::remove_dir_all(&dir);
}
