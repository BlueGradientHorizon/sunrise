//! Dual-role entity and level handle lookups shared across server and client roles.
#[allow(unused_imports)]
use super::{Game, commands::Target, remote::Remote, text_displays::Shows};
#[allow(unused_imports)]
use mod_api::{DimensionKey, EntityRef, LevelRef};
#[allow(unused_imports)]
use sunrise_core::geometry::{Aabb, Vec3};

impl Game<'_> {
    /// The logged-in window and network id for an entity handle in its current world.
    pub(in crate::game) fn native_client_entity_context(
        &self,
        entity: EntityRef,
    ) -> Option<(&Remote, i32)> {
        let remote = self.remote.as_ref().filter(|r| r.logged_in)?;
        (self.native_lifecycle.epoch() == Some(entity.epoch())).then_some(())?;
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

    /// What a handle names for a read that works on both sides.
    pub(in crate::game) fn native_entity_read_target(&self, entity: EntityRef) -> Option<Target> {
        self.side_native_entity_read_target(entity)
    }

    /// The entity `id` as a handle for the `entity_*` methods.
    pub(in crate::game) fn entity(&self, id: u32) -> Option<EntityRef> {
        let entity = EntityRef::new(self.native_lifecycle.epoch()?, id);
        (self.native_entity_read_target(entity).is_some()
            || self.native_client_tracked(entity).is_some())
        .then_some(entity)
    }

    /// Whether the entity is made of parts that are entities of their own (`Entity.isMultipartEntity`).
    pub(in crate::game) fn entity_is_multipart(&self, entity: EntityRef) -> Option<bool> {
        Some(matches!(
            self.native_entity_read_target(entity)?,
            Target::Dragon(_)
        ))
    }

    /// The entities a multipart entity is made of (`Entity.getParts`), in `getSubEntities` order.
    pub(in crate::game) fn entity_parts(&self, entity: EntityRef) -> Option<Vec<EntityRef>> {
        let Target::Dragon(id) = self.native_entity_read_target(entity)? else {
            return Some(Vec::new());
        };
        (1..=8)
            .map(|offset| self.entity(id.checked_add(offset)?))
            .collect()
    }

    /// The level `dimension` (`minecraft:the_nether`) as a handle.
    pub(in crate::game) fn level(&self, dimension: &str) -> Option<LevelRef> {
        self.side_level(dimension)
    }

    /// The client level in place as a handle: the level this window shows and ticks.
    pub(in crate::game) fn native_client_level(&self) -> Option<LevelRef> {
        let epoch = self.native_lifecycle.epoch()?;
        let remote = self.remote.as_ref().filter(|r| r.has_level())?;
        Some(LevelRef::new(epoch, remote.level_key))
    }

    /// The current world on its owning side, as a handle for a native query.
    pub(in crate::game) fn native_current_level(&self) -> Option<LevelRef> {
        self.side_native_current_level()
    }

    /// The name of a dimension's key.
    pub(in crate::game) fn dimension_key_id(&self, key: DimensionKey) -> Option<&str> {
        mod_api::__private::dimension_id(key)
    }

    /// The key of the level in place: on a client the level it shows.
    pub(in crate::game) fn native_level_key(&self) -> DimensionKey {
        self.remote
            .as_ref()
            .map_or_else(|| self.dim.key(), |r| r.level_key)
    }

    /// The entity's position (`Entity.position`); client displays use their committed tick pose.
    pub(in crate::game) fn entity_position(&self, entity: EntityRef) -> Option<[f64; 3]> {
        if self.native_lifecycle.epoch() == Some(entity.epoch())
            && let Some((id, position)) = self.native_lifecycle.entity_sections.write_position
            && id == entity.id()
        {
            return Some(position);
        }
        self.side_entity_position(entity)
    }

    /// The block the entity's position is in (`Entity.blockPosition`).
    pub(in crate::game) fn entity_block_position(&self, entity: EntityRef) -> Option<[i32; 3]> {
        self.entity_position(entity)
            .map(|p| p.map(|v| v.floor() as i32))
    }

    /// The box of a client's copy `id` of a non-mob entity at `position` (`Entity.getBoundingBox`).
    pub(in crate::game) fn client_copy_box(&self, id: u32, position: Vec3, vanilla: Aabb) -> Aabb {
        self.side_client_copy_box(id, position, vanilla)
    }
}
