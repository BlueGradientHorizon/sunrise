#[allow(unused_imports)]
pub(in crate::game::server_side) use super::native_entity_refs::tests::{
    fixture_root, run_fresh_cases, with_client_game,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, PackResources, Screen};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::ASKED;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::Asked;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::CLEAR_BLUE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::Choice;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::DRAWING;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::EXTRACTED;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::FirstChoice;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::InvertWater;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::LEAVE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::LastChoice;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::ObserveOverlay;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::OverlayInput;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::PLAN;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::Painter;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::Plan;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::Planned;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::PlannedRenderer;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::QUAD;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::RED;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::ReceiveCancelled;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::block_screen_effect_draw_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use BlockScreenEffect::{Block, Fire, Water};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    BlockScreenEffect, BlockScreenEffectRender, BlockScreenEffectRenderer, CameraSubmersionEvent,
    ClientHost, ExtractBlockScreenEffectEvent, Flow, GuiRenderPostEvent, Hooks, ModEntry, Mods,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    camera::{CameraType, hand_projection},
    chunk::Chunk,
    clock::FixedStep,
    fog::FogType,
    gfx::ScreenshotRequest,
    motion::{MotionWorld, PlayerMotion, WATER},
    screen_effects::{Blend, ScreenQuad},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{geometry::Vec3, position::BlockPos};

/// Sample the player's fluid state as a tick would, without advancing their fire timer.
pub(in crate::game) fn sample_eye_in_water(game: &mut crate::game::ClientGame<'_>) -> bool {
    // The fluid scan requires the chunks around the interaction box to be loaded too.
    let layout = game.world.layout();
    for (x, z) in [(-1, -1), (-1, 0), (0, -1)] {
        if !game.world.chunks.contains_key(&(x, z)) {
            game.world
                .chunks
                .insert((x, z), Arc::new(Chunk::with_layout(x, z, layout)));
        }
    }
    let side = game_side!(game);
    let mut motion = MotionWorld {
        side: &side,
        entities: &[],
        player: game.player.as_mut().unwrap(),
        scene: &mut game.world,
        assets: &game.pack.assets,
        collision_binding: game.collision,
        resources: game.motion,
        health: &game.health_resources,
        loop_context: &game.loop_context,
        damage_hooks: &game.damage_hooks,
        tag_items: None,
        block_registries: None,
    };
    motion.update_fluids().unwrap();
    motion.player.is_eye_in_fluid(WATER)
}
