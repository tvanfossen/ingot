//! Topic placeholders beyond the per-device ones, bounded in count and length:
//! - per-message vars, declared per topic: on a pub topic the publisher fills them at publish
//!   time; on a sub topic each is one whole topic level, which the consumer subscribes to as a
//!   single-level wildcard and extracts from a received topic;
//! - declared placeholders, contract-level: named values the consumer is configured with (a
//!   build or device value), usable in any template.

use super::keys::Errors;
use super::model::Tables;
use super::schema::{Direction, Topic, TopicVar};
use super::shadow_size::valid_path;
use super::topics::PLACEHOLDERS;
use std::collections::BTreeSet;

/// The most vars one topic declares (`DTP_VARS_MAX`), and the most declared placeholders
/// (`DTP_PLACEHOLDERS_MAX`).
pub const VARS_MAX: usize = 8;

/// The longest value a var or declared placeholder may declare, in bytes (`DTP_VAR_LEN_MAX`).
pub const VAR_LEN_MAX: u16 = 128;

/// Whether `name` is a var of `t`.
///
/// @version 1
/// @req REQ-ING-TOP-004
pub fn declared(t: &Topic, name: &str) -> bool {
    t.vars.iter().any(|v| v.name == name)
}

/// Why the name or length bound of `v`, a `kind` ("var" or "placeholder"), is refused.
///
/// @version 1
/// @req REQ-ING-TOP-004
/// @req REQ-ING-TOP-007
fn name_rules(kind: &str, v: &TopicVar) -> Vec<String> {
    let mut why = Vec::new();
    let name = &v.name;
    if !valid_path(name) || name.contains('.') || PLACEHOLDERS.contains(&name.as_str()) {
        why.push(format!(
            "{kind} {name} must be an identifier other than a built-in placeholder"
        ));
    }
    if v.max_len == 0 || v.max_len > VAR_LEN_MAX {
        why.push(format!("{kind} {name} needs max_len 1..={VAR_LEN_MAX}"));
    }
    why
}

/// Why the list `all` of `kind` is refused: more than the bound, a name declared twice, and each
/// entry's name rules.
///
/// @version 1
/// @req REQ-ING-TOP-004
/// @req REQ-ING-TOP-007
fn list_rules(kind: &str, all: &[TopicVar]) -> Vec<String> {
    let mut why = Vec::new();
    if all.len() > VARS_MAX {
        why.push(format!("more than {VARS_MAX} {kind}s"));
    }
    let mut seen = BTreeSet::new();
    for v in all {
        if !seen.insert(&v.name) {
            why.push(format!("{kind} {} is declared twice", v.name));
        }
        why.extend(name_rules(kind, v));
    }
    why
}

/// Whether `template` uses `{name}`.
///
/// @version 1
/// @req REQ-ING-TOP-004
/// @req REQ-ING-TOP-007
fn uses(template: &str, name: &str) -> bool {
    template.contains(&format!("{{{name}}}"))
}

/// Whether `{name}` is exactly one whole level of the template, once.
///
/// @version 1
/// @req REQ-ING-TOP-006
fn one_whole_level(template: &str, name: &str) -> bool {
    let placeholder = format!("{{{name}}}");
    let levels = template.split('/').filter(|l| *l == placeholder).count();
    levels == 1 && template.matches(&placeholder).count() == 1
}

/// Why var `v` of `t` is refused beyond the list rules: a template without it, a declared
/// placeholder's name, or on a sub topic not one whole level used once.
///
/// v2: a declared placeholder's name; the sub-topic rule here; name rules in `list_rules`.
/// @version 2
/// @req REQ-ING-TOP-004
/// @req REQ-ING-TOP-006
/// @req REQ-ING-TOP-007
fn var_rule(t: &Topic, v: &TopicVar, placeholders: &[TopicVar]) -> Vec<String> {
    let mut why = Vec::new();
    let name = &v.name;
    if !uses(&t.template, name) {
        why.push(format!("var {name} is not used by its template"));
    }
    if placeholders.iter().any(|p| &p.name == name) {
        why.push(format!("var {name} is a declared placeholder"));
    }
    if t.direction == Direction::Sub && !one_whole_level(&t.template, name) {
        why.push(format!(
            "var {name} must be one whole topic level, used once, on a sub topic"
        ));
    }
    why
}

/// Every var refusal of topic `t` (`placeholders`: the declared ones).
///
/// v2: vars on sub topics, each one whole level.
/// v3: the list rules shared with declared placeholders; a placeholder's name refused.
/// @version 3
/// @req REQ-ING-TOP-004
/// @req REQ-ING-TOP-006
/// @req REQ-ING-TOP-007
pub fn check(t: &Topic, placeholders: &[TopicVar], errors: &mut Errors) {
    let mut why = list_rules("var", &t.vars);
    for v in &t.vars {
        why.extend(var_rule(t, v, placeholders));
    }
    errors.extend(why.into_iter().map(|w| format!("topic {}: {w}", t.id)));
}

/// Every refusal of the declared placeholders: the list rules, and one no topic uses.
///
/// @version 1
/// @req REQ-ING-TOP-007
pub fn check_placeholders(tables: &Tables, errors: &mut Errors) {
    let s = &tables.shadow;
    errors.extend(list_rules("placeholder", &s.placeholders));
    for p in &s.placeholders {
        if !s.topics.iter().any(|t| uses(&t.template, &p.name)) {
            errors.push(format!("placeholder {} is used by no topic", p.name));
        }
    }
}
