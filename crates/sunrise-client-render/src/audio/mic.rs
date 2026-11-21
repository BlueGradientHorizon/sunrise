//! Client-side microphone capture: bounded drop-oldest lock-free PCM ring, pure-Rust
//! channel/sample-rate converter, distinct endpoint resolution, and a dedicated per-stream
//! teardown thread that owns each `!Send` `cpal::Stream` locally.

#[cfg(feature = "audio")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
#[cfg(all(
    feature = "audio",
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd"
    )
))]
use std::collections::HashMap;
#[cfg(any(feature = "audio", test))]
use std::collections::HashMap as ActiveMap;
use std::sync::Arc;
#[cfg(any(feature = "audio", test))]
use std::sync::{
    Mutex, TryLockError,
    atomic::{AtomicBool, AtomicI16, AtomicU64, Ordering, fence},
    mpsc,
};
#[cfg(any(feature = "audio", test))]
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
/// allocation, never locks) and any consumer thread (`read_frames`, nonblocking via
/// `scratch.try_lock()` and seqlock validation against concurrent overwrites so unreturned
/// samples never touch the caller's slice).
#[cfg(any(feature = "audio", test))]
struct CaptureRing {
    slots: Box<[AtomicI16]>,
    channels: usize,
    cap_frames: usize,
    head: AtomicU64,
    writing_tail: AtomicU64,
    committed_tail: AtomicU64,
    dropped: AtomicU64,
    scratch: Mutex<Box<[i16]>>,
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
        let total_samples = cap_frames * channels;
        let slots = (0..total_samples)
            .map(|_| AtomicI16::new(0))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let scratch = vec![0_i16; total_samples].into_boxed_slice();
        Self {
            slots,
            channels,
            cap_frames,
            head: AtomicU64::new(0),
            writing_tail: AtomicU64::new(0),
            committed_tail: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            scratch: Mutex::new(scratch),
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

        // Announce write window before touching slots so concurrent readers detect overwrites.
        self.writing_tail.store(new_tail, Ordering::SeqCst);
        fence(Ordering::SeqCst);

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
    /// producer or on another concurrent reader. Only validated whole frames are copied into
    /// `out`; any suffix beyond `returned * channels` (and the entire slice on contention or
    /// empty read) is left untouched.
    fn read_frames(&self, out: &mut [i16]) -> u32 {
        self.read_frames_inner(out, |_| {})
    }

    fn read_frames_inner(&self, out: &mut [i16], mut before_validate: impl FnMut(usize)) -> u32 {
        let ch = self.channels;
        let max_frames = out.len() / ch;
        if max_frames == 0 {
            return 0;
        }
        let mut scratch = match self.scratch.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => return 0,
            Err(TryLockError::Poisoned(err)) => err.into_inner(),
        };
        let cap = self.cap_frames as u64;

        for attempt in 0..8 {
            let h = self.head.load(Ordering::SeqCst);
            let t = self.committed_tail.load(Ordering::SeqCst);
            if t <= h {
                return 0;
            }
            if t - h > cap {
                continue;
            }
            let avail = (t - h).min(max_frames as u64) as usize;
            let sample_count = avail * ch;
            for i in 0..avail {
                let slot_base = ((h + i as u64) % cap) as usize * ch;
                let dst_base = i * ch;
                for c in 0..ch {
                    scratch[dst_base + c] = self.slots[slot_base + c].load(Ordering::Relaxed);
                }
            }
            before_validate(attempt);
            fence(Ordering::SeqCst);
            let wt = self.writing_tail.load(Ordering::SeqCst);
            if wt.saturating_sub(cap) <= h
                && self
                    .head
                    .compare_exchange(h, h + avail as u64, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            {
                out[..sample_count].copy_from_slice(&scratch[..sample_count]);
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

#[cfg(any(feature = "audio", test))]
#[inline]
fn i16_to_f32(s: i16) -> f32 {
    f32::from(s) / 32768.0
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

    /// Passes native signed 16-bit PCM straight through unchanged whenever channels and sample
    /// rate already match, avoiding lossy float round-trips on full-scale samples.
    fn push_i16_samples(&mut self, samples: &[i16], ring: &CaptureRing) {
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
        if self.in_rate == self.out_rate && self.in_channels == self.out_channels {
            ring.push_frames(&samples[..in_frames * self.in_channels]);
            return;
        }
        self.push_samples(samples, ring, i16_to_f32);
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

#[cfg(any(feature = "audio", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PcmFormat {
    I8,
    I16,
    I24,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
}

#[cfg(any(feature = "audio", test))]
impl PcmFormat {
    fn rank(self) -> u8 {
        match self {
            Self::I16 => 3,
            Self::F32 => 2,
            Self::I24 | Self::I32 | Self::F64 => 1,
            Self::I8 | Self::I64 | Self::U8 | Self::U16 | Self::U32 | Self::U64 => 0,
        }
    }
}

#[cfg(any(feature = "audio", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InputConfigRange {
    channels: u16,
    min_rate: u32,
    max_rate: u32,
    format: PcmFormat,
}

#[cfg(any(feature = "audio", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InputConfig {
    channels: u16,
    rate: u32,
    format: PcmFormat,
}

#[cfg(any(feature = "audio", test))]
enum ProbeConfigs {
    Supported(Vec<InputConfigRange>),
    NotInput,
}

#[cfg(any(feature = "audio", test))]
trait BackendDevice: Clone + Send + 'static {
    fn label(&self) -> Option<String>;
    fn stable_selector(&self) -> Option<String>;
    fn same_endpoint(&self, other: &Self) -> bool;
    fn supported_input_configs(&self) -> Result<ProbeConfigs, mod_api::MicRefusal>;
    fn default_input_config(&self) -> Result<InputConfig, mod_api::MicRefusal>;
}

#[cfg(any(feature = "audio", test))]
trait CaptureBackend: Send + Sync + 'static {
    type Device: BackendDevice;
    type Stream;

    /// On macOS CoreAudio, opening at a sample rate other than the device's current nominal rate
    /// mutates the system-wide hardware clock under active streams. Open at `default_input_config`
    /// and resample locally instead.
    fn preserve_hardware_rate(&self) -> bool;

    /// Enumerates all backend devices without an error-erasing `supports_input()` filter.
    fn devices(&self) -> Result<Vec<Self::Device>, mod_api::MicRefusal>;

    fn default_input_device(&self) -> Option<Self::Device>;

    #[allow(clippy::too_many_arguments)]
    fn open_stream(
        &self,
        device: &Self::Device,
        config: InputConfig,
        spec: &mod_api::MicSpec,
        ring: Arc<CaptureRing>,
        closed: Arc<AtomicBool>,
        device_open: Arc<AtomicBool>,
        error_stop: mpsc::Sender<()>,
    ) -> Result<Self::Stream, mod_api::MicRefusal>;
}

#[cfg(any(feature = "audio", test))]
struct DeviceRegistry<D> {
    next_id: u64,
    entries: Vec<(String, D)>,
}

#[cfg(any(feature = "audio", test))]
impl<D: BackendDevice> DeviceRegistry<D> {
    fn new() -> Self {
        Self {
            next_id: 1,
            entries: Vec::new(),
        }
    }

    fn selector_for(&mut self, dev: &D) -> String {
        if let Some(stable) = dev.stable_selector() {
            return stable;
        }
        if let Some((id, _)) = self
            .entries
            .iter()
            .find(|(_, existing)| existing.same_endpoint(dev))
        {
            return id.clone();
        }
        let id = format!("input:{}", self.next_id);
        self.next_id += 1;
        self.entries.push((id.clone(), dev.clone()));
        id
    }

    fn lookup(&self, selector: &str) -> Option<D> {
        self.entries
            .iter()
            .find(|(id, _)| id == selector)
            .map(|(_, dev)| dev.clone())
    }
}

#[cfg(any(feature = "audio", test))]
fn choose_best_config(
    configs: &[InputConfigRange],
    spec: &mod_api::MicSpec,
) -> Option<InputConfig> {
    let target_channels = u16::from(spec.channels);
    let mut best: Option<((bool, bool, bool, u8, u32, u16), InputConfig)> = None;

    for range in configs {
        if range.channels == 0 || range.min_rate == 0 || range.max_rate < range.min_rate {
            continue;
        }
        let rate = spec.rate.clamp(range.min_rate, range.max_rate);
        let exact_rate = rate == spec.rate;
        let exact_channels = range.channels == target_channels;
        let enough_channels = range.channels >= target_channels;
        let format_rank = range.format.rank();
        let rate_closeness = u32::MAX - rate.abs_diff(spec.rate);
        let channel_closeness = u16::MAX - range.channels.abs_diff(target_channels);
        let key = (
            exact_rate,
            exact_channels,
            enough_channels,
            format_rank,
            rate_closeness,
            channel_closeness,
        );
        let cfg = InputConfig {
            channels: range.channels,
            rate,
            format: range.format,
        };
        if best.as_ref().is_none_or(|(best_key, _)| key > *best_key) {
            best = Some((key, cfg));
        }
    }
    best.map(|(_, cfg)| cfg)
}

#[cfg(any(feature = "audio", test))]
fn select_config_for_device<D: BackendDevice>(
    device: &D,
    spec: &mod_api::MicSpec,
    preserve_hardware_rate: bool,
) -> Result<InputConfig, mod_api::MicRefusal> {
    if preserve_hardware_rate {
        return device.default_input_config();
    }
    match device.supported_input_configs()? {
        ProbeConfigs::NotInput => Err(mod_api::MicRefusal::NoDevice),
        ProbeConfigs::Supported(ranges) => {
            choose_best_config(&ranges, spec).ok_or(mod_api::MicRefusal::DeviceFailed)
        }
    }
}

#[cfg(any(feature = "audio", test))]
fn list_devices_with<B: CaptureBackend>(
    backend: &B,
    registry: &Mutex<DeviceRegistry<B::Device>>,
) -> Result<Vec<mod_api::MicDevice>, mod_api::MicRefusal> {
    let devices = backend.devices()?;
    let mut found = Vec::new();
    let mut first_actionable_err = None;
    let mut reg = registry.lock().unwrap_or_else(|e| e.into_inner());

    for dev in devices {
        match dev.supported_input_configs() {
            Ok(ProbeConfigs::Supported(_)) => {
                if let Some(label) = dev.label()
                    && !label.is_empty()
                {
                    let id = reg.selector_for(&dev);
                    found.push(mod_api::MicDevice::new(id, label));
                }
            }
            Ok(ProbeConfigs::NotInput) | Err(mod_api::MicRefusal::NoDevice) => {}
            Err(mod_api::MicRefusal::OsDenied) => return Err(mod_api::MicRefusal::OsDenied),
            Err(err) => {
                first_actionable_err.get_or_insert(err);
            }
        }
    }
    if found.is_empty()
        && let Some(err) = first_actionable_err
    {
        return Err(err);
    }
    Ok(found)
}

#[cfg(any(feature = "audio", test))]
fn resolve_and_open_stream<B: CaptureBackend>(
    backend: &B,
    registry: &Mutex<DeviceRegistry<B::Device>>,
    spec: &mod_api::MicSpec,
    ring: Arc<CaptureRing>,
    closed: Arc<AtomicBool>,
    device_open: Arc<AtomicBool>,
    error_stop: mpsc::Sender<()>,
) -> Result<B::Stream, mod_api::MicRefusal> {
    let preserve_rate = backend.preserve_hardware_rate();
    let (device, config) = match spec.device.as_deref() {
        None => {
            let dev = backend
                .default_input_device()
                .ok_or(mod_api::MicRefusal::NoDevice)?;
            let cfg = select_config_for_device(&dev, spec, preserve_rate)?;
            (dev, cfg)
        }
        Some(target) => {
            let devices = backend.devices()?;
            let mut by_selector: Vec<B::Device> = devices
                .iter()
                .filter(|dev| dev.stable_selector().as_deref() == Some(target))
                .cloned()
                .collect();
            if !by_selector.is_empty() {
                drop(devices);
                if by_selector.len() > 1 {
                    return Err(mod_api::MicRefusal::AmbiguousDevice);
                }
                let dev = by_selector.remove(0);
                let cfg = select_config_for_device(&dev, spec, preserve_rate)?;
                (dev, cfg)
            } else if let Some(remembered) = registry
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .lookup(target)
            {
                let mut by_endpoint: Vec<B::Device> = devices
                    .into_iter()
                    .filter(|dev| dev.same_endpoint(&remembered))
                    .collect();
                if by_endpoint.is_empty() {
                    return Err(mod_api::MicRefusal::NoDevice);
                }
                if by_endpoint.len() > 1 {
                    return Err(mod_api::MicRefusal::AmbiguousDevice);
                }
                let dev = by_endpoint.remove(0);
                let cfg = select_config_for_device(&dev, spec, preserve_rate)?;
                (dev, cfg)
            } else {
                let by_label: Vec<B::Device> = devices
                    .into_iter()
                    .filter(|dev| dev.label().as_deref() == Some(target))
                    .collect();
                if by_label.is_empty() {
                    return Err(mod_api::MicRefusal::NoDevice);
                }
                let mut candidates = Vec::new();
                let mut first_actionable_err = None;
                for dev in by_label {
                    match dev.supported_input_configs() {
                        Ok(ProbeConfigs::Supported(ranges)) => candidates.push((dev, ranges)),
                        Ok(ProbeConfigs::NotInput) | Err(mod_api::MicRefusal::NoDevice) => {}
                        Err(mod_api::MicRefusal::OsDenied) => {
                            return Err(mod_api::MicRefusal::OsDenied);
                        }
                        Err(err) => {
                            first_actionable_err.get_or_insert(err);
                        }
                    }
                }
                if candidates.len() > 1 {
                    return Err(mod_api::MicRefusal::AmbiguousDevice);
                }
                if let Some((dev, ranges)) = candidates.pop() {
                    let cfg = if preserve_rate {
                        dev.default_input_config()?
                    } else {
                        choose_best_config(&ranges, spec)
                            .ok_or(mod_api::MicRefusal::DeviceFailed)?
                    };
                    (dev, cfg)
                } else {
                    return Err(first_actionable_err.unwrap_or(mod_api::MicRefusal::NoDevice));
                }
            }
        }
    };

    backend.open_stream(&device, config, spec, ring, closed, device_open, error_stop)
}

#[cfg(any(feature = "audio", test))]
struct StreamTeardown {
    stop_tx: mpsc::Sender<()>,
    join: thread::JoinHandle<()>,
}

/// Engine-side handle backing a mod's [`mod_api::MicRef`].
///
/// Never passed to the OS audio callback so calling `close()` or dropping `MicRef` signals the
/// stream's dedicated teardown thread and waits for the backend stream to be released before
/// returning.
#[cfg(any(feature = "audio", test))]
struct MicHandle {
    ring: Arc<CaptureRing>,
    channels: u8,
    rate: u32,
    closed: Arc<AtomicBool>,
    device_open: Arc<AtomicBool>,
    teardown: Mutex<Option<StreamTeardown>>,
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
        self.closed.store(true, Ordering::Release);
        self.device_open.store(false, Ordering::Release);
        let mut slot = self.teardown.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(teardown) = slot.take() {
            let _ = teardown.stop_tx.send(());
            let _ = teardown.join.join();
        }
    }
}

#[cfg(any(feature = "audio", test))]
impl Drop for MicHandle {
    fn drop(&mut self) {
        mod_api::MicSource::close(self);
    }
}

#[cfg(any(feature = "audio", test))]
type ActiveStreams = ActiveMap<u64, (Arc<AtomicBool>, mpsc::Sender<()>)>;

#[cfg(any(feature = "audio", test))]
struct MicPoolInner<B: CaptureBackend> {
    enabled: bool,
    backend: Arc<B>,
    registry: Arc<Mutex<DeviceRegistry<B::Device>>>,
    active: Arc<Mutex<ActiveStreams>>,
    shutdown: Arc<AtomicBool>,
    next_id: AtomicU64,
}

#[cfg(any(feature = "audio", test))]
impl<B: CaptureBackend> MicPoolInner<B> {
    fn new(enabled: bool, backend: Arc<B>) -> Self {
        Self {
            enabled,
            backend,
            registry: Arc::new(Mutex::new(DeviceRegistry::new())),
            active: Arc::new(Mutex::new(ActiveStreams::new())),
            shutdown: Arc::new(AtomicBool::new(false)),
            next_id: AtomicU64::new(1),
        }
    }

