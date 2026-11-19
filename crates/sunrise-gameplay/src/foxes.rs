//! Foxes and dolphins: what they take from the ground and where they go for it, on `MobLevel`.
use crate::{item_areas::ItemAreas, mob_level::MobLevel};
use sunrise_client::mobs::{
    Species, dolphin,
    equip::Slot,
    finalize::{self, set_drop_chance, set_item_stack},
    fox,
};
use sunrise_core::geometry::Vec3;

/// `Fox.ALLOWED_ITEMS` and the looting block of `Mob.aiStep` for foxes, once a tick.
pub fn tick_foxes(level: &mut impl MobLevel) {
    let griefing = level.mob_griefing();
    let mut near = ItemAreas::default();
    for i in 0..level.parts().mobs.len() {
        let (id, reach, can_loot) = {
            let p = level.parts();
            let m = &p.mobs[i];
            if m.species() != Species::Fox || !m.alive() || m.removed || !p.ticked.contains(&m.id) {
                continue;
            }
            // `Mob.aiStep`: the items in the box grown by (1, 0, 1), in order.
            (
                m.id,
                m.bounds().inflate(Vec3::new(1.0, 0.0, 1.0)),
                m.can_pick_up_loot(),
            )
        };
        if can_loot && level.entity_grief(id, griefing) && {
            let p = level.parts();
            near.may_meet(p.dropped, p.next_entity, reach)
        } {
            let mut j = 0;
            while j < level.parts().dropped.len() {
                let p = level.parts();
                let e = &p.dropped[j];
                if e.pickup_delay > 0 || e.stack.count == 0 || !reach.intersects(e.bounds()) {
                    j += 1;
                    continue;
                }
                let item_food = fox_food(p.items, &e.stack.item);
                let held_food = p.mobs[i]
                    .extra
                    .main_hand_id()
                    .is_some_and(|held| fox_food(p.items, held));
                if !fox::can_hold_item(&p.mobs[i], item_food, held_food) {
                    j += 1;
                    continue;
                }
                // The entity is discarded: the next one is at `j` now.
                fox_pick_up(level, i, j);
            }
        }
        // `FoxSearchForItemsGoal`: the first allowed item within 8 blocks.
        let p = level.parts();
        let area = p.mobs[i].bounds().inflate(Vec3::new(8.0, 8.0, 8.0));
        let at = if near.may_meet(p.dropped, p.next_entity, area) {
            p.dropped
                .iter()
                .find(|e| e.pickup_delay == 0 && e.health > 0 && area.intersects(e.bounds()))
                .map(|e| [e.position.x, e.position.y, e.position.z])
        } else {
            None
        };
        fox::set_item_at(&mut p.mobs[i], at);
    }
}

/// `Dolphin.pickUpItem` in `Mob.aiStep`'s looting block, and the item
/// `Dolphin.MoveToItemGoal` swims to (`Dolphin.ALLOWED_ITEMS`: no pickup delay, alive, in water).
pub fn tick_dolphins(level: &mut impl MobLevel) {
    let griefing = level.mob_griefing();
    let mut near = ItemAreas::default();
    for i in 0..level.parts().mobs.len() {
        let (id, reach, can_loot) = {
            let p = level.parts();
            let m = &p.mobs[i];
            if m.species() != Species::Dolphin
                || !m.alive()
                || m.removed
                || !p.ticked.contains(&m.id)
            {
                continue;
            }
            // `Mob.aiStep`: the items in the box grown by (1, 0, 1); `Dolphin.pickUpItem`
            // takes the whole stack when the mouth is empty (`Mob.canHoldItem` is true).
            (
                m.id,
                m.bounds().inflate(Vec3::new(1.0, 0.0, 1.0)),
                m.can_pick_up_loot(),
            )
        };
        if can_loot && level.entity_grief(id, griefing) && {
            let p = level.parts();
            near.may_meet(p.dropped, p.next_entity, reach)
        } {
            let mut j = 0;
            while j < level.parts().dropped.len() {
                let p = level.parts();
                let e = &p.dropped[j];
                if e.pickup_delay > 0
                    || e.stack.count == 0
                    || !reach.intersects(e.bounds())
                    || p.mobs[i].extra.main_hand_id().is_some()
                {
                    j += 1;
                    continue;
                }
                let before = p.dropped[j].clone();
                level.on_item_pickup(i, &before);
                let m = &mut level.parts().mobs[i];
                set_item_stack(m, Slot::MainHand.index(), before.stack.clone());
                set_drop_chance(m, Slot::MainHand.index(), 2.0);
                let id = m.id;
                level.take(id, before);
                level.discard_item(j);
            }
        }
        let p = level.parts();
        let area = p.mobs[i].bounds().inflate(Vec3::new(8.0, 8.0, 8.0));
        let at = if near.may_meet(p.dropped, p.next_entity, area) {
            p.dropped
                .iter()
                .find(|e| {
                    e.pickup_delay == 0
                        && e.health > 0
                        && area.intersects(e.bounds())
                        && p.facts
                            .get(p.world.block(
                                e.position.x.floor() as i32,
                                e.position.y.floor() as i32,
                                e.position.z.floor() as i32,
                            ))
                            .fluid
                            == sunrise_client::blocks::Fluid::Water
                })
                .map(|e| [e.position.x, e.position.y, e.position.z])
        } else {
            None
        };
        dolphin::set_item_at(&mut p.mobs[i], at);
    }
}

/// `Fox.isConsumableFood`.
fn fox_food(items: &sunrise_client::items::Items, item: &str) -> bool {
    items
        .get(item)
        .is_some_and(|i| i.food.is_some() && i.consumable.is_some())
}

/// `Fox.pickUpItem`: the extra items drop where it stands, what it held is spat out
/// ahead of it, and it holds one of the item.
fn fox_pick_up(level: &mut impl MobLevel, fox_index: usize, item_index: usize) {
    let (before, at) = {
        let p = level.parts();
        let m = &p.mobs[fox_index];
        (
            p.dropped[item_index].clone(),
            [m.position.x, m.position.y, m.position.z],
        )
    };
    let mut stack = before.stack.clone();
    let count = stack.count;
    // `dropItemStack(itemStack.split(count - 1))`: a default ItemEntity at its feet.
    if count > 1 {
        let mut rest = stack.clone();
        rest.count = count - 1;
        stack.count = 1;
        level.spawn_item_own(rest, at, 10);
    }
    // `spitOutItem(getItemBySlot(MAINHAND))`.
    let (held, look, fox_id) = {
        let m = &level.parts().mobs[fox_index];
        (
            finalize::item_stack(m, Slot::MainHand.index()),
            m.view_vector(),
            m.id,
        )
    };
    if let Some(held) = held {
        let from = [at[0] + look[0], at[1] + 1.0, at[2] + look[2]];
        level.spawn_item_own(held, from, 40);
        let id = level.mob_uuid(fox_id);
        if let Some(thrown) = level.just_spawned_item() {
            thrown.thrower = Some(id);
        }
        level.play_sound("minecraft:entity.fox.spit", at, 1.0, 1.0);
    }
    // `onItemPickup`, then `setItemSlot(MAINHAND, split(1))`, `setGuaranteedDrop`.
    level.on_item_pickup(fox_index, &before);
    let m = &mut level.parts().mobs[fox_index];
    set_item_stack(m, Slot::MainHand.index(), stack);
    set_drop_chance(m, Slot::MainHand.index(), 2.0);
    fox::took_item(m);
    let id = m.id;
    // `take(entity, count)` and `entity.discard()`.
    level.take(id, before);
    level.discard_item(item_index);
}
