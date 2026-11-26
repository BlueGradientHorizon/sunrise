//! Common loaded-column heightmap queries shared by server block edits and client views.
use crate::game::CommonGame;
use mod_api::Heightmap;
use sunrise_level::world::World;
use sunrise_world::{
    heightmap::HeightmapType, registry::BlockStateId, worldgen::terrain::TerrainBlockStates,
};

/// The most columns one `surface_heights` reads.
pub const MAX_COLUMNS: usize = 1 << 16;

/// A change `World::edit_runs` reported: position, previous and new state.
pub type Tracked = ((i32, i32, i32), u32, u32);

impl crate::game::CommonGame<'_> {
    /// Each column's height on `h` as `edit_height` reads it, none where its chunk is not loaded.
    pub fn loaded_column_heights(
        &self,
        columns: &[(i32, i32)],
        h: Heightmap,
    ) -> Vec<Option<i32>> {
        loaded_heights(&self.world, columns, |x, z| self.edit_height(h, x, z))
    }

    /// One above the highest block of column (x, z) that counts on heightmap `h`.
    pub fn edit_height(&self, h: Heightmap, x: i32, z: i32) -> i32 {
        let properties = &self.pack.assets.properties;
        column_height(
            &self.world,
            h,
            (x, z),
            |s| self.res.facts().is_air(s),
            |t, s| t.is_opaque(properties.heightmap_state(s)),
        )
    }
}

/// One above the highest block of column `(x, z)` on heightmap `h`.
pub fn column_height(
    world: &World,
    h: Heightmap,
    (x, z): (i32, i32),
    air: impl Fn(BlockStateId) -> bool,
    counts: impl Fn(HeightmapType, BlockStateId) -> bool,
) -> i32 {
    if h == Heightmap::WorldSurface {
        return world.world_surface(x, z, air);
    }
    let top = world.motion_blocking(x, z, |s| counts(HeightmapType::MotionBlocking, s));
    let kind = match h {
        Heightmap::MotionBlockingNoLeaves => HeightmapType::MotionBlockingNoLeaves,
        Heightmap::OceanFloor => HeightmapType::OceanFloor,
        _ => return top,
    };
    let bottom = world
        .chunks
        .get(&(x >> 4, z >> 4))
        .map_or(world.layout().min_y(), |c| c.min_y());
    let mut y = top;
    while y > bottom && !counts(kind, world.block(x, y - 1, z)) {
        y -= 1;
    }
    y
}

/// `height(x, z)` of each column, none where its chunk is not loaded.
pub fn loaded_heights(
    world: &World,
    columns: &[(i32, i32)],
    height: impl Fn(i32, i32) -> i32,
) -> Vec<Option<i32>> {
    let mut last: Option<((i32, i32), bool)> = None;
    columns
        .iter()
        .map(|&(x, z)| {
            let key = (x >> 4, z >> 4);
            let loaded = match last {
                Some((k, l)) if k == key => l,
                _ => {
                    let l = world.chunks.get(&key).is_some();
                    last = Some((key, l));
                    l
                }
            };
            loaded.then(|| height(x, z))
        })
        .collect()
}
