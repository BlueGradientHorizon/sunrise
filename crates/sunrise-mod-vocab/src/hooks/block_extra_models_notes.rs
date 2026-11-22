//! The prose of `ClientHost::set_block_extra_models` (#2763), kept out of `notes.rs`.
pub trait Notes {
    #[notes = r#"## Block extra models

`ClientHost::set_block_extra_models(pos, models)` replaces the standalone models (`RenderAssets::model`) drawn as part of the block at `pos`; an empty list clears it. The lists are client-only and need no block entity. A resend of a resident column preserves attachments at unchanged cells; unloading the column clears them. A block state or level change also clears the affected lists.

Native client mods can set the list in the World phase. `NotLoaded` means the position is outside a resident column or the level height; `Model` means the handle is stale or is not a registered standalone model; `Unavailable` means the call is outside the World phase. A refused call changes nothing. Resolve new handles in `client_resources_reloaded`, then set the lists again from a World-phase callback (`client_tick_pre`).

After the block geometry, extra faces are drawn in block and list order, with the block's light, neighbour culling and tints but their own render layers. Mod tint providers are asked before the mesh is scheduled; a provider that panics supplies white. Drawing stops before the next face whose worst-case expansion cannot fit the section's upload budget, so the last model may be partially drawn."#]
    fn block_extra_models();
}
