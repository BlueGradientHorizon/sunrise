//! One lazy PCM producer for all native pull streams on a shared output.
use super::{Control, Ring};
use crate::crash::contain;
use mod_api::{
    __private::{mod_index, mod_index_on},
    AUDIO_STREAM_RATE_MAX, AUDIO_STREAM_RING_MAX_MS, AUDIO_STREAMS_TOTAL, AudioRead, AudioRefusal,
    AudioStream,
    content::crash::{ModCall, enter_native_mod},
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering::*},
        mpsc::{self, Receiver, Sender, SyncSender, TryRecvError, TrySendError},
    },
    thread::{self, Thread},
};

const CAPACITY: usize = AUDIO_STREAMS_TOTAL as usize;
const SCRATCH_SAMPLES: usize =
    (AUDIO_STREAM_RATE_MAX * AUDIO_STREAM_RING_MAX_MS / 1000 * 2) as usize;

/// Pair the thread's park token with a flag: source code runs on this thread and may use
/// standard synchronization that consumes the token before we get back to the worker loop.
#[derive(Clone)]
pub(super) struct Wake {
    pending: Arc<AtomicBool>,
    thread: Thread,
}

impl Wake {
    pub(super) fn signal(&self) {
        if !self.pending.swap(true, Release) {
            self.thread.unpark();
        }
    }
}

pub(super) struct Demand {
    pub(super) wake: Wake,
    /// The desired number of queued frames, or zero when no request is pending. The latest
    /// output buffer replaces an older request rather than accumulating a backlog.
    pub(super) frames: AtomicUsize,
    pub(super) eof: AtomicBool,
}

/// Holds the only job sender. Disconnecting it and waking the worker makes teardown independent
/// of the next device callback. Live voices and handles have only the wake token.
pub(super) struct Worker {
    sender: SyncSender<PullJob>,
    wake: Wake,
    owners: HashMap<&'static str, Arc<PullOwner>>,
    panic_sender: Sender<&'static str>,
    panics: Receiver<&'static str>,
}

impl Worker {
    pub(super) fn start() -> Result<Self, AudioRefusal> {
        let (sender, receiver) = mpsc::sync_channel(CAPACITY);
        let pending = Arc::new(AtomicBool::new(false));
        let signaled = pending.clone();
        let worker = thread::Builder::new()
            .name("audio-pull".into())
            .spawn(move || {
                let mut jobs = Vec::<PullJob>::with_capacity(CAPACITY);
                let mut scratch = vec![0; SCRATCH_SAMPLES];
                loop {
                    while jobs.len() < CAPACITY {
                        match receiver.try_recv() {
                            Ok(job) => jobs.push(job),
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => return,
                        }
                    }
                    let was_full = jobs.len() == CAPACITY;
                    jobs.retain_mut(|job| job.refill(&mut scratch));
                    // If room freed up without polling the whole queue, check it again. Otherwise
                    // park even if a read was short. A wake sent during this pass is not lost.
                    if !was_full || jobs.len() == CAPACITY {
                        while !signaled.swap(false, Acquire) {
                            thread::park();
                        }
                    }
                }
            })
            .map_err(|_| AudioRefusal::WorkerUnavailable)?;
        Ok(Self::with_queue(
            sender,
            Wake {
                pending,
                thread: worker.thread().clone(),
            },
        ))
    }

    fn with_queue(sender: SyncSender<PullJob>, wake: Wake) -> Self {
        let (panic_sender, panics) = mpsc::channel();
        Self {
            sender,
            wake,
            owners: HashMap::new(),
            panic_sender,
            panics,
        }
    }

    pub(super) fn owner(&mut self, id: &'static str) -> Arc<PullOwner> {
        let panics = &self.panic_sender;
        let wake = &self.wake;
        self.owners
            .entry(id)
            .or_insert_with(|| {
                Arc::new(PullOwner {
                    id,
                    index: mod_index(id),
                    poisoned: AtomicBool::new(false),
                    wake: wake.clone(),
                    panics: panics.clone(),
                })
            })
            .clone()
    }

