//! Lightning (`ServerLevel.tickThunder`, `LightningBolt`,
//! `LightningBoltRenderer`): strikes on the highest motion-blocking block or
//! a living entity under open sky nearby, skeleton trap horses, fire,
//! `thunderHit` (damage and burning, charged creepers, zombified piglins,
//! witches, mooshroom colours, turtles, burnt items), lightning rods, the sky
//! flash, the sounds and the branching additive bolt.
use super::Game;
use sunrise_client::random_source;

use sunrise_core::{
    geometry::Vec3,
    random::{LegacyRandom, RandomSource},
};
use sunrise_world::level::Ticks;

#[derive(Default)]
pub(in crate::game) struct StrikeState {
    /// Mooshrooms and copper golems change once per bolt.
    pub(in crate::game) converted: Vec<u32>,
    /// The mobs it has struck (`hitEntities`).
    pub(in crate::game) hit: Vec<u32>,
    /// The dropped items in the strike box (`hitEntities` holds every entity).
    pub(in crate::game) hit_items: Vec<u32>,
    /// Items a cushion dropped under it, spared for 20 ticks
    /// (`Cushion.dropItem`: `setInvulnerableTime(20)`).
    pub(in crate::game) spared: Vec<u32>,
}

pub(in crate::game) struct Bolt {
    /// Entity-owned registration metadata; content clones are detached.
    pub(in crate::game) attachment_lifetime: sunrise_mod_runtime::attachments::AttachmentLifetime,
    pub(in crate::game) join: sunrise_client::join_seq::JoinSeq,
    /// The entity id a server's players know it by (0 until it has one).
    pub id: u32,
    pub at: [f64; 3],
    pub velocity: Vec3,
    pub rotation: [f32; 2],
    pub fall_distance: f64,
    pub fire_ticks: i32,
    pub air: i32,
    pub on_ground: bool,
    pub invulnerable: bool,
    pub invulnerable_time: i32,
    pub portal_cooldown: i32,
    pub custom_name: Option<sunrise_core::nbt::Tag>,
    pub custom_name_visible: bool,
    pub silent: bool,
    pub no_gravity: bool,
    pub glowing: bool,
    pub ticks_frozen: i32,
    pub has_visual_fire: bool,
    /// Vehicle entity id when this bolt rides a serializable entity (`Entity.vehicle`).
    pub mount: Option<u32>,
    pub vehicle_order: i64,
    pub(in crate::game) life: i32,
    pub(in crate::game) flashes: i32,
    pub(in crate::game) seed: i64,
    /// The bolt's own random (`Entity.random`): UUID, then seed, flashes, flash
    /// timing and fire spots draw from it, never the level's.
    pub(in crate::game) random: LegacyRandom,
    pub(in crate::game) visual_only: bool,
    pub(in crate::game) strike: StrikeState,
    /// Blocks it set on fire (`blocksSetOnFire`).
    pub(in crate::game) fires: i32,
    /// The player who channelled it (`LightningBolt.cause`): `Some(None)` for
    /// the host, `Some(Some(conn))` for a guest.
    pub cause: Option<Option<u32>>,
}

sunrise_client::impl_joined!(Bolt);
sunrise_mod_runtime::impl_attachment_lifetime!(Bolt);

impl Bolt {
    /// `LightningBolt.<init>`: `Entity.<init>` draws the UUID first, then `seed` and `flashes`.
    pub(in crate::game) fn new(id: u32, at: [f64; 3], visual_only: bool) -> (Self, [i32; 4]) {
        let mut random = random_source::entity();
        let uuid = sunrise_client::save::insecure_uuid(&mut random);
        let seed = random.next_long();
        let flashes = random.next_int_bound(3).unwrap_or(0) + 1;
        (
            Self {
                attachment_lifetime: Default::default(),
                join: Default::default(),
                id,
                at,
                velocity: Vec3::ZERO,
                rotation: [0.0; 2],
                fall_distance: 0.0,
                fire_ticks: 0,
                air: 300,
                on_ground: false,
                invulnerable: false,
                invulnerable_time: 0,
                portal_cooldown: 0,
                custom_name: None,
                custom_name_visible: false,
                silent: false,
                no_gravity: false,
                glowing: false,
                ticks_frozen: 0,
                has_visual_fire: false,
                mount: None,
                vehicle_order: 0,
                life: 2,
                flashes,
                seed,
                random,
                visual_only,
                strike: StrikeState::default(),
                fires: 0,
                cause: None,
            },
            uuid,
        )
    }

