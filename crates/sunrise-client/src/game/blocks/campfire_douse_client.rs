#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, Hit};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::campfire_douse::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result, animate_tick::make_campfire_smoke, items::ItemStack,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::position::BlockPos;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::chunk::BlockState;

impl Game<'_> {
    /// `CampfireBlock.useItemOn` as this client runs it before the server's
    /// answer (`MultiPlayerGameMode.useItemOn`): the twenty smoke puffs of
    /// `CampfireBlock.douse` and the campfire gone out, kept as predicted
    /// under `seq`. The level event's sound comes from the server. True when
    /// it doused (SUCCESS), which ends the click for both hands.
    pub(in crate::game) fn predict_campfire_douse(
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
            make_campfire_smoke(&mut self.particles.0, pos, signal, true);
        }
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        self.predict_blocks(&[(at, unlit)], seq);
        true
    }
    #[inline(always)]
    pub(in crate::game) fn use_campfire_douse_client(&mut self, pos: [i32; 3], signal: bool) {
        for _ in 0..20 {
            make_campfire_smoke(&mut self.particles.0, pos, signal, true);
        }
    }
}
