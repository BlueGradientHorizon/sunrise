#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::api_values::{
    StackReadMetadata, project_stack, stack_read_metadata,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::commands::{Target, items::Owner};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::living_attributes::AttributeWrite;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::alive_at;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::api_modifier;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::armor_index;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::attribute_modifiers;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::equipment_stack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::hand_slot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::lend_equipment;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::player_slot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::slot_name;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::with_equipment_view;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::world_equipment_slot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::mobs::Who;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::client::first_person::Hand;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    AttributeModifier, AttributeOperation, Direction, EntityRef, EquipmentSlot, Identifier,
    ItemStackView, LivingRef, ModifierLifetime,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::health::{
    Attack, DamageLevel, HitEffects, Hurting, Knock, NO_ATTACK,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::items::{EntityTypeTags, ItemStack};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::{
    Mob,
    finalize::{self, EquipmentRead},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::attributes::{Modifier, Operation};

impl Game<'_> {
    /// The effects of `LivingEntity.onEquipItem` once the guards passed (server
    /// side, not a spectator, not the first tick, another item): the equip
    /// sound when there is one, and the `equip` (the stack is equippable) or
    /// `unequip` game event.
    pub(in crate::game) fn equip_item_effects(
        &mut self,
        at: [f64; 3],
        sound: Option<String>,
        equips: bool,
        ctx: sunrise_client::sculk::Context,
    ) {
        if let Some(sound) = sound {
            self.audio.0.play(&sound, Some(at), 1.0, 1.0);
        }
        self.post_game_event(
            if equips {
                "minecraft:equip"
            } else {
                "minecraft:unequip"
            },
            at,
            ctx,
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::super::api_values::TryIntoApi;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::tests::ZOMBIE_NBT;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::tests::ZOMBIE_UUID;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::tests::hurt_pig;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::tests::send_to_the_nether;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::tests::summon_mob;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::native_living_refs::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;

    #[test]
    #[ignore = "daily"]
    fn armor_stand_first_tick_equipment_change_fires_after_base_tick() {
        use mod_api::{self, Hooks, LivingEquipmentChangeEvent, ModEntry, ServerHost};
        struct Reequip;
        #[mod_api::__subscriptions]
        impl Hooks for Reequip {
            fn living_equipment_change(
                &self,
                host: &mut dyn ServerHost,
                event: &mut LivingEquipmentChangeEvent<'_>,
            ) {
                if event.entity == 18 && event.slot == "mainhand" {
                    let entity = host.entity(event.entity).unwrap();
                    let living = host.living(entity).unwrap();
                    assert!(
                        living
                            .set_equipment(
                                host,
                                EquipmentSlot::Head,
                                Some(
                                    ItemStack::new("minecraft:iron_helmet", 1)
                                        .try_into_api()
                                        .unwrap()
                                ),
                            )
                            .unwrap()
                    );
                }
            }
        }
        mod_platform::testing::with_mods(
            vec![ModEntry::new("first_tick_equipment", Box::new(Reequip))].into(),
            || {
                super::super::native_entity_refs::tests::with_game(|game| {
                    let mut stand = sunrise_client::entities::ArmorStand::new(
                        18,
                        Vec3::new(0.5, 70.0, 0.5),
                        0.0,
                    );
                    stand.no_gravity = true;
                    stand.equipment[0] = Some(ItemStack::new("minecraft:stone", 1));
                    game.armor_stands.push(stand);
                    game.audio.0.capture = Some(Vec::new());
                    super::super::dispense_fixture::start_recording();
                    game.tick_armor_stands().unwrap();
                    let events = super::super::dispense_fixture::stop_recording();
                    assert_eq!(
                        game.armor_stands[0].equipment[5],
                        Some(ItemStack::new("minecraft:iron_helmet", 1)),
                        "real listener/handle write control",
                    );
                    assert!(!game.armor_stands[0].first_tick);
                    let sounds = game.audio.0.capture.as_ref().unwrap().len();
                    let equips = events
                        .iter()
                        .filter(|e| e.ends_with("minecraft:equip"))
                        .count();
                    assert_eq!((sounds, equips), (1, 1));
                });
            },
        );
    }

    #[test]
    #[ignore = "daily"]
    fn mob_equipment_writes_emit_vanilla_equip_events() {
        use sunrise_client::mobs::Mob;
        use sunrise_core::geometry::Vec3;
        super::super::native_entity_refs::tests::with_game(|game| {
            let kind = game
                .res
                .mob_types()
                .get("minecraft:zombie")
                .unwrap()
                .clone();
            game.mobs.push(Mob::new(12, kind, Vec3::ZERO, 0.0));
            let living = game.living(game.entity(12).unwrap()).unwrap();
            game.audio.0.capture = Some(Vec::new());
            super::super::dispense_fixture::start_recording();
            let helmet = ItemStack::new("minecraft:iron_helmet", 1);
            assert!(game.living_set_equipment(living, EquipmentSlot::Head, Some(helmet.clone())));
            assert!(game.living_set_equipment(living, EquipmentSlot::Head, None));
            assert!(game.audio.0.capture.as_ref().unwrap().is_empty());
            game.mobs[0].first_tick = false;
            assert!(game.living_set_equipment(living, EquipmentSlot::Head, Some(helmet.clone())));
            assert_eq!(game.audio.0.capture.as_ref().unwrap().len(), 1);
            assert!(game.living_set_equipment(living, EquipmentSlot::Head, Some(helmet)));
            assert_eq!(game.audio.0.capture.as_ref().unwrap().len(), 1);
            game.mobs[0].silent = true;
            assert!(game.living_set_equipment(
                living,
                EquipmentSlot::Head,
                Some(ItemStack::new("minecraft:diamond_helmet", 1))
            ));
            assert!(game.living_set_equipment(living, EquipmentSlot::Head, None));
            game.mobs[0].silent = false;
            // Equippable, but its head slot does not match chest: event, no sound.
            assert!(game.living_set_equipment(
                living,
                EquipmentSlot::Chest,
                Some(ItemStack::new("minecraft:iron_helmet", 1))
            ));
            assert_eq!(game.audio.0.capture.as_ref().unwrap().len(), 1);
            let events = super::super::dispense_fixture::stop_recording();
            assert_eq!(
                events
                    .iter()
                    .filter(|e| e.ends_with("minecraft:equip"))
                    .count(),
                3
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|e| e.ends_with("minecraft:unequip"))
                    .count(),
                1
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn stand_and_dragon_equipment_writes_run_on_equip_item() {
        use sunrise_core::geometry::Vec3;
        super::super::native_entity_refs::tests::with_game(|game| {
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
            );
            let mut stand = sunrise_client::entities::ArmorStand::new(16, Vec3::ZERO, 0.0);
            stand.id = 16;
            game.armor_stands.push(stand);
            let mut dragon = super::super::dragon::Dragon::new(Vec3::ZERO, 0.0, 0);
            dragon.id = 17;
            game.dragon.replace(dragon);
            let stand = game.living(game.entity(16).unwrap()).unwrap();
            let dragon = game.living(game.entity(17).unwrap()).unwrap();
            game.audio.0.capture = Some(Vec::new());
            super::super::dispense_fixture::start_recording();
            let helmet = ItemStack::new("minecraft:iron_helmet", 1);
            // Before its first tick an entity makes no sound and no event.
            assert!(game.living_set_equipment(stand, EquipmentSlot::Head, Some(helmet.clone())));
            assert!(game.living_set_equipment(stand, EquipmentSlot::Head, None));
            assert!(game.audio.0.capture.as_ref().unwrap().is_empty());
            game.tick_armor_stands().unwrap();
            game.dragon.as_mut().unwrap().ticks = 1;
            // An equippable item in its own slot: sound and `equip`; the same
            // item again changes nothing.
            for _ in 0..2 {
                assert!(game.living_set_equipment(
                    stand,
                    EquipmentSlot::Head,
                    Some(helmet.clone())
                ));
            }
            // A stored body armor: sound and `equip`, body is storage only.
            let body = ItemStack::new("minecraft:diamond_horse_armor", 1);
            assert!(game.living_set_equipment(stand, EquipmentSlot::Body, Some(body)));
            // Not equippable: `unequip` and no sound.
            let stone = ItemStack::new("minecraft:stone", 1);
            assert!(game.living_set_equipment(stand, EquipmentSlot::Saddle, Some(stone.clone())));
            // The dragon the same.
            let chest = ItemStack::new("minecraft:iron_chestplate", 1);
            assert!(game.living_set_equipment(dragon, EquipmentSlot::Chest, Some(chest)));
            assert!(game.living_set_equipment(dragon, EquipmentSlot::Saddle, Some(stone)));
            assert_eq!(game.audio.0.capture.as_ref().unwrap().len(), 3);
            let events = super::super::dispense_fixture::stop_recording();
            let count = |name: &str| events.iter().filter(|e| e.ends_with(name)).count();
            assert_eq!(count("minecraft:equip"), 3);
            assert_eq!(count("minecraft:unequip"), 2);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn the_host_players_body_and_saddle_are_slots_and_saved_in_equipment() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let owner = Owner::Entity(Target::Player);
            let mut body = ItemStack::new("minecraft:diamond_horse_armor", 1);
            body.damage = 4;
            body.data_mut().custom_name = Some(sunrise_core::nbt::Tag::String("armor".into()));
            let saddle = ItemStack::new("minecraft:saddle", 1);
            game.player = Some(
                sunrise_client::motion::PlayerMotion::new(
                    game.motion,
                    sunrise_core::geometry::Vec3::ZERO,
                )
                .unwrap(),
            );
            game.audio.0.capture = Some(Vec::new());
            for (name, stack) in [("armor.body", &body), ("saddle", &saddle)] {
                let slots = game.slots_named(&owner, name);
                assert_eq!(slots.len(), 1, "{name} is one slot");
                assert!(game.slot_set(&slots[0], Some(stack.clone())));
            }
            assert_eq!(
                (game.equipment.get(6), game.equipment.get(7)),
                (Some(&body), Some(&saddle))
            );
            // onEquipItem: each item in its own slot plays its equip sound.
            assert_eq!(game.audio.0.capture.as_ref().unwrap().len(), 2);
            game.sync_level();
            let tag = game.level.player_tag(&Default::default());
            let equipment = tag.get("equipment").unwrap();
            assert!(equipment.get("body").is_some() && equipment.get("saddle").is_some());
            let mut read = sunrise_client::save::Level::default();
            read.read_player(&tag);
            // Loading the file back into a Game restores both, components too.
            game.equipment = Default::default();
            let level = read;
            game.load_saved_inventory(&level);
            assert_eq!(
                (game.equipment.get(6), game.equipment.get(7)),
                (Some(&body), Some(&saddle))
            );
        });
    }
}
