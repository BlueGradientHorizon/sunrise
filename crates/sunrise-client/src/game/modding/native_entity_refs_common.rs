//! Dual-role entity and level handle lookups shared across server and client roles.
use super::{Game, commands::Target, remote::Remote, text_displays::Shows};
use mod_api::{DimensionKey, EntityRef, LevelRef};
use sunrise_core::geometry::{Aabb, Vec3};

impl<__Side: crate::game::Side> Game<'_, __Side> {
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

    /// The world whose chunks, blocks and light `level` reads on this side.
    pub(in crate::game) fn native_level_view_world(
        &self,
        level: LevelRef,
    ) -> Option<&sunrise_client::world::World> {
        self.side_level_view_world(level)
    }

    /// The block state ID at `pos` (`Level.getBlockState`).
    pub(in crate::game) fn level_block_state(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<sunrise_world::registry::BlockStateId> {
        let world = self.native_level_view_world(level)?;
        let [x, y, z] = pos;
        world.is_loaded_at(x, y, z).then(|| world.block(x, y, z))
    }

    /// The stored light of one layer at `pos` (`BlockAndLightGetter.getBrightness`), 0 to 15.
    pub(in crate::game) fn level_brightness(
        &self,
        level: LevelRef,
        layer: mod_api::LightLayer,
        pos: [i32; 3],
    ) -> Option<i32> {
        let world = self.native_level_view_world(level)?;
        world.is_loaded_at(pos[0], pos[1], pos[2]).then(|| {
            crate::game::server_side::native_level_refs::world_brightness(world, layer, pos)
        })
    }

    /// Read the mod-published connection group at `pos`.
    pub(in crate::game) fn level_connection_group(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Result<Option<u64>, mod_api::LevelReadError> {
        let world = self.native_level_view_world(level).ok_or_else(|| {
            if self.side_level_handle_current(level) {
                mod_api::LevelReadError::Unavailable
            } else {
                mod_api::LevelReadError::Stale
            }
        })?;
        Ok(world.connection_group(pos[0], pos[1], pos[2]))
    }

    pub(in crate::game) fn level_get_enchant_power_bonus(
        &self,
        level: LevelRef,
        pos: [i32; 3],
        state: sunrise_world::registry::BlockStateId,
        table_pos: [i32; 3],
    ) -> Result<f32, mod_api::LevelReadError> {
        use mod_api::LevelReadError;
        use sunrise_core::position::BlockPos;
        if self.native_level_view_world(level).is_none() {
            return Err(if self.side_level_handle_current(level) {
                LevelReadError::Unavailable
            } else {
                LevelReadError::Stale
            });
        }
        self.enchant_power(
            Some(level),
            state,
            BlockPos::new(pos[0], pos[1], pos[2]),
            BlockPos::new(table_pos[0], table_pos[1], table_pos[2]),
        )
        .ok_or(LevelReadError::Unavailable)
    }

    /// Clip an empty-context ray against collider or outline shapes, without fluids.
    pub(in crate::game) fn level_clip(
        &self,
        level: LevelRef,
        from: [f64; 3],
        to: [f64; 3],
        blocks: mod_api::ClipBlock,
        fluids: mod_api::ClipFluid,
    ) -> Option<mod_api::ClipHit> {
        use mod_api::{BlockFace, ClipBlock, ClipFluid, ClipHit};
        use sunrise_client::collisions::SceneCollisions;
        use sunrise_core::geometry::Direction;
        use sunrise_world::{
            collision_context::CollisionContext,
            world_ray::{RayBlockMode, RayFluidMode, clip_world},
        };
        let world = self.native_level_view_world(level)?;
        let mut rays = crate::game::server_side::explosion::ColliderRays {
            scene: SceneCollisions {
                world,
                binding: self.collision,
                subject: None,
            },
            context: CollisionContext::empty(false),
            clip_data: Some(crate::game::server_side::explosion::ClipData {
                facts: self.res.facts(),
                behaviours: self.res.behaviours(),
                tables: &self.pack.context.tables,
                registry: self.pack.assets.registry,
            }),
        };
        let mode = match blocks {
            ClipBlock::Collider => RayBlockMode::Collider,
            ClipBlock::Outline => RayBlockMode::Outline,
            _ => unreachable!("ClipBlock: unhandled variant"),
        };
        let fluid = match fluids {
            ClipFluid::None => RayFluidMode::None,
            _ => unreachable!("ClipFluid: unhandled variant"),
        };
        let result = clip_world(
            &mut rays,
            Vec3::new(from[0], from[1], from[2]),
            Vec3::new(to[0], to[1], to[2]),
            mode,
            fluid,
            4096,
        )
        .ok()?;
        let h = result.hit;
        let face = match h.direction {
            Direction::Down => BlockFace::Down,
            Direction::Up => BlockFace::Up,
            Direction::North => BlockFace::North,
            Direction::South => BlockFace::South,
            Direction::West => BlockFace::West,
            Direction::East => BlockFace::East,
        };
        Some(ClipHit {
            miss: result.miss,
            block: [h.block.x, h.block.y, h.block.z],
            face,
            location: [h.location.x, h.location.y, h.location.z],
            inside: h.inside,
            world_border: false,
        })
    }

    /// Visit intersecting, readable entities in `Level.getEntities` query order.
    pub(in crate::game) fn level_visit_entities_in(
        &self,
        level: LevelRef,
        min: [f64; 3],
        max: [f64; 3],
        visit: &mut dyn FnMut(EntityRef) -> core::ops::ControlFlow<()>,
    ) -> Result<core::ops::ControlFlow<()>, mod_api::LevelReadError> {
        self.side_level_visit_entities_in(level, min, max, visit)
    }

    /// Where `entity` is resident now (`EntityPartState` / `EntityResidency`).
    pub(in crate::game) fn entity_residency(&self, entity: EntityRef) -> mod_api::EntityResidency {
        self.side_entity_residency(entity)
    }

    /// The entity's type (`Entity.getType`): `minecraft:zombie`. API 373.
    pub(in crate::game) fn entity_kind(&self, entity: EntityRef) -> Option<mod_api::EntityTypeKey> {
        self.side_entity_kind(entity)
    }

    /// The level the entity is in (`Entity.level`).
    pub(in crate::game) fn entity_level(&self, entity: EntityRef) -> Option<LevelRef> {
        self.side_entity_level(entity)
    }

    /// The entity's bounding box (`Entity.getBoundingBox`).
    pub(in crate::game) fn entity_bounding_box(&self, entity: EntityRef) -> Option<[[f64; 3]; 2]> {
        self.side_entity_bounding_box(entity)
    }

    /// The entity's velocity in blocks per tick (`Entity.getDeltaMovement`).
    pub(in crate::game) fn entity_velocity(&self, entity: EntityRef) -> Option<[f64; 3]> {
        self.side_entity_velocity(entity)
    }

    /// The entity's eye height above its feet (`Entity.getEyeHeight`).
    pub(in crate::game) fn entity_eye_height(&self, entity: EntityRef) -> Option<f64> {
        self.side_entity_eye_height(entity)
    }

    /// The entity's eye position (`Entity.getEyePosition`).
    pub(in crate::game) fn entity_eye_position(&self, entity: EntityRef) -> Option<[f64; 3]> {
        let [x, y, z] = self.entity_position(entity)?;
        Some([x, y + self.entity_eye_height(entity)?, z])
    }

    /// Whether the entity is holding its shift key down (`Entity.isShiftKeyDown`).
    pub(in crate::game) fn entity_is_shift_key_down(&self, entity: EntityRef) -> Option<bool> {
        self.side_entity_is_shift_key_down(entity)
    }

    /// The entity's main hand (`LivingEntity.getMainArm` / `HumanoidArm`).
    pub(in crate::game) fn entity_main_arm(&self, entity: EntityRef) -> Option<mod_api::MainHand> {
        self.side_entity_main_arm(entity)
    }

    /// The entity as a living entity (`LivingEntity`), or None for another kind or a stale handle.
    pub(in crate::game) fn living(&self, entity: EntityRef) -> Option<mod_api::LivingRef> {
        let living = mod_api::LivingRef::new(entity);
        self.native_living_read_target(living).map(|_| living)
    }

    /// Resolve a living entity handle for reads on either side.
    pub(in crate::game) fn native_living_read_target(
        &self,
        living: mod_api::LivingRef,
    ) -> Option<Target> {
        self.side_living_read_target(living)
    }

    /// Computed value of `attribute` on `living`.
    pub(in crate::game) fn living_attribute_value(
        &self,
        living: mod_api::LivingRef,
        attribute: &str,
    ) -> Option<f64> {
        self.side_living_attribute_value(living, attribute)
    }

    /// Equipment stack held in `slot` on `living`.
    pub(in crate::game) fn living_equipment_stack(
        &self,
        living: mod_api::LivingRef,
        slot: mod_api::EquipmentSlot,
    ) -> Option<sunrise_client::mobs::finalize::EquipmentRead<'_>> {
        self.side_living_equipment_stack(living, slot)
    }
}