    /// `Entity.load` inherited fields (`LightningBolt.readAdditionalSaveData` is empty).
    pub(in crate::game) fn apply_nbt(&mut self, tag: &sunrise_core::nbt::Tag) {
        use sunrise_client::{entity_nbt, nbt_path::int_value, save::float};
        if let Some(pos) = entity_nbt::position(tag) {
            self.at = [
                pos.x.clamp(-3.000_051_2e7, 3.000_051_2e7),
                pos.y.clamp(-2.0e7, 2.0e7),
                pos.z.clamp(-3.000_051_2e7, 3.000_051_2e7),
            ];
        }
        self.velocity = entity_nbt::loaded_motion(tag);
        self.rotation = entity_nbt::loaded_rotation(tag);
        self.fall_distance = float(tag.get("fall_distance")).unwrap_or(0.0);
        self.fire_ticks = i32::from(
            tag.get("Fire")
                .and_then(int_value)
                .map_or(0_i16, |v| v as i16),
        );
        self.air = tag.get("Air").and_then(int_value).unwrap_or(300);
        self.on_ground = entity_nbt::flag(tag, "OnGround");
        self.invulnerable = entity_nbt::flag(tag, "Invulnerable");
        self.invulnerable_time = tag
            .get("invulnerable_time")
            .and_then(int_value)
            .unwrap_or(0);
        self.portal_cooldown = tag.get("PortalCooldown").and_then(int_value).unwrap_or(0);
        self.custom_name = tag
            .get("CustomName")
            .and_then(entity_nbt::validated_component);
        self.custom_name_visible = entity_nbt::flag(tag, "CustomNameVisible");
        self.silent = entity_nbt::flag(tag, "Silent");
        self.no_gravity = entity_nbt::flag(tag, "NoGravity");
        self.glowing = entity_nbt::flag(tag, "Glowing");
        self.ticks_frozen = tag.get("TicksFrozen").and_then(int_value).unwrap_or(0);
        self.has_visual_fire = entity_nbt::flag(tag, "HasVisualFire");
    }
}
/// The box `LightningBolt.tick` collects the `LightningStrikeTrigger` bystanders from: 15 blocks
/// around the bolt, 15 down and `6 + 15` up.
pub(in crate::game) fn in_viewer_box(bolt: [f64; 3], p: [f64; 3]) -> bool {
    (p[0] - bolt[0]).abs() <= 15.0
        && (p[2] - bolt[2]).abs() <= 15.0
        && p[1] >= bolt[1] - 15.0
        && p[1] <= bolt[1] + 21.0
}

/// `ServerLevel.tickThunder` -> `Level.getBlockRandomPos(minX, 0, minZ, 15)`:
/// picks the strike column from the level's separate `randValue` LCG.
pub(in crate::game) fn thunder_strike_column(ticks: &mut Ticks, cx: i32, cz: i32) -> (i32, i32) {
    let r = ticks.block_random_pos(cx * 16, 0, cz * 16, 15);
    (r.x, r.z)
}

#[cfg(test)]
impl Bolt {
    /// A spent bolt at `at`, for tests that call `thunder_hit` directly.
    pub(in crate::game) fn struck_at(id: u32, at: [f64; 3]) -> Self {
        Self {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id,
            at,
            velocity: Vec3::ZERO,
            rotation: [0.0; 2],
            fall_distance: 0.0,
            fire_ticks: 0,
            air: 300,
            on_ground: false,
            invulnerable: false,
            invulnerable_time: 0,
            portal_cooldown: 0,
            custom_name: None,
            custom_name_visible: false,
            silent: false,
            no_gravity: false,
            glowing: false,
            ticks_frozen: 0,
            has_visual_fire: false,
            mount: None,
            vehicle_order: 0,
            life: 0,
            flashes: 0,
            seed: 0,
            random: LegacyRandom::new(0),
            visual_only: false,
            strike: StrikeState::default(),
            fires: 0,
            cause: None,
        }
    }
}

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
        let _save_lease = sunrise_client::join_seq::visibility::PassLease::enter();
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
    use mod_api::{self, ServerHost};
    use std::sync::Arc;
    use sunrise_client::mobs::{Anim, Brain, Mob, MobType, Species};
    use sunrise_core::geometry::Vec3;

    /// `LightningBolt.spawnFire` places nothing, and draws no extra sources,
    /// unless `canSpreadFireAround` passes at the bolt (#11376).
    #[test]
    #[ignore = "daily"]
    fn a_bolt_away_from_players_starts_no_fire() {
        use sunrise_client::gamerules::Value;
        super::super::native_entity_refs::tests::with_game(|game| {
            game.world
                .chunks
                .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
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
                    sunrise_client::motion::PlayerMotion::new(
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
        let params = sunrise_client::mobs::conversion::Params {
            keep_equipment: false,
            preserve_can_pick_up_loot: true,
        };
        for (pig, baby) in [(&adult_pig, false), (&baby_pig, true)] {
            let mut zombified = Mob::new(2, Arc::new(kind.clone()), Vec3::ZERO, 0.0);
            let mut pig = pig.clone();
            sunrise_client::mobs::conversion::single(&pig, &mut zombified, params);
            sunrise_client::mobs::conversion::pig_thunder_hit(&mut pig, &mut zombified);
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
