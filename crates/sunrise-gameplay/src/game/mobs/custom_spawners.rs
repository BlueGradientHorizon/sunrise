//! A level's custom spawners (`ServerLevel.customSpawners`) and their tick
//! (`ServerLevel.tickCustomSpawners`): vanilla's, minus what a mod's
//! `modify_custom_spawners` took out, plus the mod spawners it added.
use crate::game::ServerGame;
use mod_api::{CustomSpawners, SpawnerEntry, SpawnerKind};

impl ServerGame<'_> {
    /// The level's list, built (and `modify_custom_spawners` fired) on first use.
    fn level_spawners(&mut self) -> &CustomSpawners {
        let dim = self.common.dim;
        if !self.common.creatures.custom_spawners.contains_key(&dim) {
            let mut list = CustomSpawners::vanilla();
            if mod_api::__private::listened(mod_api::HookId::ModifyCustomSpawners) {
                self.native_modify_custom_spawners(dim.id(), &mut list);
            }
            self.common.creatures.custom_spawners.insert(dim, list);
        }
        &self.common.creatures.custom_spawners[&dim]
    }

    /// Whether vanilla's `kind` is still in this level's list.
    pub(in crate::game) fn has_custom_spawner(&mut self, kind: SpawnerKind) -> bool {
        self.level_spawners().contains(&SpawnerEntry::Builtin(kind))
    }

    /// `ServerLevel.tickCustomSpawners`: every spawner in the list, in order. A vanilla
    /// spawner keeps the place the game ticks it today (the phantom spawner runs with
    /// the stats, after the mobs: `spawn_phantoms`); a mod's spawner is the
    /// `custom_spawner_tick` hook for its key.
    pub fn tick_custom_spawners(&mut self) {
        self.level_spawners();
        let dim = self.common.dim;
        let spawn_enemies = self
            .common
            .ui
            .rules
            .bool_at(sunrise_game_data::gamerules::vanilla::SPAWN_MONSTERS)
            && self.common.ui.difficulty != 0;
        let mut at = 0;
        while let Some(entry) = self.common.creatures.custom_spawners[&dim]
            .entries()
            .get(at)
            .cloned()
        {
            at += 1;
            match entry {
                SpawnerEntry::Builtin(SpawnerKind::WanderingTrader) => self.tick_wandering_trader(),
                SpawnerEntry::Builtin(SpawnerKind::Patrol) => self.tick_patrol_spawner(),
                SpawnerEntry::Builtin(SpawnerKind::Cat) => self.tick_cat_spawner(),
                SpawnerEntry::Builtin(SpawnerKind::VillageSiege) => self.tick_village_siege(),
                SpawnerEntry::Mod(key) => {
                    self.native_custom_spawner_tick(dim.id(), key, spawn_enemies)
                }
                // `Phantom` ticks in `spawn_phantoms`.
                _ => {}
            }
        }
    }
}
