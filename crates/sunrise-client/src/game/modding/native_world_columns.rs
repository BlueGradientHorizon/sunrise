//! Column reads for server mods on `LevelRef` (and the client-side surface-height helper):
//! heights, generator surfaces, and block slices round each column's height.
use super::Game;
use core::ops::ControlFlow;
use mod_api::{ColumnBlockSlice, ColumnReadError, Heightmap, LevelReadError, LevelRef};

/// Temporary client-side cap for `client_surface_heights` (removed by P3-42).
const CLIENT_MAX_COLUMNS: usize = 1 << 16;

impl Game<'_> {
    /// For a client mod: each column's height by `heightmap` in the world this window shows,
    /// none for a column whose chunk this client does not hold. `Err` says why nothing was read:
    /// no client world, or more than 65,536 columns (`CLIENT_MAX_COLUMNS`).
    pub(super) fn client_surface_heights(
        &self,
        columns: &[(i32, i32)],
        heightmap: Heightmap,
    ) -> Result<Vec<Option<i32>>, String> {
        if !self.remote.as_ref().is_some_and(|r| r.logged_in) {
            return Err("no client world is open".to_string());
        }
        if columns.len() > CLIENT_MAX_COLUMNS {
            return Err(format!(
                "{} columns, more than {CLIENT_MAX_COLUMNS} a call",
                columns.len()
            ));
        }
        Ok(self.loaded_column_heights(columns, heightmap))
    }
    /// For a server mod: visit each column's height by `heightmap` in this server level (active
    /// or parked), lending `None` for an unloaded column and stopping on `ControlFlow::Break(())`.
    pub(super) fn level_visit_surface_heights(
        &self,
        level: LevelRef,
        columns: &[(i32, i32)],
        heightmap: Heightmap,
        visit: &mut dyn FnMut((i32, i32), Option<i32>) -> ControlFlow<()>,
    ) -> Result<(), LevelReadError> {
        let world = self.native_level_world_result(level)?;
        let started = std::time::Instant::now();
        let mut last: Option<((i32, i32), bool)> = None;
        for &(x, z) in columns {
            let key = (x >> 4, z >> 4);
            let loaded = match last {
                Some((k, l)) if k == key => l,
                _ => {
                    let l = world.chunks.contains_key(&key);
                    last = Some((key, l));
                    l
                }
            };
            let height = loaded.then(|| self.world_column_height(world, heightmap, x, z));
            if visit((x, z), height).is_break() {
                break;
            }
        }
        self.native_read_logged(started, "height query", columns.len());
        Ok(())
    }
    /// For a server mod: visit `below` blocks under and `above` blocks from each column's height
    /// by `heightmap` in this server level (active or parked), lending a reusable
    /// [`ColumnBlockSlice`] per column and stopping on `ControlFlow::Break(())`.
    pub(super) fn level_visit_columns(
        &self,
        level: LevelRef,
        columns: &[(i32, i32)],
        heightmap: Heightmap,
        range: (u32, u32),
        visit: &mut dyn for<'a> FnMut(ColumnBlockSlice<'a>) -> ControlFlow<()>,
    ) -> Result<(), ColumnReadError> {
        let world = self.native_level_world_result(level)?;
        let (below, above) = range;
        let height = world.layout().height();
        let per = below
            .checked_add(above)
            .filter(|&per| per > 0 && i64::from(per) <= i64::from(height))
            .ok_or(ColumnReadError::InvalidRange)? as usize;
        let index = super::api_values::block_state::current()?;
        let air = index.to_api(world.air)?;
        let biomes = world_biomes!(self);
        let started = std::time::Instant::now();
        let mut scratch = vec![air; per];
        for &(x, z) in columns {
            let Some(chunk) = world.chunks.get(&(x >> 4, z >> 4)) else {
                scratch.fill(air);
                let slice: ColumnBlockSlice<'_> = mod_api::__private::built::ColumnBlockSlice {
                    pos: (x, z),
                    base: None,
                    water_color: 0,
                    blocks: &scratch,
                }
                .into();
                if visit(slice).is_break() {
                    break;
                }
                continue;
            };
            let base = self.world_column_height(world, heightmap, x, z);
            let water_color = biomes.get(chunk.biome(x, base - 1, z)).water;
            let (bottom, top) = (chunk.min_y(), chunk.max_y());
            let y0 = i64::from(base) - i64::from(below);
            for (slot, y) in scratch.iter_mut().zip(y0..y0 + per as i64) {
                *slot = match i32::try_from(y) {
                    Ok(y) if (bottom..top).contains(&y) => {
                        index.to_api(chunk.block(x & 15, y, z & 15))?
                    }
                    _ => air,
                };
            }
            let slice: ColumnBlockSlice<'_> = mod_api::__private::built::ColumnBlockSlice {
                pos: (x, z),
                base: Some(base),
                water_color,
                blocks: &scratch,
            }
            .into();
            if visit(slice).is_break() {
                break;
            }
        }
        self.native_read_logged(started, "column read", columns.len());
        Ok(())
    }
    /// For a server mod: visit the level generator's undecorated surface for this server level
    /// (active or parked), lending a borrowed [`mod_api::TerrainColumnView`] per column and
    /// stopping on `ControlFlow::Break(())`.
    pub(super) fn level_visit_terrain_surface(
        &self,
        level: LevelRef,
        columns: &[(i32, i32)],
        visit: &mut dyn for<'a> FnMut(
            (i32, i32),
            Option<mod_api::TerrainColumnView<'a>>,
        ) -> ControlFlow<()>,
    ) -> Result<(), LevelReadError> {
        self.native_level_world_result(level)?;
        let locator = if level.dimension() == self.dim.key() {
            self.end.locator()
        } else {
            self.levels.parked_locator_of(level.dimension_id())
        };
        let started = std::time::Instant::now();
        match locator {
            None => {
                for &(x, z) in columns {
                    if visit((x, z), None).is_break() {
                        break;
                    }
                }
            }
            Some(generator) => {
                let index = super::native_worldgen::prepare(self.pack.assets.registry)
                    .map_err(|_| LevelReadError::Unavailable)?;
                let biomes = world_biomes!(self);
                for &(x, z) in columns {
                    let view = match generator.terrain_surface(x, z) {
                        None => None,
                        Some(surface) => {
                            let state = index
                                .to_api(surface.state)
                                .map_err(|_| LevelReadError::Unavailable)?;
                            Some(
                                mod_api::__private::built::TerrainColumnView {
                                    height: surface.height,
                                    block: state.block(),
                                    water: surface.water.map(|(height, _)| height),
                                    biome: biomes.name(surface.biome),
                                }
                                .into(),
                            )
                        }
                    };
                    if visit((x, z), view).is_break() {
                        break;
                    }
                }
            }
        }
        self.native_read_logged(started, "terrain surface query", columns.len());
        Ok(())
    }
    /// A read's time logged with `SUNRISE_SPIKES`, as an edit's is.
    fn native_read_logged(&self, started: std::time::Instant, what: &str, columns: usize) {
        let ms = started.elapsed().as_secs_f32() * 1000.0;
        if self.options.spikes.is_some_and(|threshold| ms > threshold) {
            log::info!("Sunrise: slow mod {what} {ms:.1} ms ({columns} columns)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::native_entity_refs::tests::with_game;
    use core::ops::ControlFlow;
    use mod_api::{ColumnReadError, DimensionKey, Heightmap, LevelReadError};
    use std::sync::Arc;
    use sunrise_client::vanillagen::{TerrainResources, VanillaTerrain};
    use sunrise_world::worldgen::generator::FlatSettings;

    #[test]
    #[ignore = "daily"]
    fn terrain_surface_reads_flat_layers_without_chunks() {
        with_game(|game| {
            let level = game.level_ref(game.native_lifecycle.epoch().unwrap(), game.dim.key());
            let directory = game.options.resources.join("worldgen");
            let terrain = VanillaTerrain::for_flat(
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
            .unwrap();
            game.end.set_locator(Some(Arc::new(terrain)));
            let before = (game.world.chunks.len(), game.streaming.pending());
            let columns = [(100000, -100000), (0, 0), (-1, 31), (100000, -100000)];
            let mut answer = Vec::new();
            game.level_visit_terrain_surface(level, &columns, &mut |pos, col| {
                answer.push((
                    pos,
                    col.map(|c| (c.height, c.block, c.water, c.biome.to_owned())),
                ));
                ControlFlow::Continue(())
            })
            .unwrap();
            assert_eq!(answer.len(), columns.len());
            let grass = mod_api::__private::block_key("minecraft:grass_block").unwrap();
            // Four layers from the overworld minimum -64, independent of the sampler.
            for (index, (pos, column)) in answer.iter().enumerate() {
                assert_eq!(*pos, columns[index]);
                let (height, block, water, biome) = column.as_ref().unwrap();
                assert_eq!(*height, -60);
                assert_eq!(*block, grass);
                assert_eq!(*water, None);
                assert_eq!(biome, "minecraft:plains");
            }
            let mut again = Vec::new();
            game.level_visit_terrain_surface(level, &columns, &mut |pos, col| {
                again.push((
                    pos,
                    col.map(|c| (c.height, c.block, c.water, c.biome.to_owned())),
                ));
                ControlFlow::Continue(())
            })
            .unwrap();
            assert_eq!(again, answer);
            // Early break stops after the first column.
            let mut count = 0;
            game.level_visit_terrain_surface(level, &columns, &mut |_, _| {
                count += 1;
                ControlFlow::Break(())
            })
            .unwrap();
            assert_eq!(count, 1);
            assert_eq!(before, (game.world.chunks.len(), game.streaming.pending()));
        });
    }

    #[test]
    #[ignore = "daily"]
    fn terrain_surface_refuses_before_sampling() {
        with_game(|game| {
            let level = game.level_ref(game.native_lifecycle.epoch().unwrap(), game.dim.key());
            let stale = game.level_ref(level.epoch(), DimensionKey::NETHER);
            game.end.set_locator(None);
            let mut visited = 0;
            assert_eq!(
                game.level_visit_terrain_surface(stale, &[(0, 0)], &mut |_, _| {
                    visited += 1;
                    ControlFlow::Continue(())
                }),
                Err(LevelReadError::Stale)
            );
            assert_eq!(visited, 0);
            // No invented column cap: more than 4096 columns visit cleanly.
            let columns = vec![(0, 0); 5000];
            let mut seen = 0;
            game.level_visit_terrain_surface(level, &columns, &mut |_, col| {
                assert!(col.is_none());
                seen += 1;
                ControlFlow::Continue(())
            })
            .unwrap();
            assert_eq!(seen, 5000);
            let mut empty_seen = 0;
            game.level_visit_terrain_surface(level, &[], &mut |_, _| {
                empty_seen += 1;
                ControlFlow::Continue(())
            })
            .unwrap();
            assert_eq!(empty_seen, 0);
            // Client refuses before sampling.
            game.options.execution_role = crate::game::execution_role::ExecutionRole::Client;
            assert_eq!(
                game.level_visit_terrain_surface(level, &[(0, 0)], &mut |_, _| {
                    visited += 1;
                    ControlFlow::Continue(())
                }),
                Err(LevelReadError::Unavailable)
            );
            assert_eq!(visited, 0);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn read_columns_returns_server_water_colors_without_colormaps() {
        use std::path::Path;
        use sunrise_assets::resource_source::ResourceRoot;
        use sunrise_client::{biome::Biomes, chunk::Chunk};

        fn link_tree(src: &Path, dst: &Path) {
            #[cfg(unix)]
            std::os::unix::fs::symlink(src, dst).unwrap();
            #[cfg(not(unix))]
            {
                if src.is_dir() {
                    std::fs::create_dir(dst).unwrap();
                    for entry in std::fs::read_dir(src).unwrap() {
                        let entry = entry.unwrap();
                        link_tree(&entry.path(), &dst.join(entry.file_name()));
                    }
                } else if std::fs::hard_link(src, dst).is_err() {
                    std::fs::copy(src, dst).unwrap();
                }
            }
        }

        fn mirror_omitting(src: &Path, dst: &Path, omit: &[&[&str]]) {
            for entry in std::fs::read_dir(src).unwrap() {
                let entry = entry.unwrap();
                let name = entry.file_name();
                let Some(name_str) = name.to_str() else {
                    link_tree(&entry.path(), &dst.join(&name));
                    continue;
                };
                let under: Vec<&[&str]> = omit
                    .iter()
                    .filter_map(|path| {
                        path.split_first()
                            .filter(|(head, _)| **head == name_str)
                            .map(|(_, tail)| tail)
                    })
                    .collect();
                if under.iter().any(|rest| rest.is_empty()) {
                    continue;
                }
                let target = dst.join(&name);
                if under.is_empty() {
                    link_tree(&entry.path(), &target);
                } else {
                    std::fs::create_dir(&target).unwrap();
                    mirror_omitting(&entry.path(), &target, &under);
                }
            }
        }

        const OMIT: &[&[&str]] = &[
            &[sunrise_core::prepared_files::FILE_NAME],
            &["assets", "minecraft", "textures", "colormap", "grass.png"],
            &["assets", "minecraft", "textures", "colormap", "foliage.png"],
            &[
                "assets",
                "minecraft",
                "textures",
                "colormap",
                "dry_foliage.png",
            ],
        ];
        let scratch = tempfile::tempdir().unwrap();
        let resources = scratch.path().join("resources");
        std::fs::create_dir(&resources).unwrap();
        mirror_omitting(
            &super::super::native_entity_refs::tests::fixture_root(),
            &resources,
            OMIT,
        );
        for name in ["grass.png", "foliage.png", "dry_foliage.png"] {
            assert!(
                !resources
                    .join("assets/minecraft/textures/colormap")
                    .join(name)
                    .exists(),
                "private fixture must omit {name}"
            );
        }
        assert!(
            Biomes::load(&ResourceRoot::directory(resources.clone())).is_err(),
            "client biome load must fail when colormaps are absent"
        );

        super::super::native_entity_refs::tests::with_game_role(
            resources,
            crate::game::execution_role::ExecutionRole::DedicatedServer,
            |game| {
                game.options.dedicated = Some(Vec::new());
                assert!(game.world_biomes.is_none());
                let plains = world_biomes!(game).id("minecraft:plains");
                let swamp = world_biomes!(game).id("minecraft:swamp");
                let mut chunk = Chunk::empty(0, 0);
                for qy in 0..chunk.sections.len() as i32 * 4 {
                    for qz in 0..4 {
                        chunk.set_biome_cell(0, qy, qz, plains);
                        chunk.set_biome_cell(1, qy, qz, swamp);
                    }
                }
                game.world.chunks.insert((0, 0), Arc::new(chunk));
                let stone = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:stone")
                    .unwrap();
                game.world.set(0, 64, 0, stone).unwrap();
                game.world.set(4, 64, 0, stone).unwrap();

                let level = game.level_ref(game.native_lifecycle.epoch().unwrap(), game.dim.key());
                let mut bases = Vec::new();
                let mut water_colors = Vec::new();
                let mut blocks_per_col = Vec::new();
                game.level_visit_columns(
                    level,
                    &[(0, 0), (4, 0), (32, 0)],
                    Heightmap::WorldSurface,
                    (1, 1),
                    &mut |slice| {
                        bases.push(slice.base);
                        water_colors.push(slice.water_color);
                        blocks_per_col.push(slice.blocks.to_vec());
                        ControlFlow::Continue(())
                    },
                )
                .unwrap();
                assert_eq!(bases, vec![Some(65), Some(65), None]);
                assert_eq!(water_colors, vec![0x3F_76E4, 0x61_7B64, 0]);
                assert_eq!(blocks_per_col[0].len(), 2);
                assert_eq!(blocks_per_col[2].len(), 2);
                // Early break stops visiting and invalid ranges refuse before visiting.
                let mut seen = 0;
                game.level_visit_columns(
                    level,
                    &[(0, 0), (4, 0)],
                    Heightmap::WorldSurface,
                    (1, 1),
                    &mut |_| {
                        seen += 1;
                        ControlFlow::Break(())
                    },
                )
                .unwrap();
                assert_eq!(seen, 1);
                assert_eq!(
                    game.level_visit_columns(
                        level,
                        &[(0, 0)],
                        Heightmap::WorldSurface,
                        (0, 0),
                        &mut |_| ControlFlow::Continue(()),
                    ),
                    Err(ColumnReadError::InvalidRange)
                );
                assert_eq!(
                    game.level_visit_columns(
                        level,
                        &[(0, 0)],
                        Heightmap::WorldSurface,
                        (u32::MAX, 1),
                        &mut |_| ControlFlow::Continue(()),
                    ),
                    Err(ColumnReadError::InvalidRange)
                );
            },
        );
    }
}
