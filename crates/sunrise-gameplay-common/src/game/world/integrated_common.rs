use crate::game::CommonGame;

impl CommonGame<'_> {
    /// `Minecraft.hasSingleplayerServer`: a world of this window's own, run on its integrated server.
    pub fn has_singleplayer_server(&self) -> bool {
        self.side_has_singleplayer_server()
    }

    /// `Minecraft.isMultiplayerServer`: on another's server, or a world of one's own open to LAN.
    pub fn is_multiplayer_server(&self) -> bool {
        !self.has_singleplayer_server() || self.is_published()
    }

    /// `IntegratedServer.isPublished`: the world is open to the network.
    pub fn is_published(&self) -> bool {
        self.side_is_published()
    }
}

use crate::game::side_ops::presentation::PresentationSideGameExt as _;
