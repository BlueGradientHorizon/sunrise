//! Common boat physics, fluid/status queries, placement clip, and collision helpers shared by server and client.
#[allow(unused_imports)]
use super::{Game, collision_source::Mover, entity::Entity};
#[allow(unused_imports)]
use crate::game::server_side::boats::{
    Boat, HEIGHT, Status, powder_snow_stuck, reached_cells, registered_boat_item, wrap_degrees_f64,
};
#[allow(unused_imports)]
use std::{io, sync::Arc};
#[allow(unused_imports)]
use sunrise_client::{
    blocks::Fluid,
    collisions::{EntityCollisions, SceneCollisions, no_collision_with_context},
    items::ItemStack,
    motion::{BubbleColumn, bubble_column_at},
};
#[allow(unused_imports)]
use sunrise_core::{
    geometry::{Aabb, ShapeOp, Vec3, VoxelShape},
    position::BlockPos,
    random::RandomSource,
};
#[allow(unused_imports)]
use sunrise_world::{
    collision_context::CollisionContext,
    registry::BlockStateId,
    world_ray::{RayBlockMode, RayFluidMode, RayWorld, clip_world},
};

pub(in crate::game) struct OutlineFluidRay<'a, 'r>(pub(in crate::game) &'a Game<'r>);

impl RayWorld for OutlineFluidRay<'_, '_> {
    type Error = io::Error;
    type Block = BlockStateId;
    type Fluid = Option<(f32, bool, Fluid)>;
    fn block_state(&mut self, p: BlockPos) -> io::Result<BlockStateId> {
        Ok(self.0.world.block(p.x, p.y, p.z))
    }
    fn fluid_state(&mut self, p: BlockPos) -> io::Result<Self::Fluid> {
        Ok(self
            .0
            .fluid_height_of(p.x, p.y, p.z, None)
            .map(|(h, source)| {
                (
                    h,
                    source,
                    self.0
                        .res
                        .facts()
                        .get(self.0.world.block(p.x, p.y, p.z))
                        .fluid,
                )
            }))
    }
    fn block_shape(
        &mut self,
        state: BlockStateId,
        p: BlockPos,
        _: RayBlockMode,
    ) -> io::Result<VoxelShape> {
        // LiquidBlock.getShape is empty. Waterlogged blocks retain their outline.
        if matches!(
            &*self.0.res.facts().get(state).name,
            "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column"
        ) {
            return Ok(VoxelShape::empty());
        }
        let mut shape = VoxelShape::empty();
        for b in self.0.outline_boxes([p.x, p.y, p.z]) {
            let b: [f64; 6] = std::array::from_fn(|i| b[i] - f64::from([p.x, p.y, p.z][i % 3]));
            let part = VoxelShape::from_box(Aabb::new(
                Vec3::new(b[0], b[1], b[2]),
                Vec3::new(b[3], b[4], b[5]),
            ))
            .map_err(|e| io::Error::other(format!("{e:?}")))?;
            shape = shape
                .join(&part, ShapeOp::OR)
                .map_err(|e| io::Error::other(format!("{e:?}")))?;
        }
        Ok(shape)
    }
    fn interaction_shape(&mut self, _: BlockStateId, _: BlockPos) -> io::Result<VoxelShape> {
        Ok(VoxelShape::empty())
    }
    fn fluid_is_source(&mut self, s: Self::Fluid) -> io::Result<bool> {
        Ok(s.is_some_and(|s| s.1))
    }
    fn fluid_is_empty(&mut self, s: Self::Fluid) -> io::Result<bool> {
        Ok(s.is_none())
    }
    fn fluid_is_water(&mut self, s: Self::Fluid) -> io::Result<bool> {
        Ok(s.is_some_and(|s| s.2 == Fluid::Water))
    }
    fn fluid_shape(&mut self, s: Self::Fluid, _: BlockPos) -> io::Result<VoxelShape> {
        match s {
            None => Ok(VoxelShape::empty()),
            Some((h, _, _)) => {
                VoxelShape::from_box(Aabb::new(Vec3::ZERO, Vec3::new(1.0, f64::from(h), 1.0)))
                    .map_err(|e| io::Error::other(format!("{e:?}")))
            }
        }
    }
}

