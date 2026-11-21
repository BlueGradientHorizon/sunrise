//! The `player_respawned` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0118

pub trait Hooks {
    /// `PlayerRespawnPositionEvent`: where `player` respawns, once the server has
    /// found it: the bed or anchor they stand up at (facing it), or the world
    /// spawn's free column. `original_level` and `original_position` are that
    /// point; `missing_respawn_block` says the bed or anchor was gone, so the
    /// server resolved the world spawn instead. Change `level`, `position`, `yaw`
    /// and `pitch` to choose the replacement's destination. `copy_original_spawn_position`
    /// (default: the block wasn't missing) says whether it keeps the old respawn point.
    /// On death this hook sees the old player, before the replacement is installed or
    /// sent to the client, even if the server had to wait for chunks first. Writes to
    /// the old body follow the usual respawn copy rules. Server-side.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_player_respawn_position",
        dispatch = "api_carrier",
        side = "server",
        contract = "listen",
        neoforge = "PlayerRespawnPositionEvent",
        version = 2
    )]
    fn player_respawn_position(
        player: PlayerRef,
        original_level: LevelDimension,
        original_position: [f64; 3],
        missing_respawn_block: bool,
        level: &mut LevelDimension,
        position: &mut [f64; 3],
        yaw: &mut f32,
        pitch: &mut f32,
        copy_original_spawn_position: &mut bool,
    );
    /// `player` (`name`) respawned (NeoForge's `PlayerRespawnEvent`): at the
    /// end of `PlayerList.respawn`, after the replacement is placed and sent to
    /// the client in its final level, with the replacement's state swapped in.
    /// Once for each death that ends in a respawn, on a server; not for an End exit.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_player_respawned",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        neoforge = "PlayerEvent.PlayerRespawnEvent",
        version = 2
    )]
    fn player_respawned(player: u128, name: &str);
}
