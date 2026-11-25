//! Common `OminousItemSpawner` insertion shared by client packets and server tests.
use super::{Game, ominous_item_spawner::OminousItemSpawner};
use sunrise_client::items::ItemStack;

impl Game<'_> {
    /// `handleAddEntity` for a spawner a server sent: its item comes with the
    /// entity data (`ItemStack.EMPTY` until then, which draws nothing).
    pub(in crate::game) fn add_remote_ominous_item_spawner(&mut self, id: u32, position: [f64; 3]) {
        self.ominous_spawners.push(OminousItemSpawner {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id,
            item: ItemStack::new("minecraft:air", 1),
            position,
            spawn_item_after: 0,
            tick_count: 0,
            fire_ticks: 0,
            synced_fire: false,
            has_visual_fire: false,
            uuid: [0; 4],
            raw: None,
        });
    }
}
