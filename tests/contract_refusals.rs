//! Contract-mode refusals: one model edit per structural, rights and bounds refusal, one per
//! previous-contract refusal, and the release review of new principals.

mod common;

use common::*;
use std::fs;
use std::path::Path;

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
    ("v1_0", "ns/core.toml", "namespace_id = 1", "namespace_id = 0", "encodes to ID 0, which is reserved forever"),
    ("v1_0", "ns/drive.toml", "id = \"enabled\"", "id = \"enabled\"\nread_only = true", "read_only and thread_safe"),
    ("v1_0", "ns/drive.toml", "key_index = 4", "key_index = 9", "is reserved"),
    ("v1_0", "ns/drive.toml", "write_rate = \"1/min\"\ndefault = 100", "default = 100", "needs a write_rate"),
    ("v1_0", "ns/drive.toml", "deferred_ms = 5000", "deferred_ms = 0", "deferred_ms > 0"),
    ("v1_0", "ns/drive.toml", "max_size = 65536\n", "", "needs max_size"),
    ("v1_0", "ns/drive.toml", "id = \"enabled\"", "id = \"enabled\"\nalias_of = \"drive.state.nope\"", "alias_of must name"),
    ("v1_0", "ns/drive.toml", "introduced_in = \"1.0\"\nwrite_rate = \"10/s\"", "introduced_in = \"1.1\"\nwrite_rate = \"10/s\"", "later than the contract"),
    ("v1_0", "ns/drive.toml", "default = 100", "default = 70000", "out of range"),
    ("v1_0", "ns/core.toml", "wk = \"SECRETS_FALLBACK\"", "", "SECRETS_FALLBACK is not mapped"),
    ("v1_0", "ns/core.toml", "type = \"uint32\"\nintroduced_in = \"1.0\"\npersistence = \"deferred\"", "type = \"uint16\"\nintroduced_in = \"1.0\"\npersistence = \"deferred\"", "core.diag.corruption: role DIAG_CORRUPTION needs type uint32 (has uint16)"),
    ("v1_0", "ns/mfg.toml", "mfg = true", "mfg = false", "MFG namespace"),
    ("v1_0", "ns/core.toml", "type = \"uint32\"\nintroduced_in = \"1.0\"\npersistence = \"deferred\"", "type = \"uint32\"\nintroduced_in = \"1.0\"\npersistence = \"durable\"", "core.diag.corruption: role DIAG_CORRUPTION needs persistence deferred (has durable)"),
    ("v1_0", "ns/core.toml", "survives_reset = true\n", "", "core.config.boot_epoch: role BOOT_EPOCH needs survives_reset = true (has false)"),
    ("v1_0", "ns/core.toml", "payload_max = 118", "payload_max = 0", "payload_max > 0"),
    ("v1_0", "ns/core.toml", "payload_max = 118", "payload_max = 117", "payload_max >= 118"),
    ("v1_0", "ns/core.toml", "max_size = 512", "max_size = 163", "DIAG_SUBSCRIBERS needs max_size >= 164"),
    ("v1_0", "ns/core.toml", "max_size = 32\nintroduced_in = \"1.0\"\nwk = \"DIAG_KEYSTORE_VERSION\"", "max_size = 29\nintroduced_in = \"1.0\"\nwk = \"DIAG_KEYSTORE_VERSION\"", "DIAG_KEYSTORE_VERSION needs max_size >= 30"),
    ("v1_0", "ns/core.toml", "max_size = 32\nintroduced_in = \"1.0\"\nwk = \"DIAG_CONTRACT_VERSION\"", "max_size = 16\nintroduced_in = \"1.0\"\nwk = \"DIAG_CONTRACT_VERSION\"", "DIAG_CONTRACT_VERSION needs max_size >= 17"),
    ("v1_0", "contract.toml", "uid = 2003\nkind", "uid = 2001\nkind", "(kind, uid, token)"),
    ("v1_0", "contract.toml", "heartbeat_window_ms = 2000", "heartbeat_window_ms = 0", "watchdog_required needs"),
    ("v1_0", "contract.toml", "staging_total_bytes = 262144", "staging_total_bytes = 1000", "staging_total_bytes"),
    ("v1_0", "contract.toml", "inproc_snapshot_bytes = 4096", "inproc_snapshot_bytes = 46", "inproc_snapshot_bytes 46 is below its lower bound 47"),
    ("v1_0", "contract.toml", "uid = 2003\ninflight_fd", "uid = 2999\ninflight_fd", "uid_caps must list"),
    ("v1_0", "contract.toml", "{ key = \"drive.state.speed\", access = \"rw\" },", "{ namespace = \"net\", class = \"credentials\", access = \"w\" },", "may not have rights on a secret"),
    ("v1_0", "contract.toml", "{ key = \"net.health.iotd_hb\", access = \"rw\" },", "{ key = \"net.health.iotd_hb\", access = \"rw\" },\n  { key = \"net.credentials.wifi_psk\", access = \"r\" },", "not on the secret's reader list"),
    ("v1_0", "contract.toml", "{ namespace = \"drive\", class = \"state\", access = \"r\" },\n]", "{ namespace = \"drive\", class = \"state\", access = \"r\" },\n  { namespace = \"drive\", class = \"map\", access = \"r\" },\n]", "may not have rights on a struct"),
    ("v1_1", "contract.toml", "  { key = \"drive.state.velocity\", access = \"rw\" },\n", "", "different rights on the alias"),
    ("v1_0", "contract.toml", "{ key = \"drive.state.speed\", access = \"rw\" },", "{ key = \"drive.state.speed\", access = \"rw\" },\n  { namespace = \"mfg\", access = \"w\" },", "mfg.identity.serial: UART principal mcu may not write"),
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
    ("v1_0", "ns/drive.toml", "id = \"enabled\"\nkey_index = 2\ntype = \"bool\"\nintroduced_in = \"1.0\"", "id = \"enabled\"\nkey_index = 2\ntype = \"bool\"\nintroduced_in = \"1\"", "drive.state: introduced_in '1' is not a"),
    ("v1_0", "ns/drive.toml", "write_rate = \"10/s\"", "write_rate = \"10/week\"", "write rate"),
    ("v1_0", "ns/drive.toml", "unit = \"mm/s\"", "unit = \"mm/s\"\nmax_size = 4", "max_size on a scalar"),
    ("v1_0", "ns/drive.toml", "key_index = 4", "key_index = 1024", "key_index 1024 exceeds 1023"),
    ("v1_0", "ns/core.toml", "cmd_index = 6", "cmd_index = 1024", "core.command.trace_enable: key_index 1024 exceeds 1023"),
    ("v1_0", "contract.toml", "{ key = \"drive.state.speed\", access = \"rw\" },", "{ key = \"drive.state.speed\", access = \"rw\" },\n  { key = \"core.command.pre_reset\", access = \"w\" },", "core.command.pre_reset: UART principal mcu may not write"),
    ("v1_0", "contract.toml", "{ key = \"drive.state.speed\", access = \"rw\" },", "{ key = \"drive.state.speed\", access = \"rw\" },\n  { key = \"core.command.trace_enable\", access = \"w\" },", "core.command.trace_enable: UART principal mcu may not write"),
    ("v1_0", "contract.toml", "{ key = \"net.health.iotd_hb\", access = \"rw\" },", "{ key = \"net.health.iotd_hb\", access = \"r\" },", "net.health.iotd_hb: heartbeat of iotd must be written"),
    ("v1_0", "ns/drive.toml", "class_index = 2", "class_index = 1", "out of range or reused"),
    ("v1_0", "ns/drive.toml", "id = \"enabled\"", "id = \"enabled\"\nthread_safe = true", "read_only and thread_safe"),
    ("v1_0", "ns/drive.toml", "max_size = 32\nintroduced_in = \"1.0\"\ndefault = \"idle\"", "max_size = 0\nintroduced_in = \"1.0\"\ndefault = \"idle\"", "needs max_size > 0"),
    ("v1_0", "ns/core.toml", "namespace_id = 1", "namespace_id = 1024", "namespace_id 1024 exceeds 1023"),
    ("v1_0", "ns/drive.toml", "class_index = 2", "class_index = 32", "out of range or reused"),
    ("v1_0", "ns/drive.toml", "id = \"enabled\"", "id = \"enabled\"\nreaders = [\"iotd\"]", "only for secret keys"),
    ("v1_1", "ns/drive.toml", "key_index = 5\ntype = \"int16\"\nintroduced_in = \"1.1\"", "key_index = 5\ntype = \"int16\"\nintroduced_in = \"1.1\"\nalias_of = \"drive.state.speed\"", "alias_of must name"),
    ("v1_0", "ns/drive.toml", "id = \"enabled\"", "id = \"enabled\"\nalias_of = \"drive.state.speed\"", "alias_of must name"),
    ("v1_0", "ns/core.toml", "wk = \"DIAG_FSYNC_COUNT\"", "wk = \"DIAG_CORRUPTION\"", "mapped more than once"),
    ("v1_0", "ns/core.toml", "persistence = \"none\"\nowner = \"core\"\npayload_max = 118", "persistence = \"durable\"\nowner = \"core\"\npayload_max = 118", "non-persistent binary command"),
    ("v1_0", "ns/core.toml", "payload_max = 118", "payload = \"struct\"\npayload_max = 118", "non-persistent binary command"),
    ("v1_0", "contract.toml", "name = \"tool\"", "name = \"iotd\"", "is used twice or is reserved"),
    ("v1_0", "contract.toml", "staging_bound_bytes = 16384\nintroduced_in = \"1.0\"\ngrants = [\n  { namespace = \"core\", class = \"diag\", access = \"r\" },\n]", "staging_bound_bytes = 16384\nintroduced_in = \"1.1\"\ngrants = [\n  { namespace = \"core\", class = \"diag\", access = \"r\" },\n]", "principal tool: introduced_in is later"),
    ("v1_0", "contract.toml", "heartbeat = \"net.health.integrator_hb\"", "heartbeat = \"net.health.nope\"", "heartbeat is not a key"),
    ("v1_0", "contract.toml", "heartbeat = \"net.health.integrator_hb\"\n", "", "watchdog_required needs"),
    ("v1_0", "contract.toml", "[[uid_caps]]\nuid = 2003", "[[uid_caps]]\nuid = 2002\ninflight_fd = 1\ninflight_bytes = 1\nopen_fds = 1\n\n[[uid_caps]]\nuid = 2003", "uid_caps must list"),
    ("v1_0", "contract.toml", "{ key = \"drive.state.speed\", access = \"rw\" },", "{ key = \"drive.state.speed\", access = \"rw\" },\n  { namespace = \"drive\", class = \"map\", access = \"r\" },", "may not have rights on a struct"),
    ("v1_0", "contract.toml", "{ key = \"drive.state.speed\", access = \"rw\" },", "{ key = \"drive.state.speed\", access = \"rw\" },\n  { key = \"core.command.factory_reset\", access = \"w\" },", "core.command.factory_reset: UART principal mcu may not write"),
    ("v1_0", "contract.toml", "flags = [\"trace_reader\"]", "flags = [\"trace_reader\"]\nheartbeat = \"net.health.iotd_hb\"\nheartbeat_window_ms = 5000", "heartbeat of"),
    ("v1_0", "contract.toml", "  { key = \"core.command.trace_enable\", access = \"rw\" },\n", "", "exactly one writer"),
    ("v1_0", "ns/core.toml", "owner = \"integrator\"", "owner = \"nobody\"", "owner nobody is not a principal"),
    ("v1_0", "contract.toml", "  { key = \"core.command.pre_reset\", access = \"r\" },\n", "", "owner integrator needs"),
    ("v1_0", "contract.toml", "inproc_queue_bytes = 8192", "inproc_queue_bytes = 511", "inproc_queue_bytes 511 is below its lower bound 512"),
    ("v1_0", "contract.toml", "{ key = \"net.health.iotd_hb\", access = \"rw\" },", "{ key = \"net.health.iotd_hb\", access = \"rw\" },\n  { namespace = \"drive\", key = \"drive.state.speed\", access = \"r\" },", "names either a key or a namespace"),
    ("v1_0", "contract.toml", "{ key = \"net.health.iotd_hb\", access = \"rw\" },", "{ key = \"net.health.iotd_hb\", access = \"rw\" },\n  { class = \"state\", access = \"r\" },", "names either a key or a namespace"),
    ("v1_0", "contract.toml", "{ key = \"net.health.iotd_hb\", access = \"rw\" },", "{ key = \"net.health.iotd_hb\", access = \"rw\" },\n  { namespace = \"drvie\", access = \"r\" },", "grant on drvie covers no key"),
    ("v1_0", "contract.toml", "{ key = \"net.health.iotd_hb\", access = \"rw\" },", "{ key = \"net.health.iotd_hb\", access = \"rw\" },\n  { namespace = \"drive\", class = \"stat\", access = \"r\" },", "grant on drive.stat covers no key"),
    ("v1_0", "ns/net.toml", "id = \"net\"", "id = \"drive\"", "declared by two files"),
    ("v1_0", "ns/drive.toml", "defaults = { sku_b = 80 }", "defaults = { sku_c = 80 }", "drive.settings.speed_limit: variant sku_c is not declared (declared: sku_a, sku_b)"),
    ("v1_0", "ns/drive.toml", "[enums.mode.variants.sku_b]  ", "[enums.mode.variants.sku_c]  ", "drive::mode: variant sku_c is not declared"),
    ("v1_0", "ns/drive.toml", "remove = [\"sport\"]", "remove = [\"sprot\"]", "drive::mode (sku_b): the variant removes sprot, which is not a base value"),
    ("v1_0", "ns/drive.toml", "saver = 1", "saver = 3", "drive::mode (sku_b): number 3 has two names"),
    ("v1_0", "ns/drive.toml", "wire = \"sportMode\"", "wire = \"eco\"", "drive::mode (sku_a): wire string eco is used twice"),
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
/// The map_edit command of the generic model, and the same three paths, IDs and types declared
/// as plain keys: only the command membership differs.
const MAP_EDIT_COMMAND: &str = "[[classes.commands]]\nid = \"map_edit\"\ncmd_index = 0\nresult_index = 1\ndedupe_index = 2\nintroduced_in = \"1.0\"\npersistence = \"durable\"\nowner = \"iotd.map\"\npayload = \"struct\"\npayload_max = 32768\nresult_max = 256\nmax_edits = 32\ndedupe_ring = 8\nwrite_rate = \"1/min\"\n";
const MAP_EDIT_PLAIN: &str = "persistence = \"durable\"\n\n[[classes.keys]]\nid = \"map_edit\"\nkey_index = 0\ntype = \"struct\"\nmax_size = 65536\nintroduced_in = \"1.0\"\nwrite_rate = \"1/min\"\n\n[[classes.keys]]\nid = \"map_edit_result\"\nkey_index = 1\ntype = \"binary\"\nmax_size = 65536\nintroduced_in = \"1.0\"\nwrite_rate = \"1/min\"\n\n[[classes.keys]]\nid = \"map_edit_dedupe\"\nkey_index = 2\ntype = \"binary\"\nmax_size = 65536\nintroduced_in = \"1.0\"\nwrite_rate = \"1/min\"\n";

