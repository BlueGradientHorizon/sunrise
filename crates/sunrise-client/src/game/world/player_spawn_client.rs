#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::player_spawn::*;

impl Game<'_> {
    #[inline(always)]
    #[allow(dead_code)]
    pub(in crate::game) fn spawn_player_client(
        &mut self,
        player: PlayerMotion,
        feet: Vec3,
        entering: bool,
    ) {
        // A saved death already dropped inventory/XP and counted the death.
        if player.health.dead_or_dying() {
            self.set_screen(Screen::Dead);
            self.client.death_focus = None;
            self.ui.died_at = Some(Instant::now());
            self.ui.death_text = None;
            self.ui.death_confirm = None;
            self.capture(false);
        }
        self.previous_position = feet;
        if entering {
            // The world's first player: a server's client gets it with its
            // level (Minecraft.setLevel), which stops every sound.
            self.stop_sounds_for_new_level();
        }
        self.hook.discard();
        self.player_state.attachments.retire();
        self.resident_entities.local_player.renew();
        self.player = Some(player);
        self.bind_local_player_residency();
        self.input_player_replaced();
        self.client.sway = Default::default();
        self.apply_equipment();
        self.eye = 1.62;
        self.client.previous_eye = 1.62;
        self.clock.reset();
        self.capture_loaded_world();
    }
}
