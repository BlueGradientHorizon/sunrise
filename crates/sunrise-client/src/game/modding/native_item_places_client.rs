#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::api_values::component_edit::{
    GearEdits, prepare_component_edit,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::native_server::PlayerRead;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::transactions::{After, DeferredEffect};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_item_places::PlayerStackSlot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_item_places::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::__private::game::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    CraftedByError, EquipmentSlot, ItemComponentEditor, ItemPlace, ItemPlaceEditError,
    ItemStackView, LivingRef, MenuSlotBacking, TransactionContext,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    first_person::Hand,
    items::{ItemStack, Items},
    menu::Menu,
    mobs::finalize::{self, EquipmentRead},
};

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::super::BlockEntity;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::super::api_values::TryIntoApi;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::super::native_entity_refs::tests::{
        with_client_game, with_game,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_item_places::tests::Bin;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_item_places::tests::Offer;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_item_places::tests::guest;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_item_places::tests::open_to_lan;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_item_places::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use mod_api::HostTransaction;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use mod_api::ServerHost;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use mod_api::{
        CraftedByError, ENTITY_AUTOMATION, ENTITY_ITEMS, ItemPlace, ItemPlaceEditError,
        ItemResource, MenuRef, TransactionError,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use std::sync::{Arc, atomic::AtomicI32, mpsc};
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_client::{
        items::ItemStack,
        lan::{Joined, NetConfig, PlayerSender, guest::Guest, hub::HubConfig},
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_core::geometry::Direction;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_protocol::packets::configuration::ClientInformation;

    #[test]
    #[ignore = "daily"]
    fn stand_equipment_effects_wait_for_root_commit() {
        with_game(|game| {
            let mut stand = sunrise_client::entities::ArmorStand::new(
                40,
                sunrise_core::geometry::Vec3::ZERO,
                0.0,
            );
            stand.first_tick = false;
            game.armor_stands.push(stand);
            game.start_audio_capture();
            let items = game
                .entity(40)
                .unwrap()
                .capability(game, &ENTITY_ITEMS, ())
                .unwrap();
            let chest = ItemResource::of(
                &ItemStack::new("minecraft:diamond_chestplate", 1)
                    .try_into_api()
                    .unwrap(),
            );
            super::super::dispense_fixture::start_recording();
            let result = game.storage_transaction(|host, tx| {
                assert_eq!(items.insert_at(host, tx, 4, &chest, 1), Ok(1));
                Err::<(), ()>(())
            });
            assert_eq!(result, Err(()));
            assert!(game.armor_stands[0].equipment.get(4).is_none());
            assert!(
                game.side.pending_sounds.is_empty(),
                "aborted stand equip emitted sound"
            );
            assert!(super::super::dispense_fixture::stop_recording().is_empty());

            super::super::dispense_fixture::start_recording();
            let result = game.storage_transaction(|host, _| {
                host.storage_transaction(|host, tx| {
                    assert_eq!(items.insert_at(host, tx, 4, &chest, 1), Ok(1));
                    Ok::<(), ()>(())
                })
                .unwrap();
                Err::<(), ()>(())
            });
            assert_eq!(result, Err(()));
            assert!(game.armor_stands[0].equipment.get(4).is_none());
            assert!(game.side.pending_sounds.is_empty());
            assert!(super::super::dispense_fixture::stop_recording().is_empty());

            super::super::dispense_fixture::start_recording();
            let result = game.storage_transaction(|host, _| {
                // An inner abort must not contribute effects to the root.
                let aborted = host.transaction(|host, tx| {
                    assert_eq!(items.insert_at(host, tx, 4, &chest, 1), Ok(1));
                    Err::<(), ()>(())
                });
                assert_eq!(aborted, Err(TransactionError::Aborted(())));
                host.transaction(|host, tx| {
                    assert_eq!(items.insert_at(host, tx, 4, &chest, 1), Ok(1));
                    Ok::<(), ()>(())
                })
                .unwrap();
                assert!(super::super::dispense_fixture::stop_recording().is_empty());
                super::super::dispense_fixture::start_recording();
                Ok::<(), ()>(())
            });
            assert_eq!(result, Ok(()));
            assert_eq!(game.armor_stands[0].equipment.get(4).unwrap().count, 1);
            assert_eq!(game.side.pending_sounds.len(), 1);
            let events = super::super::dispense_fixture::stop_recording();
            assert_eq!(events.len(), 1, "only one event at root commit: {events:?}");
            assert!(events[0].ends_with("minecraft:equip"));
        });
    }
}
