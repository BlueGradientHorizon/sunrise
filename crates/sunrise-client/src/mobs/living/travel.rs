//! The mob's travel family: `LivingEntity.travel`'s branch selection and its
//! air, fluid, glide and flying branches, split out of `living.rs` (no
//! behaviour change). The `living_travel` hook (#2414, PR 2) fires at this
//! family's entry.
use super::super::{Species, allay, aquatic, frog, guardian, nautilus, turtle, undead, vex};
use super::{DamageLevel, Fluid, Mob, MobEvent, MobWorld, MoveGate, Vec3};
use sunrise_world::travel::TravelMode;

enum TravelRoute {
    Air,
    AirEndingGlide,
    Fluid { water: bool },
    Glide,
    Flying,
    Allay,
    Vex,
    Squid,
    Fish,
    Drowned,
    Frog,
    Guardian,
    Nautilus,
    Turtle,
}

impl TravelRoute {
    fn mode(&self) -> TravelMode {
        match self {
            Self::Air | Self::AirEndingGlide => TravelMode::Air,
            Self::Fluid { water: true } => TravelMode::Water,
            Self::Fluid { water: false } => TravelMode::Lava,
            Self::Glide => TravelMode::Glide,
            Self::Flying | Self::Allay | Self::Vex => TravelMode::Flying,
            Self::Squid => TravelMode::VelocityOnly,
            Self::Fish
            | Self::Drowned
            | Self::Frog
            | Self::Guardian
            | Self::Nautilus
            | Self::Turtle => TravelMode::Water,
        }
    }
}

