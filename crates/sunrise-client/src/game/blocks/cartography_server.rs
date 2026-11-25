//! Server-side cartography map loading and registration (`MapItem.scaleMap`, `lockMap`).
use super::{Game, cartography::LevelMaps, maps::map_id};
use std::collections::HashMap;
use sunrise_client::items::ItemStack;

impl Game<'_, crate::game::server_side::ServerState> {
    /// The maps of `stacks` (loaded as needed) for a menu over this level.
    pub(in crate::game) fn level_maps<'s>(
        &mut self,
        stacks: impl IntoIterator<Item = &'s ItemStack>,
    ) -> LevelMaps {
        let ids: Vec<i32> = stacks.into_iter().filter_map(map_id).collect();
        let mut states = HashMap::new();
        for id in ids {
            if let Some(m) = self.map(id) {
                states.insert(id, (m.locked, m.scale));
            }
        }
        LevelMaps::new(states, self.peek_map_id())
    }

    /// The scaled and locked copies a menu made (`level.setMapData`), staged if a transaction is open.
    pub(in crate::game) fn register_made_maps(&mut self, maps: LevelMaps) {
        for (id, source, scale) in maps.into_made() {
            let Some(data) = self
                .map(source)
                .map(|m| if scale { m.scaled() } else { m.locked_copy() })
            else {
                continue;
            };
            let new = self.new_map_id(data);
            debug_assert_eq!(new, id, "map ids follow peek_map_id");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::maps::MapData;
    use sunrise_client::items::ItemStack;

    #[test]
    #[ignore = "daily"]
    fn crafting_map_takes_use_real_saved_maps() {
        use sunrise_client::menu::{Env, Input, Kind, Menu};
        crate::game::native_entity_refs::tests::with_game(|game| {
            for mode in [Input::Swap, Input::Pickup, Input::QuickMove] {
                let source = game.new_map_id(MapData::fresh(4.5, 4.5, 0, "minecraft:overworld"));
                let mut input = ItemStack::new("minecraft:filled_map", 1);
                input.data_mut().map_id = Some(source);
                let maps = game.level_maps([&input]);
                let env = Env {
                    maps: Some(&maps),
                    ..game.menu_env()
                };
                let mut menu = Menu::new(Kind::Crafting);
                for i in [1, 2, 3, 4, 6, 7, 8, 9] {
                    menu.set_slot(&env, i, Some(ItemStack::new("minecraft:paper", 1)));
                }
                menu.set_slot(&env, 5, Some(input));
                let preview = menu.slot(0).expect("real scaling recipe admitted");
                assert_eq!(preview.data().and_then(|d| d.map_id), Some(source));
                assert_eq!(preview.data().and_then(|d| d.map_post_processing), Some(1));
                menu.click(&env, 0, if mode == Input::Swap { 2 } else { 0 }, mode)
                    .unwrap();
                let output = match mode {
                    Input::Swap => menu.player_item(2),
                    Input::Pickup => menu.carried.as_ref(),
                    _ => menu.player_item(8),
                }
                .expect("crafted map remains present");
                assert_eq!(output.count, 1);
                assert_eq!(output.data().and_then(|d| d.map_id), Some(source + 1));
                assert_eq!(output.data().and_then(|d| d.map_post_processing), None);
                assert!((1..10).all(|i| menu.slot(i).is_none()));
                game.register_made_maps(maps);
                let original = game.map(source).unwrap();
                assert_eq!((original.scale, original.center), (0, [0, 0]));
                let derived = game.map(source + 1).unwrap();
                assert_eq!((derived.scale, derived.center), (1, [64, 64]));
            }
        });
    }
}
