//! Detached off-world chunk generation (`DetachedChunkGenerator`, #5236).
//!
//! Hands mods a `Send + Sync + 'static` handle backed by the level's `VanillaTerrain` generator
//! and a bounded, idle-expiring `GeneratorWorker` pool (`Mutex<Vec<(Instant, GeneratorWorker)>>`).
//! Executes purely in memory on the caller's thread without mutating `World`, `Streaming`, tickets,
//! or `SaveDir`. On level close, the open flag flips to `false` and every pooled worker is
//! immediately dropped.
use super::native_chunk_snapshot::EngineChunkSnapshot;
use super::{Dim, Game};
use mod_api::__private::DetachedGeneratorBackend;
use mod_api::{
    ChunkSnapshot, DetachedChunkGenerator, DimensionKey, GenerationError, GenerationStage,
    LevelRef, TerrainColumn,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};
use sunrise_client::assets::Properties;
use sunrise_client::biome::Biomes;
use sunrise_client::light::LightTables;
use sunrise_client::streaming::chunk_pos_is_valid;
use sunrise_client::vanillagen::VanillaTerrain;
use sunrise_world::worldgen::generator::GeneratorWorker;

/// Pooled `GeneratorWorker`s idle longer than this are dropped on the next checkout/return so
/// quiet threads never pin multi-MB generator scratch memory.
const WORKER_IDLE_TTL: Duration = Duration::from_secs(5);
/// Maximum idle workers retained per dimension pool; bounded by host CPU parallelism.
fn max_pooled_workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

fn dim_of(key: DimensionKey) -> Dim {
    match key {
        DimensionKey::OVERWORLD => Dim::Overworld,
        DimensionKey::NETHER => Dim::Nether,
        DimensionKey::END => Dim::End,
        other => Dim::Extra(other),
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "generator panicked".to_owned()
    }
}

/// Open detached chunk generators handed out by `LevelRef::detached_generator`, keyed by level.
pub(super) type DetachedGenerators =
    std::cell::RefCell<std::collections::HashMap<Dim, Arc<DetachedGeneratorState>>>;

/// Shared lifecycle + bounded idle-worker pool for one dimension's detached generator handles.
pub(super) struct DetachedGeneratorState {
    open: AtomicBool,
    workers: Mutex<Vec<(Instant, GeneratorWorker)>>,
}

impl DetachedGeneratorState {
    fn new() -> Self {
        Self {
            open: AtomicBool::new(true),
            workers: Mutex::new(Vec::new()),
        }
    }

    fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }

    /// Flip the open flag and immediately drain all idle workers so closed levels keep zero state.
    pub(super) fn close(&self) {
        self.open.store(false, Ordering::Release);
        let drained = {
            let mut guard = self.workers.lock().unwrap_or_else(|e| e.into_inner());
            std::mem::take(&mut *guard)
        };
        drop(drained);
    }

    fn checkout(&self, terrain: &VanillaTerrain) -> GeneratorWorker {
        let now = Instant::now();
        let mut stale = Vec::new();
        let reused = {
            let mut guard = self.workers.lock().unwrap_or_else(|e| e.into_inner());
            let mut i = 0;
            while i < guard.len() {
                if now.saturating_duration_since(guard[i].0) > WORKER_IDLE_TTL {
                    stale.push(guard.remove(i));
                } else {
                    i += 1;
                }
            }
            guard.pop().map(|(_, worker)| worker)
        };
        drop(stale);
        reused.unwrap_or_else(|| terrain.new_worker())
    }

    fn checkin(&self, mut worker: GeneratorWorker) {
        if !self.is_open() {
            return;
        }
        worker.release();
        let now = Instant::now();
        let cap = max_pooled_workers();
        let mut stale = Vec::new();
        {
            let mut guard = self.workers.lock().unwrap_or_else(|e| e.into_inner());
            if !self.is_open() {
                return;
            }
            let mut i = 0;
            while i < guard.len() {
                if now.saturating_duration_since(guard[i].0) > WORKER_IDLE_TTL {
                    stale.push(guard.remove(i));
                } else {
                    i += 1;
                }
            }
            if guard.len() < cap {
                guard.push((now, worker));
            }
        }
        drop(stale);
    }
}

