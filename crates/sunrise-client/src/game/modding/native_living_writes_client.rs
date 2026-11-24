#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, commands::Target};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_living_writes::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{Identifier, LivingRef};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::first_person::{Hand, SwingAnimation};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::lan::waypoints::Icon;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::{spear_use, stuck::Stuck};

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::super::native_entity_refs::tests::with_game;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::super::native_living_flags::tests::{
        guest, guest_server, packets, sync_guests,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::super::native_living_position::tests::floor_and_zombies;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::Stuck;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_living_writes::tests::Shared;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_living_writes::tests::UseMod;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_living_writes::tests::UsePart;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_living_writes::tests::Uses;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_living_writes::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use mod_api::client::first_person::Hand;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use mod_api::{
        EntityRef, Flow, Hooks, HostSide, Identifier, ItemPart, LivingEntityUseItemFinishEvent,
        LivingEntityUseItemStartEvent, LivingEntityUseItemStopEvent, LivingEntityUseItemTickEvent,
        LivingRef, ModEntry, Mods, OnUseTickEvent, Registrar, ReleaseUsingEvent, WorldEpoch,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use std::sync::{Arc, Mutex};
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_client::items::ItemStack;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_client::lan::waypoints::Icon;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_client::mobs::Mob;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;

    /// Runs `test` on a game whose acting player stands at the origin, the use
    /// button down, under a mod that fills the returned record.
    pub(in crate::game) fn with_uses(test: impl FnOnce(&mut super::Game<'_>, &Shared, LivingRef)) {
        let uses = Shared::default();
        let mods: Mods = vec![ModEntry::new("uses", Box::new(UseMod(uses.clone())))].into();
        crate::game::api_values::install_parts(&mods).unwrap();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                crate::game::native_entity_refs::tests::resolve_parts(game).unwrap();
                game.player = Some(
                    sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::ZERO).unwrap(),
                );
                crate::game::native_entity_refs::tests::bind_local_player(game, 20, 123);
                game.client.screen = super::super::Screen::Playing;
                game.set_mapping_held("key.use", true);
                let player = game.living(game.entity(20).unwrap()).unwrap();
                test(game, &uses, player);
            });
        });
    }
}
