//! Contract-mode input of the shadow wire mapping (G2 1.4): the per-key attributes' types, the
//! apply groups, echo-only desired paths and constant reported rows of `contract.toml`. The same
//! types are written into contract.json and the keyspace files.

use serde::{Deserialize, Serialize};

/// How a path carries an enum key's value.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WireForm {
    /// The value's wire string (inbound: a wire string or an accept alias).
    Enum,
    /// The code as a JSON integer, inbound and outbound.
    Integer,
}

/// The reported documents a path is in.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// The full document only.
    Full,
    /// The on-change delta only.
    Delta,
    /// Both.
    Both,
}

/// One shadow path of a key, with its declared wire form and scope (absent: the defaults).
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PathForm {
    pub path: String,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub form: Option<WireForm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
}

impl PathForm {
    /// Whether the path carries the code as an integer.
    ///
    /// @version 1
    /// @req REQ-ING-SHD-003
    pub fn integer(&self) -> bool {
        self.form == Some(WireForm::Integer)
    }

    /// Whether the path is in the full document.
    ///
    /// @version 1
    /// @req REQ-ING-SHD-003
    pub fn in_full(&self) -> bool {
        self.scope != Some(Scope::Delta)
    }

    /// Whether the path is in the delta.
    ///
    /// @version 1
    /// @req REQ-ING-SHD-003
    pub fn in_delta(&self) -> bool {
        self.scope != Some(Scope::Full)
    }

    /// Whether the path declares neither a form nor a scope.
    ///
    /// @version 1
    /// @req REQ-ING-SHD-003
    pub fn plain(&self) -> bool {
        self.form.is_none() && self.scope.is_none()
    }
}

/// One entry of a key's `shadow`: a bare path, or a path with its form and scope.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PathDecl {
    Bare(String),
    Form(PathForm),
}

impl PathDecl {
    /// The entry as a path with its declared form and scope.
    ///
    /// @version 1
    /// @req REQ-ING-SHD-003
    fn form(&self) -> PathForm {
        match self {
            PathDecl::Bare(path) => PathForm {
                path: path.clone(),
                form: None,
                scope: None,
            },
            PathDecl::Form(f) => f.clone(),
        }
    }
}

/// A key's `shadow`: one entry or several.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Paths {
    One(PathDecl),
    Many(Vec<PathDecl>),
}

impl Paths {
    /// The paths with their forms and scopes, in declaration order.
    ///
    /// v2: entries may declare a wire form and a scope.
    /// @version 2
    /// @req REQ-ING-SHD-001
    /// @req REQ-ING-SHD-003
    pub fn list(&self) -> Vec<PathForm> {
        match self {
            Paths::One(p) => vec![p.form()],
            Paths::Many(list) => list.iter().map(PathDecl::form).collect(),
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

/// Direction of a topic.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Pub,
    Sub,
}

/// The connection a topic uses when the model names none.
///
/// @version 1
/// @req REQ-ING-TOP-001
fn main_connection() -> String {
    "main".to_string()
}

/// `[[topics]]`: one MQTT topic of the device.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Topic {
    pub id: String,
    /// Over `{stage}`, `{duid}`, `{model}`, `{thing}` and `{shadow_name}`, each repeatable.
    pub template: String,
    pub direction: Direction,
    #[serde(default = "main_connection")]
    pub connection: String,
    pub subscribe_order: Option<u16>,
    /// The key of the engineering `/eng/cmd` wrapper that feeds this topic's handler.
    pub local_wrapper: Option<String>,
    /// The pub topic responses go to.
    pub ack: Option<String>,
    pub envelope: Option<Envelope>,
    /// Every body on this topic also runs the settings parsers on its top-level fields.
    #[serde(default)]
    pub settings_in_every_body: bool,
    /// The shadow name is a literal here although another topic uses `{shadow_name}` (kept).
    #[serde(default)]
    pub literal_shadow_name: bool,
}

/// A command envelope: the id and the source of a request.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub id: Option<EnvelopeId>,
    pub source: Option<EnvelopeSource>,
}

/// The envelope id: required, or optional with a declared absent value.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeId {
    pub field: String,
    #[serde(default)]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absent: Option<toml::Value>,
}

/// The envelope source: the field it is read from (none: never read) and the origin enum
/// member it takes when absent.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeSource {
    pub field: Option<String>,
    /// `namespace::enum`
    #[serde(rename = "enum")]
    pub enum_ref: String,
    pub absent: String,
}

/// How a topic's requests are processed.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    /// One request, one command.
    #[default]
    Single,
    /// An array of command names drained one at a time, one response at the end.
    CommandArray,
}

/// A selector's wire value: a string or a bool.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum WireLit {
    Flag(bool),
    Text(String),
}

/// Which requests a row matches: `field` equal to `wire`, holding `element` in its array, or
/// being an object.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Selector {
    pub field: String,
    pub wire: Option<WireLit>,
    pub element: Option<String>,
    #[serde(default)]
    pub object: bool,
}

/// The kind of a request body.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BodyKind {
    /// The arguments are fields of the request.
    Fields,
    /// The request is a settings body (the desired tree without `settings.`).
    Settings,
    /// A raw binary body: no library build accepts one (refused).
    Binary,
}

/// `body = { object, selector }`: the arguments are fields of one object holding the verb.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObjectBody {
    pub object: String,
    pub selector: Selector,
}

/// A row's body.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum Body {
    Kind(BodyKind),
    Object(ObjectBody),
}

impl Default for Body {
    /// Fields of the request.
    ///
    /// @version 1
    /// @req REQ-ING-CMD-001
    fn default() -> Self {
        Body::Kind(BodyKind::Fields)
    }
}

/// `required = "per_handler"`: not yet read from the handler (refused).
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Unresolved {
    PerHandler,
}

/// Whether an argument must be present.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(untagged)]
pub enum Required {
    Flag(bool),
    Unresolved(Unresolved),
}

impl Default for Required {
    /// Optional.
    ///
    /// @version 1
    /// @req REQ-ING-CMD-001
    fn default() -> Self {
        Required::Flag(false)
    }
}

/// How a text argument is encoded on the wire.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    /// Base64 text, passed through as text (never decoded).
    Base64,
}

/// One argument of a row: the request field and the key it is written to.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Arg {
    pub field: String,
    pub key: String,
    #[serde(default)]
    pub required: Required,
    /// The result code a missing required argument answers.
    pub ec: Option<u8>,
    /// The value written when the field is absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absent: Option<toml::Value>,
    /// Nothing is written when the field is absent.
    #[serde(default)]
    pub omit_when_absent: bool,
    pub encoding: Option<Encoding>,
}

/// `[[shadow_commands]]`: one way a request reaches a command, or a settings body.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ShadowCommand {
    pub topic: String,
    /// The variants the row exists on (absent: every one), and the library build it is grounded in.
    pub variants: Option<Vec<String>>,
    pub grounded_in: Option<String>,
    #[serde(default)]
    pub protocol: Protocol,
    pub selector: Selector,
    #[serde(default)]
    pub body: Body,
    /// The command type's path (`namespace.class.command`); none for a settings body.
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<Arg>,
}