const PREVIOUS_CASES: &[(&str, &str, &str, &str, &str, &str)] = &[
    (
        "v1_1",
        "v1_0",
        "ns/drive.toml",
        "id = \"speed_limit\"",
        "id = \"speed_limit\"\npersistence = \"none\"",
        "drive.settings.speed_limit: ID",
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
        "drive.settings.mode: ID",
    ),
    ("v1_1", "v1_0", "contract.toml", "[[reserved]]\nid = 0x00C20480   # drive.state index 9 (bool): retired before 1.0\n", "", "reserved ID 0x00C20480 is no longer reserved"),
    ("v2_0", "v1_1", "ns/drive.toml", "id = \"enabled\"", "id = \"active\"", "is reused by drive.state.active"),
    ("v1_1", "v1_0", "ns/drive.toml", "enum = \"mode\"\nintroduced_in = \"1.0\"", "enum = \"mode\"\nintroduced_in = \"1.1\"", "drive.settings.mode: ID"),
    ("v1_1", "v1_0", "ns/drive.toml", "enum = \"mode\"\nintroduced_in = \"1.0\"", "introduced_in = \"1.0\"", "drive.settings.mode: ID"),
    ("v1_1", "v1_0", "ns/drive.toml", "wire = \"sportMode\"", "wire = \"sport_mode\"", "drive.settings.mode: ID"),
    ("v1_1", "v1_0", "ns/net.toml", "secret = true\nreaders = [\"iotd.net\", \"integrator\", \"iotd.ota\"]\n", "", "net.credentials.cloud_token: ID"),
    ("v1_1", "v1_0", "ns/drive.toml", "id = \"speed_limit\"", "id = \"speed_limit\"\nsurvives_reset = true", "drive.settings.speed_limit: ID"),
    ("v1_1", "v1_0", "ns/drive.toml", "deferred_ms = 5000", "deferred_ms = 6000", "drive.state.odometer: ID"),
    ("v1_1", "v1_0", "ns/drive.toml", "max_size = 32\nintroduced_in = \"1.0\"\ndefault = \"idle\"", "max_size = 16\nintroduced_in = \"1.0\"\ndefault = \"idle\"", "drive.state.label: ID"),
    ("v1_1", "v1_0", "ns/drive.toml", "id = \"enabled\"\nkey_index = 2", "id = \"enabled\"\nkey_index = 6", "drive.state.enabled: ID"),
    ("v1_1", "v1_1", "ns/drive.toml", "alias_of = \"drive.state.velocity\"\n", "", "drive.state.speed: ID"),
    ("v1_1", "v1_0", "ns/drive.toml", MAP_EDIT_COMMAND, MAP_EDIT_PLAIN, "drive.command.map_edit: ID"),
    ("v1_0", "v1_0", "ns/drive.toml", "id = \"error_overcurrent\"", "id = \"spare\"\nkey_index = 6\ntype = \"bool\"\nintroduced_in = \"1.0\"\n\n[[classes.keys]]\nid = \"error_overcurrent\"", "keys were added without a minor bump"),
    ("v1_1", "v1_0", "contract.toml", "name = \"tool\"", "name = \"tools\"", "principal tool was removed"),
    ("v1_1", "v1_0", "contract.toml", "uid = 1\nkind = \"uart\"", "uid = 2\nkind = \"uart\"", "principal mcu was removed"),
    ("v1_1", "v1_0", "contract.toml", "token = 3", "token = 5", "principal iotd.map was removed"),
    ("v1_1", "v1_0", "contract.toml", "uid = 10\nkind = \"inproc\"", "uid = 10\nkind = \"uart\"", "principal inproc was removed"),
    (
        "v2_0",
        "v1_1",
        "contract.toml",
        "[[reserved]]\nid = 0x00C201C0",
        "",
        "was removed but its ID",
    ),
];

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

