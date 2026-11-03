//! Checks against the previous release's contract.json (specification section 8 #15, R-V7,
//! R-V8) and the release-review report for new principals (Q13).

use super::keys::Errors;
use super::snapshot::{KeyJson, Snapshot};
use std::collections::{BTreeMap, BTreeSet};

/// Roles whose writers a release review must see (with MFG keys). TRACE_ENABLE is not one: it
/// is required from the first contract with exactly one writer, previous principals keep their
/// rights in a minor and the role cannot move, so no new principal can ever write it.
const SENSITIVE_ROLES: [&str; 2] = ["RESET_REQUEST", "RESET_PRE_COMMAND"];

/// Compare `cur` with `prev`; refusals go to `errors`, release-review lines are returned.
///
/// @version 1
/// @req REQ-ING-TIV-012
pub fn compare(prev: &Snapshot, cur: &Snapshot, errors: &mut Errors) -> Vec<String> {
    let (p, c) = (version(prev), version(cur));
    if c < p {
        errors.push(format!(
            "contract {}.{} is older than the previous {}.{}",
            c.0, c.1, p.0, p.1
        ));
    }
    reserved_kept(prev, cur, errors);
    if c.0 > p.0 {
        removed_ids_reserved(prev, cur, errors);
        return Vec::new();
    }
    minor_additive(prev, cur, errors);
    review(prev, cur)
}

/// `(major, minor)` of a snapshot.
///
/// @version 1
/// @req REQ-ING-TIV-012
fn version(s: &Snapshot) -> (u16, u16) {
    (s.contract.major, s.contract.minor)
}

/// Every previously reserved ID stays reserved.
///
/// @version 1
/// @req REQ-ING-TIV-012
fn reserved_kept(prev: &Snapshot, cur: &Snapshot, errors: &mut Errors) {
    for id in prev.reserved.iter().filter(|id| !cur.reserved.contains(id)) {
        errors.push(format!("reserved ID {id:#010X} is no longer reserved"));
    }
}

/// In a major bump, every previous ID is either live under the same path or reserved.
///
/// @version 1
/// @req REQ-ING-TIV-012
fn removed_ids_reserved(prev: &Snapshot, cur: &Snapshot, errors: &mut Errors) {
    let live: BTreeMap<u32, &str> = cur.keys.iter().map(|k| (k.id, k.path.as_str())).collect();
    for key in &prev.keys {
        match live.get(&key.id) {
            Some(path) if *path != key.path => errors.push(format!(
                "ID {:#010X} of removed key {} is reused by {path}",
                key.id, key.path
            )),
            Some(_) => {}
            None if !cur.reserved.contains(&key.id) => errors.push(format!(
                "{} was removed but its ID {:#010X} is not reserved",
                key.path, key.id
            )),
            None => {}
        }
    }
}

/// Within a major: keys, principals and rights only grow, at a new minor, and no well-known
/// role moves.
///
/// v2: well-known roles stay on their keys.
/// @version 2
/// @req REQ-ING-TIV-012
fn minor_additive(prev: &Snapshot, cur: &Snapshot, errors: &mut Errors) {
    let cur_keys: BTreeMap<&str, &KeyJson> =
        cur.keys.iter().map(|k| (k.path.as_str(), k)).collect();
    for old in &prev.keys {
        match cur_keys.get(old.path.as_str()) {
            Some(new) => key_unchanged(old, new, errors),
            None => errors.push(format!(
                "{} was removed in a minor (a removal is a major)",
                old.path
            )),
        }
    }
    let old_paths: BTreeSet<&str> = prev.keys.iter().map(|k| k.path.as_str()).collect();
    new_keys_at_this_version(prev, cur, &old_paths, errors);
    principals_unchanged(prev, cur, errors);
    rights_unchanged(prev, cur, &old_paths, errors);
    roles_kept(prev, cur, errors);
}

/// Every well-known role of `prev` is still on the same key: moving one would change what the
/// core reads and writes, which is a major (decided 2026-11-03).
///
/// @version 1
/// @req REQ-ING-TIV-012
fn roles_kept(prev: &Snapshot, cur: &Snapshot, errors: &mut Errors) {
    let roles = |s: &Snapshot| -> BTreeMap<String, String> {
        s.keys
            .iter()
            .filter_map(|k| k.wk.clone().map(|w| (w, k.path.clone())))
            .collect()
    };
    let now = roles(cur);
    for (role, path) in roles(prev) {
        let moved_to = now.get(&role).map_or("no key", String::as_str);
        if moved_to != path {
            errors.push(format!(
                "well-known role {role} moved from {path} to {moved_to} in a minor (a major)"
            ));
        }
    }
}

