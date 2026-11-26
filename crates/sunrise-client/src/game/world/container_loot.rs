//! First-access loot shared by blocks and chest vehicles.
use super::Game;
use crate::game::container_loot_level::{LootTarget, fill_with_random, take_block_pending};
use sunrise_core::nbt::Tag;
use sunrise_level::containers::BlockEntity;
use sunrise_sim::save::PendingContainerLoot;

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game::server_side) fn loot_inventory_item(
        &mut self,
        key: (i32, i32, i32),
        index: usize,
    ) -> Option<sunrise_items::items::ItemStack> {
        self.unpack_loot(LootTarget::Block(key), None);
        self.world
            .block_entities
            .get(&key)?
            .slots()
            .get(index)?
            .clone()
    }
    /// A hopper cart's tick owns its metadata while its resident row is lent out.
    /// Publish only a pending inventory for this access, then recover the result.
    pub(in crate::game::server_side) fn unpack_ticked_cart_loot(
        &mut self,
        cart: &mut super::minecarts::Minecart,
    ) -> bool {
        if cart.loot.is_none() {
            return true;
        }
        let Some(i) = self
            .carts
            .iter()
            .position(|c| c.id == cart.id && c.join == cart.join)
        else {
            return false;
        };
        self.carts.update(i, |owner| {
            owner.loot = cart.loot.take();
            owner.items.clone_from(&cart.items);
            owner.position = cart.position;
        });
        self.unpack_loot(LootTarget::Cart(cart.id), None);
        let Some(i) = self
            .carts
            .iter()
            .position(|c| c.id == cart.id && c.join == cart.join)
        else {
            return false;
        };
        self.carts.update(i, |owner| {
            cart.loot = owner.loot.take();
            cart.items.clone_from(&owner.items);
        });
        true
    }
    /// Consumes metadata before callbacks, so reentrant access cannot roll twice.
    /// A player context is captured from the admitted actor, also during as_guest.
    pub(in crate::game) fn unpack_loot(
        &mut self,
        target: LootTarget,
        player: Option<mod_api::PlayerRef>,
    ) {
        let (pending, pos) = match target {
            LootTarget::Block(key) => {
                let Some(pending) = take_block_pending(&mut self.world, key) else {
                    return;
                };
                // Keep the block-container rule: a table coexisting with items is dropped.
                if self.world.block_entities[&key]
                    .slots()
                    .iter()
                    .any(Option::is_some)
                {
                    return;
                }
                (pending, key)
            }
            LootTarget::Cart(id) => {
                let Some(i) = self.carts.iter().position(|c| c.id == id) else {
                    return;
                };
                let Some(pending) = self.carts.update(i, |c| c.loot.take()) else {
                    return;
                };
                self.store_cart_items((i32::MAX - 15, super::minecarts::CART_KEY_Y, id as i32));
                let c = &self.carts[i];
                (
                    pending,
                    (
                        c.position.x.floor() as i32,
                        c.position.y.floor() as i32,
                        c.position.z.floor() as i32,
                    ),
                )
            }
            LootTarget::Boat(id) => {
                let Some(i) = self.boats.iter().position(|b| b.id == id) else {
                    return;
                };
                let Some(pending) = self
                    .persist
                    .raw
                    .get(&id)
                    .and_then(PendingContainerLoot::from_tag)
                else {
                    return;
                };
                if let Some(Tag::Compound(map)) = self.persist.raw.get_mut(&id) {
                    map.remove(&"LootTable".into());
                    map.remove(&"LootTableSeed".into());
                }
                self.store_cart_items((i32::MAX - 15, super::minecarts::CART_KEY_Y, id as i32));
                let b = &self.boats[i];
                (
                    pending,
                    (
                        b.position.x.floor() as i32,
                        b.position.y.floor() as i32,
                        b.position.z.floor() as i32,
                    ),
                )
            }
        };
        // ContainerEntity unpacks the table even when some vehicle slots are taken.
        // LootTable.fill chooses only the free slots.
        let table = pending.table_id();
        if player.is_some() {
            self.trigger_loot_table(&table);
        }
        let Some(loot) = self.side.server_loot.0.get() else {
            return;
        };
        let biome = self
            .biome_name_at(super::Vec3::new(pos.0 as f64, pos.1 as f64, pos.2 as f64))
            .unwrap_or_default();
        let mut entity = player.map(|_| self.killer_loot_entity(None));
        if let Some(entity) = &mut entity {
            entity.on_fire = self
                .player
                .as_ref()
                .is_some_and(sunrise_sim::motion::PlayerMotion::is_on_fire);
        }
        let luck = if player.is_some() {
            self.player
                .as_mut()
                .and_then(|p| p.attribute("minecraft:luck").ok())
                .unwrap_or(0.0) as f32
        } else {
            0.0
        };
        let context = sunrise_world::loot::ContainerLootContext {
            biome: Some(&biome),
            luck,
            entity: entity.as_ref(),
        };
        let Some(mut slots) = (match target {
            LootTarget::Block(key) => self
                .world
                .block_entities
                .get(&key)
                .map(|e| e.slots().to_vec()),
            LootTarget::Cart(id) => self
                .carts
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.items.clone()),
            LootTarget::Boat(id) => self
                .boats
                .iter()
                .find(|b| b.id == id)
                .map(|b| b.items.clone()),
        }) else {
            return;
        };
        let origin = match target {
            LootTarget::Block(_) => [pos.0 as f64 + 0.5, pos.1 as f64 + 0.5, pos.2 as f64 + 0.5],
            LootTarget::Cart(id) => {
                let p = self.carts.iter().find(|c| c.id == id).unwrap().position;
                [p.x, p.y, p.z]
            }
            LootTarget::Boat(id) => {
                let p = self.boats.iter().find(|b| b.id == id).unwrap().position;
                [p.x, p.y, p.z]
            }
        };
        let mut random = self.world.random.clone();
        let mut sequences = std::mem::take(&mut self.side.rand_sequences);
        fill_with_random(
            &pending,
            &mut slots,
            &loot,
            &mut random,
            &mut sequences,
            self.level.seed,
            context,
            |items| self.container_fill_hook(&table, target, origin, player, items),
        );
        self.world.random = random;
        self.side.rand_sequences = sequences;
        match target {
            LootTarget::Block(key) => {
                if let Some(e) = self.world.block_entities.get_mut(&key) {
                    e.replace_slots(&slots);
                }
            }
            LootTarget::Cart(id) => {
                if let Some(i) = self.carts.iter().position(|c| c.id == id) {
                    self.carts.update(i, |c| c.items = slots);
                }
            }
            LootTarget::Boat(id) => {
                if let Some(i) = self.boats.iter().position(|b| b.id == id) {
                    self.boats.update(i, |b| b.items = slots);
                }
            }
        }
        let id = match target {
            LootTarget::Cart(id) | LootTarget::Boat(id) => Some(id),
            _ => None,
        };
        if let Some(id) = id {
            let items = match target {
                LootTarget::Cart(_) => self
                    .carts
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.items.clone()),
                _ => self
                    .boats
                    .iter()
                    .find(|b| b.id == id)
                    .map(|b| b.items.clone()),
            };
            if let (Some(items), Some(BlockEntity::Chest(open))) = (
                items,
                self.world.block_entities.get_mut(&(
                    i32::MAX - 15,
                    super::minecarts::CART_KEY_Y,
                    id as i32,
                )),
            ) {
                *open = items;
            }
        }
    }
}

#[cfg(test)]
#[path = "container_loot/tests.rs"]
mod tests;
