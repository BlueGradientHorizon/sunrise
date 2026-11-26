//! The server's tags on this client (`ClientPacketListener.handleUpdateTags`, and configuration's
//! `RegistryDataCollector`): install gameplay tags, then notify native mods.
use crate::game::ClientGame;
use std::sync::Arc;
use sunrise_items::items::{Items, TagRegistry};
use sunrise_protocol::{item::Names, packets::configuration::UpdateTags, registry::SyncedNames};

/// The registries whose tags the tag helpers read.
pub(in crate::game) const READ_REGISTRIES: [(&str, TagRegistry); 3] = [
    ("minecraft:block", TagRegistry::Block),
    ("minecraft:item", TagRegistry::Item),
    ("minecraft:fluid", TagRegistry::Fluid),
];
/// The synced registry whose tags `Tags::damage_type_in_tag` reads.
pub(in crate::game) const DAMAGE_TYPE: &str = "minecraft:damage_type";

impl ClientGame<'_> {
    /// `ClientboundUpdateTagsPacket` in play.
    pub fn remote_update_tags(&mut self, tags: &UpdateTags) {
        if let Some(r) = self.client.remote.as_mut() {
            Arc::make_mut(&mut r.registries).add_tags(tags);
        }
        self.install_server_items();
        self.client_refresh_infiniburn();
        self.refresh_creative_search_tags();
        self.install_remote_tags();
        if self.common.recipe_tables.server_stations.is_some()
            && tags
                .registries
                .iter()
                .any(|tags| tags.registry == "minecraft:item")
        {
            self.refresh_remote_stonecutter();
        }
    }

    pub(in crate::game::client_side) fn refresh_creative_search_tags(&mut self) {
        let tags = self.client.remote.as_ref().and_then(|r| {
            let ids = r.builtin.get("minecraft:item")?;
            let tags = r.registries.tags.get("minecraft:item")?;
            Some(
                tags.iter()
                    .map(|(tag, members)| {
                        let items = members
                            .iter()
                            .filter_map(|&id| ids.name(id).map(str::to_owned))
                            .collect();
                        (tag.clone(), items)
                    })
                    .collect(),
            )
        });
        self.client.creative_screen.0.set_item_tags(tags.as_ref());
    }

    /// Make the server's tags what native mods read, then tell them. Snapshot client
    /// enchant answers only after the tags, rebound parts and tag listeners are published.
    pub fn install_remote_tags(&mut self) {
        if mod_api::__private::mods().is_empty() {
            self.resolve_enchant_particles();
            return;
        }
        let Some(r) = self.client.remote.as_mut() else {
            return;
        };
        let damage_types = r.registries.registries.get(DAMAGE_TYPE);
        if r.registries.tags.is_empty() && damage_types.is_none() {
            r.tags = None;
            self.refresh_recipe_property_context();
            return;
        }
        let names = SyncedNames {
            builtin: &r.builtin,
            dynamic: &r.registries,
        };
        let mut items = Items::empty();
        for (key, registry) in READ_REGISTRIES {
            let Some(tags) = r.registries.tags.get(key) else {
                continue;
            };
            let members = |ids: &[i32]| ids.iter().filter_map(|&id| names.name(key, id)).collect();
            let tags = tags.iter().map(|(tag, ids)| (tag.clone(), members(ids)));
            items.set_tags(registry, tags.collect());
        }
        // The damage type registry and its tags, for `Tags::damage_type_in_tag`. A registry
        // sent with no tag group binds its types with no tags, as vanilla binds received
        // elements whose tag payload was omitted.
        if let Some(types) = damage_types {
            let members = |ids: &[i32]| {
                ids.iter()
                    .filter_map(|&id| names.name(DAMAGE_TYPE, id))
                    .collect()
            };
            items.bind_damage_registry(
                types.iter().map(|(name, _)| name.clone()).collect(),
                r.registries
                    .tags
                    .get(DAMAGE_TYPE)
                    .into_iter()
                    .flatten()
                    .map(|(tag, ids)| (tag.clone(), members(ids)))
                    .collect(),
            );
        }
        super::class_tags::install(&mut items, self.common.res.facts());
        // Rebuild scoped parts against the received tags before refreshing state-only facts.
        let mut part_items = self.items().clone();
        let names = items.block_state_names();
        part_items.block_tags = items.block_tags.clone();
        part_items.rebuild_block_tag_flags(names);
        part_items.item_tags = items.item_tags.clone();
        let parts = match self.prepare_parts(&part_items) {
            Ok(parts) => parts,
            Err(error) => {
                log::warn!("Sunrise: client tag parts refused: {error}");
                return;
            }
        };
        if let Some(r) = self.client.remote.as_mut() {
            r.tags = Some(items);
        }
        self.bind_parts(parts);
        self.refresh_recipe_property_context();
        self.announce_data_bound(mod_api::TagsUpdateCause::ClientPacketReceived {
            integrated_server: self.client.integrated.is_some(),
        });
        self.resolve_primary_items();
        self.resolve_enchant_particles();
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use crate::game::native_entity_refs::tests::with_client_game;
    use mod_api::{Hooks, Identifier, ModEntry, TagMembers, TagsUpdateCause, TagsUpdatedEvent};
    use mod_platform::__private::part_tests::block_key;
    use std::sync::{Arc, Mutex};
    use sunrise_protocol::packets::{
        configuration::{RegistryTags, TagEntry, UpdateTags},
        play::{self, Clientbound as C},
    };
    use sunrise_protocol::registry::Builtin;

    

    
#[cfg(test)]
/// What a listener read of the tags during its notice.
    #[derive(Debug, PartialEq)]
    pub(in crate::game) struct Seen {
        pub(in crate::game) cause: TagsUpdateCause,
        pub(in crate::game) items: TagMembers<mod_api::ItemKey>,
        pub(in crate::game) empty_items: TagMembers<mod_api::ItemKey>,
        pub(in crate::game) fluids: TagMembers<mod_api::FluidKey>,
        pub(in crate::game) stone_in_block_tag: Option<bool>,
        pub(in crate::game) dirt_in_block_tag: Option<bool>,
        pub(in crate::game) stick_in_item_tag: Option<bool>,
        pub(in crate::game) lava_in_fluid_tag: Option<bool>,
    }
#[cfg(test)]
pub(in crate::game) struct Probe(pub(in crate::game) Arc<Mutex<Vec<Seen>>>);

    #[mod_api::__subscriptions]
    impl Hooks for Probe {
        fn tags_updated(&self, context: &dyn mod_api::ReloadContext, event: &mut TagsUpdatedEvent) {
            assert!(
                matches!(event.cause, TagsUpdateCause::ClientPacketReceived { .. }),
                "the server's tags are a client's to apply"
            );
            let tags = context.tags();
            let key = |tag| tags.key(&Identifier::parse(tag).unwrap()).unwrap();
            let stick = context.item_key(&Identifier::parse("minecraft:stick").unwrap());
            self.0.lock().unwrap().push(Seen {
                cause: event.cause,
                items: tags.items(key("test:items")),
                empty_items: tags.items(key("test:empty_items")),
                fluids: tags.fluids(key("test:fluids")),
                stone_in_block_tag: context
                    .block_in_tag(block_key("minecraft:stone"), "test:blocks"),
                dirt_in_block_tag: context.block_in_tag(block_key("minecraft:dirt"), "test:blocks"),
                stick_in_item_tag: tags.is_in_tag(stick.unwrap(), key("test:items")),
                lava_in_fluid_tag: context.fluid_in_tag("minecraft:lava", "test:fluids"),
            });
        }
    }

    

    

    

    

    

    

    

    
}

use sunrise_gameplay_common::game::tables::GameItems as _;

use sunrise_gameplay_common::game::tables::WorldTablesExt as _;

use sunrise_gameplay_common::game::native_data_bound::NativeDataBoundGameExt1 as _;

use sunrise_gameplay_common::game::native_world_read_common::NativeWorldReadGameExt1 as _;
