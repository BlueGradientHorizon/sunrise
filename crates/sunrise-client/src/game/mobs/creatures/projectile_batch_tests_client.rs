#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::BoundProjectiles;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::CancelImpacts;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::Observed;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::VALUE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::WindContinuationObserved;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::WindContinuationObserver;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::assert_boat_matches_witness;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::assert_vec3_bits;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::load_wind_continuation_witness_26_3;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::shoot_arrow_at_wall;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::terminal_egg;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::wind_continuation_boat_witness_26_3;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::wind_continuation_witness_26_3;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::wind_continuation_witness_row_26_3;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::with_batch;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::creatures::projectile_batch_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{
    native_entity_refs::tests::run_fresh_cases,
    native_projectiles::impact_probe::{Action, with_probe},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    Flow, HearsProjectileImpactsEvent, Hooks, ProjectileImpactEvent, ServerHost,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::{Arc, Mutex, atomic::Ordering};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{chunk::Chunk, join_seq::JoinSeq};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_mod_runtime::attachments::{
    AttachmentType, Attachments, HasAttachmentLifetime, Holder, index::AttachmentIndex,
};

#[test]
#[ignore = "daily"]
fn an_arrow_does_not_stick_when_the_impact_listener_clears_its_block() {
    const CASE: &str = "SUNRISE_PROJECTILE_BATCH_CASE";
    if run_fresh_cases(
        "game::client_side::creatures_client::projectile_batch_tests::an_arrow_does_not_stick_when_the_impact_listener_clears_its_block",
        CASE,
        &["clear", "allow"],
    ) {
        return;
    }
    let clear = std::env::var(CASE).as_deref() == Ok("clear");
    let action = if clear {
        Action::ClearBlockOnImpact
    } else {
        Action::Allow
    };
    with_probe(action, true, |game, observed| {
        game.start_audio_capture();
        shoot_arrow_at_wall(game, 2, 0.5);
        let hits = observed.hits.lock().unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].at, [2.0, 64.5, 0.5]);
        assert_eq!(hits[0].projectile_pos, hits[0].at);
        let p = &game.creatures.projectiles[0];
        let arrow = p.arrow.as_ref().unwrap();
        if clear {
            assert_eq!(
                p.position.components(),
                hits[0].at,
                "refusal must not replay the flight"
            );
            assert_eq!(p.velocity, Vec3::new(3.0 * f64::from(0.99_f32), -0.05, 0.0));
            assert_eq!(arrow.shake, 0);
        } else {
            assert_eq!(p.position.x, 2.0 - f64::from(0.05_f32));
            assert_eq!(p.velocity, Vec3::ZERO);
            assert_eq!(arrow.shake, 7);
        }
        assert_eq!(arrow.in_ground, !clear);
        assert_eq!(observed.lands.load(Ordering::Relaxed), usize::from(!clear));
        assert_eq!(
            game.side
                .pending_sounds
                .iter()
                .any(|sound| { sound.event == "minecraft:entity.arrow.hit" }),
            !clear
        );
    });
}
