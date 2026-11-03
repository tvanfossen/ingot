pub mod enums;
pub mod rust_kvp;
pub mod storage;
pub mod target;
pub mod yaml_manifest;

use crate::model::key::{define_stem, KeyEncoding};
use crate::model::schema::{DataModel, DataType, KeyDef};
use serde::Serialize;
use std::path::Path;
use tera::{Context, Tera};

/// Write generated text to `path`, normalised to end in exactly one newline.
///
/// @version 1
/// @req REQ-ING-GEN-001
pub(crate) fn write_generated(path: impl AsRef<Path>, content: String) -> std::io::Result<()> {
    let normalized = format!("{}\n", content.trim_end_matches('\n'));
    std::fs::write(path, normalized)
}

/// Join values as `a, b, c`, the element list of a generated C or Rust array initialiser.
///
/// @version 1
/// @req REQ-ING-GEN-004
/// @req REQ-ING-GEN-005
/// @req REQ-ING-GEN-006
/// @req REQ-ING-RST-002
pub(crate) fn join_list<T: std::fmt::Display>(values: &[T]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// A key definition ready for template rendering.
#[derive(Debug, Serialize)]
pub struct KeyDefRenderable {
    pub namespace: String,
    pub class: String,
    pub name: String,
    pub define_name: String,
    pub hex_value: String,
    pub type_name: String,
    pub unit: Option<String>,
    pub read_only: bool,
    pub thread_safe: bool,
    pub persistent: bool,
    pub event: bool,
}

/// A single event key ready for the dispatch-by-key switch.
#[derive(Debug, Serialize)]
struct EventKeyRenderable {
    /// The key #define (e.g. "DM_KEY_APPLIANCE_STATUS_MODE") — the switch case.
    define_name: String,
    /// The tinyfsm event struct (e.g. "FSM_EVENT_APPLIANCE_STATUS_MODE").
    fsm_event_name: String,
}

/// Event keys grouped by (namespace, class) for the event-struct header.
#[derive(Debug, Serialize)]
struct EventGroupRenderable {
    namespace: String,
    class: String,
    /// FSM event struct names in this namespace/class, in declaration order.
    events: Vec<String>,
}

/// Integer type descriptor for the API dispatch templates.
#[derive(Debug, Serialize)]
struct DmIntTypeInfo {
    type_enum: String,
    c_type: String,
    val_field: String,
    get_fn: String,
    set_fn: String,
    wrapper_suffix: String,
}

/// Map integer storage suffix to API dispatch info.
/// @version 1
/// @req REQ-ING-GEN-011
fn int_type_info(suffix: &str) -> DmIntTypeInfo {
    let (type_enum, c_type, val_field, wrapper) = match suffix {
        "UINT8" => ("DM_KEY_TYPE_UINT8", "uint8_t", "u8val", "UInt8"),
        "SINT8" => ("DM_KEY_TYPE_INT8", "int8_t", "s8val", "SInt8"),
        "UINT16" => ("DM_KEY_TYPE_UINT16", "uint16_t", "u16val", "UInt16"),
        "SINT16" => ("DM_KEY_TYPE_INT16", "int16_t", "s16val", "SInt16"),
        "UINT32" => ("DM_KEY_TYPE_UINT32", "uint32_t", "u32val", "UInt32"),
        "SINT32" => ("DM_KEY_TYPE_INT32", "int32_t", "s32val", "SInt32"),
        _ => unreachable!("unknown integer suffix: {suffix}"),
    };
    DmIntTypeInfo {
        type_enum: type_enum.to_string(),
        c_type: c_type.to_string(),
        val_field: val_field.to_string(),
        get_fn: format!("IntegerStorage_Get{suffix}Key"),
        set_fn: format!("IntegerStorage_Set{suffix}Key"),
        wrapper_suffix: wrapper.to_string(),
    }
}

/// Generate all C code from a parsed data model.
///
/// v2: the Unity test collectors no longer take the namespace ID they did not use.
/// v3: neither does the helper collector.
/// @version 3
/// @req REQ-ING-GEN-001
pub fn generate(
    model: &DataModel,
    ns_id: u16,
    output_dir: &Path,
    template_dir: &Path,
    target: &target::TargetConfig,
    no_events: bool,
    emit_tinyfsm: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(output_dir)?;

    let tera = Tera::new(
        template_dir
            .join("*")
            .to_str()
            .ok_or("invalid template path")?,
    )?;

    let version = env!("CARGO_PKG_VERSION");

    // Collect all key definitions
    let key_defs = collect_key_definitions(model, ns_id);

    // Generate key_definitions.h
    {
        let mut ctx = Context::new();
        ctx.insert("version", version);
        ctx.insert("keys", &key_defs);
        let rendered = tera.render("key_definitions.h", &ctx)?;
        write_generated(output_dir.join("key_definitions.h"), rendered)?;
        log::info!("Generated key_definitions.h ({} keys)", key_defs.len());
    }

    // Generate C++/tinyfsm event artifacts (opt-in, additive — see --emit-tinyfsm).
    // Off by default so C99-only consumers and their output are untouched.
    if emit_tinyfsm {
        generate_tinyfsm_events(&tera, version, &key_defs, output_dir)?;
    }

    // Generate jenkins_hash.h and jenkins_hash.c
    {
        let mut ctx = Context::new();
        ctx.insert("version", version);
        let h = tera.render("jenkins_hash.h", &ctx)?;
        let c = tera.render("jenkins_hash.c", &ctx)?;
        write_generated(output_dir.join("jenkins_hash.h"), h)?;
        write_generated(output_dir.join("jenkins_hash.c"), c)?;
        log::info!("Generated jenkins_hash.h/.c");
    }

    // Generate dm_key.h
    {
        let mut ctx = Context::new();
        ctx.insert("version", version);
        let h = tera.render("dm_key.h", &ctx)?;
        write_generated(output_dir.join("dm_key.h"), h)?;
        log::info!("Generated dm_key.h");
    }

    // Generate dm_namespace_definitions.h
    {
        let namespaces = collect_namespaces(model, ns_id);
        let mut ctx = Context::new();
        ctx.insert("version", version);
        ctx.insert("namespaces", &namespaces);
        let h = tera.render("dm_namespace_definitions.h", &ctx)?;
        write_generated(output_dir.join("dm_namespace_definitions.h"), h)?;
        log::info!("Generated dm_namespace_definitions.h");
    }

    // Generate dm_enums.h (only when the model defines named enums)
    let enum_defs = enums::collect_enums(model);
    if !enum_defs.is_empty() {
        let mut ctx = Context::new();
        ctx.insert("version", version);
        ctx.insert("enums", &enum_defs);
        let h = tera.render("dm_enums.h", &ctx)?;
        write_generated(output_dir.join("dm_enums.h"), h)?;
        log::info!("Generated dm_enums.h ({} enum(s))", enum_defs.len());
    }

    // Generate dm_full.yaml manifest
    yaml_manifest::generate_yaml_manifest(model, ns_id, output_dir)?;

    // --- Collect all storage data ---
    let bool_storage = storage::boolean::collect_boolean_storage(model, ns_id)?;
    let int_storages = storage::integer::collect_integer_storage(model, ns_id)?;
    let str_storage = storage::string::collect_string_storage(model, ns_id)?;
    let persist_storage = storage::persistence::collect_persistence_storage(model);

    // Generate boolean_storage.h/.c
    if let Some(ref bs) = bool_storage {
        let mut ctx = Context::new();
        ctx.insert("version", version);
        ctx.insert("bool", bs);
        let h = tera.render("boolean_storage.h", &ctx)?;
        let c = tera.render("boolean_storage.c", &ctx)?;
        write_generated(output_dir.join("boolean_storage.h"), h)?;
        write_generated(output_dir.join("boolean_storage.c"), c)?;
        log::info!(
            "Generated boolean_storage.h/.c ({} keys, {} word(s))",
            bs.num_keys,
            bs.num_words
        );
    }

    // Generate integer_storage.h/.c
    if !int_storages.is_empty() {
        let mut ctx = Context::new();
        ctx.insert("version", version);
        ctx.insert("types", &int_storages);
        let h = tera.render("integer_storage.h", &ctx)?;
        let c = tera.render("integer_storage.c", &ctx)?;
        write_generated(output_dir.join("integer_storage.h"), h)?;
        write_generated(output_dir.join("integer_storage.c"), c)?;
        log::info!(
            "Generated integer_storage.h/.c ({} type groups)",
            int_storages.len()
        );
    }

    // Generate string_storage.h/.c
    if let Some(ref ss) = str_storage {
        let mut ctx = Context::new();
        ctx.insert("version", version);
        ctx.insert("total_keys", &ss.total_keys);
        ctx.insert("ro", &ss.ro);
        ctx.insert("rw", &ss.rw);
        let h = tera.render("string_storage.h", &ctx)?;
        let c = tera.render("string_storage.c", &ctx)?;
        write_generated(output_dir.join("string_storage.h"), h)?;
        write_generated(output_dir.join("string_storage.c"), c)?;
        log::info!(
            "Generated string_storage.h/.c ({} total, {} RO, {} RW)",
            ss.total_keys,
            ss.ro.as_ref().map_or(0, |g| g.num_keys),
            ss.rw.as_ref().map_or(0, |g| g.num_keys),
        );
    }

    // Generate persistence_storage.h/.c
    if let Some(ref ps) = persist_storage {
        let mut ctx = Context::new();
        ctx.insert("version", version);
        ctx.insert("persistence", ps);
        let h = tera.render("persistence_storage.h", &ctx)?;
        let c = tera.render("persistence_storage.c", &ctx)?;
        write_generated(output_dir.join("persistence_storage.h"), h)?;
        write_generated(output_dir.join("persistence_storage.c"), c)?;
        log::info!("Generated persistence_storage.h/.c ({} keys)", ps.num_keys);
    }

    // --- Generate dm.h / dm.c (main API layer) ---
    {
        let has_bool = bool_storage.is_some();
        let has_integers = !int_storages.is_empty();
        let has_ro_strings = str_storage.as_ref().and_then(|s| s.ro.as_ref()).is_some();
        let has_rw_strings = str_storage.as_ref().and_then(|s| s.rw.as_ref()).is_some();

        let has_persistence = persist_storage.is_some();

        let api_int_types: Vec<DmIntTypeInfo> = int_storages
            .iter()
            .map(|s| int_type_info(&s.suffix))
            .collect();

        let mut ctx = Context::new();
        ctx.insert("version", version);
        ctx.insert("has_bool", &has_bool);
        ctx.insert("has_integers", &has_integers);
        ctx.insert("has_ro_strings", &has_ro_strings);
        ctx.insert("has_rw_strings", &has_rw_strings);
        ctx.insert("has_persistence", &has_persistence);
        ctx.insert("int_types", &api_int_types);
        ctx.insert("target", target);
        ctx.insert("no_events", &no_events);

        let h = tera.render("dm.h", &ctx)?;
        let c = tera.render("dm.c", &ctx)?;
        write_generated(output_dir.join("dm.h"), h)?;
        write_generated(output_dir.join("dm.c"), c)?;
        log::info!(
            "Generated dm.h/.c (bool={}, int_types={}, ro_str={}, rw_str={})",
            has_bool,
            api_int_types.len(),
            has_ro_strings,
            has_rw_strings,
        );
    }

    // --- Generate dm_helpers.h / dm_helpers.c ---
    {
        let helpers = collect_helpers(model);
        if !helpers.is_empty() {
            let has_string_helpers = helpers.iter().any(|h| h.is_string);
            let has_enum_helpers = helpers.iter().any(|h| h.is_enum);
            let mut ctx = Context::new();
            ctx.insert("version", version);
            ctx.insert("helpers", &helpers);
            ctx.insert("has_string_helpers", &has_string_helpers);
            ctx.insert("has_enum_helpers", &has_enum_helpers);
            let h = tera.render("dm_helpers.h", &ctx)?;
            write_generated(output_dir.join("dm_helpers.h"), h)?;
            if has_string_helpers {
                let c = tera.render("dm_helpers.c", &ctx)?;
                write_generated(output_dir.join("dm_helpers.c"), c)?;
            }
            log::info!(
                "Generated dm_helpers.h{} ({} helpers)",
                if has_string_helpers { "/.c" } else { "" },
                helpers.len()
            );
        }
    }

    // --- Generate Unity test files ---
    {
        let test_keys = collect_test_keys(model);
        let has_persistence = persist_storage.is_some();
        let persist_test_entries = collect_persistence_test_entries(model);
        let mut ctx = Context::new();
        ctx.insert("version", version);
        ctx.insert("keys", &test_keys);
        ctx.insert("namespace", &model.meta.id);
        ctx.insert("no_events", &no_events);
        ctx.insert("has_persistence", &has_persistence);
        ctx.insert("persistence_entries", &persist_test_entries);
        let test_c = tera.render("test_dm.c", &ctx)?;
        write_generated(output_dir.join("test_dm.c"), test_c)?;
        let cmake = tera.render("CMakeLists.txt", &ctx)?;
        write_generated(output_dir.join("CMakeLists.txt"), cmake)?;
        log::info!(
            "Generated test_dm.c + CMakeLists.txt ({} test keys)",
            test_keys.len()
        );
    }

    Ok(())
}

/// Templates rendered from the flat list of event keys, each written under its own name.
///
/// They share one context, so every file lists exactly the same keys in the same order.
const EVENT_KEY_TEMPLATES: [&str; 4] = [
    "dm_key_events_wrapper.hpp",
    "dm_key_events_wrapper.cpp",
    "dm_key_has_event.hpp",
    "dm_key_event_list.hpp",
];

/// Render the template `name` and write the result to `output_dir/name`.
///
/// @version 1
/// @req REQ-ING-GEN-001
fn render_template(
    tera: &Tera,
    name: &str,
    ctx: &Context,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let rendered = tera.render(name, ctx)?;
    write_generated(output_dir.join(name), rendered)?;
    Ok(())
}

/// Render the C++/tinyfsm event header, the dispatch-by-key wrapper, the
/// has-event predicate and the (key id, event) typelist.
///
/// Emits nothing when no key carries `event = true` — a consumer with no
/// event keys gets no C++ artifacts even with the flag on.
///
/// v2: renders through render_template over EVENT_KEY_TEMPLATES and adds the has-event
/// predicate, which shares the dispatch switch's context.
/// @version 2
/// @req REQ-ING-EVT-001
/// @req REQ-ING-EVT-002
/// @req REQ-ING-EVT-003
fn generate_tinyfsm_events(
    tera: &Tera,
    version: &str,
    key_defs: &[KeyDefRenderable],
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let (events, groups) = collect_event_keys(key_defs);
    if events.is_empty() {
        log::info!("No event keys — skipping tinyfsm event generation");
        return Ok(());
    }

    let mut hpp_ctx = Context::new();
    hpp_ctx.insert("version", version);
    hpp_ctx.insert("groups", &groups);
    render_template(tera, "dm_key_events.hpp", &hpp_ctx, output_dir)?;

    let mut events_ctx = Context::new();
    events_ctx.insert("version", version);
    events_ctx.insert("events", &events);
    for name in EVENT_KEY_TEMPLATES {
        render_template(tera, name, &events_ctx, output_dir)?;
    }

    log::info!(
        "Generated dm_key_events.hpp + {} ({} event keys)",
        EVENT_KEY_TEMPLATES.join(" + "),
        events.len()
    );
    Ok(())
}

/// Build the flat dispatch list + (namespace, class)-grouped struct list for
/// the `event = true` keys.
///
/// The FSM event struct name mirrors the key #define with the `DM_KEY_`
/// prefix swapped for `FSM_EVENT_`, matching the UDM generator's naming so
/// ingot is a drop-in replacement.
/// @version 1
/// @req REQ-ING-EVT-001
fn collect_event_keys(
    key_defs: &[KeyDefRenderable],
) -> (Vec<EventKeyRenderable>, Vec<EventGroupRenderable>) {
    let mut events = Vec::new();
    let mut groups: Vec<EventGroupRenderable> = Vec::new();

    for key in key_defs.iter().filter(|k| k.event) {
        let fsm_event_name = key.define_name.replacen("DM_KEY_", "FSM_EVENT_", 1);
        events.push(EventKeyRenderable {
            define_name: key.define_name.clone(),
            fsm_event_name: fsm_event_name.clone(),
        });

        let namespace = key.namespace.to_uppercase();
        let class = key.class.to_uppercase();
        match groups.last_mut() {
            Some(g) if g.namespace == namespace && g.class == class => {
                g.events.push(fsm_event_name)
            }
            _ => groups.push(EventGroupRenderable {
                namespace,
                class,
                events: vec![fsm_event_name],
            }),
        }
    }

    (events, groups)
}

/// A namespace entry for template rendering.
#[derive(Debug, Serialize)]
struct NamespaceDefRenderable {
    namespace_upper: String,
    ns_id: u16,
}

/// Collect unique namespaces from the model (deduplicated, ordered).
/// @version 1
/// @req REQ-ING-GEN-003
fn collect_namespaces(model: &DataModel, fallback_ns_id: u16) -> Vec<NamespaceDefRenderable> {
    let mut seen = std::collections::BTreeMap::new();
    for class in &model.classes {
        let name = class
            .namespace_name
            .as_deref()
            .unwrap_or(&model.meta.id)
            .to_uppercase();
        let id = class.namespace_id.unwrap_or(fallback_ns_id);
        seen.entry(name).or_insert(id);
    }
    if seen.is_empty() {
        // No classes — still emit the model-level namespace
        seen.insert(model.meta.id.to_uppercase(), fallback_ns_id);
    }
    seen.into_iter()
        .map(|(namespace_upper, ns_id)| NamespaceDefRenderable {
            namespace_upper,
            ns_id,
        })
        .collect()
}

/// Resolve the namespace ID for a class, using per-class override or model-level fallback.
/// @version 1
/// @req REQ-ING-KEY-002
pub(crate) fn resolve_ns_id(class: &crate::model::schema::Class, fallback: u16) -> u16 {
    class.namespace_id.unwrap_or(fallback)
}

/// Resolve the namespace name for a class, using per-class override or model-level fallback.
/// @version 1
/// @req REQ-ING-KEY-002
pub(crate) fn resolve_ns_name(class: &crate::model::schema::Class, model: &DataModel) -> String {
    class
        .namespace_name
        .as_deref()
        .unwrap_or(&model.meta.id)
        .to_string()
}

/// `<NAMESPACE>_<CLASS>_<KEY>` of `key` in `class`; the key's define is `DM_KEY_` + this.
///
/// @version 1
/// @req REQ-ING-GEN-002
pub(crate) fn key_stem(
    class: &crate::model::schema::Class,
    model: &DataModel,
    key: &KeyDef,
) -> String {
    define_stem(&resolve_ns_name(class, model), &class.id, &key.id)
}

/// Resolve the class index, using explicit class_index or positional fallback.
/// @version 1
/// @req REQ-ING-KEY-002
pub(crate) fn resolve_class_idx(class: &crate::model::schema::Class, position: usize) -> u8 {
    class.class_index.unwrap_or(position as u8)
}

/// Encoded 32-bit ID of `key`, declared at `key_pos` in a class with namespace ID `ns_id`
/// and class index `class_idx`: its explicit key_index or its position, its type code and
/// its thread-safe and read-only flags.
///
/// @version 1
/// @req REQ-ING-KEY-001
/// @req REQ-ING-KEY-002
pub(crate) fn encode_key(ns_id: u16, class_idx: u8, key_pos: usize, key: &KeyDef) -> u32 {
    KeyEncoding {
        namespace: ns_id,
        class: class_idx,
        id: key.key_index.unwrap_or(key_pos as u16),
        data_type: key.data_type.type_code(),
        thread_safe: key.thread_safe,
        derived: false,
        read_only: key.read_only,
    }
    .encode()
}

/// Keys paired with their encoded 32-bit IDs.
pub(crate) type EncodedKeys<'m> = Vec<(u32, &'m KeyDef)>;