impl Game<'_> {
    /// `Item.getPlayerPOVHitResult`: float view vector, OUTLINE and Fluid.ANY.
    pub(in crate::game) fn boat_item_hit(&self) -> Option<[f64; 3]> {
        let [x, y, z] = self.eye_position()?;
        let start = Vec3::new(x, y, z);
        let dir = sunrise_world::view_direction::view_vector(self.pitch, self.yaw);
        let end = start.add_vector(dir.scale(self.reach()));
        let hit = clip_world(
            &mut OutlineFluidRay(self),
            start,
            end,
            RayBlockMode::Outline,
            RayFluidMode::Any,
            256,
        )
        .ok()?;
        (!hit.miss).then_some([hit.hit.location.x, hit.hit.location.y, hit.hit.location.z])
    }

    pub(in crate::game) fn fluid_height(&self, x: i32, y: i32, z: i32) -> Option<(f32, bool)> {
        self.fluid_height_of(x, y, z, Some(Fluid::Water))
    }

    /// `FluidState.getHeight` and whether it is a source, for `fluid` or,
    /// with `None`, any fluid (`ClipContext.Fluid.ANY`).
    pub(in crate::game) fn fluid_height_of(
        &self,
        x: i32,
        y: i32,
        z: i32,
        fluid: Option<Fluid>,
    ) -> Option<(f32, bool)> {
        let f = self.res.facts().get(self.world.block(x, y, z));
        if f.fluid == Fluid::None || fluid.is_some_and(|want| f.fluid != want) {
            return None;
        }
        let above = self
            .res
            .facts()
            .get(self.world.block(x, y.wrapping_add(1), z));
        // FluidState.getHeight: 1 under the same fluid, else amount / 9.
        let height = if above.fluid == f.fluid {
            1.0
        } else {
            f32::from(f.fluid_level) / 9.0
        };
        Some((height, f.fluid_level == 8 && !f.fluid_falling))
    }

    /// `getWaterLevelAbove`: the top of the water column the hull is in.
    /// A float, as vanilla's: the block's y plus its water height in float.
    pub(in crate::game) fn water_level_above(&self, b: &Boat) -> f32 {
        let bb = b.bounds();
        let (lo, hi) = (bb.min_position(), bb.max_position());
        let (y0, y1) = (hi.y.floor() as i32, hi.y.ceil() as i32);
        'rows: for y in y0..y1 {
            let mut height = 0.0f32;
            for x in lo.x.floor() as i32..hi.x.ceil() as i32 {
                for z in lo.z.floor() as i32..hi.z.ceil() as i32 {
                    if let Some((h, _)) = self.fluid_height(x, y, z) {
                        height = height.max(h);
                    }
                    if height >= 1.0 {
                        continue 'rows;
                    }
                }
            }
            if height < 1.0 {
                return y as f32 + height;
            }
        }
        (i64::from(y1) + 1) as f32
    }

    pub(in crate::game) fn boat_status(
        &self,
        b: &Boat,
        water_level: &mut f64,
        land_friction: &mut f32,
    ) -> Status {
        let bb = b.bounds();
        let (lo, hi) = (bb.min_position(), bb.max_position());
        // isUnderwater
        let max_y = hi.y + 0.001;
        let mut under = false;
        for x in lo.x.floor() as i32..hi.x.ceil() as i32 {
            for y in hi.y.floor() as i32..max_y.ceil() as i32 {
                for z in lo.z.floor() as i32..hi.z.ceil() as i32 {
                    if let Some((h, source)) = self.fluid_height(x, y, z)
                        && max_y < f64::from(y as f32 + h)
                    {
                        if !source {
                            *water_level = hi.y;
                            return Status::UnderFlowingWater;
                        }
                        under = true;
                    }
                }
            }
        }
        if under {
            *water_level = hi.y;
            return Status::UnderWater;
        }
        // checkInWater
        let mut in_water = false;
        *water_level = -f64::MAX;
        for x in lo.x.floor() as i32..hi.x.ceil() as i32 {
            for y in lo.y.floor() as i32..(lo.y + 0.001).ceil() as i32 {
                for z in lo.z.floor() as i32..hi.z.ceil() as i32 {
                    if let Some((h, _)) = self.fluid_height(x, y, z) {
                        // `float height = y + getHeight`: summed in float.
                        let level = f64::from(y as f32 + h);
                        *water_level = water_level.max(level);
                        in_water |= lo.y < level;
                    }
                }
            }
        }
        if in_water {
            return Status::InWater;
        }

        let friction = self.boat_ground_friction(b, bb);
        if friction > 0.0 {
            *land_friction = friction;
            Status::OnLand
        } else {
            Status::InAir
        }
    }

    /// `AbstractBoat.getGroundFriction`: average friction of the blocks whose
    /// collision shape meets a thin slab just under the hull (lily pads skipped).
    pub(in crate::game) fn boat_ground_friction(&self, b: &Boat, bb: Aabb) -> f32 {
        let (lo, hi) = (bb.min_position(), bb.max_position());
        let ground = Aabb::new(
            Vec3::new(lo.x, lo.y - 0.001, lo.z),
            Vec3::new(hi.x, lo.y, hi.z),
        );
        let (g_lo, g_hi) = (ground.min_position(), ground.max_position());
        let (x0, x1, y0, y1, z0, z1) = (
            (g_lo.x.floor() - 1.0) as i32,
            (g_hi.x.ceil() + 1.0) as i32,
            (g_lo.y.floor() - 1.0) as i32,
            (g_hi.y.ceil() + 1.0) as i32,
            (g_lo.z.floor() - 1.0) as i32,
            (g_hi.z.ceil() + 1.0) as i32,
        );
        let slab = VoxelShape::from_box(ground).unwrap_or_else(|_| VoxelShape::empty());
        let mut scene = SceneCollisions {
            world: &self.world,
            binding: self.collision,
            subject: None,
        };
        let (mut friction, mut count) = (0.0f32, 0);
        for x in x0..x1 {
            for z in z0..z1 {
                let edges = i32::from(x == x0 || x == x1 - 1) + i32::from(z == z0 || z == z1 - 1);
                if edges == 2 {
                    continue;
                }
                for y in y0..y1 {
                    if edges == 1 && (y == y0 || y == y1 - 1) {
                        continue;
                    }
                    let state = self.world.block(x, y, z);
                    let f = self.res.facts().get(state);
                    if f.name.as_ref() == "minecraft:air" || f.has_class("LilyPadBlock") {
                        continue;
                    }
                    let Ok(shape) = self.collision.collision_shape_with_context(
                        &mut scene,
                        state,
                        BlockPos::new(x, y, z),
                        &CollisionContext::empty(false),
                    ) else {
                        continue;
                    };
                    let world_shape =
                        shape.moved(Vec3::new(f64::from(x), f64::from(y), f64::from(z)));
                    if world_shape
                        .join_is_not_empty(&slab, ShapeOp::AND)
                        .unwrap_or(false)
                    {
                        friction +=
                            self.boat_friction(b, BlockPos::new(x, y, z), state, f.friction);
                        count += 1;
                    }
                }
            }
        }
        if count > 0 {
            friction / count as f32
        } else {
            0.0
        }
    }

    /// `Entity.move`'s fall reset for an entity that keeps a fall and moves on
    /// by `moved` from `from`: a move of a block or more casts up to 8 blocks
    /// of it (`ClipContext.Block.FALLDAMAGE_RESETTING`, `ClipContext.Fluid.WATER`)
    /// and a hit resets the fall, before the move's own drop counts.
    pub(in crate::game) fn vehicle_fall_reset(&self, fall: f64, from: Vec3, moved: Vec3) -> f64 {
        let length = moved.length_squared();
        if fall == 0.0 || length < 1.0 {
            return fall;
        }
        let reset = &self.res.motion().fall_reset;
        let fluids = &self.res.motion().fluids;
        let to = from.add_vector(moved.normalize().scale(length.sqrt().min(8.0)));
        let hit = sunrise_core::geometry::traverse_blocks(from, to, 1_000_000, |p| {
            // A tagged block clips as a full block, not its collider.
            let name = &self.res.facts().get(self.world.block(p.x, p.y, p.z)).name;
            if reset.contains(&**name) && VoxelShape::block().clip(from, to, p).is_some() {
                return Some(());
            }
            // The water's shape, through the shared fluid shape cache.
            sunrise_client::entities::water_clip(
                &self.loop_context,
                &self.world,
                fluids,
                from,
                to,
                p,
            )
            .then_some(())
        });
        if matches!(hit, Ok(Some(()))) {
            0.0
        } else {
            fall
        }
    }

    /// `AbstractBoat.checkFallDamage` after a move that went `moved_y` (with
    /// `b` already where it landed): on the ground the fall resets; a move
    /// down over anything but water (the block under its `blockPosition`)
    /// adds to it.
    pub(in crate::game) fn boat_fall(&self, b: &Boat, moved_y: f64) -> f64 {
        if b.ground.on_ground {
            return 0.0;
        }
        if moved_y < 0.0 {
            let below = self.res.facts().get(self.world.block(
                b.position.x.floor() as i32,
                (b.position.y.floor() as i32).wrapping_sub(1),
                b.position.z.floor() as i32,
            ));
            if below.fluid != Fluid::Water {
                return b.fall_distance - f64::from(moved_y as f32);
            }
        }
        b.fall_distance
    }

    /// `AbstractInterpolationHandler.interpolate` for the boat this client
    /// drives: its own moves since the last step carry the target along (where
    /// the boat fits there), then `LinearInterpolationHandler.doInterpolate`
    /// takes a `1 / steps` share of the way.
    pub(in crate::game) fn interpolate_driven_boat(&self, b: &mut Boat) {
        let Some(mut l) = b.lerp.take() else {
            return;
        };
        let moved = b.position.subtract(l.last.0);
        if moved.length_squared() > f64::from(1.0e-5_f32) {
            let to = l.target.add_vector(moved);
            if self.boat_lerp_target_fits(b, to) {
                l.target = to;
            }
        }
        l.yaw += b.yaw - l.last.1;
        l.pitch += b.pitch - l.last.2;
        let alpha = 1.0 / f64::from(l.steps);
        b.position = b.position.lerp(l.target, alpha);
        let yaw = f64::from(b.yaw) + alpha * wrap_degrees_f64(f64::from(l.yaw) - f64::from(b.yaw));
        let pitch = f64::from(b.pitch) + alpha * (f64::from(l.pitch) - f64::from(b.pitch));
        // Entity.setRot
        b.yaw = yaw as f32 % 360.0;
        b.pitch = pitch as f32 % 360.0;
        l.steps -= 1;
        l.last = (b.position, b.yaw, b.pitch);
        if l.steps > 0 {
            b.lerp = Some(l);
        }
    }

    /// `AbstractInterpolationHandler.adjustInterpolationPosition`: check the
    /// shifted target against blocks, entities and the border at the boat's live pose.
    pub(in crate::game) fn boat_lerp_target_fits(&self, boat: &Boat, target: Vec3) -> bool {
        let bounds = boat.bounds_at(target);
        let scene = SceneCollisions {
            world: &self.world,
            binding: self.collision,
            subject: None,
        };
        let mut collisions = EntityCollisions::with_border(
            scene,
            self.border.geometry(),
            [boat.position.x, boat.position.z],
        );
        let e = sunrise_core::geometry::COLLISION_EPSILON;
        let boxes = self.entity_collisions(Mover::Boat {
            id: boat.id,
            area: bounds.inflate(Vec3::new(e, e, e)),
        });
        collisions.entities = &boxes;
        no_collision_with_context(
            &mut collisions,
            bounds,
            CollisionContext::position(boat.position.y),
        )
    }

    /// `Entity.recordMovement` for a both-side lid push: move an active
    /// interpolation target by the displacement, if the boat fits there.
    pub(in crate::game) fn record_boat_lid_movement(&mut self, i: usize, delta: Vec3) {
        let boat = &self.boats[i];
        if let Some(mut lerp) = boat.lerp {
            let to = lerp.target.add_vector(delta);
            if self.boat_lerp_target_fits(boat, to) {
                lerp.target = to;
            }
            // The next step measures any *other* move since the last pose.
            lerp.last.0 = lerp.last.0.add_vector(delta);
            self.boats.update(i, |boat| boat.lerp = Some(lerp));
        } else if let Some(lerp) = self
            .remote
            .as_ref()
            .and_then(|r| r.lerps.get(&(boat.id as i32)))
            && lerp.steps > 0
        {
            let [x, y, z] = lerp.target;
            let to = Vec3::new(x, y, z).add_vector(delta);
            if self.boat_lerp_target_fits(boat, to) {
                self.remote
                    .as_mut()
                    .unwrap()
                    .lerps
                    .get_mut(&(boat.id as i32))
                    .unwrap()
                    .target = to.components();
            }
        }
    }

    pub(in crate::game) fn tick_boat(&mut self, mut b: Boat, controlled: bool) -> Boat {
        b.previous = b.position;
        b.previous_yaw = b.yaw;
        // Entity.commonTick: the client's interpolation, before the boat ticks.
        if controlled {
            let from = b.position;
            self.interpolate_driven_boat(&mut b);
            // A step that crosses a section is filed where it happens, not folded into
            // the end-of-tick scan with the boat's own move.
            if from != b.position {
                self.entity_section_step(
                    b.id,
                    [from.x, from.y, from.z],
                    [b.position.x, b.position.y, b.position.z],
                );
            }
        }
        // Entity.baseTick ends with Leashable.tickLeash, on a server only
        // (a ridden boat's lead ticks in `tick_boats`).
        if !controlled {
            self.side_tick_boat_leash(&mut b);
        }
        let had_status = std::mem::replace(&mut b.status_known, true);
        b.old_status = b.status;
        let (mut water_level, mut land_friction) = (b.water_level, b.land_friction);
        b.status = self.boat_status(&b, &mut water_level, &mut land_friction);
        (b.water_level, b.land_friction) = (water_level, land_friction);
        b.tick_submersion();
        // `tick_boats_now` wears it off for the boats it ticks, before they burn.
        if controlled {
            b.wear_off_hurt();
        }
        // A remote client gets the controller from the passenger packet, even when
        // this window occupies the second seat. Local worlds derive it here.
        if self.remote.is_none() {
            b.paddle_controller = controlled;
        }
        if !controlled {
            b.paddling = [false; 2];
            b.input = [false; 4];
        }
        // `Entity.baseTick`: the boat is immersed and pushed by the current it
        // sits in (`isPushedByFluid`), before `floatBoat` scales the velocity.
        // updateFluidInteraction: in water the fall resets.
        if b.push_by_fluid(&self.world, &self.res.motion().fluids, &self.loop_context)
            .0
            > 0.0
        {
            b.fall_distance = 0.0;
        }
        // floatBoat
        // `-getGravity()`: none with NoGravity.
        let mut vspeed = 0.0 - b.get_gravity();
        let mut buoyancy = 0.0;
        let inv_friction: f32;
        let obstacles = self.boat_move_obstacles(&b);
        let mut scene = SceneCollisions {
            world: &self.world,
            binding: self.collision,
            subject: None,
        };
        if had_status
            && b.old_status == Status::InAir
            && b.status != Status::InAir
            && b.status != Status::OnLand
        {
            // getY(1.0): the top of its box.
            b.water_level = b.position.y + HEIGHT;
            // getWaterLevelAbove() - getBbHeight(): a float subtraction.
            let target = f64::from(self.water_level_above(&b) - HEIGHT as f32) + 0.101;
            let moved = Aabb::new(
                Vec3::new(
                    b.bounds().min_position().x,
                    target,
                    b.bounds().min_position().z,
                ),
                Vec3::new(
                    b.bounds().max_position().x,
                    target + HEIGHT,
                    b.bounds().max_position().z,
                ),
            );
            if scene.unobstructed(moved).unwrap_or(false) {
                b.position.y = target;
                b.velocity.y = 0.0;
            }
            b.status = Status::InWater;
        } else {
            match b.status {
                Status::InWater => {
                    buoyancy = (b.water_level - b.position.y) / HEIGHT;
                    inv_friction = 0.9;
                }
                Status::UnderFlowingWater => {
                    vspeed = -7.0e-4;
                    inv_friction = 0.9;
                }
                Status::UnderWater => {
                    buoyancy = f64::from(0.01_f32);
                    inv_friction = 0.45;
                }
                Status::InAir => inv_friction = 0.9,
                Status::OnLand => {
                    inv_friction = b.land_friction;
                    if controlled {
                        b.land_friction /= 2.0;
                    }
                }
            }
            let f = f64::from(inv_friction);
            b.velocity = Vec3::new(b.velocity.x * f, b.velocity.y + vspeed, b.velocity.z * f);
            b.delta_rotation *= inv_friction;
            if buoyancy > 0.0 {
                b.velocity.y = (b.velocity.y + buoyancy * (0.04 / 0.65)) * 0.75;
            }
        }
        // controlBoat
        if controlled {
            let [left, right, up, down] = b.input;
            let mut acceleration = 0.0f32;
            if left {
                b.delta_rotation -= 1.0;
            }
            if right {
                b.delta_rotation += 1.0;
            }
            if right != left && !up && !down {
                acceleration += 0.005;
            }
            b.yaw += b.delta_rotation;
            if up {
                acceleration += 0.04;
            }
            if down {
                acceleration -= 0.005;
            }
            // AbstractBoat.controlBoat: Mth's sine table, at the float angle.
            use sunrise_core::geometry::trig;
            b.velocity.x +=
                f64::from(trig::sin(f64::from(-b.yaw * trig::DEG_TO_RAD)) * acceleration);
            b.velocity.z +=
                f64::from(trig::cos(f64::from(b.yaw * trig::DEG_TO_RAD)) * acceleration);
            b.paddling = [right && !left || up, left && !right || up];
        }
        let mut scene = EntityCollisions {
            blocks: scene,
            entities: &obstacles,
            border: Some((self.border.geometry(), [b.position.x, b.position.z])),
        };
        let movement = b.take_movement();
        let mut gate = sunrise_client::move_gate::armed_with(
            &self.loop_context,
            b.id,
            || std::borrow::Cow::Owned(b.entity_type()),
            sunrise_world::entity_move::MoverType::SelfMovement,
            movement,
        );
        let movement = gate.delta();
        if let Ok(moved) = sunrise_client::move_gate::entities_resolve(
            &mut gate,
            &mut scene,
            b.bounds(),
            movement,
            0.0,
            b.ground.on_ground,
        ) {
            b.fall_distance = self.vehicle_fall_reset(b.fall_distance, b.position, moved);
            b.position = Vec3::new(
                b.position.x + moved.x,
                b.position.y + moved.y,
                b.position.z + moved.z,
            );
            let side = game_side!(self);
            let motion = sunrise_client::motion::BodyMotion {
                world: &self.world,
                collision: self.collision,
                facts: self.res.facts(),
                blocks: Some(&self.motion.blocks),
                context: &self.loop_context,
                side: Some(&side),
            };
            if !gate.skipped() {
                motion.record_move(
                    sunrise_client::motion::BodyCollision {
                        footprint: (b.position, b.bounds()),
                        movement: (movement, moved),
                        authoritative: true,
                        context: sunrise_client::motion::BodyMotion::entity_context(
                            b.id, b.position,
                        ),
                        subject: None,
                    },
                    &mut b.ground,
                );
            }
            let effect = motion.legacy_contact(b.position, &b.ground);
            b.fall_distance = self.boat_fall(&b, moved.y);
            let entity = (b.id, b.entity_type_name());
            let response = motion.restitute(
                entity,
                sunrise_client::motion::BodyMove {
                    wanted: movement,
                    moved,
                    effect,
                },
                b.get_gravity(),
                1.0_f32,
                b.velocity,
                b.fall_distance,
            );
            b.velocity = self.side_finish_body_restitution(entity, b.position, response);
        }
        self.body_motion(&game_side!(self))
            .apply_block_speed(b.position, &mut b.velocity);
        self.boat_step_on(&mut b);
        self.boat_stuck_in_blocks(&mut b);
        b
    }

    /// Refuse a removed tick target before carrying its slot into the clone.
    pub(in crate::game) fn store_boat_back(&mut self, index: usize, boat: Boat) -> bool {
        if self
            .persist
            .removals
            .contains(&super::commands::Target::Boat(boat.id))
        {
            return false;
        }
        self.boats.store_back(index, boat)
    }

    /// `Entity.canSimulateMovement`: for a boat (`AUTHORITATIVE_SIDE`).
    pub(in crate::game) fn boat_can_simulate_movement(&self, id: u32) -> bool {
        self.side_boat_can_simulate_movement(id)
    }

    /// `AbstractBoat.canAddPassenger`: a seat is free and the boat is not under water.
    pub(in crate::game) fn boat_has_room(&self, i: usize) -> bool {
        self.side_boat_has_room(i)
    }

    /// Where the player in boat `i` sits: in the middle alone, in front of a mob.
    pub(in crate::game) fn player_boat_seat(&self, i: usize) -> Vec3 {
        self.side_player_boat_seat(i)
    }

    /// `AbstractBoat.tick`: `status = getStatus()` for a boat this window does not tick.
    pub(in crate::game) fn refresh_boat_status(&mut self, i: usize) {
        let (mut water_level, mut land_friction) = (0.0, 0.0);
        let status = self.boat_status(&self.boats[i], &mut water_level, &mut land_friction);
        self.boats.update(i, |b| b.status = status);
        self.boats.update(i, |b| b.status_known = true);
        self.boats.update(i, Boat::tick_submersion);
    }

    /// `BubbleColumnBlock.entityInside`: the bubble columns boat `i` touches.
    pub(in crate::game) fn boat_bubble_columns(&self, i: usize) -> Vec<BubbleColumn> {
        let b = self.boats[i].bounds().deflate(Vec3::new(
            f64::from(1.0e-5f32),
            f64::from(1.0e-5f32),
            f64::from(1.0e-5f32),
        ));
        let (lo, hi) = (b.min_position(), b.max_position());
        let mut columns = Vec::new();
        for x in lo.x.floor() as i32..=hi.x.floor() as i32 {
            for y in lo.y.floor() as i32..=hi.y.floor() as i32 {
                for z in lo.z.floor() as i32..=hi.z.floor() as i32 {
                    if let Some(column) = bubble_column_at(
                        &self.world,
                        self.pack.assets.registry,
                        self.res.facts(),
                        self.collision,
                        BlockPos::new(x, y, z),
                    ) {
                        columns.push(column);
                    }
                }
            }
        }
        columns
    }

    /// `Entity.handleOnInsideBubbleColumn`.
    pub(in crate::game) fn boat_inside_bubble_column(&mut self, i: usize, column: BubbleColumn) {
        if !self.boat_can_simulate_movement(self.boats[i].id) {
            return;
        }
        self.boats.update(i, |b| column.push(&mut b.velocity));
    }

    /// `AbstractBoat.onAboveBubbleColumn`: whether the splash comes up, 1 in 100 above water.
    pub(in crate::game) fn boat_splash_roll(&mut self, i: usize) -> bool {
        !self.boats[i].under_water() && self.world.random.next_int_bound(100).unwrap_or(1) == 0
    }

    /// `AbstractBoat.handleBubbleColumnEffect`: up at 0.6, down by -0.7.
    pub(in crate::game) fn handle_bubble_column_effect(&mut self, i: usize, drag_down: bool) {
        if !self.boat_can_simulate_movement(self.boats[i].id) {
            return;
        }
        self.boats.update(i, |b| {
            if drag_down {
                b.velocity.y += -0.7;
            } else {
                b.velocity.y = 0.6;
            }
        });
    }

    pub(in crate::game) fn boat_step_on(&mut self, boat: &mut Boat) {
        let mut velocity = boat.velocity;
        let writes = self.body_step_on(
            (boat.id, boat.entity_type_name()),
            (boat.position, boat.bounds()),
            &boat.ground,
            &mut velocity,
        );
        boat.apply_step_writes(velocity, writes);
    }

    /// The second `applyEffectsFromBlocks` of `AbstractBoat.tick` uses the movement left by the first.
    pub(in crate::game) fn boat_second_step(&mut self, i: usize) {
        let b = &self.boats[i];
        if b.ground.on_ground {
            let mut velocity = b.velocity;
            let (id, kind, footprint, ground) = (
                b.id,
                b.shared_type_name(),
                (b.position, b.bounds()),
                b.ground,
            );
            let writes = self.body_step_on((id, &kind), footprint, &ground, &mut velocity);
            self.boats
                .update(i, |b| b.apply_step_writes(velocity, writes));
        }
    }

    /// `Entity.makeStuckInBlock` from powder snow and cobweb cells a boat reaches into.
    pub(in crate::game) fn boat_stuck_in_blocks(&self, b: &mut Boat) {
        let mut cells = Vec::new();
        if b.previous != b.position {
            cells.extend(reached_cells(b.bounds_at(b.previous), |_| true));
        }
        for cell in reached_cells(b.bounds(), |_| true) {
            if !cells.contains(&cell) {
                cells.push(cell);
            }
        }
        for cell in cells {
            let name = &*self
                .res
                .facts()
                .get(self.world.block(cell[0], cell[1], cell[2]))
                .name;
            b.stuck = match name {
                "minecraft:powder_snow" => powder_snow_stuck(),
                "minecraft:cobweb" => sunrise_client::falling_block::web_multiplier(),
                _ => continue,
            };
            b.fall_distance = 0.0;
        }
    }

    /// The boat `BoatItem.use` would place for `stack` (id = 0; server caller assigns its entity id).
    pub(in crate::game) fn boat_to_place(&self, stack: &ItemStack) -> Option<Boat> {
        let (wood, chest) = registered_boat_item(&stack.item, self.res.items())?;
        let hit = self.boat_item_hit()?;
        let mut boat = Boat::new(
            0,
            Arc::from(wood),
            Vec3::new(hit[0], hit[1], hit[2]),
            self.yaw,
        );
        if chest {
            boat = boat.with_chest();
        }
        let bb = boat.bounds();
        let (lo, hi) = (bb.min_position(), bb.max_position());
        let e = 1.0e-7;
        let inner = Aabb::new(
            Vec3::new(lo.x + e, lo.y + e, lo.z + e),
            Vec3::new(hi.x - e, hi.y - e, hi.z - e),
        );
        let mut scene = SceneCollisions {
            world: &self.world,
            binding: self.collision,
            subject: None,
        };
        if !scene.unobstructed(inner).unwrap_or(false) || self.boat_entity_collision(bb) {
            return None;
        }
        Some(boat)
    }

    /// Whether the player rides a chest boat (`HasCustomInventoryScreen`).
    pub(in crate::game) fn riding_chest_boat(&self) -> bool {
        self.riding
            .and_then(|id| self.boats.iter().find(|b| b.id == id))
            .is_some_and(|b| b.chest)
    }

    /// What boat `b` may run into this tick.
    pub(in crate::game) fn boat_move_obstacles(&self, b: &Boat) -> Vec<Aabb> {
        let reach = b.velocity.length() + 1.0;
        self.entity_collisions(Mover::Boat {
            id: b.id,
            area: b.bounds().inflate(Vec3::new(reach, reach, reach)),
        })
    }

    /// `noEntityCollision(boat, box)` for a new boat (`BoatItem.use`).
    pub(in crate::game) fn boat_entity_collision(&self, bb: Aabb) -> bool {
        // EntityGetter.getEntityCollisions: the box grown by 1e-7.
        let area = bb.inflate(Vec3::new(1.0e-7, 1.0e-7, 1.0e-7));
        !self
            .entity_collisions(Mover::Boat { id: u32::MAX, area })
            .is_empty()
    }
}
