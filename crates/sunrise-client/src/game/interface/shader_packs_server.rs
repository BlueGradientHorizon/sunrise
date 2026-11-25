#[allow(unused_imports)]
pub(in crate::game::server_side) use super::native_shaders::{
    fingerprint, outcome, refused, remember,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::remote_camera::CameraFogEffects;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Dim, Game, Screen};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::BlockIdVotes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::PackIds;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::PickedPack;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::TogglePlan;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::picks_material_pack;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::cell::RefCell;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::collections::{BTreeMap, HashMap};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::camera::CameraFluid;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::gfx::pack_uniforms::{
    PackInputs, PlayerStatus, ViewEntity,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::mod_list::ModList;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::shader_options::ShaderOptions;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::text::Text;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client_render::render_geometry::EntitySpan;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_shader_api::Api;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_shaders::{
    BlockIds, Macros, Pack, Properties, dimension_folder_named, load_post,
};

#[cfg(test)]
pub(in crate::game) mod player_status_tests {
    #[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
        chunk::Chunk,
        effects::EffectInstance,
        motion::{MotionWorld, PlayerMotion},
    };
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::player_status_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::player_status_tests::stand_player;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::player_status_tests::rain_on_the_player;

    /// The player booleans only custom uniforms read follow the window's player (spec 3.4):
    /// `LivingEntity.isAlive`, `Entity.isCurrentlyGlowing` (the Glowing effect),
    /// `Entity.isInLava`, `Entity.isVehicle` and `Entity.isInWaterOrRain`.
    #[test]
    #[ignore = "daily"]
    fn pack_status_follows_the_player() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            stand_player(game);
            crate::game::native_entity_refs::tests::bind_local_player(game, 20, 123);
            game.install_live_features();

            let status = game.pack_player_status();
            assert!(status.alive);
            assert!(!status.glowing && !status.in_lava && !status.ridden && !status.wet);

            // Rain over the player's open plains column, and none once it stops.
            rain_on_the_player(game, true);
            assert!(game.pack_player_status().wet, "rain");
            rain_on_the_player(game, false);
            assert!(!game.pack_player_status().wet);
            game.player.as_mut().unwrap().fluids.was_touching_water = true;
            assert!(game.pack_player_status().wet, "water");
            game.player.as_mut().unwrap().fluids.was_touching_water = false;

            game.potions
                .effects
                .add(EffectInstance::new("glowing", 200, 0));
            assert!(game.pack_player_status().glowing);

            let pig = game.res.mob_types().get("minecraft:pig").unwrap().clone();
            let mut rider = sunrise_client::mobs::Mob::new(42, pig, Vec3::new(0.5, 65.8, 0.5), 0.0);
            rider.extra.mount = Some(20);
            game.mobs.push(rider);
            assert!(game.pack_player_status().ridden);
            // `LivingEntity.die` doesn't dismount: a dying rider rides until it is removed.
            game.mobs[0].health = 0.0;
            assert!(
                game.pack_player_status().ridden,
                "a dying rider still rides"
            );
            game.mobs[0].removed = true;
            assert!(!game.pack_player_status().ridden, "a removed rider is gone");

            let lava = game
                .pack
                .assets
                .registry
                .default_state("minecraft:lava")
                .unwrap();
            assert!(game.world.set_stored(0, 64, 0, lava));
            let side = game_side!(game);
            let mut world = MotionWorld {
                side: &side,
                entities: &[],
                player: game.player.as_mut().unwrap(),
                scene: &mut game.world,
                assets: &game.pack.assets,
                collision_binding: game.collision,
                resources: game.motion,
                health: &game.health_resources,
                tag_items: None,
                loop_context: &game.loop_context,
                damage_hooks: &game.damage_hooks,
                block_registries: None,
            };
            world.fluid_base_tick().unwrap();
            assert!(game.pack_player_status().in_lava);

            game.player
                .as_mut()
                .unwrap()
                .set_health(0.0, false)
                .unwrap();
            assert!(!game.pack_player_status().alive);
        });
    }

    /// A frame's player booleans only read the level: on a LAN host in the rain, the particles,
    /// sounds and spill queued for the next level access stay queued.
    #[test]
    #[ignore = "daily"]
    fn pack_status_leaves_queued_effects_alone() {
        use sunrise_client::particles::ParticleType;
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            stand_player(game);
            crate::game::native_entity_refs::tests::bind_local_player(game, 20, 123);
            game.install_live_features();
            let (hub, _near, _far) =
                crate::game::lan_particles::tests::test_hub("PackStatusTest", Vec::new(), None);
            game.lan = Some(crate::game::lan::LanHost::for_test(hub));
            let at = [0.5, 64.0, 0.5];
            game.server
                .pending_particles
                .push(crate::lan::server_effects::ServerQueuedParticle {
                    dimension: "minecraft:overworld",
                    particle: crate::lan::server_effects::ServerParticle::simple(
                        ParticleType::Flame,
                    ),
                    at,
                    spread: [0.0; 3],
                    speed: [0.0; 3],
                    count: 1,
                    randomization: 0,
                    except: None,
                });
            game.server
                .pending_native_sounds
                .push(crate::game::creatures::NativeSound {
                    dimension: "minecraft:overworld",
                    sound: "minecraft:entity.pig.ambient".into(),
                    at,
                    source: 0,
                    volume: 1.0,
                    pitch: 1.0,
                    seed: 0,
                    except: None,
                    entity: None,
                    skip_hooks: false,
                });
            game.spill.0.orbs.push((at, 1, 0));
            rain_on_the_player(game, true);

            assert!(game.pack_player_status().wet, "rain");
            assert_eq!(game.server.pending_particles.len(), 1, "particles");
            assert_eq!(game.server.pending_native_sounds.len(), 1, "sounds");
            assert_eq!(game.spill.0.orbs.len(), 1, "spill");
            game.lan = None;
        });
    }
}
