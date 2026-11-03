//! Templates compiled into the binary, so the ingot commit alone determines the output.
//!
//! `--templates` / `INGOT_TEMPLATES_DIR` replaces the embedded set with a directory, for
//! template development.

use std::path::Path;
use tera::Tera;

/// Every file in `templates/`, under its file name. A test checks the list is complete.
const EMBEDDED: &[(&str, &str)] = &[
    (
        "CMakeLists.txt",
        include_str!("../../templates/CMakeLists.txt"),
    ),
    (
        "boolean_storage.c",
        include_str!("../../templates/boolean_storage.c"),
    ),
    (
        "boolean_storage.h",
        include_str!("../../templates/boolean_storage.h"),
    ),
    ("dm.c", include_str!("../../templates/dm.c")),
    ("dm.h", include_str!("../../templates/dm.h")),
    ("dm_enums.h", include_str!("../../templates/dm_enums.h")),
    ("dm_helpers.c", include_str!("../../templates/dm_helpers.c")),
    ("dm_helpers.h", include_str!("../../templates/dm_helpers.h")),
    ("dm_key.h", include_str!("../../templates/dm_key.h")),
    (
        "dm_key_event_list.hpp",
        include_str!("../../templates/dm_key_event_list.hpp"),
    ),
    (
        "dm_key_events.hpp",
        include_str!("../../templates/dm_key_events.hpp"),
    ),
    (
        "dm_key_events_wrapper.cpp",
        include_str!("../../templates/dm_key_events_wrapper.cpp"),
    ),
    (
        "dm_key_events_wrapper.hpp",
        include_str!("../../templates/dm_key_events_wrapper.hpp"),
    ),
    (
        "dm_key_has_event.hpp",
        include_str!("../../templates/dm_key_has_event.hpp"),
    ),
    (
        "dm_namespace_definitions.h",
        include_str!("../../templates/dm_namespace_definitions.h"),
    ),
    ("dm_rust.rs", include_str!("../../templates/dm_rust.rs")),
    ("dmt_types.h", include_str!("../../templates/dmt_types.h")),
    (
        "integer_storage.c",
        include_str!("../../templates/integer_storage.c"),
    ),
    (
        "integer_storage.h",
        include_str!("../../templates/integer_storage.h"),
    ),
    (
        "jenkins_hash.c",
        include_str!("../../templates/jenkins_hash.c"),
    ),
    (
        "jenkins_hash.h",
        include_str!("../../templates/jenkins_hash.h"),
    ),
    (
        "key_definitions.h",
        include_str!("../../templates/key_definitions.h"),
    ),
    (
        "persistence_storage.c",
        include_str!("../../templates/persistence_storage.c"),
    ),
    (
        "persistence_storage.h",
        include_str!("../../templates/persistence_storage.h"),
    ),
    (
        "string_storage.c",
        include_str!("../../templates/string_storage.c"),
    ),
    (
        "string_storage.h",
        include_str!("../../templates/string_storage.h"),
    ),
    ("test_dm.c", include_str!("../../templates/test_dm.c")),
];

/// Load the templates: the embedded set, or every file in `dir` when one is given.
///
/// @version 1
/// @req REQ-ING-CLI-002
pub fn load(dir: Option<&Path>) -> Result<Tera, Box<dyn std::error::Error>> {
    match dir {
        Some(dir) => from_dir(dir),
        None => embedded(),
    }
}

/// The templates compiled into this binary.
///
/// @version 1
/// @req REQ-ING-CLI-002
fn embedded() -> Result<Tera, Box<dyn std::error::Error>> {
    let mut tera = Tera::default();
    tera.add_raw_templates(EMBEDDED.iter().copied())?;
    Ok(tera)
}

/// Every file in `dir`, which must exist.
///
/// @version 1
/// @req REQ-ING-CLI-002
fn from_dir(dir: &Path) -> Result<Tera, Box<dyn std::error::Error>> {
    if !dir.is_dir() {
        return Err(format!(
            "Specified templates directory does not exist: {}",
            dir.display()
        )
        .into());
    }
    let glob = dir.join("*");
    Ok(Tera::new(glob.to_str().ok_or("invalid template path")?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_template_file_is_embedded() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
        let on_disk: BTreeSet<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        let embedded: BTreeSet<String> = EMBEDDED.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(embedded, on_disk, "EMBEDDED must list exactly templates/");
    }

    #[test]
    fn embedded_set_loads_every_template() {
        let tera = load(None).unwrap();
        let names: BTreeSet<&str> = tera.get_template_names().collect();
        assert_eq!(names.len(), EMBEDDED.len());
        assert!(names.contains("dmt_types.h"));
    }

    #[test]
    fn missing_directory_is_an_error() {
        let err = load(Some(Path::new("/nonexistent/ingot-templates"))).unwrap_err();
        assert!(err.to_string().contains("does not exist"), "{err}");
    }

    #[test]
    fn directory_replaces_the_embedded_set() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("only.h"), "x").unwrap();
        let tera = load(Some(tmp.path())).unwrap();
        let names: Vec<&str> = tera.get_template_names().collect();
        assert_eq!(names, vec!["only.h"]);
    }
}
