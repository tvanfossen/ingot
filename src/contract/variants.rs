//! Declared variants: when contract.toml lists its variants, --variant and every per-variant
//! entry of the namespace files must name one of them.

use super::keys::Errors;
use super::schema::NamespaceFile;

/// Refuse an undeclared or missing `--variant` and any per-variant entry for an undeclared
/// variant. A model that declares no variants accepts any.
///
/// @version 1
/// @req REQ-ING-TIV-015
pub fn check(
    declared: &[String],
    variant: Option<&str>,
    namespaces: &[NamespaceFile],
    errors: &mut Errors,
) {
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
    for (path, v) in uses(namespaces) {
        if !declared.contains(&v) {
            errors.push(format!(
                "{path}: variant {v} is not declared (declared: {list})"
            ));
        }
    }
}

/// Every (where, variant) a namespace file names: per-variant defaults and enum variants.
///
/// @version 1
/// @req REQ-ING-TIV-015
fn uses(namespaces: &[NamespaceFile]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for ns in namespaces {
        for (name, def) in &ns.enums {
            let path = format!("{}::{name}", ns.meta.id);
            out.extend(def.variants.keys().map(|v| (path.clone(), v.clone())));
        }
        for class in &ns.classes {
            for key in &class.keys {
                let path = format!("{}.{}.{}", ns.meta.id, class.id, key.id);
                out.extend(key.defaults.keys().map(|v| (path.clone(), v.clone())));
            }
        }
    }
    out
}
