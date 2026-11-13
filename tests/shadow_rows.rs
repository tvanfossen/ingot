//! Forms of the shadow mapping beyond one path per key and one command per row: declared
//! outbound many-to-one (report_as), per-variant paths, argument forms and commandless rows.

mod common;

use common::*;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

/// Keys added to the command class of the 1.0 model for the argument forms.
const ARG_KEYS: &str = "id = \"map_payload\"\nkey_index = 9\ntype = \"string\"\nmax_size = 512\nintroduced_in = \"1.0\"\ndoc = \"Base64 text of a map request, passed through as text.\"\n\n[[classes.keys]]\nid = \"drive_mode\"\nkey_index = 10\ntype = \"uint8\"\nenum = \"mode\"\nintroduced_in = \"1.0\"\n\n[[classes.keys]]\nid = \"plan_blob\"\nkey_index = 11\ntype = \"binary\"\nmax_size = 96\nintroduced_in = \"1.0\"\n\n[[classes.keys]]\nid = \"path_from\"\nkey_index = 12\ntype = \"int16\"\nintroduced_in = \"1.0\"";

/// The go row of the 1.0 model.
const GO_ROW: &str =
    "selector = { field = \"state\", wire = \"go\" }\ncommand = \"drive.command.drive\"";

/// The stop row of the 1.0 model.
const STOP_ROW: &str =
    "selector = { field = \"state\", wire = \"stop\" }\ncommand = \"drive.command.drive\"";

/// @brief Generate the edited 1.0 model for `variant` and return the output directory.
/// @version 1
fn generated(
    dir: &Path,
    tag: &str,
    variant: &str,
    edits: &[(&str, &str, &str)],
) -> std::path::PathBuf {
    let case = dir.join(tag);
    edited_copy("v1_0", &case, edits);
    let out = case.join(variant);
    let (ok, stderr) = generate(&case.join("contract.toml"), &out, &["--variant", variant]);
    assert!(ok, "{tag} ({variant}): {stderr}");
    out
}

/// @brief A parsed generated JSON file.
/// @version 1
fn read(gen: &Path, file: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(gen.join(file)).unwrap()).unwrap()
}

