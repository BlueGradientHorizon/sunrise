//! Entity Part state (#796 stage 3) on the game side: where a handle's entity is now, and
//! which entity types keep Part state, checked once every type is known.
use super::Game;
use mod_api::{EntityRef, EntityResidency};

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn client_entity_residency(&self, entity: EntityRef) -> EntityResidency {
        if self.native_lifecycle.epoch() != Some(entity.epoch()) {
            return EntityResidency::Gone;
        }
        match self.native_client_entity_target(entity) {
            Some(_) => EntityResidency::InPlace,
            None => EntityResidency::Gone,
        }
    }
}
