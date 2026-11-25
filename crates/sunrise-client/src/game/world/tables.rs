//! Read-only world content tables shared by server and client games.
use super::{Biomes, Game, Items, Providers, Recipes, Resources, WorldTables};
use std::sync::Arc;

/// [`Game::level_suffocation`] from the fields it reads, for a caller that holds other fields: the
/// world's own tables (its datapacks' `causes_suffocation`), else the built-in ones.
pub(in crate::game) fn level_suffocation_of<'a>(
    world_tables: &'a Option<WorldTables>,
    res: &'a Resources,
) -> &'a sunrise_world::suffocation::SuffocationRegistry<'static> {
    world_tables
        .as_ref()
        .map_or_else(|| res.suffocation(), |t| &t.suffocation)
}

impl Game<'_> {
    /// The server's recipes a client was sent, over its own; None on a
    /// server and until a server sends any.
    pub(in crate::game) fn synced_recipes(&self) -> Option<&sunrise_client::crafting::Recipes> {
        self.recipe_tables.synced.as_ref()
    }

    /// Server reads active station enchantments; client reads synced remote enchantments or active items.
    pub(in crate::game) fn item_enchantments(&self) -> &Arc<sunrise_client::enchant::Enchantments> {
        if self.is_logical_server() {
            &self.stations().enchantments
        } else if let Some(ench) = self.remote.as_ref().and_then(|r| r.enchantments.as_ref()) {
            ench
        } else {
            &self.items().enchantments
        }
    }

    /// The content tables the world plays with: its data packs' over the
    /// game's own when it has packs, `Resources.rest`'s own otherwise.
    pub(in crate::game) fn items(&self) -> &Items {
        world_items!(self)
    }

    /// The level's suffocation rules, for its mobs and its players.
    pub(in crate::game) fn level_suffocation(
        &self,
    ) -> &sunrise_world::suffocation::SuffocationRegistry<'static> {
        level_suffocation_of(&self.world_tables.0, &self.res)
    }

    pub(in crate::game) fn recipes(&self) -> &Recipes {
        if self.is_logical_server() {
            match &self.world_tables.0 {
                Some(t) => &t.recipes,
                None => self.res.recipes(),
            }
        } else {
            self.synced_recipes().unwrap_or_else(|| self.res.recipes())
        }
    }

    pub(in crate::game) fn active_providers(&self) -> &Providers {
        self.world_tables
            .0
            .as_ref()
            .map_or_else(|| self.res.providers(), |t| &t.providers)
    }

    pub(in crate::game) fn stations(&self) -> &sunrise_client::stations::StationData {
        world_stations!(self)
    }

    /// The biome colours used by the mesher and client tint queries. On a
    /// world without its own table, a colour refresh updates the pack's table.
    pub(in crate::game) fn mesh_biomes(&self) -> &Arc<Biomes> {
        self.world_biomes
            .as_ref()
            .unwrap_or(&self.pack.context.biomes)
    }
}