    pub(super) fn take_panics(&self) -> Vec<&'static str> {
        self.panics.try_iter().collect()
    }

    pub(super) fn demand(&self) -> Demand {
        Demand {
            wake: self.wake.clone(),
            frames: AtomicUsize::new(0),
            eof: AtomicBool::new(false),
        }
    }

    /// Returns ownership on admission failure so the caller can drop the source after
    /// releasing its guard.
    pub(super) fn admit(&self, job: PullJob) -> Result<(), PullJob> {
        match self.sender.try_send(job) {
            Ok(()) => {
                self.wake();
                Ok(())
            }
            Err(TrySendError::Full(job) | TrySendError::Disconnected(job)) => Err(job),
        }
    }

    pub(super) fn wake(&self) {
        self.wake.signal();
    }

    pub(super) fn shutdown(self) {
        drop(self.sender);
        self.wake.signal();
    }
}

/// All pull sources from one mod share a session poison, separate from the mod's other callbacks.
pub(super) struct PullOwner {
    id: &'static str,
    index: Option<usize>,
    poisoned: AtomicBool,
    wake: Wake,
    panics: Sender<&'static str>,
}

impl PullOwner {
    pub(super) fn pull_disabled(&self) -> bool {
        self.poisoned.load(Acquire)
    }

    fn enabled(&self) -> bool {
        !self.pull_disabled() && self.index.is_none_or(mod_index_on)
    }

    /// Destruction runs mod code even for a disabled owner. True if it completed normally.
    pub(super) fn drop_source(&self, source: Box<dyn AudioStream>) -> bool {
        self.call("AudioStream drop", || drop(source)).is_ok()
    }

    fn call<T>(&self, hook: &'static str, call: impl FnOnce() -> T) -> Result<T, String> {
        let id = self.id;
        let _guard = enter_native_mod(ModCall {
            id,
            hook,
            part: None,
        });
        let result = contain(call);
        match &result {
            Ok(_) => {}
            Err(_) => {
                log::error!(
                    "mod {id}: {hook} panicked; audio stream reads are disabled for this session"
                );
                self.poisoned.store(true, Release);
                // A rejected job can stop and wake the worker before its source panics on
                // the opening thread. Wake again after publishing the poison for live siblings.
                self.wake.signal();
                // If the output has been destroyed, the log above still reports the panic.
                let _ = self.panics.send(id);
            }
        }
        result
    }
}

pub(super) struct PullJob {
    pub(super) control: Arc<Control>,
    pub(super) demand: Arc<Demand>,
    pub(super) ring: Arc<Ring>,
    pub(super) owner: Arc<PullOwner>,
    pub(super) source: Option<Box<dyn AudioStream>>,
}

impl PullJob {
    /// At most one read per pass, using only the missing whole frames. False retires the job
    /// and its source on the worker; only the mixer decides when an EOF voice has drained.
    fn refill(&mut self, scratch: &mut [i16]) -> bool {
        if self.control.closed.load(Acquire) || !self.owner.enabled() {
            return false;
        }
        let target = self.demand.frames.swap(0, AcqRel);
        if target == 0 {
            return true;
        }
        let (tail, queued, free) = self.ring.producer_window();
        let missing = target.saturating_sub(queued).min(free);
        if missing == 0 {
            return true;
        }
        let channels = self.ring.channels;
        let pcm = &mut scratch[..missing * channels];
        pcm.fill(0);
        let source = self.source.as_mut().expect("live pull job owns its source");
        match self.owner.call("AudioStream::read", || source.read(pcm)) {
            Ok(AudioRead::Frames(frames)) if frames > missing => {
                log::warn!(
                    "mod {}: AudioStream::read returned {frames} frames with room for {missing}; stopping this stream",
                    self.owner.id,
                );
                false
            }
            Err(_) => false,
            // A rejected source can poison the owner on the opening thread during this read.
            _ if !self.owner.enabled() => false,
            Ok(AudioRead::Frames(0)) => true,
            Ok(AudioRead::Frames(frames)) => {
                self.ring.publish_frames(tail, &pcm[..frames * channels]);
                true
            }
            Ok(AudioRead::Eof) => {
                let source = self.source.take().expect("live pull job owns its source");
                // Destruction is still mod code. Only commit normal EOF retirement after it
                // succeeds, at the last enabled check before publishing EOF to the mixer.
                if self.owner.drop_source(source) && self.owner.enabled() {
                    self.demand.eof.store(true, Release);
                }
                false
            }
        }
    }
}

