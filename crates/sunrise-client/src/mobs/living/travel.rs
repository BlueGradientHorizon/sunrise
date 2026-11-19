//! The mob's travel family: `LivingEntity.travel`'s branch selection and its
//! air, fluid, glide and flying branches, split out of `living.rs` (no
//! behaviour change). The `living_travel` hook (#2414, PR 2) fires at this
//! family's entry.
use super::{DamageLevel, Fluid, Mob, MobEvent, MobWorld, MoveGate, Vec3};

impl Mob {
    /// LivingEntity.travel.
    pub fn travel(
        &mut self,
        w: &MobWorld,
        level: &mut DamageLevel<'_>,
        mut input: Vec3,
        events: &mut Vec<MobEvent>,
    ) {
        // The living_travel hook (#2414): one decision at travel's entry,
        // on the authoritative side, one branch with no listener. A cancel
        // skips the invocation - this branch family and its nested move
        // included; nothing the tick already did is undone.
        let mut velocity = self.velocity;
        if !crate::travel_gate::decide(
            w.loop_context,
            self.id,
            &self.kind.id,
            || self.travel_mode(w),
            &mut input,
            &mut velocity,
        ) {
            return;
        }
        self.velocity = velocity;
        // Camel.travel: on the ground, a camel that sits or changes pose keeps
        // the vertical part of its motion and drops the horizontal input.
        if super::super::camel::is_camel(self.species())
            && self.on_ground
            && super::super::camel::refuse_to_move(self)
        {
            let vertical = Vec3::new(0.0, 1.0, 0.0);
            self.velocity = self.velocity.multiply(vertical);
            input = input.multiply(vertical);
        }
        if super::super::ai::custom_travel(self, w, level, input, events) {
            return;
        }
        if self.kind.flags.flying {
            return self.travel_flying(w, level, input, events);
        }
        // shouldTravelInFluid: not where the fluid at its feet holds it up.
        let (x, y, z) = (
            self.position.x.floor() as i32,
            self.position.y.floor() as i32,
            self.position.z.floor() as i32,
        );
        let stands_on_fluid = self.kind.flags.lava_walker && w.fluid(x, y, z) == Fluid::Lava;
        if self.is_in_liquid() && self.affected_by_fluids() && !stands_on_fluid {
            self.travel_in_fluid(w, level, input, events);
        } else if self.extra.fall_flying {
            self.travel_fall_flying(w, level, input, events);
        } else {
            self.travel_in_air(w, level, input, events);
        }
    }
    /// The branch this travel is about to run, as the `living_travel` hook
    /// of #2414 hears it (its `mode`, built only when a listener asks): the
    /// same branch the selection in [`travel`] picks, a flying kind first,
    /// then the fluid it moves through, then the glide.
    fn travel_mode(&self, w: &MobWorld) -> sunrise_world::travel::TravelMode {
        use sunrise_world::travel::TravelMode;
        if self.kind.flags.flying {
            return TravelMode::Flying;
        }
        // shouldTravelInFluid: not where the fluid at its feet holds it up.
        let (x, y, z) = (
            self.position.x.floor() as i32,
            self.position.y.floor() as i32,
            self.position.z.floor() as i32,
        );
        let stands_on_fluid = self.kind.flags.lava_walker && w.fluid(x, y, z) == Fluid::Lava;
        if self.is_in_liquid() && self.affected_by_fluids() && !stands_on_fluid {
            // `travel_in_fluid` picks water first.
            if self.fluids.in_water {
                TravelMode::Water
            } else {
                TravelMode::Lava
            }
        } else if self.extra.fall_flying {
            TravelMode::Glide
        } else {
            TravelMode::Air
        }
    }
    /// `LivingEntity.travelFallFlying`: use the mob's existing authoritative move
    /// and damage routes, with the shared `updateFallFlyingMovement` arithmetic.
    fn travel_fall_flying(
        &mut self,
        w: &MobWorld,
        level: &mut DamageLevel<'_>,
        input: Vec3,
        events: &mut Vec<MobEvent>,
    ) {
        if self.on_climbable(w) {
            self.travel_in_air(w, level, input, events);
            self.extra.fall_flying = false;
            return;
        }
        let before = (self.velocity.x * self.velocity.x + self.velocity.z * self.velocity.z).sqrt();
        self.velocity = crate::motion::fall_flying_movement(
            self.velocity,
            self.head_pitch,
            self.yaw,
            self.effective_gravity(),
        );
        self.do_move(w, level, events, &mut MoveGate::new());
        if self.horizontal_collision {
            let damage = ((before
                - (self.velocity.x * self.velocity.x + self.velocity.z * self.velocity.z).sqrt())
                * 10.0
                - 3.0) as f32;
            if damage > 0.0 {
                let monster = super::super::ai::is_monster(self.species());
                let sound = match (monster, damage as i32 > 4) {
                    (true, true) => "entity.hostile.big_fall",
                    (true, false) => "entity.hostile.small_fall",
                    (false, true) => "entity.generic.big_fall",
                    (false, false) => "entity.generic.small_fall",
                };
                if !self.silent {
                    events.push(MobEvent::Sound(
                        sound,
                        [self.position.x, self.position.y, self.position.z],
                        1.0,
                        1.0,
                    ));
                }
                self.damage(
                    level,
                    damage,
                    super::super::Damage::of(super::super::DamageKind::FlyIntoWall),
                );
            }
        }
    }
    /// LivingEntity.travelFlying: no gravity, `0.91F` drag in the air,
    /// `0.8F` in water and 0.5 in lava. A happy ghast swims as fast as it
    /// flies (`HappyGhast.travel`, at its `flying_speed` attribute); the others
    /// at 0.02.
    fn travel_flying(
        &mut self,
        w: &MobWorld,
        level: &mut DamageLevel<'_>,
        input: Vec3,
        events: &mut Vec<MobEvent>,
    ) {
        let (fluid_speed, air_speed) = if self.species() == super::super::Species::HappyGhast {
            let speed = super::super::happy_ghast::travel_speed(self);
            (speed, speed)
        } else {
            (0.02, self.kind.air_speed)
        };
        self.travel_flying_speeds(w, level, input, events, fluid_speed, fluid_speed, air_speed);
    }
    /// `LivingEntity.travelFlying(input, waterSpeed, lavaSpeed, airSpeed)`:
    /// the speeds vary per mob (an allay flies at the speed its move control
    /// set, `Allay.travel`).
    pub fn travel_flying_speeds(
        &mut self,
        w: &MobWorld,
        level: &mut DamageLevel<'_>,
        input: Vec3,
        events: &mut Vec<MobEvent>,
        water_speed: f32,
        lava_speed: f32,
        air_speed: f32,
    ) {
        let (speed, drag) = if self.fluids.in_water {
            (water_speed, f64::from(0.8_f32))
        } else if self.fluids.in_lava {
            (lava_speed, 0.5)
        } else {
            (air_speed, f64::from(0.91_f32))
        };
        self.move_relative(speed, input);
        self.do_move(w, level, events, &mut MoveGate::new());
        self.velocity = Vec3::new(
            self.velocity.x * drag,
            self.velocity.y * drag,
            self.velocity.z * drag,
        );
    }
    fn travel_in_air(
        &mut self,
        w: &MobWorld,
        level: &mut DamageLevel<'_>,
        input: Vec3,
        events: &mut Vec<MobEvent>,
    ) {
        let friction = if self.on_ground {
            // FRICTION_MODIFIER scales the block's slipperiness.
            let modifier = super::super::sulfur_cube::physics(self)
                .map_or(1.0, |a| a.friction_modifier.clamp(0.0, 2048.0));
            Self::modified_friction(self.block_friction(w), modifier)
        } else {
            1.0
        };
        let speed = self.friction_speed(w, friction);
        self.move_relative(speed, input);
        // LivingEntity.handleRelativeFrictionAndCalculateMovement:
        // handleOnClimbable, the move, then the climb up a wall.
        if self.on_climbable(w) {
            self.fall_distance = 0.0;
            self.velocity = Self::climbable_velocity(self.velocity);
        }
        self.do_move(w, level, events, &mut MoveGate::new());
        let mut movement = self.velocity;
        if (self.horizontal_collision || self.ai.jumping) && self.on_climbable(w) {
            movement.y = 0.2;
        }
        let mut y = movement.y;
        // LivingEntity.travelInAir: levitation lifts by its amplifier.
        if let Some(&super::super::StoredEffect { amplifier: amp, .. }) = self
            .extra
            .effects
            .iter()
            .find(|e| e.id == "minecraft:levitation")
        {
            y += (0.05 * f64::from(amp + 1) - movement.y) * 0.2;
        } else {
            y -= self.effective_gravity();
        }
        // LivingEntity.shouldDiscardFriction (a breeze's, a frog's or a goat's
        // long jump).
        if super::super::breeze::discards_friction(self)
            || self.extra.frog.discard_friction
            || self.extra.goat.discard_friction
        {
            self.velocity = Vec3::new(movement.x, y, movement.z);
            return;
        }
        let air_drag_modifier = self.air_drag_modifier();
        let drag = Self::modified_friction(0.91, air_drag_modifier);
        let f = friction * drag;
        let vertical = if self.kind.flags.omnidirectional {
            drag
        } else {
            Self::modified_friction(0.98, air_drag_modifier)
        };
        self.velocity = Vec3::new(
            movement.x * f64::from(f),
            y * f64::from(vertical),
            movement.z * f64::from(f),
        );
    }
    fn travel_in_fluid(
        &mut self,
        w: &MobWorld,
        level: &mut DamageLevel<'_>,
        input: Vec3,
        events: &mut Vec<MobEvent>,
    ) {
        let falling = self.velocity.y <= 0.0;
        let old_y = self.position.y;
        let gravity = self.effective_gravity();
        if self.fluids.in_water {
            let mut slow = if self.extra.sprinting {
                0.9
            } else {
                self.kind.water_slowdown
            };
            let mut speed = 0.02f32;
            let mut efficiency = self.kind.water_efficiency as f32;
            if !self.on_ground {
                efficiency *= 0.5;
            }
            if efficiency > 0.0 {
                slow += (0.546_000_06 - slow) * efficiency;
                speed += (self.speed(w) - speed) * efficiency;
            }
            self.move_relative(speed, input);
            self.do_move(w, level, events, &mut MoveGate::new());
            let mut m = self.velocity;
            // LivingEntity.travelInWater: `horizontalCollision && onClimbable()`,
            // a ladder or scaffolding under water as well as a climbing spider.
            if self.horizontal_collision && self.on_climbable(w) {
                m.y = 0.2;
            }
            m = Vec3::new(
                m.x * f64::from(slow),
                m.y * f64::from(0.8_f32),
                m.z * f64::from(slow),
            );
            self.velocity = super::fluid_falling(gravity, falling, self.extra.sprinting, m);
        } else {
            self.move_relative(0.02, input);
            self.do_move(w, level, events, &mut MoveGate::new());
            if self.fluids.lava <= self.fluid_jump_threshold() {
                self.velocity = Vec3::new(
                    self.velocity.x * 0.5,
                    self.velocity.y * f64::from(0.8_f32),
                    self.velocity.z * 0.5,
                );
                self.velocity =
                    super::fluid_falling(gravity, falling, self.extra.sprinting, self.velocity);
            } else {
                self.velocity = Vec3::new(
                    self.velocity.x * 0.5,
                    self.velocity.y * 0.5,
                    self.velocity.z * 0.5,
                );
            }
            if gravity != 0.0 {
                self.velocity.y -= gravity / 4.0;
            }
        }
        // jumpOutOfFluid: Entity.isFree wants the box dry and clear of the
        // entities a mob collides with (canBeCollidedWith: living shulkers).
        let m = self.velocity;
        let ahead = self.bounds().move_by(Vec3::new(
            m.x,
            m.y + f64::from(0.6_f32) - self.position.y + old_y,
            m.z,
        ));
        let wide = ahead.inflate(Vec3::new(1.0e-7, 1.0e-7, 1.0e-7));
        if self.horizontal_collision
            && w.no_collision(ahead)
            && !w.contains_any_liquid(ahead)
            && !w.others_near(wide).any(|s| {
                s.id != self.id
                    && s.alive
                    && s.species == super::super::Species::Shulker
                    && s.bounds.intersects(wide)
            })
        {
            self.velocity.y = f64::from(0.3_f32);
        }
        // `LivingEntity.travelInFluid` ends with
        // `floatInLiquidWhileRidden(FluidTags.ENTITY_FLOATABLE)`: a mount with
        // a passenger that is in `minecraft:can_float_while_ridden` and deeper
        // in the water than its jump threshold rises 0.04 a tick, which is
        // what keeps a ridden horse and its rider afloat.
        if (self.rider.is_some() || self.extra.rider.is_some())
            && w.data
                .entity_has("minecraft:can_float_while_ridden", &self.kind.id)
            && self.fluids.water > self.fluid_jump_threshold()
        {
            self.velocity.y += f64::from(0.04_f32);
        }
    }
}
