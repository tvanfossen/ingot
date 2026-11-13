//! The per-principal SDKs through a binding: every principal's SDK, rendered with the neutral
//! test binding (tests/fixtures/sdk_binding), compiled against its test store, the generic
//! model's SDKs driven against a recording store
//! (tests/fixtures/sdk/check_sdk.c), the handshake identity against contract.json, and the
//! accessors each principal's rights give.

mod common;

use common::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// @brief The fixture directory of the SDK tests.
/// @version 1
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sdk")
}

/// @brief Generate the 1.1 model into `gen` with the neutral test binding.
/// @version 1
fn generate_bound(gen: &Path) {
    let binding = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sdk_binding");
    generate_ok("v1_1", gen, &["--sdk-binding", binding.to_str().unwrap()]);
}

/// @brief Run `cc` with `args` as strict C11, asserting success.
/// @version 1
fn cc(args: &[String]) {
    let strict = ["-std=c11", "-Wall", "-Wextra", "-pedantic", "-Werror"];
    let out = Command::new("cc")
        .args(strict)
        .args(args)
        .output()
        .expect("spawn cc");
    assert!(
        out.status.success(),
        "cc {args:?}:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// @brief The include flags and source of principal `p`'s SDK in `gen`.
/// @version 1
fn sdk(gen: &Path, p: &str) -> [String; 2] {
    let dir = gen.join("sdk").join(p);
    [
        format!("-I{}", dir.display()),
        dir.join(format!("sdk_{p}.c")).display().to_string(),
    ]
}

/// @brief The text of principal `p`'s SDK header or source (`ext` "h" or "c").
/// @version 1
fn text(gen: &Path, p: &str, ext: &str) -> String {
    fs::read_to_string(gen.join("sdk").join(p).join(format!("sdk_{p}.{ext}"))).unwrap()
}

/// @brief The include flags of the runtime stand-in and the generated tables in `gen`.
/// @version 1
fn includes(gen: &Path) -> Vec<String> {
    vec![
        format!("-I{}", fixtures().display()),
        format!("-I{}", gen.display()),
    ]
}

#[test]
fn every_principal_s_sdk_compiles() {
    let dir = unique_dir("sdk_build");
    let gen = dir.join("gen");
    generate_bound(&gen);
    for entry in fs::read_dir(gen.join("sdk")).unwrap() {
        let p = entry.unwrap().file_name().to_string_lossy().into_owned();
        let obj = dir.join(format!("{p}.o"));
        let tail = vec!["-c".into(), "-o".into(), obj.display().to_string()];
        cc(&[includes(&gen), sdk(&gen, &p).to_vec(), tail].concat());
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_generic_sdks_drive_the_runtime() {
    let dir = unique_dir("sdk_drive");
    let gen = dir.join("gen");
    generate_bound(&gen);
    let exe = dir.join("check_sdk");
    let mut args = includes(&gen);
    for p in ["inproc", "integrator", "app_cloud", "app_map"] {
        args.extend(sdk(&gen, p));
    }
    for f in ["store_fake.c", "check_sdk.c"] {
        args.push(fixtures().join(f).display().to_string());
    }
    args.extend(["-o".into(), exe.display().to_string()]);
    cc(&args);
    assert!(Command::new(&exe).status().unwrap().success(), "check_sdk");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_identity_is_the_principal_s_digest_and_keyset() {
    let dir = unique_dir("sdk_identity");
    let gen = dir.join("gen");
    generate_bound(&gen);
    for (name, p) in [("integrator", "integrator"), ("app.cloud", "app_cloud")] {
        let digest = digests(&gen, name).last().unwrap().clone();
        let bytes: Vec<String> = (0..64)
            .step_by(2)
            .map(|i| format!("0x{}U", &digest[i..i + 2]))
            .collect();
        let c = text(&gen, p, "c");
        assert!(
            c.contains(&format!(
                "digest[STORE_DIGEST_BYTES] = {{ {} }}",
                bytes.join(", ")
            )),
            "{name}"
        );
        let keys = contract_json(&gen)["rights"][name]
            .as_object()
            .unwrap()
            .len();
        assert!(
            c.contains(&format!("id.keyset_len = {keys}u;")),
            "{name}: {keys} keys"
        );
    }
    let h = text(&gen, "integrator", "h");
    assert!(h.contains("#define SDK_CONTRACT_MINOR         1u"));
    assert!(h.contains(&format!(
        "#define SDK_MODEL_HASH             \"{}\"",
        contract_json(&gen)["model_hash"].as_str().unwrap()
    )));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn accessors_follow_rights_kinds_and_command_parts() {
    let dir = unique_dir("sdk_rights");
    let gen = dir.join("gen");
    generate_bound(&gen);
    let tool = text(&gen, "tool", "h");
    assert!(
        tool.contains("sdk_tool_get_core_diag_corruption") && !tool.contains("_set_"),
        "read only"
    );
    let integrator = text(&gen, "integrator", "h");
    assert!(
        integrator.contains("sdk_integrator_get_drive_command_drive("),
        "a command it reads"
    );
    assert!(
        !integrator.contains("submit_drive_command_drive"),
        "a command it may not submit"
    );
    assert!(
        !integrator.contains("set_drive_settings_gain"),
        "a setting it only reads"
    );
    let inproc = text(&gen, "inproc", "h");
    assert!(inproc.contains("sdk_inproc_submit_drive_command_drive("));
    assert!(
        !inproc.contains("set_drive_command_drive("),
        "a command key has no plain setter"
    );
    let map = text(&gen, "app_map", "h");
    assert!(map.contains("sdk_app_map_set_drive_map_map_data(store_client_t *c, const void *v"));
    assert!(
        !map.contains("get_drive_map_map_data"),
        "struct values are read by subscribing"
    );
    let cloud = text(&gen, "app_cloud", "c");
    assert!(cloud.contains("cfg.transport = (uint32_t)STORE_TRANSPORT_UNIX;"));
    assert!(text(&gen, "mcu", "c")
        .contains("cfg.transport = (uint32_t)STORE_TRANSPORT_UART;\n    cfg.principal_id = 1u;"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_read_only_struct_and_a_write_only_key_have_no_getter() {
    let dir = unique_dir("sdk_kinds");
    let gen = dir.join("gen");
    generate_bound(&gen);
    let integrator = text(&gen, "integrator", "h");
    assert!(
        !integrator.contains("drive.map.map_data"),
        "a struct it only reads"
    );
    let inproc = text(&gen, "inproc", "h");
    assert!(
        !inproc.contains("get_drive_command_drive("),
        "a key it may only write"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn without_a_binding_there_is_no_sdk_and_a_binding_holds_templates_only() {
    let dir = unique_dir("sdk_binding_rules");
    let gen = dir.join("plain");
    generate_ok("v1_1", &gen, &[]);
    assert!(!gen.join("sdk").exists(), "no SDK without a binding");
    let binding = dir.join("binding");
    fs::create_dir_all(binding.join("nested")).unwrap();
    let contract = model("v1_1").join("contract.toml");
    let args = ["--sdk-binding", binding.to_str().unwrap()];
    let (ok, stderr) = generate(&contract, &dir.join("a"), &args);
    assert!(
        !ok && stderr.contains("a binding holds files only"),
        "{stderr}"
    );
    fs::remove_dir_all(binding.join("nested")).unwrap();
    let (ok, stderr) = generate(&contract, &dir.join("b"), &args);
    assert!(
        !ok && stderr.contains("the binding has no templates"),
        "{stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_binding_file_renders_once_per_principal_under_its_name() {
    let dir = unique_dir("sdk_binding_files");
    let binding = dir.join("binding");
    fs::create_dir_all(&binding).unwrap();
    let body = "{{ principal }} {{ kind }} {{ uid }} {{ token }}";
    fs::write(binding.join("PRINCIPAL.txt"), body).unwrap();
    let out = dir.join("out");
    let args = ["--sdk-binding", binding.to_str().unwrap()];
    let (ok, stderr) = generate(&model("v1_1").join("contract.toml"), &out, &args);
    assert!(ok, "{stderr}");
    let text = fs::read_to_string(out.join("sdk/app_cloud/app_cloud.txt")).unwrap();
    assert_eq!(text, "app.cloud unix 2002 1\n");
    let _ = fs::remove_dir_all(&dir);
}
