//! Per-message placeholders of topics, declared per topic and bounded in count and length: on a
//! pub topic the publisher fills them at publish time; on a sub topic each is one whole topic
//! level, which the consumer subscribes to as a single-level wildcard and extracts from a
//! received topic.

use super::keys::Errors;
use super::schema::{Direction, Topic, TopicVar};
use super::shadow_size::valid_path;
use super::topics::PLACEHOLDERS;
use std::collections::BTreeSet;

/// The most vars one topic declares (`DTP_VARS_MAX`).
pub const VARS_MAX: usize = 8;

/// The longest value a var may declare, in bytes (`DTP_VAR_LEN_MAX`).
pub const VAR_LEN_MAX: u16 = 128;

/// Whether `name` is a var of `t`.
///
/// @version 1
/// @req REQ-ING-TOP-004
pub fn declared(t: &Topic, name: &str) -> bool {
    t.vars.iter().any(|v| v.name == name)
}

/// Why var `v` of `t` is refused: its name, its length bound, or a template without it.
///
/// @version 1
/// @req REQ-ING-TOP-004
fn var_rule(t: &Topic, v: &TopicVar) -> Vec<String> {
    let mut why = Vec::new();
    let name = &v.name;
    if !valid_path(name) || name.contains('.') || PLACEHOLDERS.contains(&name.as_str()) {
        why.push(format!(
            "var {name} must be an identifier other than a built-in placeholder"
        ));
    }
    if v.max_len == 0 || v.max_len > VAR_LEN_MAX {
        why.push(format!("var {name} needs max_len 1..={VAR_LEN_MAX}"));
    }
    if !t.template.contains(&format!("{{{name}}}")) {
        why.push(format!("var {name} is not used by its template"));
    }
    why
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

/// Every var refusal of topic `t`: on a sub topic a var that is not one whole level used once,
/// too many vars, a name declared twice, and each var's own rules.
///
/// v2: vars on sub topics, each one whole level.
/// @version 2
/// @req REQ-ING-TOP-004
/// @req REQ-ING-TOP-006
pub fn check(t: &Topic, errors: &mut Errors) {
    let mut why = Vec::new();
    let sub = t.direction == Direction::Sub;
    for v in t
        .vars
        .iter()
        .filter(|v| sub && !one_whole_level(&t.template, &v.name))
    {
        why.push(format!(
            "var {} must be one whole topic level, used once, on a sub topic",
            v.name
        ));
    }
    if t.vars.len() > VARS_MAX {
        why.push(format!("more than {VARS_MAX} vars"));
    }
    let mut seen = BTreeSet::new();
    for v in &t.vars {
        if !seen.insert(&v.name) {
            why.push(format!("var {} is declared twice", v.name));
        }
        why.extend(var_rule(t, v));
    }
    errors.extend(why.into_iter().map(|w| format!("topic {}: {w}", t.id)));
}
