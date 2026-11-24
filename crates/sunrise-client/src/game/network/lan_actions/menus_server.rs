#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::menus::*;

impl Game<'_> {
    /// `ContainerOpenersCounter.incrementOpeners` / `ContainerOpenersCounter.decrementOpeners`:
    /// the first opener and the last closer each trigger a sound and game event.
    pub(in crate::game) fn guest_container_opener(
        &mut self,
        conn: u32,
        pos: (i32, i32, i32),
        open: bool,
    ) {
        let acting = self.server.lan_acting == Some(conn);
        let spectator = if acting {
            self.spectator()
        } else {
            let Some(guest) = self.lan.as_ref().and_then(|host| host.hub.guest(conn)) else {
                return;
            };
            guest.spectator()
        };
        if spectator || !self.container_openers_changed(pos, open) {
            return;
        }
        let state = self.world.block(pos.0, pos.1, pos.2);
        let name = &self.res.facts().get(state).name;
        let shulker = super::super::storage::is_shulker_box(name);
        // Resolve the sound before the game event; its listener can replace the block.
        let sound = self.resolve_container_sound(pos, name, open);
        // ContainerOpenersCounter calls onOpen/onClose before the game event;
        // ShulkerBoxBlockEntity emits its game event before playing its sound.
        if !shulker && let Some((sound, at)) = sound {
            self.play_container_sound(sound, at);
        }
        let event = if open {
            "minecraft:container_open"
        } else {
            "minecraft:container_close"
        };
        let post = |game: &mut Self| {
            game.post_block_game_event(
                event,
                [pos.0, pos.1, pos.2],
                Some(sunrise_client::mobs::Who::Player),
                None,
            );
        };
        if acting {
            post(self);
        } else {
            self.as_guest(conn, post);
        }
        if shulker && let Some((sound, at)) = sound {
            self.play_container_sound(sound, at);
        }
    }
    /// Whether the mods let the guest open the menu `kind` (`native_container_opened`),
    /// heard with the guest's state swapped in. Asked before the menu that
    /// was open closes. None if the guest's turn cannot be entered.
    pub(in crate::game) fn guest_may_open(
        &mut self,
        conn: u32,
        kind: &str,
        pos: Option<[i32; 3]>,
    ) -> Option<bool> {
        let guest = self.lan.as_ref()?.hub.guest(conn)?;
        if !guest.participates_in_player_simulation() {
            return None;
        }
        let (uuid, name) = (guest.uuid, guest.name.clone());
        let hook = |game: &mut Self| {
            let mut cancel = false;
            game.native_container_opened(uuid, &name, kind, pos, &mut cancel);
            !cancel
        };
        // The guest whose turn it is is in place already.
        if self.server.lan_acting == Some(conn) {
            Some(hook(self))
        } else {
            self.as_guest(conn, hook)
        }
    }
}
