//! Client `play_sound` hook dispatch: `SoundEngine` stages starts and live refreshes (decide),
//! `Game` dispatches `play_sound` here with its scoped [`ClientHost`](mod_api::ClientHost) (fire),
//! and hands the answer back to `SoundEngine::apply_play_sound` (apply).

use super::*;
use sunrise_client::sound_engine::{InstanceId, PlayResult, SoundEngine, SoundInstance, Sounds};

/// `Sounds` adapter over `&mut Game<'_>` that flushes staged `play_sound` decisions synchronously
/// so callers that read `PlayResult`, `is_active`, or `instance.random` right after `play_*`
/// observe the applied decision in vanilla order.
pub(in crate::game) struct GameSounds<'a, 'b>(pub(in crate::game) &'a mut Game<'b>);

impl Sounds for GameSounds<'_, '_> {
    fn engine(&mut self) -> &mut SoundEngine {
        &mut self.0.audio.engine
    }
    fn play_instance(&mut self, instance: SoundInstance, retain: bool) -> (InstanceId, PlayResult) {
        let (id, result) = self.0.audio.play_instance(instance, retain);
        let result = self.0.flush_play_sound_for(id).unwrap_or(result);
        (id, result)
    }
    fn play_id(&mut self, id: InstanceId) -> PlayResult {
        let result = self.0.audio.play_id(id);
        self.0.flush_play_sound_for(id).unwrap_or(result)
    }
    fn stop_instance(&mut self, id: InstanceId) {
        self.0.audio.stop_instance(id);
    }
    fn update_category_volume(&mut self, source: usize, gain: f32) {
        self.0.audio.update_category_volume(source, gain);
        self.0.flush_play_sound();
    }
}

impl Game<'_> {
    /// Hook point for `play_sound` (`PlaySoundEvent`, vanilla's `SoundEngine.play`).
    /// Fired from `Game` with its scoped `ClientHost` after `SoundEngine` stages a pending start
    /// or live refresh, before applying the decision back to the same instance.
    #[inline(never)]
    pub(in crate::game) fn native_play_sound(
        &mut self,
        instance: u64,
        starting: bool,
        original: &mod_api::SoundPlay,
        sound: &mut Option<mod_api::SoundPlay>,
    ) {
        self.fire_play_sound(instance, starting, original, sound);
    }

    /// Play a sound instance and flush any staged `play_sound` decision synchronously so callers
    /// that inspect `PlayResult` (such as minecart loops) receive the applied result.
    pub(in crate::game) fn play_sound_instance(
        &mut self,
        instance: SoundInstance,
        retain: bool,
    ) -> (InstanceId, PlayResult) {
        let (id, result) = self.audio.play_instance(instance, retain);
        let result = self.flush_play_sound_for(id).unwrap_or(result);
        (id, result)
    }

    /// Fire and apply every staged `play_sound` decision in FIFO order. Free when empty.
    pub(in crate::game) fn flush_play_sound(&mut self) {
        if !self.audio.has_pending_play_sound() {
            return;
        }
        while let Some(pending) = self.audio.next_pending_play_sound() {
            let mut sound = Some(pending.original.clone());
            self.native_play_sound(
                pending.instance,
                pending.starting(),
                &pending.original,
                &mut sound,
            );
            self.audio.apply_play_sound(pending, sound);
        }
    }

    /// Fire and apply staged `play_sound` decisions in FIFO order, returning the applied
    /// `PlayResult` for `target`'s start if one was staged.
    pub(in crate::game) fn flush_play_sound_for(
        &mut self,
        target: InstanceId,
    ) -> Option<PlayResult> {
        if !self.audio.has_pending_play_sound() {
            return None;
        }
        let mut target_result = None;
        while let Some(pending) = self.audio.next_pending_play_sound() {
            let id = pending.instance;
            let starting = pending.starting();
            let mut sound = Some(pending.original.clone());
            self.native_play_sound(id, starting, &pending.original, &mut sound);
            let result = self.audio.apply_play_sound(pending, sound);
            if id == target && starting {
                target_result = Some(result);
            }
        }
        target_result
    }
}

