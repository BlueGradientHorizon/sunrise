#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game, commands::Target, remote::Remote, text_displays::Shows,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_entity_refs_common::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{DimensionKey, EntityRef, LevelRef};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::{Aabb, Vec3};

impl Game<'_> {
    /// What a handle names on a client: the local player or an entity tracked by the server.
    pub(in crate::game) fn native_client_entity_target(&self, entity: EntityRef) -> Option<Target> {
        let (remote, id) = self.native_client_entity_context(entity)?;
        if id == remote.entity_id {
            return self.player.as_ref().map(|_| Target::Player);
        }
        // The server tracks the dragon alone; its parts are the client's own copies of it.
        if self.resident_dragon_part(entity.id()).is_some() {
            return Some(Target::DragonPart(entity.id()));
        }
        let (kind, _) = remote.tracked(id)?;
        if Shows::from_kind(kind).is_some() {
            // Displays have their own client bodies; never resolve one through another store.
            return None;
        }
        self.level_entity(id)
    }
}