    fn devices(&self) -> Result<Vec<mod_api::MicDevice>, mod_api::MicRefusal> {
        if !self.enabled || self.shutdown.load(Ordering::Acquire) {
            return Ok(Vec::new());
        }
        list_devices_with(self.backend.as_ref(), &self.registry)
    }

    fn open(
        &self,
        spec: &mod_api::MicSpec,
    ) -> Result<Arc<dyn mod_api::MicSource>, mod_api::MicRefusal> {
        validate_spec(spec)?;
        if !self.enabled || self.shutdown.load(Ordering::Acquire) {
            return Err(mod_api::MicRefusal::NoDevice);
        }
        let ring = Arc::new(CaptureRing::new(spec.rate, spec.channels));
        let closed = Arc::new(AtomicBool::new(false));
        let device_open = Arc::new(AtomicBool::new(true));
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), mod_api::MicRefusal>>(1);
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let backend = Arc::clone(&self.backend);
        let registry = Arc::clone(&self.registry);
        let active = Arc::clone(&self.active);
        let shutdown = Arc::clone(&self.shutdown);
        let spec_owned = spec.clone();
        let ring_t = Arc::clone(&ring);
        let closed_t = Arc::clone(&closed);
        let dev_open_t = Arc::clone(&device_open);
        let err_stop_tx = stop_tx.clone();
        let shutdown_stop_tx = stop_tx.clone();

