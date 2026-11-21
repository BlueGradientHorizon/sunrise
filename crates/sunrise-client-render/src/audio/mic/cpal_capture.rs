//! CPAL microphone capture.

use super::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub(super) struct CpalBackend;

#[derive(Clone)]
pub(super) struct CpalDevice {
    device: cpal::Device,
    label: Option<String>,
    selector: Option<String>,
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

fn map_devices_error(err: cpal::DevicesError) -> mod_api::MicRefusal {
    match err {
        cpal::DevicesError::BackendSpecific { err } => classify_error_message(&err.description),
    }
}

fn map_default_config_error(err: cpal::DefaultStreamConfigError) -> mod_api::MicRefusal {
    match err {
        cpal::DefaultStreamConfigError::DeviceNotAvailable => mod_api::MicRefusal::NoDevice,
        cpal::DefaultStreamConfigError::BackendSpecific { err } => {
            classify_error_message(&err.description)
        }
        cpal::DefaultStreamConfigError::StreamTypeNotSupported => mod_api::MicRefusal::DeviceFailed,
    }
}

fn map_build_stream_error(err: cpal::BuildStreamError) -> mod_api::MicRefusal {
    match err {
        cpal::BuildStreamError::DeviceNotAvailable => mod_api::MicRefusal::NoDevice,
        cpal::BuildStreamError::BackendSpecific { err } => classify_error_message(&err.description),
        cpal::BuildStreamError::StreamConfigNotSupported
        | cpal::BuildStreamError::InvalidArgument
        | cpal::BuildStreamError::StreamIdOverflow => mod_api::MicRefusal::DeviceFailed,
    }
}

fn map_play_stream_error(err: cpal::PlayStreamError) -> mod_api::MicRefusal {
    match err {
        cpal::PlayStreamError::DeviceNotAvailable => mod_api::MicRefusal::NoDevice,
        cpal::PlayStreamError::BackendSpecific { err } => classify_error_message(&err.description),
    }
}

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
