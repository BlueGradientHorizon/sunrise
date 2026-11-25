#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::native_shaders::{
    fingerprint, outcome, refused, remember,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::remote_camera::CameraFogEffects;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Dim, Game, Screen};
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::BlockIdVotes;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::PackIds;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::PickedPack;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::TogglePlan;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::shader_packs::picks_material_pack;
#[cfg(feature = "client")]
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
    /// `LivingEntity.isAlive`, `Entity.isCurrentlyGlowing` (the Glowing effect fallback),
    /// `Entity.isInLava` and `Entity.isInWaterOrRain`.
    #[test]
    #[ignore = "daily"]
    fn pack_status_follows_the_player() {
        crate::game::native_entity_refs::tests::with_client_game(|game| {
            stand_player(game);

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
}
