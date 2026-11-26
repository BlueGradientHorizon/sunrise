//! Server-side piston, shulker lid, block-push-up, and forced entity move passes.
use super::{
    Game,
    entity::{Entity, EntityHandle, with_entity, with_entity_mut},
    minecarts::CartHit,
    move_gate::MoveWho,
};
use crate::game::collision_source::Mover;
use crate::game::piston_push::{
    Body, Lid, Push, PushTarget, ShulkerPush, StepHeights, first_axis_direction, pushed_movement,
    shulker_lid,
};
use sunrise_core::geometry::{
    Aabb, Axis, COLLISION_EPSILON, Direction, ShapeOp, Vec3, entity_collision_bounds,
};
use sunrise_core::position::BlockPos;
use sunrise_level::collisions::{SceneCollisions, with_piston_noclip};
use sunrise_sim::entity_rules::EntityRules;
use sunrise_sim::join_seq::{JoinSeq, Joined};
use sunrise_sim::motion::MotionWorld;
use sunrise_world::{
    entity_move::MoverType, level::data::PushReaction, move_gate::MoveGate,
    piston_movement::PistonMovement, piston_push,
};

impl Game<'_, crate::game::server_side::ServerState> {
    /// Server arm of [`Game::side_push_piston_entities`].
    pub(in crate::game) fn server_push_piston_entities(&mut self, pushes: Vec<Push>) {
        let mut accumulators = std::collections::HashMap::new();
        let mut pricked = Vec::new();
        let index = self.build_piston_index();
        let steps = self.push_step_heights();
        let targets = self.piston_push_targets(&pushes, &steps);
        for push in pushes {
            let pos = push.0;
            self.push_for_piston(push, &targets, &steps, &mut accumulators, &mut pricked);
            self.advance_piston_collision(pos, &index);
        }
        self.hurt_cactus_pricked_vehicles(pricked);
    }

    /// `moveEntityByPiston`'s `applyEffectsFromBlocks` for a boat or a
    /// minecart: every cactus its box touches on the way from `start` to
    /// `end` pricks it (`CactusBlock.entityInside`, `Entity.checkInsideBlocks`
    /// over the box less 1e-5), even when the push moved it nowhere. The hurt
    /// waits for the end of the tick's pushes, whose targets are found first.
    pub(in crate::game) fn piston_cactus_contact(
        &self,
        target: MoveWho,
        start: Aabb,
        end: Aabb,
        pricked: &mut Vec<u32>,
    ) {
        let id = match target {
            MoveWho::Entity(EntityHandle::Boat(i)) => self.boats[i].id,
            MoveWho::Entity(EntityHandle::Cart(i)) => self.carts[i].id,
            _ => return,
        };
        let eps = f64::from(1.0e-5_f32);
        let (a, b) = (start.min_position(), end.min_position());
        let lo = Vec3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z));
        let (a, b) = (start.max_position(), end.max_position());
        let hi = Vec3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z));
        let facts = self.res.facts();
        for x in (lo.x + eps).floor() as i32..=(hi.x - eps).floor() as i32 {
            for y in (lo.y + eps).floor() as i32..=(hi.y - eps).floor() as i32 {
                for z in (lo.z + eps).floor() as i32..=(hi.z - eps).floor() as i32 {
                    if &*facts.get(self.world.block(x, y, z)).name == "minecraft:cactus" {
                        pricked.push(id);
                    }
                }
            }
        }
    }

    /// `Entity.hurt(cactus, 1)` for the vehicles `piston_cactus_contact`
    /// found, in the order of their pushes.
    pub(in crate::game) fn hurt_cactus_pricked_vehicles(&mut self, pricked: Vec<u32>) {
        for id in pricked {
            if let Some(i) = self.boats.iter().position(|b| b.id == id) {
                self.hurt_boat(i, 1.0, false);
            } else if let Some(i) = self.carts.iter().position(|c| c.id == id) {
                self.hurt_cart(i, 1.0, false, false, CartHit::default());
            }
        }
    }

    /// The bodies any of this tick's moving blocks can reach, in `push_targets` order.
    pub(in crate::game) fn piston_push_targets(
        &self,
        pushes: &[Push],
        steps: &StepHeights,
    ) -> Vec<PushTarget> {
        let Some(reach) = pushes
            .iter()
            .map(|(p, ..)| {
                let low = Vec3::new(f64::from(p.x), f64::from(p.y), f64::from(p.z));
                Aabb::new(
                    low.add_vector(Vec3::new(-2.0, -2.0, -2.0)),
                    low.add_vector(Vec3::new(3.0, 3.0, 3.0)),
                )
            })
            .reduce(|a, b| a.union_bounds(b))
        else {
            return Vec::new();
        };
        let at = pushes[0].0;
        self.push_targets()
            .filter(|&target| {
                self.piston_body(target, Some(at), steps)
                    .is_some_and(|body| body.bounds.intersects(reach))
            })
            .filter_map(|target| self.stable_push_target(target))
            .collect()
    }

    /// `moveEntityByPiston`'s `applyEffectsFromBlocks(previousPos, position)` for projectile `i`.
    pub(in crate::game) fn projectile_piston_block_effects(&mut self, i: usize, from: Vec3) {
        let p = &self.creatures.projectiles[i];
        let end = p.aabb();
        let offset = from.subtract(p.position);
        let start = end.move_by(offset);
        let (lo, hi) = (
            Vec3::new(
                start.min_position().x.min(end.min_position().x) + 1.0e-5,
                start.min_position().y.min(end.min_position().y) + 1.0e-5,
                start.min_position().z.min(end.min_position().z) + 1.0e-5,
            ),
            Vec3::new(
                start.max_position().x.max(end.max_position().x) - 1.0e-5,
                start.max_position().y.max(end.max_position().y) - 1.0e-5,
                start.max_position().z.max(end.max_position().z) - 1.0e-5,
            ),
        );
        let mut stuck = None;
        for x in lo.x.floor() as i32..=hi.x.floor() as i32 {
            for y in lo.y.floor() as i32..=hi.y.floor() as i32 {
                for z in lo.z.floor() as i32..=hi.z.floor() as i32 {
                    let name = &self.res.facts().get(self.world.block(x, y, z)).name;
                    if let Some(s) = sunrise_sim::projectiles::stuck_speed_of(name) {
                        stuck = Some(s);
                    }
                }
            }
        }
        if let Some(stuck) = stuck {
            self.creatures
                .projectiles
                .update(i, |p| p.stuck_speed = stuck);
        }
        let stuck_arrow = self.creatures.projectiles[i]
            .arrow
            .as_ref()
            .is_some_and(|a| a.in_ground);
        if !stuck_arrow {
            let (world, registry, facts, collision) = (
                &self.world,
                self.pack.assets.registry,
                self.res.facts(),
                self.collision,
            );
            let tops = self.creatures.projectiles.update(i, |p| {
                p.bubble_surfaces.clear();
                p.bubble_columns_since(from, |at| {
                    sunrise_sim::motion::bubble_column_at(
                        world,
                        registry,
                        facts,
                        collision,
                        BlockPos::new(at[0], at[1], at[2]),
                    )
                    .map(|c| (c.drag, c.surface))
                });
                std::mem::take(&mut p.bubble_surfaces)
            });
            for [x, y, z] in tops {
                self.bubble_column_particles(BlockPos::new(x, y, z));
            }
        }
    }

    /// `moveEntityByPiston`'s `applyEffectsFromBlocks` for entity-sensing blocks.
    pub(in crate::game) fn piston_block_triggers(&mut self, start: Aabb, end: Aabb) {
        let eps = 1.0e-5;
        let (a, b) = (start.min_position(), end.min_position());
        let lo = Vec3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z));
        let (a, b) = (start.max_position(), end.max_position());
        let hi = Vec3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z));
        let behaviours = self.res.behaviours();
        let mut sensing = Vec::new();
        for x in (lo.x + eps).floor() as i32..=(hi.x - eps).floor() as i32 {
            for y in (lo.y + eps).floor() as i32..=(hi.y - eps).floor() as i32 {
                for z in (lo.z + eps).floor() as i32..=(hi.z - eps).floor() as i32 {
                    if behaviours.senses_entities(self.world.block(x, y, z)) {
                        sensing.push(BlockPos::new(x, y, z));
                    }
                }
            }
        }
        if sensing.is_empty() {
            return;
        }
        self.with_level(|b, level, ticks| {
            for pos in sensing {
                b.entity_inside(level, ticks, pos);
            }
        });
    }

    /// An opening lid's push on the server (`ShulkerBoxBlockEntity` / `Shulker`).
    pub(in crate::game) fn lid_push(&mut self, lid: Lid, area: Aabb, movement: Vec3) {
        let steps = self.push_step_heights();
        let root = self.lid_root(lid);
        // The armor stands' obstacles, built at the first stand the lid meets.
        let mut stand_grid = None;
        let targets: Vec<_> = self
            .push_targets()
            .filter_map(|target| self.stable_push_target(target))
            .collect();
        targets.into_iter().for_each(|selected| {
            let Some(target) = self.resolve_push_target(selected) else {
                return;
            };
            let body = match lid {
                Lid::Box => self.piston_body(target, None, &steps),
                Lid::Shulker(_) => self.push_body(target, None, &steps),
            };
            let Some(body) = body else {
                return;
            };
            if !body.bounds.intersects(area) {
                return;
            }
            let no_physics = self.pushed_no_physics(target);
            if self.lid_spares(lid, root, target, no_physics) {
                return;
            }
            self.move_pushed_body(
                lid.mover(),
                target,
                body,
                movement,
                no_physics,
                &mut stand_grid,
            );
        });
        self.lid_breaks_attached(area, root);
    }

    /// The root vehicle of a shulker whose lid pushes; none for a box.
    pub(in crate::game) fn lid_root(&self, lid: Lid) -> Option<u32> {
        let id = lid.shulker()?;
        self.native_root_vehicle_id(id, super::commands::Target::Mob(id))
    }

    /// Whether a shulker's lid spares a body it meets.
    pub(in crate::game) fn lid_spares(
        &self,
        lid: Lid,
        root: Option<u32>,
        target: MoveWho,
        no_physics: bool,
    ) -> bool {
        self.lid_spares_with_root(lid, root, target, self.push_root(target), no_physics)
    }

    /// `BlockAttachedEntity.move`: a lid that moves a painting, an item frame
    /// (`ItemFrame.move`: not a fixed one) or a cushion breaks it on the server.
    pub(in crate::game) fn lid_breaks_attached(&mut self, area: Aabb, root: Option<u32>) {
        let meets = |(lo, hi): ([f64; 3], [f64; 3])| {
            Aabb::new(
                Vec3::new(lo[0], lo[1], lo[2]),
                Vec3::new(hi[0], hi[1], hi[2]),
            )
            .intersects(area)
        };
        let mut i = 0;
        while i < self.paintings.len() {
            if !(meets(self.paintings[i].bounds()) && self.break_painting_by(i, false, None)) {
                i += 1;
            }
        }
        let mut i = 0;
        while i < self.item_frames.len() {
            let f = &self.item_frames[i];
            if f.fixed || !meets(f.bounds()) || !self.break_frame_by(i, None) {
                i += 1;
            }
        }
        let mut i = 0;
        while i < self.cushions.len() {
            let c = &self.cushions[i];
            if root == Some(c.id) || !meets(c.bounds()) || !self.break_cushion_by(i, None) {
                i += 1;
            }
        }
    }

    /// The id of the lowest vehicle under a pushed body on the server.
    pub(in crate::game) fn push_root(&self, target: MoveWho) -> Option<u32> {
        use super::commands::Target as Named;
        let vehicle_root = |v: u32| self.native_root_vehicle_id(v, Named::Mob(v));
        match target {
            MoveWho::Player => self.riding.and_then(vehicle_root),
            MoveWho::Guest(conn) => self.command_guest(conn)?.vehicle.and_then(vehicle_root),
            MoveWho::Entity(handle) => {
                let (id, named) =
                    with_entity!(self, handle, entity => (entity.id(), entity.selector_target()));
                match named {
                    Some(named) => self.native_root_vehicle_id(id, named),
                    None => Some(id),
                }
            }
        }
    }

    /// `Entity.move(mover, movement)` for a lid's push, collider carry/shove, or mod `move_by`.
    pub(in crate::game) fn move_pushed_body(
        &mut self,
        mover: MoverType,
        target: MoveWho,
        body: Body,
        movement: Vec3,
        no_physics: bool,
        stand_grid: &mut Option<sunrise_sim::entities::ObstacleGrid>,
    ) {
        if target == MoveWho::Player {
            let _ = self.server_move_local_player(mover, movement, false);
            return;
        }
        // Armor stands arm their gate in their own collided shove.
        if let MoveWho::Entity(EntityHandle::ArmorStand(i)) = target {
            self.shoves_armor_stand(i, mover, movement, stand_grid);
            return;
        }
        let Some(selected) = self.stable_push_target(target) else {
            return;
        };
        let mut gate = self.entity_move(target, mover, movement);
        if gate.skipped() {
            return;
        }
        let movement = gate.delta();
        let lid_move = matches!(mover, MoverType::ShulkerBox | MoverType::Shulker);
        // `Entity.move` with `noPhysics`: `setPos` the whole way.
        if no_physics {
            self.apply_push(&mut gate, target, movement);
            if let Some(target) = self.resolve_push_target(selected) {
                if lid_move && let MoveWho::Entity(EntityHandle::Boat(i)) = target {
                    self.record_boat_lid_movement(i, movement);
                }
                self.release_after_block_move(target);
            }
            return;
        }
        let entities = match target {
            MoveWho::Entity(EntityHandle::Boat(i)) if lid_move => {
                let area =
                    entity_collision_bounds(movement, body.bounds, body.step_height).inflate(
                        Vec3::new(COLLISION_EPSILON, COLLISION_EPSILON, COLLISION_EPSILON),
                    );
                self.entity_collisions(Mover::Boat {
                    id: self.boats[i].id,
                    area,
                })
            }
            _ => self.pushed_entity_colliders(target),
        };
        let collision = self.pushed_living_collision(target);
        let Ok(moved) = pushed_movement(
            &mut gate,
            SceneCollisions {
                world: &self.world,
                binding: self.collision,
                subject: collision.map(|(_, subject)| subject),
            },
            self.border.geometry(),
            [body.position.x, body.position.z],
            body.bounds,
            movement,
            body.step_height,
            body.on_ground,
            collision.map(|(context, _)| context),
            &entities,
        ) else {
            return;
        };
        let placed = moved.length_squared() > 1.0e-7
            || movement.length_squared() - moved.length_squared() < 1.0e-7;
        if placed {
            if lid_move && let MoveWho::Entity(EntityHandle::Boat(i)) = target {
                let boat = &self.boats[i];
                let fall = self.vehicle_fall_reset(boat.fall_distance, body.position, moved);
                self.boats.update(i, |boat| boat.fall_distance = fall);
            }
            self.apply_push(&mut gate, target, moved);
        }
        let Some(target) = self.resolve_push_target(selected) else {
            return;
        };
        if lid_move
            && placed
            && let MoveWho::Entity(EntityHandle::Boat(i)) = target
        {
            self.record_boat_lid_movement(i, moved);
        }
        self.set_pushed_on_ground(target, movement, moved, None);
        if lid_move && let MoveWho::Entity(EntityHandle::Boat(i)) = target {
            let boat = &self.boats[i];
            if self.boat_can_simulate_movement(boat.id) {
                let fall = self.boat_fall(boat, moved.y);
                self.boats.update(i, |boat| boat.fall_distance = fall);
            }
        }
        if let Some(target) =
            self.finish_pushed_restitution(selected, target, body.position, movement, moved)
        {
            self.release_after_block_move(target);
        }
    }

    /// A mod's `Entity.move(cause, movement)` for the listed entity `handle`.
    pub(in crate::game) fn move_listed_entity(
        &mut self,
        handle: EntityHandle,
        cause: MoverType,
        movement: Vec3,
    ) -> Vec3 {
        let target = MoveWho::Entity(handle);
        let selected = self.stable_push_target(target).expect("listed move target");
        let from = with_entity!(self, handle, entity => entity.position());
        let body = self.listed_body(handle, None);
        if self.pushed_no_physics(target) {
            let mut gate = self.entity_move(target, cause, movement);
            let movement = gate.delta();
            self.apply_push(&mut gate, target, movement);
            if let Some(target) = self.resolve_push_target(selected) {
                self.release_after_block_move(target);
            }
        } else if let Some(direction) = (cause == MoverType::Piston)
            .then(|| first_axis_direction(movement))
            .flatten()
        {
            let mut pricked = Vec::new();
            self.move_pushed(
                target,
                body.bounds,
                [body.position.x, body.position.z],
                body.step_height,
                body.on_ground,
                movement,
                direction,
                &mut std::collections::HashMap::new(),
                &mut pricked,
            );
            self.hurt_cactus_pricked_vehicles(pricked);
        } else {
            let stand = match handle {
                EntityHandle::ArmorStand(i) => Some(self.armor_stands[i].velocity),
                _ => None,
            };
            self.move_pushed_body(cause, target, body, movement, false, &mut None);
            if let Some(velocity) = stand
                && let Some(MoveWho::Entity(EntityHandle::ArmorStand(i))) =
                    self.resolve_push_target(selected)
            {
                self.armor_stands[i].velocity = velocity;
            }
        }
        if let Some(MoveWho::Entity(handle)) = self.resolve_push_target(selected) {
            with_entity!(self, handle, entity => entity.position()).subtract(from)
        } else {
            Vec3::ZERO
        }
    }

    pub(in crate::game) fn push_for_piston(
        &mut self,
        (pos, piston, new_progress): Push,
        targets: &[PushTarget],
        steps: &StepHeights,
        accumulators: &mut std::collections::HashMap<(u32, JoinSeq), PistonMovement>,
        pricked: &mut Vec<u32>,
    ) {
        let delta_progress = f64::from(new_progress - piston.progress);
        if delta_progress <= 0.0 {
            return;
        }
        let Some(bindings) = self.collision.pistons() else {
            return;
        };
        let Ok(state) = bindings.collision_related_state(piston.shape_state()) else {
            return;
        };
        let mut scene = SceneCollisions {
            world: &self.world,
            binding: self.collision,
            subject: None,
        };
        let Ok(shape) = self
            .collision
            .collision_shape(&mut scene, state, [pos.x, pos.y, pos.z])
        else {
            return;
        };
        if shape.is_empty() {
            return;
        }
        let movement = piston.movement_direction();
        let boxes: Vec<Aabb> = shape.boxes().to_vec();
        let facts = self.res.facts();
        let slime = &*facts.get(piston.moved_state).name == "minecraft:slime_block";
        let honey = &*facts.get(piston.moved_state).name == "minecraft:honey_block";
        let sticky_top = if honey {
            let mut scene = SceneCollisions {
                world: &self.world,
                binding: self.collision,
                subject: None,
            };
            self.collision
                .collision_shape(&mut scene, piston.moved_state, [pos.x, pos.y, pos.z])
                .ok()
                .and_then(|s| s.bounds())
                .map_or(0.0, |b| b.max_position().y)
        } else {
            0.0
        };

        let search = shape.bounds().map(|bounds| {
            piston_push::movement_search_area(
                piston_push::move_by_position_and_progress(pos, bounds, piston.shape_state()),
                movement,
                delta_progress,
            )
        });
        targets.iter().copied().for_each(|selected| {
            let Some(target) = self.resolve_push_target(selected) else {
                return;
            };
            let Some(body) = self.piston_body(target, Some(pos), steps) else {
                return;
            };
            if search.is_some_and(|area| !area.intersects(body.bounds)) {
                return;
            }
            let amount = piston_push::collided_amount(
                &boxes,
                pos,
                piston.shape_state(),
                new_progress,
                movement,
                body.bounds,
            );
            if slime && search.is_some_and(|area| area.intersects(body.bounds)) {
                self.bounce(target, movement);
            }
            if amount > 0.0 {
                let step = piston_push::step_vector(movement);
                let before = body.bounds;
                self.move_pushed(
                    target,
                    before,
                    [body.position.x, body.position.z],
                    body.step_height,
                    body.on_ground,
                    step.scale(amount),
                    movement,
                    accumulators,
                    pricked,
                );
                if !piston.extending
                    && piston.source
                    && let Some(target) = self.resolve_push_target(selected)
                    && let Some(after) = self.piston_body(target, Some(pos), steps)
                    && let Some(fix) = piston_push::fix_within_piston_base(
                        pos,
                        after.bounds,
                        movement,
                        delta_progress,
                    )
                {
                    let back = piston_push::step_vector(movement.opposite());
                    self.move_pushed(
                        target,
                        after.bounds,
                        [after.position.x, after.position.z],
                        after.step_height,
                        after.on_ground,
                        back.scale(fix),
                        movement,
                        accumulators,
                        pricked,
                    );
                }
            }
        });
        // Vanilla selects honey passengers after all collided entities have moved.
        if honey && movement.normal()[1] == 0 {
            let area = piston_push::sticky_area(pos, piston.shape_state(), sticky_top);
            for &selected in targets {
                let Some(target) = self.resolve_push_target(selected) else {
                    continue;
                };
                let Some(body) = self.piston_body(target, Some(pos), steps) else {
                    continue;
                };
                if piston_push::sticky_entity_selected(
                    area,
                    body.bounds,
                    body.piston_reaction,
                    body.on_ground,
                    body.supported,
                    body.position,
                ) {
                    self.move_pushed(
                        target,
                        body.bounds,
                        [body.position.x, body.position.z],
                        body.step_height,
                        body.on_ground,
                        piston_push::step_vector(movement).scale(delta_progress),
                        movement,
                        accumulators,
                        pricked,
                    );
                }
            }
        }
    }

    pub(in crate::game) fn stable_push_target(&self, hint: MoveWho) -> Option<PushTarget> {
        let (id, join) = match hint {
            MoveWho::Player => (self.player.as_ref()?.entity_id as u32, None),
            MoveWho::Guest(conn) => (self.lan.as_ref()?.hub.guest(conn)?.entity_id as u32, None),
            MoveWho::Entity(handle) => with_entity!(self, handle, entity => {
                (entity.id(), Some(entity.join_seq()))
            }),
        };
        Some(PushTarget { hint, id, join })
    }

    pub(in crate::game) fn resolve_push_target(
        &self,
        PushTarget { hint, id, join }: PushTarget,
    ) -> Option<MoveWho> {
        match hint {
            MoveWho::Player => self
                .player
                .as_ref()
                .filter(|p| p.entity_id as u32 == id)
                .map(|_| hint),
            MoveWho::Guest(conn) => {
                let hub = &self.lan.as_ref()?.hub;
                hub.guest(conn)
                    .filter(|g| g.entity_id as u32 == id && hub.here(g))
                    .map(|_| hint)
            }
            MoveWho::Entity(handle) => {
                let handle = self.refresh_entity_handle(id, handle)?;
                with_entity!(self, handle, entity => {
                    (!entity.is_removed() && Some(entity.join_seq()) == join
                        && !entity.selector_target().is_some_and(|named| self.persist.removals.contains(&named)))
                        .then_some(MoveWho::Entity(handle))
                })
            }
        }
    }

    /// `Entity.getStepHeight` for each body, read once.
    pub(in crate::game) fn push_step_heights(&mut self) -> StepHeights {
        let player = self
            .player
            .as_mut()
            .and_then(|p| p.attribute("minecraft:step_height").ok())
            .map_or(0.6, |v| v as f32);
        let mut guests = std::collections::HashMap::new();
        if let Some(host) = self.lan.as_mut() {
            host.hub.guests.update_all(|g| {
                let step = g
                    .motion
                    .as_mut()
                    .and_then(|m| m.attribute("minecraft:step_height").ok())
                    .map_or(0.6, |v| v as f32);
                guests.insert(g.conn, step);
            });
        }
        StepHeights { player, guests }
    }

    /// The entities a moving block can run into: the local player, LAN guests, then listed entities.
    pub(in crate::game) fn push_targets(&self) -> impl Iterator<Item = MoveWho> + use<> {
        let player = self.player.is_some().then_some(MoveWho::Player);
        let guests: Vec<MoveWho> = self.lan.as_ref().map_or_else(Vec::new, |host| {
            host.hub
                .guests
                .iter()
                .filter(|g| host.hub.here(g) && g.loaded && !g.dead)
                .map(|g| MoveWho::Guest(g.conn))
                .collect()
        });
        player
            .into_iter()
            .chain(guests)
            .chain(self.entity_handles().map(MoveWho::Entity))
    }

    /// Pistons and shulker boxes skip bodies that ignore piston reactions.
    pub(in crate::game) fn piston_body(
        &self,
        target: MoveWho,
        pos: Option<BlockPos>,
        steps: &StepHeights,
    ) -> Option<Body> {
        let body = match target {
            // PistonMovingBlockEntity collects a resident stand regardless of health.
            MoveWho::Entity(handle @ EntityHandle::ArmorStand(i)) if pos.is_some() => {
                (!self.armor_stands[i].is_removed()).then(|| self.listed_body(handle, pos))
            }
            _ => self.push_body(target, pos, steps),
        }?;
        (body.piston_reaction != PushReaction::IgnoreEntity).then_some(body)
    }

    /// What the push needs from a live, non-spectating body.
    pub(in crate::game) fn push_body(
        &self,
        target: MoveWho,
        pos: Option<BlockPos>,
        steps: &StepHeights,
    ) -> Option<Body> {
        match target {
            MoveWho::Player => {
                let p = self.player.as_ref()?;
                if p.spectator {
                    return None;
                }
                Some(Body {
                    bounds: p.bounds(),
                    position: p.position,
                    on_ground: p.ground.on_ground,
                    step_height: steps.player,
                    piston_reaction: PushReaction::PushPull,
                    supported: pos.is_some() && p.ground.supporting_block == pos,
                })
            }
            MoveWho::Guest(conn) => {
                let host = self.lan.as_ref()?;
                let g = host.hub.guests.iter().find(|g| g.conn == conn)?;
                if g.dead || g.game_mode == 3 {
                    return None;
                }
                let p = g.position;
                Some(Body {
                    bounds: g.dimensions().bounds(Vec3::new(p[0], p[1], p[2])),
                    position: Vec3::new(p[0], p[1], p[2]),
                    on_ground: g.on_ground,
                    step_height: steps.guests.get(&conn).copied().unwrap_or(0.6),
                    piston_reaction: PushReaction::PushPull,
                    supported: g
                        .motion
                        .as_ref()
                        .is_some_and(|m| pos.is_some() && m.ground.supporting_block == pos),
                })
            }
            MoveWho::Entity(handle) => with_entity!(self, handle, entity => entity.is_alive())
                .then(|| self.listed_body(handle, pos)),
        }
    }

    /// `Entity.move(MoverType.PISTON, movement)` for one pushed entity.
    pub(in crate::game) fn move_pushed(
        &mut self,
        target: MoveWho,
        before: Aabb,
        at: [f64; 2],
        step_height: f32,
        on_ground: bool,
        movement: Vec3,
        piston_direction: Direction,
        accumulators: &mut std::collections::HashMap<(u32, JoinSeq), PistonMovement>,
        pricked: &mut Vec<u32>,
    ) {
        let cause = MoverType::Piston;
        if target == MoveWho::Player {
            with_piston_noclip(piston_direction, || {
                let _ = self.server_move_local_player(cause, movement, false);
            });
            let after = self.player.as_ref().map_or(before, |p| p.bounds());
            self.piston_block_triggers(before, after);
            self.piston_cactus_contact(target, before, after, pricked);
            return;
        }
        let Some(selected) = self.stable_push_target(target) else {
            return;
        };
        let from = match target {
            MoveWho::Entity(handle) => with_entity!(self, handle, entity => entity.position()),
            _ => Vec3::ZERO,
        };
        let mut gate = self.entity_move(target, cause, movement);
        if gate.skipped() {
            return;
        }
        let movement = gate.delta();
        let Some(limited) = self.limit_piston(target, movement, accumulators) else {
            return;
        };
        if limited == Vec3::ZERO {
            with_piston_noclip(piston_direction, || {
                self.release_after_block_move(target);
                if let MoveWho::Entity(EntityHandle::Tnt(i)) = target {
                    let from = self.tnts[i].position;
                    self.tnt_piston_block_effects(i, from);
                }
                if let MoveWho::Entity(EntityHandle::Projectile(i)) = target {
                    let at = self.creatures.projectiles[i].position;
                    self.projectile_piston_block_effects(i, at);
                }
                self.piston_block_triggers(before, before);
                if let Some(target) = self.resolve_push_target(selected) {
                    self.piston_cactus_contact(target, before, before, pricked);
                }
            });
            return;
        }
        let projectile_from = match target {
            MoveWho::Entity(EntityHandle::Projectile(i)) => {
                self.creatures.projectiles.update(i, |p| {
                    if p.stuck_speed.length_squared() > 1.0e-7 {
                        p.stuck_speed = Vec3::ZERO;
                        p.velocity = Vec3::ZERO;
                    }
                    Some((i, p.position))
                })
            }
            _ => None,
        };
        let tnt = match target {
            MoveWho::Entity(EntityHandle::Tnt(i)) => {
                let t = &mut self.tnts[i];
                t.take_stuck();
                Some((
                    i,
                    t.position,
                    t.powder_snow_subject(),
                    t.collision_context(),
                ))
            }
            _ => None,
        };
        let entities = self.pushed_entity_colliders(target);
        with_piston_noclip(piston_direction, || {
            let collision = tnt
                .map(|(_, _, subject, context)| (context, subject))
                .or_else(|| self.pushed_living_collision(target));
            let resolved = pushed_movement(
                &mut gate,
                SceneCollisions {
                    world: &self.world,
                    binding: self.collision,
                    subject: collision.map(|(_, subject)| subject),
                },
                self.border.geometry(),
                at,
                before,
                limited,
                step_height,
                on_ground,
                collision.map(|(context, _)| context),
                &entities,
            )
            .unwrap_or(Vec3::ZERO);

            let moved = resolved.length_squared() > 1.0e-7
                || limited.length_squared() - resolved.length_squared() < 1.0e-7;
            if moved {
                self.apply_push(&mut gate, target, resolved);
            }
            let after = if moved {
                before.move_by(resolved)
            } else {
                before
            };
            let Some(target) = self.resolve_push_target(selected) else {
                self.piston_block_triggers(before, after);
                return;
            };
            self.set_pushed_on_ground(target, limited, resolved, tnt.map(|(_, _, s, c)| (s, c)));
            if let Some(target) =
                self.finish_pushed_restitution(selected, target, from, limited, resolved)
            {
                self.release_after_block_move(target);
                if let (MoveWho::Entity(EntityHandle::Tnt(i)), Some((_, from, _, _))) =
                    (target, tnt)
                {
                    self.finish_tnt_piston_move(i, from);
                }
                if let (MoveWho::Entity(EntityHandle::Projectile(i)), Some((_, from))) =
                    (target, projectile_from)
                {
                    self.projectile_piston_block_effects(i, from);
                }
            }
            self.piston_block_triggers(before, after);
            if let Some(target) = self.resolve_push_target(selected) {
                self.piston_cactus_contact(target, before, after, pricked);
            }
        });
    }

    /// `Entity.limitPistonMovement`: each axis may travel at most 0.51 blocks per game tick.
    pub(in crate::game) fn limit_piston(
        &mut self,
        target: MoveWho,
        movement: Vec3,
        accumulators: &mut std::collections::HashMap<(u32, JoinSeq), PistonMovement>,
    ) -> Option<Vec3> {
        let time = self.world.game_time;
        let limit = |state: &mut PistonMovement| state.limit(movement, || Ok::<i64, ()>(time)).ok();
        match target {
            MoveWho::Guest(conn) => {
                let host = self.lan.as_mut()?;
                limit(&mut host.hub.guests.motion_mut(conn)?.piston)
            }
            MoveWho::Entity(EntityHandle::Mob(i)) => limit(&mut self.mobs[i].piston),
            MoveWho::Entity(handle) => {
                let key = with_entity!(self, handle, entity => (entity.id(), entity.join_seq()));
                limit(accumulators.entry(key).or_default())
            }
            MoveWho::Player => unreachable!("local player limits its own move"),
        }
    }

    pub(in crate::game) fn apply_push(
        &mut self,
        gate: &mut MoveGate,
        target: MoveWho,
        movement: Vec3,
    ) {
        if gate.skipped() {
            return;
        }
        let id = match target {
            MoveWho::Player => {
                if let Some(p) = self.player.as_mut() {
                    p.position = p.position.add_vector(movement);
                }
                u32::try_from(self.own_entity_id()).ok()
            }
            MoveWho::Guest(conn) => self.lan.as_mut().and_then(|host| {
                host.hub.update_guest(conn, |g| {
                    for (slot, delta) in g.position.iter_mut().zip(movement.components()) {
                        *slot += delta;
                    }
                    if let Some(m) = g.motion.as_deref_mut() {
                        m.position = m.position.add_vector(movement);
                    }
                    g.entity_id as u32
                })
            }),
            MoveWho::Entity(handle) => {
                let id = with_entity_mut!(self, handle, entity => {
                    entity.move_entity(gate, movement);
                    entity.id()
                });
                Some(id)
            }
        };
        if let Some(id) = id {
            self.entity_section_moved_id(id);
            self.tell_entity_sections();
        }
    }

    /// `causeBounce`: a slime block sets the velocity along the movement axis.
    pub(in crate::game) fn bounce(&mut self, target: MoveWho, movement: Direction) {
        match target {
            MoveWho::Player => {
                if let Some(p) = self.player.as_mut() {
                    p.velocity = piston_push::bounce_velocity(movement, p.velocity);
                }
            }
            MoveWho::Guest(conn) => {
                if let Some(host) = self.lan.as_mut()
                    && let Some(motion) = host.hub.guests.motion_mut(conn)
                {
                    motion.velocity = piston_push::bounce_velocity(movement, motion.velocity);
                }
            }
            MoveWho::Entity(handle) => {
                with_entity_mut!(self, handle, entity => Entity::piston_slime_bounce(entity, movement))
            }
        }
    }

    /// Finish the collided base move shared by pistons, shulker lids and native moves.
    /// A piston's direct slime launch is not a collided move.
    pub(in crate::game) fn finish_pushed_restitution(
        &mut self,
        selected: PushTarget,
        target: MoveWho,
        from: Vec3,
        wanted: Vec3,
        moved: Vec3,
    ) -> Option<MoveWho> {
        let MoveWho::Entity(handle) = target else {
            return Some(target);
        };
        let server = self.is_logical_server();
        if let EntityHandle::Mob(i) = handle {
            let mob = &self.mobs[i];
            // The controlling player's side owns a ridden mob's restitution.
            // Even an observing copy updates support after a vertical move.
            let authoritative = if server {
                use super::commands::Target;
                let own = (self.riding == Some(mob.id))
                    .then(|| self.riding_player())
                    .flatten();
                let guests = self.lan.as_ref().into_iter().flat_map(|host| {
                    host.hub
                        .guests
                        .iter()
                        .filter(|g| host.hub.here(g) && g.vehicle == Some(mob.id))
                });
                let mut players = guests
                    .map(|g| (g.entity_id as u32, Target::Guest(g.conn)))
                    .chain(own.map(|(id, _)| (id, Target::Player)))
                    .peekable();
                let controller = if players.peek().is_some() {
                    self.first_seated_passenger(mob.id)
                        .filter(|&(_, _, is_player)| is_player)
                        .and_then(|(_, first, _)| players.find(|(id, _)| *id == first))
                } else {
                    None
                };
                !controller.is_some_and(|(_, rider)| self.native_player_controls(mob, rider))
            } else {
                false
            };
            if authoritative || wanted.y.abs() > 0.0 {
                let mut paused_hurt = None;
                let mut mobs = self.mobs.lift();
                let mut lent = sunrise_sim::health::LentLevel::take(&mut self.world);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let world = self.mob_world_base(&[]);
                    let mut level = sunrise_sim::health::DamageLevel::lent(
                        &self.world,
                        self.pack.assets.registry,
                        &mut lent,
                        Some(world_items!(self)),
                        Some(&self.pack.block_registries),
                        &self.damage_hooks,
                    )
                    .with_reads(crate::game::entity::hit_reads_of!(self))
                    .with_mob_hurt_handoff(&mut paused_hurt);
                    mobs[i].update_pushed_support(&world, wanted, moved);
                    if authoritative {
                        mobs[i].restitute_movement(&world, &mut level, wanted, moved);
                    }
                }));
                lent.give_back(&mut self.world);
                self.mobs
                    .restore(mobs, sunrise_sim::join_seq::RestoreOrder::SurvivorsFirst);
                if let Err(payload) = result {
                    std::panic::resume_unwind(payload);
                }
                if let Some(hit) = paused_hurt
                    && let Some(index) = self.mob_index.find(&self.mobs, hit.victim)
                {
                    self.side.paused_mob_hurt.0 = Some(hit);
                    self.finish_mob_damage_at(index);
                }
            }
            return self.resolve_push_target(selected);
        }
        // Stands retain support, but do not use nonliving restitution.
        if matches!(handle, EntityHandle::ArmorStand(_)) {
            return Some(target);
        }
        let can_simulate = match handle {
            EntityHandle::Boat(i) => self.boat_can_simulate_movement(self.boats[i].id),
            EntityHandle::Item(_)
            | EntityHandle::Tnt(_)
            | EntityHandle::Orb(_)
            | EntityHandle::Falling(_)
            | EntityHandle::Projectile(_)
            | EntityHandle::Rocket(_)
            | EntityHandle::Trident(_) => true,
            _ => server,
        };
        let Some((id, feet, effect)) = with_entity!(self, handle, entity => {
            (!entity.is_removed()).then(|| entity.ground_state()).flatten().map(|ground| {
                let feet = entity.position();
                (entity.id(), feet, self.body_motion(&game_side!(self)).legacy_contact(feet, ground))
            })
        }) else {
            return Some(target);
        };
        if server {
            match handle {
                EntityHandle::Tnt(i) => {
                    let t = &mut self.tnts[i];
                    if t.check_fall_damage(moved.y) {
                        let bounds = t.bounds();
                        self.nonliving_fall_on(id, feet, effect);
                        self.post_landing_event(feet, bounds, false);
                    }
                }
                EntityHandle::Falling(i) => self.finish_falling_piston_move(i, moved),
                EntityHandle::Cart(i) => {
                    let (distance, landing) = self.cart_move_fall(&self.carts[i], from, moved);
                    self.carts.update(i, |c| c.fall_distance = distance);
                    if !self.finish_cart_landing(landing, effect, |game, power| {
                        let mut cart = game.carts[i].clone();
                        game.explode_cart(&mut cart, power)
                    }) {
                        return None;
                    }
                }
                _ => {}
            }
        }
        let MoveWho::Entity(handle) = self.resolve_push_target(selected)? else {
            return None;
        };
        let differs = sunrise_world::entity_move::different;
        if !can_simulate
            || !(wanted.y != moved.y || differs(wanted.x, moved.x) || differs(wanted.z, moved.z))
        {
            return Some(MoveWho::Entity(handle));
        }
        let drag = match handle {
            EntityHandle::Boat(_) => 1.0_f32,
            EntityHandle::Cart(_) => 0.95_f32,
            _ => 0.98_f32,
        };
        let boat_name = match handle {
            EntityHandle::Boat(i) => Some(self.boats[i].shared_type_name()),
            _ => None,
        };
        let kind = match handle {
            EntityHandle::Boat(_) => boat_name.as_deref()?,
            EntityHandle::Cart(i) => self.carts[i].kind.id(),
            EntityHandle::Projectile(i) => self.creatures.projectiles[i].entity_type(),
            EntityHandle::Item(_) => "minecraft:item",
            EntityHandle::Orb(_) => "minecraft:experience_orb",
            EntityHandle::Falling(_) => "minecraft:falling_block",
            EntityHandle::Tnt(_) => "minecraft:tnt",
            EntityHandle::Rocket(_) => "minecraft:firework_rocket",
            EntityHandle::Trident(_) => "minecraft:trident",
            EntityHandle::ArmorStand(_) => super::armor_stands::TYPE_ID,
            EntityHandle::Mob(_) | EntityHandle::General(_) => {
                return Some(MoveWho::Entity(handle));
            }
        };
        let response = with_entity!(self, handle, entity => {
            self.body_motion(&game_side!(self)).restitute((id, kind),
                sunrise_sim::motion::BodyMove { wanted, moved, effect },
                entity.get_gravity(), drag, entity.velocity(), entity.body_fall_distance())
        });
        let velocity = self.server_finish_body_restitution((id, kind), feet, response);
        let MoveWho::Entity(handle) = self.resolve_push_target(selected)? else {
            return None;
        };
        with_entity_mut!(self, handle, entity => *entity.velocity_mut() = velocity);
        Some(MoveWho::Entity(handle))
    }

    /// Server arm of [`Game::side_move_local_player`]: `Entity.move(mover, …)`
    /// with acting-guest landing handoff (`trample_farmland`, `finish_guest_damage_before_resume`).
    pub(in crate::game) fn server_move_local_player(
        &mut self,
        mover: MoverType,
        movement: Vec3,
        auto_jump: bool,
    ) -> sunrise_client_core::Result<Option<Vec3>> {
        use sunrise_sim::landing::PlayerLandingPause;

        let Some(player) = self.player.as_ref() else {
            return Ok(None);
        };
        let (before, id, conn, dim) = (
            player.position,
            player.entity_id,
            self.side.lan_acting,
            self.dim,
        );
        macro_rules! motion_phase {
            ($world:ident, $body:expr) => {{
                if self.side.lan_acting != conn || self.dim != dim {
                    return Ok(None);
                }
                let entities = self.entity_collisions(Mover::Player);
                let section_scope = self.player_section_scope();
                let Some(player) = self.player.as_mut().filter(|p| p.entity_id == id) else {
                    return Ok(None);
                };
                let side = game_side!(self);
                let _sections = section_scope.map(|scope| scope.begin(&mut self.native_lifecycle));
                let mut $world = MotionWorld {
                    side: &side,
                    entities: &entities,
                    player,
                    scene: &mut self.world,
                    assets: &self.pack.assets,
                    collision_binding: self.collision,
                    resources: self.motion,
                    health: &self.health_resources,
                    loop_context: &self.loop_context,
                    damage_hooks: &self.damage_hooks,
                    tag_items: Some(world_items!(self)),
                    block_registries: Some(&self.pack.block_registries),
                };
                $body
            }};
        }
        let pause = motion_phase!(
            world,
            world.move_ordinary(
                &mut MoveGate::new(),
                mover,
                movement,
                auto_jump,
                conn.is_some(),
            )
        )?;
        if let Some(pause) = pause {
            let landing = match pause.landing {
                PlayerLandingPause::Trample(trample) => {
                    self.trample_farmland(id as u32, true, trample);
                    motion_phase!(world, world.finish_trample_fall(trample))?
                }
                PlayerLandingPause::Damage(landing) => Some(landing),
            };
            if let Some(landing) = landing {
                if self.finish_guest_damage_before_resume().is_err() {
                    return Ok(None);
                }
                motion_phase!(world, {
                    world.finish_guest_landing(landing)?;
                    pause.movement.finish(&mut world)
                })?;
            } else {
                motion_phase!(world, pause.movement.finish(&mut world))?;
            }
        }
        Ok(self
            .player
            .as_ref()
            .map(|player| player.position.subtract(before)))
    }

    /// `Block.pushEntitiesUp(old, new, level, pos)` before the block at `pos` changes.
    pub(in crate::game) fn push_entities_up(
        &mut self,
        pos: BlockPos,
        old: sunrise_world::registry::BlockStateId,
        new: sunrise_world::registry::BlockStateId,
        write: impl FnOnce(&mut Self) -> bool,
    ) -> bool {
        if new == old {
            return write(self);
        }
        let at = [pos.x, pos.y, pos.z];
        let shape = |state| {
            let mut scene = SceneCollisions {
                world: &self.world,
                binding: self.collision,
                subject: None,
            };
            self.collision.collision_shape(&mut scene, state, at).ok()
        };
        let (Some(old_shape), Some(new_shape)) = (shape(old), shape(new)) else {
            return write(self);
        };
        let Ok(added) = old_shape.join_unoptimized(&new_shape, ShapeOp::ONLY_SECOND) else {
            return write(self);
        };
        let added = added.moved(Vec3::new(
            f64::from(pos.x),
            f64::from(pos.y),
            f64::from(pos.z),
        ));
        let Some(bounds) = added.bounds() else {
            return write(self);
        };
        let mut lift: Vec<_> = self
            .push_targets()
            .filter_map(|target| {
                let (body, id, join) = match target {
                    MoveWho::Player => {
                        let p = self.player.as_ref().filter(|p| !p.spectator)?;
                        (p.bounds(), p.entity_id as u32, None)
                    }
                    MoveWho::Guest(conn) if Some(conn) != self.side.lan_acting => {
                        let g = self
                            .lan
                            .as_ref()?
                            .hub
                            .guest(conn)
                            .filter(|g| !g.spectator())?;
                        let [x, y, z] = g.position;
                        (
                            g.dimensions().bounds(Vec3::new(x, y, z)),
                            g.entity_id as u32,
                            None,
                        )
                    }
                    MoveWho::Guest(_) => return None,
                    MoveWho::Entity(handle) => with_entity!(self, handle, entity => {
                        if entity.is_removed() {
                            return None;
                        }
                        (Entity::bounds(entity), entity.id(), Some(entity.join_seq()))
                    }),
                };
                body.intersects(bounds).then_some((target, id, join, None))
            })
            .collect();
        for &mut (hint, id, join, ref mut moved) in &mut lift {
            let Some(target) = self.resolve_push_target(PushTarget { hint, id, join }) else {
                continue;
            };
            let body = match target {
                MoveWho::Player => self.player.as_ref().unwrap().bounds(),
                MoveWho::Guest(conn) => {
                    let g = self.lan.as_ref().unwrap().hub.guest(conn).unwrap();
                    let [x, y, z] = g.position;
                    g.dimensions().bounds(Vec3::new(x, y, z))
                }
                MoveWho::Entity(handle) => {
                    with_entity!(self, handle, entity => Entity::bounds(entity))
                }
            };
            let dy = 1.0 + added.collide(Axis::Y, body.move_by(Vec3::new(0.0, 1.0, 0.0)), -1.0);
            let (from, motion_y) = match target {
                MoveWho::Player => {
                    let p = self.player.as_mut().unwrap();
                    let from = p.position;
                    p.position.y += dy;
                    (from, None)
                }
                MoveWho::Guest(conn) => self
                    .lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .update_guest(conn, |g| {
                        let [x, y, z] = g.position;
                        g.position[1] += dy;
                        let motion_y = g.motion.as_mut().map(|m| {
                            let y = m.position.y;
                            m.position.y += dy;
                            y
                        });
                        (Vec3::new(x, y, z), motion_y)
                    })
                    .unwrap(),
                MoveWho::Entity(handle) => with_entity_mut!(self, handle, entity => {
                    let pos = entity.position_mut();
                    let from = *pos;
                    pos.y += dy;
                    (from, None)
                }),
            };
            let to = [from.x, from.y + dy, from.z];
            *moved = Some((from, to, motion_y));
            self.entity_section_moved_id(id);
        }
        if write(self) {
            for (_, id, _, moved) in lift {
                if let Some((from, to, _)) = moved {
                    self.entity_section_entered(id, from.components(), to);
                }
            }
            return true;
        }
        for (target, id, join, moved) in lift.into_iter().rev() {
            let Some((from, _, motion_y)) = moved else {
                continue;
            };
            let Some(target) = self.resolve_push_target(PushTarget {
                hint: target,
                id,
                join,
            }) else {
                continue;
            };
            match target {
                MoveWho::Player => self.player.as_mut().unwrap().position = from,
                MoveWho::Guest(conn) => {
                    self.lan.as_mut().unwrap().hub.update_guest(conn, |g| {
                        g.position = from.components();
                        if let (Some(m), Some(y)) = (&mut g.motion, motion_y) {
                            m.position.y = y;
                        }
                    });
                }
                MoveWho::Entity(handle) => with_entity_mut!(self, handle, entity => {
                    *entity.position_mut() = from;
                }),
            }
            self.entity_section_joined(id, from.components());
        }
        false
    }

    /// `ShulkerBoxBlockEntity.moveCollidedEntities` for the server's bodies.
    pub(in crate::game) fn push_shulker_entities(&mut self, pushes: Vec<ShulkerPush>) {
        for (_, facing, lid) in pushes {
            let (movement, area) = shulker_lid(facing, lid);
            self.lid_push(Lid::Box, area, movement);
        }
    }

    /// The ground recorded after placing the body during `Entity.move`.
    pub(in crate::game) fn set_pushed_on_ground(
        &mut self,
        target: MoveWho,
        wanted: Vec3,
        moved: Vec3,
        tnt: Option<(
            sunrise_level::collisions::PowderSnowSubject<'static>,
            sunrise_world::collision_context::CollisionContext<'static>,
        )>,
    ) {
        use sunrise_world::entity_move::different;
        let MoveWho::Entity(handle) = target else {
            return;
        };
        if let EntityHandle::Mob(i) = handle {
            self.mobs[i].ground.horizontal_collision =
                different(wanted.x, moved.x) || different(wanted.z, moved.z);
            return;
        }
        let authoritative = match handle {
            EntityHandle::Boat(i) => self.boat_can_simulate_movement(self.boats[i].id),
            _ => self.is_logical_server(),
        };
        if let EntityHandle::ArmorStand(i) = handle {
            let stand = &mut self.armor_stands[i];
            stand.ground.horizontal_collision =
                different(wanted.x, moved.x) || different(wanted.z, moved.z);
            if authoritative || wanted.y.abs() > 0.0 {
                let _ = stand.update_retained_ground(
                    &self.world,
                    self.collision,
                    &world_items!(self).entity_type_tags,
                    wanted.y < 0.0 && wanted.y != moved.y,
                    Some(moved),
                );
            }
            return;
        }
        let motion = sunrise_sim::motion::BodyMotion {
            world: &self.world,
            collision: self.collision,
            facts: self.res.facts(),
            blocks: Some(&self.res.motion().blocks),
            context: &self.loop_context,
            side: None,
        };
        with_entity_mut!(self, handle, entity => {
            let (at, bounds, id) = (entity.position(), entity.bounds(), entity.id());
            let (subject, context) = tnt.map_or_else(
                || {
                    let context = if matches!(handle, EntityHandle::Item(_)) {
                        sunrise_world::collision_context::CollisionContext::for_entity(
                            sunrise_world::collision_context::CollisionEntity {
                                identity: sunrise_world::entity_obstruction::EntityIdentity(u64::from(id)),
                                kind: sunrise_world::collision_context::EntityContextKind::Item,
                                y: at.y,
                                descending: false,
                                main_hand: None,
                            },
                            false,
                        )
                    } else {
                        sunrise_sim::motion::BodyMotion::entity_context(id, at)
                    };
                    (None, context)
                },
                |(subject, context)| (Some(subject), context),
            );
            if let Some(ground) = entity.ground_state_mut() {
                motion.record_move(
                    sunrise_sim::motion::BodyCollision {
                        footprint: (at, bounds),
                        movement: (wanted, moved),
                        authoritative,
                        context,
                        subject,
                    },
                    ground,
                );
            } else {
                entity.set_piston_on_ground(wanted.y < 0.0 && wanted.y != moved.y);
            }
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod order_tests {
    use crate::game::{Options, Progress, Settings, load_resources};
    use std::sync::Arc;
    use sunrise_core::geometry::Vec3;
    use sunrise_core::{geometry::Direction, position::BlockPos};
    use sunrise_level::chunk::Chunk;
    use sunrise_world::level::pistons::MovingPiston;

    #[test]
    #[ignore = "daily"]
    fn a_piston_pushes_a_resident_armor_stand_with_zero_health() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.build_registries();
            game.set_allow_commands(true);
            let registry = &game.pack.assets.registry;
            let stone = registry.default_state("minecraft:stone").unwrap();
            let moving = registry.default_state("minecraft:moving_piston").unwrap();
            let mut chunk = Chunk::empty(0, 0);
            chunk.set(5, 200, 5, moving.0);
            game.world.chunks.insert((0, 0), Arc::new(chunk));
            assert!(game.run_command("summon armor_stand 5.5 200 5.5 {Health:0f}"));
            assert_eq!(game.armor_stands[0].health, 0.0);

            let pos = BlockPos::new(5, 200, 5);
            let piston = MovingPiston {
                moved_state: stone,
                direction: Direction::East,
                extending: true,
                source: false,
                progress: 0.0,
                progress_o: 0.0,
                last_ticked: 0,
                death_ticks: 0,
                id: 1,
            };
            game.block_ticks.moving_pistons = vec![(
                pos,
                MovingPiston {
                    progress: 0.5,
                    ..piston
                },
            )];
            game.sync_moving_pistons();
            game.push_piston_entities(vec![(pos, piston, 0.5)]);
            assert_eq!(game.armor_stands[0].position.x, 5.5 + (0.25 + 0.01));
        });
    }

    #[test]
    #[ignore = "daily"]
    fn a_pushed_entity_meets_the_other_moving_blocks_as_they_are_at_its_push() {
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.game_dir = Some(
            std::env::temp_dir().join(format!("sunrise-piston-order-unit-{}", std::process::id())),
        );
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);
        let registry = &game.pack.assets.registry;
        let stone = registry.default_state("minecraft:stone").unwrap();
        let moving = registry.default_state("minecraft:moving_piston").unwrap();
        let mut chunk = Chunk::empty(0, 0);
        for x in 0..16 {
            for z in 0..16 {
                chunk.set(x, 199, z, stone.0);
            }
        }
        chunk.set(5, 200, 5, moving.0);
        chunk.set(6, 200, 4, moving.0);
        game.world.chunks.insert((0, 0), Arc::new(chunk));
        let piston = |id, direction, progress| MovingPiston {
            moved_state: stone,
            direction,
            extending: true,
            source: false,
            progress,
            progress_o: progress,
            last_ticked: 0,
            death_ticks: 0,
            id,
        };
        let mut moved = Vec::new();
        for far_first in [true, false] {
            let near = (BlockPos::new(5, 200, 5), piston(1, Direction::East, 0.0));
            let far = (BlockPos::new(6, 200, 4), piston(2, Direction::North, 0.5));
            game.block_ticks.moving_pistons = if far_first {
                vec![far, near]
            } else {
                vec![near, far]
            };
            game.sync_moving_pistons();
            game.tnts.clear();
            assert!(game.run_command("summon tnt 5.5 200 5.5 {fuse:80,Motion:[0.0d,0.0d,0.0d]}"));
            let before = game.tnts[0].position.x;
            let pushes: Vec<_> = game
                .block_ticks
                .moving_pistons
                .iter()
                .map(|&(pos, old)| (pos, old, old.progress + 0.5))
                .collect();
            for (_, piston) in &mut game.block_ticks.moving_pistons {
                piston.progress += 0.5;
            }
            game.push_piston_entities(pushes);
            moved.push(game.tnts[0].position.x - before);
        }
        let expected = f64::from(0.98_f32) * 0.5 + 0.01;
        assert!(
            (moved[0] - expected).abs() < 1e-9,
            "far stone gone first: {}",
            moved[0]
        );
        assert!(moved[1] < 0.02, "far stone still in the way: {}", moved[1]);
    }

    #[test]
    #[ignore = "daily"]
    fn a_cancelled_push_spends_none_of_the_piston_budget() {
        use mod_api::{EntityMoveEvent, Flow, Hooks, LoopContext};
        struct Gate(std::sync::Arc<std::sync::atomic::AtomicBool>);
        #[mod_api::__subscriptions]
        impl Hooks for Gate {
            fn entity_move(&self, _: &dyn LoopContext, _: &mut EntityMoveEvent<'_>) -> Flow {
                if self.0.load(std::sync::atomic::Ordering::Relaxed) {
                    Flow::Cancel
                } else {
                    Flow::Continue
                }
            }
        }
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "cancel_gate",
            Box::new(Gate(cancel.clone())),
        )]
        .into();
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.game_dir = Some(
            std::env::temp_dir().join(format!("sunrise-piston-budget-unit-{}", std::process::id())),
        );
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);
        let registry = &game.pack.assets.registry;
        let stone = registry.default_state("minecraft:stone").unwrap();
        let moving = registry.default_state("minecraft:moving_piston").unwrap();
        let mut chunk = Chunk::empty(0, 0);
        for x in 0..16 {
            for z in 0..16 {
                chunk.set(x, 199, z, stone.0);
            }
        }
        chunk.set(5, 200, 5, moving.0);
        game.world.chunks.insert((0, 0), Arc::new(chunk));
        let piston = |id, direction, progress| MovingPiston {
            moved_state: stone,
            direction,
            extending: true,
            source: false,
            progress,
            progress_o: progress,
            last_ticked: 0,
            death_ticks: 0,
            id,
        };
        mod_platform::testing::with_mods(mods, || {
            let mut travelled = Vec::new();
            for cancelled in [true, false] {
                cancel.store(cancelled, std::sync::atomic::Ordering::Relaxed);
                game.block_ticks.moving_pistons =
                    vec![(BlockPos::new(5, 200, 5), piston(1, Direction::East, 0.0))];
                game.sync_moving_pistons();
                game.tnts.clear();
                assert!(
                    game.run_command("summon tnt 5.5 200 5.5 {fuse:80,Motion:[0.0d,0.0d,0.0d]}")
                );
                let before = game.tnts[0].position.x;
                let pushes: Vec<_> = game
                    .block_ticks
                    .moving_pistons
                    .iter()
                    .map(|&(pos, old)| (pos, old, old.progress + 0.5))
                    .collect();
                for (_, piston) in &mut game.block_ticks.moving_pistons {
                    piston.progress += 0.5;
                }
                game.push_piston_entities(pushes);
                travelled.push(game.tnts[0].position.x - before);
            }
            assert!(
                travelled[0].abs() < 1e-9,
                "the cancelled push moved the TNT: {}",
                travelled[0]
            );
            assert!(
                travelled[1] > 0.4,
                "the second push of the same tick lost the budget: {}",
                travelled[1]
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn a_pushed_tnt_loses_velocity_into_a_wall() {
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.game_dir = Some(std::env::temp_dir().join(format!(
            "sunrise-piston-velocity-unit-{}",
            std::process::id()
        )));
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);
        let registry = &game.pack.assets.registry;
        let stone = registry.default_state("minecraft:stone").unwrap();
        let moving = registry.default_state("minecraft:moving_piston").unwrap();
        let mut chunk = Chunk::empty(0, 0);
        for x in 0..16 {
            for z in 0..16 {
                chunk.set(x, 199, z, stone.0);
            }
        }
        chunk.set(6, 200, 5, stone.0);
        chunk.set(5, 200, 5, moving.0);
        game.world.chunks.insert((0, 0), Arc::new(chunk));
        let piston = MovingPiston {
            moved_state: stone,
            direction: Direction::East,
            extending: true,
            source: false,
            progress: 0.0,
            progress_o: 0.0,
            last_ticked: 0,
            death_ticks: 0,
            id: 1,
        };
        game.block_ticks.moving_pistons = vec![(BlockPos::new(5, 200, 5), piston)];
        game.sync_moving_pistons();
        game.tnts.clear();
        assert!(game.run_command("summon tnt 5.5 200 5.5 {fuse:80,Motion:[0.4d,0.0d,0.0d]}"));
        assert!(
            (game.tnts[0].velocity.x - 0.4).abs() < 1.0e-9,
            "TNT starts with vx 0.4, got {:?}",
            game.tnts[0].velocity
        );
        let pushes: Vec<_> = game
            .block_ticks
            .moving_pistons
            .iter()
            .map(|&(pos, old)| (pos, old, old.progress + 0.5))
            .collect();
        for (_, piston) in &mut game.block_ticks.moving_pistons {
            piston.progress += 0.5;
        }
        game.push_piston_entities(pushes);
        assert!(
            game.tnts[0].velocity.x.abs() < 1.0e-9,
            "TNT velocity.x should be zeroed by the wall, got {:?}",
            game.tnts[0].velocity
        );
    }

    #[test]
    #[ignore = "daily"]
    fn a_pushed_tnt_bounces_on_slime() {
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.game_dir = Some(
            std::env::temp_dir().join(format!("sunrise-piston-slime-unit-{}", std::process::id())),
        );
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);
        let registry = &game.pack.assets.registry;
        let stone = registry.default_state("minecraft:stone").unwrap();
        let slime = registry.default_state("minecraft:slime_block").unwrap();
        let moving = registry.default_state("minecraft:moving_piston").unwrap();
        let mut chunk = Chunk::empty(0, 0);
        for x in 0..16 {
            for z in 0..16 {
                chunk.set(x, 199, z, stone.0);
            }
        }
        chunk.set(5, 199, 5, slime.0);
        chunk.set(5, 200, 5, moving.0);
        game.world.chunks.insert((0, 0), Arc::new(chunk));
        let piston = MovingPiston {
            moved_state: stone,
            direction: Direction::Down,
            extending: true,
            source: false,
            progress: 0.0,
            progress_o: 0.0,
            last_ticked: 0,
            death_ticks: 0,
            id: 1,
        };
        for (floor, y, vy, final_y, final_vy) in [
            (slime, 200.5, -0.4, 200.0, 0.3510000138133766_f64),
            (stone, 200.5, -0.4, 200.0, 0.0),
            (slime, 200.0, -0.4, 200.0, 0.3200000047683716),
            (slime, 200.00001, -0.4, 200.00001, 0.32000064476813395),
            (slime, 200.5, 0.3, 200.0, -0.0),
        ] {
            game.world.set(5, 199, 5, floor).unwrap();
            game.block_ticks.moving_pistons = vec![(BlockPos::new(5, 200, 5), piston)];
            game.sync_moving_pistons();
            game.tnts.clear();
            assert!(game.run_command(&format!(
                "summon tnt 5.5 {y} 5.5 {{fuse:80,Motion:[0.0d,{vy}d,0.0d]}}"
            )));
            assert_eq!(game.tnts[0].velocity.y, vy);
            let pushes: Vec<_> = game
                .block_ticks
                .moving_pistons
                .iter()
                .map(|&(pos, old)| (pos, old, old.progress + 0.5))
                .collect();
            for (_, piston) in &mut game.block_ticks.moving_pistons {
                piston.progress += 0.5;
            }
            game.push_piston_entities(pushes);
            assert_eq!(game.tnts[0].position, Vec3::new(5.5, final_y, 5.5));
            assert_eq!(game.tnts[0].velocity.y.to_bits(), final_vy.to_bits());
            assert!(game.tnts[0].ground.on_ground);
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod h255_mob_piston_ground_tests {
    use super::*;
    use crate::game::{Options, Progress, Settings, load_resources};
    use std::sync::Arc;
    use sunrise_core::{geometry::Vec3, position::ChunkPos, random::RandomSource};
    use sunrise_level::chunk::Chunk;
    use sunrise_sim::motion::PlayerMotion;

    pub(in crate::game) fn with_game(test: impl FnOnce(&mut crate::game::ServerGame<'_>)) {
        with_role_game(
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            test,
        );
    }

    pub(in crate::game) fn with_role_game(
        role: crate::game::execution_role::ExecutionRole,
        test: impl FnOnce(&mut crate::game::ServerGame<'_>),
    ) {
        mod_platform::testing::with_mods(Default::default(), || {
            let root =
                std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
            let settings = Settings::default();
            let loaded = load_resources(
                sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
                Vec::new(),
                None,
                &root,
                &settings,
                std::time::Instant::now(),
                false,
                crate::game::execution_role::ExecutionRole::IntegratedServer,
                &Progress::new(),
                &mut |_, _| unreachable!(),
            )
            .unwrap();
            let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
            options.resources = root.clone();
            options.game_dir = Some(
                std::path::PathBuf::from(std::env::var_os("SUNRISE_HOME").unwrap())
                    .join(format!("h255-{:?}", std::thread::current().id())),
            );
            options.execution_role = role;
            let mut game = crate::game::ServerGame::new(
                &loaded.resources,
                loaded.pack,
                options,
                settings.sim_settings(),
                None,
            )
            .unwrap();
            game.native_lifecycle
                .begin_open(mod_api::Authority::IntegratedServer);
            game.build_registries();
            game.set_allow_commands(true);

            let mut center = Chunk::empty(0, 0);
            center.lit = true;
            game.world.chunks.insert((0, 0), Arc::new(center));
            game.block_ticks
                .add_empty_chunk(ChunkPos::new(0, 0))
                .unwrap();
            assert!(game.run_command("difficulty normal"));
            assert!(game.run_command("forceload add 0 0"));
            for x in -1..=1 {
                for z in -1..=1 {
                    if x == 0 && z == 0 {
                        continue;
                    }
                    let mut chunk = Chunk::empty(x, z);
                    chunk.lit = true;
                    game.world.chunks.insert((x, z), Arc::new(chunk));
                    game.block_ticks
                        .add_empty_chunk(ChunkPos::new(x, z))
                        .unwrap();
                }
            }
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(2.5, 200.0, 2.5)).unwrap());
            game.load_ticking_neighbourhood(0, 0);
            game.creative = true;
            assert!(game.run_command("fill 0 199 0 31 199 31 stone"));
            game.world.random.set_seed(255);
            game.update_chunk_tickets();
            assert!(crate::game::ticking_chunk(&game.world, 0, 0));
            assert!(game.side.tickets.0.levels().block_ticking(0, 0));
            test(&mut game);
        });
    }

    pub(in crate::game) fn mob_index(game: &crate::game::ServerGame<'_>, n: i32) -> usize {
        let id = game
            .persist
            .uuids
            .iter()
            .find_map(|(id, u)| (*u == [255, 1, 0, n]).then_some(*id))
            .unwrap();
        game.mobs.iter().position(|m| m.id == id).unwrap()
    }

    #[test]
    #[ignore = "daily"]
    fn h255_real_honey_piston_lifecycle_clears_mob_ground_before_second_drag() {
        with_game(|game| {
            game.world.game_time = 0;
            for cmd in [
                "gamerule spawn_mobs false",
                "gamerule random_tick_speed 0",
                "gamerule advance_time false",
                "setblock 3 201 4 piston[facing=east]",
                "setblock 4 201 4 honey_block",
                "setblock 4 201 12 honey_block",
                "setblock 12 200 12 piston[facing=up]",
                "setblock 12 201 12 stone",
                "summon husk 4.5 202 4.5 {UUID:[I;255,1,0,1],Health:10f,NoAI:1b,NoGravity:1b,OnGround:1b,Motion:[0d,0d,0d]}",
                "summon husk 4.5 202 12.5 {UUID:[I;255,1,0,2],Health:10f,NoAI:1b,NoGravity:1b,OnGround:1b,Motion:[0d,0d,0d]}",
                "summon husk 12.5 202 12.5 {UUID:[I;255,1,0,3],Health:10f,NoAI:1b,NoGravity:1b,OnGround:0b,Motion:[0d,0d,0d]}",
            ] {
                assert!(game.run_command(cmd), "{cmd}");
            }
            let main = mob_index(game, 1);
            let idle = mob_index(game, 2);
            let vertical = mob_index(game, 3);
            let id = game.mobs[main].id;
            let age = game.mobs[main].age;
            assert!(game.mobs[main].ground.on_ground);
            assert!(game.mobs[idle].ground.on_ground);
            assert!(!game.mobs[vertical].ground.on_ground);
            assert_eq!(game.mobs[main].position, Vec3::new(4.5, 202.0, 4.5));
            assert_eq!(
                game.res
                    .facts()
                    .get(game.world.block(4, 201, 4))
                    .name
                    .as_ref(),
                "minecraft:honey_block"
            );
            assert!(game.run_command("setblock 3 201 5 redstone_block"));
            assert!(game.run_command("setblock 13 200 12 redstone_block"));
            let mut failures = Vec::new();
            for tick in 1..=2 {
                game.world.game_time += 1;
                game.update_chunk_tickets();
                assert!(game.entity_ticking_area()(4, 4));
                game.tick_blocks().unwrap();
                game.tick_creature_entities().unwrap();
                game.tick_moving_pistons().unwrap();
                let p = game
                    .block_ticks
                    .moving_pistons
                    .iter()
                    .find(|(p, _)| *p == BlockPos::new(5, 201, 4))
                    .expect("real moving honey block");
                assert_eq!(p.1.progress, 0.5 * tick as f32);
                assert_eq!(
                    p.1.moved_state,
                    game.pack
                        .assets
                        .registry
                        .default_state("minecraft:honey_block")
                        .unwrap()
                );
                assert_eq!(game.mobs[main].id, id);
                assert_eq!(game.uuid_for(id), [255, 1, 0, 1]);
                assert_eq!(game.mobs[main].age, age + tick);
                assert!(game.mobs[main].no_ai);
                assert_eq!(game.mobs[main].health, 10.0);
                assert_eq!(game.mobs[main].velocity, Vec3::ZERO);
                if game.mobs[main].ground.on_ground {
                    failures.push(format!("tick{tick}: actual Mob still grounded"));
                }
                if game.mobs[main].position != Vec3::new(5.0, 202.0, 4.5) {
                    failures.push(format!(
                        "tick{tick}: position {:?}",
                        game.mobs[main].position
                    ));
                }
                if game.mob_tree_nbt(main).get("OnGround") != Some(&sunrise_core::nbt::Tag::Byte(0))
                {
                    failures.push(format!("tick{tick}: saved ground is stale"));
                }
                assert_eq!(game.mobs[idle].position, Vec3::new(4.5, 202.0, 12.5));
                assert!(game.mobs[idle].ground.on_ground);
                assert_eq!(
                    game.mobs[vertical].position,
                    Vec3::new(12.5, if tick == 1 { 202.51 } else { 203.01 }, 12.5)
                );
                assert!(!game.mobs[vertical].ground.on_ground);
            }
            assert!(
                failures.is_empty(),
                "actual lifecycle failures: {failures:#?}"
            );
        });
    }

    pub(in crate::game) fn added_mob(
        game: &mut crate::game::ServerGame<'_>,
        n: i32,
        kind: &str,
        no_ai: bool,
        x: f64,
    ) -> usize {
        let tame = if kind == "horse" { ",Tame:1b" } else { "" };
        assert!(game.run_command(&format!(
            "summon {kind} {x} 202 4.5 {{UUID:[I;255,1,0,{n}],Health:10f,NoAI:{}b,NoGravity:1b,OnGround:1b,Motion:[0d,0d,0d]{tame}}}",
            i32::from(no_ai)
        )));
        let i = mob_index(game, n);
        assert_eq!(game.mobs[i].no_ai, no_ai);
        assert!(game.mobs[i].ground.on_ground);
        assert_eq!(game.uuid_for(game.mobs[i].id), [255, 1, 0, n]);
        i
    }

    pub(in crate::game) fn added_push(
        game: &mut crate::game::ServerGame<'_>,
        i: usize,
        movement: Vec3,
    ) {
        let m = &game.mobs[i];
        let (bounds, at, step, ground) = (
            m.bounds(),
            [m.position.x, m.position.z],
            m.kind.step_height as f32,
            m.ground.on_ground,
        );
        game.move_pushed(
            MoveWho::Entity(EntityHandle::Mob(i)),
            bounds,
            at,
            step,
            ground,
            movement,
            Direction::East,
            &mut std::collections::HashMap::new(),
            &mut Vec::new(),
        );
    }

    #[test]
    #[ignore = "daily"]
    fn a_stuck_arrow_whose_push_a_wall_stops_falls_when_nothing_holds_it() {
        with_game(|game| {
            assert!(game.run_command("setblock 6 202 4 minecraft:stone"));
            game.creatures.projectiles.clear();
            assert!(game.run_command("summon minecraft:arrow 5.75 202.5 4.5 {NoGravity:0b}"));
            let i = game
                .creatures
                .projectiles
                .iter()
                .position(|p| p.arrow.is_some())
                .expect("the arrow");
            game.creatures.projectiles.update(i, |p| {
                p.velocity = Vec3::ZERO;
                p.stuck = true;
                p.arrow.as_mut().unwrap().in_ground = true;
            });
            let p = &game.creatures.projectiles[i];
            let (bounds, at) = (p.bounds(), [p.position.x, p.position.z]);
            let before = p.position;
            game.move_pushed(
                MoveWho::Entity(EntityHandle::Projectile(i)),
                bounds,
                at,
                0.0,
                false,
                Vec3::new(0.3, 0.0, 0.0),
                Direction::East,
                &mut std::collections::HashMap::new(),
                &mut Vec::new(),
            );
            let p = &game.creatures.projectiles[i];
            assert_eq!(p.position, before, "the stone stops the push");
            assert!(
                !p.arrow.as_ref().unwrap().in_ground,
                "nothing holds it, so it falls"
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn h255_added_tnt_authority_and_exhausted_budget() {
        use crate::game::execution_role::ExecutionRole;
        for role in [
            ExecutionRole::IntegratedServer,
            ExecutionRole::DedicatedServer,
        ] {
            with_role_game(role, |game| {
                assert!(game.run_command("setblock 5 199 5 slime_block"));
                assert!(
                    game.run_command("summon tnt 5.5 200.5 5.5 {fuse:80,Motion:[0d,-0.4d,0d]}")
                );
                let mut budget = std::collections::HashMap::new();
                let push = |game: &mut crate::game::ServerGame<'_>,
                            budget: &mut std::collections::HashMap<_, _>,
                            movement| {
                    let tnt = &game.tnts[0];
                    game.move_pushed(
                        MoveWho::Entity(EntityHandle::Tnt(0)),
                        tnt.bounds(),
                        [tnt.position.x, tnt.position.z],
                        0.0,
                        tnt.ground.on_ground,
                        movement,
                        Direction::Down,
                        budget,
                        &mut Vec::new(),
                    );
                };
                push(game, &mut budget, Vec3::ZERO);
                assert_eq!(game.tnts[0].position.y, 200.5);
                assert_eq!(game.tnts[0].velocity.y, -0.4);
                push(game, &mut budget, Vec3::new(0.0, -0.51, 0.0));
                let expected = 0.3510000138133766_f64;
                assert_eq!(game.tnts[0].velocity.y.to_bits(), expected.to_bits());
                game.tnts[0].position.y = 200.5;
                game.tnts[0].velocity.y = -0.4;
                push(game, &mut budget, Vec3::new(0.0, -0.51, 0.0));
                assert_eq!(game.tnts[0].position.y, 200.5);
                assert_eq!(game.tnts[0].velocity.y, -0.4);
            });
        }
    }

    #[test]
    #[ignore = "daily"]
    fn h255_added_no_request_and_exhausted_budget_keep_state() {
        with_game(|game| {
            let i = added_mob(game, 20, "horse", true, 8.5);
            added_push(game, i, Vec3::ZERO);
            assert!(game.mobs[i].ground.on_ground);
            assert_eq!(game.mobs[i].position, Vec3::new(8.5, 202.0, 4.5));
            game.mobs[i].set_saddle(Some(sunrise_items::items::ItemStack::new(
                "minecraft:saddle",
                1,
            )));
            assert!(game.run_command("ride @s mount 000000ff-0000-0001-0000-000000000014"));
            assert_eq!(game.riding, Some(game.mobs[i].id));
            added_push(game, i, Vec3::new(0.51, 0.0, 0.0));
            assert!(
                game.mobs[i].ground.on_ground,
                "occupied path remains excluded"
            );
            assert_eq!(game.mobs[i].position, Vec3::new(9.01, 202.0, 4.5));
            assert!(game.run_command("setblock 9 201 4 slime_block"));
            game.mobs[i].position.y = 202.25;
            game.mobs[i].velocity = Vec3::new(0.0, -1.0, 0.0);
            game.mobs[i].sync_position = false;
            added_push(game, i, Vec3::new(0.0, -0.5, 0.0));
            assert_eq!(
                game.mobs[i].velocity.y, -1.0,
                "the controlling rider's client owns restitution"
            );
            assert_eq!(game.mobs[i].position.y, 202.0);
            assert!(
                game.mobs[i].ground.on_ground,
                "even the observer records vertical collision"
            );
            assert!(!game.mobs[i].sync_position);
            assert!(game.run_command("ride @s dismount"));
            assert!(game.riding.is_none());
            assert!(game.mobs[i].rider.is_none());
            assert!(game.mobs[i].extra.rider.is_none());
            assert!(game.mobs[i].extra.mount.is_none());
            assert!(!game.mobs[i].extra.passenger);
            let before = game.mobs[i].position;
            added_push(game, i, Vec3::new(0.25, 0.0, 0.0));
            assert_eq!(
                game.mobs[i].position, before,
                "real same-tick piston budget is exhausted"
            );
            assert!(
                game.mobs[i].ground.on_ground,
                "limitedZERO precedes the ground update"
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn h255_added_blocked_request_updates_ground_before_tiny_return() {
        with_game(|game| {
            assert!(game.run_command("fill 9 202 4 9 204 4 stone"));
            let width = game.res.mob_types().get("minecraft:husk").unwrap().width as f32;
            let x = 9.0 - f64::from(width) * 0.5;
            let i = added_mob(game, 21, "husk", true, x);
            assert_eq!(game.mobs[i].bounds().max_position().x, 9.0);
            let before = game.mobs[i].position;
            added_push(game, i, Vec3::new(0.25, 0.0, 0.0));
            assert_eq!(
                game.mobs[i].position, before,
                "actual loaded wall blocks the nonzero request"
            );
            assert!(
                !game.mobs[i].ground.on_ground,
                "Entity.move ground ordering, source/unit-only coverage"
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn h255_added_client_role_ai_and_vertical_ground_state() {
        use crate::game::execution_role::ExecutionRole as Role;
        for (role, no_ai, movement, on_ground) in [
            (
                Role::IntegratedServer,
                false,
                Vec3::new(0.25, 0.0, 0.0),
                false,
            ),
            (
                Role::IntegratedServer,
                true,
                Vec3::new(0.0, 0.25, 0.0),
                false,
            ),
        ] {
            with_role_game(role, |game| {
                let i = added_mob(game, 22, "husk", no_ai, 8.5);
                let before = game.mobs[i].position;
                added_push(game, i, movement);
                assert_eq!(game.mobs[i].position, before.add_vector(movement));
                assert_eq!(
                    game.mobs[i].ground.on_ground, on_ground,
                    "{role:?}, NoAI={no_ai}"
                );
            });
        }
    }

    #[test]
    #[ignore = "daily"]
    fn h255_added_partial_push_reads_slime_after_the_position_changes() {
        with_game(|game| {
            assert!(game.run_command("setblock 8 201 4 slime_block"));
            let i = added_mob(game, 23, "iron_golem", true, 8.5);
            game.mobs[i].position.y = 202.25;
            game.mobs[i].velocity = Vec3::new(0.0, -1.0, 0.0);
            game.mobs[i].sync_position = false;
            added_push(game, i, Vec3::new(0.0, -0.5, 0.0));
            assert_eq!(game.mobs[i].position, Vec3::new(8.5, 202.0, 4.5));
            assert!(game.mobs[i].velocity.y > 0.9);
            assert!(game.mobs[i].sync_position);
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod lid_push_tests {
    use super::h255_mob_piston_ground_tests::with_game;
    use super::{EntityHandle, Lid};

    use sunrise_core::geometry::{Aabb, Vec3};
    use sunrise_sim::mobs::{MobEvent, Species};

    fn shulker(game: &mut crate::game::ServerGame<'_>) -> u32 {
        assert!(game.run_command("summon minecraft:shulker 24 200 24 {NoAI:1b}"));
        game.mobs
            .iter()
            .find(|m| m.species() == Species::Shulker)
            .expect("the shulker")
            .id
    }

    fn area(x0: f64, z0: f64, x1: f64, z1: f64) -> Aabb {
        Aabb::new(Vec3::new(x0, 200.0, z0), Vec3::new(x1, 201.0, z1))
    }

    fn item_y(game: &crate::game::ServerGame<'_>) -> f64 {
        game.dropped.iter().next().expect("the item").position.y
    }

    fn arrow_y(game: &crate::game::ServerGame<'_>) -> f64 {
        let arrow = game
            .creatures
            .projectiles
            .iter()
            .find(|p| p.arrow.is_some());
        arrow.expect("the arrow").position.y
    }

    #[test]
    #[ignore = "daily"]
    fn forced_living_moves_land_booted_bodies_on_powder_snow() {
        use sunrise_core::position::BlockPos;
        use sunrise_world::entity_move::MoverType;

        crate::game::in_own_process(
            "game::server_side::piston_push_server::lid_push_tests::forced_living_moves_land_booted_bodies_on_powder_snow",
            || {
                with_game(|game| {
                    assert!(game.run_command("fill 8 200 8 15 200 8 powder_snow"));
                    for (i, cause) in [MoverType::Piston, MoverType::ShulkerBox]
                        .into_iter()
                        .enumerate()
                    {
                        let x = 8 + 4 * i as i32;
                        for (kind, x) in [("armor_stand", x), ("zombie", x + 2)] {
                            assert!(game.run_command(&format!(
                                r#"summon {kind} {x}.5 201.25 8.5 {{NoAI:1b,equipment:{{feet:{{id:"minecraft:leather_boots",count:1}}}}}}"#,
                            )));
                        }
                        for (handle, x) in [
                            (EntityHandle::ArmorStand(i), x),
                            (EntityHandle::Mob(i), x + 2),
                        ] {
                            let moved =
                                game.move_listed_entity(handle, cause, Vec3::new(0.0, -0.5, 0.0));
                            assert_eq!(moved.y, -0.25, "{cause:?}");
                            let ground = match handle {
                                EntityHandle::ArmorStand(i) => &game.armor_stands[i].ground,
                                EntityHandle::Mob(i) => &game.mobs[i].ground,
                                _ => unreachable!(),
                            };
                            assert!(ground.on_ground);
                            assert_eq!(ground.supporting_block, Some(BlockPos::new(x, 200, 8)));
                        }
                    }
                });
            },
        );
    }

    #[test]
    #[ignore = "daily"]
    fn shulker_push_updates_stand_support_in_both_join_orders() {
        use sunrise_core::position::BlockPos;

        crate::game::in_own_process(
            "game::server_side::piston_push_server::lid_push_tests::shulker_push_updates_stand_support_in_both_join_orders",
            || {
                with_game(|game| {
                    assert!(game.run_command("summon armor_stand 10.5 200.25 10.5"));
                    let shulker = shulker(game);
                    assert!(game.run_command("summon armor_stand 13.5 200.25 10.5"));
                    let shulker_join = game.mobs.iter().find(|m| m.id == shulker).unwrap().join;
                    assert!(game.armor_stands[0].join < shulker_join);
                    assert!(game.armor_stands[1].join > shulker_join);
                    let swept = area(9.0, 9.0, 15.0, 12.0);
                    game.handle_mob_events(vec![MobEvent::LidPush {
                        shulker,
                        area: swept,
                        delta: [0.0, -0.5, 0.0],
                    }])
                    .unwrap();
                    for (stand, x) in game.armor_stands.iter().zip([10, 13]) {
                        assert_eq!(stand.position.y, 200.0);
                        assert!(stand.ground.on_ground);
                        assert_eq!(
                            stand.ground.supporting_block,
                            Some(BlockPos::new(x, 199, 10))
                        );
                    }
                    game.handle_mob_events(vec![MobEvent::LidPush {
                        shulker,
                        area: swept,
                        delta: [0.25, 0.0, 0.0],
                    }])
                    .unwrap();
                    for (stand, x) in game.armor_stands.iter().zip([10.75, 13.75]) {
                        assert_eq!(stand.position.x, x);
                        assert!(!stand.ground.on_ground);
                        assert_eq!(stand.ground.supporting_block, None);
                    }
                });
            },
        );
    }

    #[test]
    #[ignore = "daily"]
    fn a_shulker_lid_pushes_an_item_a_boat_and_a_projectile() {
        with_game(|game| {
            let shulker = shulker(game);
            assert!(game.run_command(
                "summon minecraft:item 10.5 200 10.5 {Item:{id:\"minecraft:stone\",count:1}}"
            ));
            assert!(game.run_command("summon minecraft:oak_boat 13.5 200 10.5"));
            assert!(game.run_command("summon minecraft:arrow 16.5 200 10.5 {NoGravity:1b}"));
            game.handle_mob_events(vec![MobEvent::LidPush {
                shulker,
                area: area(9.0, 9.0, 18.0, 12.0),
                delta: [0.0, 0.25, 0.0],
            }])
            .unwrap();
            assert_eq!(item_y(game), 200.25, "the item rises with the lid");
            assert_eq!(
                game.boats[0].position.y, 200.25,
                "the boat rises with the lid"
            );
            assert_eq!(arrow_y(game), 200.25, "the arrow rises with the lid");
        });
    }

    #[test]
    #[ignore = "daily"]
    fn a_wall_stops_what_a_shulker_lid_pushes() {
        with_game(|game| {
            let shulker = shulker(game);
            assert!(game.run_command("setblock 10 201 10 minecraft:stone"));
            assert!(game.run_command("setblock 13 201 13 minecraft:stone"));
            assert!(game.run_command("setblock 2 202 2 minecraft:stone"));
            assert!(game.run_command(
                "summon minecraft:item 10.5 200 10.5 {Item:{id:\"minecraft:stone\",count:1}}"
            ));
            assert!(game.run_command("summon minecraft:pig 13.5 200 13.5 {NoAI:1b}"));
            let lid = Lid::Shulker(shulker);
            game.lid_push(lid, area(10.0, 10.0, 11.0, 11.0), Vec3::new(0.0, 1.0, 0.0));
            game.lid_push(lid, area(13.0, 13.0, 14.0, 14.0), Vec3::new(0.0, 0.5, 0.0));
            game.lid_push(lid, area(2.0, 2.0, 3.0, 3.0), Vec3::new(0.0, 0.5, 0.0));
            assert_eq!(item_y(game), 200.75, "the stone stops the item");
            let pig = game.mobs.iter().find(|m| m.species() == Species::Pig);
            let pig = pig.expect("the pig").position.y;
            let pig_height = f64::from(0.9_f32);
            assert!(
                (pig - (201.0 - pig_height)).abs() < 1.0e-9,
                "the stone stops the pig: {pig}"
            );
            let player = game.player.as_ref().unwrap().position.y;
            let player_height = f64::from(1.8_f32);
            assert!(
                (player - (202.0 - player_height)).abs() < 1.0e-9,
                "the stone stops the player: {player}"
            );
        });
    }

    fn dropped(game: &crate::game::ServerGame<'_>, item: &str) -> usize {
        game.dropped
            .iter()
            .filter(|d| &*d.stack.item == item)
            .count()
    }

    #[test]
    #[ignore = "daily"]
    fn a_shulker_lid_breaks_the_hanging_entities_it_meets() {
        with_game(|game| {
            let shulker = shulker(game);
            assert!(game.run_command(
                "summon minecraft:painting 10.5 200.5 10.5 {block_pos:[I;10,200,10],facing:2b,variant:\"minecraft:kebab\"}"
            ));
            assert!(game.run_command(
                "summon minecraft:item_frame 13.5 200.5 10.5 {Facing:2b,Item:{id:\"minecraft:stick\",count:1}}"
            ));
            assert!(game.run_command(
                "summon minecraft:item_frame 16.5 200.5 10.5 {Facing:2b,Fixed:1b,Item:{id:\"minecraft:stick\",count:1}}"
            ));
            assert!(game.run_command("summon minecraft:cushion 19.5 200 10.5"));
            assert_eq!(game.paintings.len(), 1);
            assert_eq!(game.item_frames.len(), 2);
            assert_eq!(game.cushions.len(), 1);
            game.handle_mob_events(vec![MobEvent::LidPush {
                shulker,
                area: area(9.0, 9.0, 21.0, 12.0),
                delta: [0.0, 0.25, 0.0],
            }])
            .unwrap();
            assert!(game.paintings.is_empty(), "the painting broke");
            assert_eq!(dropped(game, "minecraft:painting"), 1);
            assert_eq!(game.item_frames.len(), 1, "the loose frame broke");
            assert!(game.item_frames[0].fixed, "the fixed frame stays");
            assert!(game.item_frames[0].item.is_some(), "with its stick");
            assert_eq!(dropped(game, "minecraft:item_frame"), 1);
            assert_eq!(dropped(game, "minecraft:stick"), 1);
            assert!(game.cushions.is_empty(), "the cushion broke");
            assert_eq!(dropped(game, "minecraft:white_cushion"), 1);
        });
    }

    /// `isPassengerOfSameVehicle` on the server walk: a shulker riding a boat
    /// leaves the boat and the player riding it where they are.
    #[test]
    #[ignore = "daily"]
    fn a_shulker_lid_spares_what_shares_its_vehicle() {
        with_game(|game| {
            let shulker = shulker(game);
            assert!(game.run_command("summon minecraft:oak_boat 2.5 200 6.5"));
            let boat = game.boats[0].id;
            let i = game.mobs.iter().position(|m| m.id == shulker).unwrap();
            game.mobs[i].extra.mount = Some(boat);
            game.riding = Some(boat);
            let (around, lift) = (area(1.0, 1.0, 4.0, 8.0), Vec3::new(0.0, 0.25, 0.0));
            game.lid_push(Lid::Shulker(shulker), around, lift);
            assert_eq!(
                game.boats[0].position.y, 200.0,
                "the shulker's vehicle stays"
            );
            let player_y =
                |game: &crate::game::ServerGame<'_>| game.player.as_ref().unwrap().position.y;
            assert_eq!(player_y(game), 200.0, "its fellow passenger stays");
        });
    }

    #[test]
    #[ignore = "daily"]
    fn a_shulker_lid_leaves_no_physics_bodies_and_shulkers_be() {
        with_game(|game| {
            let shulker = shulker(game);
            assert!(game.run_command("summon minecraft:armor_stand 10.5 200 10.5 {NoGravity:1b}"));
            assert!(game.run_command("summon minecraft:shulker_bullet 13.5 200 10.5"));
            assert!(game.run_command("summon minecraft:shulker 16 200 10 {NoAI:1b}"));
            let lid = area(9.0, 9.0, 18.0, 12.0);
            let lift = Vec3::new(0.0, 0.25, 0.0);
            game.lid_push(Lid::Shulker(shulker), lid, lift);
            assert_eq!(
                game.armor_stands[0].position.y, 200.0,
                "the stand has no physics"
            );
            let bullet = game
                .creatures
                .projectiles
                .iter()
                .next()
                .expect("the bullet");
            assert_eq!(bullet.position.y, 200.0, "the bullet has no physics");
            let other = game
                .mobs
                .iter()
                .find(|m| m.id != shulker)
                .expect("the other shulker");
            assert_eq!(other.position.y, 200.0, "a shulker is not pushed");
            game.lid_push(Lid::Box, lid, lift);
            assert_eq!(
                game.armor_stands[0].position.y, 200.25,
                "a box's lid moves the stand"
            );
        });
    }
}
