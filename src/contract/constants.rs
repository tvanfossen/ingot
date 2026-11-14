//! `dmt_constants.h`: the code of every value of every enum key and of every command's outcome
//! enum as a named C constant, `DMT_E_<KEY OR COMMAND PATH>_<VALUE>`, for the variant the tables
//! resolve, so a consumer never writes an enum code by hand. The header is per model and outside
//! the table interface.

use super::enums::ResolvedEnum;
use super::keys::Errors;
use super::model::Tables;
use crate::codegen::render_template;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;
use tera::{Context, Tera};

/// The longest constant name: C99's significant initial characters of an identifier.
pub const NAME_MAX: usize = 63;

/// One enum constant: its name, its code and the key it is of.
#[derive(Serialize)]
pub struct EnumConstant {
    pub name: String,
    pub value: i64,
    pub key: String,
}

/// `DMT_E_<KEY PATH>_<VALUE>` in upper case, each dot an underscore.
///
/// @version 1
/// @req REQ-ING-TIV-017
fn name_of(key: &str, value: &str) -> String {
    format!("DMT_E_{}_{}", key.replace('.', "_"), value).to_uppercase()
}

/// Every enum a constant is generated for, with the ID it is placed at and the path it is
/// named after: each enum key's enum, and each command's outcome enum at its command key.
///
/// @version 1
/// @req REQ-ING-TIV-017
/// @req REQ-ING-TIV-018
fn sources(tables: &Tables) -> Vec<(u32, &str, &ResolvedEnum)> {
    let keys = tables
        .keys
        .iter()
        .filter_map(|k| k.enum_values.as_ref().map(|e| (k.id, k.path.as_str(), e)));
    let commands = tables
        .commands
        .iter()
        .filter_map(|c| c.outcome.as_ref().map(|e| (c.keys[0], c.path.as_str(), e)));
    let mut all: Vec<_> = keys.chain(commands).collect();
    all.sort_by_key(|(id, _, _)| *id);
    all
}

/// The constants of every enum key and command outcome, in key ID order and each one's values
/// in code order.
///
/// v2: command outcomes, at their command key's ID.
/// @version 2
/// @req REQ-ING-TIV-017
/// @req REQ-ING-TIV-018
pub fn enum_constants(tables: &Tables) -> Vec<EnumConstant> {
    let mut all = Vec::new();
    for (_, path, values) in sources(tables) {
        let mut mine: Vec<EnumConstant> = values
            .iter()
            .map(|(name, m)| EnumConstant {
                name: name_of(path, name),
                value: m.value,
                key: path.to_string(),
            })
            .collect();
        mine.sort_by_key(|c| c.value);
        all.extend(mine);
    }
    all
}

/// Whether `name` is a C identifier.
///
/// @version 1
/// @req REQ-ING-TIV-017
fn c_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Every constant is a C identifier of at most `NAME_MAX` characters, and no two (key, value)
/// pairs name one constant.
///
/// @version 1
/// @req REQ-ING-TIV-017
pub fn check(tables: &Tables, errors: &mut Errors) {
    let mut owner: BTreeMap<String, String> = BTreeMap::new();
    for c in enum_constants(tables) {
        let mut refuse =
            |why: String| errors.push(format!("{}: enum constant {} {why}", c.key, c.name));
        if !c_identifier(&c.name) {
            refuse("is not a C identifier".into());
        } else if c.name.len() > NAME_MAX {
            refuse(format!("is longer than {NAME_MAX} characters"));
        }
        match owner.get(&c.name) {
            Some(other) => refuse(format!("is also {other}'s")),
            None => {
                owner.insert(c.name.clone(), c.key.clone());
            }
        }
    }
}

/// Write `dmt_constants.h` into `output_dir`.
///
/// @version 1
/// @req REQ-ING-TIV-017
pub fn write(
    tera: &Tera,
    tables: &Tables,
    commit: &str,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let v = &tables.version;
    let mut ctx = Context::new();
    ctx.insert("ingot_version", env!("CARGO_PKG_VERSION"));
    ctx.insert("ingot_commit", commit);
    ctx.insert("name", &tables.name);
    ctx.insert(
        "version",
        &format!("{}.{}.{}", v.major, v.minor, tables.patch),
    );
    ctx.insert("variant", &tables.variant);
    ctx.insert("constants", &enum_constants(tables));
    render_template(tera, "dmt_constants.h", &ctx, output_dir)?;
    Ok(())
}