/// Keys new since `prev` carry this version, and adding any needs a minor bump.
///
/// @version 1
/// @req REQ-ING-TIV-012
fn new_keys_at_this_version(
    prev: &Snapshot,
    cur: &Snapshot,
    old_paths: &BTreeSet<&str>,
    errors: &mut Errors,
) {
    let now = format!("{}.{}", cur.contract.major, cur.contract.minor);
    let added: Vec<&KeyJson> = cur
        .keys
        .iter()
        .filter(|k| !old_paths.contains(k.path.as_str()))
        .collect();
    for key in added.iter().filter(|k| k.introduced_in != now) {
        errors.push(format!(
            "{}: a new key must have introduced_in {now}",
            key.path
        ));
    }
    if !added.is_empty() && prev.contract.minor == cur.contract.minor {
        errors.push("keys were added without a minor bump".to_string());
    }
}

/// An existing key keeps its ID (which encodes its type), version and every attribute a minor
/// may not change.
///
/// v2: no separate type comparison; the type is part of the ID.
/// @version 2
/// @req REQ-ING-TIV-012
fn key_unchanged(old: &KeyJson, new: &KeyJson, errors: &mut Errors) {
    let same = old.id == new.id
        && old.introduced_in == new.introduced_in
        && old.persist == new.persist
        && old.secret == new.secret
        && old.survives_reset == new.survives_reset
        && old.deferred_ms == new.deferred_ms
        && (old.alias_of.is_none() || old.alias_of == new.alias_of)
        && old.command == new.command
        && new.max_size >= old.max_size
        && enum_kept(old, new);
    if !same {
        errors.push(format!(
            "{}: ID, type, introduced_in, persistence, secret, survives-reset, interval, alias, \
             command, enum values or a smaller max_size changed in a minor",
            old.path
        ));
    }
}

/// Every previous enum member keeps its value, and every previous number its wire string.
///
/// v2: wire strings too (a snapshot from before wire strings has none to keep).
/// @version 2
/// @req REQ-ING-TIV-012
/// @req REQ-ING-TIV-016
fn enum_kept(old: &KeyJson, new: &KeyJson) -> bool {
    let values = match (&old.enum_values, &new.enum_values) {
        (None, None) => true,
        (Some(a), Some(b)) => a.iter().all(|(name, v)| b.get(name) == Some(v)),
        _ => false,
    };
    let empty = BTreeMap::new();
    let new_wire = new.enum_wire.as_ref().unwrap_or(&empty);
    let wires = old
        .enum_wire
        .iter()
        .flatten()
        .all(|(code, wire)| new_wire.get(code) == Some(wire));
    values && wires
}

/// Every previous principal is still there with the same uid, token, kind and flags (R-V8).
///
/// @version 1
/// @req REQ-ING-TIV-012
fn principals_unchanged(prev: &Snapshot, cur: &Snapshot, errors: &mut Errors) {
    for old in &prev.principals {
        let kept = cur.principals.iter().any(|new| {
            new.name == old.name
                && (new.uid, new.token, &new.kind, new.flags)
                    == (old.uid, old.token, &old.kind, old.flags)
        });
        if !kept {
            errors.push(format!(
                "principal {} was removed or its uid, token, kind or flags changed in a minor",
                old.name
            ));
        }
    }
}

/// Previous principals keep exactly their rights on previous keys.
///
/// @version 1
/// @req REQ-ING-TIV-012
fn rights_unchanged(
    prev: &Snapshot,
    cur: &Snapshot,
    old_paths: &BTreeSet<&str>,
    errors: &mut Errors,
) {
    let empty = BTreeMap::new();
    for (name, old) in &prev.rights {
        let new = cur.rights.get(name).unwrap_or(&empty);
        let changed = old_paths
            .iter()
            .any(|path| old.get(*path) != new.get(*path));
        if changed {
            errors.push(format!(
                "principal {name}: rights on existing keys changed in a minor"
            ));
        }
    }
}

/// Release-review lines: new principals granted read on an existing secret, or write on an
/// existing MFG or reset key.
///
/// @version 1
/// @req REQ-ING-TIV-012
fn review(prev: &Snapshot, cur: &Snapshot) -> Vec<String> {
    let old: BTreeMap<&str, &KeyJson> = prev.keys.iter().map(|k| (k.path.as_str(), k)).collect();
    let known: BTreeSet<&str> = prev.principals.iter().map(|p| p.name.as_str()).collect();
    let mut lines = Vec::new();
    for (name, cells) in cur
        .rights
        .iter()
        .filter(|(n, _)| !known.contains(n.as_str()))
    {
        for (path, access) in cells {
            let Some(key) = old.get(path.as_str()) else {
                continue;
            };
            if sensitive(key, access) {
                lines.push(format!("new principal {name}: {access} on existing {path}"));
            }
        }
    }
    lines
}

/// Whether `access` on `key` is a grant a release review must acknowledge.
///
/// v2: reset roles only (see SENSITIVE_ROLES).
/// @version 2
/// @req REQ-ING-TIV-012
fn sensitive(key: &KeyJson, access: &str) -> bool {
    let writes = access.contains('w');
    let reset = key
        .wk
        .as_deref()
        .is_some_and(|w| SENSITIVE_ROLES.contains(&w));
    (key.secret && access.contains('r')) || (writes && (reset || key.mfg))
}