#[cfg(test)]
mod play_sound_tests {
    use crate::game::native_entity_refs::tests::with_client_game;
    use mod_api::{ClientHost, Hooks, ModEntry, PlaySoundEvent};
    use mod_platform::testing::with_mods;
    use std::sync::{Arc, Mutex};
    use sunrise_client::sound_engine::{PlayResult, SoundInstance, Sounds};
    use sunrise_core::random::LegacyRandom;

    #[test]
    #[ignore = "daily"]
    fn play_sound_receives_client_host_and_refuses_an_instance_released_before_apply() {
        #[derive(Default)]
        struct Observed {
            saw_click_event: bool,
            device: Option<String>,
            muffled: bool,
            played: Vec<(u64, bool, String)>,
        }

        struct Observer(Arc<Mutex<Observed>>);

        #[mod_api::__subscriptions]
        impl Hooks for Observer {
            fn play_sound(&self, host: &mut dyn ClientHost, event: &mut PlaySoundEvent<'_>) {
                let mut log = self.0.lock().unwrap();
                log.saw_click_event |= host
                    .sound_events()
                    .iter()
                    .any(|id| id.as_str() == "minecraft:ui.button.click");
                log.device = Some(host.audio_device());
                if event.original.identifier == "minecraft:ui.button.click" {
                    log.muffled = host.muffle_sounds(0.5, 0.25, 4, &[]);
                    if let Some(sound) = event.sound.as_mut() {
                        sound.volume = 0.5;
                    }
                }
                log.played.push((
                    event.instance,
                    event.starting,
                    event.original.identifier.clone(),
                ));
            }
        }

        let observed = Arc::new(Mutex::new(Observed::default()));
        let mods = vec![ModEntry::new(
            "observer",
            Box::new(Observer(observed.clone())),
        )];
        with_mods(mods.into(), || {
            with_client_game(|game| {
                // Enable the loaded flag so SoundEngine can allocate a channel on its silent backend
                // if apply succeeds, proving a released instance is actually refused rather than
                // merely blocked by `!loaded`.
                game.audio.engine.loaded = true;
                game.audio.engine.registry.add_file(
                    "minecraft",
                    br#"{"ui.button.click": {"sounds": ["a"]}, "block.stone.break": {"sounds": ["a"]}}"#,
                    &|_, _| true,
                );

                // Normal start through Game's decide -> fire -> apply boundary: listener sees
                // ClientHost state (level, sound_device, muffle_sounds) and edits apply.
                let click = SoundInstance::for_ui(
                    "minecraft:ui.button.click",
                    1.0,
                    1.0,
                    LegacyRandom::new(1),
                );
                let (click_id, click_result) = game.play_sound_instance(click, true);
                assert_eq!(click_result, PlayResult::Started);
                assert!(game.audio.engine.is_active(click_id));
                assert_eq!(game.audio.engine.instance(click_id).unwrap().volume, 0.5);

                // Instance released between decide (`audio.play_instance`) and apply
                // (`flush_play_sound_for`): must be refused and never become active.
                let dropped = SoundInstance::for_ui(
                    "minecraft:block.stone.break",
                    1.0,
                    1.0,
                    LegacyRandom::new(2),
                );
                let (dropped_id, staged) = game.audio.play_instance(dropped, true);
                assert_eq!(staged, PlayResult::NotStarted);
                assert!(game.audio.has_pending_play_sound());
                assert!(!game.audio.engine.is_active(dropped_id));

                game.audio.release(dropped_id);
                assert!(game.audio.engine.instance(dropped_id).is_none());

                let applied = game.flush_play_sound_for(dropped_id);
                assert_eq!(applied, Some(PlayResult::NotStarted));
                assert!(!game.audio.engine.is_active(dropped_id));
                assert!(game.audio.engine.instance(dropped_id).is_none());
            });
        });

        let log = observed.lock().unwrap();
        assert!(
            log.saw_click_event,
            "play_sound listener must receive a live ClientHost"
        );
        assert!(log.device.is_some());
        assert!(
            log.muffled,
            "play_sound listener can call &mut ClientHost methods"
        );
        assert_eq!(log.played.len(), 2);
        assert_eq!(log.played[0].2, "minecraft:ui.button.click");
        assert!(log.played[0].1);
        assert_eq!(log.played[1].2, "minecraft:block.stone.break");
        assert!(log.played[1].1);
    }
}
