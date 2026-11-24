#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::DISMOUNT_POSES;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::DismountWorld;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::GameDismountWorld;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::Mount;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::MountState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::Rider;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::animal_dismount_location;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::dismount_location;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::dismounts_underwater;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::eating;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::horse_dismount_location;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::horse_side;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::on_top;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::rider_poses;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::seat_of;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::start_riding_jump;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::steering_item;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::strider_dismount_location;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::turned;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::mounts::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    blocks::Fluid,
    first_person::Hand,
    hud::HudBatch,
    items::{EntityTypeTags, ItemStack},
    mobs::{Mob, MobEvent, Species, Who, equine},
    motion::{MotionWorld, PlayerMotion},
    pose::{Dimensions, Pose},
    sprint::Keys,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3, VoxelShape},
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::living_air::escape_vector;

impl Game<'_> {
    /// `AbstractHorse.mobInteract`. Returns whether the click was used.
    pub(in crate::game) fn interact_mount(&mut self, index: usize, item: &str) -> bool {
        let held = self.held().cloned();
        let m = &self.mobs[index];
        let species = m.species();
        // Pigs and striders check occupancy only for boarding, not for saddling.
        if matches!(species, Species::Pig | Species::Strider) {
            return self.interact_steerable(index, item);
        }
        // A server player rides it already (`isVehicle`: no seat, no feeding).
        if self.ridden_by_guest(m.id) {
            return false;
        }
        if sunrise_client::mobs::llama::is_llama(species) {
            return self.interact_llama(index, item);
        }
        if species == Species::HappyGhast {
            return self.interact_happy_ghast(index, item);
        }
        if sunrise_client::mobs::nautilus::is_nautilus(species) {
            return self.interact_nautilus(index, item);
        }
        if !equine::is_equine(species) || !m.alive() || Some(m.id) == self.riding {
            return false;
        }
        // AbstractHorse.mobInteract: a horse a mob rides (`isVehicle`) has no
        // seat, inventory or feeding to give. Camels seat two and check their
        // passengers themselves.
        let id = m.id;
        if !matches!(species, Species::Camel | Species::CamelHusk)
            && self
                .mobs
                .iter()
                .any(|p| p.extra.mount == Some(id) && !p.removed && p.alive())
        {
            return false;
        }
        // SkeletonHorse.mobInteract: nothing for an untamed one.
        if species == Species::SkeletonHorse && !m.extra.tame {
            return false;
        }
        let sneaking = self.held_mapping(Action::Sneak)
            || self.client.keys.raw(&winit::keyboard::KeyCode::ShiftRight);
        // Horse / ZombieHorse.mobInteract: food is fed, and anything else in
        // hand makes an untamed one mad instead of seating the player.
        let dandelion = [self.held(), self.offhand.as_ref()]
            .into_iter()
            .flatten()
            .any(|s| &*s.item == "minecraft:golden_dandelion");
        let mut events = Vec::new();
        match equine::held_item(
            &mut self.mobs[index],
            item,
            sneaking,
            dandelion,
            &mut events,
        ) {
            Some(equine::HeldItem::Feed) => return self.feed_mount(index, item),
            Some(equine::HeldItem::Refused) => {
                let _ = self.handle_mob_events(events);
                return true;
            }
            None => {}
        }
        let m = &self.mobs[index];
        // AbstractHorse.mobInteract: shift + use opens a tamed adult's inventory.
        if !m.is_baby() && m.extra.tame && sneaking {
            self.open_mount_menu(index);
            return true;
        }
        if self.feed_mount(index, item) {
            return true;
        }
        let m = &mut self.mobs[index];
        if m.is_baby() {
            return false;
        }
        let at = m.center();
        let tamed = m.extra.tame;
        let sound = if item == "minecraft:saddle" && tamed && !m.extra.saddled() {
            m.set_saddle(held.map(|s| ItemStack { count: 1, ..s }));
            // Camel.getSaddleSound (the equip sound of a camel's saddle).
            Some(match species {
                Species::Camel => "minecraft:entity.camel.saddle",
                Species::CamelHusk => "minecraft:entity.camel_husk.saddle",
                _ => "minecraft:entity.horse.saddle",
            })
        } else if item.ends_with("_horse_armor")
            && species == Species::Horse
            && tamed
            && m.extra.body_id().is_none()
        {
            m.extra
                .set_body_stack(held.map(|s| ItemStack { count: 1, ..s }));
            Some("minecraft:entity.horse.armor")
        } else if item == "minecraft:chest"
            && matches!(species, Species::Donkey | Species::Mule)
            && tamed
            && !m.extra.chested
        {
            m.extra.chested = true;
            // AbstractChestedHorse.playChestEquipsSound, which Mule overrides
            // with its own chest sound.
            Some(match species {
                Species::Mule => "minecraft:entity.mule.chest",
                _ => "minecraft:entity.donkey.chest",
            })
        } else {
            None
        };
        if let Some(sound) = sound {
            let chest = item == "minecraft:chest";
            let m = &mut self.mobs[index];
            let silent = m.silent;
            // `LivingEntity.onEquipItem`: a saddle's or armour's equip sound plays
            // at the mount's feet at pitch `1.0F`, seeded with the mount's own
            // random's `nextLong`. `AbstractChestedHorse.playChestEquipsSound`
            // pitches a chest's sound from two floats of that same random, which
            // it draws even while silent, at volume 1.0.
            let pitch = if chest {
                (m.random.next_float() - m.random.next_float()) * 0.2 + 1.0
            } else {
                1.0
            };
            let seed = if chest || silent {
                None
            } else {
                Some(m.random.next_long())
            };
            let feet = [m.position.x, m.position.y, m.position.z];
            if !silent {
                match seed {
                    Some(seed) => self
                        .audio
                        .0
                        .play_seeded(sound, Some(feet), 1.0, pitch, seed),
                    None => self.audio.0.play(sound, Some(feet), 1.0, pitch),
                }
            }
            self.consume_held();
            if !chest {
                let id = self.mobs[index].id;
                let pos = self.mobs[index].position;
                self.mob_equip_event(id, [pos.x, pos.y, pos.z], true);
            }
            self.world
                .modified
                .insert((at[0].floor() as i32 >> 4, at[2].floor() as i32 >> 4));
            return true;
        }
        // doPlayerRide: `AbstractHorse.doPlayerRide` turns the rider to the
        // horse's yaw before it seats them.
        let id = self.mobs[index].id;
        if !self.native_may_board(id) {
            return true;
        }
        let m = &mut self.mobs[index];
        m.extra.flag = false;
        m.extra.counter = 0;
        self.yaw = m.yaw;
        self.mount.jump_ticks = 0;
        self.mount.jump_scale = 0.0;
        self.start_riding(id);
        true
    }
    /// `Pig.mobInteract` / `Strider.mobInteract`: saddle an adult, or ride a saddled mount with no passenger.
    pub(in crate::game) fn interact_steerable(&mut self, index: usize, item: &str) -> bool {
        let held = self.inventory[self.selected].clone();
        let sneaking = self.held_mapping(Action::Sneak);
        let m = &self.mobs[index];
        if m.is_baby() || !m.alive() || Some(m.id) == self.riding {
            return false;
        }
        let at = m.center();
        if item == "minecraft:saddle" && !m.extra.saddled() {
            let sound = if m.species() == Species::Strider {
                "minecraft:entity.strider.saddle"
            } else {
                "minecraft:entity.pig.saddle"
            };
            self.mobs[index].set_saddle(held.map(|s| ItemStack { count: 1, ..s }));
            self.audio.0.play(sound, Some(at), 1.0, 1.0);
            self.consume_held();
            let id = self.mobs[index].id;
            let pos = self.mobs[index].position;
            self.mob_equip_event(id, [pos.x, pos.y, pos.z], true);
            self.world
                .modified
                .insert((at[0].floor() as i32 >> 4, at[2].floor() as i32 >> 4));
            return true;
        }
        // Pig.mobInteract: held food goes to Animal.mobInteract, not a ride.
        let food = m.kind.food.iter().any(|f| f.as_str() == item);
        if !m.extra.saddled() || food || sneaking || self.has_seated_passengers(m.id) {
            return false;
        }
        // `Strider.canAddPassenger`: a strider whose eyes are in lava has no
        // seat, though the click still counts as used.
        if m.species() == Species::Strider && m.fluids.eye_in_lava {
            return true;
        }
        let id = m.id;
        if !self.native_may_board(id) {
            return true;
        }
        self.start_riding(id);
        true
    }
    /// `Entity.interact` / `Mob.canShearEquipment` / `Mob.attemptToShearEquipment`
    /// / `Mob.shearItem`: the saddle of a `#can_equip_saddle` mob comes off
    /// before its mobInteract can ride it, with the saddle's shearing sound.
    /// An occupied mount is not sheared: its mobInteract runs instead.
    pub(in crate::game) fn shear_saddle(&mut self, index: usize, item: &str) -> Option<bool> {
        let m = &self.mobs[index];
        // EntityTypeTags.CAN_EQUIP_SADDLE.
        let saddleable = matches!(
            m.species(),
            Species::Horse
                | Species::SkeletonHorse
                | Species::ZombieHorse
                | Species::Donkey
                | Species::Mule
                | Species::Pig
                | Species::Strider
                | Species::Camel
                | Species::CamelHusk
                | Species::Nautilus
                | Species::ZombieNautilus
        );
        if item != "minecraft:shears" || !saddleable || m.is_baby() || !m.alive() {
            return None;
        }
        let id = m.id;
        if self.riding == Some(id)
            || self.ridden_by_guest(id)
            || equine::is_vehicle(m)
            || self
                .mobs
                .iter()
                .any(|r| r.alive() && !r.removed && r.extra.mount == Some(id))
        {
            // !canShearEquipment (isVehicle): the click goes on to mobInteract. A
            // ridden pig or strider has nothing more to give, but a camel takes a
            // second rider and a horse or nautilus opens its inventory.
            return matches!(m.species(), Species::Pig | Species::Strider).then_some(false);
        }
        if self.held_mapping(Action::Sneak) {
            return None;
        }
        // attemptToShearEquipment takes the body slot before the saddle: horse
        // and nautilus armor come off first (not modelled here).
        if m.extra
            .body_id()
            .is_some_and(|b| b.ends_with("_horse_armor") || b.ends_with("_nautilus_armor"))
        {
            return None;
        }
        let saddle = m.saddle_item()?;
        if &*saddle.item != "minecraft:saddle" {
            return None;
        }
        let silent = m.silent;
        let at = m.position;
        let offset = sunrise_client::mobs::riders::passenger_attachment(m)[1];
        self.damage_held_tool();
        self.mobs[index].set_saddle(None);
        let context = self.who_context(Some(Who::Player), None);
        self.post_game_event("minecraft:shear", [at.x, at.y, at.z], context);
        self.mob_equip_event(id, [at.x, at.y, at.z], false);
        self.spawn_at_location(at, offset, saddle.clone());
        let target = self.mob_entity(&self.mobs[index]);
        self.trigger_item_entity("minecraft:player_sheared_equipment", &saddle, &target);
        // attemptToShearEquipment: playSound(equippable.shearingSound()).
        if !silent {
            self.audio.0.play(
                "minecraft:item.saddle.unequip",
                Some([at.x, at.y, at.z]),
                1.0,
                1.0,
            );
        }
        Some(true)
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::mounts::tests::SURFACE;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::mounts::tests::Sea;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::mounts::tests::Shown;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::mounts::tests::block_of;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::mounts::tests::mob;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::mounts::tests::mount;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::mounts::tests::rider;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::mounts::tests::server_game;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::mounts::tests::sparkle_draws;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::mounts::tests::strider_at;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::mounts::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use std::collections::HashMap;

    /// `AbstractChestedHorse.equipChest`: a tamed donkey or mule given a chest
    /// plays its chest sound at volume 1.0 (`AbstractChestedHorse`
    /// `playChestEquipsSound` uses `DONKEY_CHEST`, `Mule` overrides it with
    /// `MULE_CHEST`) and posts no mob equip event, which only the saddle and
    /// the armour do.
    #[test]
    #[ignore = "daily"]
    fn a_chest_on_a_donkey_or_mule_plays_its_own_sound_at_full_volume() {
        let mut resources = None;
        let Some(mut game) = server_game(&mut resources) else {
            return;
        };
        for (kind, sound) in [
            ("minecraft:donkey", "minecraft:entity.donkey.chest"),
            ("minecraft:mule", "minecraft:entity.mule.chest"),
        ] {
            mount(&mut game, kind, 5, [2.5, 64.0, 2.5], 7);
            game.inventory[game.selected] = Some(ItemStack::new("minecraft:chest", 1));
            game.audio.0.capture = Some(Vec::new());
            crate::game::sculk::TEST_POSTED_EVENTS
                .with(|events| *events.borrow_mut() = Some(Vec::new()));
            assert!(game.interact_mount(0, "minecraft:chest"), "{kind}");
            let sounds: Vec<_> = game
                .audio
                .0
                .capture
                .take()
                .unwrap()
                .into_iter()
                .map(|s| (s.event, s.position, s.volume, s.pitch))
                .collect();
            let equip_events = crate::game::sculk::TEST_POSTED_EVENTS
                .with(|events| events.borrow_mut().take().unwrap_or_default());
            crate::game::sculk::TEST_POSTED_EVENTS.with(|events| *events.borrow_mut() = None);
            assert!(game.mobs[0].extra.chested, "{kind} wears the chest");
            assert_eq!(sounds.len(), 1, "{kind}: {sounds:?}");
            assert_eq!(sounds[0].0, sound, "{kind} chest sound");
            assert_eq!(sounds[0].2, 1.0, "{kind} chest sound volume");
            assert!(
                (0.8..=1.2).contains(&sounds[0].3),
                "{kind} chest pitch {} outside vanilla's 0.8..1.2",
                sounds[0].3
            );
            assert!(
                equip_events.is_empty(),
                "{kind} posted a mob equip event: {equip_events:?}"
            );
        }
    }

    /// Equips `item` on mob 0 and collects the sounds and the `minecraft:equip`
    /// game events it made.
    pub(in crate::game) fn equip(game: &mut Game<'_>, item: &str) -> (bool, Shown) {
        game.inventory[game.selected] = Some(ItemStack::new(item, 2));
        game.audio.0.capture = Some(Vec::new());
        crate::game::sculk::TEST_POSTED_EVENTS
            .with(|events| *events.borrow_mut() = Some(Vec::new()));
        let used = game.interact_mount(0, item);
        let sounds = game
            .audio
            .0
            .capture
            .take()
            .unwrap()
            .into_iter()
            .map(|s| (s.event, s.position, s.volume, s.pitch))
            .collect();
        let events = crate::game::sculk::TEST_POSTED_EVENTS.with(|events| {
            events
                .borrow_mut()
                .take()
                .unwrap_or_default()
                .into_iter()
                .filter(|(event, _, _)| *event == "minecraft:equip")
                .collect()
        });
        (used, Shown { sounds, events })
    }

    /// Feeds `item` to mob 0 and collects the sounds and the `minecraft:eat`
    /// game events it made.
    pub(in crate::game) fn feed(game: &mut Game<'_>, item: &str) -> (bool, Shown) {
        game.inventory[game.selected] = Some(ItemStack::new(item, 2));
        game.audio.0.capture = Some(Vec::new());
        crate::game::sculk::TEST_POSTED_EVENTS
            .with(|events| *events.borrow_mut() = Some(Vec::new()));
        let used = game.interact_mount(0, item);
        let sounds = game
            .audio
            .0
            .capture
            .take()
            .unwrap()
            .into_iter()
            .map(|s| (s.event, s.position, s.volume, s.pitch))
            .collect();
        let events = crate::game::sculk::TEST_POSTED_EVENTS.with(|events| {
            events
                .borrow_mut()
                .take()
                .unwrap_or_default()
                .into_iter()
                .filter(|(event, _, _)| *event == "minecraft:eat")
                .collect()
        });
        (used, Shown { sounds, events })
    }
}