        let join = thread::Builder::new()
            .name("mic-stream".into())
            .spawn(move || {
                let opened = resolve_and_open_stream(
                    backend.as_ref(),
                    &registry,
                    &spec_owned,
                    ring_t,
                    closed_t,
                    Arc::clone(&dev_open_t),
                    err_stop_tx,
                );
                match opened {
                    Ok(stream) => {
                        let registered = match active.lock() {
                            Ok(mut map) => {
                                if shutdown.load(Ordering::Acquire) {
                                    false
                                } else {
                                    map.insert(id, (Arc::clone(&dev_open_t), shutdown_stop_tx));
                                    true
                                }
                            }
                            Err(_) => false,
                        };
                        if !registered {
                            dev_open_t.store(false, Ordering::Release);
                            drop(stream);
                            let _ = ready_tx.send(Err(mod_api::MicRefusal::NoDevice));
                            return;
                        }
                        let _ = ready_tx.send(Ok(()));
                        let _ = stop_rx.recv();
                        drop(stream);
                    }
                    Err(refusal) => {
                        dev_open_t.store(false, Ordering::Release);
                        let _ = ready_tx.send(Err(refusal));
                    }
                }
                dev_open_t.store(false, Ordering::Release);
                if let Ok(mut map) = active.lock() {
                    map.remove(&id);
                }
            })
            .map_err(|e| {
                log::warn!("Sunrise: failed to start mic-stream thread: {e}");
                mod_api::MicRefusal::DeviceFailed
            })?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Arc::new(MicHandle {
                ring,
                channels: spec.channels,
                rate: spec.rate,
                closed,
                device_open,
                teardown: Mutex::new(Some(StreamTeardown { stop_tx, join })),
            })),
            Ok(Err(refusal)) => {
                let _ = join.join();
                Err(refusal)
            }
            Err(_) => {
                let _ = join.join();
                Err(mod_api::MicRefusal::DeviceFailed)
            }
        }
    }
}

