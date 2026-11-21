use super::*;

/// A click sent to a server: its slot, button and input, the slots it
/// changed and the carried stack after it.
pub(super) type Predicted = (
    i16,
    i32,
    Input,
    Vec<(i16, Option<ItemStack>)>,
    Option<ItemStack>,
);

/// How many stacks the creative screen keeps in step with the server (`Remote::synced`).
pub(super) const CREATIVE_SYNCED: usize = 41;

/// The player's stacks in `Remote::synced` order: the 36 inventory slots, the off hand, then
/// the armour from the head down. `CreativeInventoryListener.slotChanged` sends every
/// InventoryMenu slot the creative screen changes, the armour slots too.
pub(super) fn creative_sync_stacks(
    inventory: &[Option<ItemStack>],
    offhand: &Option<ItemStack>,
    armor: &[Option<ItemStack>; 4],
) -> Vec<Option<ItemStack>> {
    let mut stacks: Vec<Option<ItemStack>> = inventory.iter().take(36).cloned().collect();
    stacks.resize(36, None);
    stacks.push(offhand.clone());
    stacks.extend(armor.iter().cloned());
    stacks
}

/// Entries of `creative_sync_stacks` that went from `before` to `now` are synced as `now`.
pub(super) fn mark_synced(
    synced: &mut [Option<ItemStack>],
    before: &[Option<ItemStack>],
    now: Vec<Option<ItemStack>>,
) {
    for (i, stack) in now.into_iter().enumerate() {
        if before.get(i) != Some(&stack)
            && let Some(s) = synced.get_mut(i)
        {
            *s = stack;
        }
    }
}

/// InventoryMenu's slot for entry `i` of `creative_sync_stacks`: the hotbar is 36..=44, the
/// main inventory 9..=35, the off hand 45 and the armour 5..=8 (head first).
pub(super) fn creative_sync_slot(i: usize) -> i16 {
    (match i {
        0..9 => 36 + i,
        36 => 45,
        37.. => 5 + (i - 37),
        _ => i,
    }) as i16
}

/// `MultiPlayerGameMode.handleContainerInput` for each click on the engine's
/// copy of the menu: the click with the slots it changed and the carried
/// stack after it, and whether a click dropped a stack. Every drop of
/// `AbstractContainerMenu.doClick` (a THROW, a click outside the window, a
/// swap or a result move with no room left) is `Prediction.PREDICTED`.
/// None when the engine refuses a click.
pub(super) fn predict_clicks(
    m: &mut sunrise_client::menu::Menu,
    env: &sunrise_client::menu::Env,
    clicks: &[(i16, i32, Input)],
) -> Option<(Vec<Predicted>, bool)> {
    let mut predicted = Vec::with_capacity(clicks.len());
    let mut dropped = false;
    for &(slot, button, input) in clicks {
        let before = menu_items(m);
        let thrown = m.dropped.len();
        m.click(env, i32::from(slot), button, input).ok()?;
        dropped |= m.dropped.len() > thrown;
        let changed = changed_slots(&before, m);
        predicted.push((slot, button, input, changed, m.carried.clone()));
    }
    Some((predicted, dropped))
}

pub(super) fn menu_items(m: &sunrise_client::menu::Menu) -> Vec<Option<ItemStack>> {
    (0..m.slot_count()).map(|i| m.slot(i).cloned()).collect()
}

/// The slots whose stack is no longer `before`'s (`ItemStack.matches`).
/// `MultiPlayerGameMode.handleContainerInput`'s changed slots, in the order the packet writes them.
pub(super) fn changed_slots(
    before: &[Option<ItemStack>],
    m: &sunrise_client::menu::Menu,
) -> Vec<(i16, Option<ItemStack>)> {
    let changed: Vec<i16> = (0..m.slot_count())
        .filter(|&i| m.slot(i) != before.get(i).and_then(Option::as_ref))
        .map(|i| i as i16)
        .collect();
    open_hash_order(&changed)
        .into_iter()
        .map(|i| (i, m.slot(i as usize).cloned()))
        .collect()
}

/// The order a new fastutil `Int2ObjectOpenHashMap` iterates `keys` put in this order: vanilla
/// collects a click's changed slots in one and writes them as it iterates. The key 0 comes first,
/// then the table from its last position down; a key sits at `HashCommon.mix(key) & mask` or the
/// next free position. The table starts at 32 (16 expected, load factor 0.75) and doubles when an
/// insert finds it holding 24 (`size++ >= maxFill`), re-inserting from its last position down.
pub(super) fn open_hash_order(keys: &[i16]) -> Vec<i16> {
    fn mix(key: i16) -> usize {
        let h = i32::from(key).wrapping_mul(0x9E37_79B9_u32 as i32);
        (h ^ ((h as u32) >> 16) as i32) as u32 as usize
    }
    fn put(table: &mut [Option<i16>], key: i16) {
        let mask = table.len() - 1;
        let mut pos = mix(key) & mask;
        while table[pos].is_some() {
            pos = (pos + 1) & mask;
        }
        table[pos] = Some(key);
    }
    // HashCommon.maxFill(n, 0.75): min(ceil(n * 0.75), n - 1).
    let max_fill = |n: usize| (n * 3).div_ceil(4).min(n - 1);
    let mut table = vec![None; 32];
    let (mut zero, mut size) = (false, 0);
    for &key in keys {
        if key == 0 {
            zero = true;
        } else {
            put(&mut table, key);
        }
        size += 1;
        // `size++ >= maxFill`: the size before this key.
        if size > max_fill(table.len()) {
            // HashCommon.arraySize(size + 1, 0.75): the power of two holding size + 1 at that load.
            let n = ((size + 1) * 4).div_ceil(3).next_power_of_two();
            let mut grown = vec![None; n];
            for &key in table.iter().rev().flatten() {
                put(&mut grown, key);
            }
            table = grown;
        }
    }
    let rest = table.iter().rev().flatten().copied();
    zero.then_some(0).into_iter().chain(rest).collect()
}