/// Every key of the model that `wanted` accepts, with its encoded ID, in declaration order.
///
/// @version 1
/// @req REQ-ING-KEY-002
/// @req REQ-ING-GEN-004
/// @req REQ-ING-GEN-005
/// @req REQ-ING-GEN-006
pub(crate) fn encoded_keys(
    model: &DataModel,
    ns_id: u16,
    wanted: impl Fn(&KeyDef) -> bool,
) -> EncodedKeys<'_> {
    let mut keys = Vec::new();
    for (pos, class) in model.classes.iter().enumerate() {
        let c_ns_id = resolve_ns_id(class, ns_id);
        let c_idx = resolve_class_idx(class, pos);
        for (key_pos, key) in class.keys.iter().enumerate() {
            if wanted(key) {
                keys.push((encode_key(c_ns_id, c_idx, key_pos, key), key));
            }
        }
    }
    keys
}

/// Build renderable key definitions from the model.
///
/// v2: the key ID comes from the shared encode_key.
/// v3: the define name comes from the shared key_stem.
/// @version 3
/// @req REQ-ING-GEN-002
fn collect_key_definitions(model: &DataModel, ns_id: u16) -> Vec<KeyDefRenderable> {
    let mut defs = Vec::new();

    for (pos, class) in model.classes.iter().enumerate() {
        let c_ns_id = resolve_ns_id(class, ns_id);
        let c_idx = resolve_class_idx(class, pos);
        for (key_pos, key) in class.keys.iter().enumerate() {
            let encoded = encode_key(c_ns_id, c_idx, key_pos, key);

            defs.push(KeyDefRenderable {
                namespace: resolve_ns_name(class, model),
                class: class.id.clone(),
                name: key.id.clone(),
                define_name: format!("DM_KEY_{}", key_stem(class, model, key)),
                hex_value: format!("{encoded:#010X}"),
                type_name: format!("{:?}", key.data_type).to_lowercase(),
                unit: key.unit.clone(),
                read_only: key.read_only,
                thread_safe: key.thread_safe,
                persistent: key.persistent,
                event: key.event,
            });
        }
    }

    defs
}

