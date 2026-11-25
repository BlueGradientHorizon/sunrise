#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::level_sounds::LevelSound;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::copper_golems::RECHECK_TICKS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::copper_golems::golem_preserved_equipment;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::copper_golems::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::SoundSource;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    containers::BlockEntity,
    items::ItemStack,
    mobs::{
        Mob, Species,
        transport_items::{held, set_held},
    },
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::{Compound, Tag};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::random::RandomSource;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::chunk::BlockState;

impl Game<'_> {
    /// A copper golem opened or closed a chest half: the counter changes, the
    /// first opener posts CONTAINER_OPEN and the last closer CONTAINER_CLOSE
    /// (`ContainerOpenersCounter.incrementOpeners` / `decrementOpeners`), and
    /// the clients see the block event (`openerCountChanged`).
    pub(in crate::game) fn golem_chest_open(&mut self, chest: [i32; 3], open: bool, mob: u32) {
        let key = (chest[0], chest[1], chest[2]);
        if self.mod_menu_at(key).is_some() {
            self.mod_container_opener(key, open, Some(sunrise_client::mobs::Who::Mob(mob)), 3.0);
            return;
        }
        let entry = self.world.chest_openers.entry(key).or_insert((0, 0.0));
        let before = entry.0;
        if open {
            entry.0 += 1;
            entry.1 = entry.1.max(3.0);
        } else {
            entry.0 -= 1;
            if entry.0 == 0 {
                entry.1 = 0.0;
            }
        }
        let count = entry.0;
        if count <= 0 {
            self.world.chest_openers.remove(&key);
        }
        let who = Some(sunrise_client::mobs::Who::Mob(mob));
        if open && before == 0 {
            self.post_block_game_event("minecraft:container_open", chest, who, None);
        } else if !open && count == 0 {
            self.post_block_game_event("minecraft:container_close", chest, who, None);
        }
        self.block_entity_event(chest, 1, count.max(0));
        // The guests get the same `ClientboundBlockEventPacket` (the lid).
        self.lan_block_event(chest, 1, count.max(0));
    }
    /// A copper golem took from or filled a chest slot.
    pub(in crate::game) fn golem_chest_slot(
        &mut self,
        chest: [i32; 3],
        slot: usize,
        stack: Option<ItemStack>,
    ) {
        let key = (chest[0], chest[1], chest[2]);
        if let Some(e) = self.world.block_entities.get_mut(&key)
            && let Some(s) = e.slot_mut(slot)
        {
            *s = stack;
            // TransportItemsBetweenContainers calls Container.setChanged.
            self.container_changed(key);
        }
    }
    /// `CopperGolem.mobInteract`; true when handled.
    pub(in crate::game) fn interact_copper_golem(&mut self, index: usize, item: &str) -> bool {
        if self.mobs[index].species() != Species::CopperGolem || !self.mobs[index].alive() {
            return false;
        }
        let at = {
            let m = &self.mobs[index];
            [m.position.x, m.position.y, m.position.z]
        };
        if item.is_empty() {
            // Empty hand: the golem throws what it carries back.
            let Some(h) = held(&self.mobs[index]).cloned() else {
                return false;
            };
            set_held(&mut self.mobs[index], None);
            // BehaviorUtils.throwItem: from 0.3 below the golem's eyes towards
            // the player's feet at 0.3 blocks a tick, default pickup delay.
            let p = self.player.as_ref().map(|p| p.position);
            let eye = self.mobs[index].eye_height();
            let from = [at[0], at[1] + eye - f64::from(0.3f32), at[2]];
            let v = p.map_or([0.0; 3], |p| {
                let d = [p.x - at[0], p.y - at[1], p.z - at[2]];
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                if len < 1.0e-5 {
                    return [0.0; 3];
                }
                let k = f64::from(0.3f32);
                [d[0] / len * k, d[1] / len * k, d[2] / len * k]
            });
            self.spawn_item(h, from, v, 10);
            return true;
        }
        let name = item;
        let is_axe = name.ends_with("_axe");
        let waxed = self.mobs[index].extra.vector[0] as i64 == -2;
        if name == "minecraft:shears" && self.golem_shearable(index) {
            // CopperGolem.shear: the sound, the antenna slot emptied (an
            // unequip) and the flower dropped 1.5 up.
            // level.playSound(null, this, COPPER_GOLEM_SHEAR, PLAYERS, 1, 1).
            let shear = LevelSound::new("minecraft:entity.copper_golem.shear", at, 1.0, 1.0)
                .in_source(SoundSource::Players);
            self.play_bound_sound(shear, self.mobs[index].id, None);
            let antenna = self.mobs[index].saddle_item();
            self.mobs[index].set_saddle(None);
            let id = self.mobs[index].id;
            self.mob_equip_event(id, at, false);
            if let Some(a) = antenna {
                let p = self.mobs[index].position;
                self.spawn_at_location(p, 1.5, a);
            }
            // mobInteract: gameEvent(SHEAR, player) after the shear, before the
            // tool's damage.
            let ctx = self.who_context(Some(sunrise_client::mobs::Who::Player), None);
            self.post_game_event("minecraft:shear", at, ctx);
            self.damage_held_tool();
            return true;
        }
        let block = {
            let p = self.mobs[index].position;
            [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]
        };
        if name == "minecraft:honeycomb" && !waxed {
            self.mobs[index].extra.vector[0] = -2.0;
            self.server_level_event_at(3003, block, 0);
            self.server_play_sound("minecraft:item.honeycomb.wax_on", at, 1.0, 1.0);
            self.consume_held();
            return true;
        }
        if is_axe && waxed {
            self.mobs[index].extra.vector[0] = -1.0;
            // level.playSound(null, this, AXE_SCRAPE, getSoundSource(), 1, 1).
            let scrape = LevelSound::new("minecraft:item.axe.scrape", at, 1.0, 1.0)
                .in_source(SoundSource::Neutral);
            self.play_bound_sound(scrape, self.mobs[index].id, None);
            self.server_level_event_at(3004, block, 0);
            self.damage_held_tool();
            return true;
        }
        if is_axe && self.mobs[index].extra.variant > 0 {
            let m = &mut self.mobs[index];
            m.extra.vector[0] = -1.0;
            m.extra.variant = sunrise_client::mobs::copper_golem::weather_previous(m.extra.variant);
            let id = m.id;
            self.persist.force_meta(id, 16);
            let scrape = LevelSound::new("minecraft:item.axe.scrape", at, 1.0, 1.0)
                .in_source(SoundSource::Neutral);
            self.play_bound_sound(scrape, id, None);
            self.server_level_event_at(3005, block, 0);
            self.damage_held_tool();
            return true;
        }
        false
    }
    /// `OfferFlowerGoal.stop` (`MobEvent::Gift`): the item on the target's
    /// empty antenna, as a guaranteed drop.
    pub(in crate::game) fn receive_gift(&mut self, target: u32, item: &'static str) {
        if let Some(m) = self.mobs.iter_mut().find(|m| m.id == target)
            && m.saddle_item().is_none()
        {
            m.set_saddle(Some(ItemStack::new(item, 1)));
            // OfferFlowerGoal.stop: Mob.setGuaranteedDrop(antenna).
            sunrise_client::mobs::finalize::set_drop_chance(
                m,
                sunrise_items::equipment::SADDLE,
                2.0,
            );
        }
    }
}
