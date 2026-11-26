use super::{Game, owner_link::OwnerRequest};
use crate::game::mods::ModsReload;
use crate::game::mods::ModsState;
#[allow(unused_imports)]
use mod_api::__private::game::*;
use sunrise_game_data::lang;

#[path = "package_files_client.rs"]
pub(in crate::game) mod package_files;

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    /// The content mods the resources load with: the ones applied, which a
    /// switch that needs a restart doesn't change.
    pub(in crate::game) fn applied_mods(&self) -> std::collections::BTreeSet<String> {
        self.mods
            .applied
            .clone()
            .unwrap_or_else(|| self.side.settings.mods.enabled.clone())
    }

    pub(in crate::game) fn client_set_audio_loop_context(
        &mut self,
        audio: Option<sunrise_world::native_load_context::LoopContext>,
    ) {
        if let Some(audio) = audio {
            self.side.audio.0.set_loop_context(audio);
        }
    }

    pub(in crate::game) fn client_retire_disabled_instance_groups(
        &self,
        effective: &std::collections::BTreeSet<String>,
    ) {
        self.side
            .instance_groups
            .borrow_mut()
            .retire_disabled(effective);
    }

    /// A live switch's resources applied: the server reloads its data packs
    /// from the new root, as `/reload` does (recipes, tags, loot tables,
    /// advancements, functions), and sends them to its players.
    pub(in crate::game) fn mods_reloaded(&mut self, applied: bool) {
        let Some(started) = self.mods.live.take() else {
            return;
        };
        let resources_ms = started.elapsed().as_millis();
        if !applied {
            return;
        }
        let reload = ModsReload {
            root: self.options.resources.clone(),
            data_source: self.pack.data_source.clone(),
            data_changes: self.pack.data_changes.clone(),
            additional_data: self.pack.additional_data.clone(),
            enabled: self.applied_mods(),
            selected: self.mods.selected.clone(),
            refused: self.mods.refused.clone(),
        };
        log::info!("Sunrise: content mods switched live: resources reloaded in {resources_ms} ms");
        if let Some(server) = &self.integrated {
            server.tell(OwnerRequest::ReloadMods(Box::new(reload)));
        }
    }
    /// The player turned content mods on or off (Options > Mods, or an
    /// install from Browse Mods): those with only assets and data apply now,
    /// through the resource reload and then the
    /// server's data reload (`mods_reloaded`); those that add blocks, items or
    /// worldgen wait for a restart, and the Mods page says so.
    pub(in crate::game::client_side) fn switch_mods(&mut self) {
        let Some(applied) = self.mods.applied.clone() else {
            return;
        };
        let wanted = self.side.settings.mods.enabled.clone();
        // Ordinary native mods' `mod.<id>` lines switch live (`sync_native_switches`);
        // a serving host bound to active or withheld `served_by` packages, and any content
        // mod with blocks, items, worldgen, or `served_by`, keep their startup state until
        // a restart.
        let content = &self.mods.content_ids;
        let mut next = wanted.clone();

        let mut waiting = Vec::new();
        for id in applied.symmetric_difference(&wanted) {
            if !content.contains(id) || self.mods.live_ids.contains(id) {
                continue;
            }
            log::info!("Sunrise: mod {id} needs a restart to switch");
            waiting.push(id.clone());
            if applied.contains(id) {
                next.insert(id.clone());
            } else {
                next.remove(id);
            }
        }
        let (_, host_waiting) = self
            .mods
            .reconcile_code_switches(&self.side.settings.mods.disabled);
        for id in host_waiting {
            log::info!("Sunrise: serving host mod {id} needs a restart to switch");
            if !waiting.contains(&id) {
                waiting.push(id);
            }
        }
        self.side.chat_ui.options.mods_note = (!waiting.is_empty())
            .then(|| lang::tr("sunrise.mods.switch.restart", &[&waiting.join(", ")]));
        let changed = applied
            .symmetric_difference(&next)
            .any(|id| content.contains(id));
        if !changed {
            self.mods.applied = Some(next);
            return;
        }
        log::info!(
            "Sunrise: content mods switched live: {:?} -> {:?}",
            applied,
            next
        );
        self.mods.applied = Some(next);
        self.mods.live = Some(std::time::Instant::now());
        self.reload_packs();
    }

    #[cfg(test)]
    pub(in crate::game) fn test_add_hook_panic_toasts(&mut self) {
        self.add_hook_panic_toasts();
    }

    #[cfg(test)]
    pub(in crate::game) fn test_open_title(&mut self, saves: std::path::PathBuf) {
        self.side.title.0 = Some(self.new_title(saves));
    }
}

impl ModsState {
    pub(in crate::game) fn startup_admission(&self) -> Option<&package_files::StartupAdmission> {
        self.startup_admission
            .as_deref()
            .filter(|admission| admission.matches_selection(&self.selected))
    }
}
