use super::Game;
use sunrise_sim::mobs::happy_ghast;

impl Game<'_, crate::game::server_side::ServerState> {
    /// `HappyGhast.mobInteract`. Returns whether the click was used.
    pub(in crate::game) fn interact_happy_ghast(&mut self, index: usize, item: &str) -> bool {
        let m = &self.mobs[index];
        if !m.alive() || Some(m.id) == self.riding {
            return false;
        }
        if m.is_baby() {
            // Animal.mobInteract: food grows it up.
            return false;
        }
        let at = m.center();
        let sneaking = self.server_acting_sneak();
        let harnessed = m.extra.body_id().is_some();
        // Equippable.equipOnInteract: a harness goes on a bare adult.
        if item.ends_with("_harness") && item.starts_with("minecraft:") && !harnessed {
            let held = self.inventory[self.selected].clone().map(|mut stack| {
                stack.count = 1;
                stack
            });
            self.mobs[index].extra.set_body_stack(held);
            self.server_play_sound("minecraft:entity.happy_ghast.equip", at, 1.0, 1.0);
            self.consume_held();
            let id = self.mobs[index].id;
            let pos = self.mobs[index].position;
            self.mob_equip_event(id, [pos.x, pos.y, pos.z], true);
            return true;
        }
        // Equippable.canBeSheared: shears take the harness off again.
        if item == "minecraft:shears"
            && harnessed
            && !sneaking
            && let Some(harness) = self.mobs[index].take_body_equipment()
        {
            self.server_play_sound("minecraft:entity.happy_ghast.unequip", at, 1.0, 1.0);
            self.spawn_item(harness, at, [0.0, 0.2, 0.0], 10);
            self.damage_held_tool();
            let id = self.mobs[index].id;
            let pos = self.mobs[index].position;
            self.mob_equip_event(id, [pos.x, pos.y, pos.z], false);
            return true;
        }
        if harnessed && !sneaking {
            // doPlayerRide: `player.startRiding(this)`, which keeps the
            // rider's view (`ServerPlayer.startRiding`'s `Relative.ROTATION`).
            let id = self.mobs[index].id;
            // `startRiding` without `force` asks `canAddPassenger`: a fifth rider is refused.
            let aboard = self.seated_passengers(id).len();
            if !sunrise_sim::mobs::riders::can_add_passenger(&self.mobs[index], aboard)
                || !self.native_may_board(id)
            {
                return true;
            }
            self.break_leash(id, true, false);
            let was_empty = self.mobs[index].rider.is_none();
            // addPassenger: the rider is not standing on top, so no still
            // timeout.
            self.mobs[index].extra.counter = 0;
            self.mount.reset_jump();
            if was_empty {
                let down = happy_ghast::goggles(true, &self.mobs[index]);
                let _ = self.handle_mob_events(vec![down]);
            }
            self.start_riding(id);
            return true;
        }
        false
    }
}
