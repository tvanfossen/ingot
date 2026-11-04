//! Contract-mode input of the shadow wire mapping (G2 1.4): the per-key attributes' types, the
//! apply groups, echo-only desired paths and constant reported rows of `contract.toml`. The same
//! types are written into contract.json and the keyspace files.

use serde::{Deserialize, Serialize};

/// A key's `shadow`: one dotted path or several.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Paths {
    One(String),
    Many(Vec<String>),
}

impl Paths {
    /// The paths, in declaration order.
    ///
    /// @version 1
    /// @req REQ-ING-SHD-001
    pub fn list(&self) -> Vec<String> {
        match self {
            Paths::One(p) => vec![p.clone()],
            Paths::Many(list) => list.clone(),
        }
    }
}

/// What the binder reports for a rejected desired value.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnReject {
    /// Today: the value is reported back as received, with the outcome.
    Echo,
    /// `desired.<path> = null` plus the outcome (a wire change).
    ClearDesired,
}

/// `implies = [{ key, value }]`: a coupled write in the same batch.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Implies {
    pub key: String,
    pub value: toml::Value,
}

/// How a reported key's value is rendered.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenderKind {
    /// A comma-separated index string reported as the array of the indexed enum's wire strings.
    CsvIndexList,
}

/// `render = { kind, enum, max_items }`.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Render {
    pub kind: RenderKind,
    /// The enum (of the key's namespace) whose wire strings the indices select.
    #[serde(rename = "enum")]
    pub enum_ref: String,
    /// The most items the list holds (today's `MAX_LANGUAGES`).
    pub max_items: u16,
}

/// `[[apply_groups]]`: SETTINGS keys one desired update writes as one keystore batch.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ApplyGroup {
    pub id: String,
    pub doc: Option<String>,
}

/// The JSON type of an echo-only path.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EchoType {
    Bool,
    Int,
    Float,
    String,
}

/// `[[shadow_echo]]`: a desired path reported back as received, with no key.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Echo {
    pub path: String,
    #[serde(rename = "type")]
    pub ty: EchoType,
    /// Longest text of a string path, in bytes.
    pub max_size: Option<u32>,
    /// The SETTINGS key command bodies apply this field to (the delta path only echoes it).
    pub command_key: Option<String>,
}

/// A constant whose value the image supplies.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Image {
    /// `"M.m.p"` of the contract, filled by ingot.
    ContractVersion,
    /// The 64-hex SHA-256 of the resolved keyspace body, filled when the image is built.
    ContractHash,
}

/// `[[shadow_constant]]`: a reported path with a literal or image value and no key.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Constant {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<toml::Value>,
    pub image: Option<Image>,
    /// Re-emitted on every delta.
    #[serde(default)]
    pub forced: bool,
}
