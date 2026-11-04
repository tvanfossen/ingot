//! The contract-level shadow rows of a model (G2 1.4: apply groups, echo-only paths, constant
//! rows), the checks of the whole mapping, and what contract.json and the keyspace say about it.

use super::keys::Errors;
use super::model::Tables;
use super::refuse_shadow;
use super::schema::{ApplyGroup, Constant, ContractFile, Echo};
use super::shadow_size::{trees, SHADOW_QUOTA};
use serde::Serialize;
use serde_json::{json, Value};

/// The contract-level rows of the mapping.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Shadow {
    pub apply_groups: Vec<ApplyGroup>,
    pub echo: Vec<Echo>,
    pub constants: Vec<Constant>,
}

/// The rows `contract.toml` declares, taken out of it.
///
/// @version 1
/// @req REQ-ING-SHD-002
pub fn rows(file: &mut ContractFile) -> Shadow {
    Shadow {
        apply_groups: std::mem::take(&mut file.apply_groups),
        echo: std::mem::take(&mut file.shadow_echo),
        constants: std::mem::take(&mut file.shadow_constant),
    }
}

/// Whether the model maps anything to the shadow.
///
/// @version 1
/// @req REQ-ING-SHD-001
pub fn declared(tables: &Tables) -> bool {
    let s = &tables.shadow;
    tables.keys.iter().any(|k| !k.wire.shadow.is_empty())
        || !(s.apply_groups.is_empty() && s.echo.is_empty() && s.constants.is_empty())
}

/// The mapping's rows and worst-case sizes, `None` when the model maps nothing.
///
/// @version 1
/// @req REQ-ING-SHD-002
/// @req REQ-ING-SIZ-001
pub fn json(tables: &Tables) -> Option<Value> {
    if !declared(tables) {
        return None;
    }
    let t = trees(tables);
    let s = &tables.shadow;
    Some(json!({
        "apply_groups": s.apply_groups, "echo": s.echo, "constants": s.constants,
        "full_max_bytes": t.reported.bytes(), "delta_forced_bytes": t.forced.bytes(),
    }))
}

/// Every refusal of the mapping: keys and rows (S1, S3, S4, S5), the path trees (S1) and the
/// full document's size (Z1).
///
/// @version 1
/// @req REQ-ING-SHD-001
/// @req REQ-ING-SIZ-001
pub fn check(tables: &Tables, errors: &mut Errors) {
    refuse_shadow::check(tables, errors);
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
