//! The per-variant bounds `dsh_tables.h` states for a shadow consumer's buffers (shadow table
//! interface minor 8): rejected-list members, desired path length, published error ids and the
//! largest error id.

use super::super::error_groups::ErrorGroup;
use super::super::model::Tables;
use super::desired::forms;
use serde::Serialize;

/// The four bounds of one variant.
#[derive(Serialize)]
pub struct Bounds {
    /// Members one rejected-settings list can hold: one per desired and echo path; 0 without a
    /// rejected list.
    rejected_max_items: usize,
    /// The longest desired or echo path with its terminator; 0 with none.
    desired_path_max_bytes: usize,
    /// The error ids the full error table publishes.
    error_ids_published: usize,
    /// The largest wire id an error payload can carry; 0 with none.
    error_id_max: u32,
}

/// The bounds of the variant `tables` holds.
///
/// @version 1
/// @req REQ-ING-DSH-006
pub fn of(tables: &Tables) -> Bounds {
    let paths = desired_paths(tables);
    let rejected_max_items = if tables.shadow.rejected.is_some() {
        paths.len()
    } else {
        0
    };
    Bounds {
        rejected_max_items,
        desired_path_max_bytes: paths.iter().map(|p| p.len() + 1).max().unwrap_or(0),
        error_ids_published: tables
            .error_groups
            .iter()
            .map(|g| published(g).count())
            .sum(),
        error_id_max: tables
            .error_groups
            .iter()
            .filter_map(|g| highest_id(tables, g))
            .max()
            .unwrap_or(0),
    }
}

/// Every path of `dsh_desired` (each shadow form of a SETTINGS key) and `dsh_echo`.
///
/// @version 1
/// @req REQ-ING-DSH-006
fn desired_paths(tables: &Tables) -> Vec<&str> {
    let desired = forms(tables).map(|(_, f)| f.path.as_str());
    let echo = tables.shadow.echo.iter().map(|e| e.path.as_str());
    desired.chain(echo).collect()
}

/// The bits of group `g` the full error table publishes: 0..=publish_max_bit, not reserved.
///
/// @version 1
/// @req REQ-ING-DSH-006
fn published(g: &ErrorGroup) -> impl Iterator<Item = u8> + '_ {
    (0..=g.publish_max_bit).filter(|b| !g.reserved_bits.contains(b))
}

/// The largest wire id of group `g`: its highest published bit or highest declared error bit.
///
/// @version 1
/// @req REQ-ING-DSH-006
fn highest_id(tables: &Tables, g: &ErrorGroup) -> Option<u32> {
    let declared = tables
        .keys
        .iter()
        .filter_map(|k| k.error.as_ref().filter(|e| e.group == g.id).map(|e| e.bit));
    published(g)
        .chain(declared)
        .max()
        .map(|bit| g.wire_base + u32::from(bit))
}
