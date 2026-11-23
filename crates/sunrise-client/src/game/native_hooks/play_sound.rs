use super::*;

impl Game<'_> {
    /// `PlaySoundEvent` (vanilla's `SoundEngine.play`): the engine prepared the start or live
    /// refresh, and the game fires the hook with its `ClientHost`.
    pub(in crate::game) fn native_play_sound(&mut self, call: super::fire::PlaySoundCall<'_>) {
        self.fire_play_sound(call);
    }

    /// Fires `play_sound` for every start the engine prepared and every live refresh it needs, then
    /// hands each answer back: a cancel, an edit or the sound as asked for. Runs before the audio
    /// tick, so sounds started during a tick play on the next one. With no listener, nothing
    /// is prepared and this costs one check.
    pub(in crate::game) fn dispatch_play_sounds(&mut self) {
        while let Some(prepared) = self.audio.next_prepared() {
            let mut sound = Some(prepared.original.clone());
            self.native_play_sound(super::fire::PlaySoundCall {
                instance: prepared.instance,
                starting: true,
                original: &prepared.original,
                sound: &mut sound,
            });
            self.audio.finish_prepared(prepared.instance, sound);
        }
        for prepared in self.audio.live_requests() {
            let mut sound = Some(prepared.original.clone());
            self.native_play_sound(super::fire::PlaySoundCall {
                instance: prepared.instance,
                starting: false,
                original: &prepared.original,
                sound: &mut sound,
            });
            self.audio.set_live_answer(prepared.instance, sound);
        }
    }
}
