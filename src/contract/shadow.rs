//! The contract-level shadow rows of a model (G2 1.4: apply groups, echo-only paths, constant
//! rows, topics and the command rows of the variant), the checks of the whole mapping, and what
//! contract.json and the keyspace say about it.

use super::keys::Errors;
use super::model::{present_on, Tables};
use super::schema::{
    ApplyGroup, Constant, ContractFile, Echo, NamespaceFile, ShadowCommand, Topic,
};
use super::shadow_size::{trees, SHADOW_QUOTA};
use super::{refuse_commands, refuse_shadow, topics};
use serde::Serialize;
use serde_json::{json, Value};

/// The contract-level rows of the mapping.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Shadow {
    pub apply_groups: Vec<ApplyGroup>,
    pub echo: Vec<Echo>,
    pub constants: Vec<Constant>,
    pub topics: Vec<Topic>,
    /// The command rows that exist on the variant, in namespace-file order.
    pub commands: Vec<ShadowCommand>,
}

/// The rows `contract.toml` declares, taken out of it, and the namespace files' command rows
/// that exist on `variant`.
///
/// v2: topics and command rows.
/// @version 2
/// @req REQ-ING-SHD-002
/// @req REQ-ING-CMD-001
/// @req REQ-ING-TOP-001
pub fn rows(
    file: &mut ContractFile,
    namespaces: &[NamespaceFile],
    variant: Option<&str>,
) -> Shadow {
    let commands = namespaces
        .iter()
        .flat_map(|ns| &ns.shadow_commands)
        .filter(|r| present_on(r.variants.as_deref(), variant))
        .cloned()
        .collect();
    Shadow {
        apply_groups: std::mem::take(&mut file.apply_groups),
        echo: std::mem::take(&mut file.shadow_echo),
        constants: std::mem::take(&mut file.shadow_constant),
        topics: std::mem::take(&mut file.topics),
        commands,
    }
}

/// Whether the model maps anything to the shadow (a command row needs a topic, so the topics
/// stand for the command rows).
///
/// v2: topics.
/// @version 2
/// @req REQ-ING-SHD-001
/// @req REQ-ING-TOP-001
pub fn declared(tables: &Tables) -> bool {
    let s = &tables.shadow;
    let rows = s.apply_groups.len() + s.echo.len() + s.constants.len() + s.topics.len();
    tables.keys.iter().any(|k| !k.wire.shadow.is_empty()) || rows > 0
}

/// The mapping's rows and worst-case sizes, `None` when the model maps nothing.
///
/// v2: topics and command rows.
/// @version 2
/// @req REQ-ING-SHD-002
/// @req REQ-ING-SIZ-001
/// @req REQ-ING-CMD-001
pub fn json(tables: &Tables) -> Option<Value> {
    if !declared(tables) {
        return None;
    }
    let t = trees(tables);
    let s = &tables.shadow;
    Some(json!({
        "apply_groups": s.apply_groups, "echo": s.echo, "constants": s.constants,
        "topics": s.topics, "commands": s.commands,
        "full_max_bytes": t.reported.bytes(), "delta_forced_bytes": t.forced.bytes(),
    }))
}

/// Every refusal of the mapping: keys and rows (S1, S3, S4, S5), the path trees (S1), the
/// full document's size (Z1), topics (T1, T2) and command rows (S2, S6).
///
/// v2: topics and command rows.
/// @version 2
/// @req REQ-ING-SHD-001
/// @req REQ-ING-SIZ-001
/// @req REQ-ING-TOP-001
/// @req REQ-ING-CMD-001
pub fn check(tables: &Tables, errors: &mut Errors) {
    refuse_shadow::check(tables, errors);
    topics::check(tables, errors);
    refuse_commands::check(tables, errors);
    let t = trees(tables);
    let full = t.reported.bytes();
    errors.extend(t.desired.errors);
    errors.extend(t.reported.errors);
    if full > SHADOW_QUOTA {
        errors.push(format!(
            "the full shadow document is up to {full} bytes, over the {SHADOW_QUOTA}-byte AWS IoT shadow quota"
        ));
    }
}