struct EngineDetachedGenerator {
    terrain: Weak<VanillaTerrain>,
    state: Arc<DetachedGeneratorState>,
    states: Arc<super::api_values::block_state::Index>,
    biomes: Arc<Biomes>,
    properties: Properties,
    light_tables: Arc<LightTables>,
    sky_light: bool,
}

impl EngineDetachedGenerator {
    fn active_terrain(&self) -> Result<Arc<VanillaTerrain>, GenerationError> {
        if !self.state.is_open() {
            return Err(GenerationError::Unavailable);
        }
        self.terrain.upgrade().ok_or(GenerationError::Unavailable)
    }
}

impl DetachedGeneratorBackend for EngineDetachedGenerator {
    fn generate_chunk(
        &self,
        pos: [i32; 2],
        stage: GenerationStage,
    ) -> Result<ChunkSnapshot, GenerationError> {
        let terrain = self.active_terrain()?;
        if !chunk_pos_is_valid((pos[0], pos[1])) {
            return Err(GenerationError::InvalidPos);
        }
        let mut worker = self.state.checkout(&terrain);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            terrain.detached_chunk(&mut worker, pos, stage, &self.light_tables, self.sky_light)
        }));
        match outcome {
            Ok(Ok((chunk, has_light))) => {
                self.state.checkin(worker);
                if !self.state.is_open() {
                    return Err(GenerationError::Unavailable);
                }
                Ok(EngineChunkSnapshot::from_chunk(
                    &chunk,
                    self.states.clone(),
                    self.biomes.clone(),
                    self.properties.clone(),
                    has_light,
                ))
            }
            Ok(Err(message)) => {
                self.state.checkin(worker);
                Err(GenerationError::Failed(message))
            }
            Err(payload) => {
                // Do not return a worker that unwound mid-generation to the pool.
                drop(worker);
                Err(GenerationError::Failed(panic_message(payload)))
            }
        }
    }

    fn sample_surface_columns(
        &self,
        columns: &[(i32, i32)],
    ) -> Result<Vec<Option<TerrainColumn>>, GenerationError> {
        let terrain = self.active_terrain()?;
        for &(x, z) in columns {
            if !chunk_pos_is_valid((x >> 4, z >> 4)) {
                return Err(GenerationError::InvalidPos);
            }
        }
        if columns.is_empty() {
            return Ok(Vec::new());
        }
        let mut worker = self.state.checkout(&terrain);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut out = Vec::with_capacity(columns.len());
            for &(x, z) in columns {
                let sample = match terrain.detached_surface(&mut worker, x, z) {
                    None => None,
                    Some(surface) => {
                        let state = self.states.to_api(surface.state).map_err(|error| {
                            format!("detached surface block conversion: {error:?}")
                        })?;
                        Some(
                            mod_api::__private::built::TerrainColumn {
                                height: surface.height,
                                block: state.block(),
                                water: surface.water.map(|(height, _)| height),
                                biome: self.biomes.name(surface.biome).to_owned(),
                            }
                            .into(),
                        )
                    }
                };
                out.push(sample);
            }
            Ok::<_, String>(out)
        }));
        match outcome {
            Ok(Ok(result)) => {
                self.state.checkin(worker);
                if !self.state.is_open() {
                    return Err(GenerationError::Unavailable);
                }
                Ok(result)
            }
            Ok(Err(message)) => {
                self.state.checkin(worker);
                Err(GenerationError::Failed(message))
            }
            Err(payload) => {
                drop(worker);
                Err(GenerationError::Failed(panic_message(payload)))
            }
        }
    }
}

