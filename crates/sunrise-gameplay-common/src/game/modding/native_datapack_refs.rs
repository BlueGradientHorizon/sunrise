//! Datapack registry reads through a level handle.
use mod_api::LevelRef;

pub trait NativeDatapackRefsGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// The datapack registry set of this level's world session: on the logical server, the
    /// server's own set for both the active dimension and any parked loaded dimension
    /// (matching vanilla `Level.registryAccess()` on `ServerLevel`); on a client, the set its
    /// server sent at the join for the active client level.
    fn level_datapack_set(
        &self,
        level: LevelRef,
    ) -> Option<std::sync::Arc<mod_api::DatapackRegistrySet>> {
        self.side_level_datapack_set(level)
    }

    /// The entry ids of a mod's datapack registry as the world last loaded it.
     fn level_datapack_entries(
        &self,
        level: LevelRef,
        registry: &str,
    ) -> Option<Vec<String>> {
        let entries = self
            .level_datapack_set(level)?
            .entries(registry)
            .ok()?
            .into_iter()
            .map(str::to_owned)
            .collect();
        Some(entries)
    }

    /// Visit the published entry `entry` of `registry`.
     fn level_datapack_read(
        &self,
        level: LevelRef,
        registry: &str,
        entry: &str,
        visit: &mut dyn FnMut(&mod_api::DatapackEntry),
    ) -> Result<(), mod_api::DatapackReadError> {
        let set = self
            .level_datapack_set(level)
            .ok_or(mod_api::DatapackReadError::UnknownRegistry)?;
        if !set.registry_available(registry) {
            return Err(mod_api::DatapackReadError::UnknownRegistry);
        }
        visit(set.get(registry, entry)?);
        Ok(())
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeDatapackRefsGameExt1<'__sg> for G {}


use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;