/// A helper getter/setter entry for template rendering.
#[derive(Debug, Serialize)]
struct HelperEntry {
    /// The key #define name (e.g. "DM_KEY_BATTERY_STATUS_VOLTAGE")
    define_name: String,
    /// Helper function suffix (e.g. "BATTERY_STATUS_VOLTAGE")
    helper_name: String,
    /// C type for the value (e.g. "uint16_t", "bool")
    c_type: String,
    /// dm_val_t union field (e.g. "u16val", "bval") — empty for strings
    val_field: String,
    /// True for string-type keys
    is_string: bool,
    /// True for read-only keys
    is_read_only: bool,
    /// True when this key has an `enum = "..."` reference; `c_type` is then
    /// the generated enum typedef name rather than the raw integer type.
    is_enum: bool,
    /// Raw integer C type backing an enum-typed key (unused otherwise).
    storage_c_type: String,
}

/// Collect helper entries for keys with helpers=true.
///
/// v2: an unused KeyEncoding (and the namespace ID it needed) is no longer built.
/// v3: C type, dm_val_t field and buffer flag come from DataType.
/// v4: the define and helper names come from the shared key_stem.
/// @version 4
/// @req REQ-ING-GEN-008
fn collect_helpers(model: &DataModel) -> Vec<HelperEntry> {
    let mut helpers = Vec::new();

    for class in &model.classes {
        for key in &class.keys {
            if !key.helpers {
                continue;
            }

            let helper_name = key_stem(class, model, key);
            let define_name = format!("DM_KEY_{helper_name}");

            let raw_c_type = key.data_type.c_value_type().to_string();
            let is_string = key.data_type.is_buffer();

            let is_enum = !is_string && key.enum_ref.is_some();
            let c_type = match (&key.enum_ref, is_enum) {
                (Some(enum_ref), true) => enums::enum_type_name(enum_ref, &model.meta.id),
                _ => raw_c_type.clone(),
            };

            helpers.push(HelperEntry {
                define_name,
                helper_name,
                c_type,
                val_field: key.data_type.val_field().to_string(),
                is_string,
                is_read_only: key.read_only,
                is_enum,
                storage_c_type: raw_c_type,
            });
        }
    }

    helpers
}

