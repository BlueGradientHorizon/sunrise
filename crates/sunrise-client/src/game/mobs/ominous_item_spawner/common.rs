//! Common `OminousItemSpawner` insertion shared by client packets and server tests.
use super::Game;
use sunrise_items::items::ItemStack;

/// `OminousItemSpawnerRenderer`: grows over its first 50 ticks, turning 40
/// degrees a tick.
pub(in crate::game) const TICKS_SCALING: f32 = 50.0;
pub(in crate::game) const ROTATION_SPEED: f32 = 40.0;

pub(in crate::game) struct OminousItemSpawner {
    /// Entity-owned registration metadata; content clones are detached.
    pub(in crate::game) attachment_lifetime: sunrise_mod_runtime::attachments::AttachmentLifetime,
    pub(in crate::game) join: sunrise_sim::join_seq::JoinSeq,
    /// Its entity id: from the entity counter on the server, the server's on
    /// a client.
    pub(in crate::game) id: u32,
    /// `DATA_ITEM`; `minecraft:air` stands for `ItemStack.EMPTY`.
    pub(in crate::game) item: ItemStack,
    pub(in crate::game) position: [f64; 3],
    pub(in crate::game) spawn_item_after: i64,
    pub(in crate::game) tick_count: i64,
    /// `Entity.remainingFireTicks`, counted down by `baseTick`.
    pub(in crate::game) fire_ticks: i32,
    /// `Entity.DATA_SHARED_FLAGS_ID` on-fire bit, set after `baseTick` fire processing.
    pub(in crate::game) synced_fire: bool,
    /// `Entity.hasVisualFire` (`HasVisualFire` in NBT).
    pub(in crate::game) has_visual_fire: bool,
    /// Its UUID, and the compound it was loaded from (fields Sunrise doesn't
    /// model survive a save).
    pub(in crate::game) uuid: [i32; 4],
    pub(in crate::game) raw: Option<Box<sunrise_core::nbt::Tag>>,
}

sunrise_sim::impl_joined!(OminousItemSpawner);
sunrise_mod_runtime::impl_attachment_lifetime!(OminousItemSpawner);

impl<__Side: crate::game::Side> Game<'_, __Side> {
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
