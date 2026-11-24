#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game, Hit, decorated_pot,
    item_draw::{BlockItemDraw, block_entity_pose},
    view_vector,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::brush::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    block_entity_render::vertex_light,
    camera::rotate_degrees,
    containers::BlockEntity,
    mesher::Vertex,
    particles::{ParticleOptions, ParticleType},
    save,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client_render::block_entity_reach::Reach;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::RandomSource;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::brushing::{
    Brushing, USE_DURATION, completion_state,
};

impl Game<'_> {
    /// Level event 3008 (`LevelEventHandler`): the finished block's sound
    /// and break particles.
    pub(in crate::game) fn brush_complete_event(&mut self, pos: [i32; 3], data: i32) {
        let state = sunrise_world::registry::BlockStateId(data as u32);
        let sound = match &*self.res.facts().get(state).name {
            "minecraft:suspicious_sand" => "minecraft:item.brush.brushing.sand.complete",
            "minecraft:suspicious_gravel" => "minecraft:item.brush.brushing.gravel.complete",
            _ => "",
        };
        if !sound.is_empty() {
            let at = pos.map(|v| f64::from(v) + 0.5);
            // Each client plays it from the event itself.
            self.audio.0.play_unshared(sound, Some(at), 1.0, 1.0);
        }
        self.destroy_particles(pos, state);
    }
    /// `BrushItem.spawnDustParticles`.
    pub(in crate::game) fn brush_dust(&mut self, hit: &Hit, off_hand: bool) {
        let state = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
        let Some((sprite, tint)) = self.particle_sprite(state, hit.pos) else {
            return;
        };
        let options = ParticleOptions::block(ParticleType::Block, sprite, tint);
        // The arm that brushes: the main arm, or the other one for the off hand.
        let flip = if off_hand == self.settings.main_hand_left {
            1.0
        } else {
            -1.0
        };
        let view = view_vector(self.yaw, self.pitch);
        // DustParticlesDelta.fromDirection.
        let (dx, dz) = match hit.face {
            [0, _, 0] => (view[2], -view[0]),
            [0, 0, -1] => (1.0, -0.1),
            [0, 0, 1] => (-1.0, 0.1),
            [-1, 0, 0] => (-0.1, -1.0),
            _ => (0.1, 1.0),
        };
        let count = 7 + self
            .particles
            .0
            .level_random()
            .next_int_bound(5)
            .unwrap_or(0);
        let x = hit.point[0] - if hit.face == [-1, 0, 0] { 1.0e-6 } else { 0.0 };
        let z = hit.point[2] - if hit.face == [0, 0, -1] { 1.0e-6 } else { 0.0 };
        for _ in 0..count {
            let random = self.particles.0.level_random();
            let v = [
                dx * flip * 3.0 * random.next_double(),
                0.0,
                dz * flip * 3.0 * random.next_double(),
            ];
            self.particles.0.add(&options, [x, hit.point[1], z], v);
        }
    }
}
