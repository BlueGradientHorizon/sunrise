//! The sound engine's hook points for native mods. `play_sound` stages pending starts and
//! refreshes here so the game can dispatch it with its scoped [`ClientHost`](mod_api::ClientHost)
//! and apply the decision back to the same instance. The three engine-loop notices
//! (`play_sound_source`, `play_streaming_source`, `sound_engine_load`) still fire from the
//! sound engine with the client's [`LoopContext`](mod_api::LoopContext).
use super::{Attenuation, Backend, ChannelId, InstanceId, PlayResult, SoundEngine, SoundInstance};
use mod_api::__private::built;
use mod_api::{ChannelRef, HookId, SoundAttenuation, SoundPlay, SoundSource};

/// Hook point for native mods (docs/modding/src/how-to/native-mods.md, declared in `hooks/`
/// with the game's): a sound that is not streamed has started on `channel`
/// (`PlaySoundSourceEvent`), after the engine set its pitch, volume and position.
#[inline(never)]
pub fn native_play_sound_source(
    context: &sunrise_world::native_load_context::LoopContext,
    sound: &SoundPlay,
    channel: &ChannelRef,
) {
    if !mod_api::__private::listened(HookId::PlaySoundSource) {
        return;
    }
    mod_api::dispatch_play_sound_source(
        &crate::api_context::LoopReader(context).context(),
        sound,
        channel,
    );
}

/// Hook point for native mods (docs/modding/src/how-to/native-mods.md, declared in `hooks/`
/// with the game's): a streamed sound (music, a record) has started on `channel`
/// (`PlayStreamingSourceEvent`).
#[inline(never)]
pub fn native_play_streaming_source(
    context: &sunrise_world::native_load_context::LoopContext,
    sound: &SoundPlay,
    channel: &ChannelRef,
) {
    if !mod_api::__private::listened(HookId::PlayStreamingSource) {
        return;
    }
    mod_api::dispatch_play_streaming_source(
        &crate::api_context::LoopReader(context).context(),
        sound,
        channel,
    );
}

/// Hook point for native mods (docs/modding/src/how-to/native-mods.md, declared in `hooks/`
/// with the game's): the sound engine opened its output, at window start, after a resource
/// reload and when the output device changes (`SoundEngineLoadEvent`).
#[inline(never)]
pub fn native_sound_engine_load(context: &sunrise_world::native_load_context::LoopContext) {
    if !mod_api::__private::listened(HookId::SoundEngineLoad) {
        return;
    }
    mod_api::dispatch_sound_engine_load(&crate::api_context::LoopReader(context).context());
}

