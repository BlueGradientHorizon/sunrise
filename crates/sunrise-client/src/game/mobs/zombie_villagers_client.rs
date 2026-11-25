use super::Game;
use sunrise_client::mobs::zombie_villager;

impl Game<'_, crate::game::client_side::ClientState> {
    /// `ZombieVillager.handleEntityEvent(16)`: only the client plays the cure sound.
    pub(in crate::game::client_side) fn zombie_villager_cure_sound(&mut self, id: u32) {
        if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id)
            && let Some(sunrise_client::mobs::MobEvent::Sound(name, at, volume, pitch)) =
                zombie_villager::cure_sound(m)
        {
            self.side.audio
                .0
                .play_unshared(&format!("minecraft:{name}"), Some(at), volume, pitch);
        }
    }
}