#[cfg(any(feature = "audio", test))]
impl<B: CaptureBackend> Drop for MicPoolInner<B> {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Ok(mut map) = self.active.lock() {
            for (_, (dev_open, stop_tx)) in map.drain() {
                dev_open.store(false, Ordering::Release);
                let _ = stop_tx.send(());
            }
        }
    }
}

/// Client microphone capture pool. Spawns no threads until a stream is opened.
pub(super) struct MicPool {
    #[cfg(feature = "audio")]
    inner: MicPoolInner<CpalBackend>,
}

impl MicPool {
    /// Creates a new capture pool without spawning any thread.
    pub(super) fn new(enabled: bool) -> Self {
        #[cfg(not(feature = "audio"))]
        let _ = enabled;
        Self {
            #[cfg(feature = "audio")]
            inner: MicPoolInner::new(enabled, Arc::new(CpalBackend)),
        }
    }

    /// Lists available audio input devices with distinct selectors and display labels.
    pub(super) fn devices(&self) -> Result<Vec<mod_api::MicDevice>, mod_api::MicRefusal> {
        #[cfg(not(feature = "audio"))]
        {
            Ok(Vec::new())
        }
        #[cfg(feature = "audio")]
        {
            self.inner.devices()
        }
    }

    /// Validates `spec` and opens a live microphone capture stream.
    pub(super) fn open(
        &self,
        spec: &mod_api::MicSpec,
    ) -> Result<Arc<dyn mod_api::MicSource>, mod_api::MicRefusal> {
        #[cfg(not(feature = "audio"))]
        {
            validate_spec(spec)?;
            Err(mod_api::MicRefusal::NoDevice)
        }
        #[cfg(feature = "audio")]
        {
            self.inner.open(spec)
        }
    }
}

#[cfg(feature = "audio")]
struct CpalBackend;

#[cfg(feature = "audio")]
#[derive(Clone)]
struct CpalDevice {
    device: cpal::Device,
    label: Option<String>,
    selector: Option<String>,
}

#[cfg(any(all(feature = "audio", target_os = "macos"), test))]
fn parse_coreaudio_device_id(debug: &str) -> Option<u32> {
    let rest = debug.split_once("audio_device_id:")?.1.trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

#[cfg(all(
    feature = "audio",
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd"
    )
))]
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(feature = "audio")]
fn pcm_format_from_cpal(format: cpal::SampleFormat) -> Option<PcmFormat> {
    use cpal::SampleFormat as F;
    Some(match format {
        F::I8 => PcmFormat::I8,
        F::I16 => PcmFormat::I16,
        F::I24 => PcmFormat::I24,
        F::I32 => PcmFormat::I32,
        F::I64 => PcmFormat::I64,
        F::U8 => PcmFormat::U8,
        F::U16 => PcmFormat::U16,
        F::U32 => PcmFormat::U32,
        F::U64 => PcmFormat::U64,
        F::F32 => PcmFormat::F32,
        F::F64 => PcmFormat::F64,
        _ => return None,
    })
}

