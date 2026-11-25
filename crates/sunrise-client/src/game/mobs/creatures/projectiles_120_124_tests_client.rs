use std::collections::BTreeMap;
use sunrise_client::mobs::Who;

pub(in crate::game) fn step_recording(
    game: &mut crate::game::ServerGame<'_>,
) -> (
    Vec<(&'static str, [f64; 3], Option<Who>)>,
    BTreeMap<String, usize>,
) {
    game.start_audio_capture();
    super::super::sculk::TEST_POSTED_EVENTS.with(|e| *e.borrow_mut() = Some(Vec::new()));
    game.tick_server(false).unwrap();
    let events =
        super::super::sculk::TEST_POSTED_EVENTS.with(|e| e.borrow_mut().take().unwrap_or_default());
    let mut sounds = BTreeMap::new();
    for sound in std::mem::take(&mut game.side.pending_sounds) {
        *sounds.entry(sound.event).or_insert(0) += 1;
    }
    game.stop_audio_capture();
    (events, sounds)
}
