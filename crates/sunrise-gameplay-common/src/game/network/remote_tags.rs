

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use mod_api::{Hooks, Identifier, ModEntry, TagMembers, TagsUpdateCause, TagsUpdatedEvent};

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
pub fn update(
        builtin: &Builtin,
        tags: &[(&str, &str, &[&str])],
    ) -> UpdateTags {
        let mut registries = std::collections::BTreeMap::<_, Vec<_>>::new();
        for &(registry, tag, members) in tags {
            let names = builtin.get(registry).unwrap();
            registries.entry(registry).or_default().push(TagEntry {
                name: tag.into(),
                ids: members.iter().map(|name| names.id(name).unwrap()).collect(),
            });
        }
        UpdateTags {
            registries: registries
                .into_iter()
                .map(|(registry, tags)| RegistryTags {
                    registry: registry.into(),
                    tags,
                })
                .collect(),
        }
    }
pub fn login() -> C {
        C::Login(play::Login {
            player_id: 7,
            hardcore: false,
            levels: vec!["minecraft:overworld".into()],
            max_players: 20,
            chunk_radius: 8,
            simulation_distance: 8,
            reduced_debug_info: false,
            show_death_screen: true,
            limited_crafting: false,
            spawn: play::SpawnInfo {
                dimension_type: 0,
                dimension: "minecraft:overworld".into(),
                seed: 1,
                game_type: 0,
                previous_game_type: None,
                debug: false,
                flat: false,
                last_death: None,
                portal_cooldown: 0,
                sea_level: 63,
            },
            online_mode: false,
            enforces_secure_chat: false,
        })
    }
pub fn power_update(
        builtin: &Builtin,
        tags: &[(&str, &[&str])],
    ) -> UpdateTags {
        let blocks = builtin.get("minecraft:block").unwrap();
        UpdateTags {
            registries: vec![RegistryTags {
                registry: "minecraft:block".into(),
                tags: tags
                    .iter()
                    .map(|(name, members)| TagEntry {
                        name: (*name).into(),
                        ids: members.iter().map(|id| blocks.id(id).unwrap()).collect(),
                    })
                    .collect(),
            }],
        }
    }
}
