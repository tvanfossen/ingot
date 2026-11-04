//! `contract.json`: the machine-readable contract of one generated table set. It is read back
//! as `--previous-contract` and carries the CI inputs (rates, persistence, sizes, bounds).

use super::bounds::queue_bound;
use super::digest::digests;
use super::enums::ResolvedEnum;
use super::error_groups::ErrorGroup;
use super::model::hex;
use super::model::Tables;
use super::ranges::Range;
use super::schema::{ErrorRef, Kind, UnitDef};
use super::types::WK_RULES;
use super::wire::KeyWire;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The whole contract of one table set.
#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub tiv: String,
    pub contract: ContractJson,
    pub variant: String,
    /// The declared variants; absent in snapshots from before they existed.
    #[serde(default)]
    pub variants: Vec<String>,
    /// The unit vocabulary; absent in snapshots from before it existed.
    #[serde(default)]
    pub units: BTreeMap<String, UnitDef>,
    /// The error groups as this variant resolves them; absent in older snapshots.
    #[serde(default)]
    pub error_groups: Vec<ErrorGroup>,
    pub model_hash: String,
    pub ingot: IngotJson,
    pub keys: Vec<KeyJson>,
    pub reserved: Vec<u32>,
    pub principals: Vec<PrincipalJson>,
    /// principal -> key path -> "r" | "w" | "rw" (absent: none)
    pub rights: BTreeMap<String, BTreeMap<String, String>>,
    pub commands: Vec<CommandJson>,
    /// The shadow wire mapping's rows and sizes (G2 1.4); absent when the model maps nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<serde_json::Value>,
}

/// Contract name and version.
#[derive(Debug, Serialize, Deserialize)]
pub struct ContractJson {
    pub name: String,
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

/// The ingot build that generated the set.
#[derive(Debug, Serialize, Deserialize)]
pub struct IngotJson {
    pub version: String,
    pub commit: String,
}

/// One key.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct KeyJson {
    pub path: String,
    pub id: u32,
    #[serde(rename = "type")]
    pub ty: String,
    pub introduced_in: String,
    pub persist: String,
    pub deferred_ms: u32,
    pub rate_per_day: u32,
    pub rate_burst: u32,
    pub max_size: u32,
    pub secret: bool,
    pub survives_reset: bool,
    pub mfg: bool,
    pub alias_of: Option<String>,
    #[serde(rename = "enum")]
    pub enum_values: Option<BTreeMap<String, i64>>,
    /// Number to wire string of the resolved enum; absent in snapshots from before wire strings.
    #[serde(default)]
    pub enum_wire: Option<BTreeMap<i64, String>>,
    pub command: Option<String>,
    pub wk: Option<String>,
    pub doc: Option<String>,
    pub unit: Option<String>,
    /// Range, allowed set, sentinel default, scale and error bit; absent in older snapshots.
    #[serde(flatten)]
    pub limits: Limits,
}

/// The shadow wire mapping of a key in contract.json (G2 1.2.1, 1.4).
#[derive(Debug, Serialize, PartialEq, Default)]
pub struct Mapping {
    /// Number to the accept aliases of the values that have any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_accept: Option<BTreeMap<i64, Vec<String>>>,
    #[serde(flatten)]
    pub wire: KeyWire,
}

impl Mapping {
    /// The mapping of key `k`.
    ///
    /// @version 1
    /// @req REQ-ING-SHD-001
    fn of(k: &super::keys::Key) -> Self {
        Mapping {
            enum_accept: k.enum_values.as_ref().and_then(enum_accepts),
            wire: k.wire.clone(),
        }
    }
}

/// The G2 limits and shadow wire mapping of a key in contract.json (written flat, beside the
/// other key fields).
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Limits {
    /// The shadow wire mapping, only where declared; not read back (no previous-contract check
    /// reads it yet).
    #[serde(flatten, skip_deserializing)]
    pub mapping: Mapping,
    #[serde(default)]
    pub range: Option<Range>,
    #[serde(default)]
    pub allowed: Option<Vec<i64>>,
    #[serde(default)]
    pub unset: Option<i64>,
    #[serde(default = "one")]
    pub scale: f64,
    /// The error group and bit (R24).
    #[serde(default)]
    pub error: Option<ErrorRef>,
}