impl Drop for PullJob {
    fn drop(&mut self) {
        // A job that leaves without EOF cannot produce again. Only an EOF voice drains.
        if !self.demand.eof.load(Relaxed) {
            self.control.stop();
        }
        if let Some(source) = self.source.take() {
            self.owner.drop_source(source);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{Audio, Mixer, mix_pieces};
    use super::super::{StreamPool, request_pull};
    use super::*;
    use mod_api::{AudioStreamSource, AudioStreamState, SoundSource, StreamSpec};
    use std::{sync::Mutex, thread::ThreadId, time::Duration};

    struct Source<F>(F);

    impl<F: FnMut(&mut [i16]) -> AudioRead + Send + 'static> AudioStream for Source<F> {
        fn read(&mut self, pcm: &mut [i16]) -> AudioRead {
            (self.0)(pcm)
        }
    }

    /// Takes the admitted job instead of starting a thread, so each refill happens at a chosen
    /// output buffer boundary. Uses the same queue, refill and mixer as a live output.
    struct Rig {
        pool: StreamPool,
        mixer: Mutex<Mixer>,
        stream: Arc<dyn AudioStreamSource>,
        job: Option<PullJob>,
        admitted: Receiver<PullJob>,
        scratch: [i16; 16],
    }

    impl Rig {
        fn new(read: impl FnMut(&mut [i16]) -> AudioRead + Send + 'static) -> Self {
            mod_platform::__private::initialize().unwrap();
            let pool = StreamPool::default();
            let (sender, receiver) = mpsc::sync_channel(CAPACITY);
            pool.state().worker = Some(Worker::with_queue(
                sender,
                Wake {
                    pending: Arc::default(),
                    thread: thread::current(),
                },
            ));
            let mixer = Mutex::new(Mixer {
                stream_gain: [1.0; 11],
                ..Mixer::default()
            });
            let spec = StreamSpec::new(SoundSource::Voice, 2, 24_000);
            let stream = pool
                .open(&mixer, "mod", &spec, Some(Box::new(Source(read))))
                .unwrap();
            Self {
                pool,
                mixer,
                stream,
                job: Some(receiver.try_recv().unwrap()),
                admitted: receiver,
                scratch: [99; 16],
            }
        }

        fn play<const SAMPLES: usize>(&self) -> [f32; SAMPLES] {
            let mut out = [0.0; SAMPLES];
            mix_pieces(&mut self.mixer.lock().unwrap(), &mut out, 2, 48_000);
            out
        }

        fn refill(&mut self) -> bool {
            if self.job.as_mut().unwrap().refill(&mut self.scratch) {
                true
            } else {
                self.job = None;
                false
            }
        }
    }

    #[test]
    fn a_pull_worker_reads_and_drops_the_source_with_both_guards_held() {
        mod_platform::__private::initialize().unwrap();
        struct Probe {
            read: SyncSender<(ThreadId, usize)>,
            dropped: SyncSender<ThreadId>,
        }

        impl AudioStream for Probe {
            fn read(&mut self, pcm: &mut [i16]) -> AudioRead {
                pcm[0] = 16_384;
                let _ = self.read.try_send((thread::current().id(), pcm.len()));
                AudioRead::Frames(1)
            }
        }

        impl Drop for Probe {
            fn drop(&mut self) {
                let _ = self.dropped.try_send(thread::current().id());
            }
        }

        let (read, reads) = mpsc::sync_channel(1);
        let (dropped, drops) = mpsc::sync_channel(1);
        let pool = StreamPool::default();
        let mixer = Mutex::new(Mixer::default());
        let spec = StreamSpec::new(SoundSource::Voice, 2, 24_000);
        let stream = pool
            .open(
                &mixer,
                "mod",
                &spec,
                Some(Box::new(Probe { read, dropped })),
            )
            .unwrap();

        let _pool = pool.state();
        let mut mixer = mixer.lock().unwrap();
        let mut out = [0.0; 8];
        mix_pieces(&mut mixer, &mut out, 2, 48_000);
        assert_eq!(out, [0.0; 8]);

        let timeout = Duration::from_secs(1);
        let (worker, samples) = reads
            .recv_timeout(timeout)
            .expect("worker did not read PCM");
        assert_ne!(worker, thread::current().id());
        assert_eq!(samples, 6); // Four output frames need two stereo source frames plus lookahead.

        stream.stop();
        assert_eq!(stream.state(), AudioStreamState::Stopped);
        // Still holding both guards, with no further mix or reap call.
        assert_eq!(
            drops.recv_timeout(timeout).expect("source was not dropped"),
            worker
        );
    }

    #[test]
    fn pull_panic_retires_the_owner_without_stopping_the_shared_worker() {
        mod_platform::__private::initialize().unwrap();
        let mut audio = Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
            std::path::PathBuf::from("/nonexistent/sunrise-test-audio"),
        ));
        audio.backend_live_for_bench();
        let mixer = &audio.backend.mixer;
        mixer.lock().unwrap().stream_gain = [1.0; 11];
        let spec = StreamSpec::new(SoundSource::Voice, 2, 48_000);
        let (called, calls) = mpsc::sync_channel(2);
        let (release, released) = mpsc::channel();
        let broken_called = called.clone();
        let broken = audio
            .open_stream(
                "broken",
                &spec,
                Some(Box::new(Source(move |pcm: &mut [i16]| {
                    let _ = broken_called.try_send((
                        thread::current().id(),
                        mod_api::content::crash::current_native_call(),
                    ));
                    let _ = released.recv();
                    pcm.fill(i16::MAX);
                    panic!("broken pull callback");
                }))),
            )
            .unwrap();
        let sibling = audio
            .open_stream(
                "broken",
                &spec,
                Some(Box::new(Source(|_: &mut [i16]| {
                    panic!("a disabled owner's other stream must not be read");
                }))),
            )
            .unwrap();
        let good = audio
            .open_stream(
                "good",
                &spec,
                Some(Box::new(Source(move |pcm: &mut [i16]| {
                    pcm.fill(16_384);
                    let _ = called.try_send((
                        thread::current().id(),
                        mod_api::content::crash::current_native_call(),
                    ));
                    AudioRead::Frames(pcm.len() / 2)
                }))),
            )
            .unwrap();
        // An admission wake may already be scanning the jobs. Only the failing source has
        // demand until it has entered read, so its sibling cannot be the first to panic.
        request_pull(&mixer.lock().unwrap().streams[..1], 1, 48_000);
        let timeout = Duration::from_secs(1);
        let (worker, broken_call) = calls
            .recv_timeout(timeout)
            .expect("broken mod did not read");
        request_pull(&mixer.lock().unwrap().streams[1..], 1, 48_000);
        release.send(()).unwrap();
        let (good_worker, good_call) = calls
            .recv_timeout(timeout)
            .expect("other mod lost its worker");
        assert_eq!(good_worker, worker);
        for (call, id) in [(broken_call, "broken"), (good_call, "good")] {
            assert_eq!(
                call,
                Some(ModCall {
                    id,
                    hook: "AudioStream::read",
                    part: None
                })
            );
        }
        assert_eq!(audio.take_pull_panics(), ["broken"]);
        assert_eq!(broken.state(), AudioStreamState::Stopped);
        assert_eq!(sibling.state(), AudioStreamState::Stopped);
        assert_eq!(good.state(), AudioStreamState::Playing);
        assert!(matches!(
            audio.open_stream(
                "broken",
                &spec,
                Some(Box::new(Source(|_: &mut [i16]| AudioRead::Eof)))
            ),
            Err(AudioRefusal::PullDisabled)
        ));

