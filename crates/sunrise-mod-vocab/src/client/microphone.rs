//! A client mod's microphone capture handle: [`MicRef`] reads whole PCM frames from a bounded
//! client-local ring filled off the OS audio input callback.
use std::sync::Arc;

/// The lowest sample rate microphone capture accepts, in Hz.
pub const MIC_RATE_MIN: u32 = 8_000;
/// The highest sample rate microphone capture accepts, in Hz.
pub const MIC_RATE_MAX: u32 = 48_000;
/// The capacity of a microphone capture ring, in milliseconds of audio.
pub const MIC_RING_MAX_MS: u32 = 500;

/// The input device and PCM format requested when opening microphone capture on this client.
///
/// ```compile_fail,E0277
/// fn requires_wire<T: mod_api::wire::Wire>() {}
/// requires_wire::<mod_api::MicSpec>();
/// ```
// no wire: client-local microphone capture specification, never crosses the mod wire
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct MicSpec {
    /// Input device name from `microphone_devices()`, or `None` to select the system default
    /// input device.
    pub device: Option<String>,
    /// Channel count: `1` for mono or `2` for stereo.
    pub channels: u8,
    /// Sample rate in Hz, from [`MIC_RATE_MIN`] to [`MIC_RATE_MAX`].
    pub rate: u32,
}

impl MicSpec {
    /// A specification for the system default input device with `channels` (`1` or `2`) at `rate`
    /// Hz.
    pub fn new(channels: u8, rate: u32) -> Self {
        Self {
            device: None,
            channels,
            rate,
        }
    }

    /// The same specification targeting the named input `device` from `microphone_devices()`.
    pub fn with_device(self, device: impl Into<String>) -> Self {
        Self {
            device: Some(device.into()),
            ..self
        }
    }
}

/// What the client found wrong with a [`MicSpec`]. This is a local validation result.
///
/// ```compile_fail,E0277
/// fn requires_wire<T: mod_api::wire::Wire>() {}
/// requires_wire::<mod_api::MicSpecFault>();
/// ```
// no wire: microphone spec validation is local to the client
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MicSpecFault {
    /// `channels` is not 1 or 2.
    Channels,
    /// `rate` is outside [`MIC_RATE_MIN`] to [`MIC_RATE_MAX`].
    Rate,
}

impl std::fmt::Display for MicSpecFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Channels => "channels is not 1 or 2",
            Self::Rate => "rate is outside 8000 to 48000 Hz",
        })
    }
}

/// Why a request to list or open microphone capture was refused. This is a local result, not a
/// network error.
///
/// ```compile_fail,E0277
/// fn requires_wire<T: mod_api::wire::Wire>() {}
/// requires_wire::<mod_api::MicRefusal>();
/// ```
// no wire: microphone refusals report local capture or host conditions
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MicRefusal {
    /// This is a server: it has no microphone.
    OnServer,
    /// Called outside `ClientSetup` or later, or the host has no client audio.
    Unavailable,
    /// The capture specification has an invalid `channels` or `rate`.
    BadSpec(MicSpecFault),
    /// No default input device exists, the named device was not found, or client audio is
    /// disabled.
    NoDevice,
    /// The operating system denied microphone permission.
    OsDenied,
    /// The input device is busy or held in exclusive use.
    Busy,
    /// The input device or capture worker failed to open or start.
    DeviceFailed,
}

impl std::fmt::Display for MicRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OnServer => f.write_str("a server has no microphone"),
            Self::Unavailable => f.write_str("microphone capture is unavailable in this phase"),
            Self::BadSpec(fault) => write!(f, "the microphone spec is refused: {fault}"),
            Self::NoDevice => f.write_str("no matching microphone device is available"),
            Self::OsDenied => f.write_str("the operating system denied microphone permission"),
            Self::Busy => f.write_str("the microphone device is busy"),
            Self::DeviceFailed => f.write_str("the microphone device failed to open or start"),
        }
    }
}

impl std::error::Error for MicRefusal {}

impl crate::AccessRefusal for MicRefusal {
    fn unavailable() -> Self {
        Self::Unavailable
    }
}

