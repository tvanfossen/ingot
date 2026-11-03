//! Contract-mode input: `contract.toml` and the namespace files it names (table-interface
//! specification section 7). Unknown fields are errors; every index is explicit.

use serde::Deserialize;
use std::collections::BTreeMap;

/// `contract.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractFile {
    pub contract: ContractMeta,
    pub bounds: Bounds,
    #[serde(default)]
    pub reserved: Vec<Reserved>,
    #[serde(default)]
    pub principals: Vec<PrincipalDef>,
    #[serde(default)]
    pub uid_caps: Vec<UidCapsDef>,
}

/// `[contract]`: name, version and where the namespace files are.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractMeta {
    pub name: String,
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub namespace_dir: String,
    /// The product variants (SKUs); when declared, --variant must name one.
    #[serde(default)]
    pub variants: Vec<String>,
}

/// `[bounds]`: model-level inputs to the queue bounds and budgets.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    pub headroom_pct: u32,
    pub min_queue: u32,
    pub default_rate_burst: u32,
    pub staging_total_bytes: u32,
    #[serde(default)]
    pub inproc_queue_bytes: u32,
    #[serde(default)]
    pub inproc_snapshot_bytes: u32,
}

/// One `[[reserved]]` key ID.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reserved {
    pub id: u32,
}

/// Principal kind.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Unix,
    Inproc,
    Uart,
}

/// Principal flag.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalFlag {
    WatchdogRequired,
    TraceReader,
    TokenIssuer,
}

/// Access a grant confers.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    None,
    R,
    W,
    Rw,
}

/// One grant: a whole namespace, one (namespace, class), or one key (`ns.class.key`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub namespace: Option<String>,
    pub class: Option<String>,
    pub key: Option<String>,
    pub access: Access,
}

/// Per-(uid, token) caps.
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Caps {
    #[serde(default)]
    pub inflight_fd: u32,
    #[serde(default)]
    pub inflight_bytes: u32,
    #[serde(default)]
    pub open_fds: u32,
    #[serde(default)]
    pub trace_share_pct: u16,
}

/// One `[[principals]]` entry.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalDef {
    pub name: String,
    pub uid: u32,
    #[serde(default)]
    pub token: u16,
    pub kind: Kind,
    #[serde(default)]
    pub flags: Vec<PrincipalFlag>,
    pub heartbeat: Option<String>,
    #[serde(default)]
    pub heartbeat_window_ms: u32,
    pub latency_budget_ms: u32,
    #[serde(default)]
    pub caps: Caps,
    pub resync_k: u16,
    pub resync_t_ms: u32,
    pub staging_bound_bytes: u32,
    pub introduced_in: String,
    #[serde(default)]
    pub grants: Vec<Grant>,
}

/// One `[[uid_caps]]` entry: totals for a UNIX uid.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UidCapsDef {
    pub uid: u32,
    pub inflight_fd: u32,
    pub inflight_bytes: u32,
    pub open_fds: u32,
}

/// A namespace file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamespaceFile {
    pub meta: NamespaceMeta,
    #[serde(default)]
    pub enums: BTreeMap<String, EnumDef>,
    #[serde(default)]
    pub classes: Vec<ClassDef>,
}

/// `[meta]` of a namespace file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamespaceMeta {
    pub id: String,
    pub namespace_id: u16,
    #[serde(default)]
    pub mfg: bool,
    /// Documentation only; accepted, not generated.
    #[serde(rename = "doc")]
    pub _doc: Option<String>,
}

/// A named enum: member name to value (and wire string), and its per-variant changes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnumDef {
    /// Documentation only; accepted, not generated.
    #[serde(rename = "doc")]
    pub _doc: Option<String>,
    pub values: BTreeMap<String, EnumValueDef>,
    #[serde(default)]
    pub variants: BTreeMap<String, EnumVariantDef>,
}

/// One enum value: a bare number (its wire string is its name) or a number with a wire string.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum EnumValueDef {
    Number(i64),
    Wire(WireValue),
}

/// `{ value = n, wire = "..." }`.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct WireValue {
    pub value: i64,
    pub wire: String,
}

/// A variant's changes to an enum: values merged over the base by number, base names removed.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnumVariantDef {
    #[serde(default)]
    pub values: BTreeMap<String, EnumValueDef>,
    #[serde(default)]
    pub remove: Vec<String>,
}

/// Class role.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Settings,
    State,
    Command,
    Config,
    Diag,
}

/// Persistence class.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Persist {
    #[default]
    None,
    Deferred,
    Durable,
}

/// Value type of a key.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KeyType {
    Bool,
    Uint8,
    Int8,
    Uint16,
    Int16,
    Uint32,
    Int32,
    Float32,
    String,
    Binary,
    Struct,
}

/// A class of a namespace.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassDef {
    pub id: String,
    pub class_index: u8,
    pub role: Role,
    #[serde(default)]
    pub persistence: Persist,
    #[serde(default)]
    pub deferred_ms: u32,
    /// Documentation only; accepted, not generated.
    #[serde(rename = "doc")]
    pub _doc: Option<String>,
    #[serde(default)]
    pub keys: Vec<KeyDef>,
    #[serde(default)]
    pub commands: Vec<CommandDef>,
}

/// A key of a class.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyDef {
    pub id: String,
    pub key_index: u16,
    #[serde(rename = "type")]
    pub ty: KeyType,
    pub introduced_in: String,
    pub write_rate: Option<String>,
    pub rate_burst: Option<u32>,
    pub persistence: Option<Persist>,
    pub deferred_ms: Option<u32>,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub readers: Vec<String>,
    #[serde(default)]
    pub survives_reset: bool,
    pub alias_of: Option<String>,
    pub default: Option<toml::Value>,
    #[serde(default)]
    pub defaults: BTreeMap<String, toml::Value>,
    #[serde(rename = "enum")]
    pub enum_ref: Option<String>,
    pub max_size: Option<u32>,
    pub wk: Option<String>,
    pub doc: Option<String>,
    pub unit: Option<String>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub thread_safe: bool,
}

/// Payload kind of a command key.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Payload {
    #[default]
    Binary,
    Struct,
}

/// A command type: expands to its command, result and dedupe keys.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandDef {
    pub id: String,
    pub cmd_index: u16,
    pub result_index: u16,
    pub dedupe_index: u16,
    pub introduced_in: String,
    pub persistence: Persist,
    pub owner: String,
    #[serde(default)]
    pub payload: Payload,
    #[serde(default)]
    pub payload_max: u32,
    #[serde(default)]
    pub result_max: u32,
    #[serde(default)]
    pub max_edits: u16,
    pub dedupe_ring: u16,
    pub write_rate: Option<String>,
    pub wk: Option<String>,
    pub doc: Option<String>,
}
