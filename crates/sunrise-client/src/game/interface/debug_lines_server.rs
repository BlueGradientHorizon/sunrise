#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::debug_lines::LOOKING_AT_RANGE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::debug_lines::SERVER_ENTRIES;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::debug_lines::ServerDebug;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::debug_lines::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::collections::HashSet;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    debug_entries::{self, ENTRIES},
    debug_overlay as overlay,
    debug_overlay::Displayer,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::java_number;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::{
    heightmap::{HeightmapState, HeightmapType},
    worldgen::terrain::TerrainBlockStates,
};

impl Game<'_> {
    /// The server thread's half of the debug entries, in the level of
    /// `dim` while it is in place: the server chunk's heightmaps, the local
    /// difficulty, the last spawn state and the chunk source's counts.
    pub(in crate::game::server_side) fn server_debug_lines(&self, feet: [i32; 3]) -> ServerDebug {
        let [x, y, z] = feet;
        let key = (x >> 4, z >> 4);
        let loaded = self.world.chunks.contains_key(&key);
        let inside = y >= self.world.bottom_y() && y < self.world.top_y();
        let heights = loaded.then(|| {
            [
                HeightmapType::WorldSurface,
                HeightmapType::OceanFloor,
                HeightmapType::MotionBlocking,
                HeightmapType::MotionBlockingNoLeaves,
            ]
            .map(|kind| self.column_height(kind, x, z))
        });
        let difficulty = (loaded && inside).then(|| {
            let at = [f64::from(x), f64::from(y), f64::from(z)];
            overlay::local_difficulty_line(self.local_difficulty(at).effective)
        });
        let spawn_counts = self
            .server
            .spawn_counts
            .get(&self.dim)
            .map(|(chunks, counts)| overlay::spawn_counts_line(*chunks, counts));
        // ServerLevel.gatherChunkSourceStats: the chunk map's size, then
        // PersistentEntitySectionManager's known and visible entities, its
        // sections, the chunks it loaded and sees, and its two queues
        // (Sunrise loads a chunk's entities with it: both stay empty).
        let (entities, loaded) = (self.debug_entity_count(), self.world.chunks.len());
        let chunks = Some(format!(
            "Chunks[S] W: {loaded} E: {entities},{entities},{},{loaded},{loaded},0,0",
            self.debug_entity_sections()
        ));
        ServerDebug {
            heights,
            difficulty,
            spawn_counts,
            chunks,
        }
    }
}
