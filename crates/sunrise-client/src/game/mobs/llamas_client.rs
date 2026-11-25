#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::llamas::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::{equine, llama};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::RandomSource;

impl Game<'_> {
    /// `AbstractChestedHorse.mobInteract` and `AbstractHorse.mobInteract` for
    /// llamas and trader llamas. Returns whether the click was used.
    pub(in crate::game) fn interact_llama(&mut self, index: usize, item: &str) -> bool {
        let m = &self.mobs[index];
        if !m.alive() || Some(m.id) == self.riding {
            return false;
        }
        let sneaking = self.server_acting_sneak();
        let (baby, tamed, vehicle) = (m.is_baby(), m.extra.tame, m.rider.is_some());
        let open_inventory = !baby && tamed && sneaking;
        if !vehicle && !open_inventory && !item.is_empty() {
            if self.mobs[index].kind.food.iter().any(|f| f == item) {
                return self.feed_llama(index, item);
            }
            if !tamed {
                let mut events = Vec::new();
                equine::make_mad(&mut self.mobs[index], &mut events);
                let _ = self.handle_mob_events(events);
                return true;
            }
            if !self.mobs[index].extra.chested && item == "minecraft:chest" {
                self.mobs[index].extra.chested = true;
                self.llama_sound(index, "entity.llama.chest", 1.0);
                self.consume_held();
                return true;
            }
        }
        // AbstractHorse.mobInteract.
        if vehicle || baby {
            return false;
        }
        if tamed && sneaking {
            // AbstractHorse.openCustomInventoryScreen.
            self.server_request_mount_menu(index);
            return true;
        }
        let m = &self.mobs[index];
        let carpet = |i: &str| i.ends_with("_carpet") && i.starts_with("minecraft:");
        if !item.is_empty() {
            if item == "minecraft:shears" && m.extra.body_id().is_some_and(carpet) {
                // Equippable.canBeSheared: the carpet comes off.
                let at = m.center();
                let carpet = self.mobs[index].take_body_equipment().unwrap();
                self.server_play_sound("minecraft:item.llama_carpet.unequip", at, 1.0, 1.0);
                self.spawn_item(carpet, at, [0.0, 0.2, 0.0], 10);
                self.damage_held_tool();
                let id = self.mobs[index].id;
                let pos = self.mobs[index].position;
                self.mob_equip_event(id, [pos.x, pos.y, pos.z], false);
                return true;
            }
            if carpet(item) && m.extra.body_id().is_none() {
                let held = self.inventory[self.selected].clone().map(|mut stack| {
                    stack.count = 1;
                    stack
                });
                self.mobs[index].extra.set_body_stack(held);
                self.llama_sound(index, "entity.llama.swag", 1.0);
                self.consume_held();
                let id = self.mobs[index].id;
                let pos = self.mobs[index].position;
                self.mob_equip_event(id, [pos.x, pos.y, pos.z], true);
                return true;
            }
        }
        // TraderLlama.doPlayerRide: not while a wandering trader leads it.
        if self.is_led_by_trader(index) {
            return true;
        }
        // doPlayerRide; mounting drops a lead.
        let id = self.mobs[index].id;
        if !self.native_may_board(id) {
            return true;
        }
        self.break_leash(id, true, false);
        let m = &mut self.mobs[index];
        m.extra.flag = false;
        m.extra.counter = 0;
        self.yaw = m.yaw;
        self.mount.reset_jump();
        self.start_riding(id);
        true
    }
    pub(in crate::game) fn llama_sound(&mut self, index: usize, sound: &str, volume: f32) {
        let at = self.mobs[index].center();
        let pitch = (self.world.random.next_float() - self.world.random.next_float()) * 0.2 + 1.0;
        self.server_play_sound(&format!("minecraft:{sound}"), at, volume, pitch);
    }
}
