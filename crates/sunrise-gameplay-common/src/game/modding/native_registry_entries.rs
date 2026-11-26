//! `RegistryAccess.lookup` and `HolderLookup.listElements` on the current side's actual tables.
use crate::game::side_ops::modapi_modding::{FunctionRegistry, SideOps as ModdingSideOps};
use mod_api::RegistryEntry;
use std::{collections::BTreeMap, sync::Arc};
use sunrise_command::args::normalized;
use sunrise_core::identifier::Identifier;
use sunrise_protocol::registry::{Builtin, Dynamic};
use sunrise_sim::lan::data::SyncData;

enum LocalRegistry<'a> {
    Enchantments(&'a sunrise_items::enchant::Enchantments),
    Advancements(&'a sunrise_sim::advancements::Tree),
    Rules(&'a sunrise_game_data::gamerules::RuleRegistry),
    Json(&'a BTreeMap<String, serde_json::Value>),
    HashJson(&'a std::collections::HashMap<String, serde_json::Value>),
    Loot(&'a sunrise_items::items::Items),
    Recipes(&'a sunrise_sim::crafting::Recipes),
    Professions(&'a sunrise_sim::mobs::villager::ResolvedProfessions),
}
impl LocalRegistry<'_> {
    fn contains(&self, id: &str) -> bool {
        match self {
            Self::Enchantments(table) => table.get(id).is_some(),
            Self::Rules(table) => table.position(id).is_some(),
            Self::Advancements(table) => table.by_id.contains_key(id),
            Self::Json(table) => table.contains_key(id),
            Self::HashJson(table) => table.contains_key(id),
            Self::Loot(table) => table.has_loot_table(id),
            Self::Professions(table) => table.registry_contains(id),
            // RecipeSnapshot is the accepted recipe table, including custom recipe types.
            // Unlike the other owners it has no global name index.
            Self::Recipes(table) => table
                .by_type
                .values()
                .flat_map(|entries| entries.iter())
                .any(|entry| entry.id == id),
        }
    }
    fn ids(&self) -> Vec<&str> {
        let mut ids: Vec<_> = match self {
            Self::Enchantments(table) => table.list.iter().map(|entry| entry.id.as_ref()).collect(),
            Self::Advancements(table) => table.list.iter().map(|entry| entry.id.as_str()).collect(),
            Self::Rules(table) => table
                .available_rules(true)
                .map(|rule| rule.name.as_str())
                .collect(),
            Self::Json(table) => table.keys().map(String::as_str).collect(),
            Self::HashJson(table) => table.keys().map(String::as_str).collect(),
            Self::Loot(table) => table.loot_table_ids().collect(),
            Self::Professions(table) => table.registry_ids().collect(),
            Self::Recipes(table) => table
                .by_type
                .values()
                .flat_map(|entries| entries.iter())
                .map(|entry| entry.id.as_str())
                .collect(),
        };
        // Registry-wide reads promise each identity once, including aliases between recipe types.
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

impl crate::game::CommonGame<'_> {
    fn registry_generator_data(
        &self,
    ) -> Option<Arc<sunrise_world::worldgen::generator::GeneratorData>> {
        self.end
            .locator()
            .map(|terrain| Arc::clone(terrain.generator_data()))
            .or_else(|| {
                self.res
                    .behaviours()
                    .live_features()
                    .map(|features| Arc::clone(features.data()))
            })
    }
}

pub trait NativeRegistryReads<'g>: crate::game::SideGame<'g> {
    /// The same published provider definitions that `/compute` evaluates.
    fn context_registry<'__loan>(
        &'__loan self,
        registry: &str,
    ) -> Option<&std::collections::HashMap<String, serde_json::Value>> where 'g: '__loan {
        self.active_providers().registry(registry)
    }

    fn local_registry<'__loan>(&'__loan self, registry: &str) -> Option<LocalRegistry<'_>> where 'g: '__loan {
        Some(match registry {
            "minecraft:enchantment" => {
                LocalRegistry::Enchantments(self.active_enchantment_definitions())
            }
            "minecraft:advancement" => LocalRegistry::Advancements(&self.adv.tree),
            "minecraft:game_rule" => LocalRegistry::Rules(&self.level.rule_registry),
            "minecraft:predicate" => LocalRegistry::Json(ModdingSideOps::side_function_registry(
                self.side_state(),
                FunctionRegistry::Predicate,
            )?),
            "minecraft:item_modifier" => LocalRegistry::Json(
                ModdingSideOps::side_function_registry(self.side_state(), FunctionRegistry::ItemModifier)?,
            ),
            "minecraft:slot_source" => LocalRegistry::Json(ModdingSideOps::side_function_registry(
                self.side_state(),
                FunctionRegistry::SlotSource,
            )?),
            "minecraft:loot_table" => LocalRegistry::Loot(self.items()),
            "minecraft:recipe" => LocalRegistry::Recipes(self.recipes()),
            "minecraft:trade_set" => LocalRegistry::HashJson(&self.stations().trades.sets),
            "minecraft:villager_trade" => LocalRegistry::HashJson(&self.stations().trades.trades),
            "minecraft:villager_profession" => {
                LocalRegistry::Professions(&self.stations().professions)
            }
            _ => LocalRegistry::HashJson(self.context_registry(registry)?),
        })
    }

    fn registry_datapacks(&self, registry: &str) -> Option<Arc<mod_api::DatapackRegistrySet>> {
        if self.is_logical_server() {
            self.mods.datapack_registries.clone()
        } else if self.side_has_remote() {
            // This set has already been filtered by the offer, sync policy and accepted schema.
            sunrise_sim::datapack_net::received().filter(|set| set.registry_available(registry))
        } else {
            None
        }
    }

    fn registry_sync(&self) -> Option<Arc<SyncData>> {
        self.side_registry_sync()
    }

    fn registry_has_entry(&self, registry: &str, id: &str) -> bool {
        if let Some(names) = self.side_remote_registry_names() {
            if registry == "minecraft:game_rule" {
                return self.ui.rules.registry().position(id).is_some();
            }
            return network_id(names.builtin, names.dynamic, registry, id).is_some()
                || self
                    .registry_datapacks(registry)
                    .is_some_and(|set| set.contains_entry(registry, id));
        }
        if !self.is_logical_server() || self.native_lifecycle.epoch().is_none() {
            return false;
        }
        if self
            .registry_datapacks(registry)
            .is_some_and(|set| set.contains_entry(registry, id))
        {
            return true;
        }
        if let Some(table) = self.local_registry(registry) {
            return table.contains(id);
        }
        if registry == "minecraft:dimension" {
            return self.level(id).is_some();
        }
        if registry == "minecraft:trial_spawner" {
            return sunrise_sim::mobs::trial_spawner::registry_contains(id).unwrap_or(false);
        }
        if let Some(found) = self
            .res
            .behaviours()
            .live_features()
            .and_then(|features| features.registry_contains(registry, id))
        {
            return found;
        }
        if let Some(found) = self
            .registry_generator_data()
            .and_then(|data| data.registry_contains(registry, id))
        {
            return found;
        }
        self.registry_sync()
            .is_some_and(|sync| network_id(&sync.builtin, &sync.dynamic, registry, id).is_some())
    }

    /// `RegistryAccess.lookup` followed by `HolderGetter.get`: unknown registry/entry is absent.
    fn registry_entry(
        &self,
        registry: &str,
        id: &str,
    ) -> Option<RegistryEntry> {
        let registry = normalized(registry);
        let id = normalized(id);
        self.registry_has_entry(&registry, &id).then_some(())?;
        Some(RegistryEntry::new(
            Identifier::parse(&registry).ok()?,
            Identifier::parse(&id).ok()?,
        ))
    }

    /// `HolderLookup.listElements`, from the same owners used by `registry_entry`.
    fn registry_entries(&self, registry: &str) -> Option<Vec<Identifier>> {
        let registry = normalized(registry);
        if !self.is_logical_server() && !self.side_has_remote() {
            return None;
        }
        if self.is_logical_server() {
            self.native_lifecycle.epoch()?;
        }
        if let Some(set) = self.registry_datapacks(&registry)
            && let Ok(entries) = set.entries(&registry)
        {
            return identifiers(entries);
        }
        if let Some(names) = self.side_remote_registry_names() {
            if registry == "minecraft:game_rule" {
                return identifiers(LocalRegistry::Rules(self.ui.rules.registry()).ids());
            }
            return network_entries(names.builtin, names.dynamic, &registry);
        }
        if let Some(table) = self.local_registry(&registry) {
            return identifiers(table.ids());
        }
        if registry == "minecraft:dimension" {
            let mut entries = Vec::new();
            entries.extend(Identifier::parse(self.dim.id()).ok());
            ModdingSideOps::side_visit_parked_dimensions(self.side_state(), |dim| {
                entries.extend(Identifier::parse(dim.id()).ok());
            });
            return Some(entries);
        }
        if registry == "minecraft:trial_spawner" {
            return identifiers(sunrise_sim::mobs::trial_spawner::registry_ids()?);
        }
        if let Some(entries) = self.res.behaviours().live_features().and_then(|features| {
            features
                .registry_ids(&registry)
                .map(|entries| identifiers(entries.iter().map(String::as_str)))
        }) {
            return entries;
        }
        if let Some(entries) = self
            .registry_generator_data()
            .and_then(|data| data.registry_ids(&registry).map(identifiers))
        {
            return entries;
        }
        let sync = self.registry_sync()?;
        network_entries(&sync.builtin, &sync.dynamic, &registry)
    }

    /// `Holder.is(TagKey)`: plain values are resolved again against the current table.
    /// An absent tag is false; an absent registry or entry is None.
    fn registry_entry_in_tag(
        &self,
        entry: &RegistryEntry,
        tag: &str,
    ) -> Option<bool> {
        let registry = entry.registry.to_string();
        let id = entry.id.to_string();
        if registry == "minecraft:damage_type" && self.is_logical_server() {
            self.native_lifecycle.epoch()?;
            let damage = &self.health_resources.damage;
            return damage.id(&id).map(|id| damage.in_tag(id, &normalized(tag)));
        }
        if !self.registry_has_entry(&registry, &id) {
            return None;
        }
        let tag = normalized(tag);
        if let Some(set) = self.registry_datapacks(&registry)
            && let Some(found) = set.tag_contains(&registry, &id, &tag)
        {
            return Some(found);
        }
        if let Some(names) = self.side_remote_registry_names() {
            let Some(index) = network_id(names.builtin, names.dynamic, &registry, &id) else {
                return Some(false);
            };
            return Some(
                names
                    .dynamic
                    .tag(&registry, &tag)
                    .is_some_and(|members| members.contains(&index)),
            );
        }
        if matches!(
            registry.as_str(),
            "minecraft:enchantment" | "minecraft:entity_type"
        ) {
            return Some(self.sync_tag_contains(&registry, &id, &tag));
        }
        if registry == "minecraft:slot_source" {
            return Some(ModdingSideOps::side_slot_source_tag_contains(
                self.side_state(), &tag, &id,
            ));
        }
        if registry == "minecraft:villager_trade" {
            return Some(
                self.stations()
                    .trades
                    .tags
                    .get(tag.as_ref())
                    .is_some_and(|members| members.iter().any(|member| member == &id)),
            );
        }
        let item_tags = match registry.as_str() {
            "minecraft:block" => Some(sunrise_items::items::TagRegistry::Block),
            "minecraft:item" => Some(sunrise_items::items::TagRegistry::Item),
            "minecraft:fluid" => Some(sunrise_items::items::TagRegistry::Fluid),
            _ => None,
        };
        if let Some(kind) = item_tags {
            return Some(
                self.items()
                    .tag_members(kind, &tag)
                    .is_some_and(|members| members.iter().any(|member| member == &id)),
            );
        }
        if let Some(found) = self
            .res
            .behaviours()
            .live_features()
            .and_then(|features| features.registry_tag_contains(&registry, &id, &tag))
        {
            return Some(found);
        }
        if let Some(found) = self
            .registry_generator_data()
            .and_then(|data| data.registry_tag_contains(&registry, &id, &tag))
        {
            return Some(found);
        }
        if let Some(tables) = crate::game::Side::world_tables(self.side_state())
            && let Some(kind) = registry.strip_prefix("minecraft:")
            && let Some(tags) = tables.pack_tags.get(kind)
        {
            return Some(
                tags.get(tag.as_ref())
                    .is_some_and(|members| members.contains(&id)),
            );
        }
        Some(self.sync_tag_contains(&registry, &id, &tag))
    }

    fn sync_tag_contains(&self, registry: &str, id: &str, tag: &str) -> bool {
        let Some(sync) = self.registry_sync() else {
            return false;
        };
        let Some(index) = network_id(&sync.builtin, &sync.dynamic, registry, id) else {
            return false;
        };
        sync.tags
            .registries
            .iter()
            .find(|tags| tags.registry == registry)
            .and_then(|tags| tags.tags.iter().find(|candidate| candidate.name == tag))
            .is_some_and(|tag| tag.ids.contains(&index))
    }

    fn registry_entry_key_slot(&self, entry: &RegistryEntry) -> Option<u32> {
        if !self.registry_has_entry(&entry.registry.to_string(), &entry.id.to_string()) {
            return None;
        }
        mod_api::__private::registry_entry_key_slot(entry)
    }
}

fn identifiers<'a>(entries: impl IntoIterator<Item = &'a str>) -> Option<Vec<Identifier>> {
    Some(
        entries
            .into_iter()
            .filter_map(|name| Identifier::parse(name).ok())
            .collect(),
    )
}
fn network_id(builtin: &Builtin, dynamic: &Dynamic, registry: &str, id: &str) -> Option<i32> {
    builtin
        .get(registry)
        .and_then(|entries| entries.id(id))
        .or_else(|| dynamic.id(registry, id))
        .or_else(|| {
            (dynamic.custom_components && registry == "minecraft:data_component_type")
                .then(|| sunrise_protocol::item::custom::id(id))
                .flatten()
        })
}
fn network_entries(
    builtin: &Builtin,
    dynamic: &Dynamic,
    registry: &str,
) -> Option<Vec<Identifier>> {
    if let Some(ids) = builtin.get(registry) {
        let mut entries = identifiers(
            ids.names
                .iter()
                .filter(|name| !name.is_empty())
                .map(String::as_str),
        )?;
        if dynamic.custom_components && registry == "minecraft:data_component_type" {
            entries.extend(
                sunrise_protocol::item::custom::entries()
                    .filter(|(_, name)| ids.id(name).is_none())
                    .filter_map(|(_, name)| Identifier::parse(name).ok()),
            );
        }
        return Some(entries);
    }
    identifiers(
        dynamic
            .registries
            .get(registry)?
            .iter()
            .map(|(name, _)| name.as_str()),
    )
}

impl<'g, G: crate::game::SideGame<'g>> NativeRegistryReads<'g> for G {}

use crate::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
use crate::game::side_ops::modapi_hooks::ModapiHooksSideGameExt as _;
use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;
use crate::game::tables::{GameItems, WorldTablesExt};
