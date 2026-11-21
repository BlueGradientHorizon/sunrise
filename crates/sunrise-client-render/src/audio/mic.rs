//! Client-side microphone capture: bounded drop-oldest lock-free PCM ring, pure-Rust
//! channel/sample-rate converter, and a lazy dedicated `mic-capture` worker thread that owns
//! `!Send` `cpal::Stream` instances locally.

#[cfg(feature = "audio")]
use cpal::Sample;
#[cfg(feature = "audio")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;
#[cfg(feature = "audio")]
use std::sync::mpsc;
#[cfg(any(feature = "audio", test))]
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicI16, AtomicU64, Ordering, fence},
};
#[cfg(feature = "audio")]
use std::thread;

/// Validates a requested microphone capture specification.
pub(super) fn validate_spec(spec: &mod_api::MicSpec) -> Result<(), mod_api::MicRefusal> {
    if spec.channels != 1 && spec.channels != 2 {
        return Err(mod_api::MicRefusal::BadSpec(
            mod_api::MicSpecFault::Channels,
        ));
    }
    if !(mod_api::MIC_RATE_MIN..=mod_api::MIC_RATE_MAX).contains(&spec.rate) {
        return Err(mod_api::MicRefusal::BadSpec(mod_api::MicSpecFault::Rate));
    }
    Ok(())
}

/// Bounded drop-oldest PCM capture ring.
///
/// Single producer (`push_frames`, called from the OS capture callback: wait-free, zero
/// allocation, never locks) and any consumer thread (`read_frames`, serialized by `read_lock`
/// with a seqlock check against concurrent overwrites).
#[cfg(any(feature = "audio", test))]
struct CaptureRing {
    slots: Box<[AtomicI16]>,
    channels: usize,
    cap_frames: usize,
    head: AtomicU64,
    writing_tail: AtomicU64,
    committed_tail: AtomicU64,
    dropped: AtomicU64,
    read_lock: Mutex<()>,
}

#[cfg(any(feature = "audio", test))]
impl CaptureRing {
    fn new(rate: u32, channels: u8) -> Self {
        let cap_frames = ((rate as usize) * (mod_api::MIC_RING_MAX_MS as usize))
            .div_ceil(1_000)
            .max(1);
        Self::with_capacity_frames(cap_frames, usize::from(channels))
    }

