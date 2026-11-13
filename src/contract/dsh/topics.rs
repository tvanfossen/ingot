//! The topic table `dtp_topics` (G2 section 4), with each sub topic's envelope (1.4.8).

use super::super::model::Tables;
use super::super::schema::{Direction, Envelope, Topic};
use super::{index16, text, Pools};

/// `.source_field`, `.source_absent` and `.source_enum` of an envelope's source.
///
/// @version 1
/// @req REQ-ING-TOP-002
fn source(tables: &Tables, env: Option<&Envelope>, pools: &mut Pools) -> String {
    let source = env.and_then(|e| e.source.as_ref());
    let resolved = source.and_then(|s| tables.enums.get(&s.enum_ref).map(|e| (s, e)));
    let (source_enum, absent) = match resolved {
        Some((s, e)) => (
            Some(pools.enums.index(&s.enum_ref, e)),
            e.get(&s.absent).map_or(0, |m| m.value),
        ),
        None => (None, 0),
    };
    format!(
        ".source_field = {}, .source_absent = {absent}, .source_enum = {}",
        text(source.and_then(|s| s.field.as_deref())),
        index16(source_enum)
    )
}

/// `.id_field`, `.id_absent` and the source fields of an envelope, and its `DTP_F_*` flags.
///
/// @version 1
/// @req REQ-ING-TOP-002
fn envelope(
    tables: &Tables,
    env: Option<&Envelope>,
    pools: &mut Pools,
) -> (String, Vec<&'static str>) {
    let id = env.and_then(|e| e.id.as_ref());
    let id_absent = id
        .and_then(|i| i.absent.as_ref())
        .map(|v| serde_json::to_string(v).unwrap_or_default());
    let mut flags = Vec::new();
    if env.is_some() {
        flags.push("DTP_F_ENVELOPE");
    }
    if id.is_some_and(|i| i.required) {
        flags.push("DTP_F_ID_REQUIRED");
    }
    let fields = format!(
        ".id_field = {}, .id_absent = {}, {}",
        text(id.map(|i| i.field.as_str())),
        text(id_absent.as_deref()),
        source(tables, env, pools),
    );
    (fields, flags)
}

/// One `dtp_topics` row.
///
/// @version 1
/// @req REQ-ING-TOP-002
fn row(tables: &Tables, t: &Topic, pools: &mut Pools) -> String {
    let (fields, mut flags) = envelope(tables, t.envelope.as_ref(), pools);
    if t.settings_in_every_body {
        flags.push("DTP_F_SETTINGS_IN_EVERY_BODY");
    }
    if t.literal_shadow_name {
        flags.push("DTP_F_LITERAL_SHADOW_NAME");
    }
    let flags = super::flags(&flags);
    let ack = t
        .ack
        .as_ref()
        .and_then(|a| tables.shadow.topics.iter().position(|o| &o.id == a))
        .map_or("DSH_NONE_U8".to_string(), |i| format!("{i}u"));
    let direction = match t.direction {
        Direction::Pub => "DTP_PUB",
        Direction::Sub => "DTP_SUB",
    };
    format!(
        "{{ .id = {}, .pattern = {}, .connection = {}, .local_wrapper = {}, {fields}, \
         .subscribe_order = {}u, .direction = {direction}, .ack = {ack}, .flags = {flags} }}",
        text(Some(&t.id)),
        text(Some(&t.template)),
        text(Some(&t.connection)),
        text(t.local_wrapper.as_deref()),
        t.subscribe_order.unwrap_or(0),
    )
}

/// The topics, in declaration order.
///
/// @version 1
/// @req REQ-ING-TOP-002
pub fn rows(tables: &Tables, pools: &mut Pools) -> Vec<String> {
    tables
        .shadow
        .topics
        .iter()
        .map(|t| row(tables, t, pools))
        .collect()
}