#[cfg(feature = "audio")]
fn map_devices_error(err: cpal::DevicesError) -> mod_api::MicRefusal {
    match err {
        cpal::DevicesError::BackendSpecific { err } => classify_error_message(&err.description),
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

#[cfg(feature = "audio")]
impl BackendDevice for CpalDevice {
    fn label(&self) -> Option<String> {
        self.label.clone()
    }

    fn stable_selector(&self) -> Option<String> {
        self.selector.clone()
    }

    fn same_endpoint(&self, other: &Self) -> bool {
        #[cfg(target_os = "macos")]
        {
            if let (Some(a), Some(b)) = (&self.selector, &other.selector) {
                return a == b;
            }
            let (
                cpal::platform::DeviceInner::CoreAudio(a),
                cpal::platform::DeviceInner::CoreAudio(b),
            ) = (self.device.as_inner(), other.device.as_inner());
            a == b
        }
        #[cfg(windows)]
        {
            match (self.device.as_inner(), other.device.as_inner()) {
                (
                    cpal::platform::DeviceInner::Wasapi(a),
                    cpal::platform::DeviceInner::Wasapi(b),
                ) => a == b,
                #[allow(unreachable_patterns)]
                _ => self.selector.is_some() && self.selector == other.selector,
            }
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            self.selector.is_some() && self.selector == other.selector
        }
    }

    fn supported_input_configs(&self) -> Result<ProbeConfigs, mod_api::MicRefusal> {
        match self.device.supported_input_configs() {
            Ok(configs) => {
                let ranges: Vec<InputConfigRange> = configs
                    .filter_map(|r| {
                        Some(InputConfigRange {
                            channels: r.channels(),
                            min_rate: r.min_sample_rate().0,
                            max_rate: r.max_sample_rate().0,
                            format: pcm_format_from_cpal(r.sample_format())?,
                        })
                    })
                    .collect();
                if ranges.is_empty() {
                    Ok(ProbeConfigs::NotInput)
                } else {
                    Ok(ProbeConfigs::Supported(ranges))
                }
            }
            Err(cpal::SupportedStreamConfigsError::DeviceNotAvailable)
            | Err(cpal::SupportedStreamConfigsError::InvalidArgument) => {
                Err(mod_api::MicRefusal::NoDevice)
            }
            Err(cpal::SupportedStreamConfigsError::BackendSpecific { err }) => {
                Err(classify_error_message(&err.description))
            }
        }
    }

    fn default_input_config(&self) -> Result<InputConfig, mod_api::MicRefusal> {
        let cfg = self
            .device
            .default_input_config()
            .map_err(map_default_config_error)?;
        let format =
            pcm_format_from_cpal(cfg.sample_format()).ok_or(mod_api::MicRefusal::DeviceFailed)?;
        Ok(InputConfig {
            channels: cfg.channels(),
            rate: cfg.sample_rate().0,
            format,
        })
    }
}

#[cfg(feature = "audio")]
impl CaptureBackend for CpalBackend {
    type Device = CpalDevice;
    type Stream = cpal::Stream;

    fn preserve_hardware_rate(&self) -> bool {
        cfg!(target_os = "macos")
    }

    fn devices(&self) -> Result<Vec<Self::Device>, mod_api::MicRefusal> {
        let host = cpal::default_host();
        let raw_devices = host.devices().map_err(map_devices_error)?;
        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd"
        ))]
        let (cards_hash, mut occurrences) = {
            let cards = std::fs::read_to_string("/proc/asound/cards").unwrap_or_default();
            (fnv1a_64(cards.as_bytes()), HashMap::<String, usize>::new())
        };

        let mut out = Vec::new();
        for device in raw_devices {
            let label = device.name().ok().filter(|name| !name.is_empty());
            #[cfg(target_os = "macos")]
            let selector = {
                let cpal::platform::DeviceInner::CoreAudio(dev) = device.as_inner();
                parse_coreaudio_device_id(&format!("{dev:?}")).map(|id| format!("coreaudio:{id}"))
            };
            #[cfg(windows)]
            let selector = None;
            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd"
            ))]
            let selector = label.as_deref().map(|name| {
                const BUILTINS: &[&str] = &["default", "pipewire", "pulse", "jack", "oss"];
                let occ = *occurrences
                    .entry(name.to_owned())
                    .and_modify(|n| *n += 1)
                    .or_insert(0);
                if occ == 0 && BUILTINS.contains(&name) {
                    format!("alsa:builtin:{name}")
                } else {
                    format!("alsa:{cards_hash:016x}:{occ}:{name}")
                }
            });
            #[cfg(not(any(
                target_os = "macos",
                windows,
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd"
            )))]
            let selector = label.clone();

            out.push(CpalDevice {
                device,
                label,
                selector,
            });
        }
        Ok(out)
    }

    fn default_input_device(&self) -> Option<Self::Device> {
        let device = cpal::default_host().default_input_device()?;
        let label = device.name().ok().filter(|name| !name.is_empty());
        Some(CpalDevice {
            device,
            label,
            selector: None,
        })
    }

    fn open_stream(
        &self,
        device: &Self::Device,
        config: InputConfig,
        spec: &mod_api::MicSpec,
        ring: Arc<CaptureRing>,
        closed: Arc<AtomicBool>,
        device_open: Arc<AtomicBool>,
        error_stop: mpsc::Sender<()>,
    ) -> Result<Self::Stream, mod_api::MicRefusal> {
        let stream_config = cpal::StreamConfig {
            channels: config.channels,
            sample_rate: cpal::SampleRate(config.rate),
            buffer_size: cpal::BufferSize::Default,
        };
        let stream = match config.format {
            PcmFormat::I16 => build_i16_stream(
                &device.device,
                &stream_config,
                spec,
                ring,
                closed,
                device_open,
                error_stop,
            ),
            PcmFormat::I8 => build_typed_stream::<i8>(
                &device.device,
                &stream_config,
                spec,
                ring,
                closed,
                device_open,
                error_stop,
            ),
            PcmFormat::I24 => build_typed_stream::<cpal::I24>(
                &device.device,
                &stream_config,
                spec,
                ring,
                closed,
                device_open,
                error_stop,
            ),
            PcmFormat::I32 => build_typed_stream::<i32>(
                &device.device,
                &stream_config,
                spec,
                ring,
                closed,
                device_open,
                error_stop,
            ),
            PcmFormat::I64 => build_typed_stream::<i64>(
                &device.device,
                &stream_config,
                spec,
                ring,
                closed,
                device_open,
                error_stop,
            ),
            PcmFormat::U8 => build_typed_stream::<u8>(
                &device.device,
                &stream_config,
                spec,
                ring,
                closed,
                device_open,
                error_stop,
            ),
            PcmFormat::U16 => build_typed_stream::<u16>(
                &device.device,
                &stream_config,
                spec,
                ring,
                closed,
                device_open,
                error_stop,
            ),
            PcmFormat::U32 => build_typed_stream::<u32>(
                &device.device,
                &stream_config,
                spec,
                ring,
                closed,
                device_open,
                error_stop,
            ),
            PcmFormat::U64 => build_typed_stream::<u64>(
                &device.device,
                &stream_config,
                spec,
                ring,
                closed,
                device_open,
                error_stop,
            ),
            PcmFormat::F32 => build_typed_stream::<f32>(
                &device.device,
                &stream_config,
                spec,
                ring,
                closed,
                device_open,
                error_stop,
            ),
            PcmFormat::F64 => build_typed_stream::<f64>(
                &device.device,
                &stream_config,
                spec,
                ring,
                closed,
                device_open,
                error_stop,
            ),
        }
        .map_err(map_build_stream_error)?;
        stream.play().map_err(map_play_stream_error)?;
        Ok(stream)
    }
}

#[cfg(feature = "audio")]
#[allow(clippy::too_many_arguments)]
fn build_i16_stream(
    device: &cpal::Device,
    stream_config: &cpal::StreamConfig,
    spec: &mod_api::MicSpec,
    ring: Arc<CaptureRing>,
    closed: Arc<AtomicBool>,
    device_open: Arc<AtomicBool>,
    error_stop: mpsc::Sender<()>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let mut converter = InputConverter::new(
        stream_config.channels,
        stream_config.sample_rate.0,
        spec.channels,
        spec.rate,
    );
    let err_open = Arc::clone(&device_open);
    device.build_input_stream(
        stream_config,
        move |data: &[i16], _info: &cpal::InputCallbackInfo| {
            if closed.load(Ordering::Relaxed) || !device_open.load(Ordering::Relaxed) {
                return;
            }
            converter.push_i16_samples(data, &ring);
        },
        move |e| {
            err_open.store(false, Ordering::Release);
            let _ = error_stop.send(());
            log::warn!("Sunrise: microphone stream error: {e}");
        },
        None,
    )
}

