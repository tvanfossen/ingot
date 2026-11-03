//! Contract mode: generate the table-interface (TIV 1.0) store tables of a keyed model.
//!
//! The input is `contract.toml` plus its namespace files (specification section 7); the outputs
//! are `dmt_types.h`, `dmt_tables.h`, `dmt_tables.c`, `contract.json` and, against a previous
//! contract, `release-review.txt`. Every refusal of section 8 is an error.

pub mod bounds;
pub mod digest;
pub mod emit;
pub mod enums;
pub mod keys;
pub mod model;
pub mod previous;
pub mod ranges;
pub mod refuse_model;
pub mod refuse_rights;
pub mod rights;
pub mod schema;
pub mod snapshot;
pub mod types;
pub mod value;
pub mod variants;

use crate::codegen::{tables::write_table_types, write_generated};
use std::path::Path;
use tera::Tera;

/// The ingot commit this binary was built from (`-dirty` with uncommitted changes).
const COMMIT: &str = env!("INGOT_GIT_COMMIT");

/// Inputs of one contract-mode run.
pub struct Options<'a> {
    pub contract: &'a Path,
    pub variant: Option<&'a str>,
    pub previous: Option<&'a Path>,
    pub output: &'a Path,
    pub allow_dirty: bool,
}

/// The commit to stamp into the outputs; a dirty or unknown build is refused unless allowed.
///
/// @version 1
/// @req REQ-ING-TIV-013
pub fn provenance(commit: &str, allow_dirty: bool) -> Result<String, String> {
    let clean = !commit.ends_with("-dirty") && commit != "unknown";
    if clean || allow_dirty {
        Ok(commit.to_string())
    } else {
        Err(format!(
            "ingot was built from {commit}; contract mode needs a clean commit (or --allow-dirty-ingot)"
        ))
    }
}

/// Generate the contract-mode outputs, or fail with every refusal logged.
///
/// @version 1
/// @req REQ-ING-TIV-009
/// @req REQ-ING-TIV-010
/// @req REQ-ING-TIV-011
/// @req REQ-ING-TIV-012
pub fn generate(opts: &Options, tera: &Tera) -> Result<(), Box<dyn std::error::Error>> {
    let commit = provenance(COMMIT, opts.allow_dirty)?;
    let (tables, mut errors) = model::build(opts.contract, opts.variant)?;
    refuse_model::check(&tables, &mut errors);
    refuse_rights::check(&tables, &mut errors);
    bounds::check(&tables, &mut errors);
    let snapshot = snapshot::snapshot(&tables, &commit);
    let review = match opts.previous {
        Some(path) => Some(previous::compare(
            &read_snapshot(path)?,
            &snapshot,
            &mut errors,
        )),
        None => None,
    };
    refuse(&errors)?;
    write_outputs(opts, tera, &tables, &snapshot, review.as_deref())
}

/// Write every contract-mode output for accepted tables.
///
/// @version 1
/// @req REQ-ING-TIV-009
/// @req REQ-ING-TIV-012
fn write_outputs(
    opts: &Options,
    tera: &Tera,
    tables: &model::Tables,
    snapshot: &snapshot::Snapshot,
    review: Option<&[String]>,
) -> Result<(), Box<dyn std::error::Error>> {
    write_table_types(tera, opts.output)?;
    emit::write(tera, tables, &snapshot.ingot.commit, opts.output)?;
    let json = serde_json::to_string_pretty(snapshot)?;
    write_generated(opts.output.join("contract.json"), json)?;
    if let Some(lines) = review {
        write_generated(opts.output.join("release-review.txt"), review_text(lines))?;
    }
    log::info!(
        "Contract {} {}.{}: {} keys, {} principals",
        tables.name,
        tables.version.major,
        tables.version.minor,
        tables.keys.len(),
        tables.principals.len()
    );
    Ok(())
}

/// Read a previous release's contract.json.
///
/// @version 1
/// @req REQ-ING-TIV-012
fn read_snapshot(path: &Path) -> Result<snapshot::Snapshot, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Log every refusal and fail when there is any.
///
/// @version 1
/// @req REQ-ING-TIV-010
fn refuse(errors: &[String]) -> Result<(), String> {
    for e in errors {
        log::error!("{e}");
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} refusal(s); nothing was generated",
            errors.len()
        ))
    }
}

/// The release-review report.
///
/// @version 1
/// @req REQ-ING-TIV-012
fn review_text(lines: &[String]) -> String {
    if lines.is_empty() {
        "no sensitive grants to new principals".to_string()
    } else {
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_or_unknown_builds_need_the_override() {
        assert_eq!(provenance("abc123", false), Ok("abc123".to_string()));
        assert!(provenance("abc123-dirty", false).is_err());
        assert!(provenance("unknown", false).is_err());
        assert_eq!(
            provenance("abc123-dirty", true),
            Ok("abc123-dirty".to_string())
        );
    }

    #[test]
    fn review_text_says_when_there_is_nothing() {
        assert_eq!(review_text(&[]), "no sensitive grants to new principals");
        assert_eq!(review_text(&["a".into(), "b".into()]), "a\nb");
    }
}
