#[allow(dead_code)]
pub(in crate::game) fn host_animations(game: &crate::game::ClientGame<'_>) -> u64 {
    game.side.pickups.0.made().get("item").copied().unwrap_or(0)
}
