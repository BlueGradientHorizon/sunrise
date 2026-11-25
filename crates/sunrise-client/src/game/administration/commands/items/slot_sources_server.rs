#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Owner, R, err};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::items::slot_sources::expand;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::items::slot_sources::id;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::items::slot_sources::limit;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::items::slot_sources::typed_source;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::items::slot_sources::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use serde_json::Value;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::collections::BTreeSet;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::datapack::DataStore;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::Tag;

impl Game<'_> {
    /// Literal ranges retain priority; all remaining consumers resolve the same holder snapshot.
    pub(in crate::game::server_side) fn resolved_slot_source(&self, raw: &str) -> R<Option<Value>> {
        if sunrise_client::command::args::slot_ranges()
            .iter()
            .any(|(name, _)| name == raw)
        {
            return Ok(None);
        }
        let value = if self.fns.0.store.slot_sources.contains_key(&id(raw)) {
            Value::String(id(raw))
        } else {
            let (tag, _) = sunrise_core::nbt::parse_snbt(raw)
                .map_err(|error| err("argument.resource_or_id.failed_to_parse", &[&error]))?;
            typed_source(&tag)
        };
        expand(&self.fns.0.store, &value)
            .map(Some)
            .map_err(|error| err("argument.resource_or_id.failed_to_parse", &[&error]))
    }
}