/// The engine's side of one open microphone capture stream. A mod reaches it through [`MicRef`],
/// not directly. Every method works from any thread without blocking on a tick or OS callback.
// no wire: engine-side capture source reached in-process through MicRef
pub trait MicSource: Send + Sync + 'static {
    /// Reads whole interleaved signed 16-bit PCM frames into `pcm` without blocking or waiting on
    /// a tick. Any trailing incomplete frame in `pcm` is left untouched. Returns the number of
    /// whole frames written, or `0` when no frames are ready or after [`close`](Self::close).
    fn read(&self, pcm: &mut [i16]) -> u32;
    /// Channel count of the captured PCM (`1` for mono or `2` for stereo).
    fn channels(&self) -> u8;
    /// Sample rate of the captured PCM, in Hz.
    fn rate(&self) -> u32;
    /// Cumulative captured frames dropped from the bounded [`MIC_RING_MAX_MS`] ms ring because
    /// the reader fell behind.
    fn dropped_frames(&self) -> u64;
    /// Whether capture is still active; `false` after [`close`](Self::close) or if the OS input
    /// device disconnects or errors.
    fn is_open(&self) -> bool;
    /// Stops capture for good. Idempotent.
    fn close(&self);
}

/// An owning handle to a live microphone capture stream on this client.
///
/// Read captured PCM frames from any thread with [`read`](Self::read) without waiting for a tick
/// or running on the OS audio callback. Capture stops immediately when [`close`](Self::close) is
/// called or when the handle is dropped.
///
/// ```compile_fail,E0277
/// fn requires_wire<T: mod_api::wire::Wire>() {}
/// requires_wire::<mod_api::MicRef>();
/// ```
// no wire: the handle reads live PCM from a microphone on this client
// exhaustive: its field is private, so mods cannot build or destructure it
pub struct MicRef {
    source: Arc<dyn MicSource>,
}

impl MicRef {
    /// A handle on the engine's `source`.
    // hidden: a game-side constructor, not part of the mod surface
    #[doc(hidden)]
    pub fn new(source: Arc<dyn MicSource>) -> Self {
        Self { source }
    }

    /// Reads whole interleaved signed 16-bit PCM frames into `pcm` without blocking or waiting on
    /// a tick. Any trailing incomplete frame in `pcm` is left untouched. Returns the number of
    /// whole frames written, or `0` when no frames are ready or after [`close`](Self::close).
    pub fn read(&self, pcm: &mut [i16]) -> u32 {
        self.source.read(pcm)
    }

    /// Channel count of the captured PCM (`1` for mono or `2` for stereo).
    pub fn channels(&self) -> u8 {
        self.source.channels()
    }

    /// Sample rate of the captured PCM, in Hz.
    pub fn rate(&self) -> u32 {
        self.source.rate()
    }

    /// Cumulative captured frames dropped from the bounded [`MIC_RING_MAX_MS`] ms ring because
    /// the reader fell behind.
    pub fn dropped_frames(&self) -> u64 {
        self.source.dropped_frames()
    }

    /// Whether capture is still active; `false` after [`close`](Self::close) or if the OS input
    /// device disconnects or errors.
    pub fn is_open(&self) -> bool {
        self.source.is_open()
    }

    /// Stops capture for good. Idempotent, and also called automatically on drop.
    pub fn close(&self) {
        self.source.close();
    }
}

impl Drop for MicRef {
    fn drop(&mut self) {
        self.source.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AccessRefusal;
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    };

    struct StubMicSource {
        channels: u8,
        rate: u32,
        dropped: AtomicU64,
        open: AtomicBool,
        closes: AtomicU32,
        samples: Mutex<Vec<i16>>,
    }

    impl StubMicSource {
        fn new(channels: u8, rate: u32, dropped: u64, samples: Vec<i16>) -> Self {
            Self {
                channels,
                rate,
                dropped: AtomicU64::new(dropped),
                open: AtomicBool::new(true),
                closes: AtomicU32::new(0),
                samples: Mutex::new(samples),
            }
        }
    }