/// New-principal grants of the 1.1 model: write on an MFG key and on both reset commands, read
/// on an MFG key, a secret and a diag class, and write on a secret and on a key new in 1.1 (and
/// its alias).
const OTA_GRANTS: &str = "  { namespace = \"drive\", class = \"settings\", access = \"r\" },\n  { namespace = \"core\", class = \"diag\", access = \"r\" },\n  { key = \"mfg.identity.serial\", access = \"w\" },\n  { key = \"mfg.identity.device_id\", access = \"r\" },\n  { key = \"core.command.factory_reset\", access = \"w\" },\n  { key = \"core.command.pre_reset\", access = \"w\" },\n  { key = \"net.credentials.wifi_psk\", access = \"w\" },\n  { key = \"drive.state.velocity\", access = \"w\" },\n  { key = \"drive.state.speed\", access = \"w\" },\n]";

/// Where the 1.1 model's iotd.ota grants end.
const OTA_GRANTS_END: &str =
    "  { namespace = \"drive\", class = \"settings\", access = \"r\" },\n]\n\n[[uid_caps]]";

/// @brief Generate the 1.1 model edited by `edits` (file, from, to) against 1.0; return the
/// release-review lines.
/// @version 1
fn review_lines(tag: &str, edits: &[(&str, &str, &str)]) -> Vec<String> {
    let dir = unique_dir(tag);
    let (prev, model_dir, gen) = (dir.join("prev"), dir.join("model"), dir.join("gen"));
    generate_ok("v1_0", &prev, &[]);
    edited_copy("v1_1", &model_dir, edits);
    let previous = prev.join("contract.json");
    let args = ["--previous-contract", previous.to_str().unwrap()];
    let (ok, stderr) = generate(&model_dir.join("contract.toml"), &gen, &args);
    assert!(ok, "{stderr}");
    let review = fs::read_to_string(gen.join("release-review.txt")).unwrap();
    let _ = fs::remove_dir_all(&dir);
    review.lines().map(str::to_string).collect()
}

