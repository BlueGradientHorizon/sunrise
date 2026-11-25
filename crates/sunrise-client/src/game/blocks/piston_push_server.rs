#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game,
    entity::{Entity, EntityHandle, with_entity, with_entity_mut},
    minecarts::CartHit,
    move_gate::MoveWho,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::collision_source::Mover;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::piston_push::Body;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::piston_push::Lid;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::piston_push::PistonIndex;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::piston_push::Push;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::piston_push::PushTarget;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::piston_push::ShulkerPush;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::piston_push::StepHeights;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::piston_push::first_axis_direction;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::piston_push::pushed_movement;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::piston_push::shulker_lid;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::piston_push::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::collisions::{
    EntityCollisions, SceneCollisions, no_collision_empty, with_piston_noclip,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::entity_rules::EntityRules;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::join_seq::{JoinSeq, Joined};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::mobs::Species;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::motion::MotionWorld;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::{
    Aabb, Axis, COLLISION_EPSILON, Direction, ShapeOp, Vec3, border::BorderGeometry,
    entity_collision_bounds,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::position::BlockPos;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::{
    block_collision_scan::BlockCollisionWorld, collision_context::CollisionContext,
    entity_move::MoverType, level::data::PushReaction, level::pistons::MovingPiston,
    move_gate::MoveGate, piston_movement::PistonMovement, piston_push,
    world_movement::EntityMovementWorld,
};

impl Game<'_> {
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
                self.drives(mob)
            };
            if authoritative || wanted.y.abs() > 0.0 {
                let mut paused_hurt = None;
                let mut mobs = self.mobs.lift();
                let mut lent = sunrise_client::health::LentLevel::take(&mut self.world);
                // Restitution may call a block Part. Return both loans, including
                // their progressed state, before propagating a callback panic.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let world = self.mob_world_base(&[]);
                    let mut level = sunrise_client::health::DamageLevel::lent(
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
                    .restore(mobs, sunrise_client::join_seq::RestoreOrder::SurvivorsFirst);
                if let Err(payload) = result {
                    std::panic::resume_unwind(payload);
                }
                if let Some(hit) = paused_hurt
                    && let Some(index) = self.mob_index.find(&self.mobs, hit.victim)
                {
                    self.paused_mob_hurt.0 = Some(hit);
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
                        // Only a TNT landing needs its live cart loaned to the explosion.
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
                sunrise_client::motion::BodyMove { wanted, moved, effect },
                entity.get_gravity(), drag, entity.velocity(), entity.body_fall_distance())
        });
        let velocity = self.finish_body_restitution((id, kind), feet, response);
        let MoveWho::Entity(handle) = self.resolve_push_target(selected)? else {
            return None;
        };
        with_entity_mut!(self, handle, entity => *entity.velocity_mut() = velocity);
        Some(MoveWho::Entity(handle))
    }
    /// `Entity.move(mover, …)` for the player in place, with `LocalPlayer.move`'s
    /// auto-jump tail where the caller uses it (riptide). An acting guest's landing
    /// settles while the original turn and resolved collision are still in place.
    pub(in crate::game) fn move_local_player(
        &mut self,
        mover: MoverType,
        movement: Vec3,
        auto_jump: bool,
    ) -> sunrise_client::Result<Option<Vec3>> {
        use sunrise_client::landing::PlayerLandingPause;

        let Some(player) = self.player.as_ref() else {
            return Ok(None);
        };
        let (before, id, conn, dim) = (
            player.position,
            player.entity_id,
            self.server.lan_acting,
            self.dim,
        );
        // A guest landing can call Game in between these borrows. Rebind the
        // original body and its level, or abandon the rest of the old move.
        macro_rules! motion_phase {
            ($world:ident, $body:expr) => {{
                if self.server.lan_acting != conn || self.dim != dim {
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
    /// `Block.pushEntitiesUp(old, new, level, pos)` before the block at `pos`
    /// changes (`FarmlandBlock.turnToBaseBlock`): every body
    /// touching the collision that the new state adds over `old` is lifted
    /// onto it with `teleportRelative`, no collision scan. If the write refuses
    /// after section listeners have run, put the tentative positions back quietly.
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
        // Like the level query in `Block.pushEntitiesUp`, take the intersecting
        // bodies before any lift dispatches a section-change listener. A listener
        // can remove another entity, so a saved slot alone is not an identity.
        let mut lift: Vec<_> = self
            .push_targets()
            .filter_map(|target| {
                let (body, id, join) = match target {
                    MoveWho::Player => {
                        let p = self.player.as_ref().filter(|p| !p.spectator)?;
                        (p.bounds(), p.entity_id as u32, None)
                    }
                    MoveWho::Guest(conn) if Some(conn) != self.server.lan_acting => {
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
            // `teleportRelative` writes the position only. It does not run an
            // entity move or release projectiles embedded in the old block.
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
    /// `ShulkerBoxBlockEntity.moveCollidedEntities` for the bodies this game
    /// moves itself: every entity the sweeping lid box meets moves out along
    /// the box's facing by the box's own size plus 0.01, through [`Self::lid_push`].
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
            sunrise_client::collisions::PowderSnowSubject<'static>,
            sunrise_world::collision_context::CollisionContext<'static>,
        )>,
    ) {
        use sunrise_world::entity_move::different;
        let MoveWho::Entity(handle) = target else {
            return;
        };
        // The controlled-mob path owns the ground flag and supporting block.
        // Horizontal collision does not depend on movement authority.
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
        let motion = sunrise_client::motion::BodyMotion {
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
                        sunrise_client::motion::BodyMotion::entity_context(id, at)
                    };
                    (None, context)
                },
                |(subject, context)| (Some(subject), context),
            );
            if let Some(ground) = entity.ground_state_mut() {
                motion.record_move(
                    sunrise_client::motion::BodyCollision {
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
pub(in crate::game) mod h255_mob_piston_ground_tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::piston_push::h255_mob_piston_ground_tests::added_mob;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::piston_push::h255_mob_piston_ground_tests::added_push;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::piston_push::h255_mob_piston_ground_tests::mob_index;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::piston_push::h255_mob_piston_ground_tests::with_game;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::piston_push::h255_mob_piston_ground_tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::{
        Audio, Options, Progress, Settings, load_resources,
    };
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use std::sync::Arc;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_client::{chunk::Chunk, motion::PlayerMotion};
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_core::{
        geometry::Vec3, position::ChunkPos, random::RandomSource,
    };

    pub(in crate::game) fn with_role_game(
        role: crate::game::execution_role::ExecutionRole,
        test: impl FnOnce(&mut Game<'_>),
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
            let mut game = Game::new(
                &loaded.resources,
                loaded.pack,
                loaded.gui.unwrap(),
                options,
                settings,
                Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                    root.clone(),
                )),
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
            assert!(game.tickets.0.levels().block_ticking(0, 0));
            test(&mut game);
        });
    }
}
