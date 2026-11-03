//! The keyspace files of contract mode (G2 section 2): the resolved body, its gzip and head,
//! the all-variants file with overrides, the schema's trust rule, determinism and size.

mod common;

use common::*;
use std::fs;
use std::io::Read;
use std::path::Path;

/// @brief The file `name` in `gen`.
/// @version 1
fn file(gen: &Path, name: &str) -> Vec<u8> {
    fs::read(gen.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// @brief The JSON file `name` in `gen`.
/// @version 1
fn json(gen: &Path, name: &str) -> serde_json::Value {
    serde_json::from_slice(&file(gen, name)).unwrap()
}

#[test]
fn the_head_names_the_body_and_the_gzip_is_the_body() {
    use sha2::{Digest, Sha256};
    let dir = unique_dir("ks_head");
    let gen = dir.join("gen");
    generate_ok("v1_1", &gen, &["--variant", "sku_b"]);
    let body = file(&gen, "keyspace.sku_b.json");
    let head = json(&gen, "keyspace.sku_b.head.json");
    let hash: String = Sha256::digest(&body)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(head["hash"], hash);
    assert_eq!(head["size"], body.len());
    assert_eq!(head["gz_size"], file(&gen, "keyspace.sku_b.json.gz").len());
    assert_eq!(
        (head["sku"].clone(), head["minor"].clone()),
        ("sku_b".into(), 1.into())
    );
    let mut unzipped = Vec::new();
    let gz = file(&gen, "keyspace.sku_b.json.gz");
    flate2::read::GzDecoder::new(&gz[..])
        .read_to_end(&mut unzipped)
        .unwrap();
    assert_eq!(unzipped, body);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_resolved_body_declares_its_keys() {
    let dir = unique_dir("ks_body");
    let gen = dir.join("gen");
    generate_ok("v1_1", &gen, &["--variant", "sku_b"]);
    let body = json(&gen, "keyspace.sku_b.json");
    assert_eq!(body["schema_version"], "1.0");
    assert_eq!(body["contract"]["minor"], 1);
    let drive = body["namespaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == "drive")
        .unwrap();
    let settings = &drive["classes"][0];
    assert_eq!(
        (settings["index"].clone(), settings["role"].clone()),
        (0.into(), "settings".into())
    );
    let limit = &settings["keys"][0];
    assert_eq!(limit["name"], "drive.settings.speed_limit");
    assert_eq!(
        limit["range"],
        serde_json::json!({"min": 0, "max": 2000, "step": 10})
    );
    assert_eq!(
        (limit["unit"].clone(), limit["present"].clone()),
        ("mm/s".into(), true.into())
    );
    let _ = fs::remove_dir_all(&dir);
}

/// @brief Whether some element of the array `v` satisfies `pred`.
/// @version 1
fn any(v: &serde_json::Value, pred: impl Fn(&serde_json::Value) -> bool) -> bool {
    v.as_array().unwrap().iter().any(pred)
}

#[test]
fn the_resolved_body_declares_enums_commands_rights_and_no_values() {
    let dir = unique_dir("ks_body2");
    let gen = dir.join("gen");
    generate_ok("v1_1", &gen, &["--variant", "sku_b"]);
    let body = json(&gen, "keyspace.sku_b.json");
    let codes: Vec<&serde_json::Value> = body["enums"][0]["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| &v["code"])
        .collect();
    assert_eq!(codes, [0, 1, 3], "sku_b's resolved mode enum");
    assert!(any(&body["commands"], |c| c["name"]
        == "core.command.factory_reset"
        && c["owner"] == "core"));
    let net = body["rights"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["principal"] == "iotd.net")
        .unwrap();
    assert!(any(&net["keys"], |k| k["key"]
        == "net.credentials.wifi_psk"
        && k["access"] == "rw"));
    let text = String::from_utf8(file(&gen, "keyspace.sku_b.json")).unwrap();
    assert!(!text.contains("\"default\""), "no values in the keyspace");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_all_variants_file_overrides_the_base_per_variant() {
    let dir = unique_dir("ks_all");
    let gen = dir.join("gen");
    generate_ok("v1_0", &gen, &[]);
    let all = json(&gen, "keyspace.json");
    let mode = &all["enums"].as_array().unwrap()[0];
    assert_eq!(mode["name"], "drive::mode");
    assert_eq!(
        mode["values"][2],
        serde_json::json!({"code": 2, "name": "sport", "wire": "sportMode"})
    );
    assert_eq!(
        mode["overrides"],
        serde_json::json!({"sku_b": {"1": {"name": "saver", "wire": "saver"}, "2": {"present": false},
                                     "3": {"name": "turbo", "wire": "turbo"}}})
    );
    let key = &all["namespaces"][0]["classes"][0]["keys"][0];
    assert_eq!(
        key["present"],
        serde_json::json!({"sku_a": true, "sku_b": true})
    );
    let _ = fs::remove_dir_all(&dir);
}

/// @brief Every object property name in `v`, recursively.
/// @version 1
fn property_names(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, x) in m {
                out.push(k.clone());
                property_names(x, out);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| property_names(x, out)),
        _ => {}
    }
}

#[test]
fn no_property_names_a_ui_layout_or_endpoint() {
    let dir = unique_dir("ks_trust");
    let gen = dir.join("gen");
    generate_ok("v1_1", &gen, &[]);
    let schema = json(&gen, "keyspace.schema.json");
    let pattern = schema["$defs"]["trusted"]["propertyNames"]["not"]["pattern"]
        .as_str()
        .unwrap();
    let re = regex::Regex::new(pattern).unwrap();
    for ok in ["required", "uid", "build", "guid", "quiet", "unit"] {
        assert!(!re.is_match(ok), "{ok} must be allowed");
    }
    for bad in [
        "ui_hint", "label", "icon_url", "color", "colour", "widget", "endpoint",
    ] {
        assert!(re.is_match(bad), "{bad} must be refused");
    }
    for name in ["keyspace.json", "keyspace.sku_a.json"] {
        let mut names = Vec::new();
        property_names(&json(&gen, name), &mut names);
        let found: Vec<&String> = names.iter().filter(|n| re.is_match(n)).collect();
        assert!(found.is_empty(), "{name}: {found:?}");
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn keyspace_files_are_deterministic_and_small() {
    let dir = unique_dir("ks_det");
    let (a, b) = (dir.join("a"), dir.join("b"));
    generate_ok("v2_0", &a, &[]);
    generate_ok("v2_0", &b, &[]);
    for name in [
        "keyspace.json",
        "keyspace.sku_a.json",
        "keyspace.sku_a.json.gz",
        "keyspace.sku_a.head.json",
        "keyspace.schema.json",
    ] {
        assert_eq!(file(&a, name), file(&b, name), "{name}");
    }
    assert!(
        file(&a, "keyspace.sku_a.json.gz").len() < 64 * 1024,
        "per-variant image budget"
    );
    let _ = fs::remove_dir_all(&dir);
}