#[test]
fn release_review_lists_only_sensitive_grants_of_new_principals() {
    let ota = format!("{OTA_GRANTS}\n\n[[uid_caps]]");
    let lines = review_lines("review", &[("contract.toml", OTA_GRANTS_END, &ota)]);
    assert_eq!(
        lines,
        [
            "new principal iotd.ota: w on existing core.command.factory_reset",
            "new principal iotd.ota: w on existing core.command.pre_reset",
            "new principal iotd.ota: w on existing mfg.identity.serial",
            "new principal iotd.ota: r on existing net.credentials.cloud_token",
        ]
    );
}

/// The 1.1 model's trace-enable command, moved off its role to an owner new in 1.1, and a new
/// command taking the role. (A role move within a major is not refused today.)
const TRACE_ROLE: &str = "owner = \"core\"\npayload_max = 118\ndedupe_ring = 4\nwk = \"TRACE_ENABLE\"\ndoc = \"Signed trace-enable envelope; the core verifies it.\"\n";
const TRACE_ROLE_MOVED: &str = "owner = \"iotd.ota\"\npayload_max = 118\ndedupe_ring = 4\ndoc = \"Signed trace-enable envelope; the core verifies it.\"\n\n[[classes.commands]]\nid = \"trace_enable2\"\ncmd_index = 9\nresult_index = 10\ndedupe_index = 11\nintroduced_in = \"1.1\"\npersistence = \"none\"\nowner = \"core\"\npayload_max = 118\ndedupe_ring = 4\nwk = \"TRACE_ENABLE\"\n";

