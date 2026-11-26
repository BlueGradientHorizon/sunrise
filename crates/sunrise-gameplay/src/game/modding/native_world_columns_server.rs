//! Server-side column reads for native mods.
use crate::game::ServerGame;
use sunrise_gameplay_common::game::block_edits_common::MAX_COLUMNS;

impl ServerGame<'_> {
    /// For a server mod: each column's height by `heightmap` in the authoritative level `dimension`.
    pub(in crate::game) fn surface_heights(
        &self,
        dimension: &str,
        columns: &[(i32, i32)],
        heightmap: mod_api::Heightmap,
    ) -> Result<Vec<Option<i32>>, String> {
        if !self.native_level_open(dimension) {
            return Err(format!("{dimension} is not the level being ticked"));
        }
        if columns.len() > MAX_COLUMNS {
            return Err(format!(
                "{} columns, more than {MAX_COLUMNS} a call",
                columns.len()
            ));
        }
        let started = std::time::Instant::now();
        let out = self.common.loaded_column_heights(columns, heightmap);
        self.common.native_read_logged(started, "height query", columns.len());
        Ok(out)
    }

    /// For a server mod: `below` blocks under and `above` blocks from each column's height.
    pub fn read_columns(
        &self,
        dimension: &str,
        columns: &[(i32, i32)],
        heightmap: mod_api::Heightmap,
        range: (u32, u32),
    ) -> Result<mod_api::ColumnBlocks, String> {
        let (below, above) = range;
        if !self.native_level_open(dimension) {
            return Err(format!("{dimension} is not the level being ticked"));
        }
        let started = std::time::Instant::now();
        let out = self.read_loaded_block_columns(columns, heightmap, below, above)?;
        self.common.native_read_logged(started, "column read", columns.len());
        Ok(out)
    }

    pub fn terrain_surface(
        &self,
        dimension: &str,
        columns: &[(i32, i32)],
    ) -> Result<Vec<Option<mod_api::TerrainColumn>>, String> {
        if !self.native_level_open(dimension) {
            return Err(format!("{dimension} is not the level being ticked"));
        }
        if columns.len() > mod_api::MAX_TERRAIN_COLUMNS {
            return Err(format!(
                "{} columns, more than {} a call",
                columns.len(),
                mod_api::MAX_TERRAIN_COLUMNS
            ));
        }
        let started = std::time::Instant::now();
        let out = match self.common.end.locator() {
            None => vec![None; columns.len()],
            Some(generator) => {
                let index = super::native_worldgen::prepare(self.common.pack.assets.registry)?;
                generator
                    .terrain_surfaces(columns)
                    .into_iter()
                    .map(|surface| {
                        surface
                            .map(|surface| {
                                let state = index
                                    .to_api(surface.state)
                                    .map_err(|error| format!("terrain surface block: {error:?}"))?;
                                Ok(mod_api::__private::built::TerrainColumn {
                                    height: surface.height,
                                    block: state.block(),
                                    water: surface.water.map(|(height, _)| height),
                                    biome: world_biomes!(self).name(surface.biome).to_owned(),
                                }
                                .into())
                            })
                            .transpose()
                    })
                    .collect::<Result<Vec<_>, String>>()?
            }
        };
        self.common.native_read_logged(started, "terrain surface query", columns.len());
        Ok(out)
    }
}