/// @brief The wire rows of command `name` in a keyspace document.
/// @version 1
fn wire_rows(doc: &Value, name: &str) -> Vec<Value> {
    doc["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap()["wire"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn two_codes_report_one_wire_string_when_one_declares_it() {
    let dir = unique_dir("rows_report_as");
    for (tag, target) in [("by_name", "boost"), ("by_wire", "high")] {
        let line = format!("boost = {{ value = 2, wire = \"high\", accept = [\"max\"] }}\nboosted = {{ value = 3, report_as = \"{target}\", accept = [\"turbo\"] }}");
        let anchor = "boost = { value = 2, wire = \"high\", accept = [\"max\"] }";
        let gen = generated(&dir, tag, "sku_b", &[("ns/drive.toml", anchor, &line)]);
        let fan = &read(&gen, "keyspace.sku_b.json")["enums"][0];
        assert_eq!(fan["name"], "drive::fan");
        assert_eq!(
            fan["values"][3],
            json!({"code": 3, "name": "boosted", "wire": "high", "report_as": "boost", "accept": ["turbo"]}),
            "{tag}"
        );
        assert!(fan["values"][2].get("report_as").is_none());
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_path_can_exist_on_one_variant() {
    let dir = unique_dir("rows_variant_paths");
    let paths = "shadow = [{ path = \"state.docked\", variants = [\"sku_a\"] }, { path = \"state.isDocked\", variants = [\"sku_b\"] }]\nforced";
    let edits = [("ns/drive.toml", "shadow = \"state.docked\"\nforced", paths)];
    for (variant, path) in [("sku_a", "state.docked"), ("sku_b", "state.isDocked")] {
        let gen = generated(&dir, variant, variant, &edits);
        let key = key_json(&gen, "drive.state.docked");
        assert_eq!(key["shadow"], json!([path]), "{variant}");
        assert_eq!(
            key["shadow_forms"][0]["variants"],
            json!([variant]),
            "{variant}"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

/// @brief The edits that add the argument keys and every argument form to the 1.0 model.
/// @version 1
fn argument_edits() -> Vec<(&'static str, String, String)> {
    let anchor = "id = \"map_payload\"\nkey_index = 9\ntype = \"string\"\nmax_size = 512\nintroduced_in = \"1.0\"\ndoc = \"Base64 text of a map request, passed through as text.\"";
    let go =
        format!("{GO_ROW}\nargs = [ {{ key = \"drive.command.drive_mode\", value = \"off\" }} ]");
    let stop = format!("{STOP_ROW}\nargs = [\n  {{ field = \"mode\", key = \"drive.command.drive_mode\", absent = \"off\" }},\n  {{ field = \"plan\", key = \"drive.command.plan_blob\", omit_when_absent = true, encoding = \"base64\" }},\n  {{ field = \"pathRange\", element = 0, key = \"drive.command.path_from\", absent = -1, encoding = \"decimal_text\" }},\n  {{ field = \"speed\", key = \"drive.state.speed\", omit_when_absent = true }},\n]");
    vec![
        ("ns/drive.toml", anchor.to_string(), ARG_KEYS.to_string()),
        ("ns/drive.toml", GO_ROW.to_string(), go),
        ("ns/drive.toml", STOP_ROW.to_string(), stop),
    ]
}

#[test]
fn constant_named_state_element_and_binary_arguments_generate() {
    let dir = unique_dir("rows_args");
    let owned = argument_edits();
    let edits: Vec<(&str, &str, &str)> = owned
        .iter()
        .map(|(f, a, b)| (*f, a.as_str(), b.as_str()))
        .collect();
    let gen = generated(&dir, "args", "sku_b", &edits);
    let rows = wire_rows(&read(&gen, "keyspace.sku_b.json"), "drive.command.drive");
    let constant = &rows[0]["args"][0];
    assert_eq!(constant["key"], "drive.command.drive_mode");
    assert_eq!(constant["value"], "off");
    assert!(constant.get("field").is_none() && constant.get("element").is_none());
    let stop = &rows[1]["args"];
    assert_eq!(stop[0]["absent"], "off");
    assert_eq!(stop[1]["encoding"], "base64");
    assert_eq!(stop[2]["element"], 0);
    assert_eq!(stop[2]["encoding"], "decimal_text");
    assert_eq!(stop[3]["key"], "drive.state.speed");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rows_without_a_command_answer_stay_internal_or_write_state() {
    let dir = unique_dir("rows_commandless");
    let rows = format!("{GO_ROW}\n\n[[shadow_commands]]\ntopic = \"command_topic\"\nselector = {{ field = \"state\", wire = \"ping\" }}\nanswer = 3\n\n[[shadow_commands]]\ntopic = \"command_topic\"\nselector = {{ field = \"state\", wire = \"getlog\" }}\ninternal = \"the library uploads its own log\"\n\n[[shadow_commands]]\ntopic = \"command_topic\"\nselector = {{ field = \"state\", wire = \"mobile\" }}\nargs = [ {{ field = \"label\", key = \"drive.state.label\", omit_when_absent = true }} ]");
    let gen = generated(
        &dir,
        "commandless",
        "sku_b",
        &[("ns/drive.toml", GO_ROW, &rows)],
    );
    let commands = &read(&gen, "keyspace.sku_b.json")["shadow"]["commands"];
    let find = |wire: &str| -> Value {
        commands
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["selector"]["wire"] == wire)
            .unwrap()
            .clone()
    };
    assert_eq!(find("ping")["answer"], 3);
    assert_eq!(
        find("getlog")["internal"],
        "the library uploads its own log"
    );
    assert_eq!(find("mobile")["args"][0]["key"], "drive.state.label");
    assert!(find("go").get("answer").is_none() && find("go").get("internal").is_none());
    let _ = fs::remove_dir_all(&dir);
}