impl Game<'_> {
    /// Server-side backend for `LevelRef::detached_generator`.
    pub(super) fn level_detached_generator(
        &self,
        level: LevelRef,
    ) -> Option<DetachedChunkGenerator> {
        if !self.is_logical_server() {
            return None;
        }
        let world = self.native_level_world(level)?;
        let dim = dim_of(level.dimension());
        let terrain = if dim == self.dim {
            self.end.locator()
        } else {
            self.levels.parked_locator_of(level.dimension_id())
        }?;
        let states = super::api_values::block_state::current().ok()?;
        let biomes = world_biomes!(self).clone();
        let properties = self.pack.assets.properties.clone();
        let light_tables = self.pack.context.light.clone();
        let sky_light = world.shape.sky_light;
        let state = self
            .detached_generators
            .borrow_mut()
            .entry(dim)
            .or_insert_with(|| Arc::new(DetachedGeneratorState::new()))
            .clone();
        Some(DetachedChunkGenerator::from_backend(Arc::new(
            EngineDetachedGenerator {
                terrain: Arc::downgrade(&terrain),
                state,
                states,
                biomes,
                properties,
                light_tables,
                sky_light,
            },
        )))
    }

    /// Close all `DetachedChunkGenerator` handles for `dim` and drop their pooled workers.
    pub(super) fn close_detached_generators_for_dim(&mut self, dim: Dim) {
        if let Some(state) = self.detached_generators.borrow_mut().remove(&dim) {
            state.close();
        }
    }

    /// Close all `DetachedChunkGenerator` handles across every dimension and drop pooled workers.
    pub(super) fn close_all_detached_generators(&mut self) {
        for (_, state) in self.detached_generators.borrow_mut().drain() {
            state.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::native_entity_refs::tests::with_game;
    use mod_api::{GenerationError, GenerationStage};
    use std::sync::Arc;
    use sunrise_client::vanillagen::{TerrainResources, VanillaTerrain};
    use sunrise_core::position::ChunkPos;
    use sunrise_world::worldgen::generator::FlatSettings;

    fn make_flat_terrain(game: &crate::game::Game<'_>) -> Arc<VanillaTerrain> {
        let directory = game.options.resources.join("worldgen");
        Arc::new(
            VanillaTerrain::for_flat(
                TerrainResources {
                    data: Some(&game.pack.data_changes),
                    dir: &directory,
                    registry: game.pack.assets.registry,
                    biomes: world_biomes!(game),
                    item_data: &game.res.core.item_data,
                },
                &Default::default(),
                12345,
                &FlatSettings {
                    layers: vec![
                        ("minecraft:bedrock".into(), 1),
                        ("minecraft:dirt".into(), 2),
                        ("minecraft:grass_block".into(), 1),
                    ],
                    biome: "minecraft:plains".into(),
                    structure_overrides: Some(Vec::new()),
                    features: false,
                    lakes: false,
                },
                false,
            )
            .expect("flat terrain"),
        )
    }

    #[test]
    #[ignore = "daily"]
    fn detached_generator_stages_lighting_surface_columns_and_close_cleanup() {
        with_game(|game| {
            let level = game.level("minecraft:overworld").unwrap();
            // No generator installed yet -> returns None.
            game.end.set_locator(None);
            assert!(level.detached_generator(game).is_none());

            let terrain = make_flat_terrain(game);
            game.end.set_locator(Some(terrain.clone()));
            let generator = level
                .detached_generator(game)
                .expect("detached generator available");

            let before = (game.world.chunks.len(), game.streaming.pending());
            let bedrock_key = mod_api::__private::block_key("minecraft:bedrock").unwrap();
            let dirt_key = mod_api::__private::block_key("minecraft:dirt").unwrap();
            let grass_key = mod_api::__private::block_key("minecraft:grass_block").unwrap();

            // Run stages off-thread across worker threads.
            let worker_gen = generator.clone();
            let handle = std::thread::spawn(move || {
                // 1. Biomes stage: biomes populated, blocks empty (all air), unlit.
                let biomes_snap = worker_gen
                    .generate_chunk([4, -3], GenerationStage::Biomes)
                    .expect("biomes stage");
                assert!(!biomes_snap.has_light());
                let plains = biomes_snap.biome_id("minecraft:plains").unwrap();
                assert_eq!(biomes_snap.biome(0, 0, 0), Some(plains));
                assert!(biomes_snap.section(0).unwrap().is_empty());

                // 2. Noise/Surface/Carvers/Features stages: layers present, unlit (no fake light).
                for stage in [
                    GenerationStage::Noise,
                    GenerationStage::Surface,
                    GenerationStage::Carvers,
                    GenerationStage::Features,
                ] {
                    let snap = worker_gen.generate_chunk([4, -3], stage).expect("stage ok");
                    assert!(!snap.has_light(), "{stage:?} must not fabricate light");
                    assert_eq!(snap.block_state(0, -64, 0).unwrap().block(), bedrock_key);
                    assert_eq!(snap.block_state(0, -63, 0).unwrap().block(), dirt_key);
                    assert_eq!(snap.block_state(0, -61, 0).unwrap().block(), grass_key);
                    assert_eq!(snap.biome(0, 0, 0), Some(plains));
                }

                // 3. Light stage: runs Sunrise's real light engine -> has_light() == true and
                // open sky above grass_block has full skylight 15.
                let lit_snap = worker_gen
                    .generate_chunk([4, -3], GenerationStage::Light)
                    .expect("light stage");
                assert!(lit_snap.has_light());
                assert_eq!(lit_snap.light(0, -60, 0), Some((15, 0)));
                assert_eq!(lit_snap.light(0, -64, 0), Some((0, 0)));

                // 4. sample_surface_columns off-thread.
                let columns = [(100000, -100000), (0, 0), (-1, 31)];
                let surfaces = worker_gen
                    .sample_surface_columns(&columns)
                    .expect("surface sample ok");
                assert_eq!(surfaces.len(), columns.len());
                for col in surfaces {
                    let col = col.unwrap();
                    assert_eq!(col.height, -60);
                    assert_eq!(col.block, grass_key);
                    assert_eq!(col.water, None);
                    assert_eq!(col.biome, "minecraft:plains");
                }
                assert!(worker_gen.sample_surface_columns(&[]).unwrap().is_empty());

                // 5. Out-of-bounds coordinates return InvalidPos without panicking.
                assert_eq!(
                    worker_gen
                        .generate_chunk([i32::MAX, 0], GenerationStage::Carvers)
                        .err(),
                    Some(GenerationError::InvalidPos)
                );
                assert_eq!(
                    worker_gen.sample_surface_columns(&[(i32::MAX, 0)]),
                    Err(GenerationError::InvalidPos)
                );
            });
            handle.join().expect("detached worker succeeded");

            // Purely in-memory: live World and Streaming were untouched.
            assert_eq!(before, (game.world.chunks.len(), game.streaming.pending()));

            // Closing the level immediately invalidates handles and drops all pooled workers.
            let dim = game.dim;
            game.close_detached_generators_for_dim(dim);
            assert_eq!(
                generator
                    .generate_chunk([4, -3], GenerationStage::Biomes)
                    .err(),
                Some(GenerationError::Unavailable)
            );
            assert_eq!(
                generator.sample_surface_columns(&[(0, 0)]),
                Err(GenerationError::Unavailable)
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn detached_carvers_matches_live_worldgen_bit_for_bit() {
        with_game(|game| {
            let directory = game.options.resources.join("worldgen");
            let terrain = Arc::new(
                VanillaTerrain::for_world(
                    TerrainResources {
                        data: Some(&game.pack.data_changes),
                        dir: &directory,
                        registry: game.pack.assets.registry,
                        biomes: world_biomes!(game),
                        item_data: &game.res.core.item_data,
                    },
                    424242,
                    "minecraft:normal",
                    "minecraft:overworld",
                    false,
                )
                .expect("overworld terrain"),
            );
            game.end.set_locator(Some(terrain.clone()));
            let level = game.level("minecraft:overworld").unwrap();
            let generator = level.detached_generator(game).unwrap();

            // Compare detached Carvers snapshot against live GeneratorWorker::generate_proto
            // on the exact same seed & chunk coordinate.
            let pos = [3, -5];
            let detached = generator
                .generate_chunk(pos, GenerationStage::Carvers)
                .expect("detached carvers");
            assert!(!detached.has_light());

            let mut live_worker = terrain.new_worker();
            let live_proto = live_worker
                .generate_proto(ChunkPos::new(pos[0], pos[1]))
                .expect("live proto");
            let states = super::super::api_values::block_state::current().unwrap();
            let (bx, bz) = (pos[0] * 16, pos[1] * 16);
            for y in detached.min_y()..(detached.min_y() + detached.height()) {
                for z in 0..16 {
                    for x in 0..16 {
                        let expected = states.to_api(live_proto.block(bx + x, y, bz + z)).unwrap();
                        let actual = detached.block_state(x as u8, y, z as u8).unwrap();
                        assert_eq!(actual, expected, "block mismatch at local ({x}, {y}, {z})");
                    }
                }
            }
        });
    }
}
