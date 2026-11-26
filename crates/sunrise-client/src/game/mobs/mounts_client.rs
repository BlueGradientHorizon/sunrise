use super::Game;
use sunrise_client::hud::HudBatch;
use sunrise_sim::mobs::Species;

impl Game<'_, crate::game::client_side::ClientState> {
    /// The jump bar when it is the contextual bar.
    pub(in crate::game) fn draw_jump_bar(&self, batch: &mut HudBatch, w: i32, h: i32) {
        let Some(i) = self.mount_index() else {
            return;
        };
        let m = &self.mobs[i];
        // JumpableVehicleBar: `getJumpCooldown`, which only the camel's and
        // the nautilus' dash have; a horse always shows the progress.
        let cooldown = i32::from(
            self.mount.jump_ticks < 0
                && (matches!(m.species(), Species::Camel | Species::CamelHusk)
                    || sunrise_sim::mobs::nautilus::is_nautilus(m.species())),
        );
        let draws = sunrise_client::hud_overlays::jump_bar(w, h, cooldown, self.mount.jump_scale);
        sunrise_client::hud_layout::paint(batch, &self.side.gui.atlas, &draws);
    }
    /// `willPrioritizeJumpInfo`: charging a jump or recharging.
    pub(in crate::game) fn mount_jumping(&self) -> bool {
        self.mount.jump_scale > 0.0 || self.mount.jump_ticks < 0
    }
    pub(in crate::game) fn client_own_passenger_index(&self, _vehicle: u32) -> usize {
        self.remote.as_ref().map_or(0, |r| r.passenger_index)
    }
}
