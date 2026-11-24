//! Read-only saved unloaded chunk inspection (`SavedChunkReader`, #5235).
//!
//! Hands mods a `Send + Sync + 'static` handle backed by a `Weak<SaveDir>` and an open-state
//! `AtomicBool` cleared when the level closes. Reads inspect `SaveDir`'s pending-write queue first
//! and then open `region/r.<rx>.<rz>.mca` directly read-only (with write-stamp fallback to the
//! locked `RegionCache` path), decoding blocks, biomes, heightmaps, and light into a G1
//! `ChunkSnapshot` without loading chunks into `World`, spawning entities, or firing hooks.
use super::native_chunk_snapshot::EngineChunkSnapshot;
use super::{Dim, Game};
use mod_api::__private::SavedChunkReaderBackend;
use mod_api::{ChunkSnapshot, DimensionKey, LevelRef, SavedChunkError, SavedChunkReader};
use std::ffi::OsStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use sunrise_assets::block_facts::BlockFacts;
use sunrise_client::biome::Biomes;
use sunrise_client::save::SaveDir;
use sunrise_client::streaming::chunk_pos_is_valid;
use sunrise_world::registry::BlockRegistry;

fn dim_of(key: DimensionKey) -> Dim {
    match key {
        DimensionKey::OVERWORLD => Dim::Overworld,
        DimensionKey::NETHER => Dim::Nether,
        DimensionKey::END => Dim::End,
        other => Dim::Extra(other),
    }
}

fn region_overlaps_valid_chunks(rx: i32, rz: i32) -> bool {
    let x_near = if rx < 0 {
        rx.checked_mul(32).and_then(|v| v.checked_add(31))
    } else {
        rx.checked_mul(32)
    };
    let z_near = if rz < 0 {
        rz.checked_mul(32).and_then(|v| v.checked_add(31))
    } else {
        rz.checked_mul(32)
    };
    match (x_near, z_near) {
        (Some(x), Some(z)) => chunk_pos_is_valid((x, z)),
        _ => false,
    }
}

struct EngineSavedChunkReader {
    save: Weak<SaveDir>,
    open: Arc<AtomicBool>,
    registry: &'static BlockRegistry,
    states: Arc<super::api_values::block_state::Index>,
    biomes: Arc<Biomes>,
    properties: Arc<BlockFacts>,
}

impl EngineSavedChunkReader {
    fn active_save(&self) -> Result<Arc<SaveDir>, SavedChunkError> {
        if !self.open.load(Ordering::Acquire) {
            return Err(SavedChunkError::Unavailable);
        }
        self.save.upgrade().ok_or(SavedChunkError::Unavailable)
    }
}

impl SavedChunkReaderBackend for EngineSavedChunkReader {
    fn read(&self, pos: [i32; 2]) -> Result<Option<ChunkSnapshot>, SavedChunkError> {
        let save = self.active_save()?;
        if !chunk_pos_is_valid((pos[0], pos[1])) {
            return Err(SavedChunkError::InvalidPos);
        }
        let chunk = save
            .read_saved_chunk_readonly(pos[0], pos[1], self.registry, &self.biomes)
            .map_err(|error| SavedChunkError::Io(error.to_string()))?;
        if !self.open.load(Ordering::Acquire) {
            return Err(SavedChunkError::Unavailable);
        }
        Ok(chunk.map(|column| {
            let has_light = column.lit || column.light_loaded;
            EngineChunkSnapshot::from_chunk(
                &column,
                self.states.clone(),
                self.biomes.clone(),
                self.properties.clone(),
                has_light,
            )
        }))
    }

    fn region_chunks(&self, rx: i32, rz: i32) -> Result<Vec<[i32; 2]>, SavedChunkError> {
        let save = self.active_save()?;
        if !region_overlaps_valid_chunks(rx, rz) {
            return Err(SavedChunkError::InvalidPos);
        }
        let chunks = save
            .saved_region_chunks(rx, rz)
            .map_err(|error| SavedChunkError::Io(error.to_string()))?;
        if !self.open.load(Ordering::Acquire) {
            return Err(SavedChunkError::Unavailable);
        }
        Ok(chunks)
    }
}

