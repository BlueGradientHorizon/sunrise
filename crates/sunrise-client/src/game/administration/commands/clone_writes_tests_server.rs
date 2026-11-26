/// Runs `command`: whether it succeeded and what it said first.
pub(in crate::game) fn run(
    game: &mut crate::game::ServerGame<'_>,
    command: &str,
) -> (bool, Option<String>) {
    game.side.chat_capture = Some(Vec::new());
    let ok = game.run_command(command);
    let said = game.side.chat_capture.take().unwrap_or_default();
    (ok, said.first().map(sunrise_client_core::text::Text::plain))
}
