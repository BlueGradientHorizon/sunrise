//! Upstream prose, retained with fork routing described in the modding book.
pub trait Notes {
    #[notes = "`sound_play` intercepts actual client sound starts and live parameter updates. `SoundPlayback` identifies the instance, event, source, position, relative flag and whether it is starting. Edit base volume/pitch and the sound's `effects` (`SoundPlay::effects`: the mixer's own gain, low-pass and high-pass filters and sends to three shared reverbs, `room`, `hall` and `cavern`) transiently, or cancel before subtitles/backend writes; invalid edits cancel, out-of-range effects included (`SoundEffects::valid`). A sound that doesn't tick keeps the effects it started with. No Host/world borrow is supplied. Runs on the game thread; use existing client hooks for world/acoustics state. Bundles need \"hooks\": @API:sound_play@."]
    fn sound_play();
}