#[test]
fn release_review_sees_a_write_on_the_previous_trace_enable_key() {
    let cloud = "  { key = \"core.command.trace_enable\", access = \"rw\" },\n  { key = \"drive.command.map_edit\", access = \"rw\" },";
    let cloud2 = "  { key = \"core.command.trace_enable\", access = \"rw\" },\n  { key = \"core.command.trace_enable2\", access = \"rw\" },\n  { key = \"drive.command.map_edit\", access = \"rw\" },";
    let ota = "  { key = \"core.command.trace_enable\", access = \"rw\" },\n  { key = \"core.command.trace_enable_result\", access = \"rw\" },\n]\n\n[[uid_caps]]";
    let lines = review_lines(
        "review_trace",
        &[
            ("ns/core.toml", TRACE_ROLE, TRACE_ROLE_MOVED),
            ("contract.toml", cloud, cloud2),
            ("contract.toml", "]\n\n[[uid_caps]]", ota),
        ],
    );
    assert_eq!(
        lines,
        [
            "new principal iotd.ota: rw on existing core.command.trace_enable",
            "new principal iotd.ota: r on existing net.credentials.cloud_token",
        ]
    );
}

/// @brief Run contract mode on `model_dir/contract.toml` and assert it is refused with a log
/// containing `expected`, generating nothing.
/// @version 1
fn refused(model_dir: &Path, expected: &str) {
    let out = model_dir.join("out");
    let (ok, stderr) = generate(&model_dir.join("contract.toml"), &out, &[]);
    assert!(
        !ok && stderr.contains(expected),
        "'{expected}' in:\n{stderr}"
    );
    assert!(!out.join("dmt_tables.c").exists(), "nothing is generated");
}