impl Limits {
    /// The limits and mapping of key `k`.
    ///
    /// v2: the shadow wire mapping.
    /// @version 2
    /// @req REQ-ING-RNG-001
    /// @req REQ-ING-ERR-001
    /// @req REQ-ING-SHD-001
    fn of(k: &super::keys::Key) -> Self {
        Limits {
            mapping: Mapping::of(k),
            range: k.range,
            allowed: k.allowed.clone(),
            unset: k.unset,
            scale: k.scale,
            error: k.error.clone(),
        }
    }
}

/// Serde default of a scale: 1.
///
/// @version 1
/// @req REQ-ING-RNG-001
fn one() -> f64 {
    1.0
}

/// One principal.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PrincipalJson {
    pub name: String,
    pub uid: u32,
    pub token: u16,
    pub kind: String,
    pub flags: u8,
    pub introduced_in: String,
    pub queue_bound: u32,
    pub latency_budget_ms: u32,
    /// KSET v1 digest, lower-case hex, for each minor 0..=contract minor.
    pub digests: Vec<String>,
}

/// One command type.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct CommandJson {
    pub path: String,
    pub persist: String,
    pub owner: String,
}

/// Name of a principal kind in contract.json.
///
/// @version 1
/// @req REQ-ING-TIV-009
fn kind_name(k: Kind) -> String {
    match k {
        Kind::Unix => "unix",
        Kind::Inproc => "inproc",
        Kind::Uart => "uart",
    }
    .to_string()
}

/// Name of a rights cell in contract.json, `None` for no rights.
///
/// @version 1
/// @req REQ-ING-TIV-009
fn access_name(bits: u8) -> Option<String> {
    match bits & 3 {
        1 => Some("r".to_string()),
        2 => Some("w".to_string()),
        3 => Some("rw".to_string()),
        _ => None,
    }
}

/// The snapshot of `tables`, generated by ingot `commit`.
///
/// v2: the declared variants.
/// v3: the unit vocabulary; principals from principals_json.
/// v4: the error groups.
/// v5: the shadow wire mapping.
/// @version 5
/// @req REQ-ING-SHD-002
/// @req REQ-ING-ERR-001
/// @req REQ-ING-TIV-015
/// @req REQ-ING-RNG-001
/// @req REQ-ING-TIV-009
pub fn snapshot(tables: &Tables, commit: &str) -> Snapshot {
    Snapshot {
        tiv: "1.0".to_string(),
        contract: contract_json(tables),
        variant: tables.variant.clone(),
        variants: tables.variants.clone(),
        units: tables.units.clone().unwrap_or_default(),
        error_groups: tables.error_groups.clone(),
        model_hash: tables.model_hash.clone(),
        ingot: IngotJson {
            version: env!("CARGO_PKG_VERSION").to_string(),
            commit: commit.to_string(),
        },
        keys: tables.keys.iter().map(key_json).collect(),
        reserved: tables.reserved.clone(),
        principals: principals_json(tables),
        rights: rights_json(tables),
        commands: commands_json(tables),
        shadow: super::shadow::json(tables),
    }
}

/// Contract name and version of the snapshot.
///
/// @version 1
/// @req REQ-ING-TIV-009
fn contract_json(tables: &Tables) -> ContractJson {
    ContractJson {
        name: tables.name.clone(),
        major: tables.version.major,
        minor: tables.version.minor,
        patch: tables.patch,
    }
}

/// Command types of the snapshot.
///
/// v2: persistence names from Persist::name.
/// @version 2
/// @req REQ-ING-TIV-009
fn commands_json(tables: &Tables) -> Vec<CommandJson> {
    tables
        .commands
        .iter()
        .map(|c| CommandJson {
            path: c.path.clone(),
            persist: c.persist.name().to_string(),
            owner: c.owner.clone(),
        })
        .collect()
}