impl Game<'_> {
    fn level_save_dir(&self, dim: Dim) -> Option<&Arc<SaveDir>> {
        let save = if dim == self.dim {
            &self.save
        } else {
            &self.levels.parked.get(&dim)?.save
        };
        if save.root.as_os_str().is_empty()
            || save.root.file_name() == Some(OsStr::new(".sunrise-title"))
        {
            return None;
        }
        Some(save)
    }

    /// Server-side backend for `LevelRef::saved_chunk_reader`.
    pub(super) fn level_saved_chunk_reader(&self, level: LevelRef) -> Option<SavedChunkReader> {
        if !self.is_logical_server() {
            return None;
        }
        self.native_level_world(level)?;
        let dim = dim_of(level.dimension());
        let save = self.level_save_dir(dim)?;
        let states = super::api_values::block_state::current().ok()?;
        let biomes = world_biomes!(self).clone();
        let properties = self.pack.assets.properties.clone();
        let open = self
            .saved_chunk_readers
            .borrow_mut()
            .entry(dim)
            .or_insert_with(|| Arc::new(AtomicBool::new(true)))
            .clone();
        Some(SavedChunkReader::from_backend(Arc::new(
            EngineSavedChunkReader {
                save: Arc::downgrade(save),
                open,
                registry: self.pack.assets.registry,
                states,
                biomes,
                properties,
            },
        )))
    }

    /// Mark all `SavedChunkReader` handles issued for `dim` as unavailable on level close.
    pub(super) fn close_saved_chunk_readers_for_dim(&mut self, dim: Dim) {
        if let Some(open) = self.saved_chunk_readers.borrow_mut().remove(&dim) {
            open.store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::native_entity_refs::tests::with_game;
    use mod_api::SavedChunkError;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use sunrise_client::chunk::{Chunk, Light};
    use sunrise_client::save::{LevelShape, SaveDir, dimension_dir};
    use sunrise_core::nbt::{Limits, NamedTag};
    use sunrise_world::region::{ChunkPos, ChunkRecord, Compression, RegionFile};

    fn temp_world_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sunrise-saved-chunk-reader-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    #[ignore = "daily"]
    fn saved_chunk_reader_reads_off_thread_last_saved_vs_dirty_and_closes_cleanly() {
        with_game(|game| {
            let world_root = temp_world_dir("basic");
            let dim_root = dimension_dir(&world_root, "minecraft:overworld");
            std::fs::create_dir_all(&dim_root).unwrap();
            let save = Arc::new(SaveDir::new(
                dim_root.clone(),
                LevelShape::OVERWORLD,
                Some(game.res.shared_items()),
            ));
            game.save = save.clone();

            let stone_key = mod_api::__private::block_key("minecraft:stone").unwrap();
            let dirt_key = mod_api::__private::block_key("minecraft:dirt").unwrap();
            let stone_id = super::super::api_values::default_state(stone_key)
                .unwrap()
                .0;
            let dirt_id = super::super::api_values::default_state(dirt_key).unwrap().0;
            let biomes = world_biomes!(game).clone();
            let desert_idx = biomes.try_id("minecraft:desert").unwrap();

            // Save chunk (4, -2) to disk without keeping it in game.world.chunks.
            let mut saved_col = Chunk::empty(4, -2);
            saved_col.lit = true;
            {
                let sec = Arc::make_mut(&mut saved_col.sections[4]);
                sec.set(0, stone_id);
                let mut light_arr = Box::new([0xF0u8; 4096]);
                light_arr[0] = (11 << 4) | 5;
                sec.light = Light::packed(light_arr);
            }
            saved_col.set_biome_cell(0, 16, 0, desert_idx);
            save.save(&[Arc::new(saved_col)], game.pack.assets.registry, &biomes)
                .expect("save unloaded chunk");
            assert!(!game.world.chunks.contains_key(&(4, -2)));

            // Save chunk (1, 1) to disk as stone, then load it into game.world and dirty it to dirt
            // WITHOUT saving: SavedChunkReader must see stone (last saved), ChunkRef::snapshot must
            // see dirt (live in-memory state).
            let mut dirty_col = Chunk::empty(1, 1);
            dirty_col.lit = true;
            Arc::make_mut(&mut dirty_col.sections[4]).set(0, stone_id);
            save.save(
                &[Arc::new(dirty_col.clone())],
                game.pack.assets.registry,
                &biomes,
            )
            .expect("save chunk (1, 1)");
            Arc::make_mut(&mut dirty_col.sections[4]).set(0, dirt_id);
            game.world.chunks.insert((1, 1), Arc::new(dirty_col));

            let level = game.level("minecraft:overworld").unwrap();
            let reader = level
                .saved_chunk_reader(game)
                .expect("server level provides SavedChunkReader");

            // Background thread reads saved unloaded chunk without loading it into game.world.
            let bg_reader = reader.clone();
            let worker = std::thread::spawn(move || {
                let snap = bg_reader
                    .read([4, -2])
                    .expect("read ok")
                    .expect("saved chunk exists");
                assert_eq!(snap.pos(), [4, -2]);
                assert_eq!(snap.block_state(0, 0, 0).unwrap().block(), stone_key);
                assert_eq!(snap.light(0, 0, 0), Some((11, 5)));
                assert!(snap.has_light());
                let desert = snap.biome_id("minecraft:desert").unwrap();
                assert_eq!(snap.biome(0, 0, 0), Some(desert));

                // Unsaved chunk returns Ok(None).
                assert!(bg_reader.read([99, 99]).expect("read ok").is_none());

                // Out-of-bounds returns InvalidPos.
                assert_eq!(
                    bg_reader.read([i32::MAX, 0]),
                    Err(SavedChunkError::InvalidPos)
                );
                assert_eq!(
                    bg_reader.region_chunks(i32::MAX, 0),
                    Err(SavedChunkError::InvalidPos)
                );

                // Region listing finds saved chunks without decompressing them.
                assert_eq!(bg_reader.region_chunks(0, -1).unwrap(), vec![[4, -2]]);
                assert_eq!(bg_reader.region_chunks(0, 0).unwrap(), vec![[1, 1]]);
                assert!(bg_reader.region_chunks(7, 7).unwrap().is_empty());

                // Dirty loaded chunk (1, 1) still reads last saved state (stone).
                let saved_11 = bg_reader.read([1, 1]).unwrap().unwrap();
                assert_eq!(saved_11.block_state(0, 0, 0).unwrap().block(), stone_key);
            });
            worker.join().expect("background reader succeeded");

            // Unloaded chunk (4, -2) was never loaded into game.world.
            assert!(!game.world.chunks.contains_key(&(4, -2)));

            // Live ChunkRef::snapshot sees the unsaved dirty edit (dirt).
            let live_11 = game.chunk(level, [1, 1]).unwrap().snapshot(game).unwrap();
            assert_eq!(live_11.block_state(0, 0, 0).unwrap().block(), dirt_key);

            // Closing the level immediately invalidates the reader without retaining SaveDir.
            game.native_levels_closing();
            assert_eq!(reader.read([4, -2]), Err(SavedChunkError::Unavailable));
            assert_eq!(
                reader.region_chunks(0, -1),
                Err(SavedChunkError::Unavailable)
            );

            drop(save);
            let _ = std::fs::remove_dir_all(&world_root);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn saved_chunk_reader_concurrent_pending_writes_and_multithread_datafix_upgrade() {
        with_game(|game| {
            let world_root = temp_world_dir("concurrent");
            let dim_root = dimension_dir(&world_root, "minecraft:overworld");
            std::fs::create_dir_all(dim_root.join("region")).unwrap();
            let save = Arc::new(SaveDir::new(
                dim_root.clone(),
                LevelShape::OVERWORLD,
                Some(game.res.shared_items()),
            ));
            game.save = save.clone();

            let stone_key = mod_api::__private::block_key("minecraft:stone").unwrap();
            let dirt_key = mod_api::__private::block_key("minecraft:dirt").unwrap();
            let short_grass_key = mod_api::__private::block_key("minecraft:short_grass").unwrap();
            let stone_id = super::super::api_values::default_state(stone_key)
                .unwrap()
                .0;
            let dirt_id = super::super::api_values::default_state(dirt_key).unwrap().0;
            let biomes = world_biomes!(game).clone();

            // 1. Write an older-DataVersion (3685, pre-1.20.3) chunk containing "minecraft:grass"
            // directly into r.0.0.mca at chunk (2, 2), then read & upgrade it concurrently across
            // 4 worker threads to prove upgrade_record has no shared mutable state races.
            let old_tag = sunrise_core::nbt::parse_snbt(
                r#"{DataVersion:3685,xPos:2,yPos:-4,zPos:2,Status:"minecraft:full",sections:[{Y:0b,block_states:{palette:[{Name:"minecraft:grass"}]},biomes:{palette:["minecraft:plains"]}}]}"#,
            )
            .expect("valid snbt")
            .0;
            {
                let mut region = RegionFile::open_writable(dim_root.join("region/r.0.0.mca"), true)
                    .expect("open writable region");
                let record = ChunkRecord::from_nbt(
                    &NamedTag {
                        name: "".into(),
                        tag: old_tag,
                    },
                    Compression::Zlib,
                    0,
                    Limits::saved(),
                )
                .expect("encode old chunk");
                region
                    .write(ChunkPos::new(2, 2), &record)
                    .expect("write old chunk");
                region.sync().expect("sync region");
            }

            let level = game.level("minecraft:overworld").unwrap();
            let reader = level.saved_chunk_reader(game).unwrap();

            let upgrade_threads: Vec<_> = (0..4)
                .map(|_| {
                    let r = reader.clone();
                    std::thread::spawn(move || {
                        for _ in 0..8 {
                            let snap = r
                                .read([2, 2])
                                .expect("upgrade read ok")
                                .expect("upgraded chunk present");
                            assert_eq!(snap.block_state(0, 0, 0).unwrap().block(), short_grass_key);
                        }
                    })
                })
                .collect();
            for handle in upgrade_threads {
                handle.join().expect("concurrent upgrade thread succeeded");
            }

            // 2. Concurrent pending->disk write/read test: repeatedly queue async saves for
            // chunk (3, 3) via save_later while a reader thread continuously reads (3, 3) and
            // lists region (0, 0). Because writes.rs removes pending only AFTER write_region_records
            // finishes, the reader must NEVER observe Ok(None) or a missing entry in region_chunks.
            let mut col_stone = Chunk::empty(3, 3);
            col_stone.lit = true;
            Arc::make_mut(&mut col_stone.sections[4]).set(0, stone_id);
            let mut col_dirt = Chunk::empty(3, 3);
            col_dirt.lit = true;
            Arc::make_mut(&mut col_dirt.sections[4]).set(0, dirt_id);
            let col_stone = Arc::new(col_stone);
            let col_dirt = Arc::new(col_dirt);

            // Seed initial pending save so (3, 3) exists from iteration 0.
            save.save_later(&[col_stone.clone()], game.pack.assets.registry, &biomes)
                .expect("seed save_later");

            let stop = Arc::new(AtomicBool::new(false));
            let stop_reader = stop.clone();
            let conc_reader = reader.clone();
            let reader_thread = std::thread::spawn(move || {
                let mut reads = 0usize;
                while !stop_reader.load(Ordering::Relaxed) || reads < 20 {
                    let snap = conc_reader
                        .read([3, 3])
                        .expect("concurrent read must not error")
                        .expect("chunk (3, 3) must never vanish between pending and disk");
                    let block = snap.block_state(0, 0, 0).unwrap().block();
                    assert!(
                        block == stone_key || block == dirt_key,
                        "unexpected block during concurrent save/read: {block:?}"
                    );
                    let listed = conc_reader
                        .region_chunks(0, 0)
                        .expect("region_chunks must not error");
                    assert!(
                        listed.contains(&[3, 3]),
                        "chunk (3, 3) must stay visible in region_chunks during pending->disk flush"
                    );
                    reads += 1;
                }
            });

            for i in 0..24 {
                let col = if i % 2 == 0 {
                    col_dirt.clone()
                } else {
                    col_stone.clone()
                };
                save.save_later(&[col], game.pack.assets.registry, &biomes)
                    .expect("async save_later");
            }
            save.save(&[col_stone.clone()], game.pack.assets.registry, &biomes)
                .expect("synchronous flush after async writes");
            stop.store(true, Ordering::Relaxed);
            reader_thread
                .join()
                .expect("concurrent writer/reader thread succeeded");

            game.native_levels_closing();
            drop(save);
            let _ = std::fs::remove_dir_all(&world_root);
        });
    }
}