impl Mob {
    /// LivingEntity.travel.
    pub fn travel(
        &mut self,
        w: &MobWorld,
        level: &mut DamageLevel<'_>,
        mut input: Vec3,
        events: &mut Vec<MobEvent>,
    ) {
        let Some(route) = self.travel_route(w) else {
            return;
        };
        let mut velocity = self.velocity;
        if !crate::travel_gate::decide(
            w.loop_context,
            self.id,
            &self.kind.id,
            || route.mode(),
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
        match route {
            TravelRoute::Air => self.travel_in_air(w, level, input, events),
            TravelRoute::AirEndingGlide => {
                self.travel_in_air(w, level, input, events);
                self.extra.fall_flying = false;
            }
            TravelRoute::Fluid { water } => self.travel_in_fluid(w, level, input, events, water),
            TravelRoute::Glide => self.travel_fall_flying(w, level, events),
            TravelRoute::Flying => self.travel_flying(w, level, input, events),
            TravelRoute::Allay => allay::travel(self, w, level, input, events),
            TravelRoute::Vex => vex::travel(self, w),
            // Squid.travel moves by its current velocity alone, even out of water.
            TravelRoute::Squid => self.do_move(w, level, events, &mut MoveGate::new()),
            TravelRoute::Fish => aquatic::travel_in_water(self, w, level, input, events),
            TravelRoute::Drowned => undead::drowned_travel(self, w, level, input, events),
            TravelRoute::Frog => frog::travel(self, w, level, input, events),
            TravelRoute::Guardian => guardian::travel(self, w, level, input, events),
            TravelRoute::Nautilus => nautilus::travel(self, w, level, input, events),
            TravelRoute::Turtle => turtle::travel(self, w, level, input, events),
        }
    }

    // Decide once, before mod input or velocity can change. Species overrides
    // take precedence over the generic branches; Shulkers do not travel.
    fn travel_route(&self, w: &MobWorld) -> Option<TravelRoute> {
        use Species as S;
        let water = self.fluids.in_water;
        Some(match self.species() {
            S::Shulker => return None,
            S::Allay => TravelRoute::Allay,
            S::Vex => TravelRoute::Vex,
            S::Squid | S::GlowSquid => TravelRoute::Squid,
            S::Axolotl
            | S::Cod
            | S::Salmon
            | S::TropicalFish
            | S::Pufferfish
            | S::Tadpole
            | S::Dolphin
                if water =>
            {
                TravelRoute::Fish
            }
            S::Drowned
                if water && self.was_eye_in_water && undead::drowned_wants_to_swim(self, w) =>
            {
                TravelRoute::Drowned
            }
            S::Frog if water => TravelRoute::Frog,
            S::Guardian | S::ElderGuardian if water => TravelRoute::Guardian,
            S::Nautilus | S::ZombieNautilus if water => TravelRoute::Nautilus,
            S::Turtle if water => TravelRoute::Turtle,
            _ if self.kind.flags.flying => TravelRoute::Flying,
            _ => {
                // shouldTravelInFluid: not where the fluid at its feet holds it up.
                let (x, y, z) = (
                    self.position.x.floor() as i32,
                    self.position.y.floor() as i32,
                    self.position.z.floor() as i32,
                );
                let stands_on_fluid =
                    self.kind.flags.lava_walker && w.fluid(x, y, z) == Fluid::Lava;
                if self.is_in_liquid() && self.affected_by_fluids() && !stands_on_fluid {
                    TravelRoute::Fluid { water }
                } else if self.extra.fall_flying {
                    if self.on_climbable(w) {
                        TravelRoute::AirEndingGlide
                    } else {
                        TravelRoute::Glide
                    }
                } else {
                    TravelRoute::Air
                }
            }
        })
    }

    /// `LivingEntity.travelFallFlying`: use the mob's existing authoritative move
    /// and damage routes, with the shared `updateFallFlyingMovement` arithmetic.
    fn travel_fall_flying(
        &mut self,
        w: &MobWorld,
        level: &mut DamageLevel<'_>,
        events: &mut Vec<MobEvent>,
    ) {
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
        water: bool,
    ) {
        let falling = self.velocity.y <= 0.0;
        let old_y = self.position.y;
        let gravity = self.effective_gravity();
        if water {
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

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Flow, Hooks, LivingTravelEvent, LoopContext, TravelMode as Mode};
    use std::sync::{Arc, Mutex};

    #[test]
    #[ignore = "daily"]
    fn custom_mob_travel_reports_the_route_it_runs() {
        struct Observe(Arc<Mutex<Vec<(u32, Mode)>>>);
        #[mod_api::__subscriptions]
        impl Hooks for Observe {
            fn living_travel(
                &self,
                _: &dyn LoopContext,
                event: &mut LivingTravelEvent<'_>,
            ) -> Flow {
                self.0.lock().unwrap().push((event.entity, event.mode));
                Flow::Continue
            }
        }

        let heard = Arc::new(Mutex::new(Vec::new()));
        let mods = vec![mod_api::ModEntry::new(
            "observe",
            Box::new(Observe(Arc::clone(&heard))),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            assert!(
                crate::mobs::world::with_test_world(&[], |w, types| {
                    let at = Vec3::new(0.5, 80.0, 0.5);
                    let mut test_level = crate::health::TestLevel::new(0);
                    for (id, name, dx, vx) in [
                        (1, "minecraft:allay", 1.0, f64::from(0.91_f32)),
                        (2, "minecraft:vex", 1.0, f64::from(0.91_f32)),
                        (3, "minecraft:squid", 1.0, 1.0),
                        (4, "minecraft:shulker", 0.0, 1.0),
                    ] {
                        let mut mob = Mob::new(id, Arc::new(types.types[name].clone()), at, 0.0);
                        mob.velocity = Vec3::new(1.0, 0.0, 0.0);
                        mob.travel(w, &mut test_level.level(), Vec3::ZERO, &mut Vec::new());
                        assert_eq!(mob.position.x, at.x + dx, "{name} displacement");
                        assert_eq!(mob.velocity.x, vx, "{name} drag");
                        if name == "minecraft:allay" {
                            assert_eq!(mob.velocity.y, 0.0, "allay flight does not apply gravity");
                        }
                    }
                }),
                "prepared resources are required"
            );
        });
        assert_eq!(
            *heard.lock().unwrap(),
            vec![
                (1, Mode::Flying),
                (2, Mode::Flying),
                (3, Mode::VelocityOnly)
            ]
        );
    }
}
