//! Enum resolution for one variant: base values with their wire strings, then the variant's
//! removals, renames and additions.

use super::keys::Errors;
use super::schema::{EnumDef, EnumValueDef, EnumVariantDef, NamespaceFile};
use std::collections::{BTreeMap, BTreeSet};

/// One member of a resolved enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub value: i64,
    /// The outbound wire string (also accepted inbound).
    pub wire: String,
    /// The other inbound strings that mean this value.
    pub accept: Vec<String>,
    /// Whether the wire string was declared (not defaulted to the name).
    pub explicit: bool,
}

/// A resolved enum: member name to member.
pub type ResolvedEnum = BTreeMap<String, Member>;

/// The member a value definition declares under `name` (its wire string is the name unless given).
///
/// v2: accept aliases, and whether the wire string was declared.
/// @version 2
/// @req REQ-ING-TIV-016
/// @req REQ-ING-SHD-001
fn member(name: &str, def: &EnumValueDef) -> Member {
    match def {
        EnumValueDef::Number(value) => Member {
            value: *value,
            wire: name.to_string(),
            accept: Vec::new(),
            explicit: false,
        },
        EnumValueDef::Wire(w) => Member {
            value: w.value,
            wire: w.wire.clone(),
            accept: w.accept.clone(),
            explicit: true,
        },
    }
}

impl Member {
    /// Every inbound string of the member: its wire string, then its accept aliases.
    ///
    /// @version 1
    /// @req REQ-ING-SHD-001
    pub fn inbound(&self) -> impl Iterator<Item = &String> {
        std::iter::once(&self.wire).chain(&self.accept)
    }
}

/// Every enum of namespace file `ns`, resolved for `variant`, by enum name. Every declared
/// variant is resolved too, so a map that is only wrong for another SKU is still refused.
///
/// @version 1
/// @req REQ-ING-TIV-016
pub fn resolve_all(
    ns: &NamespaceFile,
    declared: &[String],
    variant: Option<&str>,
    errors: &mut Errors,
) -> BTreeMap<String, ResolvedEnum> {
    let mut selected = BTreeMap::new();
    for (name, def) in &ns.enums {
        let base = format!("{}::{name}", ns.meta.id);
        for v in declared
            .iter()
            .map(String::as_str)
            .filter(|v| Some(*v) != variant)
        {
            resolve(&format!("{base} ({v})"), def, Some(v), errors);
        }
        let path = variant.map_or(base.clone(), |v| format!("{base} ({v})"));
        selected.insert(name.clone(), resolve(&path, def, variant, errors));
    }
    selected
}

/// One enum: its base values, then the selected variant's changes; numbers and wire strings
/// unique.
///
/// @version 1
/// @req REQ-ING-TIV-016
fn resolve(path: &str, def: &EnumDef, variant: Option<&str>, errors: &mut Errors) -> ResolvedEnum {
    let mut resolved: ResolvedEnum = def
        .values
        .iter()
        .map(|(name, value)| (name.clone(), member(name, value)))
        .collect();
    if let Some(changes) = variant.and_then(|v| def.variants.get(v)) {
        apply_variant(path, &mut resolved, changes, errors);
    }
    check_unique(path, &resolved, errors);
    resolved
}

/// Apply one variant's changes: remove base names, then for each variant value drop the base
/// member holding its number (a rename) and add it, replacing a base member of the same name
/// (the name moves to a new number). The result does not depend on the order of the values.
///
/// v2: a base name may move to a new number (it was refused when the moved name sorted before
/// the renamed one).
/// @version 2
/// @req REQ-ING-TIV-016
fn apply_variant(
    path: &str,
    resolved: &mut ResolvedEnum,
    changes: &EnumVariantDef,
    errors: &mut Errors,
) {
    for name in &changes.remove {
        if resolved.remove(name).is_none() {
            errors.push(format!(
                "{path}: the variant removes {name}, which is not a base value"
            ));
        }
    }
    let mut added = BTreeSet::new();
    for (name, value) in &changes.values {
        let m = member(name, value);
        resolved.retain(|old, b| added.contains(old) || b.value != m.value);
        resolved.insert(name.clone(), m);
        added.insert(name.clone());
    }
}

