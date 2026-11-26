//! Dual-role entity and level handle lookups shared across server and client roles.

use crate::game::{
    collider_rays::{ClipData, ColliderRays},
    commands_common::Target,
};
use core::ops::ControlFlow;
use mod_api::{
    BlockFace, BlockShapeKind, DimensionKey, EntityRayHit, EntityRef, EntityTypeKey,
    LevelReadError, LevelRef, MobRef, PushReaction,
};
use sunrise_core::geometry::{Aabb, Vec3};
use sunrise_items::items::ItemStack;
use sunrise_sim::join_seq::residency::table::{EntityLookup, Location};
use sunrise_world::registry::BlockStateId;

impl crate::game::CommonGame<'_> {

    /// The name of a dimension's key.
    pub fn dimension_key_id(&self, key: DimensionKey) -> Option<&str> {
        mod_api::__private::dimension_id(key)
    }
}

pub trait NativeEntityRefsCommonGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// What a handle names for a read that works on both sides.
     fn native_entity_read_target(&self, entity: EntityRef) -> Option<Target> {
        self.side_native_entity_read_target(entity)
    }

    /// The entity `id` as a handle for the `entity_*` methods.
     fn entity(&self, id: u32) -> Option<EntityRef> {
        let entity = EntityRef::new(self.native_lifecycle.epoch()?, id);
        (self.native_entity_read_target(entity).is_some()
            || self.side_native_client_tracked(entity).is_some())
        .then_some(entity)
    }

    /// Whether the entity is made of parts that are entities of their own (`Entity.isMultipartEntity`).
     fn entity_is_multipart(&self, entity: EntityRef) -> Option<bool> {
        Some(matches!(
            self.native_entity_read_target(entity)?,
            Target::Dragon(_)
        ))
    }

    /// The entities a multipart entity is made of (`Entity.getParts`), in `getSubEntities` order.
     fn entity_parts(&self, entity: EntityRef) -> Option<Vec<EntityRef>> {
        let Target::Dragon(id) = self.native_entity_read_target(entity)? else {
            return Some(Vec::new());
        };
        (1..=8)
            .map(|offset| self.entity(id.checked_add(offset)?))
            .collect()
    }

    /// The level `dimension` (`minecraft:the_nether`) as a handle.
     fn level(&self, dimension: &str) -> Option<LevelRef> {
        self.side_level(dimension)
    }

    /// The client level in place as a handle.
    #[inline(always)]
     fn native_client_level(&self) -> Option<LevelRef> {
        self.side_native_client_level()
    }

    /// The key of the level in place: on a client the level it shows.
     fn native_level_key(&self) -> DimensionKey {
        self.side_native_level_key()
    }

    /// The current world on its owning side, as a handle for a native query.
     fn native_current_level(&self) -> Option<LevelRef> {
        self.side_native_current_level()
    }

    /// The entity's position (`Entity.position`); client displays use their committed tick pose.
     fn entity_position(&self, entity: EntityRef) -> Option<[f64; 3]> {
        if self.native_lifecycle.epoch() == Some(entity.epoch())
            && let Some((id, position)) = self.native_lifecycle.entity_sections.write_position
            && id == entity.id()
        {
            return Some(position);
        }
        self.side_entity_position(entity)
    }

    /// The block the entity's position is in (`Entity.blockPosition`).
     fn entity_block_position(&self, entity: EntityRef) -> Option<[i32; 3]> {
        self.entity_position(entity)
            .map(|p| p.map(|v| v.floor() as i32))
    }

    /// The box of a client's copy `id` of a non-mob entity at `position` (`Entity.getBoundingBox`).
     fn client_copy_box(&self, id: u32, position: Vec3, vanilla: Aabb) -> Aabb {
        self.side_client_copy_box(id, position, vanilla)
    }

    /// The world whose chunks, blocks and light `level` reads on this side.
     fn native_level_view_world(
        &self,
        level: LevelRef,
    ) -> Option<&sunrise_level::world::World> {
        self.side_level_view_world(level)
    }

    /// The block state ID at `pos` (`Level.getBlockState`).
     fn level_block_state(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<sunrise_world::registry::BlockStateId> {
        let world = self.native_level_view_world(level)?;
        let [x, y, z] = pos;
        world.is_loaded_at(x, y, z).then(|| world.block(x, y, z))
    }

    /// The stored light of one layer at `pos` (`BlockAndLightGetter.getBrightness`), 0 to 15.
     fn level_brightness(
        &self,
        level: LevelRef,
        layer: mod_api::LightLayer,
        pos: [i32; 3],
    ) -> Option<i32> {
        let world = self.native_level_view_world(level)?;
        world
            .is_loaded_at(pos[0], pos[1], pos[2])
            .then(|| crate::game::native_world_read_common::world_brightness(world, layer, pos))
    }

    /// Read the mod-published connection group at `pos`.
     fn level_connection_group(
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

     fn level_get_enchant_power_bonus(
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
     fn level_clip(
        &self,
        level: LevelRef,
        from: [f64; 3],
        to: [f64; 3],
        blocks: mod_api::ClipBlock,
        fluids: mod_api::ClipFluid,
    ) -> Option<mod_api::ClipHit> {
        use mod_api::{BlockFace, ClipBlock, ClipFluid, ClipHit};
        use sunrise_core::geometry::Direction;
        use sunrise_level::collisions::SceneCollisions;
        use sunrise_world::{
            collision_context::CollisionContext,
            world_ray::{RayBlockMode, RayFluidMode, clip_world},
        };
        let world = self.native_level_view_world(level)?;
        let mut rays = ColliderRays {
            scene: SceneCollisions {
                world,
                binding: self.collision,
                subject: None,
            },
            context: CollisionContext::empty(false),
            clip_data: Some(ClipData {
                facts: self.res.facts(),
                behaviours: self.res.behaviours(),
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
     fn level_visit_entities_in(
        &self,
        level: LevelRef,
        min: [f64; 3],
        max: [f64; 3],
        visit: &mut dyn FnMut(EntityRef) -> core::ops::ControlFlow<()>,
    ) -> Result<core::ops::ControlFlow<()>, mod_api::LevelReadError> {
        self.side_level_visit_entities_in(level, min, max, visit)
    }

    /// Where `entity` is resident now (`EntityPartState` / `EntityResidency`).
     fn entity_residency(&self, entity: EntityRef) -> mod_api::EntityResidency {
        self.side_entity_residency(entity)
    }

    /// The entity's type (`Entity.getType`): `minecraft:zombie`. API 373.
     fn entity_kind(&self, entity: EntityRef) -> Option<mod_api::EntityTypeKey> {
        self.side_entity_kind(entity)
    }

    /// The level the entity is in (`Entity.level`).
     fn entity_level(&self, entity: EntityRef) -> Option<LevelRef> {
        self.side_entity_level(entity)
    }

    /// The entity's bounding box (`Entity.getBoundingBox`).
     fn entity_bounding_box(&self, entity: EntityRef) -> Option<[[f64; 3]; 2]> {
        self.side_entity_bounding_box(entity)
    }

    /// The entity's velocity in blocks per tick (`Entity.getDeltaMovement`).
     fn entity_velocity(&self, entity: EntityRef) -> Option<[f64; 3]> {
        self.side_entity_velocity(entity)
    }

    /// The entity's eye height above its feet (`Entity.getEyeHeight`).
     fn entity_eye_height(&self, entity: EntityRef) -> Option<f64> {
        self.side_entity_eye_height(entity)
    }

    /// The entity's eye position (`Entity.getEyePosition`).
     fn entity_eye_position(&self, entity: EntityRef) -> Option<[f64; 3]> {
        let [x, y, z] = self.entity_position(entity)?;
        Some([x, y + self.entity_eye_height(entity)?, z])
    }

    /// Whether the entity is holding its shift key down (`Entity.isShiftKeyDown`).
     fn entity_is_shift_key_down(&self, entity: EntityRef) -> Option<bool> {
        self.side_entity_is_shift_key_down(entity)
    }

    /// The entity's main hand (`LivingEntity.getMainArm` / `HumanoidArm`).
     fn entity_main_arm(&self, entity: EntityRef) -> Option<mod_api::MainHand> {
        self.side_entity_main_arm(entity)
    }

    /// The entity as a living entity (`LivingEntity`), or None for another kind or a stale handle.
     fn living(&self, entity: EntityRef) -> Option<mod_api::LivingRef> {
        let living = mod_api::LivingRef::new(entity);
        self.native_living_read_target(living).map(|_| living)
    }

    /// Resolve a living entity handle for reads on either side.
     fn native_living_read_target(
        &self,
        living: mod_api::LivingRef,
    ) -> Option<Target> {
        self.side_living_read_target(living)
    }

    /// Computed value of `attribute` on `living`.
     fn living_attribute_value(
        &self,
        living: mod_api::LivingRef,
        attribute: &str,
    ) -> Option<f64> {
        self.side_living_attribute_value(living, attribute)
    }

    /// Equipment stack held in `slot` on `living`.
     fn living_equipment_stack(
        &self,
        living: mod_api::LivingRef,
        slot: mod_api::EquipmentSlot,
    ) -> Option<sunrise_sim::mobs::finalize::EquipmentRead<'_>> {
        self.side_living_equipment_stack(living, slot)
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeEntityRefsCommonGameExt1<'__sg> for G {}


pub trait NativeEntityRefsCommonGameExt2<'__sg>: crate::game::SideGame<'__sg> {
    /// The id of the entity `t` rides, if it rides one.
     fn native_vehicle_id(&self, t: Target) -> Option<u32> {
        match t {
            Target::Player => self.riding,
            Target::Guest(conn) => self.command_guest(conn)?.vehicle,
            Target::Mob(id) => self.mob_object(id).filter(|m| !m.removed)?.extra.mount,
            Target::General(id) => self.general_row(id)?.mount,
            _ => None,
        }
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeEntityRefsCommonGameExt2<'__sg> for G {}


pub trait NativeEntityRefsCommonGameExt3<'__sg>: crate::game::SideGame<'__sg> {
    fn native_kind_for_physical_read<'a>(
        &'a self,
        entity: EntityRef,
        target: Option<Target>,
    ) -> Option<&'a str>
    where
        '__sg: 'a,
    {
        match target {
            Some(target) => self.native_entity_kind_name(target),
            None => self
                .side_native_client_tracked(entity)
                .map(|(kind, _)| kind)
                .or_else(|| {
                    (self.native_lifecycle.epoch() == Some(entity.epoch())).then_some(())?;
                    self.mob_in_other_level(entity.id())
                        .map(|(_, mob)| mob.kind.id.as_str())
                }),
        }
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeEntityRefsCommonGameExt3<'__sg> for G {}


pub trait NativeEntityRefsCommonGameExt4<'__sg>: crate::game::SideGame<'__sg> {
     fn native_entity_kind_key(&self, target: Target) -> Option<EntityTypeKey> {
        // A living entity's type key was resolved with the accepted resources; other kinds
        // resolve their name.
        let keys = self.res.living_type_keys();
        match target {
            Target::Player | Target::Guest(_) => keys.player,
            Target::Mob(id) => self.mobs.iter().find(|mob| mob.id == id)?.kind.type_key,
            Target::Stand(_) => keys.armor_stand,
            Target::Dragon(_) | Target::DragonPart(_) => keys.ender_dragon,
            target => {
                mod_api::__private::resolved_entity_type_key(self.native_entity_kind_name(target)?)
            }
        }
    }

    fn native_entity_kind_name<'__loan>(&'__loan self, target: Target) -> Option<&str> where '__sg: '__loan {
        Some(match target {
            Target::Player | Target::Guest(_) => super::native_living_body::PLAYER,
            Target::Mob(id) => super::native_living_body::body_kind_name(
                sunrise_sim::health::LivingBody::Mob(self.mobs.iter().find(|mob| mob.id == id)?),
            ),
            Target::Item(_) => "minecraft:item",
            Target::Orb(_) => "minecraft:experience_orb",
            Target::Projectile(id) => self
                .creatures
                .projectiles
                .iter()
                .find(|p| p.id == id)?
                .entity_type(),
            Target::Stand(_) => super::native_living_body::ARMOR_STAND,
            Target::General(id) => self.general_kind(id)?,
            Target::Frame(id) => {
                if self
                    .item_frames
                    .iter()
                    .any(|frame| frame.id == id && frame.glow)
                {
                    "minecraft:glow_item_frame"
                } else {
                    "minecraft:item_frame"
                }
            }
            Target::Painting(_) => "minecraft:painting",
            Target::Boat(id) => self.boats.iter().find(|b| b.id == id)?.entity_type_name(),
            Target::Cart(id) => self.carts.iter().find(|cart| cart.id == id)?.kind.id(),
            Target::Other(id) => {
                crate::game::side_ops::player::SideOps::side_other_entity_kind(self.side_state(), id)?
            }
            Target::Crystal(_) => "minecraft:end_crystal",
            Target::Dragon(_) | Target::DragonPart(_) => super::native_living_body::ENDER_DRAGON,
            Target::Tnt(_) => "minecraft:tnt",
            Target::Cushion(_) => "minecraft:cushion",
            Target::Falling(_) => "minecraft:falling_block",
            Target::Rocket(_) => "minecraft:firework_rocket",
            Target::Trident(_) => "minecraft:trident",
            Target::Fangs(_) => "minecraft:evoker_fangs",
            Target::Spawner(_) => "minecraft:ominous_item_spawner",
            Target::Cloud(_) => "minecraft:area_effect_cloud",
            Target::Bolt(_) => "minecraft:lightning_bolt",
            Target::Hook(_) => "minecraft:fishing_bobber",
            Target::Knot(_) => "minecraft:leash_knot",
        })
    }

    /// `Entity.getPistonPushReaction`; only the armor stand's override depends on instance state.
     fn entity_piston_push_reaction(
        &self,
        entity: EntityRef,
    ) -> Result<Option<PushReaction>, LevelReadError> {
        use super::api_values::api_push_reaction;
        use super::entity::{Entity, with_entity};
        use super::general::general_type_catalog;

        let target = self.native_entity_read_target(entity);
        if let Some(handle) = target.and_then(|_| self.native_walk_entity(entity.id())) {
            let reaction = with_entity!(self, handle, row => row.get_piston_push_reaction());
            return Ok(Some(api_push_reaction(reaction)));
        }
        if let Some(slot) = self.side_native_client_general_type_slot(entity) {
            // A tracked general row has no Entity body; both sides use its declared type.
            let reaction = general_type_catalog().piston_push_reaction(slot);
            return Ok(Some(api_push_reaction(reaction)));
        }
        if target.is_none()
            && self.native_lifecycle.epoch() == Some(entity.epoch())
            && let Some((_, mob)) = self.mob_in_other_level(entity.id())
        {
            return Ok(Some(api_push_reaction(mob.get_piston_push_reaction())));
        }
        Ok(self
            .native_kind_for_physical_read(entity, target)
            .map(|kind| match kind {
                // These kinds have no Entity body for the engine's piston to push.
                "minecraft:marker"
                | "minecraft:interaction"
                | "minecraft:block_display"
                | "minecraft:item_display"
                | "minecraft:text_display"
                | "minecraft:area_effect_cloud"
                | "minecraft:ominous_item_spawner" => PushReaction::IgnoreEntity,
                _ => PushReaction::PushPull,
            }))
    }

     fn entity_is_hanging(
        &self,
        entity: EntityRef,
    ) -> Result<Option<bool>, LevelReadError> {
        Ok(self
            .native_kind_for_physical_read(entity, self.native_entity_read_target(entity))
            .map(|kind| {
                matches!(
                    kind,
                    "minecraft:painting" | "minecraft:item_frame" | "minecraft:glow_item_frame"
                )
            }))
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeEntityRefsCommonGameExt4<'__sg> for G {}


/// `ProjectileUtil.getEntityHitResult`'s test on a candidate: grow its box and clip it
/// against the segment. A segment starting inside the grown box finds no entry (`AABB.clip`).
pub fn clip_entity_ray(
    start: Vec3,
    end: Vec3,
    padding: Vec3,
    entity: EntityRef,
    [lo, hi]: [[f64; 3]; 2],
) -> Option<(EntityRayHit, f64)> {
    let grown = Aabb::new(
        Vec3::new(lo[0], lo[1], lo[2]),
        Vec3::new(hi[0], hi[1], hi[2]),
    )
    .inflate(padding);
    let location = grown.clip(start, end)?.components();
    let from = start.components();
    let distance_squared = (0..3).map(|i| (location[i] - from[i]).powi(2)).sum::<f64>();
    Some((
        EntityRayHit {
            entity,
            location,
            distance: distance_squared.sqrt(),
        },
        distance_squared,
    ))
}

pub trait NativeEntityRefsCommonGameExt5<'__sg>: crate::game::SideGame<'__sg> {
    /// What a handle names now, or None for a stale or foreign one.
     fn native_entity_target(&self, entity: EntityRef) -> Option<Target> {
        self.native_entity_target_with_location(entity)
            .map(|(target, _)| target)
    }

    /// Resolve the handle and return its resident location, if admitted to the level.
     fn native_entity_target_with_location(
        &self,
        entity: EntityRef,
    ) -> Option<(Target, Option<Location>)> {
        self.native_entity_target_in(entity, self.dim.key())
    }

    /// [`Self::native_entity_target_with_location`] for an entity resident in the open server
    /// level `dimension`, the one in place or a parked one.
     fn native_entity_target_in(
        &self,
        entity: EntityRef,
        dimension: DimensionKey,
    ) -> Option<(Target, Option<Location>)> {
        if crate::game::Side::held_out(self.side_state()).contains(&entity.id())
            || !self.is_logical_server()
            || self.native_lifecycle.epoch() != Some(entity.epoch())
        {
            return None;
        }
        let EntityLookup::Found { location, value } =
            <Self::State as crate::game::side_ops::world::SideOps>::side_server_resident_entity(
                self,
                entity.id(),
            )
        else {
            // A lifecycle event lends the body without admitting it to the level.
            let &(_, conn, _) = self
                .player_state
                .cloning_players
                .iter()
                .rev()
                .find(|&&(_, _, id)| id == entity.id())?;
            return if crate::game::Side::lan_acting(self.side_state()) == Some(conn) {
                Some((Target::Player, None))
            } else if crate::game::Side::lan_guest_swaps(self.side_state()).contains(&conn) {
                None
            } else {
                Some((Target::Guest(conn), None))
            };
        };
        // Players are server-wide records. Ordinary body operations retain
        // their existing current-level domain; source membership is global.
        (matches!(value, Target::Player | Target::Guest(_))
            || location.owner.dimension == dimension)
            .then_some((value, Some(location)))
    }

     fn level_ray_trace_nearest_entity(
        &self,
        level: LevelRef,
        from: [f64; 3],
        to: [f64; 3],
        margin: f64,
        exclude: Option<EntityRef>,
        accept: &mut dyn FnMut(EntityRef) -> bool,
    ) -> Result<Option<EntityRayHit>, LevelReadError> {
        let mut nearest = None;
        self.visit_entity_ray_hits(
            level,
            from,
            to,
            margin,
            exclude,
            accept,
            |hit, distance_squared| {
                if nearest.is_none_or(|(_, best)| distance_squared < best) {
                    nearest = Some((hit, distance_squared));
                }
            },
        )?;
        Ok(nearest.map(|(hit, _)| hit))
    }

     fn visit_entity_ray_hits(
        &self,
        level: LevelRef,
        from: [f64; 3],
        to: [f64; 3],
        margin: f64,
        exclude: Option<EntityRef>,
        mut accept: impl FnMut(EntityRef) -> bool,
        mut visit: impl FnMut(EntityRayHit, f64),
    ) -> Result<(), LevelReadError> {
        if !(margin.is_finite() && from.iter().chain(&to).all(|v| v.is_finite())) {
            return Err(LevelReadError::Unavailable);
        }
        // Every box the inflated or deflated test can hit meets the segment's own box
        // padded by `margin.abs()` (including over-deflated boxes swapped by `Aabb::new`).
        let pad = margin.abs();
        let min = std::array::from_fn(|i| from[i].min(to[i]) - pad);
        let max = std::array::from_fn(|i| from[i].max(to[i]) + pad);
        let start = Vec3::new(from[0], from[1], from[2]);
        let end = Vec3::new(to[0], to[1], to[2]);
        let padding = Vec3::new(margin, margin, margin);
        let _ = self.level_visit_entities_in(level, min, max, &mut |entity| {
            // Match `ProjectileUtil.getEntityHitResult`: filter candidates before reading and
            // clipping their bounding boxes. The walk releases any residency borrow before
            // calling mod code.
            if Some(entity) != exclude
                && accept(entity)
                && let Some(bounds) = self.entity_bounding_box(entity)
                && let Some((hit, distance_squared)) =
                    clip_entity_ray(start, end, padding, entity, bounds)
            {
                visit(hit, distance_squared);
            }
            ControlFlow::Continue(())
        })?;
        Ok(())
    }

    /// `ItemStack.getDestroySpeed` of `tool` on the block at `pos`.
     fn level_block_break_speed(
        &self,
        level: LevelRef,
        pos: [i32; 3],
        tool: Option<&ItemStack>,
    ) -> Option<f32> {
        let world = self.native_level_read_world(level)?;
        if !world.loaded(pos[0], pos[2]) || !world.shape.contains_y(pos[1]) {
            return None;
        }
        let state = world.block(pos[0], pos[1], pos[2]);
        let name = &self.res.facts().get(state).name;
        Some(self.items().mining_speed(tool, name))
    }

    /// The supplied state's block-local outline or collision boxes at a level
    /// position with an empty context. The world is never modified. Returns None
    /// for a stale or unavailable level. Unloaded neighbors
    /// read as air; collision errors return no boxes, like `block_collision`.
     fn level_block_shape(
        &self,
        level: LevelRef,
        state: BlockStateId,
        pos: [i32; 3],
        kind: BlockShapeKind,
    ) -> Option<Vec<[f64; 6]>> {
        if !self.is_logical_server() {
            return (self.native_client_level() == Some(level)).then(|| {
                self.state_shape_boxes(
                    &self.world,
                    crate::game::Side::shulkers(self.side_state()),
                    state,
                    pos,
                    kind,
                )
            });
        }
        if !self.level_handle_current(level) {
            return None;
        }
        let (world, shulkers) = if self.dim.key() == level.dimension() {
            (&self.world, crate::game::Side::shulkers(self.side_state()))
        } else {
            <Self::State as crate::game::side_ops::world::SideOps>::side_parked_block_shapes(
                self.side_state(),
                level.dimension(),
            )?
        };
        Some(self.state_shape_boxes(world, shulkers, state, pos, kind))
    }

    /// The local face support in an open server level or the current client level,
    /// including shapes published by local block entities. None for unavailable
    /// levels, unloaded or out-of-height blocks, or a shape query error.
     fn level_block_face_sturdy(
        &self,
        level: LevelRef,
        pos: [i32; 3],
        face: BlockFace,
        support: mod_api::SupportType,
    ) -> Option<bool> {
        crate::game::native_world_read_common::block_face_sturdy(
            self.native_level_read_world(level)?,
            self.res.behaviours(),
            self.res.collision(),
            pos,
            face,
            support,
        )
    }

    /// The entity's yaw and pitch in degrees (`Entity.getYRot`, `getXRot`); 0
    /// for the kinds that don't turn. API 373.
     fn entity_rotation(&self, entity: EntityRef) -> Option<[f32; 2]> {
        Some(self.target_rotation(self.native_entity_read_target(entity)?))
    }

    /// A mob of this game that is resident in a loaded level other than the
    /// one in place, as a kill credit can name one (`getEntityInAnyDimension`):
    /// its identity reads (`entity_kind`, `entity_uuid`, `entity_level`)
    /// answer from there. None on a client or for anything else.
     fn mob_in_other_level<'__loan>(&'__loan self, id: u32) -> Option<(super::Dim, &super::Mob)> where '__sg: '__loan {
        if !self.is_logical_server() {
            return None;
        }
        let EntityLookup::Found {
            location,
            value: Target::Mob(_),
        } = <Self::State as crate::game::side_ops::world::SideOps>::side_server_resident_entity(
            self, id,
        )
        else {
            return None;
        };
        <Self::State as crate::game::side_ops::world::SideOps>::side_parked_mob(
            self.side_state(),
            location.owner.dimension,
            location.join,
        )
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeEntityRefsCommonGameExt5<'__sg> for G {}


pub trait NativeEntityRefsCommonGameExt6<'__sg>: crate::game::SideGame<'__sg> {
    /// The entity as a vanilla Mob: a mob or the ender dragon. Players, armor
    /// stands, stale handles and clients are rejected. Mobs taken out for
    /// ticking are unavailable (a #29 follow-up).
     fn mob(&self, entity: EntityRef) -> Option<MobRef> {
        match self.native_entity_target(entity)? {
            Target::Mob(_) | Target::Dragon(_) => Some(MobRef::new(entity)),
            _ => None,
        }
    }

    /// The existing portal decision for an available mob in the current level.
     fn mob_can_use_portal(&self, mob: MobRef) -> Option<bool> {
        let Target::Mob(id) = self.native_entity_target(mob.entity())? else {
            return None;
        };
        let mob = self.mobs.iter().find(|m| m.id == id)?;
        Some(self.mob_portal_eligible(mob))
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeEntityRefsCommonGameExt6<'__sg> for G {}


pub trait NativeEntityRefsCommonGameExt7<'__sg>: crate::game::SideGame<'__sg> {
    /// The entity's name as plain text (`Entity.getName`): its custom name,
    /// a player's name, else its type's name. On a client (API
    /// @API:client_entity_refs@), from the custom name the server synced
    /// (`Entity.DATA_CUSTOM_NAME`) and a player's profile name. API
    /// 386.
     fn entity_name(&self, entity: EntityRef) -> Option<String> {
        let t = self.native_entity_read_target(entity)?;
        if let Some(result) = self.side_level_player_name(entity) {
            if let Some(name) = result.ok()? {
                return Some(name.to_owned());
            }
            if let Target::Stand(id) = t
                && let Some(name) = self
                    .armor_stands
                    .iter()
                    .find(|a| a.id == id)
                    .and_then(|a| a.custom_name.as_ref())
            {
                return Some(name.plain());
            }
        }
        Some(self.target_name(t).plain())
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeEntityRefsCommonGameExt7<'__sg> for G {}


use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;

use crate::game::side_ops::player::PlayerSideGameExt as _;

use crate::game::side_ops::world::WorldSideGameExt as _;

use crate::game::tables::GameItems as _;

use crate::game::block_props::BlockPropsGameExt2 as _;

use crate::game::bookshelves_common::BookshelvesCommonGameExt1 as _;

use crate::game::commands_common::CommandsCommonGameExt2 as _;

use crate::game::native_level_refs_common::NativeLevelRefsCommonGameExt1 as _;

use crate::game::native_state_shapes::NativeStateShapesGameExt1 as _;

use crate::game::remote::general::GeneralGameExt1 as _;
