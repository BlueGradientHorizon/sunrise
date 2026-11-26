//! Common evoker fangs insertion shared by server spawning/loading and remote client packets.
use super::Game;
use crate::game::server_side::fangs::Fangs;
use sunrise_core::geometry::Vec3;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    pub(in crate::game) fn insert_fangs(
        &mut self,
        id: u32,
        at: [f64; 3],
        yaw: f32,
        delay: i32,
        owner: u32,
    ) {
        self.fangs.list.push(Fangs {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id,
            pos: at,
            motion: Vec3::ZERO,
            yaw,
            warmup: delay,
            life: 22,
            owner,
            started: false,
            fire_ticks: 0,
            synced_fire: false,
            has_visual_fire: false,
            random: sunrise_sim::mobs::entity_random(id, Vec3::new(at[0], at[1], at[2])),
        });
    }
}
