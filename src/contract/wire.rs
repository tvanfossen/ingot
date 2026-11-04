//! The shadow wire attributes of one key (G2 1.4), resolved from its declaration; they are
//! written into contract.json and the keyspace files and never enter the store tables.

use super::enums::ResolvedEnum;
use super::keys::{ClassCtx, Errors};
use super::schema::{Implies, KeyDef, OnReject, Render, Role};
use serde::Serialize;

/// A key's shadow paths and how the binder and reporter treat them.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct KeyWire {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shadow: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applies: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_group: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub implies: Vec<Implies>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_reject: Option<OnReject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inbound_unknown: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outbound_unknown: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub forced: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render: Option<Render>,
    /// The enum a rendered list indexes, resolved for the variant.
    #[serde(skip)]
    pub render_values: Option<ResolvedEnum>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telem: Option<String>,
}

/// The wire attributes `def` declares; a shadowed SETTINGS key rejects by echo unless it says
/// otherwise, and a rendered list's enum is resolved from the key's namespace.
///
/// @version 1
/// @req REQ-ING-SHD-001
/// @req REQ-ING-SHD-002
pub fn of(ctx: &ClassCtx, path: &str, def: &KeyDef, errors: &mut Errors) -> KeyWire {
    let shadow = def.shadow.as_ref().map(|p| p.list()).unwrap_or_default();
    let echo_default = ctx.class.role == Role::Settings && !shadow.is_empty();
    let on_reject = def.on_reject.or(echo_default.then_some(OnReject::Echo));
    let render_values = def
        .render
        .as_ref()
        .and_then(|r| render_enum(ctx, path, r, errors));
    KeyWire {
        shadow,
        applies: def.applies.clone(),
        apply_group: def.apply_group.clone(),
        implies: def.implies.clone(),
        on_reject,
        inbound_unknown: def.inbound_unknown.clone(),
        outbound_unknown: def.outbound_unknown.clone(),
        forced: def.forced,
        render: def.render.clone(),
        render_values,
        telem: def.telem.clone(),
    }
}

/// The enum of the key's namespace a rendered list indexes, recording an error for an
/// undefined one.
///
/// @version 1
/// @req REQ-ING-SHD-001
fn render_enum(
    ctx: &ClassCtx,
    path: &str,
    render: &Render,
    errors: &mut Errors,
) -> Option<ResolvedEnum> {
    let found = ctx.enums.get(&render.enum_ref).cloned();
    if found.is_none() {
        errors.push(format!("{path}: undefined enum '{}'", render.enum_ref));
    }
    found
}
