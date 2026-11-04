//! Refusals of the shadow wire mapping of keys and rows (G2 section 8: S1, S3, S4, S5).

use super::enums::ResolvedEnum;
use super::keys::{Errors, Key};
use super::model::Tables;
use super::schema::{EchoType, KeyType, Role};
use super::value::encode_default;
use std::collections::BTreeSet;

/// Every key and row refusal of the mapping.
///
/// @version 1
/// @req REQ-ING-SHD-001
/// @req REQ-ING-SHD-002
pub fn check(tables: &Tables, errors: &mut Errors) {
    for key in &tables.keys {
        placement(key, errors);
        enum_rules(key, errors);
        group_rules(tables, key, errors);
        applies_rule(tables, key, errors);
        twin_rule(tables, key, errors);
    }
    telem_names(tables, errors);
    rows(tables, errors);
}

/// Whether the key has a shadow path and the given role.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn shadowed(key: &Key, role: Role) -> bool {
    key.role == role && !key.wire.shadow.is_empty()
}

/// S1: shadow paths on SETTINGS and STATE keys of a JSON-valued type, each listed once.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn placement(key: &Key, errors: &mut Errors) {
    let w = &key.wire;
    let mut refuse = |why: &str| errors.push(format!("{}: {why}", key.path));
    let mapped = !w.shadow.is_empty();
    if mapped && !matches!(key.role, Role::Settings | Role::State) {
        refuse("a shadow path is for SETTINGS and STATE keys");
    }
    if mapped && matches!(key.ty, KeyType::Binary | KeyType::Struct) {
        refuse("a binary or struct key has no shadow value");
    }
    if w.shadow.iter().collect::<BTreeSet<_>>().len() != w.shadow.len() {
        refuse("a shadow path is listed twice");
    }
    attribute_placement(key, errors);
}

/// S1: forced and render on shadowed STATE keys (render on a string, with items), on_reject on
/// shadowed SETTINGS keys.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn attribute_placement(key: &Key, errors: &mut Errors) {
    let w = &key.wire;
    let mut refuse = |why: &str| errors.push(format!("{}: {why}", key.path));
    if w.forced && !shadowed(key, Role::State) {
        refuse("forced is for shadowed STATE keys");
    }
    let render_ok = shadowed(key, Role::State) && key.ty == KeyType::String;
    if w.render
        .as_ref()
        .is_some_and(|r| !render_ok || r.max_items == 0)
    {
        refuse("render is for shadowed STATE string keys, with max_items > 0");
    }
    if w.on_reject.is_some() && !shadowed(key, Role::Settings) {
        refuse("on_reject is for shadowed SETTINGS keys");
    }
}

/// The names of an enum's members whose wire string was not declared.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn implicit_wires(e: &ResolvedEnum) -> Vec<&String> {
    e.iter()
        .filter(|(_, m)| !m.explicit)
        .map(|(n, _)| n)
        .collect()
}

/// S3 and S4: a shadowed key's enum (and a rendered list's) has explicit wire strings; a
/// shadowed SETTINGS enum key declares inbound_unknown and a shadowed STATE enum key
/// outbound_unknown, and nothing else declares either.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn enum_rules(key: &Key, errors: &mut Errors) {
    let enums = [&key.enum_values, &key.wire.render_values];
    for name in enums
        .iter()
        .filter(|_| !key.wire.shadow.is_empty())
        .filter_map(|e| e.as_ref())
        .flat_map(implicit_wires)
    {
        errors.push(format!(
            "{}: enum value {name} needs an explicit wire string on a shadowed key",
            key.path
        ));
    }
    let e = key.enum_values.as_ref();
    let inbound = e.filter(|_| shadowed(key, Role::Settings));
    unknown_rule(
        key,
        "inbound_unknown",
        &key.wire.inbound_unknown,
        inbound,
        errors,
    );
    let outbound = e.filter(|_| shadowed(key, Role::State));
    unknown_rule(
        key,
        "outbound_unknown",
        &key.wire.outbound_unknown,
        outbound,
        errors,
    );
}

/// S4 for one direction: `declared` is required exactly where `applies_to` is the key's enum,
/// and is `reject` (inbound) or `raw` (outbound), or a string of the enum that way.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn unknown_rule(
    key: &Key,
    attr: &str,
    declared: &Option<String>,
    applies_to: Option<&ResolvedEnum>,
    errors: &mut Errors,
) {
    let inbound = attr == "inbound_unknown";
    let why = match (applies_to, declared.as_deref()) {
        (None, Some(_)) => Some(format!(
            "{attr} is for shadowed {} enum keys",
            role_of(inbound)
        )),
        (Some(_), None) => Some(format!("a shadowed enum key declares {attr}")),
        (Some(_), Some("index")) => Some(format!(
            "{attr} index (an unchecked read) must be resolved before generation"
        )),
        (Some(e), Some(v)) if !known_unknown(e, v, inbound) => Some(format!(
            "{attr} {v} is neither {} nor {} of the enum",
            if inbound { "reject" } else { "raw" },
            if inbound {
                "an inbound string"
            } else {
                "a wire string"
            }
        )),
        _ => None,
    };
    if let Some(why) = why {
        errors.push(format!("{}: {why}", key.path));
    }
}

/// The role an unknown rule is for.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn role_of(inbound: bool) -> &'static str {
    if inbound {
        "SETTINGS"
    } else {
        "STATE"
    }
}

