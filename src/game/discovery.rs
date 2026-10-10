//! Reducer-owned world discovery primitives shared by items, abilities, and future content.

use crate::data::datapacks::DatapackBundle;

use super::queries::find_location;
use super::state::RunState;

/// Marks every connection from the current location known and returns only the newly revealed
/// display names. Presentation callers decide how to describe this fact; they never mutate the
/// known-location ledger themselves.
pub fn reveal_current_connections(
    state: &mut RunState,
    bundle: &DatapackBundle,
) -> Result<Vec<String>, String> {
    let Some(current_location) = find_location(bundle, &state.current_location_id) else {
        return Err("Current location could not be resolved.".to_owned());
    };
    let mut newly_known = Vec::new();
    for connection_id in &current_location.connections {
        if state.known_locations.insert(connection_id.clone()) {
            if let Some(instance) = state.location_instances.get_mut(connection_id) {
                instance.known = true;
            }
            let name = find_location(bundle, connection_id)
                .map(|location| location.name.clone())
                .unwrap_or_else(|| connection_id.clone());
            newly_known.push(name);
        }
    }
    Ok(newly_known)
}
