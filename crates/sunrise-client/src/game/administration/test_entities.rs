//! What the game tests do to the entities and chunks of a test's area:
//! `StructureUtils.clearSpaceForStructure`'s and `GameTestInfo.succeed`'s
//! entity removal, and the force-loaded chunks of
//! `TestInstanceBlockEntity.forceLoadChunks` and `GameTestRunner`'s release.
#[allow(unused_imports)]
use super::Game;
#[allow(unused_imports)]
use sunrise_core::bounding_box::BoundingBox;

#[allow(unused_imports)]
pub(in crate::game) use sunrise_gameplay::test_entities::{Aabb, intersects, standing};
#[allow(unused_imports)]
pub use sunrise_gameplay::test_entities::{aabb_of, inflate};

impl Game<'_> {
    /// `ServerLevel.setChunkForced(x, z, true)` for the chunks of `area`
    /// (`TestInstanceBlockEntity.forceLoadChunks`).
    pub(in crate::game) fn force_load_test_chunks(&mut self, area: &BoundingBox) {
        let (x0, x1) = (area.min[0] >> 4, area.max[0] >> 4);
        let (z0, z1) = (area.min[2] >> 4, area.max[2] >> 4);
        self.streaming
            .set_forced_area((x0, z0), (x1, z1), true, |_, _, _| {});
    }
}
