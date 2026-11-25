/// Runs `command`: whether it succeeded and what the issuer was told.
pub(in crate::game) fn run(
    game: &mut crate::game::ServerGame<'_>,
    command: &str,
) -> (bool, Vec<String>) {
    game.side.chat_capture = Some(Vec::new());
    let ok = game.run_command(command);
    let said = game.side.chat_capture.take().unwrap_or_default();
    (
        ok,
        said.iter().map(sunrise_client::text::Text::plain).collect(),
    )
}