#[test]
fn a_model_without_principals_is_refused() {
    let dir = unique_dir("no_principals");
    copy_dir(&model("v1_0"), &dir);
    let contract = dir.join("contract.toml");
    let text = fs::read_to_string(&contract).unwrap();
    let head = text.split("[[principals]]").next().unwrap();
    fs::write(&contract, head).unwrap();
    refused(&dir, "the model declares no principals");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_namespace_directory_without_toml_files_is_refused() {
    let dir = unique_dir("empty_ns");
    copy_dir(&model("v1_0"), &dir);
    for entry in fs::read_dir(dir.join("ns")).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    fs::write(dir.join("ns/README.md"), "not a namespace").unwrap();
    refused(&dir, "no namespace .toml files");
    let _ = fs::remove_dir_all(&dir);
}

/// Refusals that need more than one edit: model version, edits (file, from, to), expected log.
type MultiCase = (
    &'static str,
    &'static [(&'static str, &'static str, &'static str)],
    &'static str,
);

const MULTI_CASES: &[MultiCase] = &[
    // a command role moved onto a plain key
    (
        "v1_0",
        &[
            ("ns/core.toml", "wk = \"RESET_PRE_COMMAND\"\n", ""),
            ("ns/core.toml", "wk = \"DIAG_FSYNC_COUNT\"", "wk = \"RESET_PRE_COMMAND\""),
        ],
        "core.diag.fsync_count: role RESET_PRE_COMMAND needs a command key",
    ),
    // a typed role on a command key of the right type, persistence and survives-reset
    (
        "v1_0",
        &[
            ("ns/core.toml", "wk = \"DIAG_SUBSCRIBERS\"\n", ""),
            (
                "ns/drive.toml",
                "persistence = \"durable\"\nowner = \"iotd.map\"\npayload = \"struct\"",
                "persistence = \"none\"\nowner = \"iotd.map\"\npayload = \"binary\"\nwk = \"DIAG_SUBSCRIBERS\"",
            ),
        ],
        "drive.command.map_edit: role DIAG_SUBSCRIBERS needs a plain key, not a command",
    ),
];

