//! One lazy PCM producer for all native pull streams on a shared output.
use super::{Control, Ring};
use mod_api::{AUDIO_STREAMS_TOTAL, AudioRefusal, AudioStream};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering::*},
        mpsc::{self, SyncSender, TryRecvError, TrySendError},
    },
    thread::{self, Thread},
};

const CAPACITY: usize = AUDIO_STREAMS_TOTAL as usize;
const ADMIT_CAPACITY: usize = CAPACITY * 2;
const SCRATCH_SAMPLES: usize = 96_000;

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

/// The only sender. Disconnecting it and waking the worker makes teardown independent of the
/// next device callback. Live voices and handles have only the wake token, not a sender.
pub(super) struct Worker {
    sender: SyncSender<PullJob>,
    wake: Wake,
}

impl Worker {
    pub(super) fn start() -> Result<Self, AudioRefusal> {
        let (sender, receiver) = mpsc::sync_channel(ADMIT_CAPACITY);
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
        Ok(Self {
            sender,
            wake: Wake {
                pending,
                thread: worker.thread().clone(),
            },
        })
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

    #[cfg(test)]
    pub(super) fn test_pair() -> (Self, mpsc::Receiver<PullJob>) {
        let (sender, receiver) = mpsc::sync_channel(ADMIT_CAPACITY);
        (
            Self {
                sender,
                wake: Wake {
                    pending: Arc::default(),
                    thread: thread::current(),
                },
            },
            receiver,
        )
    }
}

pub(super) struct PullJob {
    pub(super) control: Arc<Control>,
    pub(super) demand: Arc<Demand>,
    pub(super) ring: Arc<Ring>,
    pub(super) source: Box<dyn AudioStream>,
}

impl PullJob {
    /// At most one read per pass, using only the missing whole frames. False retires the job
    /// and its source on the worker; only the mixer decides when an EOF voice has drained.
    fn refill(&mut self, scratch: &mut [i16]) -> bool {
        if self.control.closed.load(Acquire) {
            return false;
        }
        let target = self.demand.frames.swap(0, AcqRel);
        if target == 0 {
            return true;
        }
        let (tail, queued, free) = self.ring.producer_window();
        let channels = self.ring.channels;
        let max_frames = scratch.len() / channels;
        let missing = target.saturating_sub(queued).min(free).min(max_frames);
        if missing == 0 {
            return true;
        }
        let pcm = &mut scratch[..missing * channels];
        pcm.fill(0);
        match catch_unwind(AssertUnwindSafe(|| self.source.read(pcm))) {
            Ok(0) => {
                self.demand.eof.store(true, Release);
                false
            }
            Ok(frames) if frames <= missing => {
                self.ring.publish_frames(tail, &pcm[..frames * channels]);
                true
            }
            Ok(frames) => {
                log::warn!(
                    "mod `{}` audio stream returned invalid frame count {frames} (offered {missing}); stopping stream",
                    self.control.owner
                );
                false
            }
            Err(_) => {
                log::error!(
                    "mod `{}` audio stream panicked in AudioStream::read; stopping stream",
                    self.control.owner
                );
                false
            }
        }
    }
}

impl Drop for PullJob {
    fn drop(&mut self) {
        // A job that leaves without EOF (invalid count, shutdown or worker unwinding) cannot
        // produce again. Only an EOF voice should stay open to drain.
        if !self.demand.eof.load(Relaxed) {
            self.control.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{Mixer, mix_pieces};
    use super::super::StreamPool;
    use super::*;
    use mod_api::{AudioStreamSource, AudioStreamState, SoundSource, StreamSpec};
    use std::{sync::Mutex, thread::ThreadId, time::Duration};

    struct Source<F>(F);

    impl<F: FnMut(&mut [i16]) -> usize + Send + 'static> AudioStream for Source<F> {
        fn read(&mut self, pcm: &mut [i16]) -> usize {
            (self.0)(pcm)
        }
    }

    /// Takes the admitted job instead of starting a thread, so each refill happens at a chosen
    /// output buffer boundary. Uses the same queue, refill and mixer as a live output.
    struct Rig {
        _pool: StreamPool,
        mixer: Mutex<Mixer>,
        stream: Arc<dyn AudioStreamSource>,
        job: Option<PullJob>,
        scratch: [i16; 16],
    }

    impl Rig {
        fn new(read: impl FnMut(&mut [i16]) -> usize + Send + 'static) -> Self {
            let pool = StreamPool::default();
            let (worker, receiver) = Worker::test_pair();
            pool.state().worker = Some(worker);
            let mixer = Mutex::new(Mixer {
                stream_gain: [1.0; 11],
                ..Mixer::default()
            });
            let spec = StreamSpec::new(SoundSource::Voice, 2, 24_000);
            let stream = pool
                .open(&mixer, "mod", &spec, None, Box::new(Source(read)))
                .unwrap();
            Self {
                _pool: pool,
                mixer,
                stream,
                job: Some(receiver.try_recv().unwrap()),
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
        struct Probe {
            read: SyncSender<(ThreadId, usize)>,
            dropped: SyncSender<ThreadId>,
        }

        impl AudioStream for Probe {
            fn read(&mut self, pcm: &mut [i16]) -> usize {
                pcm[0] = 16_384;
                let _ = self.read.try_send((thread::current().id(), pcm.len()));
                1
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
                None,
                Box::new(Probe { read, dropped }),
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
    fn pull_plays_short_reads_silence_frames_and_drains_the_last_frame_at_eof() {
        let reads: [(usize, &[i16], usize); 4] = [
            (6, &[16_384, -8_192], 1),
            (6, &[], 1),
            (6, &[8_192, 16_384, -16_384, 8_192], 2),
            (2, &[], 0),
        ];
        let mut reads = reads.into_iter();
        let mut rig = Rig::new(move |pcm| {
            let (offered, samples, result) = reads.next().unwrap();
            assert_eq!(pcm.len(), offered);
            assert!(pcm.iter().all(|&sample| sample == 0));
            pcm[..samples.len()].copy_from_slice(samples);
            result
        });
        // An empty callback asks for nothing.
        assert_eq!(rig.play::<0>(), [0.0; 0]);
        assert!(rig.refill());
        assert_eq!(rig.play::<8>(), [0.0; 8]);
        assert!(rig.refill());
        assert_eq!(
            rig.play::<8>(),
            [0.5, -0.25, 0.5, -0.25, 0.0, 0.0, 0.0, 0.0]
        );
        // Returning 1 without writing publishes one silent frame from the zeroed buffer.
        assert!(rig.refill());
        assert!(rig.refill());
        assert_eq!(rig.job.as_ref().unwrap().ring.producer_window().1, 1);
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
            2
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
            usize::MAX
        });
        assert_eq!(rig.play::<4>(), [0.0; 4]);
        let ring = rig.job.as_ref().unwrap().ring.clone();
        assert!(!rig.refill());
        assert_eq!(ring.producer_window().1, 0);
        assert_eq!(rig.stream.state(), AudioStreamState::Stopped);
        assert_eq!(rig.play::<4>(), [0.0; 4]);
    }

    #[test]
    fn pull_panic_stops_only_the_panicking_stream() {
        struct Panicker(SyncSender<()>);

        impl AudioStream for Panicker {
            fn read(&mut self, _pcm: &mut [i16]) -> usize {
                panic!("test audio source panic");
            }
        }

        impl Drop for Panicker {
            fn drop(&mut self) {
                let _ = self.0.try_send(());
            }
        }

        let (panicker_drop, panicker_dropped) = mpsc::sync_channel(1);
        let (healthy_read, healthy_reads) = mpsc::sync_channel(4);
        let pool = StreamPool::default();
        let mixer = Mutex::new(Mixer {
            stream_gain: [1.0; 11],
            ..Mixer::default()
        });
        let spec = StreamSpec::new(SoundSource::Voice, 2, 24_000);
        let panicker = pool
            .open(
                &mixer,
                "bad_mod",
                &spec,
                None,
                Box::new(Panicker(panicker_drop)),
            )
            .unwrap();
        let healthy = pool
            .open(
                &mixer,
                "good_mod",
                &spec,
                None,
                Box::new(Source(move |pcm| {
                    pcm[..2].copy_from_slice(&[16_384, -8_192]);
                    let _ = healthy_read.try_send(());
                    1
                })),
            )
            .unwrap();

        let timeout = Duration::from_secs(2);
        let mut out = [0.0; 8];
        mix_pieces(&mut mixer.lock().unwrap(), &mut out, 2, 48_000);
        panicker_dropped
            .recv_timeout(timeout)
            .expect("panicking source was not dropped");
        healthy_reads
            .recv_timeout(timeout)
            .expect("healthy source was not read after sibling panicked");
        assert_eq!(panicker.state(), AudioStreamState::Stopped);
        assert_eq!(healthy.state(), AudioStreamState::Playing);

        mix_pieces(&mut mixer.lock().unwrap(), &mut out, 2, 48_000);
        if out == [0.0; 8] {
            // If the worker had not yet published read #1 before the mix, wait for read #2
            // which runs serially on the same worker after read #1's publish.
            healthy_reads
                .recv_timeout(timeout)
                .expect("healthy source was not read on follow-up demand");
            mix_pieces(&mut mixer.lock().unwrap(), &mut out, 2, 48_000);
        }
        assert_eq!(out[..4], [0.5, -0.25, 0.5, -0.25]);
        assert_eq!(healthy.state(), AudioStreamState::Playing);
        healthy.stop();
    }

    #[test]
    fn pull_clamps_read_slice_to_scratch_capacity() {
        let mut rig = Rig::new(|pcm| {
            // 20 output stereo frames at 48 kHz demand 11 source frames at 24 kHz,
            // which exceeds Rig's 16-sample (8 stereo frame) scratch buffer.
            assert_eq!(pcm.len(), 16);
            pcm.fill(16_384);
            pcm.len() / 2
        });
        assert_eq!(rig.play::<40>(), [0.0; 40]);
        assert!(rig.refill());
        assert_eq!(rig.job.as_ref().unwrap().ring.producer_window().1, 8);
        assert_eq!(rig.play::<4>(), [0.5, 0.5, 0.5, 0.5]);
    }
}
