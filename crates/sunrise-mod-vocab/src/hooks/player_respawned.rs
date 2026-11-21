//! The `player_respawned` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0118

pub trait Hooks {
    /// `PlayerRespawnPositionEvent`: where `player` respawns, once the server has
    /// found it: the bed or anchor they stand up at (facing it), or the world
    /// spawn's free column. `original_level` and `original_position` are that
    /// point; `missing_respawn_block` says the bed or anchor was gone, so the
    /// player came back at the world spawn. Change `level`, `position`, `yaw`
    /// and `pitch` to choose their final destination; no preliminary placement is made.
    /// `copy_original_spawn_position` (default: the block wasn't missing) says whether
    /// they keep their respawn point. Runs on the original player before `player_clone`
    /// and placement, on death respawn and the respawn after the End credits. Server-side.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_player_respawn_position",
        dispatch = "api_carrier",
        side = "server",
        contract = "listen",
        version = 2,
        neoforge = "PlayerRespawnPositionEvent"
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
    /// end of `PlayerList.respawn`, once they are placed and their health,
    /// food and experience reset, with their state swapped in. Once for each
    /// death that ends in a respawn, on a server; not for the End-exit one.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_player_respawned",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        neoforge = "PlayerEvent.PlayerRespawnEvent"
    )]
    fn player_respawned(player: u128, name: &str);
}
