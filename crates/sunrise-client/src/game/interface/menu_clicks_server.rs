#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::menu_clicks::*;

impl Game<'_> {
    pub(in crate::game) fn click_inventory(&mut self, right: bool) -> bool {
        if self.mod_browser_press(right) || self.mount_click(right) {
            return true;
        }
        if self.recipe_book_click(right) {
            return true;
        }
        if self.station_click(right) {
            return true;
        }
        if self.remote.is_some() {
            return self.remote_click(right);
        }
        let Some(slot) = self.hovered_inventory_slot() else {
            return false;
        };
        let held_shift = self.has_shift_down();
        if self.crafter_click(slot, held_shift, right) {
            return true;
        }
        if held_shift && self.container.is_some() && self.container_quick_move(slot) {
            if let Some(key) = self.container {
                self.world.modified.insert((key.0 >> 4, key.2 >> 4));
            }
            return true;
        }
        if let Some(c) = &self.carried
            && self.refuses(slot, c)
        {
            return true;
        }
        let max = |items: &Items, s: &ItemStack| items.get(&s.item).map_or(64, |i| i.max_stack);
        let shift = held_shift;
        if slot >= 300
            && let Some(key) = self.container
        {
            self.world.modified.insert((key.0 >> 4, key.2 >> 4));
        }
        let is_furnace = matches!(self.container_kind(), Some(BlockEntity::Furnace(_)));
        if is_furnace && slot == 302 {
            // FurnaceResultSlot: take only.
            let Some(result) = self.slot_mut(302).take() else {
                return true;
            };
            match &mut self.carried {
                None => self.carried = Some(result),
                Some(c)
                    if c.item == result.item
                        && c.count + result.count <= max(world_items!(self), c) =>
                {
                    c.count += result.count
                }
                _ => *self.slot_mut(302) = Some(result),
            }
            return true;
        }
        // Changing the input restarts cooking (AbstractFurnaceBlockEntity.setItem).
        let furnace_input = (is_furnace && slot == 300).then(|| self.slot_mut(300).clone());
        if slot == 200 {
            // ResultSlot: take the crafted stack and consume one of each ingredient.
            let mut crafted = 0;
            while let Some((recipe, result)) = self.craft_recipe() {
                let made = (result.item.clone(), result.count);
                let taken = result.clone();
                let cells = self.craft_size * self.craft_size;
                let ingredients = self.craft_grid[..cells].to_vec();
                if shift {
                    if self.add_stack_to_inventory(result).is_some() {
                        break;
                    }
                } else {
                    match &mut self.carried {
                        None => self.carried = Some(result),
                        Some(c)
                            if c.item == result.item
                                && c.count + result.count <= max(world_items!(self), c) =>
                        {
                            c.count += result.count
                        }
                        _ => return true,
                    }
                }
                let remainders = self.craft_grid_remainders(&recipe);
                for cell in self.craft_grid.iter_mut() {
                    if let Some(s) = cell {
                        s.count -= 1;
                        if s.count == 0 {
                            *cell = None;
                        }
                    }
                }
                for (i, rest) in remainders.into_iter().enumerate() {
                    let Some(rest) = rest else { continue };
                    match &mut self.craft_grid[i] {
                        cell @ None => *cell = Some(rest),
                        Some(c) if c.same_item_same_components(&rest) => c.count += rest.count,
                        _ => self.give_or_drop(rest),
                    }
                }
                crafted += 1;
                // `ResultSlot.onTake` (native_item_crafted): recipe, stack taken, grid before.
                let player = self.native_acting_id();
                let id = sunrise_client::crafting::qualified(&recipe);
                self.native_item_crafted(player, &id, &taken, &ingredients);
                self.tutorial_get_item(Some(&taken));
                self.award_stat("crafted", &made.0, made.1 as i32);
                self.award_crafted(&[(id, ingredients)]);
                if !shift || crafted >= 64 {
                    break;
                }
            }
            return true;
        }
        if shift
            && (slot < 36 || (500..504).contains(&slot) || (100..109).contains(&slot))
            && self.inventory_quick_move(slot)
        {
            return true;
        }
        if (500..504).contains(&slot) {
            let wanted = ["head", "chest", "legs", "feet"][slot - 500];
            let fits = self.carried.as_ref().is_none_or(|c| {
                self.res
                    .items()
                    .get(&c.item)
                    .and_then(|i| i.equip_slot.as_deref())
                    == Some(wanted)
            });
            if !fits {
                return true;
            }
            std::mem::swap(&mut self.carried, &mut self.armor[slot - 500]);
            self.apply_equipment();
            return true;
        }
        let carried = self.carried.take();
        let current = self.slot_mut(slot).take();
        let items = self.items();
        let (new_slot, new_carried) = match (carried, current) {
            (None, Some(mut s)) if right => {
                let half = s.count.div_ceil(2);
                s.count -= half;
                let taken = ItemStack {
                    count: half,
                    ..s.clone()
                };
                ((s.count > 0).then_some(s), Some(taken))
            }
            (None, s) => (None, s),
            (Some(mut c), None) if right => {
                let one = ItemStack {
                    count: 1,
                    ..c.clone()
                };
                c.count -= 1;
                (Some(one), (c.count > 0).then_some(c))
            }
            (Some(c), None) => (Some(c), None),
            (Some(mut c), Some(mut s)) if c.same_item_same_components(&s) => {
                let room = max(items, &s).saturating_sub(s.count);
                let moved = if right {
                    1.min(room).min(c.count)
                } else {
                    room.min(c.count)
                };
                s.count += moved;
                c.count -= moved;
                (Some(s), (c.count > 0).then_some(c))
            }
            (Some(c), Some(s)) => (Some(c), Some(s)),
        };
        self.carried = new_carried;
        *self.slot_mut(slot) = new_slot;
        if let Some(before) = furnace_input
            && let Some(BlockEntity::Furnace(f)) = self
                .container
                .and_then(|k| self.world.block_entities.get_mut(&k))
        {
            f.input_clicked(&self.loop_context, world_recipes!(self), before);
        }
        true
    }
    #[inline(always)]
    pub(in crate::game) fn creative_display_click_server(
        &mut self,
        shown: Option<&ItemStack>,
        button: i32,
        input: sunrise_client::menu::Input,
    ) -> sunrise_client::creative::DisplayClick {
        let catalog = world_items!(self);
        let max = |s: &ItemStack| catalog.get(&s.item).map_or(64, |i| i.max_stack);
        sunrise_client::creative::click_display(&mut self.carried, shown, max, button, input)
    }
    /// `CreativeModeInventoryScreen.slotClicked`: THROW while a stack is carried, on an item
    /// tab's hotbar slot or any slot of the Survival Inventory tab. The menu's own THROW needs an
    /// empty hand, so the screen drops from the slot itself: one item, or with the control key
    /// the whole stack (on the inventory tab `Slot.remove(getMaxStackSize())`), through
    /// `handleCreativeModeItemDrop`, then sends the slot (`broadcastChanges`,
    /// `handleCreativeModeItemAdd`); the carried stack stays. Only the server drops it: this is
    /// the client of a server (`remote`).
    pub(in crate::game) fn creative_throw_carrying(&mut self, slot: usize, button: i32) {
        let Some(old) = self
            .slot_mut(slot)
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned()
        else {
            return;
        };
        let count = if button == 0 {
            1
        } else if self.creative_inventory_open() {
            let max = world_items!(self)
                .get(&old.item)
                .map_or(64, |i| i.max_stack);
            old.count.min(max)
        } else {
            old.count
        };
        let before = self.player_equipment();
        let left = old.count - count;
        *self.slot_mut(slot) = (left > 0).then(|| ItemStack {
            count: left,
            ..old.clone()
        });
        self.player_equipment_changed(&before);
        self.apply_equipment();
        self.creative_screen_drop(ItemStack { count, ..old });
        self.send_creative_slots();
    }
    #[inline(always)]
    pub(in crate::game) fn menu_session_with_game_server(&mut self, before: &[Option<ItemStack>]) {
        if let Some(BlockEntity::Furnace(f)) = self
            .container
            .and_then(|k| self.world.block_entities.get_mut(&k))
        {
            // AbstractFurnaceBlockEntity.setItem: only a new input restarts cooking.
            f.input_clicked(&self.loop_context, world_recipes!(self), before[0].clone());
        }
    }
}
