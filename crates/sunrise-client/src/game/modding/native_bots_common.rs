//! Dual-role block collision box scans shared by server and client levels.
use super::Game;
use std::io;
use std::ops::ControlFlow;
use sunrise_client::collisions::SceneCollisions;
use sunrise_core::geometry::{Aabb, Vec3, VoxelShape};
use sunrise_core::position::{BlockPos, ChunkPos};
use sunrise_world::block_collision_scan::{
    BlockCollisionScan, BlockCollisionWorld, CollisionBlock,
};
use sunrise_world::collision_context::CollisionContext;

const COLLISION_CELLS: usize = 1_000_000;

pub(in crate::game) fn aabb_of(b: [f64; 6]) -> Option<Aabb> {
    (b.iter().all(|v| v.is_finite()) && b[0] <= b[3] && b[1] <= b[4] && b[2] <= b[5])
        .then(|| Aabb::new(Vec3::new(b[0], b[1], b[2]), Vec3::new(b[3], b[4], b[5])))
}

pub(in crate::game) fn corners(b: Aabb) -> [f64; 6] {
    let (lo, hi) = (b.min_position(), b.max_position());
    [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z]
}

impl Game<'_> {
    pub(in crate::game) fn level_collision_boxes(
        &self,
        level: mod_api::LevelRef,
        aabb: [f64; 6],
    ) -> Option<Vec<[f64; 6]>> {
        let mut boxes = Vec::new();
        self.scan_block_collisions(level, aabb, |found| {
            boxes.extend(found.iter().copied().map(corners));
            ControlFlow::Continue(())
        })?;
        Some(boxes)
    }

    pub(in crate::game) fn level_has_collisions(
        &self,
        level: mod_api::LevelRef,
        aabb: [f64; 6],
    ) -> Option<bool> {
        let mut any = false;
        self.scan_block_collisions(level, aabb, |found| {
            any = !found.is_empty();
            if any {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })?;
        Some(any)
    }

    fn scan_block_collisions(
        &self,
        level: mod_api::LevelRef,
        aabb: [f64; 6],
        mut visit: impl FnMut(&[Aabb]) -> ControlFlow<()>,
    ) -> Option<()> {
        let bounds = aabb_of(aabb)?;
        let (world, refuse_missing) = self.side_collision_scan_world(level)?;
        let mut scene = ScanColumns {
            scene: SceneCollisions {
                world,
                binding: self.collision,
                subject: None,
            },
            refuse_missing,
        };
        let Ok(scan) = BlockCollisionScan::new(
            &mut scene,
            CollisionContext::empty(false),
            bounds,
            false,
            COLLISION_CELLS,
        ) else {
            return None;
        };
        for hit in scan {
            let Ok(hit) = hit else {
                return None;
            };
            if visit(hit.shape.boxes()).is_break() {
                break;
            }
        }
        Some(())
    }
}

struct ScanColumns<'w, 'r> {
    scene: SceneCollisions<'w, 'r>,
    refuse_missing: bool,
}

impl<'w> BlockCollisionWorld for ScanColumns<'w, '_> {
    type Error = io::Error;
    type Chunk = &'w sunrise_client::chunk::Chunk;
    fn collision_chunk(&mut self, p: ChunkPos) -> io::Result<Option<Self::Chunk>> {
        match self.scene.collision_chunk(p)? {
            None if self.refuse_missing => Err(io::Error::other(
                "collision scan reached a column the client does not hold",
            )),
            chunk => Ok(chunk),
        }
    }
    fn collision_block(&mut self, chunk: Self::Chunk, p: BlockPos) -> io::Result<CollisionBlock> {
        self.scene.collision_block(chunk, p)
    }
    fn is_suffocating(
        &mut self,
        chunk: Self::Chunk,
        block: CollisionBlock,
        p: BlockPos,
    ) -> io::Result<bool> {
        self.scene.is_suffocating(chunk, block, p)
    }
    fn context_shape(
        &mut self,
        block: CollisionBlock,
        p: BlockPos,
        context: &CollisionContext<'_>,
    ) -> io::Result<VoxelShape> {
        self.scene.context_shape(block, p, context)
    }
    fn may_reach_out(&mut self, chunk: Self::Chunk, section_y: i32) -> io::Result<bool> {
        self.scene.may_reach_out(chunk, section_y)
    }
    fn section_is_empty(&mut self, chunk: Self::Chunk, section_y: i32) -> io::Result<bool> {
        self.scene.section_is_empty(chunk, section_y)
    }
    fn missing_chunks_stay_missing(&self) -> bool {
        self.scene.missing_chunks_stay_missing()
    }
}
