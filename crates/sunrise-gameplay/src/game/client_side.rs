#[cfg(feature = "client")]
#[cfg(test)]
#[path = "network/lan_trade_decisions_tests_client.rs"]
pub mod lan_trade_decisions_tests_client;
#[cfg(feature = "client")]
#[path = "modding/native_mob_refs_client.rs"]
pub(in crate::game) mod native_mob_refs_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_player_inventory_tests.rs"]
pub(in crate::game) mod native_player_inventory_tests;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "modding/native_player_level_tests_client.rs"]
pub(in crate::game) mod native_player_level_tests_client;
#[cfg(feature = "client")]
#[cfg(test)]
#[path = "items/use_same_item_tests.rs"]
pub(in crate::game) mod use_same_item_tests;

#[cfg(feature = "client")]
#[path = "modding/native_living_writes_client.rs"]
pub(in crate::game) mod native_living_writes_client;

#[cfg(feature = "client")]
#[path = "mobs/creatures/mod_client.rs"]
pub mod creatures_client;

#[path = "items/potions.rs"]
pub(in crate::game) mod potions;

#[path = "interface/books.rs"]
pub(in crate::game) mod books;

#[path = "entities/lightning.rs"]
pub(in crate::game) mod lightning;

#[path = "blocks/growth.rs"]
pub(in crate::game) mod growth;
