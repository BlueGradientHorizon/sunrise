//! A compiled wasm mod plays audio the way a voice mod would: on each client tick it pushes 50 ms
//! into streams of its own, through the real API and the game's own mixer, while the ticks arrive
//! up to 50 ms late. The figures in the book's audio streams page come from this test.
use mod_api::prelude::*;
use mod_api::{AudioRefusal, AudioStreamRef, StreamSpec};
use sunrise_client::audio::Audio;

/// The prebuffers the fixture's three streams open with (`audio_stream.rs`), in order.
const PREBUFFERS_MS: [u32; 3] = [50, 100, 150];
/// The `tick_target_millis` value that asks the fixture guest for one tick of audio.
const TICK: f32 = -3.0;

/// A client with a sound output of its own: no device, a mixer the test runs by hand.
struct Client {
    audio: Audio,
}
impl mod_api::Host for Client {}
mod_api::impl_host_areas!(Client);
impl mod_api::ClientHost for Client {}
mod_api::impl_client_host_areas!(Client, except(audio));
impl AudioClientHost for Client {
    fn open_audio_stream(&self, spec: &StreamSpec) -> Result<AudioStreamRef, AudioRefusal> {
        self.audio
            .open_stream("wasm_fixture", spec)
            .map(AudioStreamRef::new)
    }
}

#[test]
#[ignore = "daily"]
fn a_compiled_wasm_mod_pushing_at_tick_pace_does_not_underrun_at_the_prebuffer_the_book_recommends()
{
    sunrise_guest_stack::run_on_guest_stack(|| {
        use sunrise_mod_package::manifest::Package;
        use sunrise_mod_wasm::{WasmPackage, load};
        let path = std::path::PathBuf::from(std::env::var_os("SUNRISE_WASM_FIXTURE").unwrap());
        let manifest = Package::read(&path).unwrap().manifest;
        let info = mod_api::PackageInfo::new(
            manifest.id.clone(),
            manifest.version.to_string(),
            mod_api::PackageKind::Code,
            mod_api::Side::Both,
            vec![manifest.id.clone()],
            vec![],
        );
        let loaded = load(
            &std::fs::read(std::env::var_os("SUNRISE_WASM_STD").unwrap()).unwrap(),
            vec![WasmPackage {
                manifest,
                module: Package::read_wasm(&path).unwrap(),
            }],
        );
        assert!(loaded.refused.is_empty(), "{:?}", loaded.refused);
        loaded.initialize(&[info]).unwrap();
        mod_platform::testing::with_mods(loaded.mods.into(), || {
            let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
            let mut audio =
                Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                    std::path::PathBuf::from("/nonexistent/sunrise-test-audio"),
                ));
            audio.backend_live_for_bench();
            let mut client = Client { audio };
            // One 10 ms callback of stereo output at 48 kHz.
            let mut output = [0.0f32; 960];
            // Tick k is due at k * 50 ms, and arrives up to 50 ms late (a fixed series, so the
            // figures repeat). The output callback runs every 10 ms for 30 s.
            let mut seed = 0x9e37_79b9u32;
            let mut late = || {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 16) % 51
            };
            let (mut ticks, mut next_tick) = (0u32, late());
            for ms in (0..30_000u32).step_by(10) {
                while next_tick <= ms {
                    let mut opened = TICK;
                    mod_api::dispatch_tick_target_millis(&client, &mut opened);
                    assert_eq!(opened, 3.0, "the guest opened its three streams");
                    ticks += 1;
                    next_tick = ticks * 50 + late();
                }
                output.fill(0.0);
                client.audio.mix_buffer(&mut output, 2, 48_000);
            }
            let figures = client.audio.stream_underruns_for_bench();
            eprintln!("wasm-paced underruns in 30 s, by prebuffer_ms: {figures:?}");
            assert_eq!(
                figures.iter().map(|f| f.0).collect::<Vec<_>>(),
                PREBUFFERS_MS
            );
            // The book's table: the same counts as the mixer's own simulation.
            assert_eq!(
                figures.iter().map(|f| f.1).collect::<Vec<_>>(),
                [2, 0, 0],
                "{figures:?}"
            );
        });
    });
}
