//! The keys whose presses send a packet on a server's client (swap hands, drop, a mount's
//! inventory): counted by the key handling, drained completely in the tick's `handleKeybinds`
//! phase (`while (key.consumeClick())`).
use super::{Game, S};
use sunrise_protocol::packets::play;
use winit::keyboard::KeyCode;

/// `handleKeybinds`' order for the keys counted here: the inventory key, swap hands, then drop.
pub(super) const KEY_ORDER: [KeyCode; 3] = [KeyCode::KeyE, KeyCode::KeyF, KeyCode::KeyQ];

pub(super) fn key_index(key: KeyCode) -> Option<usize> {
    KEY_ORDER.iter().position(|&k| k == key)
}

impl Game<'_> {
    /// handleKeybinds' keybind polls, before its attack, use and pick: each key's presses
    /// (`consumeClick` until none is left) in the fixed order of `KEY_ORDER`, whatever order
    /// they were pressed in, and all within the tick, so no packet goes out between
    /// CLIENT_TICK_END and a pong.
    pub(super) fn remote_queued_keys(&mut self) {
        let pending = self
            .remote
            .as_mut()
            .map(|r| std::mem::take(&mut r.pending_keys))
            .unwrap_or([0; 3]);
        for (key, count) in KEY_ORDER.into_iter().zip(pending) {
            for _ in 0..count {
                self.remote_key_press(key);
            }
        }
    }
    fn remote_key_press(&mut self, key: KeyCode) {
        match key {
            KeyCode::KeyF if !self.spectator() => self.remote_swap_hands(),
            KeyCode::KeyQ if !self.spectator() => {
                // Minecraft.hasControlDown: the physical keys, so a toggled
                // sprint on Left Control does not make Q drop the stack.
                let all = self.has_control_down() || self.input_takes_stack_drop();
                self.remote_drop(all);
            }
            KeyCode::KeyE
                if !self.in_portal_transition()
                    && (self.riding_mount_with_menu() || self.riding_chest_boat()) =>
            {
                let id = self.remote.as_ref().map_or(0, |r| r.entity_id);
                self.send(S::PlayerCommand(play::PlayerCommand {
                    id,
                    action: 5,
                    data: 0,
                }));
            }
            _ => {}
        }
    }
    /// `Minecraft.tick`'s `missTime` count, after `handleKeybinds`.
    pub(super) fn remote_count_miss_time(&mut self) {
        if self.clicks.miss_time > 0 {
            self.clicks.miss_time -= 1;
        }
    }
}
