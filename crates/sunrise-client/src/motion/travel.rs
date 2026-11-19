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
        let flying_y = self.player.flying.then_some(self.player.velocity.y);
        // The living_travel hook (#2414): one decision before the branch
        // selection, on the authoritative side, one branch with no listener.
        // A cancel skips the invocation - the branch, its nested move and
        // the flying damping below included; the swim steering above, part
        // of the tick's earlier processing, is not undone.
        let mut input = input;
        let mut velocity = self.player.velocity;
        if !crate::travel_gate::decide(
            self.loop_context,
            self.player.entity_id as u32,
            "minecraft:player",
            || self.travel_mode(),
            &mut input,
            &mut velocity,
        ) {
            return Ok(());
        }
        self.player.velocity = velocity;
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
