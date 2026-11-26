use crate::game::api_values::block_state;
use crate::game::client_side::native_scene::SceneBlock;

/// Prepare borrowed scene blocks, with a refusal naming the invalid block.
/// Conversion allocates one engine vector per scene draw, but never clones entity
/// identifiers or saved fields, and never resolves names.
#[cfg(feature = "client")]
pub(in crate::game) fn scene_blocks(
    blocks: &[mod_api::SceneBlock],
) -> Result<Vec<SceneBlock<'_>>, mod_api::SceneError> {
    if blocks.len() > mod_api::SCENE_MAX_BLOCKS {
        return Err(mod_api::SceneError::TooManyBlocks);
    }
    let index = block_state::current().map_err(|_| mod_api::SceneError::Unavailable)?;
    (0u32..)
        .zip(blocks)
        .map(|(at, block)| {
            Ok(SceneBlock {
                pos: block.pos,
                state: index
                    .to_engine(block.state)
                    .map_err(|_| mod_api::SceneError::BadBlock { index: at })?,
                entity: block.entity.as_ref(),
            })
        })
        .collect()
}
