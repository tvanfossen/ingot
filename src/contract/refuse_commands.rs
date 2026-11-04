//! Refusals of the shadow command rows (G2 1.4.6, 1.4.7): S2, and S6, every command of the
//! variant enumerated on the wire or marked internal with a reason.

use super::keys::{Errors, Key};
use super::model::Tables;
use super::schema::{
    Arg, Body, BodyKind, Direction, Encoding, KeyType, Protocol, Required, Selector, ShadowCommand,
    WireLit,
};
use super::value::encode_default;

/// Every command-row refusal.
///
/// @version 1
/// @req REQ-ING-CMD-001
pub fn check(tables: &Tables, errors: &mut Errors) {
    let rows = &tables.shadow.commands;
    for r in rows {
        let mut refuse = |why: String| errors.push(format!("{}: {why}", label(r)));
        topic_rules(tables, r, &mut refuse);
        selector_rules(tables, r, &mut refuse);
        body_rules(tables, r, &mut refuse);
        for a in &r.args {
            arg_rules(tables, r, a, &mut refuse);
        }
    }
    duplicates(rows, errors);
    enumerated(tables, errors);
}

/// `field=wire`, `field[element]` or `field{}`.
///
/// @version 1
/// @req REQ-ING-CMD-001
pub fn selector_text(s: &Selector) -> String {
    match (&s.wire, &s.element) {
        (Some(WireLit::Text(w)), _) => format!("{}={w}", s.field),
        (Some(WireLit::Flag(b)), _) => format!("{}={b}", s.field),
        (None, Some(e)) => format!("{}[{e}]", s.field),
        (None, None) => format!("{}{{}}", s.field),
    }
}

/// `shadow_commands <topic> <selector>[ <object>.<selector>]`.
///
/// @version 1
/// @req REQ-ING-CMD-001
fn label(r: &ShadowCommand) -> String {
    let inner = match &r.body {
        Body::Object(o) => format!(" {}.{}", o.object, selector_text(&o.selector)),
        Body::Kind(_) => String::new(),
    };
    format!(
        "shadow_commands {} {}{inner}",
        r.topic,
        selector_text(&r.selector)
    )
}

/// The row's topic is a sub topic; a per-variant row names the build it is grounded in.
///
/// @version 1
/// @req REQ-ING-CMD-001
fn topic_rules(tables: &Tables, r: &ShadowCommand, refuse: &mut impl FnMut(String)) {
    let topic = tables.shadow.topics.iter().find(|t| t.id == r.topic);
    if topic.is_none_or(|t| t.direction != Direction::Sub) {
        refuse(format!("topic {} is not a sub topic", r.topic));
    }
    if r.variants.is_some() != r.grounded_in.is_some() {
        refuse("a per-variant row names the build it is grounded in, and only it".into());
    }
}

/// How many of wire, element and object a selector uses.
///
/// @version 1
/// @req REQ-ING-CMD-001
fn forms(s: &Selector) -> usize {
    usize::from(s.wire.is_some()) + usize::from(s.element.is_some()) + usize::from(s.object)
}

/// One selector form; element selectors are exactly the command_array protocol, whose topic has
/// an envelope id; an object body selects its verb by wire.
///
/// @version 1
/// @req REQ-ING-CMD-001
fn selector_rules(tables: &Tables, r: &ShadowCommand, refuse: &mut impl FnMut(String)) {
    if forms(&r.selector) != 1 {
        refuse("the selector has one of wire, element or object".into());
    }
    let array = r.protocol == Protocol::CommandArray;
    if r.selector.element.is_some() != array {
        refuse("an element selector is the command_array protocol, and only it".into());
    }
    let topic = tables.shadow.topics.iter().find(|t| t.id == r.topic);
    let id = topic
        .and_then(|t| t.envelope.as_ref())
        .and_then(|e| e.id.as_ref());
    if array && id.is_none() {
        refuse("a command_array topic needs an envelope id".into());
    }
    object_rule(r, refuse);
}

/// An object body selects its verb by wire, and by nothing else.
///
/// @version 1
/// @req REQ-ING-CMD-001
fn object_rule(r: &ShadowCommand, refuse: &mut impl FnMut(String)) {
    if let Body::Object(o) = &r.body {
        if forms(&o.selector) != 1 || o.selector.wire.is_none() {
            refuse("an object body selects its verb by wire".into());
        }
    }
}

