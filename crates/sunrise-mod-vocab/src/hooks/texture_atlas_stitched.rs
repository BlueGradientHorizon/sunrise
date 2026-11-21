//! The `texture_atlas_stitched` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// An atlas was stitched and published (`TextureAtlasStitchedEvent`): `atlas` names it. Fires
    /// once per atlas, the blocks' first and then the GUI's, at the model manager's apply step.
    /// It precedes `client_setup` on startup and `client_resources_reloaded` on later reloads.
    /// Listeners ordered after the model manager apply after both stitches. A refused prepare
    /// fires nothing. Look the sprites a mod draws up here, once, and keep what it finds until the next stitch of that
    /// atlas: `texture_sprites` for [`TextureAtlas::Blocks`], `gui_sprite` for
    /// [`TextureAtlas::Gui`]. Never look them up by name every frame. Client-side.
    #[hook(
        dispatch = "flat",
        side = "client",
        contract = "listen",
        cancellable = false,
        neoforge = "TextureAtlasStitchedEvent",
        version = 2
    )]
    fn texture_atlas_stitched(atlas: mod_api::TextureAtlas);
}