        let (opened, reads) = mpsc::sync_channel(1);
        let _late = audio
            .open_stream(
                "late",
                &spec,
                Some(Box::new(Source(move |_: &mut [i16]| {
                    let _ = opened.try_send(thread::current().id());
                    AudioRead::Frames(0)
                }))),
            )
            .unwrap();
        request_pull(&mixer.lock().unwrap().streams, 1, 48_000);
        assert_eq!(
            reads
                .recv_timeout(timeout)
                .expect("new stream did not read"),
            worker
        );
        // The later job ran on the same worker, after it published the healthy mod's PCM.
        let mut out = [0.0; 2];
        mix_pieces(&mut mixer.lock().unwrap(), &mut out, 2, 48_000);
        assert_eq!(out, [0.5; 2]);
    }

    #[test]
    fn eof_pcm_still_drains_after_a_sibling_source_panics() {
        let mut first = true;
        let mut rig = Rig::new(move |pcm| {
            if std::mem::take(&mut first) {
                pcm[..4].fill(16_384);
                AudioRead::Frames(2)
            } else {
                AudioRead::Eof
            }
        });
        // Ask for one more source frame without consuming the frames already queued.
        request_pull(&rig.mixer.lock().unwrap().streams, 4, 48_000);
        assert!(rig.refill());
        request_pull(&rig.mixer.lock().unwrap().streams, 4, 48_000);
        assert!(!rig.refill());

        let spec = StreamSpec::new(SoundSource::Voice, 2, 24_000);
        let sibling = rig
            .pool
            .open(
                &rig.mixer,
                "mod",
                &spec,
                Some(Box::new(Source(|_: &mut [i16]| panic!("sibling read")))),
            )
            .unwrap();
        rig.job = Some(rig.admitted.try_recv().unwrap());
        request_pull(&rig.mixer.lock().unwrap().streams, 4, 48_000);
        assert!(!rig.refill());
        assert_eq!(rig.pool.take_pull_panics(), ["mod"]);
        assert_eq!(sibling.state(), AudioStreamState::Stopped);
        assert_eq!(rig.stream.state(), AudioStreamState::Playing);
        assert_eq!(rig.play::<8>(), [0.5; 8]);
        assert_eq!(rig.stream.state(), AudioStreamState::Stopped);
        assert_eq!(rig.play::<2>(), [0.0; 2]);
    }

    #[test]
    fn a_pull_job_retires_before_reading_when_its_mod_is_disabled() {
        struct TestMod;
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for TestMod {}

        mod_platform::testing::with_mods(
            vec![mod_api::ModEntry::new("mod", Box::new(TestMod))].into(),
            || {
                let mut rig = Rig::new(|_| panic!("the disabled mod must not read"));
                assert!(mod_api::__private::set_mod_enabled("mod", false));
                assert_eq!(rig.play::<4>(), [0.0; 4]);
                assert!(!rig.refill());
                assert_eq!(rig.stream.state(), AudioStreamState::Stopped);
                assert!(rig.pool.take_pull_panics().is_empty());
            },
        );
    }

    #[test]
    fn a_phase_refusal_leaves_the_source_available_for_later_demand() {
        let mut first = true;
        let mut rig = Rig::new(move |pcm| {
            if std::mem::take(&mut first) {
                mod_api::__private::refuse("test_read", mod_api::__private::ModPhase::Reloading);
            }
            pcm.fill(16_384);
            AudioRead::Frames(1)
        });
        assert_eq!(rig.play::<4>(), [0.0; 4]);
        assert!(rig.refill());
        assert_eq!(rig.play::<4>(), [0.0; 4]);
        assert!(rig.refill());
        assert_eq!(rig.play::<4>(), [0.5; 4]);
        assert!(rig.pool.take_pull_panics().is_empty());
    }

    #[test]
    fn a_rejected_job_contains_source_destruction_on_the_opening_thread() {
        use crate::crash::{contained_panic, tests::with_claimed_panics};

        struct Payload;
        impl Drop for Payload {
            fn drop(&mut self) {
                panic!("must not run opaque panic payload's destructor");
            }
        }
        struct SourceDrop;
        impl AudioStream for SourceDrop {
            fn read(&mut self, _: &mut [i16]) -> AudioRead {
                panic!("the queue refused this source")
            }
        }
        impl Drop for SourceDrop {
            fn drop(&mut self) {
                std::panic::panic_any(Payload);
            }
        }

        mod_platform::__private::initialize().unwrap();
        let pool = StreamPool::default();
        // This queue has a receiver but no available slot, so admit returns the job.
        let (sender, _receiver) = mpsc::sync_channel(0);
        pool.state().worker = Some(Worker::with_queue(
            sender,
            Wake {
                pending: Arc::default(),
                thread: thread::current(),
            },
        ));
        let mixer = Mutex::new(Mixer::default());
        let spec = StreamSpec::new(SoundSource::Voice, 2, 24_000);
        let recovered = Arc::new(AtomicUsize::new(0));
        let at_hook = recovered.clone();
        with_claimed_panics(
            move |info| {
                if !info.payload().is::<Payload>() {
                    return false;
                }
                if contained_panic(info) {
                    at_hook.fetch_add(1, Relaxed);
                }
                true
            },
            || {
                let open = || pool.open(&mixer, "mod", &spec, Some(Box::new(SourceDrop)));
                assert!(matches!(open(), Err(AudioRefusal::WorkerUnavailable)));
                assert!(matches!(open(), Err(AudioRefusal::PullDisabled)));
            },
        );
        assert_eq!(
            recovered.load(Relaxed),
            2,
            "both drops bypass the fatal hook"
        );
        assert_eq!(pool.take_pull_panics(), ["mod", "mod"]);
        assert!(mixer.lock().unwrap().streams.is_empty());
    }

    #[test]
    fn rejected_source_panic_retires_an_idle_sibling_without_more_output() {
        struct Rejected {
            demand: Arc<Demand>,
            resume: Sender<()>,
            reads: Receiver<()>,
        }

        impl AudioStream for Rejected {
            fn read(&mut self, _: &mut [i16]) -> AudioRead {
                AudioRead::Frames(0)
            }
        }

        impl Drop for Rejected {
            fn drop(&mut self) {
                // The rejection has already stopped its job and signaled the worker. Queue
                // one more sibling read without signaling, then let the first read return.
                self.demand.frames.store(1, Release);
                if self.resume.send(()).is_ok()
                    && self.reads.recv_timeout(Duration::from_secs(1)).is_ok()
                {
                    // The same job could only read again after consuming the pending wake.
                    // It is now out of demand and has already seen the owner as enabled.
                    panic!("rejected source destructor");
                }
            }
        }

        mod_platform::__private::initialize().unwrap();
        let pool = StreamPool::default();
        let mixer = Mutex::new(Mixer::default());
        let spec = StreamSpec::new(SoundSource::Voice, 2, 48_000);
        let (reading, reads) = mpsc::sync_channel(1);
        let (resume, resumed) = mpsc::channel();
        let (live, retired) = mpsc::channel::<()>();
        let mut first = true;
        let sibling = pool
            .open(
                &mixer,
                "wake_regression",
                &spec,
                Some(Box::new(Source(move |_: &mut [i16]| {
                    let _keep_alive = &live;
                    let _ = reading.try_send(());
                    if std::mem::take(&mut first) {
                        let _ = resumed.recv();
                    }
                    AudioRead::Frames(0)
                }))),
            )
            .unwrap();
        let timeout = Duration::from_secs(1);
        request_pull(&mixer.lock().unwrap().streams, 1, 48_000);
        reads.recv_timeout(timeout).expect("sibling did not read");

        // Force the next open to fail admission but keep the real worker's receiver connected.
        let (reject, _rejecting) = mpsc::sync_channel(0);
        let (demand, _live_sender) = {
            let mut state = pool.state();
            let demand = state.controls[0].pull.as_ref().unwrap().clone();
            let worker = state.worker.as_mut().unwrap();
            (demand, std::mem::replace(&mut worker.sender, reject))
        };
        let rejected = Rejected {
            demand,
            resume,
            reads,
        };
        assert!(matches!(
            pool.open(&mixer, "wake_regression", &spec, Some(Box::new(rejected))),
            Err(AudioRefusal::WorkerUnavailable)
        ));
        assert_eq!(pool.take_pull_panics(), ["wake_regression"]);
        assert_eq!(
            retired.recv_timeout(timeout),
            Err(mpsc::RecvTimeoutError::Disconnected),
            "the idle sibling was not dropped without another output callback"
        );
        assert_eq!(sibling.state(), AudioStreamState::Stopped);
    }

    #[test]
    fn rejected_source_panic_discards_in_flight_pcm_and_prevents_eof_drain() {
        struct PauseAtBoundary {
            waiting: Sender<()>,
            resume: Receiver<()>,
        }

        impl Drop for PauseAtBoundary {
            fn drop(&mut self) {
                if self.waiting.send(()).is_ok() {
                    let _ = self.resume.recv_timeout(Duration::from_secs(1));
                }
            }
        }

        struct Rejected;
        impl AudioStream for Rejected {
            fn read(&mut self, _: &mut [i16]) -> AudioRead {
                AudioRead::Frames(0)
            }
        }
        impl Drop for Rejected {
            fn drop(&mut self) {
                panic!("rejected source destructor");
            }
        }

        for (result, pause_in_read, boundary) in [
            (AudioRead::Frames(1), true, "PCM read"),
            (AudioRead::Eof, true, "EOF read"),
            (AudioRead::Eof, false, "EOF destruction"),
        ] {
            let (waiting, reached) = mpsc::channel();
            let (resume, released) = mpsc::channel();
            let mut pause = Some(PauseAtBoundary {
                waiting,
                resume: released,
            });
            let mut first = true;
            let mut rig = Rig::new(move |pcm| {
                if std::mem::take(&mut first) {
                    pcm[..4].fill(16_384);
                    return AudioRead::Frames(2);
                }
                if pause_in_read {
                    drop(pause.take());
                }
                if let AudioRead::Frames(frames) = result {
                    pcm[..frames * 2].fill(-8192);
                }
                result
            });
            // Queue two frames and ask for one more without ever running the mixer.
            request_pull(&rig.mixer.lock().unwrap().streams, 4, 48_000);
            assert!(rig.refill());
            request_pull(&rig.mixer.lock().unwrap().streams, 4, 48_000);

            // The original job is already admitted. A zero-slot queue rejects the next open.
            let (reject, _rejecting) = mpsc::sync_channel(0);
            rig.pool.state().worker.as_mut().unwrap().sender = reject;
            let mut job = rig.job.take().unwrap();
            let scratch = &mut rig.scratch;
            let (keep, tail) = thread::scope(|scope| {
                let worker = scope.spawn(move || {
                    let keep = job.refill(scratch);
                    (keep, job.ring.producer_window().0)
                });
                reached
                    .recv_timeout(Duration::from_secs(1))
                    .expect("sibling did not reach the read or drop boundary");
                let spec = StreamSpec::new(SoundSource::Voice, 2, 24_000);
                assert!(matches!(
                    rig.pool
                        .open(&rig.mixer, "mod", &spec, Some(Box::new(Rejected))),
                    Err(AudioRefusal::WorkerUnavailable)
                ));
                assert_eq!(rig.pool.take_pull_panics(), ["mod"]);
                resume.send(()).unwrap();
                worker.join().unwrap()
            });
            // This refill must retire the job and leave only the two original stereo frames.
            assert_eq!(
                (keep, tail),
                (false, 4),
                "{boundary}: (keep job, ring tail)"
            );
            assert_eq!(rig.stream.state(), AudioStreamState::Stopped, "{boundary}");
            assert_eq!(rig.play::<8>(), [0.0; 8], "{boundary} must not drain");
        }
    }

    #[test]
    fn a_refused_source_destruction_does_not_commit_eof_drain() {
        struct RefuseOnDrop;
        impl Drop for RefuseOnDrop {
            fn drop(&mut self) {
                mod_api::__private::refuse("test_drop", mod_api::__private::ModPhase::Reloading);
            }
        }

        let source = RefuseOnDrop;
        let mut first = true;
        let mut rig = Rig::new(move |pcm| {
            let _owned = &source;
            if std::mem::take(&mut first) {
                pcm[..4].fill(16_384);
                AudioRead::Frames(2)
            } else {
                AudioRead::Eof
            }
        });
        request_pull(&rig.mixer.lock().unwrap().streams, 4, 48_000);
        assert!(rig.refill());
        request_pull(&rig.mixer.lock().unwrap().streams, 4, 48_000);
        assert!(!rig.refill());
        assert_eq!(rig.stream.state(), AudioStreamState::Stopped);
        assert_eq!(rig.play::<8>(), [0.0; 8]);
        assert!(rig.pool.take_pull_panics().is_empty());
    }

    #[test]
    fn a_pull_source_can_panic_during_retirement_without_unwinding_the_worker() {
        struct PanicOnDrop;
        impl Drop for PanicOnDrop {
            fn drop(&mut self) {
                panic!("broken pull source destructor");
            }
        }
        let source = PanicOnDrop;
        let mut rig = Rig::new(move |_| {
            let _owned = &source;
            AudioRead::Eof
        });
        assert_eq!(rig.play::<4>(), [0.0; 4]);
        assert!(!rig.refill());
        assert_eq!(rig.stream.state(), AudioStreamState::Stopped);
        assert_eq!(rig.pool.take_pull_panics(), ["mod"]);
    }

    #[test]
    fn pull_plays_short_reads_retries_dry_reads_and_drains_the_last_frame_at_eof() {
        let reads: [(usize, &[i16], AudioRead); 4] = [
            (6, &[16_384, -8_192], AudioRead::Frames(1)),
            (6, &[99, 99], AudioRead::Frames(0)),
            (6, &[8_192, 16_384, -16_384, 8_192], AudioRead::Frames(2)),
            (2, &[], AudioRead::Eof),
        ];
        let mut reads = reads.into_iter();
        let mut rig = Rig::new(move |pcm| {
            let (offered, samples, result) = reads.next().unwrap();
            assert_eq!(pcm.len(), offered);
            assert!(pcm.iter().all(|&sample| sample == 0));
            pcm[..samples.len()].copy_from_slice(samples);
            result
        });
        assert_eq!(rig.stream.push(&[8192, 8192]), 0);
        // An empty callback asks for nothing.
        assert_eq!(rig.play::<0>(), [0.0; 0]);
        assert!(rig.refill());
        assert_eq!(rig.play::<8>(), [0.0; 8]);
        assert!(rig.refill());
        assert_eq!(
            rig.play::<8>(),
            [0.5, -0.25, 0.5, -0.25, 0.0, 0.0, 0.0, 0.0]
        );
        assert!(rig.refill()); // dry, but still open; the next pass alone must not retry
        assert!(rig.refill());
        assert_eq!(rig.stream.state(), AudioStreamState::Playing);
        assert_eq!(rig.play::<8>(), [0.0; 8]);
        assert!(rig.refill());
        assert_eq!(rig.play::<2>(), [0.25, 0.5]);
        assert!(!rig.refill()); // EOF while another frame is queued
        assert_eq!(rig.play::<4>(), [-0.125, 0.375, -0.5, 0.25]);
        assert_eq!(rig.stream.state(), AudioStreamState::Playing);
        assert_eq!(rig.play::<2>(), [-0.5, 0.25]);
        assert_eq!(rig.stream.state(), AudioStreamState::Stopped);
        assert_eq!(rig.play::<4>(), [0.0; 4]);
    }

    #[test]
    fn pull_uses_the_latest_buffer_demand_when_the_worker_has_not_read_yet() {
        let mut rig = Rig::new(|pcm| {
            // The most recent callback asked for two output frames at twice the source rate:
            // one stereo source frame plus one frame of interpolation lookahead.
            assert_eq!(pcm.len(), 4);
            pcm.copy_from_slice(&[16_384, -8192, -16_384, 8192]);
            AudioRead::Frames(2)
        });
        // Two callbacks, no producer pass in between. The smaller request replaces the larger.
        assert_eq!(rig.play::<8>(), [0.0; 8]);
        assert_eq!(rig.play::<4>(), [0.0; 4]);
        assert!(rig.refill());
        assert_eq!(
            rig.play::<8>(),
            [0.5, -0.25, 0.0, 0.0, -0.5, 0.25, -0.5, 0.25]
        );
    }

    #[test]
    fn pull_refuses_an_oversized_count_without_publishing_samples() {
        let mut rig = Rig::new(|pcm| {
            pcm.fill(16_384);
            AudioRead::Frames(usize::MAX)
        });
        assert_eq!(rig.play::<4>(), [0.0; 4]);
        let ring = rig.job.as_ref().unwrap().ring.clone();
        assert!(!rig.refill());
        assert_eq!(ring.producer_window().1, 0);
        assert_eq!(rig.stream.state(), AudioStreamState::Stopped);
        assert_eq!(rig.play::<4>(), [0.0; 4]);
    }
}
