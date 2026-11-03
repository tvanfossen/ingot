//! Load a contract-mode model and resolve it into the content of the generated tables.

use super::keys::{command_keys, declared_key, ClassCtx, Errors, Key};
use super::rights::{self, Principal};
use super::schema::{
    Bounds, CommandDef, ContractFile, NamespaceFile, Persist, Reserved, UidCapsDef,
};
use super::types::{Version, WK_RULES};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// One `dmt_commands` row.
#[derive(Debug)]
pub struct Command {
    pub path: String,
    pub keys: [u32; 3],
    pub max_payload: u32,
    pub max_result: u32,
    pub max_edits: u16,
    pub dedupe_ring: u16,
    pub owner: String,
    pub persist: Persist,
}

/// A namespace's name, ID and classes `(name, class_index)`, for the layout checks.
#[derive(Debug)]
pub struct NsLayout {
    pub name: String,
    pub id: u16,
    pub classes: Vec<(String, u8)>,
}

/// Everything the generated tables contain.
#[derive(Debug)]
pub struct Tables {
    pub layout: Vec<NsLayout>,
    pub name: String,
    pub version: Version,
    pub patch: u16,
    pub variant: String,
    pub model_hash: String,
    pub keys: Vec<Key>,
    pub reserved: Vec<u32>,
    pub principals: Vec<Principal>,
    pub rights: Vec<Vec<u8>>,
    pub commands: Vec<Command>,
    pub uid_caps: Vec<UidCapsDef>,
    pub bounds: Bounds,
}

/// The parsed input files, each with its path relative to the contract file's directory.
struct Loaded {
    contract: ContractFile,
    namespaces: Vec<NamespaceFile>,
    files: Vec<(String, Vec<u8>)>,
}

/// Load and resolve the model whose contract file is `contract_path`, for `variant`.
///
/// @version 1
/// @req REQ-ING-TIV-002
/// @req REQ-ING-TIV-003
pub fn build(contract_path: &Path, variant: Option<&str>) -> Result<(Tables, Errors), String> {
    let loaded = load(contract_path)?;
    let mut errors = Errors::new();
    let (keys, commands) = collect_keys(&loaded, variant, &mut errors);
    let (principals, grants) = rights::principals(loaded.contract.principals, &mut errors);
    rights::check_grants(&principals, &grants, &keys, &mut errors);
    let rights = rights::resolve(&principals, &grants, &keys, &mut errors);
    let reserved = reserved_ids(&loaded.contract.reserved);
    let mut uid_caps = loaded.contract.uid_caps;
    uid_caps.sort_by_key(|c| c.uid);
    let meta = loaded.contract.contract;
    let tables = Tables {
        layout: loaded.namespaces.iter().map(layout).collect(),
        version: Version {
            major: meta.major,
            minor: meta.minor,
        },
        patch: meta.patch,
        name: meta.name,
        variant: variant.unwrap_or_default().to_string(),
        model_hash: model_hash(&loaded.files, variant.unwrap_or_default()),
        keys,
        reserved,
        principals,
        rights,
        commands,
        uid_caps,
        bounds: loaded.contract.bounds,
    };
    Ok((tables, errors))
}

/// The reserved IDs, sorted, always including ID 0 (R-K2).
///
/// @version 1
/// @req REQ-ING-TIV-010
fn reserved_ids(declared: &[Reserved]) -> Vec<u32> {
    let mut reserved: Vec<u32> = declared.iter().map(|r| r.id).collect();
    reserved.push(0);
    reserved.sort_unstable();
    reserved.dedup();
    reserved
}

/// The layout record of one namespace file.
///
/// @version 1
/// @req REQ-ING-TIV-010
fn layout(ns: &NamespaceFile) -> NsLayout {
    NsLayout {
        name: ns.meta.id.clone(),
        id: ns.meta.namespace_id,
        classes: ns
            .classes
            .iter()
            .map(|c| (c.id.clone(), c.class_index))
            .collect(),
    }
}

/// Read and parse the contract file and every `.toml` in its namespace directory.
///
/// @version 1
/// @req REQ-ING-TIV-002
fn load(contract_path: &Path) -> Result<Loaded, String> {
    let base = contract_path.parent().unwrap_or(Path::new("."));
    let text = read(contract_path)?;
    let contract: ContractFile = parse(contract_path, &text)?;
    let mut files = vec![(file_name(contract_path), text.into_bytes())];
    let mut namespaces = Vec::new();
    for path in namespace_files(&base.join(&contract.contract.namespace_dir))? {
        let text = read(&path)?;
        namespaces.push(parse(&path, &text)?);
        let rel = path
            .strip_prefix(base)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        files.push((rel, text.into_bytes()));
    }
    files.sort();
    Ok(Loaded {
        contract,
        namespaces,
        files,
    })
}

/// The sorted `.toml` files of `dir`.
///
/// @version 1
/// @req REQ-ING-TIV-002
fn namespace_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    paths.sort();
    if paths.is_empty() {
        return Err(format!("{}: no namespace .toml files", dir.display()));
    }
    Ok(paths)
}

