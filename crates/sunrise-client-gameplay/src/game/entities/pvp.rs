use sunrise_client::pvp::ChatOptions;
use sunrise_level::particle_options::ParticleType;

#[derive(Default)]
pub(in crate::game) struct PvpState {
    /// While the chat box holds the search query (Ctrl+F), its message text and draft state.
    pub(in crate::game) chat_search_aside: Option<(String, bool)>,
}
impl crate::game::ClientGame<'_> {
    /// Combat feel Legacy's pictures: 1.8.9's sword block, flat item pose and hand swing.
    pub(in crate::game::client_side) fn pvp_legacy_look(&self) -> bool {
        self.client.settings.extras.pvp.combat.legacy
    }
    /// Combat feel Legacy hides the attack indicator unless the player keeps it.
    pub(in crate::game::client_side) fn pvp_hides_attack_indicator(&self) -> bool {
        let combat = self.client.settings.extras.pvp.combat;
        combat.legacy && !combat.indicator
    }
    /// A sound Combat feel Legacy keeps quiet.
    pub(in crate::game) fn pvp_mutes_sound(&self, name: &str) -> bool {
        self.client.settings.extras.pvp.combat.mutes_sound(name)
    }
    /// A particle Combat feel Legacy doesn't draw (sweeps, damage hearts).
    pub(in crate::game::client_side) fn pvp_hides_particle(&self, ty: ParticleType) -> bool {
        self.client.settings.extras.pvp.combat.hides_particle(ty)
    }
    /// The chat's options: Ctrl+F search, copy on click, and the rest (`ChatOptions::qol`).
    pub(in crate::game::client_side) fn pvp_chat(&self) -> ChatOptions {
        self.client.settings.extras.pvp.chat
    }
}
