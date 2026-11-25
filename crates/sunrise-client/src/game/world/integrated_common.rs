use super::Game;

impl Game<'_> {
    /// `Minecraft.hasSingleplayerServer`: a world of this window's own, run on its integrated server.
    pub(in crate::game) fn has_singleplayer_server(&self) -> bool {
        self.side_has_singleplayer_server()
    }

    /// `Minecraft.isMultiplayerServer`: on another's server, or a world of one's own open to LAN.
    pub(in crate::game) fn is_multiplayer_server(&self) -> bool {
        !self.has_singleplayer_server() || self.is_published()
    }

    /// `IntegratedServer.isPublished`: the world is open to the network.
    pub(in crate::game) fn is_published(&self) -> bool {
        self.side_is_published()
    }
}