#[test]
fn every_multi_edit_refusal_has_a_triggering_model() {
    let dir = unique_dir("multi");
    for (i, (version, edits, expected)) in MULTI_CASES.iter().enumerate() {
        let case = dir.join(format!("case{i}"));
        edited_copy(version, &case, edits);
        refused(&case, expected);
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_model_declaring_variants_needs_a_declared_variant() {
    let dir = unique_dir("variant_flag");
    let contract = model("v1_0").join("contract.toml");
    let out = dir.join("out");
    let base = [
        "--contract",
        contract.to_str().unwrap(),
        "--output",
        out.to_str().unwrap(),
        "--allow-dirty-ingot",
    ];
    let (ok, stderr) = run_ingot(&[&base[..], &["--variant", "nosuch"]].concat());
    assert!(
        !ok && stderr.contains("--variant nosuch is not declared (declared: sku_a, sku_b)"),
        "{stderr}"
    );
    let (ok, stderr) = run_ingot(&base);
    assert!(
        !ok && stderr.contains("--variant must name one of: sku_a, sku_b"),
        "{stderr}"
    );
    assert!(!out.join("dmt_tables.c").exists());
    let _ = fs::remove_dir_all(&dir);
}

/// @brief Remove the fields ingot added after TIV 1.0 shipped (variants, enum_wire) from the
/// contract.json in `gen`, as an older ingot would have written it.
/// @version 1
fn strip_to_old_snapshot(gen: &Path) {
    let path = gen.join("contract.json");
    let mut json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    json.as_object_mut().unwrap().remove("variants");
    for key in json["keys"].as_array_mut().unwrap() {
        key.as_object_mut().unwrap().remove("enum_wire");
    }
    fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();
}

#[test]
fn a_snapshot_from_before_wire_strings_is_still_checked() {
    // an old contract.json reads back, and enum changes are still refused through the enum
    // values alone: the enum dropped from a key, and a value removed
    let edits = [
        (
            "enum = \"mode\"\nintroduced_in = \"1.0\"",
            "introduced_in = \"1.0\"",
        ),
        ("eco = 1\n", ""),
    ];
    let dir = unique_dir("old_snapshot");
    for (i, (from, to)) in edits.iter().enumerate() {
        let (prev, case) = (dir.join(format!("prev{i}")), dir.join(format!("model{i}")));
        generate_ok("v1_0", &prev, &[]);
        strip_to_old_snapshot(&prev);
        edited_copy("v1_1", &case, &[("ns/drive.toml", from, to)]);
        refused_against(
            &case.join("contract.toml"),
            &prev,
            "drive.settings.mode: ID",
        );
    }
    let _ = fs::remove_dir_all(&dir);
}
