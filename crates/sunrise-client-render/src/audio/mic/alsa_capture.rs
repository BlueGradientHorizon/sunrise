//! ALSA input via capture-only probes. CPAL 0.16 opens both directions while enumerating,
//! discards endpoints whose opens both fail, and does not expose their PCM addresses.

use super::{BackendDevice, CaptureBackend, CaptureRing, InputConverter, StreamEvents, stop};
use alsa::{
    Direction::Capture,
    ValueOr::Nearest,
    pcm,
    poll::{Descriptors, Flags},
};
use mod_api::{MicRefusal, MicSpec};
use std::sync::Arc;

const DEFAULT_PCM: &str = "default";

pub(super) struct AlsaBackend;

#[derive(Clone)]
pub(super) struct AlsaDevice {
    pcm: String,
    label: String,
    // This enumeration's capture result, not a cache. The endpoint survives a failed probe.
    capture: Result<(), MicRefusal>,
}

impl AlsaDevice {
    fn open<T>(&self, open: impl FnOnce(&str) -> alsa::Result<T>) -> Result<T, MicRefusal> {
        self.capture?;
        open(&self.pcm).map_err(map_error)
    }
}

impl BackendDevice for AlsaDevice {
    type InputConfigs = ();

    fn label(&self) -> Option<String> {
        Some(self.label.clone())
    }

    fn stable_selector(&self) -> Option<String> {
        Some(format!("alsa:{}", self.pcm))
    }

    fn same_endpoint(&self, other: &Self) -> bool {
        self.pcm == other.pcm
    }

    fn input_configs(&self) -> Result<Option<()>, MicRefusal> {
        self.capture.map(Some)
    }
}

fn open_pcm(name: &str) -> alsa::Result<pcm::PCM> {
    // Hardware addresses already include plug; apply the same PCM conversion to the
    // named routes, whose configured devices need not natively support signed 16-bit PCM.
    if name.starts_with("plughw:") {
        pcm::PCM::new(name, Capture, true)
    } else {
        pcm::PCM::new(&format!("plug:{name}"), Capture, true)
    }
}

fn map_error(err: alsa::Error) -> MicRefusal {
    match err.errno() {
        libc::EACCES | libc::EPERM => MicRefusal::OsDenied,
        libc::EBUSY => MicRefusal::Busy,
        libc::ENOENT | libc::ENODEV | libc::ENXIO => MicRefusal::NoDevice,
        _ => MicRefusal::DeviceFailed,
    }
}

fn probe_endpoints<T>(
    endpoints: Vec<(String, String)>,
    mut open: impl FnMut(&str) -> alsa::Result<T>,
) -> Vec<AlsaDevice> {
    endpoints
        .into_iter()
        .map(|(pcm, label)| {
            let capture = open(&pcm).map(drop).map_err(map_error);
            AlsaDevice {
                pcm,
                label,
                capture,
            }
        })
        .collect()
}

pub(super) struct AlsaStream {
    pcm: pcm::PCM,
    fds: Vec<alsa::poll::pollfd>,
    samples: Vec<i16>,
    channels: usize,
    converter: InputConverter,
    ring: Arc<CaptureRing>,
    events: Arc<StreamEvents>,
}

impl CaptureBackend for AlsaBackend {
    type Device = AlsaDevice;
    type Stream = AlsaStream;

    fn devices(&self) -> Result<Vec<Self::Device>, MicRefusal> {
        // The named ALSA routes can exist independently of any hardware card (e.g. PipeWire).
        let mut endpoints: Vec<_> = [DEFAULT_PCM, "pipewire", "pulse", "jack", "oss"]
            .into_iter()
            .map(|name| (name.to_owned(), name.to_owned()))
            .collect();
        for card in alsa::card::Iter::new() {
            let card = card.map_err(map_error)?;
            let ctl = match alsa::Ctl::from_card(&card, true) {
                Ok(ctl) => ctl,
                Err(err) if map_error(err) == MicRefusal::NoDevice => continue,
                Err(err) => return Err(map_error(err)),
            };
            let info = ctl.card_info().map_err(map_error)?;
            let card_id = info.get_id().map_err(map_error)?;
            let label = info.get_name().map_err(map_error)?;
            for device in alsa::ctl::DeviceIter::new(&ctl) {
                let pcm = format!("plughw:CARD={card_id},DEV={device}");
                // Query capture capability via the control device, before opening the PCM.
                // Output-only devices have no capture PCM; a denied capture PCM still has an ID.
                match ctl.pcm_info(device as u32, 0, Capture) {
                    Ok(_) => endpoints.push((pcm, label.to_owned())),
                    Err(err) if map_error(err) == MicRefusal::NoDevice => {}
                    Err(err) => return Err(map_error(err)),
                }
            }
        }
        Ok(probe_endpoints(endpoints, open_pcm))
    }

