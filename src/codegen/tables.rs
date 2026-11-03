//! Store-table outputs of the table interface (TIV): `dmt_types.h` and, in contract mode, the
//! generated tables.

use std::path::Path;
use tera::{Context, Tera};

use super::render_template;

/// The fixed TIV header; its bytes depend only on the TIV.
pub const TYPES_HEADER: &str = "dmt_types.h";

/// Write `dmt_types.h` into `output_dir`, creating the directory.
///
/// @version 1
/// @req REQ-ING-TIV-001
pub fn write_table_types(tera: &Tera, output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(output_dir)?;
    render_template(tera, TYPES_HEADER, &Context::new(), output_dir)?;
    log::info!("Generated {TYPES_HEADER} (TIV 1.0)");
    Ok(())
}
