//! The per-variant bounds dsh_tables.h states for a shadow consumer's buffers (shadow table
//! interface minor 8): rejected-list members, desired path length, published error ids and the
//! largest error id. tests/fixtures/dsh/check_dsh.c cross-checks them against the tables in C.

mod common;

use common::*;
use std::fs;
use std::path::Path;

/// @brief The value of `#define <name> <n>u` in the dsh_tables.h of `gen`.
/// @version 1
fn bound(gen: &Path, name: &str) -> u64 {
    let h = fs::read_to_string(gen.join("dsh_tables.h")).unwrap();
    let rest = h
        .split(&format!("#define {name} "))
        .nth(1)
        .unwrap_or_else(|| panic!("{name} missing from:\n{h}"));
    let digits: String = rest
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().unwrap()
}

/// @brief The four bounds of `gen`, in header order.
/// @version 1
fn bounds(gen: &Path) -> [u64; 4] {
    [
        bound(gen, "DSH_REJECTED_MAX_ITEMS"),
        bound(gen, "DSH_DESIRED_PATH_MAX_BYTES"),
        bound(gen, "DSH_ERROR_IDS_PUBLISHED"),
        bound(gen, "DSH_ERROR_ID_MAX"),
    ]
}

/// @brief Generate the 1.1 model with `edits` for `variant` under `dir/tag`.
/// @version 1
fn generated(dir: &Path, tag: &str, variant: &str, edits: &[(&str, &str, &str)]) -> [u64; 4] {
    let case = dir.join(tag);
    edited_copy("v1_1", &case, edits);
    let out = case.join(variant);
    let (ok, stderr) = generate(&case.join("contract.toml"), &out, &["--variant", variant]);
    assert!(ok, "{tag} ({variant}): {stderr}");
    bounds(&out)
}

#[test]
fn the_generic_model_states_its_bounds_per_variant() {
    let dir = unique_dir("dsh_bounds");
    // no rejected list; "settings.speedLimit" and "settings.planExists" are 19 bytes;
    // product publishes bits 1..31 (bit 0 reserved), dock bits 0..31.
    assert_eq!(generated(&dir, "a", "sku_a", &[]), [0, 20, 63, 63]);
    // sku_b numbers its dock errors from 50: the largest id is e81.
    assert_eq!(generated(&dir, "b", "sku_b", &[]), [0, 20, 63, 81]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_rejected_list_holds_one_member_per_desired_and_echo_path() {
    let dir = unique_dir("dsh_bounds_rejected");
    let at = "[[shadow_constant]]\npath = \"state.connected\"";
    let declared = format!("[shadow_rejected]\npath = \"state.rejected\"\n\n{at}");
    let edits = [("contract.toml", at, declared.as_str())];
    assert_eq!(generated(&dir, "r", "sku_a", &edits)[0], 4);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_longest_desired_or_echo_path_sets_the_path_bound() {
    let dir = unique_dir("dsh_bounds_path");
    let from = "path = \"settings.planExists\"";
    let to = "path = \"settings.planExistsLonger\"";
    let edits = [("contract.toml", from, to)];
    assert_eq!(generated(&dir, "p", "sku_a", &edits)[1], 26);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn reserved_bits_and_publish_max_bit_bound_the_published_ids() {
    let dir = unique_dir("dsh_bounds_errors");
    let product = "reserved_bits = [0]                  # bit 0 is never an error";
    let dock = "wire_bases = { sku_b = 50 }";
    let edits = [
        (
            "contract.toml",
            product,
            "reserved_bits = [0, 2]\npublish_max_bit = 3",
        ),
        (
            "contract.toml",
            dock,
            "wire_bases = { sku_b = 50 }\npublish_max_bit = 7",
        ),
    ];
    // product bits 1 and 3, dock bits 0..7 from 32: ten ids, the largest e39.
    assert_eq!(generated(&dir, "e", "sku_a", &edits)[2..], [10, 39]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_declared_bit_above_the_published_range_still_bounds_the_largest_id() {
    let dir = unique_dir("dsh_bounds_declared");
    let product = "reserved_bits = [0]                  # bit 0 is never an error";
    let dock = "wire_bases = { sku_b = 50 }";
    let edits = [
        (
            "contract.toml",
            product,
            "reserved_bits = [0]\npublish_max_bit = 0",
        ),
        (
            "contract.toml",
            dock,
            "wire_bases = { sku_b = 50 }\nreserved_bits = [0]\npublish_max_bit = 0",
        ),
    ];
    // nothing is published; product's key on bit 1 can still report e1.
    assert_eq!(generated(&dir, "d", "sku_a", &edits)[2..], [0, 1]);
    let _ = fs::remove_dir_all(&dir);
}
