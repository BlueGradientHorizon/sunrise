#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::remove_block::*;

impl GameLevel<'_> {
    pub(in crate::game) fn removal_replacement(
        &self,
        pos: BlockPos,
        state: BlockStateId,
        moved_by_piston: bool,
        vanilla: BlockStateId,
    ) -> BlockStateId {
        if !self.native_authoritative {
            return vanilla;
        }
        let mut answer = None;
        self.native_remove_block(super::fire::RemoveBlockCall {
            dimension: self.dimension,
            pos,
            state,
            moved_by_piston,
            replacement: vanilla,
            answer: &mut answer,
        });
        match answer {
            Some(replacement) if self.registry.state(replacement).is_some() => replacement,
            Some(replacement) => {
                log::warn!(
                    "remove_block refused invalid replacement state {}; using vanilla",
                    replacement.0
                );
                vanilla
            }
            None => vanilla,
        }
    }
}