    fn with_capacity_frames(cap_frames: usize, channels: usize) -> Self {
        let cap_frames = cap_frames.max(1);
        let channels = channels.max(1);
        let slots = (0..cap_frames * channels)
            .map(|_| AtomicI16::new(0))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            slots,
            channels,
            cap_frames,
            head: AtomicU64::new(0),
            writing_tail: AtomicU64::new(0),
            committed_tail: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            read_lock: Mutex::new(()),
        }
    }

    /// Wait-free single-producer push of whole interleaved PCM frames.
    /// Drops the oldest frames whenever the ring overflows.
    fn push_frames(&self, pcm: &[i16]) {
        let ch = self.channels;
        let frames_in = pcm.len() / ch;
        if frames_in == 0 {
            return;
        }
        let (slice, n) = if frames_in > self.cap_frames {
            let skip = frames_in - self.cap_frames;
            self.dropped.fetch_add(skip as u64, Ordering::Relaxed);
            (&pcm[skip * ch..frames_in * ch], self.cap_frames)
        } else {
            (&pcm[..frames_in * ch], frames_in)
        };

        let t = self.committed_tail.load(Ordering::Relaxed);
        let new_tail = t + n as u64;
        let min_head = new_tail.saturating_sub(self.cap_frames as u64);

        // Announce write window before touching slots so concurrent readers can detect overwrites.
        self.writing_tail.store(new_tail, Ordering::SeqCst);

        // Advance head past overwritten frames and account for dropped frames.
        let old_head = self.head.fetch_max(min_head, Ordering::SeqCst);
        if min_head > old_head {
            self.dropped
                .fetch_add(min_head - old_head, Ordering::Relaxed);
        }

        let cap = self.cap_frames as u64;
        for i in 0..n {
            let slot_base = ((t + i as u64) % cap) as usize * ch;
            let src_base = i * ch;
            for c in 0..ch {
                self.slots[slot_base + c].store(slice[src_base + c], Ordering::Relaxed);
            }
        }

        self.committed_tail.store(new_tail, Ordering::SeqCst);
    }

    /// Reads whole interleaved PCM frames into `out` from any thread without blocking on the
    /// producer. Any trailing incomplete frame in `out` is untouched.
    fn read_frames(&self, out: &mut [i16]) -> u32 {
        let ch = self.channels;
        let max_frames = out.len() / ch;
        if max_frames == 0 {
            return 0;
        }
        let _guard = self.read_lock.lock().unwrap_or_else(|e| e.into_inner());
        let cap = self.cap_frames as u64;

        for _ in 0..8 {
            let h = self.head.load(Ordering::SeqCst);
            let t = self.committed_tail.load(Ordering::SeqCst);
            if t <= h {
                return 0;
            }
            if t - h > cap {
                continue;
            }
            let avail = (t - h).min(max_frames as u64) as usize;
            for i in 0..avail {
                let slot_base = ((h + i as u64) % cap) as usize * ch;
                let dst_base = i * ch;
                for c in 0..ch {
                    out[dst_base + c] = self.slots[slot_base + c].load(Ordering::Relaxed);
                }
            }
            fence(Ordering::SeqCst);
            let wt = self.writing_tail.load(Ordering::SeqCst);
            if wt.saturating_sub(cap) <= h
                && self
                    .head
                    .compare_exchange(h, h + avail as u64, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            {
                return avail as u32;
            }
        }
        0
    }

    fn dropped_frames(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

#[cfg(any(feature = "audio", test))]
#[inline]
fn float_to_i16(x: f32) -> i16 {
    (x.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

/// Pure-Rust channel mapper and linear resampler for incoming OS capture callbacks.
/// Uses a stack scratch buffer and performs zero heap allocations per callback.
#[cfg(any(feature = "audio", test))]
struct InputConverter {
    in_channels: usize,
    out_channels: usize,
    in_rate: u32,
    out_rate: u32,
    prev: [f32; 2],
    has_prev: bool,
    pos: f64,
    step: f64,
}

#[cfg(any(feature = "audio", test))]
impl InputConverter {
    fn new(in_channels: u16, in_rate: u32, out_channels: u8, out_rate: u32) -> Self {
        Self {
            in_channels: usize::from(in_channels),
            out_channels: usize::from(out_channels),
            in_rate,
            out_rate,
            prev: [0.0; 2],
            has_prev: false,
            pos: 0.0,
            step: if out_rate > 0 {
                f64::from(in_rate) / f64::from(out_rate)
            } else {
                1.0
            },
        }
    }

    #[inline]
    fn convert_frame<T: Copy>(&self, frame: &[T], to_f32: &impl Fn(T) -> f32) -> [f32; 2] {
        if self.out_channels == 1 {
            if self.in_channels == 1 {
                [to_f32(frame[0]), 0.0]
            } else {
                let mut sum = 0.0_f32;
                for &s in frame {
                    sum += to_f32(s);
                }
                [sum / self.in_channels as f32, 0.0]
            }
        } else if self.in_channels == 1 {
            let s = to_f32(frame[0]);
            [s, s]
        } else {
            [to_f32(frame[0]), to_f32(frame[1])]
        }
    }

    fn push_samples<T: Copy>(
        &mut self,
        samples: &[T],
        ring: &CaptureRing,
        to_f32: impl Fn(T) -> f32,
    ) {
        if self.in_channels == 0
            || self.out_channels == 0
            || self.in_rate == 0
            || self.out_rate == 0
        {
            return;
        }
        let in_frames = samples.len() / self.in_channels;
        if in_frames == 0 {
            return;
        }

        let mut scratch = [0_i16; 1024];
        let max_scratch = (scratch.len() / self.out_channels) * self.out_channels;
        let mut len = 0_usize;

        if self.in_rate == self.out_rate {
            if self.in_channels == self.out_channels {
                // Fast path: direct sample conversion with matching channels and rate.
                let total = in_frames * self.in_channels;
                for &s in &samples[..total] {
                    scratch[len] = float_to_i16(to_f32(s));
                    len += 1;
                    if len == max_scratch {
                        ring.push_frames(&scratch[..len]);
                        len = 0;
                    }
                }
            } else {
                for f in 0..in_frames {
                    let frame = &samples[f * self.in_channels..(f + 1) * self.in_channels];
                    let mixed = self.convert_frame(frame, &to_f32);
                    for c in 0..self.out_channels {
                        scratch[len] = float_to_i16(mixed[c]);
                        len += 1;
                    }
                    if len == max_scratch {
                        ring.push_frames(&scratch[..len]);
                        len = 0;
                    }
                }
            }
            if len > 0 {
                ring.push_frames(&scratch[..len]);
            }
            return;
        }

        let n = in_frames as isize;
        loop {
            let i0 = self.pos.floor() as isize;
            if i0 >= n {
                break;
            }
            let frac = (self.pos - i0 as f64) as f32;
            let sample = if i0 < 0 {
                let first = self.convert_frame(&samples[..self.in_channels], &to_f32);
                let a = if self.has_prev { self.prev } else { first };
                [
                    a[0] + (first[0] - a[0]) * frac,
                    a[1] + (first[1] - a[1]) * frac,
                ]
            } else if i0 == n - 1 {
                if self.pos > i0 as f64 {
                    break;
                }
                let idx = i0 as usize;
                self.convert_frame(
                    &samples[idx * self.in_channels..(idx + 1) * self.in_channels],
                    &to_f32,
                )
            } else {
                let idx0 = i0 as usize;
                let idx1 = idx0 + 1;
                let a = self.convert_frame(
                    &samples[idx0 * self.in_channels..(idx0 + 1) * self.in_channels],
                    &to_f32,
                );
                let b = self.convert_frame(
                    &samples[idx1 * self.in_channels..(idx1 + 1) * self.in_channels],
                    &to_f32,
                );
                [a[0] + (b[0] - a[0]) * frac, a[1] + (b[1] - a[1]) * frac]
            };

            for c in 0..self.out_channels {
                scratch[len] = float_to_i16(sample[c]);
                len += 1;
            }
            if len == max_scratch {
                ring.push_frames(&scratch[..len]);
                len = 0;
            }
            self.pos += self.step;
        }

        if len > 0 {
            ring.push_frames(&scratch[..len]);
        }

        let last = in_frames - 1;
        self.prev = self.convert_frame(
            &samples[last * self.in_channels..(last + 1) * self.in_channels],
            &to_f32,
        );
        self.has_prev = true;
        self.pos -= in_frames as f64;
    }

    #[cfg(test)]
    fn push_f32_samples(&mut self, samples: &[f32], ring: &CaptureRing) {
        self.push_samples(samples, ring, std::convert::identity);
    }

    #[cfg(feature = "audio")]
    fn push_cpal_samples<T: cpal::SizedSample>(&mut self, samples: &[T], ring: &CaptureRing)
    where
        f32: cpal::FromSample<T>,
    {
        self.push_samples(samples, ring, f32::from_sample);
    }
}

/// Classifies an OS audio backend error message into a [`mod_api::MicRefusal`].
#[cfg(any(feature = "audio", test))]
fn classify_error_message(msg: &str) -> mod_api::MicRefusal {
    const DENIED_PATTERNS: &[&str] = &[
        "permission denied",
        "access denied",
        "access is denied",
        "not authorized",
        "eperm",
        "eacces",
        "denied",
        "unauthorized",
        "e_accessdenied",
        "0x80070005",
    ];
    const BUSY_PATTERNS: &[&str] = &[
        "device or resource busy",
        "ebusy",
        "in use",
        "exclusive",
        "busy",
        "audclnt_e_device_in_use",
        "0x8889000a",
    ];
    let lower = msg.to_ascii_lowercase();
    if DENIED_PATTERNS.iter().any(|p| lower.contains(p)) {
        return mod_api::MicRefusal::OsDenied;
    }
    if BUSY_PATTERNS.iter().any(|p| lower.contains(p)) {
        return mod_api::MicRefusal::Busy;
    }
    mod_api::MicRefusal::DeviceFailed
}

#[cfg(feature = "audio")]
fn map_devices_error(err: cpal::DevicesError) -> mod_api::MicRefusal {
    match err {
        cpal::DevicesError::BackendSpecific { err } => classify_error_message(&err.description),
    }
}

#[cfg(feature = "audio")]
fn map_supported_configs_error(err: cpal::SupportedStreamConfigsError) -> mod_api::MicRefusal {
    match err {
        cpal::SupportedStreamConfigsError::DeviceNotAvailable => mod_api::MicRefusal::NoDevice,
        cpal::SupportedStreamConfigsError::BackendSpecific { err } => {
            classify_error_message(&err.description)
        }
        cpal::SupportedStreamConfigsError::InvalidArgument => mod_api::MicRefusal::DeviceFailed,
    }
}

#[cfg(feature = "audio")]
fn map_default_config_error(err: cpal::DefaultStreamConfigError) -> mod_api::MicRefusal {
    match err {
        cpal::DefaultStreamConfigError::DeviceNotAvailable => mod_api::MicRefusal::NoDevice,
        cpal::DefaultStreamConfigError::BackendSpecific { err } => {
            classify_error_message(&err.description)
        }
        cpal::DefaultStreamConfigError::StreamTypeNotSupported => mod_api::MicRefusal::DeviceFailed,
    }
}

#[cfg(feature = "audio")]
fn map_build_stream_error(err: cpal::BuildStreamError) -> mod_api::MicRefusal {
    match err {
        cpal::BuildStreamError::DeviceNotAvailable => mod_api::MicRefusal::NoDevice,
        cpal::BuildStreamError::BackendSpecific { err } => classify_error_message(&err.description),
        cpal::BuildStreamError::StreamConfigNotSupported
        | cpal::BuildStreamError::InvalidArgument
        | cpal::BuildStreamError::StreamIdOverflow => mod_api::MicRefusal::DeviceFailed,
    }
}

#[cfg(feature = "audio")]
fn map_play_stream_error(err: cpal::PlayStreamError) -> mod_api::MicRefusal {
    match err {
        cpal::PlayStreamError::DeviceNotAvailable => mod_api::MicRefusal::NoDevice,
        cpal::PlayStreamError::BackendSpecific { err } => classify_error_message(&err.description),
    }
}

/// Engine-side handle backing a mod's [`mod_api::MicRef`].
///
/// Never passed to the `cpal` callback so dropping `MicRef` drops `MicHandle` immediately and
/// sends a close command to stop capture.
#[cfg(any(feature = "audio", test))]
struct MicHandle {
    ring: Arc<CaptureRing>,
    channels: u8,
    rate: u32,
    closed: Arc<AtomicBool>,
    device_open: Arc<AtomicBool>,
    on_close: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

#[cfg(any(feature = "audio", test))]
impl mod_api::MicSource for MicHandle {
    fn read(&self, pcm: &mut [i16]) -> u32 {
        if self.closed.load(Ordering::Acquire) {
            return 0;
        }
        self.ring.read_frames(pcm)
    }

    fn channels(&self) -> u8 {
        self.channels
    }

    fn rate(&self) -> u32 {
        self.rate
    }

    fn dropped_frames(&self) -> u64 {
        self.ring.dropped_frames()
    }

    fn is_open(&self) -> bool {
        !self.closed.load(Ordering::Acquire) && self.device_open.load(Ordering::Acquire)
    }

    fn close(&self) {
        if !self.closed.swap(true, Ordering::AcqRel) {
            self.device_open.store(false, Ordering::Release);
            let callback = self
                .on_close
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            if let Some(callback) = callback {
                callback();
            }
        }
    }
}

#[cfg(any(feature = "audio", test))]
impl Drop for MicHandle {
    fn drop(&mut self) {
        mod_api::MicSource::close(self);
    }
}

#[cfg(feature = "audio")]
struct MicWorkerHandle {
    tx: mpsc::Sender<WorkerCmd>,
    join: Option<thread::JoinHandle<()>>,
}

#[cfg(feature = "audio")]
enum WorkerCmd {
    ListDevices {
        reply: mpsc::SyncSender<Result<Vec<String>, mod_api::MicRefusal>>,
    },
    Open {
        id: u64,
        spec: mod_api::MicSpec,
        ring: Arc<CaptureRing>,
        closed: Arc<AtomicBool>,
        device_open: Arc<AtomicBool>,
        reply: mpsc::SyncSender<Result<(), mod_api::MicRefusal>>,
    },
    Close {
        id: u64,
    },
    Shutdown,
}

/// Lazy pool owning the dedicated `mic-capture` worker thread when audio capture is enabled.
pub(super) struct MicPool {
    #[cfg(feature = "audio")]
    enabled: bool,
    #[cfg(feature = "audio")]
    worker: Mutex<Option<MicWorkerHandle>>,
    #[cfg(feature = "audio")]
    next_id: AtomicU64,
}

impl MicPool {
    /// Creates a new capture pool without spawning any worker thread.
    pub(super) fn new(enabled: bool) -> Self {
        #[cfg(not(feature = "audio"))]
        let _ = enabled;
        Self {
            #[cfg(feature = "audio")]
            enabled,
            #[cfg(feature = "audio")]
            worker: Mutex::new(None),
            #[cfg(feature = "audio")]
            next_id: AtomicU64::new(1),
        }
    }

    /// Lists available audio input device names.
    pub(super) fn devices(&self) -> Result<Vec<String>, mod_api::MicRefusal> {
        #[cfg(not(feature = "audio"))]
        {
            Ok(Vec::new())
        }
        #[cfg(feature = "audio")]
        {
            if !self.enabled {
                return Ok(Vec::new());
            }
            let tx = self.worker_sender()?;
            let (reply_tx, reply_rx) = mpsc::sync_channel(1);
            tx.send(WorkerCmd::ListDevices { reply: reply_tx })
                .map_err(|_| mod_api::MicRefusal::DeviceFailed)?;
            reply_rx
                .recv()
                .map_err(|_| mod_api::MicRefusal::DeviceFailed)?
        }
    }

    /// Validates `spec` and opens a live microphone capture stream.
    pub(super) fn open(
        &self,
        spec: &mod_api::MicSpec,
    ) -> Result<Arc<dyn mod_api::MicSource>, mod_api::MicRefusal> {
        validate_spec(spec)?;
        #[cfg(not(feature = "audio"))]
        {
            Err(mod_api::MicRefusal::NoDevice)
        }
        #[cfg(feature = "audio")]
        {
            if !self.enabled {
                return Err(mod_api::MicRefusal::NoDevice);
            }
            let tx = self.worker_sender()?;
            let ring = Arc::new(CaptureRing::new(spec.rate, spec.channels));
            let closed = Arc::new(AtomicBool::new(false));
            let device_open = Arc::new(AtomicBool::new(true));
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let (reply_tx, reply_rx) = mpsc::sync_channel(1);
            tx.send(WorkerCmd::Open {
                id,
                spec: spec.clone(),
                ring: Arc::clone(&ring),
                closed: Arc::clone(&closed),
                device_open: Arc::clone(&device_open),
                reply: reply_tx,
            })
            .map_err(|_| mod_api::MicRefusal::DeviceFailed)?;
            reply_rx
                .recv()
                .map_err(|_| mod_api::MicRefusal::DeviceFailed)??;
            Ok(Arc::new(MicHandle {
                ring,
                channels: spec.channels,
                rate: spec.rate,
                closed,
                device_open,
                on_close: Mutex::new(Some(Box::new(move || {
                    let _ = tx.send(WorkerCmd::Close { id });
                }))),
            }))
        }
    }

    #[cfg(feature = "audio")]
    fn worker_sender(&self) -> Result<mpsc::Sender<WorkerCmd>, mod_api::MicRefusal> {
        let mut slot = self.worker.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(handle) = slot.as_ref() {
            return Ok(handle.tx.clone());
        }
        let (tx, rx) = mpsc::channel();
        let join = thread::Builder::new()
            .name("mic-capture".into())
            .spawn(move || run_worker(rx))
            .map_err(|e| {
                log::warn!("Sunrise: failed to start mic-capture thread: {e}");
                mod_api::MicRefusal::DeviceFailed
            })?;
        let sender = tx.clone();
        *slot = Some(MicWorkerHandle {
            tx,
            join: Some(join),
        });
        Ok(sender)
    }
}

#[cfg(feature = "audio")]
impl Drop for MicPool {
    fn drop(&mut self) {
        let handle = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(mut handle) = handle {
            let _ = handle.tx.send(WorkerCmd::Shutdown);
            if let Some(join) = handle.join.take() {
                let _ = join.join();
            }
        }
    }
}

#[cfg(feature = "audio")]
fn run_worker(rx: mpsc::Receiver<WorkerCmd>) {
    let mut streams: Vec<(u64, Arc<AtomicBool>, cpal::Stream)> = Vec::new();
    while let Ok(cmd) = rx.recv() {
        match cmd {
            WorkerCmd::ListDevices { reply } => {
                let _ = reply.send(list_input_devices());
            }
            WorkerCmd::Open {
                id,
                spec,
                ring,
                closed,
                device_open,
                reply,
            } => {
                streams.retain(|(_, dev_open, _)| dev_open.load(Ordering::Relaxed));
                match open_input_stream(&spec, ring, closed, Arc::clone(&device_open)) {
                    Ok(stream) => {
                        streams.push((id, device_open, stream));
                        let _ = reply.send(Ok(()));
                    }
                    Err(refusal) => {
                        device_open.store(false, Ordering::Release);
                        let _ = reply.send(Err(refusal));
                    }
                }
            }
            WorkerCmd::Close { id } => {
                streams.retain(|(sid, _, _)| *sid != id);
            }
            WorkerCmd::Shutdown => {
                break;
            }
        }
    }
    for (_, dev_open, _) in &streams {
        dev_open.store(false, Ordering::Release);
    }
}

#[cfg(feature = "audio")]
fn list_input_devices() -> Result<Vec<String>, mod_api::MicRefusal> {
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(map_devices_error)?;
    let mut names = Vec::new();
    for dev in devices {
        if let Ok(name) = dev.name()
            && !name.is_empty()
            && !names.contains(&name)
        {
            names.push(name);
        }
    }
    Ok(names)
}

#[cfg(feature = "audio")]
fn open_input_stream(
    spec: &mod_api::MicSpec,
    ring: Arc<CaptureRing>,
    closed: Arc<AtomicBool>,
    device_open: Arc<AtomicBool>,
) -> Result<cpal::Stream, mod_api::MicRefusal> {
    let host = cpal::default_host();
    let device = match spec.device.as_deref() {
        None => host
            .default_input_device()
            .ok_or(mod_api::MicRefusal::NoDevice)?,
        Some(target) => {
            let mut found = None;
            for dev in host.input_devices().map_err(map_devices_error)? {
                if let Ok(name) = dev.name()
                    && name == target
                {
                    found = Some(dev);
                    break;
                }
            }
            found.ok_or(mod_api::MicRefusal::NoDevice)?
        }
    };

    let chosen = select_input_config(&device, spec)?;
    let stream = match build_stream_for_config(
        &device,
        &chosen,
        spec,
        Arc::clone(&ring),
        Arc::clone(&closed),
        Arc::clone(&device_open),
    ) {
        Ok(stream) => stream,
        Err(cpal::BuildStreamError::StreamConfigNotSupported) => {
            let fallback = device
                .default_input_config()
                .map_err(map_default_config_error)?;
            if fallback == chosen {
                return Err(mod_api::MicRefusal::DeviceFailed);
            }
            build_stream_for_config(&device, &fallback, spec, ring, closed, device_open)
                .map_err(map_build_stream_error)?
        }
        Err(err) => return Err(map_build_stream_error(err)),
    };
    stream.play().map_err(map_play_stream_error)?;
    Ok(stream)
}

#[cfg(feature = "audio")]
fn select_input_config(
    device: &cpal::Device,
    spec: &mod_api::MicSpec,
) -> Result<cpal::SupportedStreamConfig, mod_api::MicRefusal> {
    match device.supported_input_configs() {
        Ok(configs) => {
            if let Some(best) = choose_best_config(configs, spec) {
                return Ok(best);
            }
            device
                .default_input_config()
                .map_err(map_default_config_error)
        }
        Err(supported_err) => {
            let fallback_refusal = map_supported_configs_error(supported_err);
            match device.default_input_config() {
                Ok(cfg) => Ok(cfg),
                Err(default_err) => {
                    let default_refusal = map_default_config_error(default_err);
                    Err(if default_refusal == mod_api::MicRefusal::DeviceFailed {
                        fallback_refusal
                    } else {
                        default_refusal
                    })
                }
            }
        }
    }
}

#[cfg(feature = "audio")]
fn choose_best_config(
    configs: impl Iterator<Item = cpal::SupportedStreamConfigRange>,
    spec: &mod_api::MicSpec,
) -> Option<cpal::SupportedStreamConfig> {
    use cpal::SampleFormat as F;
    let target_rate = cpal::SampleRate(spec.rate);
    let target_channels = u16::from(spec.channels);
    let mut best: Option<(
        (bool, bool, bool, u8, u32, u16),
        cpal::SupportedStreamConfig,
    )> = None;

    for range in configs {
        let format_rank = match range.sample_format() {
            F::F32 => 3_u8,
            F::I16 => 2,
            F::I24 | F::I32 | F::F64 => 1,
            F::I8 | F::I64 | F::U8 | F::U16 | F::U32 | F::U64 => 0,
            _ => continue,
        };
        let (exact_rate, cfg) = match range.try_with_sample_rate(target_rate) {
            Some(cfg) => (true, cfg),
            None => {
                let clamped = cpal::SampleRate(
                    spec.rate
                        .clamp(range.min_sample_rate().0, range.max_sample_rate().0),
                );
                let Some(cfg) = range.try_with_sample_rate(clamped) else {
                    continue;
                };
                (false, cfg)
            }
        };
        let exact_channels = cfg.channels() == target_channels;
        let enough_channels = cfg.channels() >= target_channels;
        let rate_closeness = u32::MAX - cfg.sample_rate().0.abs_diff(spec.rate);
        let channel_closeness = u16::MAX - cfg.channels().abs_diff(target_channels);
        let key = (
            exact_rate,
            exact_channels,
            enough_channels,
            format_rank,
            rate_closeness,
            channel_closeness,
        );
        if best.as_ref().is_none_or(|(best_key, _)| key > *best_key) {
            best = Some((key, cfg));
        }
    }
    best.map(|(_, cfg)| cfg)
}

#[cfg(feature = "audio")]
fn build_stream_for_config(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    spec: &mod_api::MicSpec,
    ring: Arc<CaptureRing>,
    closed: Arc<AtomicBool>,
    device_open: Arc<AtomicBool>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    use cpal::SampleFormat as F;
    match config.sample_format() {
        F::I8 => build_typed_stream::<i8>(device, config, spec, ring, closed, device_open),
        F::I16 => build_typed_stream::<i16>(device, config, spec, ring, closed, device_open),
        F::I24 => build_typed_stream::<cpal::I24>(device, config, spec, ring, closed, device_open),
        F::I32 => build_typed_stream::<i32>(device, config, spec, ring, closed, device_open),
        F::I64 => build_typed_stream::<i64>(device, config, spec, ring, closed, device_open),
        F::U8 => build_typed_stream::<u8>(device, config, spec, ring, closed, device_open),
        F::U16 => build_typed_stream::<u16>(device, config, spec, ring, closed, device_open),
        F::U32 => build_typed_stream::<u32>(device, config, spec, ring, closed, device_open),
        F::U64 => build_typed_stream::<u64>(device, config, spec, ring, closed, device_open),
        F::F32 => build_typed_stream::<f32>(device, config, spec, ring, closed, device_open),
        F::F64 => build_typed_stream::<f64>(device, config, spec, ring, closed, device_open),
        _ => Err(cpal::BuildStreamError::StreamConfigNotSupported),
    }
}

#[cfg(feature = "audio")]
fn build_typed_stream<T>(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    spec: &mod_api::MicSpec,
    ring: Arc<CaptureRing>,
    closed: Arc<AtomicBool>,
    device_open: Arc<AtomicBool>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let stream_config = config.config();
    let mut converter = InputConverter::new(
        config.channels(),
        config.sample_rate().0,
        spec.channels,
        spec.rate,
    );
    let err_open = Arc::clone(&device_open);
    device.build_input_stream(
        &stream_config,
        move |data: &[T], _info: &cpal::InputCallbackInfo| {
            if closed.load(Ordering::Relaxed) || !device_open.load(Ordering::Relaxed) {
                return;
            }
            converter.push_cpal_samples(data, &ring);
        },
        move |e| {
            err_open.store(false, Ordering::Release);
            log::warn!("Sunrise: microphone stream error: {e}");
        },
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{MicRefusal, MicSource, MicSpec, MicSpecFault};
    use std::sync::atomic::AtomicU32;

    #[test]
    fn validate_spec_bounds() {
        assert_eq!(
            validate_spec(&MicSpec::new(0, 16_000)),
            Err(MicRefusal::BadSpec(MicSpecFault::Channels))
        );
        assert_eq!(
            validate_spec(&MicSpec::new(3, 16_000)),
            Err(MicRefusal::BadSpec(MicSpecFault::Channels))
        );
        assert_eq!(
            validate_spec(&MicSpec::new(1, 7_999)),
            Err(MicRefusal::BadSpec(MicSpecFault::Rate))
        );
        assert_eq!(
            validate_spec(&MicSpec::new(2, 48_001)),
            Err(MicRefusal::BadSpec(MicSpecFault::Rate))
        );
        for channels in [1_u8, 2] {
            for rate in [8_000_u32, 16_000, 44_100, 48_000] {
                assert_eq!(validate_spec(&MicSpec::new(channels, rate)), Ok(()));
            }
        }
    }

    #[test]
    fn capture_ring_fifo_and_whole_frame_truncation() {
        let sized = CaptureRing::new(16_000, 1);
        assert_eq!(sized.cap_frames, 8_000);

        let ring = CaptureRing::with_capacity_frames(8, 2);
        ring.push_frames(&[10, -10, 20, -20, 30, -30]);
        assert_eq!(ring.dropped_frames(), 0);

        let mut odd = [99_i16; 5];
        assert_eq!(ring.read_frames(&mut odd), 2);
        assert_eq!(odd, [10, -10, 20, -20, 99]);

        let mut too_small = [77_i16; 1];
        assert_eq!(ring.read_frames(&mut too_small), 0);
        assert_eq!(too_small, [77]);

        let mut rest = [0_i16; 4];
        assert_eq!(ring.read_frames(&mut rest), 1);
        assert_eq!(&rest[..2], &[30, -30]);
        assert_eq!(ring.read_frames(&mut rest), 0);
    }

    #[test]
    fn capture_ring_overflow_and_oversized_push() {
        let ring = CaptureRing::with_capacity_frames(4, 1);
        ring.push_frames(&[1, 2, 3]);
        ring.push_frames(&[4, 5, 6]);
        assert_eq!(ring.dropped_frames(), 2);

        let mut out = [0_i16; 4];
        assert_eq!(ring.read_frames(&mut out), 4);
        assert_eq!(out, [3, 4, 5, 6]);

        // Unread 2 frames + oversized push of 6 frames into capacity 4:
        // 2 skipped input frames + 2 unread frames overwritten = 4 dropped.
        ring.push_frames(&[10, 20]);
        ring.push_frames(&[30, 40, 50, 60, 70, 80]);
        assert_eq!(ring.dropped_frames(), 6);
        assert_eq!(ring.read_frames(&mut out), 4);
        assert_eq!(out, [50, 60, 70, 80]);
    }

    #[test]
    fn capture_ring_concurrent_monotonic_tear_free() {
        const TOTAL_FRAMES: u64 = 20_000;
        let ring = Arc::new(CaptureRing::with_capacity_frames(32, 2));
        let done = Arc::new(AtomicBool::new(false));

        let prod_ring = Arc::clone(&ring);
        let prod_done = Arc::clone(&done);
        let producer = std::thread::spawn(move || {
            let mut seq = 1_i16;
            let mut sent = 0_u64;
            let mut batch = [0_i16; 14]; // 7 stereo frames per push
            while sent < TOTAL_FRAMES {
                let count = ((TOTAL_FRAMES - sent) as usize).min(7);
                for i in 0..count {
                    batch[i * 2] = seq;
                    batch[i * 2 + 1] = -seq;
                    seq += 1;
                }
                prod_ring.push_frames(&batch[..count * 2]);
                sent += count as u64;
            }
            prod_done.store(true, Ordering::Release);
        });

        let mut last = 0_i16;
        let mut total_read = 0_u64;
        let mut buf = [0_i16; 24];
        loop {
            let n = ring.read_frames(&mut buf) as usize;
            if n > 0 {
                for i in 0..n {
                    let l = buf[i * 2];
                    let r = buf[i * 2 + 1];
                    assert_eq!(r, -l, "stereo frame must not tear");
                    assert!(l > last, "frames must remain strictly monotonic");
                    last = l;
                }
                total_read += n as u64;
            } else if done.load(Ordering::Acquire) {
                // One final drain after observing producer completion.
                let tail = ring.read_frames(&mut buf) as usize;
                for i in 0..tail {
                    let l = buf[i * 2];
                    let r = buf[i * 2 + 1];
                    assert_eq!(r, -l, "stereo frame must not tear");
                    assert!(l > last, "frames must remain strictly monotonic");
                    last = l;
                }
                total_read += tail as u64;
                if tail == 0 {
                    break;
                }
            } else {
                std::thread::yield_now();
            }
        }
        producer.join().unwrap();
        assert_eq!(total_read + ring.dropped_frames(), TOTAL_FRAMES);
        assert!(total_read > 0);
    }

    #[test]
    fn input_converter_channel_mapping_and_clamping() {
        // Stereo -> mono average downmix.
        let ring = CaptureRing::with_capacity_frames(16, 1);
        let mut conv = InputConverter::new(2, 16_000, 1, 16_000);
        conv.push_f32_samples(&[1.0, 0.0, -0.5, 0.5, -1.0, -1.0], &ring);
        let mut mono = [0_i16; 4];
        assert_eq!(ring.read_frames(&mut mono), 3);
        assert_eq!(&mono[..3], &[16_384, 0, -32_767]);

        // Mono -> stereo duplication.
        let ring = CaptureRing::with_capacity_frames(16, 2);
        let mut conv = InputConverter::new(1, 16_000, 2, 16_000);
        conv.push_f32_samples(&[0.25, -0.5], &ring);
        let mut stereo = [0_i16; 4];
        assert_eq!(ring.read_frames(&mut stereo), 2);
        assert_eq!(stereo, [8_192, 8_192, -16_384, -16_384]);

        // 4-channel -> mono average downmix.
        let ring = CaptureRing::with_capacity_frames(16, 1);
        let mut conv = InputConverter::new(4, 16_000, 1, 16_000);
        conv.push_f32_samples(&[0.4, 0.4, 0.0, 0.0], &ring);
        let mut out1 = [0_i16; 2];
        assert_eq!(ring.read_frames(&mut out1), 1);
        assert_eq!(out1[0], 6_553);

        // 4-channel -> stereo takes first 2 channels.
        let ring = CaptureRing::with_capacity_frames(16, 2);
        let mut conv = InputConverter::new(4, 16_000, 2, 16_000);
        conv.push_f32_samples(&[0.5, -0.25, 1.0, -1.0], &ring);
        let mut out2 = [0_i16; 2];
        assert_eq!(ring.read_frames(&mut out2), 1);
        assert_eq!(out2, [16_384, -8_192]);

        // Out-of-range float clamping.
        let ring = CaptureRing::with_capacity_frames(16, 1);
        let mut conv = InputConverter::new(1, 16_000, 1, 16_000);
        conv.push_f32_samples(&[2.5, -3.0, 1.0, -1.0], &ring);
        let mut clamped = [0_i16; 4];
        assert_eq!(ring.read_frames(&mut clamped), 4);
        assert_eq!(clamped, [32_767, -32_767, 32_767, -32_767]);
    }

    #[test]
    fn input_converter_resampling_continuity_across_chunks() {
        // 48 kHz -> 16 kHz (step = 3.0) over a linear ramp split across irregular chunks.
        let input_down: Vec<f32> = (0..30).map(|i| i as f32 / 100.0).collect();
        let ring_chunked = CaptureRing::with_capacity_frames(64, 1);
        let mut conv_chunked = InputConverter::new(1, 48_000, 1, 16_000);
        let mut offset = 0;
        for chunk_len in [7, 5, 11, 7] {
            conv_chunked.push_f32_samples(&input_down[offset..offset + chunk_len], &ring_chunked);
            offset += chunk_len;
        }
        let mut got_down = [0_i16; 16];
        let count_down = ring_chunked.read_frames(&mut got_down) as usize;
        assert_eq!(count_down, 10);
        for (k, &sample) in got_down[..count_down].iter().enumerate() {
            let expected = (((k * 3) as f32 / 100.0) * 32767.0).round() as i16;
            assert_eq!(sample, expected);
        }

        // 16 kHz -> 48 kHz (step = 1/3) over a linear ramp: chunked matches single push.
        let input_up: Vec<f32> = (0..10).map(|i| i as f32 * 0.1).collect();
        let ring_single = CaptureRing::with_capacity_frames(64, 1);
        let mut conv_single = InputConverter::new(1, 16_000, 1, 48_000);
        conv_single.push_f32_samples(&input_up, &ring_single);

        let ring_multi = CaptureRing::with_capacity_frames(64, 1);
        let mut conv_multi = InputConverter::new(1, 16_000, 1, 48_000);
        for chunk in [&input_up[0..3], &input_up[3..5], &input_up[5..10]] {
            conv_multi.push_f32_samples(chunk, &ring_multi);
        }

        let mut single_out = [0_i16; 64];
        let mut multi_out = [0_i16; 64];
        let n_single = ring_single.read_frames(&mut single_out) as usize;
        let n_multi = ring_multi.read_frames(&mut multi_out) as usize;
        assert_eq!(n_single, 28);
        assert_eq!(n_multi, n_single);
        assert_eq!(&multi_out[..n_multi], &single_out[..n_single]);
        for (k, &sample) in multi_out[..n_multi].iter().enumerate() {
            let expected = (((k as f64 / 3.0) * 0.1) as f32 * 32767.0).round() as i16;
            assert!((sample - expected).abs() <= 1);
        }
    }

    #[test]
    fn mic_handle_lifecycle_and_drop_idempotence() {
        let ring = Arc::new(CaptureRing::with_capacity_frames(8, 1));
        ring.push_frames(&[100, 200, 300]);
        let closed = Arc::new(AtomicBool::new(false));
        let device_open = Arc::new(AtomicBool::new(true));
        let close_calls = Arc::new(AtomicU32::new(0));

        let counter = Arc::clone(&close_calls);
        let handle = MicHandle {
            ring,
            channels: 1,
            rate: 16_000,
            closed: Arc::clone(&closed),
            device_open: Arc::clone(&device_open),
            on_close: Mutex::new(Some(Box::new(move || {
                counter.fetch_add(1, Ordering::Relaxed);
            }))),
        };

        assert_eq!(handle.channels(), 1);
        assert_eq!(handle.rate(), 16_000);
        assert_eq!(handle.dropped_frames(), 0);
        assert!(handle.is_open());
        assert!(!closed.load(Ordering::Acquire));

        let mut pcm = [0_i16; 1];
        assert_eq!(handle.read(&mut pcm), 1);
        assert_eq!(pcm, [100]);

        // Simulated OS disconnect flips is_open() to false.
        device_open.store(false, Ordering::Release);
        assert!(!handle.is_open());

        // Explicit close stops reads and invokes on_close once.
        handle.close();
        assert!(!handle.is_open());
        assert!(closed.load(Ordering::Acquire));
        assert_eq!(handle.read(&mut pcm), 0);
        assert_eq!(close_calls.load(Ordering::Relaxed), 1);

        // Second close + drop must not invoke on_close again.
        handle.close();
        drop(handle);
        assert_eq!(close_calls.load(Ordering::Relaxed), 1);

        // Dropping without explicit close still invokes on_close once.
        let drop_calls = Arc::new(AtomicU32::new(0));
        let counter = Arc::clone(&drop_calls);
        let handle2 = MicHandle {
            ring: Arc::new(CaptureRing::with_capacity_frames(4, 1)),
            channels: 1,
            rate: 16_000,
            closed: Arc::new(AtomicBool::new(false)),
            device_open: Arc::new(AtomicBool::new(true)),
            on_close: Mutex::new(Some(Box::new(move || {
                counter.fetch_add(1, Ordering::Relaxed);
            }))),
        };
        assert!(handle2.is_open());
        drop(handle2);
        assert_eq!(drop_calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn classify_error_message_mappings() {
        for msg in [
            "Permission denied",
            "ACCESS IS DENIED",
            "Not authorized to capture audio",
            "EPERM",
            "EACCES",
            "E_ACCESSDENIED",
            "HRESULT 0x80070005",
        ] {
            assert_eq!(classify_error_message(msg), MicRefusal::OsDenied, "{msg}");
        }
        for msg in [
            "Device or resource busy",
            "EBUSY",
            "Device already in use",
            "exclusive mode active",
            "AUDCLNT_E_DEVICE_IN_USE",
            "HRESULT 0x8889000a",
        ] {
            assert_eq!(classify_error_message(msg), MicRefusal::Busy, "{msg}");
        }
        for msg in ["unknown backend error", "cable unplugged", ""] {
            assert_eq!(
                classify_error_message(msg),
                MicRefusal::DeviceFailed,
                "{msg}"
            );
        }
    }

    #[test]
    fn disabled_mic_pool_behavior() {
        let pool = MicPool::new(false);
        assert_eq!(pool.devices(), Ok(Vec::new()));
        assert_eq!(
            pool.open(&MicSpec::new(1, 16_000)).err(),
            Some(MicRefusal::NoDevice)
        );
        assert_eq!(
            pool.open(&MicSpec::new(0, 16_000)).err(),
            Some(MicRefusal::BadSpec(MicSpecFault::Channels))
        );
        assert_eq!(
            pool.open(&MicSpec::new(1, 4_000)).err(),
            Some(MicRefusal::BadSpec(MicSpecFault::Rate))
        );
    }
}
