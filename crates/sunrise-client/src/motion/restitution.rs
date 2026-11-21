//! The player's read-only block contact Parts.
use super::{FrictionLevel, MotionWorld, Result};
use crate::block_contact;
use sunrise_core::position::BlockPos;
use sunrise_world::registry::BlockStateId;

impl MotionWorld<'_, '_, '_> {
    /// The state and position are the pair selected in Entity.move before its fall check.
    pub(super) fn part_bounce_restitution(
        &self,
        state: BlockStateId,
        position: BlockPos,
    ) -> Result<f32> {
        let own = self
            .resources
            .blocks
            .get(state)?
            .coefficients
            .bounce_restitution;
        Ok(block_contact::bounce_restitution(
            FrictionLevel {
                world: self.scene,
                registry: self.assets.registry,
                context: self.loop_context,
                side: self.side,
                coefficient: &|id| {
                    self.resources
                        .blocks
                        .get(id)
                        .ok()
                        .map(|p| p.coefficients.friction)
                },
            },
            block_contact::Contact {
                entity: self.player.entity_id,
                kind: "minecraft:player",
                state,
                position,
            },
            own,
        ))
    }

    pub(super) fn part_step_on(&mut self, state: BlockStateId, position: BlockPos) {
        let blocks = &self.resources.blocks;
        block_contact::step_on(
            FrictionLevel {
                world: self.scene,
                registry: self.assets.registry,
                context: self.loop_context,
                side: self.side,
                coefficient: &|id| Some(blocks.get(id).ok()?.coefficients.friction),
            },
            block_contact::Contact {
                entity: self.player.entity_id,
                kind: "minecraft:player",
                state,
                position,
            },
            self.player.shift_key_down,
            &mut self.player.velocity,
        );
    }
}