/// A settings body has no command and no args; any other body names a command of the variant;
/// a binary body is refused.
///
/// @version 1
/// @req REQ-ING-CMD-001
fn body_rules(tables: &Tables, r: &ShadowCommand, refuse: &mut impl FnMut(String)) {
    let known = |c: &String| tables.commands.iter().any(|x| &x.path == c);
    match (&r.body, &r.command) {
        (Body::Kind(BodyKind::Binary), _) => {
            refuse("no library build accepts a binary body; send base64 text in a field".into())
        }
        (Body::Kind(BodyKind::Settings), c) if c.is_some() || !r.args.is_empty() => {
            refuse("a settings body has no command and no args".into())
        }
        (Body::Kind(BodyKind::Settings), _) => {}
        (_, Some(c)) if known(c) => {}
        (_, c) => refuse(format!(
            "command {} is not a command of this variant",
            c.as_deref().unwrap_or("(none)")
        )),
    }
}

/// The plain key of the row command's class that `path` names.
///
/// @version 1
/// @req REQ-ING-CMD-001
fn arg_key<'a>(tables: &'a Tables, r: &ShadowCommand, path: &str) -> Option<&'a Key> {
    let class = r.command.as_deref()?.rsplit_once('.')?.0;
    let key = &tables.keys[tables.key_index(path)?];
    let in_class = path.rsplit_once('.').is_some_and(|(c, _)| c == class);
    (in_class && key.command.is_none()).then_some(key)
}

/// An argument writes a plain key of the command's class; a required one has its result code
/// and no absent rule; an optional one has exactly one absent rule and no code; an absent value
/// suits the key; base64 text goes into a string key.
///
/// @version 1
/// @req REQ-ING-CMD-001
fn arg_rules(tables: &Tables, r: &ShadowCommand, a: &Arg, refuse: &mut impl FnMut(String)) {
    let Some(key) = arg_key(tables, r, &a.key) else {
        return refuse(format!(
            "arg {}: {} is not a plain key of the command's class",
            a.field, a.key
        ));
    };
    let rule = match a.required {
        Required::Unresolved(_) => Some("required = per_handler must be resolved from the handler"),
        Required::Flag(true) if a.ec.is_none() || a.absent.is_some() || a.omit_when_absent => {
            Some("a required argument has its ec and no absent rule")
        }
        Required::Flag(false) if a.ec.is_some() || a.absent.is_some() == a.omit_when_absent => {
            Some("an optional argument has an absent value or omit_when_absent, and no ec")
        }
        _ => None,
    };
    if let Some(why) = rule {
        refuse(format!("arg {}: {why}", a.field));
    }
    let value = a
        .absent
        .as_ref()
        .map(|v| encode_default(key.ty, v, key.max_size, key.enum_values.as_ref()).map(|_| ()));
    if let Some(Err(e)) = value {
        refuse(format!("arg {}: {e}", a.field));
    }
    if a.encoding == Some(Encoding::Base64) && key.ty != KeyType::String {
        refuse(format!(
            "arg {}: base64 text goes into a string key",
            a.field
        ));
    }
}

/// Whether two rows' variant lists share a variant (absent: every variant).
///
/// @version 1
/// @req REQ-ING-CMD-001
fn overlap(a: &Option<Vec<String>>, b: &Option<Vec<String>>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x.iter().any(|v| y.contains(v)),
        _ => true,
    }
}

/// No two rows select the same request on one variant.
///
/// @version 1
/// @req REQ-ING-CMD-001
fn duplicates(rows: &[ShadowCommand], errors: &mut Errors) {
    for (i, a) in rows.iter().enumerate() {
        let twice = rows[i + 1..]
            .iter()
            .any(|b| label(a) == label(b) && overlap(&a.variants, &b.variants));
        if twice {
            errors.push(format!(
                "{}: the request is selected twice on one variant",
                label(a)
            ));
        }
    }
}

/// S6: when any row exists, every command of the variant has a row or is internal with a
/// reason, never both.
///
/// @version 1
/// @req REQ-ING-CMD-001
fn enumerated(tables: &Tables, errors: &mut Errors) {
    let rows = &tables.shadow.commands;
    if rows.is_empty() {
        return;
    }
    for c in &tables.commands {
        let on_wire = rows.iter().any(|r| r.command.as_ref() == Some(&c.path));
        let why = match (on_wire, c.internal.as_deref()) {
            (false, None) => Some(
                "no shadow_commands row enumerates it (declare one, or internal = \"<reason>\")",
            ),
            (true, Some(_)) => Some("an internal command has a shadow_commands row"),
            (_, Some(reason)) if reason.trim().is_empty() => Some("internal needs a reason"),
            _ => None,
        };
        if let Some(why) = why {
            errors.push(format!("{}: {why}", c.path));
        }
    }
}
