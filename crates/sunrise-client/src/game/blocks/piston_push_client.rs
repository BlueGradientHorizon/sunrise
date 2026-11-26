//! Client-side local player movement, piston prediction (local player + driven boats),
//! shulker/shulker-box lid prediction (local player + all intersecting boats), and
//! moving-collider carry/shove prediction for the local player.
use super::{
    Game,
    entity::{Entity, EntityHandle},
    move_gate::MoveWho,
    remote_vehicles::passenger_root,
};
use crate::game::collision_source::Mover;
use crate::game::piston_push::{
    Body, Lid, Push, PushTarget, ShulkerPush, carrying::support_of, pushed_movement, shulker_lid,
};
use sunrise_core::geometry::{
    Aabb, Axis, COLLISION_EPSILON, Direction, Vec3, entity_collision_bounds,
};
use sunrise_core::position::BlockPos;
use sunrise_level::collisions::{SceneCollisions, with_piston_noclip};
use sunrise_sim::entity_rules::EntityRules;
use sunrise_sim::join_seq::{JoinSeq, Joined};
use sunrise_sim::motion::{BodyCollision, BodyMotion, BodyMove, MotionWorld};
use sunrise_world::moving_colliders::{self, Placed, Prepared, Shove};
use sunrise_world::{
    entity_move::MoverType, level::data::PushReaction, move_gate::MoveGate,
    piston_movement::PistonMovement, piston_push,
};