/// Read a file, naming it in the error.
///
/// @version 1
/// @req REQ-ING-TIV-002
fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Parse TOML into `T`, naming the file in the error.
///
/// @version 1
/// @req REQ-ING-TIV-002
fn parse<T: serde::de::DeserializeOwned>(path: &Path, text: &str) -> Result<T, String> {
    toml::from_str(text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The file's name, for the model hash.
///
/// @version 1
/// @req REQ-ING-TIV-013
fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Model hash (diagnostic): SHA-256 over every input as `path 0x00 u64le(len) bytes`, in path
/// order, then `0x00 variant`.
///
/// @version 1
/// @req REQ-ING-TIV-013
pub fn model_hash(files: &[(String, Vec<u8>)], variant: &str) -> String {
    let mut hasher = Sha256::new();
    for (path, bytes) in files {
        hasher.update(path.as_bytes());
        hasher.update([0u8]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    hasher.update([0u8]);
    hasher.update(variant.as_bytes());
    hex(&hasher.finalize())
}

/// Lower-case hex of `bytes`.
///
/// @version 1
/// @req REQ-ING-TIV-013
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Every key of the model (declared and command-generated), sorted by ID, and the commands.
///
/// @version 1
/// @req REQ-ING-TIV-003
/// @req REQ-ING-TIV-005
fn collect_keys(
    loaded: &Loaded,
    variant: Option<&str>,
    errors: &mut Errors,
) -> (Vec<Key>, Vec<Command>) {
    let mut keys = Vec::new();
    let mut commands = Vec::new();
    for ns in &loaded.namespaces {
        for class in &ns.classes {
            let ctx = ClassCtx {
                ns,
                class,
                variant,
                default_burst: loaded.contract.bounds.default_rate_burst,
            };
            keys.extend(class.keys.iter().map(|def| declared_key(&ctx, def, errors)));
            for def in &class.commands {
                let three = command_keys(&ctx, def, errors);
                commands.push(command_row(def, &three));
                keys.extend(three);
            }
        }
    }
    keys.sort_by_key(|k| k.id);
    commands.sort_by_key(|c| c.keys[0]);
    (keys, commands)
}

/// The `dmt_commands` row of a command type.
///
/// @version 1
/// @req REQ-ING-TIV-005
fn command_row(def: &CommandDef, keys: &[Key; 3]) -> Command {
    Command {
        path: keys[0]
            .command
            .as_ref()
            .map(|(p, _)| p.clone())
            .unwrap_or_default(),
        keys: [keys[0].id, keys[1].id, keys[2].id],
        max_payload: def.payload_max,
        max_result: def.result_max,
        max_edits: def.max_edits,
        dedupe_ring: def.dedupe_ring,
        owner: def.owner.clone(),
        persist: def.persistence,
    }
}

impl Tables {
    /// Index of the key with path `path`.
    ///
    /// @version 1
    /// @req REQ-ING-TIV-003
    pub fn key_index(&self, path: &str) -> Option<usize> {
        self.keys.iter().position(|k| k.path == path)
    }

    /// Index of the principal named `name`.
    ///
    /// @version 1
    /// @req REQ-ING-TIV-006
    pub fn principal_index(&self, name: &str) -> Option<usize> {
        self.principals.iter().position(|p| p.name == name)
    }

    /// The key ID mapped to each well-known role, `0` when unmapped.
    ///
    /// @version 1
    /// @req REQ-ING-TIV-009
    pub fn wk(&self) -> [u32; WK_RULES.len()] {
        let mut ids = [0u32; WK_RULES.len()];
        for key in &self.keys {
            if let Some(role) = key.wk {
                ids[role] = key.id;
            }
        }
        ids
    }

    /// The key ID behind an optional key path, `0` when absent or unknown.
    ///
    /// @version 1
    /// @req REQ-ING-TIV-009
    pub fn id_of(&self, path: Option<&str>) -> u32 {
        path.and_then(|p| self.key_index(p))
            .map_or(0, |i| self.keys[i].id)
    }

    /// Whether `principal` may write the key at `index`.
    ///
    /// @version 1
    /// @req REQ-ING-TIV-011
    pub fn writes(&self, principal: usize, index: usize) -> bool {
        self.rights[principal][index] & rights::WRITE != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_hash_matches_an_independent_vector() {
        // printf 'a\000\001\000\000\000\000\000\000\000x\000v' | sha256sum
        let files = vec![("a".to_string(), b"x".to_vec())];
        assert_eq!(
            model_hash(&files, "v"),
            "913dffd426bb94722008ef80ff188dab32c28a371ee384dca1dca7a27c1385f4"
        );
    }

    #[test]
    fn reserved_ids_always_hold_zero_sorted_once() {
        let declared = [Reserved { id: 9 }, Reserved { id: 0 }, Reserved { id: 3 }];
        assert_eq!(reserved_ids(&declared), vec![0, 3, 9]);
        assert_eq!(reserved_ids(&[]), vec![0]);
    }
}