/// Whether `v` is an accepted unknown rule: `reject` or an inbound string, or `raw` or a wire
/// string.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn known_unknown(e: &ResolvedEnum, v: &str, inbound: bool) -> bool {
    if inbound {
        v == "reject" || e.values().any(|m| m.inbound().any(|s| s == v))
    } else {
        v == "raw" || e.values().any(|m| m.wire == v)
    }
}

/// S5: apply groups on SETTINGS keys, declared; implies writes of other keys of the same
/// group, each value valid for its key (only a SETTINGS key may join a group, so the target is
/// one).
///
/// v2: no separate role check of the target (a STATE key in a group is refused already).
/// @version 2
/// @req REQ-ING-SHD-002
fn group_rules(tables: &Tables, key: &Key, errors: &mut Errors) {
    let w = &key.wire;
    let mut refuse = |why: String| errors.push(format!("{}: {why}", key.path));
    if let Some(g) = &w.apply_group {
        if key.role != Role::Settings {
            refuse("apply_group is for SETTINGS keys".into());
        }
        if !tables.shadow.apply_groups.iter().any(|d| &d.id == g) {
            refuse(format!("apply group {g} is not declared"));
        }
    }
    for imp in &w.implies {
        let target = tables.key_index(&imp.key).map(|i| &tables.keys[i]);
        let same = target.filter(|t| {
            t.path != key.path && w.apply_group.is_some() && t.wire.apply_group == w.apply_group
        });
        match same {
            None => refuse(format!(
                "implies {} is outside the key's apply group",
                imp.key
            )),
            Some(t) => {
                let enc = encode_default(t.ty, &imp.value, t.max_size, t.enum_values.as_ref());
                if let Err(e) = enc {
                    refuse(format!("implies {}: {e}", imp.key));
                }
            }
        }
    }
}

/// S1: `applies` is on a STATE key and names a SETTINGS key of the same type and enum.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn applies_rule(tables: &Tables, key: &Key, errors: &mut Errors) {
    let Some(target) = &key.wire.applies else {
        return;
    };
    let ok = key.role == Role::State
        && tables.key_index(target).is_some_and(|i| {
            let t = &tables.keys[i];
            t.role == Role::Settings && t.ty == key.ty && t.enum_name == key.enum_name
        });
    if !ok {
        errors.push(format!(
            "{}: applies must be on a STATE key and name a SETTINGS key of the same type and enum",
            key.path
        ));
    }
}

/// S1 (D5): a shadowed SETTINGS key has exactly one reported STATE twin that applies it.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn twin_rule(tables: &Tables, key: &Key, errors: &mut Errors) {
    if !shadowed(key, Role::Settings) {
        return;
    }
    let twins = tables
        .keys
        .iter()
        .filter(|t| shadowed(t, Role::State) && t.wire.applies.as_deref() == Some(&key.path))
        .count();
    if twins != 1 {
        errors.push(format!(
            "{}: a shadowed SETTINGS key needs one reported STATE twin that applies it (has {twins})",
            key.path
        ));
    }
}

/// Telemetry names are non-empty and unique.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn telem_names(tables: &Tables, errors: &mut Errors) {
    let mut seen = BTreeSet::new();
    for key in &tables.keys {
        if let Some(t) = &key.wire.telem {
            if t.is_empty() || !seen.insert(t) {
                errors.push(format!(
                    "{}: telem name '{t}' is empty or used twice",
                    key.path
                ));
            }
        }
    }
}

/// S1 and S5 of the rows: apply group ids unique; an echo string has a max_size and only it;
/// an echo's command key is a SETTINGS key; a constant has a scalar value or an image.
///
/// @version 1
/// @req REQ-ING-SHD-002
fn rows(tables: &Tables, errors: &mut Errors) {
    let s = &tables.shadow;
    let mut ids = BTreeSet::new();
    for g in s.apply_groups.iter().filter(|g| !ids.insert(&g.id)) {
        errors.push(format!("apply group {} is declared twice", g.id));
    }
    echo_rows(tables, errors);
    constant_rows(tables, errors);
}

/// An echo string has a max_size above 0 and only a string has one; a command key is a
/// SETTINGS key.
///
/// @version 1
/// @req REQ-ING-SHD-002
fn echo_rows(tables: &Tables, errors: &mut Errors) {
    let settings = |p: &String| {
        tables
            .key_index(p)
            .is_some_and(|i| tables.keys[i].role == Role::Settings)
    };
    for e in &tables.shadow.echo {
        if (e.ty == EchoType::String) != e.max_size.is_some() || e.max_size == Some(0) {
            errors.push(format!(
                "shadow_echo {}: a string echo needs max_size > 0, and only it has one",
                e.path
            ));
        }
        if e.command_key.as_ref().is_some_and(|p| !settings(p)) {
            errors.push(format!(
                "shadow_echo {}: command_key must name a SETTINGS key",
                e.path
            ));
        }
    }
}

/// A constant has a bool, number or string value, or an image, not both.
///
/// @version 1
/// @req REQ-ING-SHD-002
fn constant_rows(tables: &Tables, errors: &mut Errors) {
    for c in &tables.shadow.constants {
        let scalar = c.value.as_ref().map(|v| {
            matches!(
                v,
                toml::Value::Boolean(_)
                    | toml::Value::Integer(_)
                    | toml::Value::Float(_)
                    | toml::Value::String(_)
            )
        });
        if scalar.is_some() == c.image.is_some() || scalar == Some(false) {
            errors.push(format!(
                "shadow_constant {}: declares a bool, number or string value, or an image",
                c.path
            ));
        }
    }
}