impl Game<'_, crate::game::client_side::ClientState> {
    /// Client arm of [`Game::side_move_local_player`]: `Entity.move(mover, …)`
    /// for the window's local player (no server guest-landing handoff).
    pub(in crate::game) fn client_move_local_player(
        &mut self,
        mover: MoverType,
        movement: Vec3,
        auto_jump: bool,
    ) -> sunrise_client_core::Result<Option<Vec3>> {
        let Some(player) = self.player.as_ref() else {
            return Ok(None);
        };
        let before = player.position;
        let entities = self.entity_collisions(Mover::Player);
        let section_scope = self.player_section_scope();
        let side_server = self.game_side_client();
        let Some(player) = self.player.as_mut() else {
            return Ok(None);
        };
        let side = game_side!(self, side_server);
        let _sections = section_scope.map(|scope| scope.begin(&mut self.native_lifecycle));
        let tag_items = self
            .server_items
            .as_deref()
            .unwrap_or_else(|| self.res.items());
        let mut world = MotionWorld {
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
            tag_items: Some(tag_items),
            block_registries: Some(&self.pack.block_registries),
        };
        let _ = world.move_ordinary(&mut MoveGate::new(), mover, movement, auto_jump, false)?;
        Ok(self
            .player
            .as_ref()
            .map(|player| player.position.subtract(before)))
    }

    /// Client arm of [`Game::side_push_piston_entities`]: predicts moving-piston
    /// pushes for the local player and client-driven boats, preserving collision,
    /// slime bounce, retracting-base correction, honey riding, and progressive
    /// piston collision advancement.
    pub(in crate::game) fn client_push_piston_entities(&mut self, pushes: Vec<Push>) {
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
            return;
        };
        let index = self.build_piston_index();
        let player_step = self
            .player
            .as_mut()
            .and_then(|p| p.attribute("minecraft:step_height").ok())
            .map_or(0.6, |v| v as f32);
        let mut targets = Vec::new();
        if let Some(p) = self.player.as_ref().filter(|p| !p.spectator)
            && p.bounds().intersects(reach)
        {
            targets.push(PushTarget {
                hint: MoveWho::Player,
                id: p.entity_id as u32,
                join: None,
            });
        }
        for (i, boat) in self.boats.iter().enumerate() {
            if self.controls_boat(boat.id)
                && boat.is_alive()
                && boat.get_piston_push_reaction() != PushReaction::IgnoreEntity
                && boat.bounds().intersects(reach)
            {
                targets.push(PushTarget {
                    hint: MoveWho::Entity(EntityHandle::Boat(i)),
                    id: boat.id,
                    join: Some(boat.join_seq()),
                });
            }
        }
        let mut accumulators = std::collections::HashMap::new();
        for push in pushes {
            let pos = push.0;
            self.client_push_for_piston(push, &targets, player_step, &mut accumulators);
            self.advance_piston_collision(pos, &index);
        }
    }

    fn client_push_for_piston(
        &mut self,
        (pos, piston, new_progress): Push,
        targets: &[PushTarget],
        player_step: f32,
        accumulators: &mut std::collections::HashMap<(u32, JoinSeq), PistonMovement>,
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
        for &selected in targets {
            let Some(target) = self.client_resolve_push_target(selected) else {
                continue;
            };
            let Some(body) = self.client_piston_body(target, Some(pos), player_step) else {
                continue;
            };
            if search.is_some_and(|area| !area.intersects(body.bounds)) {
                continue;
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
                self.client_bounce(target, movement);
            }
            if amount > 0.0 {
                let step = piston_push::step_vector(movement);
                self.client_move_piston_target(
                    selected,
                    target,
                    body,
                    step.scale(amount),
                    movement,
                    accumulators,
                );
                if !piston.extending
                    && piston.source
                    && let Some(target) = self.client_resolve_push_target(selected)
                    && let Some(after) = self.client_piston_body(target, Some(pos), player_step)
                    && let Some(fix) = piston_push::fix_within_piston_base(
                        pos,
                        after.bounds,
                        movement,
                        delta_progress,
                    )
                {
                    let back = piston_push::step_vector(movement.opposite());
                    self.client_move_piston_target(
                        selected,
                        target,
                        after,
                        back.scale(fix),
                        movement,
                        accumulators,
                    );
                }
            }
        }
        // Vanilla selects honey passengers after all collided entities have moved.
        if honey && movement.normal()[1] == 0 {
            let area = piston_push::sticky_area(pos, piston.shape_state(), sticky_top);
            for &selected in targets {
                let Some(target) = self.client_resolve_push_target(selected) else {
                    continue;
                };
                let Some(body) = self.client_piston_body(target, Some(pos), player_step) else {
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
                    self.client_move_piston_target(
                        selected,
                        target,
                        body,
                        piston_push::step_vector(movement).scale(delta_progress),
                        movement,
                        accumulators,
                    );
                }
            }
        }
    }

    fn client_resolve_push_target(&self, selected: PushTarget) -> Option<MoveWho> {
        match selected.hint {
            MoveWho::Player => self
                .player
                .as_ref()
                .filter(|p| p.entity_id as u32 == selected.id)
                .map(|_| MoveWho::Player),
            MoveWho::Entity(EntityHandle::Boat(_)) => self
                .client_resolve_boat(selected)
                .map(|i| MoveWho::Entity(EntityHandle::Boat(i))),
            _ => None,
        }
    }

    fn client_resolve_boat(&self, selected: PushTarget) -> Option<usize> {
        let matches = |b: &crate::game::boats::Boat| {
            !b.is_removed() && b.id == selected.id && Some(b.join_seq()) == selected.join
        };
        if let MoveWho::Entity(EntityHandle::Boat(i)) = selected.hint
            && self.boats.get(i).is_some_and(matches)
        {
            return Some(i);
        }
        self.boats.iter().position(matches)
    }

    fn client_piston_body(
        &self,
        target: MoveWho,
        pos: Option<BlockPos>,
        player_step: f32,
    ) -> Option<Body> {
        let body = match target {
            MoveWho::Player => {
                let p = self.player.as_ref()?;
                if p.spectator {
                    return None;
                }
                Body {
                    bounds: p.bounds(),
                    position: p.position,
                    on_ground: p.ground.on_ground,
                    step_height: player_step,
                    piston_reaction: PushReaction::PushPull,
                    supported: pos.is_some() && p.ground.supporting_block == pos,
                }
            }
            MoveWho::Entity(handle @ EntityHandle::Boat(i)) => {
                if !self.boats[i].is_alive() {
                    return None;
                }
                self.listed_body(handle, pos)
            }
            _ => return None,
        };
        (body.piston_reaction != PushReaction::IgnoreEntity).then_some(body)
    }

    fn client_bounce(&mut self, target: MoveWho, movement: Direction) {
        match target {
            MoveWho::Player => {
                if let Some(p) = self.player.as_mut() {
                    p.velocity = piston_push::bounce_velocity(movement, p.velocity);
                }
            }
            MoveWho::Entity(EntityHandle::Boat(i)) => {
                self.boats.update(i, |b| {
                    b.velocity = piston_push::bounce_velocity(movement, b.velocity);
                });
            }
            _ => {}
        }
    }

    fn client_move_piston_target(
        &mut self,
        selected: PushTarget,
        target: MoveWho,
        body: Body,
        movement: Vec3,
        piston_direction: Direction,
        accumulators: &mut std::collections::HashMap<(u32, JoinSeq), PistonMovement>,
    ) {
        if target == MoveWho::Player {
            with_piston_noclip(piston_direction, || {
                let _ = self.client_move_local_player(MoverType::Piston, movement, false);
            });
            return;
        }
        let MoveWho::Entity(EntityHandle::Boat(i)) = target else {
            return;
        };
        let mut gate = self.entity_move(target, MoverType::Piston, movement);
        if gate.skipped() {
            return;
        }
        let movement = gate.delta();
        let key = (self.boats[i].id, self.boats[i].join_seq());
        let time = self.world.game_time;
        let Some(limited) = accumulators
            .entry(key)
            .or_default()
            .limit(movement, || Ok::<i64, ()>(time))
            .ok()
        else {
            return;
        };
        if limited == Vec3::ZERO {
            return;
        }
        with_piston_noclip(piston_direction, || {
            let resolved = pushed_movement(
                &mut gate,
                SceneCollisions {
                    world: &self.world,
                    binding: self.collision,
                    subject: None,
                },
                self.border.geometry(),
                [body.position.x, body.position.z],
                body.bounds,
                limited,
                body.step_height,
                body.on_ground,
                None,
                &[],
            )
            .unwrap_or(Vec3::ZERO);
            let moved = resolved.length_squared() > 1.0e-7
                || limited.length_squared() - resolved.length_squared() < 1.0e-7;
            if moved && let Some(i) = self.client_resolve_boat(selected) {
                self.boats.update(i, |b| b.move_entity(&mut gate, resolved));
            }
            if let Some(i) = self.client_resolve_boat(selected) {
                self.client_finish_boat_ground_and_restitution(selected, i, limited, resolved);
            }
        });
    }

    fn client_finish_boat_ground_and_restitution(
        &mut self,
        selected: PushTarget,
        i: usize,
        wanted: Vec3,
        moved: Vec3,
    ) {
        let authoritative = self.controls_boat(self.boats[i].id);
        let motion = BodyMotion {
            world: &self.world,
            collision: self.collision,
            facts: self.res.facts(),
            blocks: Some(&self.res.motion().blocks),
            context: &self.loop_context,
            side: None,
        };
        self.boats.update(i, |boat| {
            let (at, bounds, id) = (boat.position, boat.bounds(), boat.id);
            let context = BodyMotion::entity_context(id, at);
            motion.record_move(
                BodyCollision {
                    footprint: (at, bounds),
                    movement: (wanted, moved),
                    authoritative,
                    context,
                    subject: None,
                },
                &mut boat.ground,
            );
        });
        let differs = sunrise_world::entity_move::different;
        if !authoritative
            || !(wanted.y != moved.y || differs(wanted.x, moved.x) || differs(wanted.z, moved.z))
        {
            return;
        }
        let side_server = self.game_side_client();
        let boat = &self.boats[i];
        let (id, feet, kind) = (boat.id, boat.position, boat.shared_type_name());
        let effect = self
            .body_motion(&game_side!(self, side_server))
            .legacy_contact(feet, &boat.ground);
        let response = self.body_motion(&game_side!(self, side_server)).restitute(
            (id, &kind),
            BodyMove {
                wanted,
                moved,
                effect,
            },
            boat.get_gravity(),
            1.0_f32,
            boat.velocity,
            boat.fall_distance,
        );
        let velocity = self.client_finish_body_restitution((id, &kind), feet, response);
        if let Some(i) = self.client_resolve_boat(selected) {
            self.boats.update(i, |b| b.velocity = velocity);
        }
    }

    /// Root vehicle of `id` in the client's riding graph (`remote.passenger_vehicles`
    /// plus `mob.extra.mount` fallback for offline client fixtures).
    fn client_vehicle_root(&self, mut id: u32) -> u32 {
        for _ in 0..16 {
            if let Some(remote) = self.remote.as_ref()
                && remote.passenger_vehicle(id).is_some()
            {
                match passenger_root(&remote.passenger_vehicles, id) {
                    Ok(root) if root != id => {
                        id = root;
                        continue;
                    }
                    Ok(root) | Err(root) => {
                        id = root;
                        break;
                    }
                }
            }
            if let Some(mount) = self
                .mobs
                .iter()
                .find(|m| m.id == id)
                .and_then(|m| m.extra.mount)
                && mount != id
            {
                id = mount;
                continue;
            }
            break;
        }
        id
    }

    fn client_lid_root(&self, lid: Lid) -> Option<u32> {
        let id = lid.shulker()?;
        Some(self.client_vehicle_root(id))
    }

    fn client_push_root(&self, target: MoveWho) -> Option<u32> {
        match target {
            MoveWho::Player => self.riding.map(|v| self.client_vehicle_root(v)),
            MoveWho::Entity(EntityHandle::Boat(i)) => {
                Some(self.client_vehicle_root(self.boats[i].id))
            }
            _ => None,
        }
    }

    /// A lid's mover is simulated on both sides: the client displaces its
    /// player and all boats the lid meets, not only the boat it drives.
    /// Restitution still belongs to the side simulating each boat.
    pub(in crate::game) fn lid_push_local(&mut self, lid: Lid, area: Aabb, movement: Vec3) {
        let root = self.client_lid_root(lid);
        let boats: Vec<_> = self
            .boats
            .iter()
            .enumerate()
            .filter(|(_, boat)| !boat.is_removed() && boat.bounds().intersects(area))
            .map(|(i, boat)| PushTarget {
                hint: MoveWho::Entity(EntityHandle::Boat(i)),
                id: boat.id,
                join: Some(boat.join_seq()),
            })
            .collect();
        if self
            .player
            .as_ref()
            .is_some_and(|p| !p.spectator && p.bounds().intersects(area))
            && !self.lid_spares_with_root(
                lid,
                root,
                MoveWho::Player,
                self.client_push_root(MoveWho::Player),
                false,
            )
        {
            let _ = self.client_move_local_player(lid.mover(), movement, false);
        }
        for selected in boats {
            let Some(i) = self.client_resolve_boat(selected) else {
                continue;
            };
            let handle = EntityHandle::Boat(i);
            let target = MoveWho::Entity(handle);
            let body = self.listed_body(handle, None);
            if lid == Lid::Box && body.piston_reaction == PushReaction::IgnoreEntity {
                continue;
            }
            let no_physics = self.pushed_no_physics(target);
            if !self.lid_spares_with_root(
                lid,
                root,
                target,
                self.client_push_root(target),
                no_physics,
            ) {
                self.client_move_lid_boat(lid.mover(), selected, i, body, movement, no_physics);
            }
        }
    }

    fn client_move_lid_boat(
        &mut self,
        mover: MoverType,
        selected: PushTarget,
        i: usize,
        body: Body,
        movement: Vec3,
        no_physics: bool,
    ) {
        let target = MoveWho::Entity(EntityHandle::Boat(i));
        let mut gate = self.entity_move(target, mover, movement);
        if gate.skipped() {
            return;
        }
        let movement = gate.delta();
        if no_physics {
            self.boats.update(i, |b| b.move_entity(&mut gate, movement));
            if let Some(i) = self.client_resolve_boat(selected) {
                self.record_boat_lid_movement(i, movement);
            }
            return;
        }
        let area = entity_collision_bounds(movement, body.bounds, body.step_height).inflate(
            Vec3::new(COLLISION_EPSILON, COLLISION_EPSILON, COLLISION_EPSILON),
        );
        let entities = self.entity_collisions(Mover::Boat {
            id: self.boats[i].id,
            area,
        });
        let Ok(moved) = pushed_movement(
            &mut gate,
            SceneCollisions {
                world: &self.world,
                binding: self.collision,
                subject: None,
            },
            self.border.geometry(),
            [body.position.x, body.position.z],
            body.bounds,
            movement,
            body.step_height,
            body.on_ground,
            None,
            &entities,
        ) else {
            return;
        };
        let placed = moved.length_squared() > 1.0e-7
            || movement.length_squared() - moved.length_squared() < 1.0e-7;
        if placed {
            let fall = self.vehicle_fall_reset(self.boats[i].fall_distance, body.position, moved);
            self.boats.update(i, |boat| boat.fall_distance = fall);
            self.boats
                .update(i, |boat| boat.move_entity(&mut gate, moved));
        }
        let Some(i) = self.client_resolve_boat(selected) else {
            return;
        };
        if placed {
            self.record_boat_lid_movement(i, moved);
        }
        // Record ground first so `boat_fall` sees the landed `on_ground` flag before restitution.
        let authoritative = self.controls_boat(self.boats[i].id);
        let motion = BodyMotion {
            world: &self.world,
            collision: self.collision,
            facts: self.res.facts(),
            blocks: Some(&self.res.motion().blocks),
            context: &self.loop_context,
            side: None,
        };
        self.boats.update(i, |boat| {
            let (at, bounds, id) = (boat.position, boat.bounds(), boat.id);
            let context = BodyMotion::entity_context(id, at);
            motion.record_move(
                BodyCollision {
                    footprint: (at, bounds),
                    movement: (movement, moved),
                    authoritative,
                    context,
                    subject: None,
                },
                &mut boat.ground,
            );
        });
        let boat = &self.boats[i];
        if authoritative
            && self
                .remote
                .as_ref()
                .is_none_or(|r| r.passenger_vehicle(boat.id).is_none())
        {
            let fall = self.boat_fall(boat, moved.y);
            self.boats.update(i, |boat| boat.fall_distance = fall);
        }
        let differs = sunrise_world::entity_move::different;
        if authoritative
            && (movement.y != moved.y
                || differs(movement.x, moved.x)
                || differs(movement.z, moved.z))
        {
            let side_server = self.game_side_client();
            let boat = &self.boats[i];
            let (id, feet, kind) = (boat.id, boat.position, boat.shared_type_name());
            let effect = self
                .body_motion(&game_side!(self, side_server))
                .legacy_contact(feet, &boat.ground);
            let response = self.body_motion(&game_side!(self, side_server)).restitute(
                (id, &kind),
                BodyMove {
                    wanted: movement,
                    moved,
                    effect,
                },
                boat.get_gravity(),
                1.0_f32,
                boat.velocity,
                boat.fall_distance,
            );
            let velocity = self.client_finish_body_restitution((id, &kind), feet, response);
            if let Some(i) = self.client_resolve_boat(selected) {
                self.boats.update(i, |b| b.velocity = velocity);
            }
        }
    }

    /// [`Self::lid_push_local`] for this tick's shulker box lids.
    pub(in crate::game) fn push_shulker_local(&mut self, pushes: Vec<ShulkerPush>) {
        for (_, facing, lid) in pushes {
            let (movement, area) = shulker_lid(facing, lid);
            self.lid_push_local(Lid::Box, area, movement);
        }
    }

    /// Notes, on a client, the collider carrying the player and how old the client's copy of it
    /// is, for the hint sent with the next position report.
    pub(in crate::game) fn observe_carry_age(&mut self, start: &Prepared) {
        let carrier = (!start.is_empty())
            .then(|| {
                let p = self.player.as_ref()?;
                if p.spectator || !p.ground.on_ground || self.riding.is_some() {
                    return None;
                }
                support_of(start, p.bounds(), p.entity_id as u32)
            })
            .flatten()
            .and_then(|id| Some((id, self.client_collider_now_frame(id)?)));
        if let Some(remote) = self.remote.as_mut() {
            remote.carry_age.observe(carrier);
        }
    }

    /// Client arm of [`Game::side_carry_pass`]: carries and shoves only the local
    /// player using client collider frames and the previous tick's shove snapshot.
    pub(in crate::game) fn client_carry_pass(&mut self, start: &Prepared) {
        let last = self
            .remote
            .as_mut()
            .and_then(|r| r.collider_shove_start.take())
            .filter(|(level, _)| *level == self.dim);
        let carry = start.placed().any(|p| {
            self.client_collider_now_frame(p.id)
                .is_some_and(|now| now != p.collider.frame)
        });
        let shove = last.as_ref().map_or(carry, |(_, placed)| {
            placed.placed().any(|p| {
                self.client_collider_now_frame(p.id)
                    .is_some_and(|now| now != p.collider.frame)
            })
        });
        if carry || shove {
            let mut carried_support = None;
            if carry
                && self.riding.is_none()
                && let Some((player_id, bounds)) = self
                    .player
                    .as_ref()
                    .filter(|p| !p.spectator && p.ground.on_ground)
                    .map(|p| (p.entity_id as u32, p.bounds()))
                && let Some(support) = support_of(start, bounds, player_id)
            {
                carried_support = Some(support);
                let now = self.client_placed_moving_colliders();
                {
                    let _scope =
                        moving_colliders::Scope::enter_level(self.client_collider_level(), now);
                    if let (Some(before), Some(after)) = (
                        start.placed().find(|p| p.id == support),
                        self.client_collider_now_frame(support),
                    ) && let Some(bounds) = self
                        .player
                        .as_ref()
                        .filter(|p| !p.spectator)
                        .map(|p| p.bounds())
                    {
                        let centre = bounds.center();
                        let contact = Vec3::new(centre.x, bounds.min(Axis::Y), centre.z);
                        let carried = after
                            .to_world(before.collider.frame.to_local(contact))
                            .subtract(contact);
                        if carried.length_squared() >= 1.0e-18 {
                            let _ =
                                self.client_move_local_player(MoverType::Carried, carried, false);
                            if let Some(player) = self.player.as_mut() {
                                player.ground.on_ground = true;
                            }
                        }
                    }
                }
                self.position_riders();
            }
            if shove {
                let before_shove = last.as_ref().map_or(start, |(_, placed)| placed);
                self.client_shove_pass(before_shove, carried_support);
            }
        }
        if self
            .remote
            .as_ref()
            .is_some_and(|r| !r.colliders.is_empty())
        {
            let now = self.client_placed_moving_colliders();
            if let Some(remote) = self.remote.as_mut() {
                remote.collider_shove_start = Some((self.dim, now));
            }
        }
    }

    fn client_shove_pass(&mut self, start: &Prepared, carried_support: Option<u32>) {
        let end = self.client_placed_moving_colliders();
        let starts: sunrise_level::fast_hash::FxHashMap<u32, &Placed> =
            start.placed().map(|p| (p.id, p)).collect();
        let mut sweeps: Vec<_> = end
            .placed()
            .filter_map(|after| {
                let before = &starts.get(&after.id)?.collider;
                let delta = after.collider.frame.origin.subtract(before.frame.origin);
                if delta == Vec3::ZERO || !before.frame.same_turn(&after.collider.frame) {
                    return None;
                }
                let swept = before.world_bounds()?.expand_towards(delta);
                Some((after, before, delta, swept))
            })
            .collect();
        if sweeps.is_empty() {
            return;
        }
        sweeps.sort_unstable_by_key(|(after, ..)| after.id);

        let mut current = end.clone();
        let mut shoved = false;
        for (after, before, delta, swept) in sweeps {
            if self.riding.is_some() {
                break;
            }
            let Some((player_id, bounds)) = self
                .player
                .as_ref()
                .filter(|p| !p.spectator)
                .map(|p| (p.entity_id as u32, p.bounds()))
            else {
                break;
            };
            let source = after.id;
            if after.exempt.contains(&player_id) || carried_support == Some(source) {
                continue;
            }
            if !swept.intersects(bounds) {
                continue;
            }
            let fraction = before.translation_push_fraction(&after.collider, bounds, delta);
            if fraction > 0.0 {
                let direction = delta.scale(1.0 / delta.length());
                let movement = delta
                    .scale(fraction)
                    .add_vector(direction.scale(piston_push::PUSH_OFFSET));
                {
                    let _scope = moving_colliders::Scope::enter_level(
                        self.client_collider_level(),
                        current.clone(),
                    );
                    let _shove = Shove::enter(player_id, source);
                    let _ = self.client_move_local_player(MoverType::Carried, movement, false);
                }
                shoved = true;
                if current.placed().any(|p| p.exempt.contains(&player_id)) {
                    self.position_riders();
                    current = self.client_placed_moving_colliders();
                }
            }
        }
        if shoved {
            self.position_riders();
        }
    }
}

