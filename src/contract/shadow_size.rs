//! The shadow path trees (S1) and the worst-case size of the full reported document (G2 3.2,
//! Z1): every reported row at its longest value text, as compact JSON under
//! `{"state":{"reported":...}}`.

use super::keys::Key;
use super::model::Tables;
use super::schema::{Constant, Echo, EchoType, Image, KeyType, PathForm, Role};
use super::value::int_range;
use std::collections::{BTreeMap, BTreeSet};

/// The AWS IoT Core shadow document quota ("Each individual shadow document must be 8KB or less").
pub const SHADOW_QUOTA: usize = 8192;

/// The longest printf text of a float32: `%f` of `-FLT_MAX` is 47 characters.
const FLOAT_TEXT: usize = 47;

/// A node of a path tree: a value of a known worst-case length, or an object.
enum Node {
    Leaf(usize),
    Tree(BTreeMap<String, Node>),
}

/// The paths of one document side, who declared each, and the conflicts found.
#[derive(Default)]
pub struct PathTree {
    root: BTreeMap<String, Node>,
    owners: BTreeMap<String, String>,
    pub errors: Vec<String>,
}

/// Whether `path` is dotted identifiers (`[A-Za-z_][A-Za-z0-9_]*`).
///
/// @version 1
/// @req REQ-ING-SHD-001
pub fn valid_path(path: &str) -> bool {
    path.split('.').all(|seg| {
        let mut chars = seg.chars();
        chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// Insert a leaf at `segs` under `map`; false when a segment is already a value or the leaf's
/// place is already an object.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn insert(map: &mut BTreeMap<String, Node>, segs: &[&str], size: usize) -> bool {
    let (first, rest) = (segs[0].to_string(), &segs[1..]);
    if rest.is_empty() {
        return map.insert(first, Node::Leaf(size)).is_none();
    }
    match map
        .entry(first)
        .or_insert_with(|| Node::Tree(BTreeMap::new()))
    {
        Node::Tree(sub) => insert(sub, rest, size),
        Node::Leaf(_) => false,
    }
}

impl PathTree {
    /// Add `owner`'s row at `path` with a value of at most `size` bytes.
    ///
    /// @version 1
    /// @req REQ-ING-SHD-001
    pub fn add(&mut self, owner: &str, path: &str, size: usize) {
        if !valid_path(path) {
            self.errors
                .push(format!("{owner}: malformed shadow path '{path}'"));
        } else if let Some(other) = self.owners.get(path) {
            self.errors
                .push(format!("{owner}: shadow path {path} is taken by {other}"));
        } else {
            self.owners.insert(path.to_string(), owner.to_string());
            let segs: Vec<&str> = path.split('.').collect();
            if !insert(&mut self.root, &segs, size) {
                self.errors.push(format!(
                    "{owner}: shadow path {path} is both a value and an object"
                ));
            }
        }
    }

    /// Compact JSON bytes of `{"state":{"reported":<tree>}}`.
    ///
    /// @version 1
    /// @req REQ-ING-SIZ-001
    pub fn bytes(&self) -> usize {
        // {"state":{"reported": ... }}: 9 + 12 + 2 closing braces
        9 + 12 + object_bytes(&self.root) + 2
    }
}

/// Compact JSON bytes of an object: braces, `"name":` per member, values and commas.
///
/// @version 1
/// @req REQ-ING-SIZ-001
fn object_bytes(map: &BTreeMap<String, Node>) -> usize {
    let members: usize = map
        .iter()
        .map(|(name, node)| {
            name.len()
                + 3
                + match node {
                    Node::Leaf(n) => *n,
                    Node::Tree(sub) => object_bytes(sub),
                }
        })
        .sum();
    2 + members + map.len().saturating_sub(1)
}

/// Digits (and sign) of the longer extreme of an integer type.
///
/// @version 1
/// @req REQ-ING-SIZ-001
fn int_text(ty: KeyType) -> usize {
    int_range(ty).map_or(0, |(min, max, _)| {
        min.to_string().len().max(max.to_string().len())
    })
}

/// The longest value text of a type: `false`, an extreme integer, the longest float, or a
/// string of `max_size` bytes each escaped to six (`\u00XX`) in quotes.
///
/// @version 1
/// @req REQ-ING-SIZ-001
fn type_text(ty: KeyType, max_size: u32) -> usize {
    match ty {
        KeyType::Bool => 5,
        KeyType::Float32 => FLOAT_TEXT,
        KeyType::String => max_size as usize * 6 + 2,
        _ => int_text(ty),
    }
}

/// The longest quoted wire string of an enum.
///
/// @version 1
/// @req REQ-ING-SIZ-001
fn longest_wire(e: &super::enums::ResolvedEnum) -> usize {
    e.values().map(|m| m.wire.len() + 2).max().unwrap_or(2)
}

/// The longest reported text of a STATE key at a path of form `f`: a rendered list, an enum
/// wire string (or its raw code), or its type's longest value (also an enum's integer form).
///
/// v2: an integer path carries the code.
/// @version 2
/// @req REQ-ING-SIZ-001
/// @req REQ-ING-SHD-003
fn key_text(key: &Key, f: &PathForm) -> usize {
    let raw = key.wire.outbound_unknown.as_deref() == Some("raw");
    let strings = key.enum_values.as_ref().filter(|_| !f.integer());
    match (&key.wire.render, &key.wire.render_values, strings) {
        (Some(r), Some(e), _) => {
            let n = usize::from(r.max_items);
            2 + n * longest_wire(e) + n.saturating_sub(1)
        }
        (_, _, Some(e)) => enum_text(e, raw, key.ty),
        _ => type_text(key.ty, key.max_size),
    }
}

/// The longest reported text of an enum key: its longest quoted wire string, or the key type's
/// longest integer when unknown codes report raw and that is longer.
///
/// @version 1
/// @req REQ-ING-SIZ-001
fn enum_text(e: &super::enums::ResolvedEnum, raw: bool, ty: KeyType) -> usize {
    let wire = longest_wire(e);
    if raw {
        wire.max(int_text(ty))
    } else {
        wire
    }
}

/// The longest text of an echo-only value.
///
/// @version 1
/// @req REQ-ING-SIZ-001
fn echo_text(e: &Echo) -> usize {
    match e.ty {
        EchoType::Bool => 5,
        EchoType::Int => int_text(KeyType::Int32),
        EchoType::Float => FLOAT_TEXT,
        EchoType::String => type_text(KeyType::String, e.max_size.unwrap_or(0)),
    }
}

/// The text of a constant: its JSON value, the quoted `"M.m.p"`, or the quoted 64-hex hash.
///
/// @version 1
/// @req REQ-ING-SIZ-001
fn constant_text(tables: &Tables, c: &Constant) -> usize {
    match (&c.value, c.image) {
        (Some(v), _) => serde_json::to_string(v).map_or(0, |s| s.len()),
        (None, Some(Image::ContractVersion)) => {
            let v = &tables.version;
            format!("{}.{}.{}", v.major, v.minor, tables.patch).len() + 2
        }
        (None, Some(Image::ContractHash)) => 66,
        (None, None) => 0,
    }
}

/// `[{"ec":"e<id>"},...]` with every error of the groups reported at `path`.
///
/// @version 1
/// @req REQ-ING-SIZ-001
fn errors_text(tables: &Tables, path: &str) -> usize {
    let ids: Vec<u32> = tables
        .error_groups
        .iter()
        .filter(|g| g.shadow.as_deref() == Some(path))
        .flat_map(|g| {
            tables.keys.iter().filter_map(move |k| {
                let e = k.error.as_ref().filter(|e| e.group == g.id)?;
                Some(g.wire_base + u32::from(e.bit))
            })
        })
        .collect();
    let elements: usize = ids.iter().map(|id| 10 + id.to_string().len()).sum();
    2 + elements + ids.len().saturating_sub(1)
}

/// The desired tree, the full reported document, the delta's reported rows and the forced
/// delta rows of the tables.
pub struct Trees {
    pub desired: PathTree,
    pub reported: PathTree,
    pub delta: PathTree,
    pub forced: PathTree,
}

/// Every shadow path of the tables in its trees: SETTINGS keys and echo paths desired; STATE
/// keys by scope, echo paths, constants and error groups in the full document; delta-scope
/// keys, echo paths and forced constants in the delta; forced rows in the forced tree too.
///
/// v2: the delta tree, and key rows by scope; the contract-level rows in `row_paths`.
/// @version 2
/// @req REQ-ING-SHD-001
/// @req REQ-ING-SIZ-001
/// @req REQ-ING-SHD-003
pub fn trees(tables: &Tables) -> Trees {
    let mut t = Trees {
        desired: PathTree::default(),
        reported: PathTree::default(),
        delta: PathTree::default(),
        forced: PathTree::default(),
    };
    for key in &tables.keys {
        key_rows(&mut t, key);
    }
    row_paths(&mut t, tables);
    let paths: BTreeSet<&str> = tables
        .error_groups
        .iter()
        .filter_map(|g| g.shadow.as_deref())
        .collect();
    for p in paths {
        t.reported.add("error groups", p, errors_text(tables, p));
    }
    t
}

/// The contract-level rows: echo paths desired, in the full document and in the delta;
/// constants in the full document, forced ones in the delta and the forced tree.
///
/// @version 1
/// @req REQ-ING-SHD-002
/// @req REQ-ING-SHD-003
fn row_paths(t: &mut Trees, tables: &Tables) {
    for e in &tables.shadow.echo {
        t.desired.add("shadow_echo", &e.path, 0);
        t.reported.add("shadow_echo", &e.path, echo_text(e));
        t.delta.add("shadow_echo", &e.path, echo_text(e));
    }
    for c in &tables.shadow.constants {
        let size = constant_text(tables, c);
        t.reported.add("shadow_constant", &c.path, size);
        if c.forced {
            t.delta.add("shadow_constant", &c.path, size);
            t.forced.add("shadow_constant", &c.path, size);
        }
    }
}

/// A key's rows: desired paths of a SETTINGS key; a STATE key's paths in the full document,
/// the delta (and forced) by scope.
///
/// v2: by scope, each at its form's text.
/// @version 2
/// @req REQ-ING-SHD-001
/// @req REQ-ING-SHD-003
fn key_rows(t: &mut Trees, key: &Key) {
    for f in &key.wire.shadow_forms {
        match key.role {
            Role::Settings => t.desired.add(&key.path, &f.path, 0),
            Role::State => state_row(t, key, f),
            _ => {}
        }
    }
}

/// A STATE key's row at `f`: in the full document unless delta-only, in the delta (and the
/// forced tree when forced) unless full-only.
///
/// @version 1
/// @req REQ-ING-SHD-003
fn state_row(t: &mut Trees, key: &Key, f: &PathForm) {
    let size = key_text(key, f);
    if f.in_full() {
        t.reported.add(&key.path, &f.path, size);
    }
    if f.in_delta() {
        t.delta.add(&key.path, &f.path, size);
        if key.wire.forced {
            t.forced.add(&key.path, &f.path, size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::enums::{Member, ResolvedEnum};

    #[test]
    fn a_raw_unknown_code_counts_the_type_s_longest_integer() {
        let m = Member {
            value: 0,
            wire: "a".to_string(),
            accept: Vec::new(),
            explicit: true,
        };
        let e: ResolvedEnum = [("a".to_string(), m)].into();
        assert_eq!(enum_text(&e, false, KeyType::Int32), 3);
        assert_eq!(enum_text(&e, true, KeyType::Int32), 11);
        assert_eq!(enum_text(&e, true, KeyType::Uint8), 3);
    }

    #[test]
    fn longest_value_texts_of_each_type() {
        assert_eq!(type_text(KeyType::Bool, 0), "false".len());
        assert_eq!(type_text(KeyType::Int8, 0), "-128".len());
        assert_eq!(type_text(KeyType::Uint16, 0), "65535".len());
        assert_eq!(type_text(KeyType::Float32, 0), FLOAT_TEXT);
        assert_eq!(type_text(KeyType::String, 3), 3 * 6 + 2);
        let echo = |ty, max_size| Echo {
            path: "a".to_string(),
            ty,
            max_size,
            command_key: None,
        };
        assert_eq!(echo_text(&echo(EchoType::Bool, None)), 5);
        assert_eq!(echo_text(&echo(EchoType::Int, None)), "-2147483648".len());
        assert_eq!(echo_text(&echo(EchoType::Float, None)), FLOAT_TEXT);
        assert_eq!(echo_text(&echo(EchoType::String, Some(2))), 2 * 6 + 2);
    }
    #[test]
    fn paths_are_dotted_identifiers() {
        for ok in ["a", "_a.b1", "state.fanSpeed"] {
            assert!(valid_path(ok), "{ok}");
        }
        for bad in ["", "a.", ".a", "1a", "a-b", "a..b"] {
            assert!(!valid_path(bad), "{bad}");
        }
    }
}
