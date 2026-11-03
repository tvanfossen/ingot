//! Declared variants: when contract.toml lists its variants, --variant and every per-variant
//! entry of the namespace files must name one of them.

use super::keys::Errors;
use super::schema::{ClassDef, NamespaceFile};

/// Refuse an undeclared or missing `--variant`. A model that declares no variants accepts any.
///
/// v2: the per-variant entries are checked by check_entries.
/// @version 2
/// @req REQ-ING-TIV-015
pub fn check_selected(declared: &[String], variant: Option<&str>, errors: &mut Errors) {
    if declared.is_empty() {
        return;
    }
    let list = declared.join(", ");
    match variant {
        Some(v) if declared.iter().any(|d| d == v) => {}
        Some(v) => errors.push(format!("--variant {v} is not declared (declared: {list})")),
        None => errors.push(format!(
            "the model declares variants; --variant must name one of: {list}"
        )),
    }
}

/// Refuse any per-variant entry of the namespace files for an undeclared variant.
///
/// @version 1
/// @req REQ-ING-TIV-015
pub fn check_entries(declared: &[String], namespaces: &[NamespaceFile], errors: &mut Errors) {
    if declared.is_empty() {
        return;
    }
    let list = declared.join(", ");
    for (path, v) in uses(namespaces) {
        if !declared.contains(&v) {
            errors.push(format!(
                "{path}: variant {v} is not declared (declared: {list})"
            ));
        }
    }
}

/// Every (where, variant) a namespace file names: per-variant defaults, enum variants and the
/// variants keys and commands are limited to.
///
/// v2: key and command `variants` lists.
/// @version 2
/// @req REQ-ING-TIV-015
/// @req REQ-ING-PRS-001
fn uses(namespaces: &[NamespaceFile]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for ns in namespaces {
        for (name, def) in &ns.enums {
            let path = format!("{}::{name}", ns.meta.id);
            out.extend(def.variants.keys().map(|v| (path.clone(), v.clone())));
        }
        for class in &ns.classes {
            class_uses(&ns.meta.id, class, &mut out);
        }
    }
    out
}

/// The (where, variant) pairs of one class: per-variant defaults and key and command limits.
///
/// @version 1
/// @req REQ-ING-TIV-015
/// @req REQ-ING-PRS-001
fn class_uses(ns: &str, class: &ClassDef, out: &mut Vec<(String, String)>) {
    let path = |id: &str| format!("{ns}.{}.{id}", class.id);
    for key in &class.keys {
        let named = key.defaults.keys().chain(key.variants.iter().flatten());
        out.extend(named.map(|v| (path(&key.id), v.clone())));
    }
    for cmd in &class.commands {
        out.extend(
            cmd.variants
                .iter()
                .flatten()
                .map(|v| (path(&cmd.id), v.clone())),
        );
    }
}
