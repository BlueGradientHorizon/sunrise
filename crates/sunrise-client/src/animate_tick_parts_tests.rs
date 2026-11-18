//! `AnimateContext::block_parts`: the sampler tells the game each picked loaded position, after
//! the block's own vanilla effects, for the mod block parts' `animate_tick`.
use super::*;
use std::cell::RefCell;

#[test]
#[ignore = "daily"]
fn each_loaded_pick_is_told_to_the_block_parts_after_its_vanilla_effects() {
    let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
    let assets = crate::assets::Assets::load(&root).unwrap();
    let facts = BlockFacts::load(&root, assets.registry).unwrap();
    let collision = crate::collisions::load(&root, assets.registry).unwrap();
    let tables = AnimateTables::load(&root, assets.registry).unwrap();
    let biomes = Biomes::load(&sunrise_assets::resource_source::ResourceRoot::directory(
        root.clone(),
    ))
    .unwrap();
    let torch = assets.registry.default_state("minecraft:torch").unwrap();
    let mut world = World::new(facts.air, 0);
    world.chunks.insert(
        (0, 0),
        std::sync::Arc::new(crate::chunk::Chunk::empty(0, 0)),
    );
    world.set(4, 64, 4, torch).unwrap();
    let told = RefCell::new(Vec::new());
    let block_parts = |state: BlockStateId, pos: [i32; 3]| told.borrow_mut().push((state, pos));
    let ctx = AnimateContext {
        enchant_power: &EnchantPower::default(),
        world: &world,
        facts: &facts,
        collision: &collision,
        biomes: &biomes,
        registry: assets.registry,
        raining_at: &|_| false,
        height_no_leaves: &|_| 0,
        firefly_bush_sounds: false,
        creaking_active: false,
        sky_darken: 0,
        marker: None,
        model_offset: &|_, _, _| None,
        gateway_particles: &|_, _| 0,
        thundering: false,
        ultrawarm: false,
        block_parts: Some(&block_parts),
    };
    let mut engine = ParticleEngine::new(
        &sunrise_assets::resource_source::ResourceSource::directory(root),
        &Default::default(),
        &Default::default(),
    );
    engine.seed_randoms(5, 9, 0);
    engine.created_requests = Some(Vec::new());
    engine.camera = [4.5, 64.5, 4.5];
    // A radius of 1 picks the centre itself: the torch's own smoke and flame, then its parts.
    do_animate_tick(&tables, &mut engine, &ctx, [4, 64, 4], 1, true);
    assert_eq!(*told.borrow(), [(torch, [4, 64, 4])]);
    let made = engine.created_requests.take().unwrap();
    assert!(made.iter().any(|(options, ..)| options.ty == T::Flame));
    // A pick in a chunk that isn't loaded draws nothing and tells nobody.
    told.borrow_mut().clear();
    do_animate_tick(&tables, &mut engine, &ctx, [40, 64, 40], 1, true);
    assert!(told.borrow().is_empty());
    // A whole tick: every loaded pick, each within the sampled cube around the player.
    animate_tick(&tables, &mut engine, &ctx, [8, 64, 8]);
    let told = told.take();
    assert!(!told.is_empty() && told.len() <= 2 * 667);
    for (state, pos) in told {
        assert!(world.loaded(pos[0], pos[2]));
        assert!(pos.iter().zip([8, 64, 8]).all(|(p, c)| (p - c).abs() < 32));
        assert_eq!(state, world.block(pos[0], pos[1], pos[2]));
    }
    // Without a recorder the sampler asks nothing: the vanilla effects alone.
    let quiet = AnimateContext {
        block_parts: None,
        ..ctx
    };
    engine.created_requests = Some(Vec::new());
    do_animate_tick(&tables, &mut engine, &quiet, [4, 64, 4], 1, true);
    assert!(!engine.created_requests.take().unwrap().is_empty());
}
