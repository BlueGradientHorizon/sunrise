#[allow(unused_imports)]
use crate::game::native_entity_refs::tests::with_game;
#[allow(unused_imports)]
use mod_api::*;
#[allow(unused_imports)]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
#[allow(unused_imports)]
use sunrise_client::chunk::Chunk;
#[allow(unused_imports)]
use sunrise_core::position::BlockPos;

pub(in crate::game) type Notices = Arc<Mutex<Vec<(BlockPos, BlockPos)>>>;
pub(in crate::game) struct Counter(pub(in crate::game) Notices);
#[mod_api::sunrise_part]
impl BlockPart for Counter {
    fn on_neighbor_change(&self, _: &dyn LevelHost, event: &mut OnNeighborChangeEvent) {
        self.0.lock().unwrap().push((event.pos, event.neighbor));
    }
}
pub(in crate::game) struct Weak(
    pub(in crate::game) Arc<AtomicBool>,
    pub(in crate::game) Arc<AtomicUsize>,
);
#[mod_api::sunrise_part]
impl BlockPart for Weak {
    fn weak_changes(&self, _: &dyn LevelHost, event: &mut WeakChangesEvent) {
        self.1.fetch_add(1, Ordering::SeqCst);
        event.receive = self.0.load(Ordering::SeqCst);
    }
}
pub(in crate::game) struct Listener(
    pub(in crate::game) Notices,
    pub(in crate::game) Arc<AtomicBool>,
    pub(in crate::game) Arc<AtomicUsize>,
);
#[mod_api::__subscriptions]
impl Hooks for Listener {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let glass = r.block_key("minecraft:glass");
        r.block_part(glass, Counter(self.0.clone()));
        r.block_part(glass, Weak(self.1.clone(), self.2.clone()));
    }
}

pub(in crate::game) struct EdgeListener(
    pub(in crate::game) Notices,
    pub(in crate::game) Arc<AtomicBool>,
    pub(in crate::game) Arc<AtomicUsize>,
);
#[mod_api::__subscriptions]
impl Hooks for EdgeListener {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let air = r.block_key("minecraft:air");
        r.block_part(air, Counter(self.0.clone()));
        r.block_part(air, Weak(self.1.clone(), self.2.clone()));
    }
}

#[test]
#[ignore = "daily"]
fn block_entity_neighbor_traversal_keeps_far_chunk_unloaded() {
    let notices = Arc::new(Mutex::new(Vec::new()));
    let queries = Arc::new(AtomicUsize::new(0));
    let mods: Mods = vec![ModEntry::new(
        "edge_neighbor",
        Box::new(EdgeListener(
            notices.clone(),
            Arc::new(AtomicBool::new(true)),
            queries.clone(),
        )),
    )]
    .into();
    mod_platform::testing::with_mods(mods.clone(), || {
        mod_platform::__private::part_tests::install_parts_fixture(&mods).unwrap();
        with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world.chunks.clear();
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let registry = game.pack.assets.registry;
            let chest = registry.default_state("minecraft:chest").unwrap();
            let stone = registry.default_state("minecraft:stone").unwrap();
            // Each case has its conductor loaded and its far cell across a chunk edge.
            for (source, adjacent, far) in [
                ((14, 64, 8), (15, 64, 8), (16, 64, 8)),
                ((1, 64, 8), (0, 64, 8), (-1, 64, 8)),
                ((8, 64, 14), (8, 64, 15), (8, 64, 16)),
                ((8, 64, 1), (8, 64, 0), (8, 64, -1)),
            ] {
                game.world.set(source.0, source.1, source.2, chest).unwrap();
                game.world
                    .set(adjacent.0, adjacent.1, adjacent.2, stone)
                    .unwrap();
                let resident = game.world.chunks.len();
                notices.lock().unwrap().clear();
                queries.store(0, Ordering::SeqCst);
                game.container_changed(source);
                assert_eq!(queries.load(Ordering::SeqCst), 1);
                assert!(notices.lock().unwrap().contains(&(
                    BlockPos::new(far.0, far.1, far.2),
                    BlockPos::new(source.0, source.1, source.2),
                )));
                assert_eq!(game.world.chunks.len(), resident);
                assert!(!game.world.loaded(far.0, far.2));
            }
        });
    });
}