    fn default_input_device(&self) -> Result<Option<Self::Device>, MicRefusal> {
        Ok(Some(AlsaDevice {
            pcm: DEFAULT_PCM.to_owned(),
            label: DEFAULT_PCM.to_owned(),
            capture: Ok(()),
        }))
    }

    fn open_stream(
        &self,
        device: &Self::Device,
        _probed: Option<()>,
        spec: &MicSpec,
        ring: Arc<CaptureRing>,
        events: Arc<StreamEvents>,
    ) -> Result<Self::Stream, MicRefusal> {
        let pcm = device.open(open_pcm)?;
        let (channels, rate, period) = (|| {
            let params = pcm::HwParams::any(&pcm)?;
            params.set_access(pcm::Access::RWInterleaved)?;
            params.set_format(pcm::Format::s16())?;
            let channels = params.set_channels_near(u32::from(spec.channels))?;
            let rate = params.set_rate_near(spec.rate, Nearest)?;
            // Match the default period and buffer times of the CPAL ALSA path.
            params.set_period_time_near(25_000, Nearest)?;
            params.set_buffer_time_near(100_000, Nearest)?;
            pcm.hw_params(&params)?;
            Ok((channels, rate, params.get_period_size()?))
        })()
        .map_err(map_error)?;
        let channels = u16::try_from(channels).map_err(|_| MicRefusal::DeviceFailed)?;
        let sample_count = usize::try_from(period)
            .ok()
            .and_then(|period| period.checked_mul(usize::from(channels)))
            .filter(|&n| n > 0 && rate > 0)
            .ok_or(MicRefusal::DeviceFailed)?;
        let mut samples = Vec::new();
        samples
            .try_reserve_exact(sample_count)
            .map_err(|_| MicRefusal::DeviceFailed)?;
        samples.resize(sample_count, 0);
        let mut fds = pcm.get().map_err(map_error)?;
        if fds.is_empty() {
            return Err(MicRefusal::DeviceFailed);
        }
        fds.try_reserve(1).map_err(|_| MicRefusal::DeviceFailed)?;
        pcm.prepare().map_err(map_error)?;
        pcm.start().map_err(map_error)?;
        Ok(AlsaStream {
            pcm,
            fds,
            samples,
            channels: usize::from(channels),
            converter: InputConverter::new(channels, rate, spec.channels, spec.rate),
            ring,
            events,
        })
    }

