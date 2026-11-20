//! A compiled guest's particle provider ticks a batch through the wasm boundary: the guest reads
//! the level view, and every particle's writes reach the game.
use mod_api::__private::game::RegistrationContextNew;
use mod_api::{
    LightLayer, ModParticle, ParticleFacing, ParticleLevelView, ParticleProviderRegistry,
};

/// Sky light 11 and block light 4 at every position.
struct Lit;
impl ParticleLevelView for Lit {
    fn block_state(&self, _: [i32; 3]) -> mod_api::BlockState {
        unreachable!("the fixture's provider reads no block state")
    }
    fn brightness(&self, layer: LightLayer, _: [i32; 3]) -> u8 {
        match layer {
            LightLayer::Sky => 11,
            LightLayer::Block => 4,
        }
    }
}

fn particle(age: i32, x: f64) -> ModParticle {
    ModParticle {
        position: [x, 5.0, 5.0],
        previous: [x, 5.0, 5.0],
        velocity: [0.0; 3],
        age,
        lifetime: 20,
        size: 0.5,
        rgba: [1.0; 4],
        frame: 0,
        removed: false,
        state: [7.0, 0.0, 0.0, 0.0],
        facing: ParticleFacing::Camera,
        roll: 0.0,
    }
}

#[test]
#[ignore = "daily"]
fn a_wasm_providers_batch_tick_writes_every_particle_and_reads_the_level() {
    crate::game::in_own_process(
        "game::wasm_particle_provider_tests::a_wasm_providers_batch_tick_writes_every_particle_and_reads_the_level",
        || {
            sunrise_guest_stack::run_on_guest_stack(|| {
                mod_platform::__private::initialize().unwrap();
                let mods: mod_api::Mods = super::block_entity_tick_reply_tests::wasm_mods().into();
                let registration = mod_api::__private::PhaseOwner::scope(
                    mod_api::__private::ModPhase::Registration,
                );
                mod_platform::testing::with_mods(mods, || {
                    let mut registry = ParticleProviderRegistry::default();
                    crate::game::Game::native_register_particle_providers(
                        &mod_api::RegistrationContext::new(),
                        &mut registry,
                    );
                    drop(registration);
                    let providers = registry.into_providers();
                    let spark = providers
                        .get("wasm_fixture:spark")
                        .expect("the guest registered its provider");
                    let mut batch = mod_api::__private::particle_batch_from_vec(vec![
                        particle(1, 0.0),
                        particle(3, 10.0),
                    ]);
                    spark.tick(&mut batch, &Lit);
                    let [first, second] = [0, 1].map(|i| batch.get_mut(i).unwrap().clone());
                    // Moved by its place in the batch, one state count each, in call order.
                    assert_eq!(first.position, [1.0, 5.0, 5.0]);
                    assert_eq!(second.position, [12.0, 5.0, 5.0]);
                    assert_eq!((first.state[0], second.state[0]), (8.0, 8.0));
                    // The guest's facing and roll cross back.
                    let flat = ParticleFacing::Fixed([-0.5, -0.5, -0.5, 0.5]);
                    assert_eq!((first.facing, first.roll), (flat, 0.25));
                    assert_eq!((second.facing, second.roll), (flat, 0.5));
                    // The guest read the host's level view: sky 11 and block 4 as fractions of 15.
                    assert_eq!(first.rgba, [11.0 / 15.0, 4.0 / 15.0, 1.0, 1.0]);
                    // Only the particle past age 2 is removed.
                    assert!(!first.removed && second.removed);
                    assert_eq!(batch.len(), 2);
                });
            });
        },
    );
}
