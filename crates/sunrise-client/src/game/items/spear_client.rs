#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, view_vector};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::spear::OWN;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::spear::Target;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::spear::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::spear::hits_in_reach;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    attack_strength::delay,
    collisions::SceneCollisions,
    first_person::Hand,
    items::ItemStack,
    mobs::Damage,
    spear::{HIT_FEEDBACK_TICKS, Kinetic, REACH},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3, traverse_blocks},
    position::BlockPos,
};

impl Game<'_, crate::game::client_side::ClientState> {
    /// A sound at the player's feet.
    pub(in crate::game) fn client_play_at_feet(&mut self, sound: &str) {
        if let Some(p) = &self.player {
            let at = [p.position.x, p.position.y, p.position.z];
            self.side.audio.0.play(sound, Some(at), 1.0, 1.0);
        }
    }
    /// `getTicksSinceLastKineticHitFeedback` of this window's player.
    pub(in crate::game) fn kinetic_hit_ticks(&self, partial: f32) -> f32 {
        self.kinetic_feedback
            .get(&OWN)
            .map_or(0.0, |&t| (self.world.game_time - t) as f32 + partial)
    }
    /// `LivingEntity.handleEntityEvent(2)` on a server's client: the
    /// player `id` (this one, or another) was hit by the spear it uses.
    pub(in crate::game) fn remote_kinetic_hit(&mut self, id: i32) {
        let own = self.remote.as_ref().is_some_and(|r| r.entity_id == id);
        let (item, at) = if own {
            let item = self
                .using
                .and_then(|(slot, _)| self.stack_in(slot))
                .map(|s| s.item.clone());
            let at = self.player.as_ref().map(|p| p.position);
            (item, at)
        } else {
            let mob = self
                .mobs
                .iter()
                .find(|m| m.id == id as u32 && m.extra.using_item > 0);
            let item = mob.and_then(|m| {
                if m.extra.using_off_hand {
                    sunrise_client::mobs::finalize::item(m, sunrise_client::mobs::finalize::OFFHAND)
                } else {
                    sunrise_client::mobs::finalize::item(
                        m,
                        sunrise_client::mobs::finalize::MAINHAND,
                    )
                }
            });
            (item, mob.map(|m| m.position))
        };
        if let (Some(kinetic), Some(at)) = (item.as_deref().and_then(Kinetic::of), at) {
            self.kinetic_hit_feedback(if own { OWN } else { id }, &kinetic, [at.x, at.y, at.z]);
        }
    }
}
