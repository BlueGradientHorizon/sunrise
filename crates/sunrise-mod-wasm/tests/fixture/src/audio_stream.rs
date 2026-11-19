//! A wasm mod that plays audio: on each client tick it pushes 50 ms of a tone into three streams
//! that differ only in `prebuffer_ms`, as a voice mod decoding one frame a tick would.
use mod_api::{AudioStreamRef, ClientHost, SoundSource, StreamSpec};
use std::cell::RefCell;

/// The `tick_target_millis` value that asks the guest for one tick of audio (the fixture's other
/// sentinels are -1 and -2).
pub const TICK: f32 = -3.0;
/// The prebuffers the three streams open with, in milliseconds.
pub const PREBUFFERS_MS: [u32; 3] = [50, 100, 150];
/// Frames in one tick's 50 ms at 48 kHz.
const FRAMES: usize = 2_400;

thread_local! {
    static STREAMS: RefCell<Vec<AudioStreamRef>> = const { RefCell::new(Vec::new()) };
}

/// Opens the streams on the first call, then pushes one tick of audio into each. Answers how many
/// streams are open.
pub fn tick(host: &dyn ClientHost) -> f32 {
    STREAMS.with(|streams| {
        let mut streams = streams.borrow_mut();
        if streams.is_empty() {
            for prebuffer_ms in PREBUFFERS_MS {
                let spec =
                    StreamSpec::new(SoundSource::Voice, 1, 48_000).with_prebuffer_ms(prebuffer_ms);
                if let Ok(stream) = host.open_audio_stream(&spec) {
                    streams.push(stream);
                }
            }
        }
        let tone = [8_192i16; FRAMES];
        for stream in streams.iter() {
            stream.push(&tone);
        }
        streams.len() as f32
    })
}