// Validate a mod's changed parameters at the playback boundary, leaving vanilla
// inputs alone. Starts and refreshes use the same contract.
fn valid_playback_edit(original: &SoundPlay, edited: &SoundPlay) -> bool {
    super::readable_base_edit(
        [original.volume, original.pitch],
        [edited.volume, edited.pitch],
    ) && (edited.effects == original.effects || edited.effects.valid())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingPlayKind {
    Start,
    CategoryRefresh { handle: u64 },
    TickRefresh { handle: u64, tick_count: i64 },
}

/// A staged `play_sound` decision produced by `SoundEngine` (decide) for `Game` to dispatch
/// with its scoped `ClientHost` (fire) and hand back to `SoundEngine::apply_play_sound` (apply).
#[derive(Clone, Debug)]
pub struct PendingPlaySound {
    pub instance: InstanceId,
    kind: PendingPlayKind,
    pub original: SoundPlay,
}

impl PendingPlaySound {
    /// True for a start or loop replay, false for a live channel refresh.
    pub fn starting(&self) -> bool {
        matches!(self.kind, PendingPlayKind::Start)
    }
}

impl SoundInstance {
    pub(super) fn play_settings(&self) -> SoundPlay {
        let mut sound = SoundPlay::ui("");
        self.write_play_settings(&mut sound);
        sound
    }
    fn write_play_settings(&self, sound: &mut SoundPlay) {
        sound.identifier.clone_from(&self.identifier);
        sound.source = SoundSource::ALL[self.source];
        sound.volume = self.volume;
        sound.pitch = self.pitch;
        sound.position = self.position;
        sound.looping = self.looping;
        sound.delay = self.delay;
        sound.attenuation = match self.attenuation {
            Attenuation::None => SoundAttenuation::None,
            Attenuation::Linear => SoundAttenuation::Linear,
        };
        sound.relative = self.relative;
        sound.effects = self.effects;
    }

    /// A sound a mod asks for (`ClientHost::play_local_sound`), through the same mapping
    /// as a `play_sound` edit.
    pub fn from_play(sound: &SoundPlay, random: sunrise_core::random::LegacyRandom) -> Self {
        let mut instance = Self::new(
            &sound.identifier,
            0,
            1.0,
            1.0,
            random,
            false,
            0,
            Attenuation::None,
            [0.0; 3],
            true,
        );
        instance.apply_play_settings(sound.clone());
        instance
    }

    /// Take a mod's edits of [`Self::play_settings`]. A new identifier is resolved again
    /// at the play, as every instance is, even one whose sound a caller chose.
    fn apply_play_settings(&mut self, edited: SoundPlay) {
        if edited.identifier != self.identifier {
            self.identifier = edited.identifier;
            self.chosen = false;
        }
        self.source = edited.source as usize;
        self.volume = edited.volume;
        self.pitch = edited.pitch;
        self.position = edited.position;
        self.looping = edited.looping;
        self.delay = edited.delay;
        self.attenuation = match edited.attenuation {
            SoundAttenuation::None => Attenuation::None,
            SoundAttenuation::Linear => Attenuation::Linear,
            _ => self.attenuation,
        };
        self.relative = edited.relative;
        self.effects = edited.effects;
    }
}

impl SoundEngine {
    /// The loop context of this client's current resource generation.
    pub fn loop_context(&self) -> Option<&sunrise_world::native_load_context::LoopContext> {
        self.loop_context.as_ref()
    }

    /// Replace the loop context when a resource generation starts or reloads.
    pub fn set_loop_context(&mut self, context: sunrise_world::native_load_context::LoopContext) {
        self.loop_context = Some(context);
    }

    /// Enable or disable staging `play_sound` decisions for `Game` to dispatch with `ClientHost`.
    /// Only client games enable this; servers never stage `play_sound`.
    pub fn set_play_sound_hooks(&mut self, enabled: bool) {
        self.play_sound_hooks = enabled;
    }

    /// Whether `play_sound` decisions should be staged right now.
    /// Free when no mod listens or on a server.
    pub fn should_stage_play_sound(&self) -> bool {
        self.play_sound_hooks && mod_api::__private::listened(HookId::PlaySound)
    }

    /// True while staged `play_sound` decisions are waiting for `Game` to fire and apply.
    pub fn has_pending_play_sound(&self) -> bool {
        !self.pending_play_sound.is_empty()
    }

    /// Pop the next staged `play_sound` decision in FIFO order.
    pub fn next_pending_play_sound(&mut self) -> Option<PendingPlaySound> {
        self.pending_play_sound.pop_front()
    }

    pub(super) fn stage_start(&mut self, id: InstanceId) {
        let Some(instance) = self.instances.get(&id) else {
            return;
        };
        if !self.pending_starts.insert(id) {
            return;
        }
        self.pending_play_sound.push_back(PendingPlaySound {
            instance: id,
            kind: PendingPlayKind::Start,
            original: instance.play_settings(),
        });
    }

    pub(super) fn stage_category_refresh(&mut self, id: InstanceId, handle: u64) {
        let Some(instance) = self.instances.get(&id) else {
            return;
        };
        self.pending_play_sound.push_back(PendingPlaySound {
            instance: id,
            kind: PendingPlayKind::CategoryRefresh { handle },
            original: instance.play_settings(),
        });
    }

    pub(super) fn stage_tick_refresh(&mut self, id: InstanceId, handle: u64, tick_count: i64) {
        let Some(instance) = self.instances.get(&id) else {
            return;
        };
        self.pending_play_sound.push_back(PendingPlaySound {
            instance: id,
            kind: PendingPlayKind::TickRefresh { handle, tick_count },
            original: instance.play_settings(),
        });
    }

    /// Apply a `play_sound` listener decision back to the same instance, re-checking that it
    /// still exists and matches its staged state (waiting to start, or still active on the same
    /// channel for a live refresh). Otherwise refuse.
    pub fn apply_play_sound(
        &mut self,
        pending: PendingPlaySound,
        decision: Option<SoundPlay>,
        backend: &mut impl Backend,
    ) -> PlayResult {
        match pending.kind {
            PendingPlayKind::Start => {
                if !self.pending_starts.remove(&pending.instance) {
                    self.collect(pending.instance);
                    return PlayResult::NotStarted;
                }
                let Some(instance) = self.instances.get(&pending.instance) else {
                    return PlayResult::NotStarted;
                };
                if instance.stopped {
                    self.collect(pending.instance);
                    return PlayResult::NotStarted;
                }
                let Some(edited) = decision else {
                    self.collect(pending.instance);
                    return PlayResult::NotStarted;
                };
                if !valid_playback_edit(&pending.original, &edited) {
                    self.collect(pending.instance);
                    return PlayResult::NotStarted;
                }
                if edited != pending.original {
                    self.instances
                        .get_mut(&pending.instance)
                        .expect("present")
                        .apply_play_settings(edited);
                }
                let result = self.play_after_hook(pending.instance, backend);
                self.collect(pending.instance);
                result
            }
            PendingPlayKind::CategoryRefresh { handle } => {
                if !self.loaded {
                    return PlayResult::NotStarted;
                }
                let Some(instance) = self.instances.get(&pending.instance) else {
                    return PlayResult::NotStarted;
                };
                if instance.stopped
                    || self.channel_of(pending.instance) != Some(handle)
                    || self.handle_stopped(handle)
                {
                    return PlayResult::NotStarted;
                }
                let Some(edited) = decision else {
                    self.cancel_playback(pending.instance, backend);
                    return PlayResult::NotStarted;
                };
                if !valid_playback_edit(&pending.original, &edited) {
                    self.cancel_playback(pending.instance, backend);
                    return PlayResult::NotStarted;
                }
                self.apply_category_refresh(
                    pending.instance,
                    handle,
                    (edited.volume, edited.pitch, edited.effects),
                    backend,
                );
                PlayResult::NotStarted
            }
            PendingPlayKind::TickRefresh { handle, tick_count } => {
                if !self.loaded {
                    return PlayResult::NotStarted;
                }
                let Some(instance) = self.instances.get(&pending.instance) else {
                    return PlayResult::NotStarted;
                };
                if instance.stopped
                    || self.channel_of(pending.instance) != Some(handle)
                    || self.handle_stopped(handle)
                {
                    return PlayResult::NotStarted;
                }
                let Some(edited) = decision else {
                    self.cancel_playback(pending.instance, backend);
                    return PlayResult::NotStarted;
                };
                if !valid_playback_edit(&pending.original, &edited) {
                    self.cancel_playback(pending.instance, backend);
                    return PlayResult::NotStarted;
                }
                self.apply_tick_refresh(
                    pending.instance,
                    handle,
                    tick_count,
                    (edited.volume, edited.pitch, edited.effects),
                    backend,
                );
                PlayResult::NotStarted
            }
        }
    }

    /// `play_sound_source` or `play_streaming_source`, for a sound that just started.
    pub(super) fn notify_source_started(
        &self,
        id: super::InstanceId,
        channel: ChannelId,
        streaming: bool,
    ) {
        let Some(source) = &self.loop_context else {
            return;
        };
        let hook = if streaming {
            HookId::PlayStreamingSource
        } else {
            HookId::PlaySoundSource
        };
        if !mod_api::__private::listened(hook) {
            return;
        }
        let Some(instance) = self.instances.get(&id) else {
            return;
        };
        let sound = instance.play_settings();
        let channel = built::ChannelRef { id: channel }.into();
        if streaming {
            native_play_streaming_source(source, &sound, &channel);
        } else {
            native_play_sound_source(source, &sound, &channel);
        }
    }

    /// `sound_engine_load`, once the output is open and the context is current.
    pub fn notify_loaded(&self) {
        let Some(source) = &self.loop_context else {
            return;
        };
        if self.loaded && mod_api::__private::listened(HookId::SoundEngineLoad) {
            native_sound_engine_load(source);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound_engine::test_backend::Null;
    use crate::sound_events::SoundRegistry;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use sunrise_core::random::LegacyRandom;
    use sunrise_world::native_load_context::{DataLoadOrigin, LoadContext, LoadInputs, LoopSide};

    /// What the mod heard, in order.
    type Log = Arc<Mutex<Vec<String>>>;

    /// Registers a `play_sound` subscription (so `SoundEngine` stages decisions) and logs the
    /// three engine-loop notices that still fire directly with `LoopContext`.
    struct SourceObserver(Log);

    #[mod_api::__subscriptions]
    impl mod_api::Hooks for SourceObserver {
        fn play_sound(
            &self,
            _host: &mut dyn mod_api::ClientHost,
            _event: &mut mod_api::PlaySoundEvent<'_>,
        ) {
        }
        fn play_sound_source(
            &self,
            _context: &dyn mod_api::LoopContext,
            event: &mut mod_api::PlaySoundSourceEvent<'_>,
        ) {
            self.0.lock().unwrap().push(format!(
                "source {} {}",
                event.sound.identifier, event.sound.volume
            ));
        }
        fn play_streaming_source(
            &self,
            _context: &dyn mod_api::LoopContext,
            event: &mut mod_api::PlayStreamingSourceEvent<'_>,
        ) {
            self.0
                .lock()
                .unwrap()
                .push(format!("stream {}", event.sound.identifier));
        }
        fn sound_engine_load(
            &self,
            context: &dyn mod_api::LoopContext,
            _event: &mut mod_api::SoundEngineLoadEvent,
        ) {
            self.0
                .lock()
                .unwrap()
                .push(format!("load {:?}", context.side()));
        }
    }

    struct PlaySoundSub;

    #[mod_api::__subscriptions]
    impl mod_api::Hooks for PlaySoundSub {
        fn play_sound(
            &self,
            _host: &mut dyn mod_api::ClientHost,
            _event: &mut mod_api::PlaySoundEvent<'_>,
        ) {
        }
    }

    fn engine() -> SoundEngine {
        let mut registry = SoundRegistry::default();
        registry.add_file(
            "minecraft",
            br#"{"block.mute": {"sounds": ["a"]}, "block.quiet": {"sounds": ["a"]},
                 "block.plain": {"sounds": ["a"]}, "music.menu": {"sounds": [{"name": "m", "stream": true}]}}"#,
            &|_, _| true,
        );
        let mut engine = SoundEngine::new(registry);
        let inputs = Arc::new(LoadInputs::new(BTreeMap::new(), std::env::temp_dir()));
        let context = LoadContext::new(DataLoadOrigin::ClientPreload, inputs, []);
        engine.set_loop_context(context.loop_context(LoopSide::Client, []));
        engine.set_play_sound_hooks(true);
        engine
    }

    fn drain(
        engine: &mut SoundEngine,
        backend: &mut Null,
        mut decide: impl FnMut(u64, bool, &SoundPlay, &mut Option<SoundPlay>),
    ) -> Option<PlayResult> {
        let mut last = None;
        while let Some(pending) = engine.next_pending_play_sound() {
            let mut sound = Some(pending.original.clone());
            decide(
                pending.instance,
                pending.starting(),
                &pending.original,
                &mut sound,
            );
            last = Some(engine.apply_play_sound(pending, sound, backend));
        }
        last
    }

    fn play_with(
        engine: &mut SoundEngine,
        backend: &mut Null,
        name: &str,
        decide: impl FnMut(u64, bool, &SoundPlay, &mut Option<SoundPlay>),
    ) -> PlayResult {
        let instance = SoundInstance::for_ui(name, 1.0, 1.0, LegacyRandom::new(1));
        let (_, immediate) = engine.play_new(instance, false, backend);
        drain(engine, backend, decide).unwrap_or(immediate)
    }

    #[test]
    fn a_mod_mutes_retunes_and_hears_the_sources_start() {
        let log: Log = Log::default();
        let mods: Vec<mod_api::ModEntry> = vec![mod_api::ModEntry::new(
            "tuner",
            Box::new(SourceObserver(log.clone())),
        )];
        let tuner = |_: u64, _: bool, original: &SoundPlay, sound: &mut Option<SoundPlay>| {
            log.lock()
                .unwrap()
                .push(format!("play {} {}", original.identifier, original.volume));
            match original.identifier.as_str() {
                "minecraft:block.mute" => *sound = None,
                "minecraft:block.quiet" => {
                    if let Some(s) = sound.as_mut() {
                        s.volume = 0.5;
                        s.pitch = 2.0;
                    }
                }
                _ => {}
            }
        };
        mod_platform::testing::with_mods(mods.into(), || {
            let (mut engine, mut backend) = (engine(), Null::default());
            engine.notify_loaded();
            assert_eq!(
                play_with(&mut engine, &mut backend, "minecraft:block.mute", tuner),
                PlayResult::NotStarted
            );
            assert!(backend.volume.is_empty(), "a muted sound takes no channel");
            play_with(&mut engine, &mut backend, "minecraft:block.quiet", tuner);
            assert_eq!(backend.volume.values().copied().collect::<Vec<_>>(), [0.5]);
            play_with(&mut engine, &mut backend, "minecraft:block.plain", tuner);
            play_with(&mut engine, &mut backend, "minecraft:music.menu", tuner);
        });
        assert_eq!(
            *log.lock().unwrap(),
            [
                "load Client",
                "play minecraft:block.mute 1",
                "play minecraft:block.quiet 1",
                "source minecraft:block.quiet 0.5",
                "play minecraft:block.plain 1",
                "source minecraft:block.plain 1",
                "play minecraft:music.menu 1",
                "stream minecraft:music.menu",
            ]
        );
    }

    #[test]
    fn with_no_listener_a_sound_plays_as_asked() {
        let (mut engine, mut backend) = (engine(), Null::default());
        engine.notify_loaded();
        let instance =
            SoundInstance::for_ui("minecraft:block.mute", 1.0, 1.0, LegacyRandom::new(1));
        assert_eq!(
            engine.play_new(instance, false, &mut backend).1,
            PlayResult::Started
        );
        assert!(!engine.has_pending_play_sound());
    }

    #[test]
    fn releasing_an_instance_between_decide_and_apply_refuses_it() {
        let mods = vec![mod_api::ModEntry::new("sub", Box::new(PlaySoundSub))];
        mod_platform::testing::with_mods(mods.into(), || {
            let (mut engine, mut backend) = (engine(), Null::default());
            let instance =
                SoundInstance::for_ui("minecraft:block.plain", 1.0, 1.0, LegacyRandom::new(1));
            let (id, staged) = engine.play_new(instance, true, &mut backend);
            assert_eq!(staged, PlayResult::NotStarted);
            assert!(backend.volume.is_empty());
            let pending = engine.next_pending_play_sound().expect("staged start");
            assert_eq!(pending.instance, id);
            assert!(pending.starting());
            engine.release(id);
            let decision = Some(pending.original.clone());
            assert_eq!(
                engine.apply_play_sound(pending, decision, &mut backend),
                PlayResult::NotStarted
            );
            assert!(backend.volume.is_empty());
            assert!(engine.instance(id).is_none());
        });
    }

    #[test]
    fn live_play_sound_edits_reach_the_channel_without_changing_the_instance() {
        let mods = vec![mod_api::ModEntry::new("sub", Box::new(PlaySoundSub))];
        mod_platform::testing::with_mods(mods.into(), || {
            let (mut engine, mut backend) = (engine(), Null::default());
            let mut n = 0usize;
            let mut seen_id = 0u64;
            let mut decide =
                |id: u64, starting: bool, _: &SoundPlay, sound: &mut Option<SoundPlay>| {
                    assert_eq!(starting, n == 0 || n == 3);
                    if n == 0 {
                        seen_id = id;
                    } else {
                        assert_eq!(id, seen_id);
                    }
                    if n == 1 {
                        let s = sound.as_mut().unwrap();
                        s.volume = 0.25;
                        s.pitch = 2.0;
                    } else if n == 2 {
                        *sound = None;
                    }
                    n += 1;
                };
            let instance =
                SoundInstance::for_ui("minecraft:block.plain", 1.0, 1.0, LegacyRandom::new(1));
            let (id, _) = engine.play_new(instance, true, &mut backend);
            let result = drain(&mut engine, &mut backend, &mut decide).unwrap();
            assert_eq!(result, PlayResult::Started);
            let channel = engine.channel(id).unwrap();
            engine.refresh_category_volume(crate::sound_engine::MASTER, &mut backend);
            drain(&mut engine, &mut backend, &mut decide);
            assert_eq!(backend.volume[&channel], 0.25);
            assert_eq!(backend.pitch[&channel], 2.0);
            assert_eq!(engine.instance(id).unwrap().volume, 1.0);
            assert_eq!(engine.instance(id).unwrap().pitch, 1.0);
            engine.refresh_category_volume(crate::sound_engine::MASTER, &mut backend);
            drain(&mut engine, &mut backend, &mut decide);
            assert!(engine.instance(id).unwrap().stopped);
            engine.instance_mut(id).unwrap().stopped = false;
            engine.play(id, &mut backend);
            let result = drain(&mut engine, &mut backend, &mut decide).unwrap();
            assert_eq!(result, PlayResult::Started);
            engine.refresh_category_volume(crate::sound_engine::MASTER, &mut backend);
            drain(&mut engine, &mut backend, &mut decide);
            assert!(
                !engine.instance(id).unwrap().stopped,
                "a replay resets a cancelled live edit"
            );
        });
    }

    #[test]
    fn a_mods_effects_reach_the_channel_and_out_of_range_ones_play_nothing() {
        let refused: [fn(&mut mod_api::SoundEffects); 8] = [
            |e| e.gain = f32::NAN,
            |e| e.direct = 1.5,
            |e| e.lowpass = f32::INFINITY,
            |e| e.highpass = -1.0,
            |e| e.room = f32::NEG_INFINITY,
            |e| e.hall = -f32::NAN,
            |e| e.cavern = 1.0001,
            |e| e.reverb_lowpass = 20_001.0,
        ];
        let mods = vec![mod_api::ModEntry::new("sub", Box::new(PlaySoundSub))];
        mod_platform::testing::with_mods(mods.into(), || {
            for edit in refused {
                let (mut engine, mut backend) = (engine(), Null::default());
                assert_eq!(
                    play_with(
                        &mut engine,
                        &mut backend,
                        "minecraft:block.plain",
                        |_, _, _, s| {
                            edit(&mut s.as_mut().unwrap().effects);
                        }
                    ),
                    PlayResult::NotStarted
                );
                assert!(backend.effects.is_empty() && backend.volume.is_empty());
            }
            let shaper = |_: u64, _: bool, original: &SoundPlay, sound: &mut Option<SoundPlay>| {
                let sound = sound.as_mut().unwrap();
                match original.identifier.as_str() {
                    "minecraft:block.quiet" => {
                        sound.effects.lowpass = 600.0;
                        sound.effects.cavern = 0.5;
                    }
                    "minecraft:block.mute" => sound.effects.lowpass = 5.0,
                    _ => {}
                }
            };
            let (mut engine, mut backend) = (engine(), Null::default());
            assert_eq!(
                play_with(&mut engine, &mut backend, "minecraft:block.quiet", shaper),
                PlayResult::Started
            );
            let effects = backend.effects.values().copied().collect::<Vec<_>>();
            assert_eq!(effects.len(), 1);
            assert_eq!((effects[0].lowpass, effects[0].cavern), (600.0, 0.5));
            play_with(&mut engine, &mut backend, "minecraft:block.plain", shaper);
            assert!(
                backend
                    .effects
                    .values()
                    .any(|e| *e == mod_api::SoundEffects::NONE)
            );
            assert_eq!(
                play_with(&mut engine, &mut backend, "minecraft:block.mute", shaper),
                PlayResult::NotStarted
            );
        });
    }

    #[test]
    fn a_ticking_sounds_live_refresh_moves_its_effects_and_a_refused_value_stops_it() {
        let mods = vec![mod_api::ModEntry::new("sub", Box::new(PlaySoundSub))];
        mod_platform::testing::with_mods(mods.into(), || {
            for refused in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 10.0, 30_000.0] {
                let (mut engine, mut backend) = (engine(), Null::default());
                let mut instance =
                    SoundInstance::for_ui("minecraft:block.plain", 1.0, 1.0, LegacyRandom::new(1));
                instance.ticking = crate::sound_engine::Ticking::Native;
                let (id, _) = engine.play_new(instance, true, &mut backend);
                let result = drain(&mut engine, &mut backend, |_, _, _, _| {}).unwrap();
                assert_eq!(result, PlayResult::Started);
                let channel = engine.channel(id).unwrap();
                assert_eq!(backend.effects[&channel], mod_api::SoundEffects::NONE);
                let ctx = crate::sound_engine::TickContext::default();
                engine.tick(false, &ctx, &mut backend);
                drain(&mut engine, &mut backend, |_, starting, _, sound| {
                    if !starting {
                        sound.as_mut().unwrap().effects.lowpass = 900.0;
                    }
                });
                assert_eq!(backend.effects[&channel].lowpass, 900.0);
                engine.tick(false, &ctx, &mut backend);
                drain(&mut engine, &mut backend, |_, starting, _, sound| {
                    if !starting {
                        sound.as_mut().unwrap().effects.lowpass = refused;
                    }
                });
                // Refused, not clamped: the channel keeps what it had and stops.
                assert_eq!(backend.effects[&channel].lowpass, 900.0);
                assert!(engine.instance(id).unwrap().stopped);
            }
        });
    }

    #[test]
    fn finite_negative_play_sound_edits_reach_the_local_sound_mixer() {
        use crate::sound_engine::{MASTER, TickContext, Ticking};
        let mods = vec![mod_api::ModEntry::new("sub", Box::new(PlaySoundSub))];
        mod_platform::testing::with_mods(mods.into(), || {
            for (at_start, category_refresh) in [(true, false), (false, false), (false, true)] {
                let (mut engine, mut backend) = (engine(), Null::default());
                let play = SoundPlay::ui("minecraft:block.plain");
                let mut instance = SoundInstance::from_play(&play, LegacyRandom::new(1));
                instance.ticking = Ticking::Mod { fade: None };
                instance.can_start_silent = true;
                let (id, _) = engine.play_new(instance, true, &mut backend);
                let edit =
                    |_: u64, starting: bool, _: &SoundPlay, sound: &mut Option<SoundPlay>| {
                        if starting == at_start {
                            let s = sound.as_mut().unwrap();
                            s.volume = -0.5;
                            s.pitch = -1.0;
                        }
                    };
                let result = drain(&mut engine, &mut backend, edit).unwrap();
                let channel = engine.channel(id).unwrap();
                if at_start {
                    assert_eq!(result, PlayResult::StartedSilently);
                } else {
                    assert_eq!(result, PlayResult::Started);
                    assert_eq!(
                        (backend.volume[&channel], backend.pitch[&channel]),
                        (1.0, 1.0)
                    );
                    if category_refresh {
                        engine.refresh_category_volume(MASTER, &mut backend);
                    } else {
                        engine.tick(false, &TickContext::default(), &mut backend);
                    }
                    drain(&mut engine, &mut backend, edit);
                    let instance = engine.instance(id).unwrap();
                    assert!(!instance.stopped);
                    assert!(!backend.stopped[&channel]);
                    assert_eq!((instance.volume, instance.pitch), (1.0, 1.0));
                }
                assert_eq!(
                    (backend.volume[&channel], backend.pitch[&channel]),
                    (0.0, 0.5)
                );
            }
        });
    }

    #[test]
    fn invalid_play_sound_edits_refuse_starts_and_stop_live_channels() {
        let mods = vec![mod_api::ModEntry::new("sub", Box::new(PlaySoundSub))];
        mod_platform::testing::with_mods(mods.into(), || {
            for at_start in [true, false] {
                let (mut engine, mut backend) = (engine(), Null::default());
                let instance =
                    SoundInstance::for_ui("minecraft:block.plain", 1.0, 1.0, LegacyRandom::new(1));
                let (id, _) = engine.play_new(instance, true, &mut backend);
                let poison =
                    |_: u64, starting: bool, _: &SoundPlay, sound: &mut Option<SoundPlay>| {
                        if starting == at_start {
                            sound.as_mut().unwrap().volume = f32::NAN;
                        }
                    };
                let result = drain(&mut engine, &mut backend, poison).unwrap();
                if at_start {
                    assert_eq!(result, PlayResult::NotStarted);
                    assert!(backend.volume.is_empty());
                } else {
                    assert_eq!(result, PlayResult::Started);
                    engine.refresh_category_volume(crate::sound_engine::MASTER, &mut backend);
                    drain(&mut engine, &mut backend, poison);
                    assert!(engine.instance(id).unwrap().stopped);
                }
            }
        });
    }
}
