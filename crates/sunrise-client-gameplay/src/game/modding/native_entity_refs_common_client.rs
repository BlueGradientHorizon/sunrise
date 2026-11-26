use {crate::game::client_side::remote_client::Remote, crate::game::client_side::remote_client::general::RemoteGeneral, crate::game::client_side::text_displays::Shows, sunrise_gameplay_common::game::commands_common::Target};
use mod_api::{EntityRef, LevelRef};

impl crate::game::ClientGame<'_> {
    /// The logged-in window and network id for an entity handle in its current world.
    pub(in crate::game) fn native_client_entity_context(
        &self,
        entity: EntityRef,
    ) -> Option<(&Remote, i32)> {
        let remote = self.client.remote.as_ref().filter(|r| r.logged_in)?;
        (self.common.native_lifecycle.epoch() == Some(entity.epoch())).then_some(())?;
        Some((remote, entity.id() as i32))
    }

    /// An entity tracked by the remote client but absent from local entity lists.
    pub(in crate::game) fn native_client_tracked(
        &self,
        entity: EntityRef,
    ) -> Option<(&str, [f64; 3])> {
        let (remote, id) = self.native_client_entity_context(entity)?;
        let (kind, position) = remote.tracked_entity(id)?;
        if Shows::from_kind(kind).is_some() {
            return Some((kind, self.side_client_display_position(entity)?));
        }
        Some((kind, position))
    }

    /// The client level in place as a handle: the level this window shows and ticks.
    pub(in crate::game) fn client_level_handle(&self) -> Option<LevelRef> {
        let epoch = self.common.native_lifecycle.epoch()?;
        let remote = self.client.remote.as_ref().filter(|r| r.has_level())?;
        Some(LevelRef::new(epoch, remote.level_key))
    }

    pub(in crate::game) fn native_client_general(
        &self,
        entity: EntityRef,
    ) -> Option<&RemoteGeneral> {
        let (remote, id) = self.native_client_entity_context(entity)?;
        remote.general.rows.get(&id)
    }

    /// What a handle names on a client: the local player or an entity tracked by the server.
    pub(in crate::game) fn native_client_entity_target(&self, entity: EntityRef) -> Option<Target> {
        let (remote, id) = self.native_client_entity_context(entity)?;
        if id == remote.entity_id {
            return self.common.player.as_ref().map(|_| Target::Player);
        }
        // The server tracks the dragon alone; its parts are the client's own copies of it.
        if self.common.dragon.as_ref().is_some_and(|d| {
            sunrise_gameplay_common::game::dragon_common::dragon_part_index(d.id, entity.id()).is_some()
        }) {
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
        if self.common.dragon.as_ref().is_some_and(|d| d.id == uid) {
            return Some(Target::Dragon(uid));
        }
        if self.common.mobs.iter().any(|m| m.id == uid) {
            return Some(Target::Mob(uid));
        }
        if self.common.dropped.iter().any(|e| e.id == uid) {
            return Some(Target::Item(uid));
        }
        if self.common.orbs.iter().any(|e| e.id == uid) {
            return Some(Target::Orb(uid));
        }
        if self.common.boats.iter().any(|e| e.id == uid) {
            return Some(Target::Boat(uid));
        }
        if self.common.carts.iter().any(|e| e.id == uid) {
            return Some(Target::Cart(uid));
        }
        if self.common.armor_stands.iter().any(|e| e.id == uid) {
            return Some(Target::Stand(uid));
        }
        if self.common.tnts.iter().any(|e| e.id == uid) {
            return Some(Target::Tnt(uid));
        }
        if self.common.falling.iter().any(|e| e.id == uid) {
            return Some(Target::Falling(uid));
        }
        if self.common.creatures.projectiles.iter().any(|e| e.id == uid) {
            return Some(Target::Projectile(uid));
        }
        if self.common.rockets.iter().any(|e| e.id == uid) {
            return Some(Target::Rocket(uid));
        }
        if self.common.tridents.iter().any(|e| e.id == uid) {
            return Some(Target::Trident(uid));
        }
        if self.common.crystals.iter().any(|e| e.id == uid) {
            return Some(Target::Crystal(uid));
        }
        if self.common.cushions.iter().any(|e| e.id == uid) {
            return Some(Target::Cushion(uid));
        }
        if self.common.item_frames.iter().any(|e| e.id == uid) {
            return Some(Target::Frame(uid));
        }
        if self.common.paintings.iter().any(|e| e.id == uid) {
            return Some(Target::Painting(uid));
        }
        if self.common.fangs.list.iter().any(|e| e.id == uid) {
            return Some(Target::Fangs(uid));
        }
        if self.common.bolts.iter().any(|b| b.id == uid) {
            return Some(Target::Bolt(uid));
        }
        if self.common.breath_clouds.iter().any(|e| e.id == uid) {
            return Some(Target::Cloud(uid));
        }
        if self.common.ominous_spawners.iter().any(|s| s.id == uid) {
            return Some(Target::Spawner(uid));
        }
        if self.client.interactions.contains_key(&id) {
            return Some(Target::Other(uid));
        }
        None
    }
}

use sunrise_gameplay_common::game::side_ops::player::PlayerSideGameExt as _;