/// A persistence test entry for Unity test generation.
#[derive(Debug, Serialize)]
struct PersistenceTestEntry {
    define_name: String,
    c_type: String,
    val_field: String,
    is_string: bool,
    is_bool: bool,
    default_c: String,
    test_c: String,
}

/// A test case entry for Unity test generation.
#[derive(Debug, Serialize)]
struct TestKeyEntry {
    define_name: String,
    c_type: String,
    val_field: String,
    is_string: bool,
    is_bool: bool,
    read_only: bool,
    /// C literal for the default value
    default_c: String,
    /// C literal for a test value (different from default)
    test_c: String,
}

/// Collect test entries for every key in the model.
///
/// v2: defaults come from the shared c_default_literal; an unused KeyEncoding is no longer built.
/// v3: C type, dm_val_t field and buffer flag come from DataType.
/// v4: the define name comes from the shared key_stem.
/// @version 4
/// @req REQ-ING-GEN-009
fn collect_test_keys(model: &DataModel) -> Vec<TestKeyEntry> {
    let mut entries = Vec::new();

    for class in &model.classes {
        for key in &class.keys {
            let define_name = format!("DM_KEY_{}", key_stem(class, model, key));

            let default_c = c_default_literal(&key.default, key.data_type);
            let test_c = format_test_value(key.data_type, &default_c);

            entries.push(TestKeyEntry {
                define_name,
                c_type: key.data_type.c_value_type().to_string(),
                val_field: key.data_type.val_field().to_string(),
                is_string: key.data_type.is_buffer(),
                is_bool: key.data_type == DataType::Bool,
                read_only: key.read_only,
                default_c,
                test_c,
            });
        }
    }

    entries
}