/// One key of the snapshot.
///
/// v2: deferred_ms is the effective interval (0 for non-deferred keys).
/// v3: the enum is the variant-resolved one, with its wire strings.
/// v4: persistence names from Persist::name.
/// v5: range, allowed set, sentinel and scale.
/// v6: the error group and bit.
/// @version 6
/// @req REQ-ING-ERR-001
/// @req REQ-ING-TIV-009
/// @req REQ-ING-TIV-016
/// @req REQ-ING-RNG-001
fn key_json(k: &super::keys::Key) -> KeyJson {
    KeyJson {
        path: k.path.clone(),
        id: k.id,
        ty: k.ty.name().to_string(),
        introduced_in: k.introduced.to_string(),
        persist: k.persist.name().to_string(),
        deferred_ms: k.effective_deferred_ms(),
        rate_per_day: k.rate_per_day,
        rate_burst: k.rate_burst,
        max_size: k.max_size,
        secret: k.secret,
        survives_reset: k.survives_reset,
        mfg: k.mfg,
        alias_of: k.alias_of.clone(),
        enum_values: k.enum_values.as_ref().map(enum_numbers),
        enum_wire: k.enum_values.as_ref().map(enum_wires),
        command: k.command.as_ref().map(|(path, _)| path.clone()),
        wk: k.wk.map(|w| WK_RULES[w].name.to_string()),
        doc: k.doc.clone(),
        unit: k.unit.clone(),
        limits: Limits::of(k),
    }
}

/// Number to accept aliases of the values of a resolved enum that have any; `None` when none do.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn enum_accepts(e: &ResolvedEnum) -> Option<BTreeMap<i64, Vec<String>>> {
    let map: BTreeMap<i64, Vec<String>> = e
        .values()
        .filter(|m| !m.accept.is_empty())
        .map(|m| (m.value, m.accept.clone()))
        .collect();
    (!map.is_empty()).then_some(map)
}

/// Name to number of a resolved enum.
///
/// @version 1
/// @req REQ-ING-TIV-016
fn enum_numbers(e: &ResolvedEnum) -> BTreeMap<String, i64> {
    e.iter().map(|(n, m)| (n.clone(), m.value)).collect()
}

/// Number to wire string of a resolved enum.
///
/// @version 1
/// @req REQ-ING-TIV-016
fn enum_wires(e: &ResolvedEnum) -> BTreeMap<i64, String> {
    e.values().map(|m| (m.value, m.wire.clone())).collect()
}

/// Every principal of the snapshot, with its digests.
///
/// @version 1
/// @req REQ-ING-TIV-009
fn principals_json(tables: &Tables) -> Vec<PrincipalJson> {
    digests(tables)
        .iter()
        .enumerate()
        .map(|(p, d)| principal_json(tables, p, d))
        .collect()
}

/// One principal of the snapshot.
///
/// @version 1
/// @req REQ-ING-TIV-009
fn principal_json(tables: &Tables, p: usize, digests: &[[u8; 32]]) -> PrincipalJson {
    let principal = &tables.principals[p];
    PrincipalJson {
        name: principal.name.clone(),
        uid: principal.uid,
        token: principal.token,
        kind: kind_name(principal.kind),
        flags: principal.flags,
        introduced_in: principal.introduced.to_string(),
        queue_bound: queue_bound(tables, p),
        latency_budget_ms: principal.latency_budget_ms,
        digests: digests.iter().map(|d| hex(d)).collect(),
    }
}

/// The non-empty rights cells, by principal and key path.
///
/// @version 1
/// @req REQ-ING-TIV-009
fn rights_json(tables: &Tables) -> BTreeMap<String, BTreeMap<String, String>> {
    tables
        .principals
        .iter()
        .zip(&tables.rights)
        .map(|(p, row)| {
            let cells = tables
                .keys
                .iter()
                .zip(row)
                .filter_map(|(k, bits)| access_name(*bits).map(|a| (k.path.clone(), a)))
                .collect();
            (p.name.clone(), cells)
        })
        .collect()
}
