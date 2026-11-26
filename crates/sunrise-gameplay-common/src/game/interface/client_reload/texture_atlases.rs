#[cfg(any(test, feature = "test-support"))]
pub mod tests {

    use super::*;

    #[cfg(test)]
    /// A mod registering an atlas on the first load only: its id and vanilla's
    /// `map_decorations` definition, already in the resources.
    pub struct Maps(std::sync::Arc<std::sync::Mutex<(u32, Vec<Option<mod_api::AtlasRegion>>)>>);
    pub fn maps() -> TextureAtlasRegistration {
        registration("maps_mod:maps", "minecraft:map_decorations")
    }
    /// An atlas whose name holds `presets`, which the menu's own picture folder also has.
    pub fn presets() -> TextureAtlasRegistration {
        registration("maps_mod:presets", "minecraft:map_decorations")
    }
    pub fn player(atlas: &TextureAtlasRegistration) -> Identifier {
        atlas.sprite_name(&Identifier::parse("minecraft:player").unwrap())
    }
    #[cfg(test)]
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Maps {
        fn register_texture_atlases(
            &self,
            _: &mod_api::RegistrationContext,
            event: &mut mod_api::RegisterTextureAtlasesEvent<'_>,
        ) {
            let mut kept = self.0.lock().unwrap();
            kept.0 += 1;
            if kept.0 == 1 {
                event.register(maps());
                event.register(presets());
            }
        }

        fn texture_atlas_stitched(
            &self,
            host: &mut dyn mod_api::ClientHost,
            event: &mut mod_api::TextureAtlasStitchedEvent,
        ) {
            if event.atlas == mod_api::TextureAtlas::Gui {
                self.0
                    .lock()
                    .unwrap()
                    .1
                    .push(host.gui_sprite(&player(&maps())));
            }
        }
    }
}
