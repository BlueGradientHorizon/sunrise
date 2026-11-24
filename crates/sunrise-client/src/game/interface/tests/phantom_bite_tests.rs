//! `LevelEventHandler` 1039 (`LevelEvent.SOUND_PHANTOM_BITE`): the sound a
//! phantom's bite posts.
#[allow(unused_imports)]
use crate::game::*;
#[allow(unused_imports)]
use sunrise_core::random::{LegacyRandom, RandomSource};

#[allow(unused_imports)]
pub(in crate::game) use crate::game::client_side::level_particles_client::phantom_bite_tests::with_game;

/// Level event 1039 is `playLocalSound(pos, SoundEvents.PHANTOM_BITE,
/// SoundSource.HOSTILE, 0.3F, random.nextFloat() * 0.1F + 0.9F, false)` with
/// `random` the client level's, at the block's centre.
#[test]
#[ignore = "daily"]
fn a_phantom_bite_level_event_plays_the_bite_at_0_3_volume() {
    with_game(|g| {
        // The client level's random, seeded so the pitch has an expected value.
        *g.particles.0.level_random() = LegacyRandom::new(7);
        let pitch = LegacyRandom::new(7).next_float() * 0.1 + 0.9;
        g.audio.0.engine.loaded = true;
        g.level_event_at(1039, [0, 64, 0], 0);
        let bite = (0..=8u64)
            .filter_map(|id| g.audio.0.engine.instance(id))
            .find(|i| i.identifier == "minecraft:entity.phantom.bite")
            .expect("level event 1039 plays the phantom's bite");
        assert_eq!(bite.volume, 0.3);
        assert_eq!(bite.pitch, pitch);
        assert_eq!(bite.position, [0.5, 64.5, 0.5]);
        assert_eq!(
            sunrise_client::settings::SOUND_SOURCES[bite.source],
            "hostile"
        );
        // It goes to the guests as the level event itself, which they play.
        assert_eq!(
            g.server.lan_events,
            vec![QueuedLevelEvent {
                kind: 1039,
                pos: [0, 64, 0],
                data: 0,
                source: None,
            }],
            "the bite reaches the guests as level event 1039"
        );
    });
}
