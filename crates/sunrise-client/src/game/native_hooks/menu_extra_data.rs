#[allow(unused_imports)]
use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// Block `block` at `pos` opens its mod menu type `menu` for server
    /// player `player`: set `data` to hand the menu's screen bytes of your
    /// own (a machine's tier, mode or recipes), up to
    /// `sunrise_protocol::menu_extra::MAX_DATA` (1,048,555 bytes, within
    /// vanilla's 1 MiB clientbound custom payload bound), which the client
    /// reads only from `MenuScreenOpen::extra` during construction
    /// (NeoForge's `openMenu` with an extra-data writer). Oversize data is
    /// logged and refuses the menu open. The first mod to set it wins, so
    /// answer only for your blocks. Live data slots travel separately. The
    /// player's state is not swapped in.
    pub(in crate::game) fn native_menu_extra_data(
        &mut self,
        player: u128,
        menu: &str,
        block: &str,
        pos: [i32; 3],
        data: &mut Option<Vec<u8>>,
    ) {
        self.fire_menu_extra_data(player, menu, block, pos, data);
    }
}

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
impl Game<'_> {
    /// The opener position for the currently shown mod menu, if known.
    pub(in crate::game) fn screen_pos(&self) -> Option<[i32; 3]> {
        self.shown_native_screen()?.menu.pos
    }
    /// The `sunrise:menu_extra` payload the server sent just before its
    /// `OpenScreen`. The pending message must match the opening; its bytes are
    /// borrowed by that factory and then discarded. A malformed payload logs an
    /// error and disconnects the client.
    pub(in crate::game) fn receive_menu_extra(&mut self, data: &[u8]) -> crate::game::Result<()> {
        match sunrise_protocol::menu_extra::decode(data) {
            Ok(extra) => {
                if let Some(r) = self.remote.as_mut() {
                    r.menu_extra = Some(extra);
                }
                Ok(())
            }
            Err(e) => {
                log::error!("Sunrise: malformed menu extra payload: {e}");
                let reason =
                    Text::translate("disconnect.genericReason", &[Text::literal(e.to_string())]);
                self.leave_server(Some(reason))
            }
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod menu_extra_data_tests {
    #[allow(unused_imports)]
    use sunrise_client::title::Mode;
    #[allow(unused_imports)]
    use sunrise_protocol::{
        connection::Pipe,
        menu_extra::CHANNEL,
        packets::{configuration::CustomPayload, play::Clientbound as C},
    };
}
