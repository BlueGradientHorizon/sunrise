use super::Game;
use crate::game::mods::ModsReload;
#[cfg(test)]
use crate::game::mods::ModsState;
#[cfg(test)]
use crate::game::mods::content_overlay;
#[allow(unused_imports)]
use mod_api::__private::game::*;
#[allow(unused_imports)]
use std::sync::Arc;

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn begin_condition_reload(
        &mut self,
        stack: &crate::game::function_pack_stack::PackStack,
        resources: &std::path::Path,
    ) -> Result<
        crate::game::server_side::server_reload::PreparedServerReload,
        mod_api::__private::reload::ReloadStageError,
    > {
        use mod_api::__private::reload::ReloadStageError;
        if self.mods.load_condition_candidate.is_none() {
            let inventory = sunrise_mod_package::enabled::EnabledMods::read(resources)
                .map_err(|error| ReloadStageError::DataSource(error.to_string()))?;
            self.mods.load_condition_candidate = Some(
                crate::game::load_conditions::conditions_for_mods(&self.mods, &inventory)
                    .map_err(ReloadStageError::DataSource)?,
            );
        }
        self.prepare_server_reload(stack)
    }

    pub(in crate::game) fn prepare_pack_condition_reload(
        &mut self,
        stack: &crate::game::function_pack_stack::PackStack,
    ) -> Result<
        crate::game::server_side::server_reload::PreparedServerReload,
        mod_api::__private::reload::ReloadStageError,
    > {
        self.mods.load_condition_candidate = None;
        self.begin_condition_reload(stack, &self.options.resources.clone())
    }

    pub(in crate::game) fn server_set_audio_loop_context(
        &mut self,
        audio: Option<sunrise_world::native_load_context::LoopContext>,
    ) {
        debug_assert!(audio.is_none());
    }

    pub(in crate::game) fn server_retire_disabled_instance_groups(
        &self,
        _effective: &std::collections::BTreeSet<String>,
    ) {
    }

    /// The server's side of a live switch: the data packs reload from the new root.
    pub(in crate::game) fn apply_mods_reload(&mut self, reload: ModsReload) {
        let at = std::time::Instant::now();
        self.options.resources = reload.root;
        self.pack.data_source = reload.data_source;
        self.pack.data_changes = reload.data_changes;
        self.pack.additional_data = reload.additional_data;
        self.side.settings.mods.enabled = reload.enabled.clone();
        self.mods.applied = Some(reload.enabled);
        self.mods.selected = reload.selected;
        self.mods.refused = reload.refused;
        self.mods.data_switched = true;
        if self.side.world_root.as_os_str().is_empty() {
            return;
        }
        if let Err(error) = self.reload_datapacks(true, &self.level.enabled_features.clone()) {
            log::warn!(
                "Sunrise: content mods data reload failed: {}",
                error.diagnostic()
            );
            return;
        }
        log::info!(
            "Sunrise: content mods switched live: server data reloaded in {} ms",
            at.elapsed().as_millis()
        );
    }
}

#[cfg(test)]
impl crate::game::ServerGame<'_> {
    pub(in crate::game) fn test_save_root(&self) -> std::path::PathBuf {
        self.side.save.0.root.clone()
    }

    pub(in crate::game) fn test_acquire_session_lock(&mut self, root: &std::path::Path) {
        self.side.session_lock.0 = Some(sunrise_sim::save::SessionLock::acquire(root).unwrap());
    }

    pub(in crate::game) fn test_clear_session_lock(&mut self) {
        self.side.session_lock.0 = None;
    }

    pub(in crate::game) fn test_set_world_recipes(
        &mut self,
        recipes: sunrise_sim::crafting::Recipes,
    ) {
        self.side.world_tables.0.as_mut().unwrap().recipes = recipes;
    }

    pub(in crate::game) fn test_update_tickets(
        &mut self,
        sources: Option<sunrise_sim::ticket_levels::Sources>,
    ) {
        self.side.tickets.0.update(sources);
    }

    pub(in crate::game) fn test_block_ticking(&self, cx: i32, cz: i32) -> bool {
        self.side.tickets.0.levels().block_ticking(cx, cz)
    }

    pub(in crate::game) fn test_last_other_id(&self) -> u32 {
        self.side.others.0.last().unwrap().id
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;
    use crate::game::mods::tests::Fixture;
    use crate::game::mods::tests::smod;

    pub(in crate::game) mod inventory {
        include!("mod_inventory_tests_server.rs");
    }
}
