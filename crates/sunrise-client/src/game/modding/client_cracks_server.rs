#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Mining};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::client_cracks::ClientCrack;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::client_cracks::*;

impl Game<'_> {
    /// Preserve the stage's stored display answer across prediction updates.
    pub(in crate::game) fn set_client_mining(&mut self, mut mining: Option<Mining>) {
        if let Some(next) = &mut mining
            && let Some(old) = self.mining.as_mut().filter(|old| old.pos == next.pos)
        {
            next.crack = old.crack.take();
        }
        self.mining = mining;
        self.update_local_crack();
    }
    pub(in crate::game) fn update_local_crack(&mut self) {
        if !self.native_window() {
            return;
        }
        let Some(mining) = self.mining.as_mut() else {
            return;
        };
        let Some(stage) = mining.stage() else {
            mining.crack = None;
            return;
        };
        let pos = mining.pos;
        if mining
            .crack
            .as_ref()
            .is_some_and(|crack| crack.stage == stage)
        {
            return;
        }
        // Mark this stage before dispatch, so a reentrant update cannot ask twice.
        mining.crack = Some(ClientCrack {
            pos,
            stage,
            updated_at: self.world.game_time,
            positions: Vec::new(),
        });
        self.with_break_progress_positions(pos, stage, |game, positions| {
            if let Some(mining) = &mut game.mining
                && mining.pos == pos
                && mining.stage() == Some(stage)
                && let Some(crack) = &mut mining.crack
            {
                crack.positions = positions;
            }
        });
    }
}