/// Collect persistence test entries for persistent keys in the model.
///
/// v2: defaults come from the shared c_default_literal; an unused KeyEncoding is no longer built.
/// v3: C type, dm_val_t field and buffer flag come from DataType.
/// v4: the define name comes from the shared key_stem.
/// @version 4
/// @req REQ-ING-GEN-009
fn collect_persistence_test_entries(model: &DataModel) -> Vec<PersistenceTestEntry> {
    let mut entries = Vec::new();

    for class in &model.classes {
        for key in &class.keys {
            if !key.persistent {
                continue;
            }

            let define_name = format!("DM_KEY_{}", key_stem(class, model, key));

            let default_c = c_default_literal(&key.default, key.data_type);
            let test_c = format_test_value(key.data_type, &default_c);

            entries.push(PersistenceTestEntry {
                define_name,
                c_type: key.data_type.c_value_type().to_string(),
                val_field: key.data_type.val_field().to_string(),
                is_string: key.data_type.is_buffer(),
                is_bool: key.data_type == DataType::Bool,
                default_c,
                test_c,
            });
        }
    }

    entries
}

/// C literal of a key's default: `true`/`false`, an integer, an escaped string, or the
/// type's zero value when the key has no default.
///
/// Shared by the persistence struct initialiser and the generated Unity tests.
///
/// @version 1
/// @req REQ-ING-GEN-007
/// @req REQ-ING-GEN-009
pub(crate) fn c_default_literal(default: &Option<toml::Value>, data_type: DataType) -> String {
    match default {
        Some(toml::Value::Boolean(b)) => if *b { "true" } else { "false" }.to_string(),
        Some(toml::Value::Integer(i)) => i.to_string(),
        Some(toml::Value::String(s)) => format!("\"{}\"", s.replace('"', "\\\"")),
        _ => match data_type {
            DataType::Bool => "false".to_string(),
            DataType::String | DataType::Binary => "\"\"".to_string(),
            _ => "0".to_string(),
        },
    }
}

