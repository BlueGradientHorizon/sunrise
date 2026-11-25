use super::{Game, Hit};
use sunrise_client::{animate_tick::make_campfire_smoke, items::ItemStack};
use sunrise_core::position::BlockPos;

impl Game<'_, crate::game::client_side::ClientState> {
    /// `CampfireBlock.useItemOn` as this client runs it before the server's
    /// answer (`MultiPlayerGameMode.useItemOn`): the twenty smoke puffs of
    /// `CampfireBlock.douse` and the campfire gone out, kept as predicted
    /// under `seq`. The level event's sound comes from the server. True when
    /// it doused (SUCCESS), which ends the click for both hands.
    pub(in crate::game::client_side) fn predict_campfire_douse(
        &mut self,
        hit: &Hit,
        held: Option<&ItemStack>,
        seq: i32,
    ) -> bool {
        let pos = hit.pos;
        let (Some(held), Some(mut state)) = (held, self.campfire_to_douse(pos, held)) else {
            return false;
        };
        state.properties.insert("lit".into(), "false".into());
        let Ok(unlit) = self.pack.assets.registry.resolve(&state) else {
            return false;
        };
        let Some(unlit) = self.modified_campfire(hit, held, unlit) else {
            return false;
        };
        let signal = state.properties.get("signal_fire").map(String::as_str) == Some("true");
        for _ in 0..20 {
            make_campfire_smoke(&mut self.side.particles.0, pos, signal, true);
        }
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        self.predict_blocks(&[(at, unlit)], seq);
        true
    }
}
