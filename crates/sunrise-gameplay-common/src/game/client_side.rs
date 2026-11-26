#[cfg(feature = "client")]
#[path = "network/remote_tags.rs"]
pub mod remote_tags;

#[cfg(feature = "client")]
#[path = "modding/native_panel.rs"]
pub mod native_panel;

#[cfg(feature = "client")]
#[path = "modding/native_block_entity_draw.rs"]
pub mod native_block_entity_draw;

#[path = "items/potions.rs"]
pub mod potions;

#[cfg(feature = "client")]
#[path = "items/client_attack_targets.rs"]
pub mod client_attack_targets;

#[cfg(feature = "client")]
#[path = "interface/startup_menu.rs"]
pub mod startup_menu;

#[cfg(feature = "client")]
#[path = "interface/shader_packs.rs"]
pub mod shader_packs;

#[path = "interface/screen_build.rs"]
pub mod screen_build;

#[cfg(feature = "client")]
#[path = "interface/gui_entity.rs"]
pub mod gui_entity;

#[cfg(feature = "client")]
#[path = "interface/container_hooks.rs"]
pub mod container_hooks;

#[cfg(feature = "client")]
#[path = "interface/client_reload/mod.rs"]
pub mod client_reload;

#[path = "entities/lightning.rs"]
pub(in crate::game) mod lightning;

#[path = "blocks/growth.rs"]
pub mod growth;
