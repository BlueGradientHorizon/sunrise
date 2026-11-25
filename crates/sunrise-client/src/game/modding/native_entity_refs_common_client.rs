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
        if self
            .dragon
            .as_ref()
            .is_some_and(|d| crate::game::dragon::dragon_part_index(d.id, entity.id()).is_some())
        {
            return Some(Target::DragonPart(entity.id()));
        }
        // General rows are not inserted into `remote.entities`, so check them before `remote.tracked`.
        if remote.general.rows.contains_key(&id) {
            return Some(Target::General(entity.id()));
        }
        let (kind, _) = remote.tracked(id)?;
        if Shows::from_kind(kind).is_some() {
            // Displays have their own client bodies; never resolve one through another store.
            return None;
        }
        self.client_level_entity(id)
    }

    /// Resolve a tracked client entity's typed target across every body store a client holds.
    pub(in crate::game::client_side) fn client_level_entity(&self, id: i32) -> Option<Target> {
        let uid = u32::try_from(id).ok()?;
        if self.dragon.as_ref().is_some_and(|d| d.id == uid) {
            return Some(Target::Dragon(uid));
        }
        if self.mobs.iter().any(|m| m.id == uid) {
            return Some(Target::Mob(uid));
        }
        if self.dropped.iter().any(|e| e.id == uid) {
            return Some(Target::Item(uid));
        }
        if self.orbs.iter().any(|e| e.id == uid) {
            return Some(Target::Orb(uid));
        }
        if self.boats.iter().any(|e| e.id == uid) {
            return Some(Target::Boat(uid));
        }
        if self.carts.iter().any(|e| e.id == uid) {
            return Some(Target::Cart(uid));
        }
        if self.armor_stands.iter().any(|e| e.id == uid) {
            return Some(Target::Stand(uid));
        }
        if self.tnts.iter().any(|e| e.id == uid) {
            return Some(Target::Tnt(uid));
        }
        if self.falling.iter().any(|e| e.id == uid) {
            return Some(Target::Falling(uid));
        }
        if self.creatures.projectiles.iter().any(|e| e.id == uid) {
            return Some(Target::Projectile(uid));
        }
        if self.rockets.iter().any(|e| e.id == uid) {
            return Some(Target::Rocket(uid));
        }
        if self.tridents.iter().any(|e| e.id == uid) {
            return Some(Target::Trident(uid));
        }
        if self.crystals.iter().any(|e| e.id == uid) {
            return Some(Target::Crystal(uid));
        }
        if self.cushions.iter().any(|e| e.id == uid) {
            return Some(Target::Cushion(uid));
        }
        if self.item_frames.iter().any(|e| e.id == uid) {
            return Some(Target::Frame(uid));
        }
        if self.paintings.iter().any(|e| e.id == uid) {
            return Some(Target::Painting(uid));
        }
        if self.fangs.iter().any(|e| e.id == uid) {
            return Some(Target::Fangs(uid));
        }
        if self.bolts.iter().any(|b| b.id == uid) {
            return Some(Target::Bolt(uid));
        }
        if self.breath_clouds.iter().any(|e| e.id == uid) {
            return Some(Target::Cloud(uid));
        }
        if self.ominous_spawners.iter().any(|s| s.id == uid) {
            return Some(Target::Spawner(uid));
        }
        if self.client.interactions.contains_key(&id) {
            return Some(Target::Other(uid));
        }
        None
    }
}
