use std::sync::Arc;
use sunrise_client::{
    chunk::Chunk,
    motion::{MotionWorld, WATER},
};

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