    fn run_stream(&self, mut stream: Self::Stream, stop: stop::Receiver) {
        let stop_index = stream.fds.len();
        stream.fds.push(stop.pollfd());
        let result = (|| {
            let io = stream.pcm.io_i16()?;
            loop {
                match alsa::poll::poll(&mut stream.fds, -1) {
                    Ok(_) => {}
                    Err(err) if err.errno() == libc::EINTR => continue,
                    Err(err) => return Err(err),
                }
                // The socket is closed for writing on stop. Prioritize it over ready audio.
                if stream.fds[stop_index].revents != 0 {
                    return Ok(());
                }
                if stream.fds[..stop_index]
                    .iter()
                    .any(|fd| fd.revents & (libc::POLLHUP | libc::POLLNVAL) != 0)
                {
                    return Err(alsa::Error::new("capture poll", libc::ENODEV));
                }
                let readiness = stream.pcm.revents(&stream.fds[..stop_index])?;
                if readiness.intersects(Flags::HUP | Flags::NVAL) {
                    return Err(alsa::Error::new("capture poll", libc::ENODEV));
                }
                if !readiness.intersects(Flags::IN | Flags::ERR) {
                    continue;
                }
                match io.readi(&mut stream.samples) {
                    Ok(frames) => stream.converter.push_i16_samples(
                        &stream.samples[..frames * stream.channels],
                        &stream.ring,
                    ),
                    Err(err) if err.errno() == libc::EAGAIN => {}
                    // An ALSA capture overrun requires preparing and restarting the same PCM.
                    // Like CPAL, keep the stream open across an xrun, not a real device error.
                    Err(err) if err.errno() == libc::EPIPE => {
                        stream.pcm.prepare()?;
                        stream.pcm.start()?;
                    }
                    Err(err) => return Err(err),
                }
            }
        })();
        if let Err(err) = result {
            stream.events.report_error(map_error(err));
            log::warn!("Sunrise: ALSA microphone stream error: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::MicPoolInner;
    use super::*;
    use std::sync::Mutex;

    struct Discovered {
        devices: Mutex<Vec<AlsaDevice>>,
        opened: Mutex<Vec<String>>,
    }

    impl CaptureBackend for Discovered {
        type Device = AlsaDevice;
        type Stream = ();

        fn devices(&self) -> Result<Vec<Self::Device>, MicRefusal> {
            Ok(self.devices.lock().unwrap().clone())
        }

        fn default_input_device(&self) -> Result<Option<Self::Device>, MicRefusal> {
            Ok(None)
        }

        fn open_stream(
            &self,
            device: &Self::Device,
            _probed: Option<()>,
            _spec: &MicSpec,
            _ring: Arc<CaptureRing>,
            _events: Arc<StreamEvents>,
        ) -> Result<(), MicRefusal> {
            device.open(|pcm| {
                self.opened.lock().unwrap().push(pcm.to_owned());
                Ok(())
            })
        }
    }

    #[test]
    fn enumeration_retains_a_capture_open_denial_for_listing_and_selection() {
        let denied = "plughw:CARD=Denied,DEV=0";
        let available = "plughw:CARD=Available,DEV=0";
        let devices = probe_endpoints(
            vec![
                (denied.into(), "Denied Mic".into()),
                (available.into(), "Desk Mic".into()),
            ],
            |name| {
                if name == denied {
                    Err(alsa::Error::new("snd_pcm_open", libc::EACCES))
                } else {
                    Ok(())
                }
            },
        );
        let backend = Arc::new(Discovered {
            devices: Mutex::new(devices),
            opened: Mutex::new(Vec::new()),
        });
        let pool = MicPoolInner::new(true, Arc::clone(&backend));
        assert_eq!(pool.devices(), Err(MicRefusal::OsDenied));
        for device in ["Denied Mic", "alsa:plughw:CARD=Denied,DEV=0"] {
            assert_eq!(
                pool.open(&MicSpec::new(1, 48_000).with_device(device))
                    .err(),
                Some(MicRefusal::OsDenied)
            );
        }
        let mic = pool
            .open(&MicSpec::new(1, 48_000).with_device("Desk Mic"))
            .unwrap();
        assert_eq!(*backend.opened.lock().unwrap(), [available]);
        mic.close();
    }

    #[test]
    fn a_busy_alsa_card_keeps_its_selector_instead_of_retargeting_a_same_label_input() {
        let first = "plughw:CARD=Headset,DEV=0";
        let second = "plughw:CARD=Headset_1,DEV=0";
        let endpoints = vec![
            (first.into(), "Headset".into()),
            (second.into(), "Headset".into()),
        ];
        let backend = Arc::new(Discovered {
            devices: Mutex::new(probe_endpoints(endpoints.clone(), |_| Ok(()))),
            opened: Mutex::new(Vec::new()),
        });
        let pool = MicPoolInner::new(true, Arc::clone(&backend));
        let initial = pool.devices().unwrap();
        assert_eq!(initial[0].id, "alsa:plughw:CARD=Headset,DEV=0");
        assert_eq!(initial[1].id, "alsa:plughw:CARD=Headset_1,DEV=0");

        // Same control inventory, but the first capture probe now fails. Only the second can be
        // listed; it must not inherit the first endpoint's old selector.
        *backend.devices.lock().unwrap() = probe_endpoints(endpoints, |name| {
            if name == first {
                Err(alsa::Error::new("snd_pcm_open", libc::EBUSY))
            } else {
                Ok(())
            }
        });
        assert_eq!(pool.devices().unwrap().as_slice(), &initial[1..]);
        let spec = MicSpec::new(1, 48_000).with_device(&initial[0].id);
        assert_eq!(pool.open(&spec).err(), Some(MicRefusal::Busy));
        assert!(backend.opened.lock().unwrap().is_empty());

        // Removal must not reinterpret that same selector as the other device either.
        backend
            .devices
            .lock()
            .unwrap()
            .retain(|dev| dev.pcm != first);
        assert_eq!(pool.open(&spec).err(), Some(MicRefusal::NoDevice));
        let mic = pool
            .open(&MicSpec::new(1, 48_000).with_device(&initial[1].id))
            .unwrap();
        assert_eq!(*backend.opened.lock().unwrap(), [second]);
        mic.close();
    }
}