#[cfg(feature = "audio")]
#[allow(clippy::too_many_arguments)]
fn build_typed_stream<T>(
    device: &cpal::Device,
    stream_config: &cpal::StreamConfig,
    spec: &mod_api::MicSpec,
    ring: Arc<CaptureRing>,
    closed: Arc<AtomicBool>,
    device_open: Arc<AtomicBool>,
    error_stop: mpsc::Sender<()>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let mut converter = InputConverter::new(
        stream_config.channels,
        stream_config.sample_rate.0,
        spec.channels,
        spec.rate,
    );
    let err_open = Arc::clone(&device_open);
    device.build_input_stream(
        stream_config,
        move |data: &[T], _info: &cpal::InputCallbackInfo| {
            if closed.load(Ordering::Relaxed) || !device_open.load(Ordering::Relaxed) {
                return;
            }
            converter.push_cpal_samples(data, &ring);
        },
        move |e| {
            err_open.store(false, Ordering::Release);
            let _ = error_stop.send(());
            log::warn!("Sunrise: microphone stream error: {e}");
        },
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{MicDevice, MicRefusal, MicSource, MicSpec, MicSpecFault};
    use std::sync::Barrier;
    use std::time::{Duration, Instant};

    #[derive(Clone)]
    struct FakeDevice {
        endpoint_id: String,
        expose_selector: bool,
        label: String,
        supported: Result<Vec<InputConfigRange>, MicRefusal>,
        default_cfg: Result<InputConfig, MicRefusal>,
    }

    impl FakeDevice {
        fn input(endpoint_id: &str, label: &str, rate: u32, format: PcmFormat) -> Self {
            Self {
                endpoint_id: endpoint_id.to_owned(),
                expose_selector: true,
                label: label.to_owned(),
                supported: Ok(vec![InputConfigRange {
                    channels: 1,
                    min_rate: 8_000,
                    max_rate: 48_000,
                    format,
                }]),
                default_cfg: Ok(InputConfig {
                    channels: 1,
                    rate,
                    format,
                }),
            }
        }
    }

    impl BackendDevice for FakeDevice {
        fn label(&self) -> Option<String> {
            Some(self.label.clone())
        }

        fn stable_selector(&self) -> Option<String> {
            self.expose_selector.then(|| self.endpoint_id.clone())
        }

        fn same_endpoint(&self, other: &Self) -> bool {
            self.endpoint_id == other.endpoint_id
        }

        fn supported_input_configs(&self) -> Result<ProbeConfigs, MicRefusal> {
            match &self.supported {
                Ok(ranges) if ranges.is_empty() => Ok(ProbeConfigs::NotInput),
                Ok(ranges) => Ok(ProbeConfigs::Supported(ranges.clone())),
                Err(err) => Err(*err),
            }
        }

        fn default_input_config(&self) -> Result<InputConfig, MicRefusal> {
            self.default_cfg
        }
    }

    struct FakeStream {
        disposed: Arc<AtomicBool>,
    }

    impl Drop for FakeStream {
        fn drop(&mut self) {
            self.disposed.store(true, Ordering::Release);
        }
    }

    struct OpenedStreamRecord {
        endpoint_id: String,
        config: InputConfig,
        disposed: Arc<AtomicBool>,
        ring: Arc<CaptureRing>,
        device_open: Arc<AtomicBool>,
        error_stop: mpsc::Sender<()>,
    }

    struct FakeBackendState {
        devices: Vec<FakeDevice>,
        default_device: Option<FakeDevice>,
        devices_error: Option<MicRefusal>,
        preserve_hardware_rate: bool,
        opened: Vec<OpenedStreamRecord>,
        devices_barrier: Option<Arc<Barrier>>,
    }

    struct FakeBackend {
        state: Mutex<FakeBackendState>,
    }

    impl FakeBackend {
        fn new(devices: Vec<FakeDevice>, default_device: Option<FakeDevice>) -> Self {
            Self {
                state: Mutex::new(FakeBackendState {
                    devices,
                    default_device,
                    devices_error: None,
                    preserve_hardware_rate: false,
                    opened: Vec::new(),
                    devices_barrier: None,
                }),
            }
        }
    }

    impl CaptureBackend for FakeBackend {
        type Device = FakeDevice;
        type Stream = FakeStream;

        fn preserve_hardware_rate(&self) -> bool {
            self.state.lock().unwrap().preserve_hardware_rate
        }

        fn devices(&self) -> Result<Vec<Self::Device>, MicRefusal> {
            let barrier = self.state.lock().unwrap().devices_barrier.clone();
            if let Some(barrier) = barrier {
                barrier.wait();
                barrier.wait();
            }
            let st = self.state.lock().unwrap();
            if let Some(err) = st.devices_error {
                return Err(err);
            }
            Ok(st.devices.clone())
        }

        fn default_input_device(&self) -> Option<Self::Device> {
            self.state.lock().unwrap().default_device.clone()
        }

        fn open_stream(
            &self,
            device: &Self::Device,
            config: InputConfig,
            _spec: &MicSpec,
            ring: Arc<CaptureRing>,
            _closed: Arc<AtomicBool>,
            device_open: Arc<AtomicBool>,
            error_stop: mpsc::Sender<()>,
        ) -> Result<Self::Stream, MicRefusal> {
            let disposed = Arc::new(AtomicBool::new(false));
            self.state.lock().unwrap().opened.push(OpenedStreamRecord {
                endpoint_id: device.endpoint_id.clone(),
                config,
                disposed: Arc::clone(&disposed),
                ring,
                device_open,
                error_stop,
            });
            Ok(FakeStream { disposed })
        }
    }

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

        ring.push_frames(&[10, 20]);
        ring.push_frames(&[30, 40, 50, 60, 70, 80]);
        assert_eq!(ring.dropped_frames(), 6);
        assert_eq!(ring.read_frames(&mut out), 4);
        assert_eq!(out, [50, 60, 70, 80]);
    }

    #[test]
    fn capture_ring_reader_contention_and_overwrite_leave_suffix_untouched() {
        const SENTINEL: i16 = 0x5A5A;

        // 1. Reader contention is nonblocking and never modifies the caller's slice.
        let ring = CaptureRing::with_capacity_frames(8, 2);
        ring.push_frames(&[11, -11, 22, -22]);
        {
            let _held = ring.scratch.lock().unwrap();
            let mut out = [SENTINEL; 6];
            assert_eq!(ring.read_frames(&mut out), 0);
            assert_eq!(out, [SENTINEL; 6]);
        }

        // 2. Overwrite retry that exhausts attempts returns 0 and leaves the entire slice untouched.
        let ring = CaptureRing::with_capacity_frames(4, 1);
        ring.push_frames(&[1, 2, 3, 4]);
        let mut out = [SENTINEL; 4];
        let count = ring.read_frames_inner(&mut out, |attempt| {
            let base = (attempt as i16 + 1) * 10;
            ring.push_frames(&[base, base + 1, base + 2, base + 3]);
        });
        assert_eq!(count, 0);
        assert_eq!(out, [SENTINEL; 4]);

        // 3. Overwrite retry that succeeds on a shorter prefix leaves the suffix completely untouched.
        let ring = CaptureRing::with_capacity_frames(4, 1);
        ring.push_frames(&[1, 2, 3, 4]);
        let mut out = [SENTINEL; 4];
        let count = ring.read_frames_inner(&mut out, |attempt| {
            if attempt == 0 {
                // Force first 4-frame attempt to fail overwrite validation, then leave only 2 new
                // frames unread by advancing head through an overflow + draining 2 frames.
                ring.push_frames(&[10, 20, 30, 40]);
                let _ = ring.head.fetch_add(2, Ordering::SeqCst);
            }
        });
        assert_eq!(count, 2);
        assert_eq!(out, [30, 40, SENTINEL, SENTINEL]);
    }

    #[test]
    fn capture_ring_concurrent_monotonic_tear_free() {
        const TOTAL_FRAMES: u64 = 20_000;
        const SENTINEL: i16 = i16::MIN;
        let ring = Arc::new(CaptureRing::with_capacity_frames(32, 2));
        let done = Arc::new(AtomicBool::new(false));

        let prod_ring = Arc::clone(&ring);
        let prod_done = Arc::clone(&done);
        let producer = std::thread::spawn(move || {
            let mut seq = 1_i16;
            let mut sent = 0_u64;
            let mut batch = [0_i16; 14];
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
        loop {
            let mut buf = [SENTINEL; 25]; // 12 stereo frames + 1 trailing sample
            let n = ring.read_frames(&mut buf) as usize;
            assert!(
                buf[n * 2..].iter().all(|&s| s == SENTINEL),
                "unreturned suffix must remain untouched"
            );
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
                let mut tail_buf = [SENTINEL; 24];
                let tail = ring.read_frames(&mut tail_buf) as usize;
                assert!(tail_buf[tail * 2..].iter().all(|&s| s == SENTINEL));
                for i in 0..tail {
                    let l = tail_buf[i * 2];
                    let r = tail_buf[i * 2 + 1];
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
    fn input_converter_native_i16_passthrough_preserves_full_scale() {
        // Matching mono i16 -> bit-exact passthrough including both full-scale endpoints.
        let ring = CaptureRing::with_capacity_frames(16, 1);
        let mut conv = InputConverter::new(1, 48_000, 1, 48_000);
        let input = [i16::MIN, -32_767, -1, 0, 1, 32_766, i16::MAX];
        conv.push_i16_samples(&input, &ring);
        let mut out = [0_i16; 8];
        assert_eq!(ring.read_frames(&mut out), 7);
        assert_eq!(&out[..7], &input);

        // Matching stereo i16 -> bit-exact passthrough including both full-scale endpoints.
        let ring = CaptureRing::with_capacity_frames(16, 2);
        let mut conv = InputConverter::new(2, 16_000, 2, 16_000);
        let stereo = [i16::MIN, i16::MAX, -1, 1];
        conv.push_i16_samples(&stereo, &ring);
        let mut out_stereo = [0_i16; 4];
        assert_eq!(ring.read_frames(&mut out_stereo), 2);
        assert_eq!(out_stereo, stereo);
    }

    #[test]
    fn input_converter_channel_mapping_and_clamping() {
        let ring = CaptureRing::with_capacity_frames(16, 1);
        let mut conv = InputConverter::new(2, 16_000, 1, 16_000);
        conv.push_f32_samples(&[1.0, 0.0, -0.5, 0.5, -1.0, -1.0], &ring);
        let mut mono = [0_i16; 4];
        assert_eq!(ring.read_frames(&mut mono), 3);
        assert_eq!(&mono[..3], &[16_384, 0, -32_767]);

        let ring = CaptureRing::with_capacity_frames(16, 2);
        let mut conv = InputConverter::new(1, 16_000, 2, 16_000);
        conv.push_f32_samples(&[0.25, -0.5], &ring);
        let mut stereo = [0_i16; 4];
        assert_eq!(ring.read_frames(&mut stereo), 2);
        assert_eq!(stereo, [8_192, 8_192, -16_384, -16_384]);

        let ring = CaptureRing::with_capacity_frames(16, 1);
        let mut conv = InputConverter::new(4, 16_000, 1, 16_000);
        conv.push_f32_samples(&[0.4, 0.4, 0.0, 0.0], &ring);
        let mut out1 = [0_i16; 2];
        assert_eq!(ring.read_frames(&mut out1), 1);
        assert_eq!(out1[0], 6_553);

        let ring = CaptureRing::with_capacity_frames(16, 2);
        let mut conv = InputConverter::new(4, 16_000, 2, 16_000);
        conv.push_f32_samples(&[0.5, -0.25, 1.0, -1.0], &ring);
        let mut out2 = [0_i16; 2];
        assert_eq!(ring.read_frames(&mut out2), 1);
        assert_eq!(out2, [16_384, -8_192]);

        let ring = CaptureRing::with_capacity_frames(16, 1);
        let mut conv = InputConverter::new(1, 16_000, 1, 16_000);
        conv.push_f32_samples(&[2.5, -3.0, 1.0, -1.0], &ring);
        let mut clamped = [0_i16; 4];
        assert_eq!(ring.read_frames(&mut clamped), 4);
        assert_eq!(clamped, [32_767, -32_767, 32_767, -32_767]);
    }

    #[test]
    fn input_converter_resampling_continuity_across_chunks() {
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
    fn pcm_format_ranking_prefers_native_i16_then_f32() {
        assert!(PcmFormat::I16.rank() > PcmFormat::F32.rank());
        assert!(PcmFormat::F32.rank() > PcmFormat::I24.rank());
        assert_eq!(PcmFormat::I24.rank(), PcmFormat::I32.rank());
        assert_eq!(PcmFormat::I32.rank(), PcmFormat::F64.rank());
        assert!(PcmFormat::F64.rank() > PcmFormat::I8.rank());
        for fmt in [
            PcmFormat::I8,
            PcmFormat::I64,
            PcmFormat::U8,
            PcmFormat::U16,
            PcmFormat::U32,
            PcmFormat::U64,
        ] {
            assert_eq!(fmt.rank(), 0);
        }
        let chosen = choose_best_config(
            &[
                InputConfigRange {
                    channels: 1,
                    min_rate: 8_000,
                    max_rate: 48_000,
                    format: PcmFormat::F32,
                },
                InputConfigRange {
                    channels: 1,
                    min_rate: 8_000,
                    max_rate: 48_000,
                    format: PcmFormat::I16,
                },
            ],
            &MicSpec::new(1, 48_000),
        );
        assert_eq!(
            chosen,
            Some(InputConfig {
                channels: 1,
                rate: 48_000,
                format: PcmFormat::I16,
            })
        );
    }

    #[test]
    fn denied_backend_preserves_os_denied_for_list_named_and_default_open() {
        let denied = FakeDevice {
            endpoint_id: "alsa:usb-mic".into(),
            expose_selector: true,
            label: "USB Mic".into(),
            supported: Err(MicRefusal::OsDenied),
            // Even if default_input_config would succeed, it must not mask OsDenied.
            default_cfg: Ok(InputConfig {
                channels: 1,
                rate: 48_000,
                format: PcmFormat::F32,
            }),
        };
        let output_only = FakeDevice {
            endpoint_id: "alsa:hdmi".into(),
            expose_selector: true,
            label: "HDMI Out".into(),
            supported: Ok(Vec::new()),
            default_cfg: Err(MicRefusal::DeviceFailed),
        };
        let backend = Arc::new(FakeBackend::new(
            vec![output_only, denied.clone()],
            Some(denied),
        ));
        let pool = MicPoolInner::new(true, backend);

        assert_eq!(pool.devices(), Err(MicRefusal::OsDenied));
        assert_eq!(
            pool.open(&MicSpec::new(1, 16_000)).err(),
            Some(MicRefusal::OsDenied)
        );
        assert_eq!(
            pool.open(&MicSpec::new(1, 16_000).with_device("USB Mic"))
                .err(),
            Some(MicRefusal::OsDenied)
        );
        assert_eq!(
            pool.open(&MicSpec::new(1, 16_000).with_device("alsa:usb-mic"))
                .err(),
            Some(MicRefusal::OsDenied)
        );
    }

    #[test]
    fn duplicate_labels_keep_distinct_selectors_and_refuse_vanished_or_ambiguous() {
        let mic_a = FakeDevice {
            expose_selector: false, // Exercise endpoint-equality selector registry (WASAPI-like).
            ..FakeDevice::input("usb-port-1", "USB Microphone", 48_000, PcmFormat::I16)
        };
        let mic_b = FakeDevice {
            expose_selector: false,
            ..FakeDevice::input("usb-port-2", "USB Microphone", 48_000, PcmFormat::I16)
        };
        let backend = Arc::new(FakeBackend::new(
            vec![mic_a.clone(), mic_b.clone()],
            Some(mic_a.clone()),
        ));
        let pool = MicPoolInner::new(true, Arc::clone(&backend));

        let listed = pool.devices().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].label, "USB Microphone");
        assert_eq!(listed[1].label, "USB Microphone");
        assert_ne!(listed[0].id, listed[1].id);

        // Passing the ambiguous display label refuses instead of opening whichever is first.
        assert_eq!(
            pool.open(&MicSpec::new(1, 48_000).with_device("USB Microphone"))
                .err(),
            Some(MicRefusal::AmbiguousDevice)
        );

        // Reorder devices in the backend: selector for mic B still resolves to usb-port-2.
        backend.state.lock().unwrap().devices = vec![mic_b.clone(), mic_a.clone()];
        let handle_b = pool
            .open(&MicSpec::new(1, 48_000).with_device(&listed[1].id))
            .unwrap();
        assert_eq!(
            backend
                .state
                .lock()
                .unwrap()
                .opened
                .last()
                .unwrap()
                .endpoint_id,
            "usb-port-2"
        );
        handle_b.close();

        // Unplug mic B: opening its selector refuses as vanished rather than opening mic A.
        backend.state.lock().unwrap().devices = vec![mic_a];
        assert_eq!(
            pool.open(&MicSpec::new(1, 48_000).with_device(&listed[1].id))
                .err(),
            Some(MicRefusal::NoDevice)
        );
    }

    #[test]
    fn coreaudio_policy_preserves_hardware_rate_across_readers() {
        let dev = FakeDevice::input("coreaudio:42", "Built-in Mic", 48_000, PcmFormat::F32);
        let backend = Arc::new(FakeBackend::new(vec![dev.clone()], Some(dev)));
        backend.state.lock().unwrap().preserve_hardware_rate = true;
        let pool = MicPoolInner::new(true, Arc::clone(&backend));

        let mic_16k = pool.open(&MicSpec::new(1, 16_000)).unwrap();
        let mic_44k = pool.open(&MicSpec::new(1, 44_100)).unwrap();
        assert_eq!(mic_16k.rate(), 16_000);
        assert_eq!(mic_44k.rate(), 44_100);

        {
            let st = backend.state.lock().unwrap();
            assert_eq!(st.opened.len(), 2);
            assert_eq!(st.opened[0].config.rate, 48_000);
            assert_eq!(st.opened[1].config.rate, 48_000);
        }

        mic_16k.close();
        mic_44k.close();
    }

    #[test]
    fn close_drop_and_backend_error_dispose_stream_promptly_without_probe_blocking() {
        let dev = FakeDevice::input("mic-1", "Desk Mic", 48_000, PcmFormat::I16);
        let backend = Arc::new(FakeBackend::new(vec![dev.clone()], Some(dev)));
        let pool = Arc::new(MicPoolInner::new(true, Arc::clone(&backend)));

        // 1. Explicit close disposes backend stream before returning, even while devices() is blocked.
        let mic1 = pool.open(&MicSpec::new(1, 48_000)).unwrap();
        let disposed1 = Arc::clone(&backend.state.lock().unwrap().opened[0].disposed);
        assert!(!disposed1.load(Ordering::Acquire));

        let barrier = Arc::new(Barrier::new(2));
        backend.state.lock().unwrap().devices_barrier = Some(Arc::clone(&barrier));
        let pool_probe = Arc::clone(&pool);
        let probe_thread = thread::spawn(move || pool_probe.devices());
        // Wait until probe_thread is actively inside backend.devices().
        barrier.wait();

        mic1.close();
        assert!(
            disposed1.load(Ordering::Acquire),
            "close() must release backend stream before returning even during a blocked probe"
        );
        assert!(!mic1.is_open());

        // Release the blocked probe.
        barrier.wait();
        let listed = probe_thread.join().unwrap().unwrap();
        assert_eq!(
            listed,
            vec![MicDevice::new("mic-1".into(), "Desk Mic".into())]
        );
        backend.state.lock().unwrap().devices_barrier = None;

        // 2. Dropping the handle disposes the backend stream before drop returns.
        let mic2 = pool.open(&MicSpec::new(1, 48_000)).unwrap();
        let disposed2 = Arc::clone(&backend.state.lock().unwrap().opened[1].disposed);
        assert!(!disposed2.load(Ordering::Acquire));
        drop(mic2);
        assert!(disposed2.load(Ordering::Acquire));

        // 3. Backend stream error callback disposes the stream promptly without close/drop/open.
        let mic3 = pool.open(&MicSpec::new(1, 48_000)).unwrap();
        let (disposed3, dev_open3, err_stop3, ring3) = {
            let st = backend.state.lock().unwrap();
            let rec = &st.opened[2];
            (
                Arc::clone(&rec.disposed),
                Arc::clone(&rec.device_open),
                rec.error_stop.clone(),
                Arc::clone(&rec.ring),
            )
        };
        ring3.push_frames(&[123]);
        let mut pcm = [0_i16; 1];
        assert_eq!(mic3.read(&mut pcm), 1);
        assert_eq!(pcm, [123]);

        // Simulate OS device disconnect error callback.
        dev_open3.store(false, Ordering::Release);
        err_stop3.send(()).unwrap();
        assert!(!mic3.is_open());

        let deadline = Instant::now() + Duration::from_secs(2);
        while !disposed3.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            disposed3.load(Ordering::Acquire),
            "backend error callback must release the failed stream promptly"
        );
        // Subsequent close/drop after error disposal is a safe no-op.
        mic3.close();
        drop(mic3);
    }

    #[test]
    fn parse_coreaudio_device_id_extracts_numeric_id() {
        assert_eq!(
            parse_coreaudio_device_id(
                "Device { audio_device_id: 47, name: Ok(\"USB Microphone\") }"
            ),
            Some(47)
        );
        assert_eq!(
            parse_coreaudio_device_id("Device { name: Ok(\"x\") }"),
            None
        );
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
