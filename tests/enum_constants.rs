//! The per-model constants header `dmt_constants.h`: one `DMT_E_<KEY PATH>_<VALUE>` define per
//! value of every enum key, as the variant resolves its enum, compiled as C.

mod common;

use common::*;
use std::fs;
use std::path::Path;
use std::process::Command;

/// @brief The `#define` lines of dmt_constants.h in `gen`, without the guard.
/// @version 1
fn defines(gen: &Path) -> Vec<String> {
    let h = fs::read_to_string(gen.join("dmt_constants.h")).unwrap();
    h.lines()
        .filter(|l| l.starts_with("#define DMT_E_"))
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

/// @brief Compile a file that includes dmt_constants.h in `gen` and uses one constant, as strict
/// C11 and as C++.
/// @version 1
fn compiles(gen: &Path) {
    let header = gen.join("use_constants.c");
    let body =
        "#include \"dmt_constants.h\"\nint main(void) { return DMT_E_DRIVE_SETTINGS_MODE_OFF; }\n";
    fs::write(&header, body).unwrap();
    for (cc, lang) in [("cc", "c"), ("c++", "c++")] {
        let out = Command::new(cc)
            .args([
                "-fsyntax-only",
                "-Wall",
                "-Wextra",
                "-pedantic",
                "-Werror",
                "-x",
                lang,
            ])
            .arg(&header)
            .output()
            .expect("spawn compiler");
        assert!(
            out.status.success(),
            "{cc}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn every_enum_key_value_is_a_named_constant_of_its_variant() {
    let dir = unique_dir("enum_constants");
    let (a, b) = (dir.join("sku_a"), dir.join("sku_b"));
    generate_ok("v1_1", &a, &["--variant", "sku_a"]);
    generate_ok("v1_1", &b, &["--variant", "sku_b"]);
    let on_a = defines(&a);
    for d in [
        "#define DMT_E_DRIVE_SETTINGS_MODE_OFF 0",
        "#define DMT_E_DRIVE_SETTINGS_MODE_ECO 1",
        "#define DMT_E_DRIVE_SETTINGS_MODE_SPORT 2",
        "#define DMT_E_DRIVE_SETTINGS_FAN_BOOST 2",
        "#define DMT_E_DRIVE_STATE_FAN_APPLIED_BOOST 2",
    ] {
        assert!(on_a.contains(&d.to_string()), "{d} in {on_a:#?}");
    }
    let on_b = defines(&b);
    assert!(on_b.contains(&"#define DMT_E_DRIVE_SETTINGS_MODE_SAVER 1".to_string()));
    assert!(on_b.contains(&"#define DMT_E_DRIVE_SETTINGS_MODE_TURBO 3".to_string()));
    assert!(!on_b.iter().any(|d| d.contains("MODE_SPORT")), "{on_b:#?}");
    let mut sorted = on_a.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), on_a.len(), "each constant once");
    compiles(&a);
    compiles(&b);
    let _ = fs::remove_dir_all(&dir);
}
