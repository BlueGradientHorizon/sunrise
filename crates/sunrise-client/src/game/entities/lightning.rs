//! Lightning (`ServerLevel.tickThunder`, `LightningBolt`,
//! `LightningBoltRenderer`): strikes on the highest motion-blocking block or
//! a living entity under open sky nearby, skeleton trap horses, fire,
//! `thunderHit` (damage and burning, charged creepers, zombified piglins,
//! witches, mooshroom colours, turtles, burnt items), lightning rods, the sky
//! flash, the sounds and the branching additive bolt.
use super::Game;
use crate::game::lightning_common::Bolt;
use sunrise_core::random::RandomSource;

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    /// `ClientLevel.addEntity` for a server's lightning bolt: on a client it
    /// only flashes, sounds and draws (`LightningBolt.tick` off the server).
    pub(in crate::game) fn spawn_remote_bolt(&mut self, id: u32, at: [f64; 3]) {
        self.bolts.retain(|b| b.id != id);
        let (bolt, _) = Bolt::new(id, at, true);
        self.side.lightning_seen[0] += 1;
        self.bolts.push(bolt);
    }
    /// Client-side `LightningBolt.tick`: thunder/impact sounds and sky flash.
    pub(in crate::game) fn client_tick_bolts(&mut self) {
        let _save_lease = sunrise_sim::join_seq::visibility::PassLease::enter();
        let ids: Vec<u32> = self.bolts.iter().map(|b| b.id).collect();
        for id in ids {
            let Some(i) = self.bolts.iter().position(|b| b.id == id) else {
                continue;
            };
            let (at, life, silent) = {
                let b = &self.bolts[i];
                (b.at, b.life, b.silent)
            };
            if life == 2 && !silent {
                let (thunder_pitch, impact_pitch) = self.bolts.update(i, |b| {
                    (
                        0.8 + b.random.next_float() * 0.2,
                        0.5 + b.random.next_float() * 0.2,
                    )
                });
                self.side.audio.0.play(
                    "minecraft:entity.lightning_bolt.thunder",
                    Some(at),
                    10000.0,
                    thunder_pitch,
                );
                self.side.audio.0.play(
                    "minecraft:entity.lightning_bolt.impact",
                    Some(at),
                    2.0,
                    impact_pitch,
                );
            }
            let Some(i) = self.bolts.iter().position(|b| b.id == id) else {
                continue;
            };
            enum Step {
                Discard,
                Reflash,
                Active,
                Idle,
            }
            let step = self.bolts.update(i, |b| {
                b.life -= 1;
                if b.life < 0 {
                    if b.flashes == 0 {
                        return Step::Discard;
                    }
                    if b.life < -b.random.next_int_bound(10).unwrap_or(0) {
                        b.flashes -= 1;
                        b.life = 1;
                        b.seed = b.random.next_long();
                        return Step::Reflash;
                    }
                }
                if b.life >= 0 {
                    Step::Active
                } else {
                    Step::Idle
                }
            });
            if matches!(step, Step::Discard) {
                self.bolts.retain(|b| b.id != id);
                continue;
            }
            if matches!(step, Step::Reflash | Step::Active) {
                if self.flash == 0 {
                    self.side.lightning_seen[1] += 1;
                }
                self.flash = 2;
            }
        }
    }
    /// `LightningBolt.shouldRenderAtSqrDistance`: a bolt is drawn within 64
    /// blocks times the view scale of the camera, and past that only its sky
    /// flash and thunder remain. Simulation-order render inputs; seeded
    /// geometry lives in the renderer.
    pub(in crate::game) fn bolt_instances(
        &self,
        camera: glam::DVec3,
    ) -> Vec<sunrise_client::gfx::lightning::Bolt> {
        let range = 64.0 * self.view_scale();
        self.bolts
            .iter()
            .filter(|b| {
                !self.hides_as_camera(b.id) && {
                    let at = glam::DVec3::new(b.at[0], b.at[1], b.at[2]);
                    at.distance_squared(camera) < range * range
                }
            })
            .map(|b| sunrise_client::gfx::lightning::Bolt {
                at: b.at,
                seed: b.seed,
            })
            .collect()
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;
    use crate::game::server_side::lightning_server::{in_viewer_box, thunder_strike_column};
    use mod_api::{self, ServerHost};
    use std::sync::Arc;
    use sunrise_core::geometry::Vec3;
    use sunrise_core::random::LegacyRandom;
    use sunrise_sim::mobs::{Anim, Brain, Mob, MobType, Species};
    use sunrise_world::level::Ticks;

    /// `LightningBolt.spawnFire` places nothing, and draws no extra sources,
    /// unless `canSpreadFireAround` passes at the bolt (#11376).
    #[test]
    #[ignore = "daily"]
    fn a_bolt_away_from_players_starts_no_fire() {
        use sunrise_game_data::gamerules::Value;
        super::super::native_entity_refs::tests::with_game(|game| {
            game.world
                .chunks
                .insert((0, 0), Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)));
            game.block_ticks
                .add_empty_chunk(sunrise_core::position::ChunkPos::new(0, 0))
                .unwrap();
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            let air = game.res.facts().air;
            game.world.set(8, 64, 8, stone).unwrap();
            let bolt = [8.5, 65.0, 8.5];
            // (radius, player's x): fires expected at the strike.
            for (radius, player_x, fires) in [
                (128, 12.5, true),
                (1, 12.5, false),
                (0, 8.5, false),
                (-1, 1000.0, true),
            ] {
                game.world.set(8, 65, 8, air).unwrap();
                game.ui
                    .rules
                    .set("fire_spread_radius_around_player", Value::Int(radius));
                game.player = Some(
                    sunrise_sim::motion::PlayerMotion::new(
                        game.motion,
                        Vec3::new(player_x, 65.0, 8.5),
                    )
                    .unwrap(),
                );
                let random = game.world.random.clone();
                let mut bolt_random = LegacyRandom::new(0);
                let set = game.bolt_fire(bolt, 4, &mut bolt_random);
                let burning = game.world.block(8, 65, 8) != air;
                assert_eq!((set > 0, burning), (fires, fires), "radius {radius}");
                if !fires {
                    // Neither the bolt nor level RNG draws when fire is disallowed.
                    assert_eq!(bolt_random.next_long(), LegacyRandom::new(0).next_long());
                    let mut before = random;
                    assert_eq!(game.world.random.next_long(), before.next_long());
                }
            }
        });
    }

    /// `LightningRodBlock.onLightningStrike` updates the neighbours of the block
    /// in front of the rod whether or not redstone experiments are on (#14371):
    /// a wire beside that block is strongly powered by the struck rod.
    #[test]
    #[ignore = "daily"]
    fn a_struck_rod_updates_the_wire_beside_the_block_behind_it() {
        let mut resources = None;
        let mut game = crate::game::game_tests::tests::server_game(&mut resources);
        let registry = game.pack.assets.registry;
        let stone = registry.default_state("minecraft:stone").unwrap();
        let wire = registry.default_state("minecraft:redstone_wire").unwrap();
        let mut rod = registry
            .state(registry.default_state("minecraft:lightning_rod").unwrap())
            .cloned()
            .unwrap();
        rod.properties.insert("facing".into(), "up".into());
        let rod = registry.id(&rod).unwrap();
        // The rod on stone at (8, 64, 8); wire on stone beside that block.
        game.world.set(8, 64, 8, stone).unwrap();
        game.world.set(9, 63, 8, stone).unwrap();
        game.world.set(9, 64, 8, wire).unwrap();
        game.world.set(8, 65, 8, rod).unwrap();
        assert!(
            !game
                .level
                .enabled_features
                .iter()
                .any(|feature| feature == "minecraft:redstone_experiments")
        );
        game.strike_rod([8.5, 66.0, 8.5]);
        let power = registry
            .state(game.world.block(9, 64, 8))
            .unwrap()
            .properties
            .get("power")
            .cloned();
        assert_eq!(power.as_deref(), Some("15"));
    }

    #[test]
    fn viewer_box_reaches_15_around_and_down_and_21_up() {
        let bolt = [0.0, 64.0, 0.0];
        assert!(in_viewer_box(bolt, [15.0, 49.0, -15.0]));
        assert!(in_viewer_box(bolt, [0.0, 85.0, 0.0]));
        assert!(!in_viewer_box(bolt, [0.0, 85.5, 0.0]));
        assert!(!in_viewer_box(bolt, [0.0, 48.5, 0.0]));
        assert!(!in_viewer_box(bolt, [15.5, 64.0, 0.0]));
    }

    /// `ServerLevel.tickThunder` picks its strike column from `Level.randValue`
    /// (`Ticks::block_random_pos`), using bits `2..=5` for X and `10..=13` for Z
    /// rather than drawing from `Level.random`.
    #[test]
    fn thunder_strike_column_uses_block_random_pos_lcg_bits() {
        let mut ticks = Ticks::new(12345);
        let mut expected_lcg = 12345i32;
        for (cx, cz) in [(0, 0), (-3, 5), (12, -8)] {
            expected_lcg = expected_lcg.wrapping_mul(3).wrapping_add(1_013_904_223);
            let r = expected_lcg >> 2;
            let expected = (cx * 16 + (r & 15), cz * 16 + ((r >> 8) & 15));
            assert_eq!(thunder_strike_column(&mut ticks, cx, cz), expected);
        }
    }

    /// `Pig.thunderHit`: striking a pig with lightning converts it to a zombified
    /// piglin while preserving age (#7510).
    #[test]
    fn a_baby_pig_hit_by_lightning_becomes_a_baby_zombified_piglin() {
        let make_pig = |baby: bool| {
            let kind = Arc::new(MobType {
                type_key: Default::default(),
                id: "minecraft:pig".into(),
                species: Species::Pig,
                width: 0.9,
                height: 0.9,
                eye_height: 0.85,
                category: "creature".into(),
                max_health: 10.0,
                speed: 0.25,
                flying_speed: 0.02,
                attack_damage: 0.0,
                attack_knockback: 0.0,
                follow_range: 16.0,
                step_height: 0.6,
                armor: 0.0,
                armor_toughness: 0.0,
                knockback_resistance: 0.0,
                jump_strength: 0.42,
                attributes: Default::default(),
                item_stack_limits: Default::default(),
                attribute_bounds: Default::default(),
                gravity: 0.08,
                safe_fall_distance: 3.0,
                fall_damage_multiplier: 1.0,
                water_efficiency: 0.0,
                water_slowdown: 0.8,
                air_speed: 0.02,
                baby_scale: 0.5,
                head_rot_speed: 10.0,
                max_head_x_rot: 40.0,
                max_head_y_rot: 75.0,
                ambient_interval: 80,
                flags: Default::default(),
                brain: Brain::Passive,
                anim: Anim::Quadruped,
                sounds: ["", "", ""],
                sound_overrides: Default::default(),
                food: Default::default(),
                tempt: Default::default(),
                passenger_attachments: Vec::new(),
            });
            let mut pig = Mob::new(1, kind, Vec3::ZERO, 45.0);
            if baby {
                pig.extra.age = -24000;
            }
            pig
        };

        let adult_pig = make_pig(false);
        assert!(!adult_pig.is_baby());
        let baby_pig = make_pig(true);
        assert!(baby_pig.is_baby());
        let mut kind = (*adult_pig.kind).clone();
        kind.species = Species::ZombifiedPiglin;
        kind.id = "minecraft:zombified_piglin".into();
        let params = sunrise_sim::mobs::conversion::Params {
            keep_equipment: false,
            preserve_can_pick_up_loot: true,
        };
        for (pig, baby) in [(&adult_pig, false), (&baby_pig, true)] {
            let mut zombified = Mob::new(2, Arc::new(kind.clone()), Vec3::ZERO, 0.0);
            let mut pig = pig.clone();
            sunrise_sim::mobs::conversion::single(&pig, &mut zombified, params);
            sunrise_sim::mobs::conversion::pig_thunder_hit(&mut pig, &mut zombified);
            assert_eq!(zombified.is_baby(), baby);
            assert_eq!(zombified.yaw, 45.0);
            assert!(zombified.persistent);
        }
    }

    /// Spares the entity whose id it holds, and logs each (entity, bolt) asked.
    pub(in crate::game) struct Spare(
        pub(in crate::game) u32,
        pub(in crate::game) Arc<std::sync::Mutex<Vec<(u32, u32)>>>,
    );
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Spare {
        fn entity_struck_by_lightning(
            &self,
            _: &mut dyn ServerHost,
            e: &mut mod_api::EntityStruckByLightningEvent,
        ) -> mod_api::Flow {
            self.1.lock().unwrap().push((e.entity.id(), e.bolt.id()));
            if e.entity.id() == self.0 {
                mod_api::Flow::Cancel
            } else {
                mod_api::Flow::Continue
            }
        }
    }

    /// `EntityStruckByLightningEvent`: each struck pig is asked with the bolt;
    /// the spared one stays a pig, the other becomes a zombified piglin, and
    /// both still count as hit.
    #[test]
    #[ignore = "daily"]
    fn a_mod_spares_one_pig_from_the_bolt() {
        let heard = Arc::default();
        let (spared, struck) = (900, 901);
        let mods = vec![mod_api::ModEntry::new(
            "spare",
            Box::new(Spare(spared, Arc::clone(&heard))),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_game(|game| {
                game.options.dedicated = Some(Vec::new());
                game.ui.difficulty = 2;
                let pig = game.res.mob_types().get("minecraft:pig").unwrap().clone();
                for (id, x) in [(spared, 0.0), (struck, 1.0)] {
                    game.mobs
                        .push(Mob::new(id, pig.clone(), Vec3::new(x, 64.0, 0.0), 0.0));
                }
                let mut bolt = Bolt::struck_at(77, [0.5, 64.0, 0.5]);
                game.thunder_hit(&mut bolt);
                let kind = |game: &crate::game::ServerGame<'_>, id| {
                    game.mobs
                        .iter()
                        .find(|m| m.id == id && !m.removed)
                        .map(|m| m.species())
                };
                assert_eq!(kind(game, spared), Some(Species::Pig));
                assert_eq!(kind(game, struck), None, "converted");
                let spared_pig = game
                    .mobs
                    .iter()
                    .find(|m| m.id == spared && !m.removed)
                    .unwrap();
                assert!(spared_pig.fire_ticks <= 0, "no fire");
                let mut heard = heard.lock().unwrap().clone();
                heard.sort();
                assert_eq!(heard, [(spared, 77), (struck, 77)]);
                bolt.strike.hit.sort();
                assert_eq!(bolt.strike.hit, [spared, struck]);
            });
        });
    }
}

// A conversion keeps the old mob's state; lives here for the lightning's.
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "tests/mob_conversion_state_tests.rs"]
pub(in crate::game) mod mob_conversion_state_tests;
