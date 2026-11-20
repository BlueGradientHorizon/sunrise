//! The player's travel family: `Player.travel`'s branch selection and the
//! glide branch, split out of `motion.rs` (no behaviour change). The
//! `living_travel` hook (#2414, PR 2) fires at this family's branch selection.
use super::{MotionEvent, MotionWorld, Result, fall_flying_movement};
use sunrise_core::{geometry::Vec3, position::BlockPos};
use sunrise_world::{living_air, move_gate::MoveGate};

impl MotionWorld<'_, '_, '_> {
    pub fn travel_in_air(&mut self, gate: &mut MoveGate, input: Vec3) -> Result<()> {
        living_air::travel_in_air(self, gate, input)
    }
    /// Player.travel: swim steering, then LivingEntity.travel (fluids first,
    /// then gliding, then air), keeping 0.6 of the vertical speed while flying.
    pub(super) fn player_travel(&mut self, input: Vec3) -> Result<()> {
        // The player's one physical move this tick goes through the move gate
        // (#2414): whichever travel branch runs, `move_self` arms the gate it
        // receives here.
        let mut gate = MoveGate::new();
        if self.player.swimming {
            let look_y =
                sunrise_world::view_direction::view_vector(self.player.pitch, self.player.yaw).y;
            let near_surface = BlockPos::containing(Vec3::new(
                self.player.position.x,
                self.player.position.y + 1.0 - 0.1,
                self.player.position.z,
            ));
            if look_y <= 0.0
                || self.player.jumping
                || self.fluid_at(near_surface).kind() != sunrise_world::fluid::FluidKind::Empty
            {
                let blend = if look_y < -0.2 { 0.085 } else { 0.06 };
                self.player.velocity.y += (look_y - self.player.velocity.y) * blend;
            }
        }
        // The living_travel hook (#2414): one decision before the branch
        // selection, on the authoritative side, one branch with no listener.
        // A cancel skips the invocation - the branch, its nested move and
        // the flying damping below included; the swim steering above, part
        // of the tick's earlier processing, is not undone.
        let mut input = input;
        let mut velocity = self.player.velocity;
        // A replayed tick runs the resolved values: replay callbacks never
        // reach a listener.
        if !self.player.replaying
            && !crate::travel_gate::decide(
                self.loop_context,
                self.player.entity_id as u32,
                "minecraft:player",
                || self.travel_mode(),
                &mut input,
                &mut velocity,
            )
        {
            return Ok(());
        }
        self.player.velocity = velocity;
        // The flying damping keeps 0.6 of the vertical speed the branch starts
        // from: the accepted replacement's, when a listener replaced it.
        let flying_y = self.player.flying.then_some(self.player.velocity.y);
        self.living_travel(&mut gate, input)?;
        if let Some(y) = flying_y {
            self.player.velocity.y = y * 0.6;
        }
        Ok(())
    }
    /// `LivingEntity.travel`: in a fluid (`isAffectedByFluids`: not while
    /// flying), gliding, or in the air.
    pub(super) fn living_travel(&mut self, gate: &mut MoveGate, input: Vec3) -> Result<()> {
        if !self.player.flying
            && (self.player.fluids.was_touching_water || self.player.is_in_lava())
        {
            sunrise_world::living_fluid::travel_in_fluid(self, gate, input)?;
            Ok(())
        } else if self.player.fall_flying {
            // LivingEntity.travelFallFlying: on a climbable the glide ends and
            // the player moves like in the air.
            if self.climbable()? {
                self.travel_in_air(gate, input)?;
                self.player.fall_flying = false;
                Ok(())
            } else {
                self.travel_fall_flying(gate)
            }
        } else {
            self.travel_in_air(gate, input)
        }
    }
    /// The branch this travel is about to run, as the `living_travel` hook
    /// of #2414 hears it (its `mode`, built only when a listener asks): the
    /// same branch the selection below picks, fluid first, then the glide,
    /// with the flying damping riding the air branch.
    fn travel_mode(&self) -> sunrise_world::travel::TravelMode {
        use sunrise_world::travel::TravelMode;
        if !self.player.flying
            && (self.player.fluids.was_touching_water || self.player.is_in_lava())
        {
            // `travel_in_fluid` picks water first.
            if self.player.fluids.was_touching_water {
                TravelMode::Water
            } else {
                TravelMode::Lava
            }
        } else if self.player.fall_flying {
            TravelMode::Glide
        } else if self.player.flying {
            TravelMode::Flying
        } else {
            TravelMode::Air
        }
    }
    /// LivingEntity.travelFallFlying with updateFallFlyingMovement.
    pub(super) fn travel_fall_flying(&mut self, gate: &mut MoveGate) -> Result<()> {
        let v = self.player.velocity;
        let move_h = (v.x * v.x + v.z * v.z).sqrt();
        let gravity = sunrise_world::living_motion::effective_gravity(self)?;
        self.player.velocity = fall_flying_movement(v, self.player.pitch, self.player.yaw, gravity);
        let movement = self.player.velocity;
        self.move_self(gate, movement)?;
        // LivingEntity.handleFallFlyingCollisions: the fall-damage sound plays
        // on both sides (a glider's own travel is client-side, and the sound
        // must reach its own ears, #8906), only the server hurts, and it
        // compares the horizontal speed before `updateFallFlyingMovement`
        // with the delta `Entity.move` leaves (the collided axes zeroed).
        if self.player.collision.horizontal {
            let v = self.player.velocity;
            let after_h = (v.x * v.x + v.z * v.z).sqrt();
            let dmg = super::environment::fly_into_wall_damage(move_h, after_h);
            if dmg > 0.0 {
                let first = self.player.events.len();
                // getFallDamageSound((int) dmg)
                let sound = if dmg as i32 > 4 {
                    crate::landing::SceneSound::BigFall
                } else {
                    crate::landing::SceneSound::SmallFall
                };
                self.player
                    .events
                    .push(MotionEvent::Fall(crate::landing::FallEvent::Sound {
                        sound,
                        position: None,
                        volume: 1.0,
                        pitch: 1.0,
                    }));
                // Only the server hurts; the sound is shared.
                if !self.player.client_side {
                    let id = self
                        .resources
                        .health
                        .damage
                        .id("minecraft:fly_into_wall")
                        .ok_or("missing fly_into_wall damage type")?;
                    let rules = self.scene.health_rules;
                    self.player.hurt(
                        &mut crate::health::DamageLevel::of(
                            self.scene,
                            self.assets.registry,
                            self.tag_items,
                            self.block_registries,
                            self.damage_hooks,
                        ),
                        self.health,
                        rules,
                        id,
                        dmg,
                    )?;
                    if self.player.server_copy {
                        self.player.wall_hit_events = Some(first..self.player.events.len());
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::motion::{MotionWorld, PlayerMotion, dry_side};
    use crate::world::World;
    use mod_api::{Flow, Hooks, LivingTravelEvent, LoopContext};
    use sunrise_core::geometry::Vec3;

    /// A listener that replaces the velocity (when it names one) and answers `Flow`.
    struct Decide(Flow, Option<[f64; 3]>);

    #[mod_api::__subscriptions]
    impl Hooks for Decide {
        fn living_travel(&self, _: &dyn LoopContext, event: &mut LivingTravelEvent<'_>) -> Flow {
            if let Some(velocity) = self.1 {
                *event.velocity = velocity;
            }
            self.0
        }
    }

    /// One `player_travel` of a flying player standing at `(20.5, 4, 20.5)` with
    /// velocity `(0.2, 0.5, 0)`, under a `living_travel` listener: the
    /// position and velocity it leaves, and where it started.
    fn flying_travel(listener: Decide, replaying: bool) -> (Vec3, Vec3, Vec3) {
        let v = crate::test_fixtures::vanilla().expect("prepared resources are required");
        let (assets, render, binding, resources) = (&v.assets, &v.render, &v.collision, &v.motion);
        let mut scene = World::test_scene(assets, render, 1).unwrap();
        let start = Vec3::new(20.5, 4.0, 20.5);
        let mut player = PlayerMotion::new(resources, start).unwrap();
        player.flying = true;
        player.replaying = replaying;
        player.velocity = Vec3::new(0.2, 0.5, 0.0);
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new("travel", Box::new(listener))].into();
        mod_platform::testing::with_mods(mods, || {
            MotionWorld {
                side: dry_side(),
                tag_items: None,
                block_registries: None,
                loop_context: crate::test_resources::server_loop_context(),
                damage_hooks: &crate::health::DamageHooks::NONE,
                entities: &[],
                player: &mut player,
                scene: &mut scene,
                assets,
                collision_binding: binding,
                resources,
                health: &resources.health,
            }
            .player_travel(Vec3::ZERO)
            .unwrap();
        });
        (player.position, player.velocity, start)
    }

    /// A flying player keeps 0.6 of the vertical speed its branch started
    /// from: a listener's replaced `0.5` -> `1.0` leaves `0.6`, not `0.3`.
    #[test]
    #[ignore = "daily"]
    fn a_flying_player_damps_the_replaced_vertical_speed() {
        let (_, velocity, _) = flying_travel(Decide(Flow::Continue, Some([0.2, 1.0, 0.0])), false);
        assert_eq!(velocity.y, 0.6);
    }

    /// Cancel skips the branch, its move and the flying damping: the flying
    /// player keeps its position and its `0.5` vertical speed.
    #[test]
    #[ignore = "daily"]
    fn a_cancelled_flying_travel_moves_and_damps_nothing() {
        let (position, velocity, start) = flying_travel(Decide(Flow::Cancel, None), false);
        assert_eq!(position, start);
        assert_eq!(velocity, Vec3::new(0.2, 0.5, 0.0));
    }

    /// A replayed tick never reaches a listener: its cancel does not hold the
    /// travel, which moves the player along its velocity.
    #[test]
    #[ignore = "daily"]
    fn a_replayed_travel_ignores_a_cancel() {
        let (position, _, start) = flying_travel(Decide(Flow::Cancel, None), true);
        assert_ne!(position, start);
    }
}
