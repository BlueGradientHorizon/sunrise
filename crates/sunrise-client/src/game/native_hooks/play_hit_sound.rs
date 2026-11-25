use super::*;

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn native_play_hit_sound(
        &mut self,
        call: super::fire::PlayHitSoundCall<'_>,
    ) {
        self.fire_play_hit_sound(call);
    }
}
