#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::storage_common::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    containers::{BlockEntity, is_chest_block},
    items::ItemStack,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::random::RandomSource;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::chests::{
    chest_connected_pos, is_shulker_box,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::chests::{chest_half, facing_step};

impl Game<'_, crate::game::server_side::ServerState> {
    /// Closing it copies the items back and keeps the block entity, as vanilla's
    /// `EnderChestBlockEntity` persists across open/close (`close_ender_chest`
    /// must not remove it, or `/data get block` loses it after a use).
    pub(in crate::game::server_side) fn close_ender_chest(&mut self) {
        let Some(key) = self.side.ender_open.take() else {
            return;
        };
        // The items go back to the player: an `EnderChestBlockEntity` holds none, so
        // nothing of theirs stays in the chunk or in `/data get block`.
        if let Some(BlockEntity::Chest(items)) = self.world.block_entities.get_mut(&key) {
            self.side.ender_items = std::mem::replace(items, vec![None; 27]);
        }
    }
    /// Opening an ender chest shows the player's ender inventory in its place.
    pub(in crate::game::server_side) fn open_ender_chest(&mut self, key: (i32, i32, i32)) {
        self.world
            .block_entities
            .insert(key, BlockEntity::Chest(self.side.ender_items.clone()));
        self.container = Some(key);
        self.side.ender_open = Some(key);
    }
    /// `ServerPlayerGameMode.useItemOn` for a spectator: the block's menu
    /// (`BlockState.getMenuProvider`) opens, and nothing else happens. The
    /// lid, its sound and the `container_open` game event come from
    /// `ContainerOpenersCounter`, which spectators never join
    /// (`ChestBlockEntity.startOpen`); the stat, the piglins' anger and the
    /// shulker box's room check are `useWithoutItem`'s, which is not called.
    /// Only the block containers whose clicks the server takes back from a
    /// spectator (`handleContainerClick`) open here: storage, the furnaces,
    /// the brewing stand and the crafter.
    pub(in crate::game) fn spectator_open_menu(&mut self, pos: [i32; 3]) {
        if self.fake_blocks(mod_api::FakeEffect::Menu) {
            return;
        }
        let key = (pos[0], pos[1], pos[2]);
        let name = self
            .res
            .facts()
            .get(self.world.block(pos[0], pos[1], pos[2]))
            .name
            .clone();
        let container = is_shulker_box(&name)
            || matches!(
                name.trim_start_matches("minecraft:"),
                "chest"
                    | "trapped_chest"
                    | "barrel"
                    | "hopper"
                    | "dispenser"
                    | "dropper"
                    | "furnace"
                    | "blast_furnace"
                    | "smoker"
                    | "brewing_stand"
                    | "crafter"
            );
        // ChestBlock.getMenuProvider: none while either half is blocked.
        if !container || self.chest_menu_blocked(key, &name) {
            return;
        }
        // BaseContainerBlockEntity.canOpen: a spectator opens no container that still
        // has a loot table to unpack (it clears while it rolls, on first open).
        // ServerPlayer.openMenu sends the refused spectator a red action bar.
        if self
            .world
            .chunks
            .get(&(key.0 >> 4, key.2 >> 4))
            .and_then(|c| c.saved.as_ref())
            .is_some_and(|s| s.pending_loot.contains_key(&key))
        {
            self.server_send_overlay_message(
                Text::translate("container.spectatorCantOpen", &[])
                    .with_color(sunrise_client::text::RED),
            );
            return;
        }
        let Some(entity) = BlockEntity::for_block(&name) else {
            return;
        };
        self.world.block_entities.get_or_insert_with(key, || entity);
        self.container = Some(key);
        self.side.open_container_slots = Some((key, self.open_container_size()));
        self.server_set_acting_use(false);
    }
    /// `Container.stillValidBlockEntity`: the open block container closes once
    /// the player's eyes are out of reach of any of its blocks (a 4-block buffer).
    pub(in crate::game) fn tick_container_reach(&mut self) {
        // A server's menu ends when the server closes it.
        if !self.is_logical_server() {
            return;
        }
        let Some(eye) = self.eye_position() else {
            return;
        };
        if self
            .container
            .is_none_or(|k| k.1 == super::minecarts::CART_KEY_Y)
        {
            return;
        }
        // `Container.stillValidBlockEntity`: the menu holds a block entity per
        // part, and a part that went away (`getBlockEntity(pos) != this`) ends
        // it. A double chest whose other half broke would otherwise show a
        // shorter container.
        if self
            .side
            .open_container_slots
            .is_some_and(|(open, slots)| {
                self.container == Some(open) && self.open_container_size() < slots
            })
        {
            self.close_container_server();
            return;
        }
        let range = self.reach() + 4.0;
        let near = self.container_parts().iter().all(|&((x, y, z), _)| {
            sunrise_client::lan::guest::within_block_range(eye, range, [x, y, z])
        });
        if !near {
            self.close_container_server();
        }
    }
    /// `CraftingMenu.stillValid`: the open crafting table closes once its
    /// block is no longer a crafting table or the player's eyes are out of
    /// reach (a 4-block buffer).
    pub(in crate::game) fn tick_craft_table(&mut self) {
        // A server's menu ends when the server closes it.
        if !self.is_logical_server() || self.craft_size != 3 {
            return;
        }
        let (Some(pos), Some(eye)) = (self.side.craft_table, self.eye_position()) else {
            return;
        };
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let table = &*self.res.facts().get(state).name == "minecraft:crafting_table";
        let range = self.reach() + 4.0;
        if !(table && sunrise_client::lan::guest::within_block_range(eye, range, pos)) {
            self.close_container_server();
        }
    }

    /// `BaseContainerBlockEntity.createMenu` (and `ChestBlock`'s provider for
    /// a double chest, which asks both halves' `canOpen`): a `lock` the opener
    /// cannot unlock refuses the menu with `sendChestLockedNotifications` -
    /// the action bar names the container and one chest-locked sound plays.
    /// True when the open is refused.
    pub(in crate::game) fn refuse_locked_container(
        &mut self,
        key: (i32, i32, i32),
        name: &str,
    ) -> bool {
        // A carrier transfer fence holds this cell: its block entity's
        // container does not open while the operation runs.
        if self.world.carrier_fences.locks_cell(&[key.0, key.1, key.2]) {
            return true;
        }
        let Some(default_key) = sunrise_client::containers::default_name_key(name) else {
            return false;
        };
        // `LockCode.canUnlock` reads `Player.getMainHandItem`, whichever
        // hand the click came from.
        let hand = self.stack_in(self.selected).cloned();
        let spectator = self.spectator();
        // The halves in `DoubleBlockCombiner` order: the right half first.
        let halves = match self.chest_partner(key) {
            Some((other, true)) => [Some(key), Some(other)],
            Some((other, false)) => [Some(other), Some(key)],
            None => [Some(key), None],
        };
        if !halves
            .iter()
            .flatten()
            .any(|&half| self.container_locked_for(half, spectator, hand.as_ref()))
        {
            return false;
        }
        let title = match (halves[0], halves[1]) {
            (Some(first), Some(second)) => sunrise_client::containers::double_chest_name(
                self.block_name_plain(first).as_deref(),
                self.block_name_plain(second).as_deref(),
            ),
            (Some(one), None) => sunrise_client::containers::display_name(
                self.block_name_plain(one).as_deref(),
                default_key,
            ),
            _ => sunrise_client::containers::display_name(None, default_key),
        };
        let title = sunrise_client::component::name_text(&title);
        self.server_send_overlay_message(Text::translate("container.isLocked", &[title]));
        let at = match (halves[0], halves[1]) {
            (Some(first), Some(second)) => [
                f64::from(first.0 + second.0) / 2.0 + 0.5,
                f64::from(first.1) + 0.5,
                f64::from(first.2 + second.2) / 2.0 + 0.5,
            ],
            (Some(one), None) => [
                f64::from(one.0) + 0.5,
                f64::from(one.1) + 0.5,
                f64::from(one.2) + 0.5,
            ],
            _ => [0.0; 3],
        };
        self.server_play_sound("minecraft:block.chest.locked", at, 1.0, 1.0);
        true
    }

    /// The block above `key` is a redstone conductor.
    pub(in crate::game::server_side) fn conductor_above(&self, key: (i32, i32, i32)) -> bool {
        let above = self.world.block(key.0, key.1 + 1, key.2);
        self.res.behaviours().facts(above).flags
            & sunrise_world::level::data::state_flags::CONDUCTOR
            != 0
    }

    /// `ChestBlock.isChestBlockedAt`: a redstone conductor above the chest or
    /// a cat sitting on it.
    pub(in crate::game::server_side) fn chest_blocked_at(&self, key: (i32, i32, i32)) -> bool {
        self.conductor_above(key) || super::machines::cat_sitting_above(&self.mobs, key)
    }

    /// `ChestBlock.getMenuProvider` (`combine(.., false)`) and
    /// `EnderChestBlock.useWithoutItem`: a blocked chest, or a double chest
    /// with either half blocked, opens no menu.
    pub(in crate::game) fn chest_menu_blocked(&self, key: (i32, i32, i32), name: &str) -> bool {
        match name {
            n if is_chest_block(n) => {
                self.chest_blocked_at(key)
                    || self
                        .chest_partner(key)
                        .is_some_and(|(other, _)| self.chest_blocked_at(other))
            }
            "minecraft:ender_chest" => self.conductor_above(key),
            _ => false,
        }
    }

    /// `ChestBlockEntity.playSound`: resolve the sound for the first or last opener.
    pub(in crate::game) fn resolve_container_sound(
        &self,
        key: (i32, i32, i32),
        name: &str,
        open: bool,
    ) -> Option<(&'static str, [f64; 3])> {
        let chest = is_chest_block(name);
        let sound = if is_shulker_box(name) {
            if open {
                "minecraft:block.shulker_box.open"
            } else {
                "minecraft:block.shulker_box.close"
            }
        } else if name == "minecraft:ender_chest" {
            if open {
                "minecraft:block.ender_chest.open"
            } else {
                "minecraft:block.ender_chest.close"
            }
        } else if chest {
            sunrise_client::mobs::transport_items::chest_hinge_sound(name, open)
        } else if name == "minecraft:barrel" {
            if open {
                "minecraft:block.barrel.open"
            } else {
                "minecraft:block.barrel.close"
            }
        } else {
            return None;
        };
        let mut at = [
            f64::from(key.0) + 0.5,
            f64::from(key.1) + 0.5,
            f64::from(key.2) + 0.5,
        ];
        if chest && let Some((other, _)) = self.chest_partner(key) {
            at[0] = f64::from(key.0 + other.0) / 2.0 + 0.5;
            at[2] = f64::from(key.2 + other.2) / 2.0 + 0.5;
        } else if name == "minecraft:barrel"
            && let Some(state) = self
                .pack
                .assets
                .registry
                .state(self.world.block(key.0, key.1, key.2))
            && let Some(facing) = state.properties.get("facing")
        {
            let (dx, dy, dz) = match facing.as_str() {
                "up" => (0.0, 0.5, 0.0),
                "down" => (0.0, -0.5, 0.0),
                f => {
                    let (x, z) = facing_step(f);
                    (f64::from(x) / 2.0, 0.0, f64::from(z) / 2.0)
                }
            };
            at = [at[0] + dx, at[1] + dy, at[2] + dz];
        }
        Some((sound, at))
    }

    pub(in crate::game) fn play_container_sound(&mut self, sound: &str, at: [f64; 3]) {
        let pitch = self.world.random.next_float() * 0.1 + 0.9;
        self.server_play_sound(sound, at, 0.5, pitch);
    }

    /// Play the current block's sound when its opener count crosses zero
    /// (`ContainerOpenersCounter.onOpen` / `onClose`).
    pub(in crate::game) fn container_edge_sound(&mut self, key: (i32, i32, i32), open: bool) {
        let state = self.world.block(key.0, key.1, key.2);
        let name = &self.res.facts().get(state).name;
        if let Some((sound, at)) = self.resolve_container_sound(key, name, open) {
            self.play_container_sound(sound, at);
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    /// `ChestBlockEntity`'s opener counter plays the block's own hinge
    /// sound: a copper chest's, by weather state, not the wooden chest's.
    #[test]
    #[ignore = "daily"]
    fn a_player_opens_and_closes_a_chest_with_its_own_hinge_sound() {
        let mut resources = None;
        let mut game = super::super::game_tests::tests::server_game(&mut resources);
        game.side.audio_capture_active = true;
        for (block, sound) in [
            ("minecraft:chest", "minecraft:block.chest"),
            (
                "minecraft:exposed_copper_chest",
                "minecraft:block.copper_chest",
            ),
            (
                "minecraft:waxed_oxidized_copper_chest",
                "minecraft:block.copper_chest_oxidized",
            ),
        ] {
            let state = game.pack.assets.registry.default_state(block).unwrap();
            game.world.set(8, 64, 8, state).unwrap();
            game.side.pending_sounds.clear();
            game.container_edge_sound((8, 64, 8), true);
            game.container_edge_sound((8, 64, 8), false);
            let mut heard: Vec<_> = game
                .side
                .pending_sounds
                .iter()
                .map(|s| s.event.clone())
                .collect();
            heard.sort();
            assert_eq!(
                heard,
                [format!("{sound}.close"), format!("{sound}.open")],
                "{block}"
            );
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod reach_tests {
    use super::super::game_tests::tests::server_game;
    use sunrise_client::{containers::BlockEntity, motion::PlayerMotion};
    use sunrise_core::geometry::Vec3;

    /// `Container.stillValidBlockEntity` for a double chest (`CompoundContainer`
    /// asks both halves): breaking the half the player did not click ends the
    /// six-row menu instead of leaving a three-row one.
    #[test]
    #[ignore = "daily"]
    fn breaking_the_other_half_closes_the_open_double_chest() {
        let mut resources = None;
        let mut game = server_game(&mut resources);
        game.player = Some(
            PlayerMotion::new(game.motion, Vec3::new(8.5, 64.0, 8.5)).expect("a player position"),
        );
        let registry = game.pack.assets.registry;
        let half = |kind: &str| {
            let mut state = registry
                .state(registry.default_state("minecraft:chest").unwrap())
                .unwrap()
                .clone();
            state.properties.insert("type".into(), kind.into());
            state.properties.insert("facing".into(), "north".into());
            registry.id(&state).unwrap()
        };
        let left = (8, 64, 8);
        let right = (9, 64, 8);
        game.world
            .set(left.0, left.1, left.2, half("left"))
            .unwrap();
        game.world
            .set(right.0, right.1, right.2, half("right"))
            .unwrap();
        for key in [left, right] {
            game.world
                .block_entities
                .insert(key, BlockEntity::Chest(vec![None; 27]));
        }
        game.open_block_effect_menu([8, 64, 8]);
        assert_eq!(game.container, Some(left), "the double chest opened");

        game.destroy_block_dropping([9, 64, 8])
            .expect("the other half breaks");
        game.tick_container_reach();
        assert_eq!(
            game.container, None,
            "the open double chest kept a menu after its other half broke"
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod lock_tests {
    use super::super::game_tests::tests::server_game;
    use sunrise_client::{containers::BlockEntity, items::ItemStack};
    use sunrise_core::nbt::Tag;
    use sunrise_protocol::packets::play::Clientbound;

    fn has_overlay(game: &crate::game::ServerGame<'_>) -> bool {
        game.side
            .guest_packets
            .iter()
            .any(|packet| matches!(packet, Clientbound::SystemChat(chat) if chat.overlay))
    }

    /// `BaseContainerBlockEntity.canOpen` (`LockCode`): a `lock` whose item
    /// predicate the opener's main hand does not match refuses the menu at
    /// `createMenu`, so the block is never set as the open container. A hand
    /// holding the lock's item opens it.
    #[test]
    #[ignore = "daily"]
    fn a_locked_container_refuses_the_local_players_open() {
        let mut resources = None;
        let mut game = server_game(&mut resources);
        game.options.dedicated = None;
        let state = game
            .pack
            .assets
            .registry
            .default_state("minecraft:chest")
            .unwrap();
        let key = (8, 64, 8);
        game.world.set(key.0, key.1, key.2, state).unwrap();
        game.world
            .block_entities
            .insert(key, BlockEntity::Chest(vec![None; 27]));
        let lock = Tag::compound([(
            "lock",
            Tag::compound([("items", Tag::String("minecraft:gold_ingot".into()))]),
        )]);
        game.world.set_block_lock(key, &lock);

        // An empty hand does not match `lock:{items:"minecraft:gold_ingot"}`.
        game.side.guest_packets.clear();
        game.open_block_effect_menu([8, 64, 8]);
        assert_eq!(game.container, None, "a locked chest opened");
        assert!(has_overlay(&game), "no `container.isLocked` action bar");

        // `LockCode.canUnlock` reads the main hand: the key in the off hand
        // does not open it.
        game.inventory[0] = None;
        game.offhand = Some(ItemStack::new("minecraft:gold_ingot", 1));
        game.side.guest_packets.clear();
        game.open_block_effect_menu([8, 64, 8]);
        assert_eq!(
            game.container, None,
            "an off-hand key opened a locked chest"
        );

        // The lock's own item in the main hand opens it.
        game.inventory[0] = Some(ItemStack::new("minecraft:gold_ingot", 1));
        game.offhand = None;
        game.selected = 0;
        game.side.guest_packets.clear();
        game.open_block_effect_menu([8, 64, 8]);
        assert_eq!(game.container, Some(key), "the lock's item did not open it");
        assert!(!has_overlay(&game));
    }

    /// `ChestBlock.MENU_PROVIDER_COMBINER.createMenu`: a double chest asks
    /// `canOpen` of both halves, so a lock on the half the player did not
    /// click still refuses the six-row menu.
    #[test]
    #[ignore = "daily"]
    fn a_lock_on_either_half_refuses_a_double_chest() {
        let mut resources = None;
        let mut game = server_game(&mut resources);
        game.options.dedicated = None;
        let registry = game.pack.assets.registry;
        let half = |kind: &str| {
            let mut state = registry
                .state(registry.default_state("minecraft:chest").unwrap())
                .unwrap()
                .clone();
            state.properties.insert("type".into(), kind.into());
            state.properties.insert("facing".into(), "north".into());
            registry.id(&state).unwrap()
        };
        let left_key = (8, 64, 8);
        let right_key = (9, 64, 8);
        game.world
            .set(left_key.0, left_key.1, left_key.2, half("left"))
            .unwrap();
        game.world
            .set(right_key.0, right_key.1, right_key.2, half("right"))
            .unwrap();
        for key in [left_key, right_key] {
            game.world
                .block_entities
                .insert(key, BlockEntity::Chest(vec![None; 27]));
        }
        // The right half holds the lock; the player clicks the left one.
        let lock = Tag::compound([(
            "lock",
            Tag::compound([("items", Tag::String("minecraft:gold_ingot".into()))]),
        )]);
        game.world.set_block_lock(right_key, &lock);

        game.side.guest_packets.clear();
        game.open_block_effect_menu([8, 64, 8]);
        assert_eq!(game.container, None, "a half-locked double chest opened");
        assert!(has_overlay(&game));
    }
}