/// Generate a test value that's different from the default.
/// @version 1
/// @req REQ-ING-GEN-009
fn format_test_value(data_type: DataType, default_c: &str) -> String {
    match data_type {
        DataType::Bool => {
            if default_c == "true" {
                "false".to_string()
            } else {
                "true".to_string()
            }
        }
        DataType::String => "\"test_value\"".to_string(),
        DataType::Binary => "\"\\x01\\x02\"".to_string(),
        DataType::Uint8 | DataType::Uint16 | DataType::Uint32 => {
            if default_c == "42" {
                "99".to_string()
            } else {
                "42".to_string()
            }
        }
        DataType::Int8 | DataType::Int16 | DataType::Int32 => {
            if default_c == "-7" {
                "42".to_string()
            } else {
                "-7".to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_list_separates_with_comma_space() {
        assert_eq!(join_list(&[0, -1, 127]), "0, -1, 127");
    }

    #[test]
    fn join_list_of_one_and_of_none() {
        assert_eq!(join_list(&[42]), "42");
        assert_eq!(join_list::<i32>(&[]), "");
    }

    #[test]
    fn c_default_literal_of_declared_defaults() {
        use toml::Value;
        assert_eq!(
            c_default_literal(&Some(Value::Boolean(true)), DataType::Bool),
            "true"
        );
        assert_eq!(
            c_default_literal(&Some(Value::Integer(-7)), DataType::Int8),
            "-7"
        );
        assert_eq!(
            c_default_literal(&Some(Value::String("a\"b".into())), DataType::String),
            "\"a\\\"b\""
        );
    }

    #[test]
    fn c_default_literal_without_default_is_the_zero_value() {
        assert_eq!(c_default_literal(&None, DataType::Bool), "false");
        assert_eq!(c_default_literal(&None, DataType::Uint32), "0");
        assert_eq!(c_default_literal(&None, DataType::String), "\"\"");
        assert_eq!(c_default_literal(&None, DataType::Binary), "\"\"");
    }

    fn two_class_model() -> DataModel {
        toml::from_str(
            r#"
[meta]
id = "rig"
version = "1.0.0"
namespace_id = 3

[[classes]]
id = "motor"
[[classes.keys]]
id = "speed"
type = "uint16"
[[classes.keys]]
id = "fault"
type = "bool"
key_index = 9
read_only = true

[[classes]]
id = "door"
class_index = 4
[[classes.keys]]
id = "open"
type = "bool"
thread_safe = true
"#,
        )
        .unwrap()
    }

    #[test]
    fn encode_key_uses_explicit_index_or_position() {
        let model = two_class_model();
        let motor = &model.classes[0];
        // 3<<22 | 0<<17 | position 0<<7 | uint16 (2)<<3
        assert_eq!(encode_key(3, 0, 0, &motor.keys[0]), 0x00C0_0010);
        // explicit key_index 9 wins over position 1; bool type 0; read-only bit 0
        assert_eq!(encode_key(3, 0, 1, &motor.keys[1]), 0x00C0_0481);
    }

    #[test]
    fn encoded_keys_filters_in_declaration_order() {
        let model = two_class_model();
        let bools = encoded_keys(&model, 3, |k| k.data_type == DataType::Bool);
        let ids: Vec<u32> = bools.iter().map(|(id, _)| *id).collect();
        // motor.fault (class 0, index 9, read-only), door.open (class_index 4, thread-safe)
        assert_eq!(ids, vec![0x00C0_0481, 0x00C8_0004]);
        assert_eq!(bools[1].1.id, "open");
    }
}