    impl MicSource for StubMicSource {
        fn read(&self, pcm: &mut [i16]) -> u32 {
            if !self.open.load(Ordering::Acquire) {
                return 0;
            }
            let channels = usize::from(self.channels);
            if channels == 0 {
                return 0;
            }
            let mut queued = self.samples.lock().unwrap();
            let frames = (pcm.len() / channels).min(queued.len() / channels);
            let sample_count = frames * channels;
            for (dst, src) in pcm[..sample_count]
                .iter_mut()
                .zip(queued.drain(..sample_count))
            {
                *dst = src;
            }
            frames as u32
        }

        fn channels(&self) -> u8 {
            self.channels
        }

        fn rate(&self) -> u32 {
            self.rate
        }

        fn dropped_frames(&self) -> u64 {
            self.dropped.load(Ordering::Relaxed)
        }

        fn is_open(&self) -> bool {
            self.open.load(Ordering::Acquire)
        }

        fn close(&self) {
            self.open.store(false, Ordering::Release);
            self.closes.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn mic_spec_new_and_with_device() {
        let default_spec = MicSpec::new(1, 48_000);
        assert_eq!(default_spec.device, None);
        assert_eq!(default_spec.channels, 1);
        assert_eq!(default_spec.rate, 48_000);

        let named_spec = default_spec.with_device("USB Microphone");
        assert_eq!(named_spec.device.as_deref(), Some("USB Microphone"));
        assert_eq!(named_spec.channels, 1);
        assert_eq!(named_spec.rate, 48_000);
    }

    #[test]
    fn mic_refusal_access_and_display() {
        assert_eq!(MicRefusal::unavailable(), MicRefusal::Unavailable);
        assert_eq!(MicSpecFault::Channels.to_string(), "channels is not 1 or 2");
        assert_eq!(
            MicSpecFault::Rate.to_string(),
            "rate is outside 8000 to 48000 Hz"
        );
        assert_eq!(
            MicRefusal::OnServer.to_string(),
            "a server has no microphone"
        );
        assert_eq!(
            MicRefusal::Unavailable.to_string(),
            "microphone capture is unavailable in this phase"
        );
        assert_eq!(
            MicRefusal::BadSpec(MicSpecFault::Channels).to_string(),
            "the microphone spec is refused: channels is not 1 or 2"
        );
        assert_eq!(
            MicRefusal::BadSpec(MicSpecFault::Rate).to_string(),
            "the microphone spec is refused: rate is outside 8000 to 48000 Hz"
        );
        assert_eq!(
            MicRefusal::NoDevice.to_string(),
            "no matching microphone device is available"
        );
        assert_eq!(
            MicRefusal::OsDenied.to_string(),
            "the operating system denied microphone permission"
        );
        assert_eq!(
            MicRefusal::Busy.to_string(),
            "the microphone device is busy"
        );
        assert_eq!(
            MicRefusal::DeviceFailed.to_string(),
            "the microphone device failed to open or start"
        );
    }

    #[test]
    fn mic_ref_forwards_and_closes_on_drop() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<MicRef>();

        let source = Arc::new(StubMicSource::new(2, 16_000, 12, vec![10, -10, 20, -20]));
        let handle: Arc<dyn MicSource> = Arc::clone(&source);
        let mic = MicRef::new(handle);

        assert_eq!(mic.channels(), 2);
        assert_eq!(mic.rate(), 16_000);
        assert_eq!(mic.dropped_frames(), 12);
        assert!(mic.is_open());

        let mut pcm = [99_i16; 5];
        assert_eq!(mic.read(&mut pcm), 2);
        assert_eq!(pcm, [10, -10, 20, -20, 99]);
        assert_eq!(mic.read(&mut pcm), 0);

        mic.close();
        assert!(!mic.is_open());
        assert_eq!(source.closes.load(Ordering::Relaxed), 1);

        drop(mic);
        assert_eq!(source.closes.load(Ordering::Relaxed), 2);

        let drop_source = Arc::new(StubMicSource::new(1, 48_000, 0, vec![5]));
        let drop_handle: Arc<dyn MicSource> = Arc::clone(&drop_source);
        let drop_mic = MicRef::new(drop_handle);
        assert!(drop_mic.is_open());
        drop(drop_mic);
        assert!(!drop_source.is_open());
        assert_eq!(drop_source.closes.load(Ordering::Relaxed), 1);
    }
}
