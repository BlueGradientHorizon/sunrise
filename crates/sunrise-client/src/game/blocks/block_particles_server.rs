#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    blocks::{MAP_COLORS, RenderShape},
    particles::{ParticleOptions, ParticleType},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::Direction;

impl Game<'_, crate::game::server_side::ServerState> {
    /// `BlockTransformer.TransformParticle.send`: its level event.
    pub(in crate::game) fn transform_particle(&mut self, particle: &str, pos: [i32; 3]) {
        let source = self
            .side
            .lan_acting_uuid
            .map(sunrise_world::level::LevelEventPlayer);
        match particle {
            "scrape" => self.level_event_from(source, 3005, pos, 0),
            "wax_on" => self.level_event_from(source, 3003, pos, 0),
            "wax_off" => self.level_event_from(source, 3004, pos, 0),
            _ => {}
        }
    }
}
