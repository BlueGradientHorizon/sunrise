//! An injection point's table fired outside the world (`Table::fire_api`), with the Game as its host.
//!
//! A plain build compiles no point sites (`mod_points::STAGED`), so the game's own points refuse every
//! listener; this fires a table of its own, sited, with the call a generated dispatcher makes for a native listener.
use mod_api::__private::points::{Place, SlotListener, Table, register, seal};
use mod_api::{Hooks, ModEntry};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

static ACTIVE: [AtomicBool; 1] = [AtomicBool::new(false)];
static TABLE: Table = Table::new(&ACTIVE);
static CALLS: AtomicU32 = AtomicU32::new(0);
static READS_DONE: AtomicU32 = AtomicU32::new(0);

/// A listener that reads the world through the Host, as a mod's listener may.
fn reads(host: &mut dyn mod_api::ServerHost) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    let _ = host.level("minecraft:overworld");
    READS_DONE.fetch_add(1, Ordering::Relaxed);
}

/// What a generated dispatcher does at the point's site.
#[allow(unsafe_code)]
fn fire(game: &mut crate::game::Game<'_>) {
    TABLE.fire_api(0, &mut |function| {
        // SAFETY: the site's only registered listener has this exact signature.
        let function = unsafe {
            std::mem::transmute::<*const (), fn(&mut dyn mod_api::ServerHost)>(function.as_ptr())
        };
        function(game);
    });
}

struct Probe;
#[mod_api::__subscriptions]
impl Hooks for Probe {}

/// Points fire in ticks, after the thread reached World. Fired in Setup and in Reloading anyway, a
/// listener's world read skips that listener for the call and is no panic; in World it runs through.
#[test]
#[ignore = "daily"]
#[allow(unsafe_code)]
fn a_point_outside_the_world_skips_the_listener_not_a_panic() {
    use mod_api::__private::{ModPhase, PhaseOwner};
    if super::native_entity_refs::tests::run_fresh_cases(
        "game::point_phase_refusal_tests::a_point_outside_the_world_skips_the_listener_not_a_panic",
        "SUNRISE_POINT_PHASE_CHILD",
        &["valid"],
    ) {
        return;
    }
    let mods = vec![ModEntry::new("pointprobe", Box::new(Probe))].into();
    mod_platform::testing::with_mods(mods, || {
        // SAFETY: `fire` uses the exact type of `reads`.
        let listener =
            unsafe { SlotListener::api("pointprobe", Place::new(&TABLE, 0), reads as *const ()) };
        register(&listener).unwrap();
        seal();
        super::native_entity_refs::tests::with_game(|game| {
            for reloading in [false, true] {
                let setup = PhaseOwner::scope(ModPhase::Setup);
                let _reload = reloading.then(|| setup.reloading().unwrap());
                fire(game);
            }
            assert_eq!(
                CALLS.load(Ordering::Relaxed),
                2,
                "each call ran up to its refused read"
            );
            assert_eq!(
                READS_DONE.load(Ordering::Relaxed),
                0,
                "no refused read completed"
            );
            let _world = PhaseOwner::scope(ModPhase::World);
            fire(game);
            assert_eq!(CALLS.load(Ordering::Relaxed), 3);
            assert_eq!(
                READS_DONE.load(Ordering::Relaxed),
                1,
                "the World read completed"
            );
        });
    });
}