/// Numbers and wire strings of a resolved enum are unique, and each inbound string (a wire
/// string or an accept alias) means one code.
///
/// v2: inbound strings (S5).
/// @version 2
/// @req REQ-ING-TIV-016
/// @req REQ-ING-SHD-001
fn check_unique(path: &str, resolved: &ResolvedEnum, errors: &mut Errors) {
    let mut values = BTreeSet::new();
    let mut wires = BTreeSet::new();
    let mut inbound = BTreeMap::new();
    for m in resolved.values() {
        if !values.insert(m.value) {
            errors.push(format!("{path}: number {} has two names", m.value));
        }
        if !wires.insert(&m.wire) {
            errors.push(format!("{path}: wire string {} is used twice", m.wire));
        }
        for s in &m.accept {
            if *inbound.entry(s).or_insert(m.value) != m.value {
                errors.push(format!("{path}: inbound string {s} maps to two codes"));
            }
        }
    }
    for m in resolved.values() {
        if inbound.get(&m.wire).is_some_and(|v| *v != m.value) {
            errors.push(format!(
                "{path}: inbound string {} maps to two codes",
                m.wire
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::schema::WireValue;

    /// @brief An enum definition from base (name, number) pairs and one variant's changes.
    /// @version 1
    fn def(base: &[(&str, i64)], values: &[(&str, i64)], remove: &[&str]) -> EnumDef {
        let map = |pairs: &[(&str, i64)]| {
            pairs
                .iter()
                .map(|(n, v)| (n.to_string(), EnumValueDef::Number(*v)))
                .collect()
        };
        let variant = EnumVariantDef {
            values: map(values),
            remove: remove.iter().map(|s| s.to_string()).collect(),
        };
        EnumDef {
            _doc: None,
            values: map(base),
            variants: BTreeMap::from([("v".to_string(), variant)]),
        }
    }

    /// @brief (number, name, wire) of every member, by number.
    /// @version 1
    fn members(e: &ResolvedEnum) -> Vec<(i64, String, String)> {
        let mut out: Vec<_> = e
            .iter()
            .map(|(n, m)| (m.value, n.clone(), m.wire.clone()))
            .collect();
        out.sort();
        out
    }

    #[test]
    fn a_variant_adds_renames_and_removes() {
        let d = def(
            &[("off", 0), ("unknown", 19), ("gone", 7)],
            &[("adding_water", 19), ("hair", 20)],
            &["gone"],
        );
        let mut errors = Errors::new();
        let e = resolve("ns::state", &d, Some("v"), &mut errors);
        assert!(errors.is_empty(), "{errors:?}");
        let row = |v: i64, n: &str| (v, n.to_string(), n.to_string());
        assert_eq!(
            members(&e),
            [row(0, "off"), row(19, "adding_water"), row(20, "hair")]
        );
        let base = resolve("ns::state", &d, None, &mut errors);
        assert_eq!(
            members(&base),
            [row(0, "off"), row(7, "gone"), row(19, "unknown")]
        );
        let other = resolve("ns::state", &d, Some("w"), &mut errors);
        assert_eq!(
            members(&other),
            members(&base),
            "a variant without changes sees the base"
        );
    }

    #[test]
    fn wire_strings_default_to_the_name_and_can_be_given() {
        let mut d = def(&[("off", 0)], &[], &[]);
        let wire = WireValue {
            value: 14,
            wire: "mapDiscovery".to_string(),
            accept: Vec::new(),
        };
        d.values
            .insert("map_discovery".to_string(), EnumValueDef::Wire(wire));
        let e = resolve("ns::state", &d, None, &mut Errors::new());
        assert_eq!(e["off"].wire, "off");
        assert_eq!(e["map_discovery"].wire, "mapDiscovery");
    }

    #[test]
    fn a_base_name_moves_to_a_new_number_in_any_order() {
        // B11 RGB: 19 is renamed and the base name "unknown" moves from 19 to 22; the result
        // must not depend on the order the variant's names sort in
        for (renamed, moved) in [
            ("adding_water", "unknown"),
            ("zz_water", "unknown"),
            ("water", "aa_unknown"),
        ] {
            let d = def(
                &[(moved, 19), ("off", 0)],
                &[(renamed, 19), (moved, 22)],
                &[],
            );
            let mut errors = Errors::new();
            let e = resolve("ns::state", &d, Some("v"), &mut errors);
            assert!(errors.is_empty(), "{renamed}/{moved}: {errors:?}");
            let row = |v: i64, n: &str| (v, n.to_string(), n.to_string());
            let mut want = vec![row(0, "off"), row(19, renamed), row(22, moved)];
            want.sort();
            assert_eq!(members(&e), want, "{renamed}/{moved}");
        }
    }
    #[test]
    fn conflicting_maps_are_refused() {
        let cases: [(EnumDef, &str); 5] = [
            (
                def(&[("a", 0)], &[], &["b"]),
                "removes b, which is not a base value",
            ),
            (def(&[("a", 0)], &[("a", 1)], &[]), ""),
            (
                def(&[("a", 0)], &[("x", 3), ("y", 3)], &[]),
                "number 3 has two names",
            ),
            (
                def(&[("a", 0), ("b", 0)], &[], &[]),
                "number 0 has two names",
            ),
            (def(&[("a", 0)], &[("b", 1)], &[]), ""),
        ];
        for (i, (d, expected)) in cases.iter().enumerate() {
            let mut errors = Errors::new();
            resolve("ns::e", d, Some("v"), &mut errors);
            if expected.is_empty() {
                assert!(errors.is_empty(), "case {i}: {errors:?}");
            } else {
                assert!(
                    errors.iter().any(|e| e.contains(expected)),
                    "case {i}: {errors:?}"
                );
            }
        }
        let mut d = def(&[("a", 0)], &[], &[]);
        let wire = WireValue {
            value: 1,
            wire: "a".to_string(),
            accept: Vec::new(),
        };
        d.values.insert("b".to_string(), EnumValueDef::Wire(wire));
        let mut errors = Errors::new();
        resolve("ns::e", &d, None, &mut errors);
        assert!(errors
            .iter()
            .any(|e| e.contains("wire string a is used twice")));
    }
}