#[cfg(test)]
mod client_lid_tests {
    use super::*;
    use crate::game::boats::Boat;
    use crate::game::native_entity_refs::tests::with_client_game;
    use std::sync::Arc;
    use sunrise_level::chunk::Chunk;
    use sunrise_sim::{mobs::Mob, motion::PlayerMotion};

    /// `isPassengerOfSameVehicle` on a client's own player: a shulker riding a
    /// boat spares the player riding the same boat; once the shulker rides
    /// nothing, the same lid lifts the player by the whole 0.25.
    #[test]
    #[ignore = "daily"]
    fn a_shulker_lid_spares_what_shares_its_vehicle_on_client() {
        with_client_game(|game| {
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            for x in 0..8 {
                for z in 0..8 {
                    game.world.set(x, 199, z, stone).unwrap();
                }
            }
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(2.5, 200.0, 2.5)).unwrap());
            let kind = game
                .res
                .mob_types()
                .get("minecraft:shulker")
                .unwrap()
                .clone();
            let mut shell = Mob::new(72, kind, Vec3::new(6.5, 200.0, 6.5), 0.0);
            shell.extra.mount = Some(71);
            game.boats
                .push(Boat::new(71, "oak".into(), Vec3::new(2.5, 200.0, 6.5), 0.0));
            game.mobs.push(shell);
            game.riding = Some(71);

            let around = Aabb::new(Vec3::new(1.0, 200.0, 1.0), Vec3::new(4.0, 201.0, 8.0));
            let lift = Vec3::new(0.0, 0.25, 0.0);
            let player_y =
                |game: &crate::game::ClientGame<'_>| game.player.as_ref().unwrap().position.y;
            game.lid_push_local(Lid::Shulker(72), around, lift);
            assert_eq!(
                game.boats[0].position.y, 200.0,
                "the shulker's vehicle stays"
            );
            assert_eq!(player_y(game), 200.0, "a client's own player stays too");

            game.mobs[0].extra.mount = None;
            game.lid_push_local(Lid::Shulker(72), around, lift);
            assert_eq!(player_y(game), 200.25, "nothing shared: the lid lifts it");
        });
    }
}
