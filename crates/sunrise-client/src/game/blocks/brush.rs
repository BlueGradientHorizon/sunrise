//! Brushing suspicious sand and gravel: `BrushItem.onUseTick` and
//! `BrushableBlockEntity`. The block's loot table stays in its saved block
//! entity (like a decorated pot's) until the first counted stroke rolls it.
#[allow(unused_imports)]
use super::{
    Game, Hit, decorated_pot,
    item_draw::{BlockItemDraw, block_entity_pose},
    view_vector,
};
#[allow(unused_imports)]
use sunrise_client::{
    block_entity_render::vertex_light,
    camera::rotate_degrees,
    containers::BlockEntity,
    mesher::Vertex,
    particles::{ParticleOptions, ParticleType},
    save,
};
#[allow(unused_imports)]
use sunrise_client_render::block_entity_reach::Reach;
#[allow(unused_imports)]
use sunrise_core::random::RandomSource;

#[allow(unused_imports)]
pub(in crate::game) use sunrise_gameplay::brushing::{Brushing, USE_DURATION, completion_state};

impl<__Side: crate::game::Side> Game<'_, __Side> {
    pub(in crate::game) fn set_dusted(&mut self, pos: [i32; 3], dusted: i32) {
        let registry = self.pack.assets.registry;
        let Some(mut state) = registry
            .state(self.world.block(pos[0], pos[1], pos[2]))
            .cloned()
        else {
            return;
        };
        state.properties.insert("dusted".into(), dusted.to_string());
        if let Ok(state) = registry.resolve(&state) {
            let _ = self.level_set(pos, state);
        }
    }

    pub(in crate::game) fn schedule_brushable_tick(&mut self, pos: [i32; 3]) {
        let at = sunrise_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        self.with_level(|b, level, ticks| b.schedule_tick(level, ticks, at, 2));
    }
}
