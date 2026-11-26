use crate::game::{
    Dim, native_menu_data::HeardMenu, stations_common::Kind as StationKind,
    trading_common::MerchantState,
};
use std::collections::{BTreeSet, HashMap};
use sunrise_core::random::LegacyRandom;
use sunrise_gameplay_common::stats::Stats;
use sunrise_items::items::ItemStack;
use sunrise_sim::{
    advancements::{Advancement, Progress},
    lan::guest::Guest,
    menu::{Menu, QuickCraftDrag},
    merchant::{self, Trader},
    mobs::villager::{Ext, ProfessionState},
    recipe_book::Settings,
};

/// The non-storage effects from one guest menu action.
#[derive(Default)]
pub struct GuestMenuEffects {
    pub(in crate::game) dropped: Vec<ItemStack>,
    pub(in crate::game) equipped: Vec<(&'static str, Option<ItemStack>)>,
    pub(in crate::game) brewed: Vec<ItemStack>,
    pub(in crate::game) brewed_potion_takes: Vec<ItemStack>,
    pub(in crate::game) crafted: Vec<(String, Vec<Option<ItemStack>>)>,
    pub(in crate::game) item_crafted: Vec<(String, ItemStack, Vec<Option<ItemStack>>)>,
    pub(in crate::game) crafted_counts: Vec<(std::sync::Arc<str>, i32)>,
}

pub struct FurnaceMenuEffects {
    pub(in crate::game) uuid: u128,
    pub(in crate::game) entity_id: i32,
    pub(in crate::game) pos: (i32, i32, i32),
    pub(in crate::game) at: [f64; 3],
    pub(in crate::game) smelted: Vec<(Option<ItemStack>, u32)>,
    pub(in crate::game) extracted: Option<(ItemStack, u32)>,
}

impl GuestMenuEffects {
    pub fn dropped(dropped: Vec<ItemStack>) -> Self {
        Self {
            dropped,
            ..Self::default()
        }
    }

    pub fn take(menu: &mut Menu) -> Self {
        let dropped = std::mem::take(&mut menu.dropped);
        menu.drops.clear();
        Self {
            dropped,
            equipped: std::mem::take(&mut menu.equipped),
            brewed: std::mem::take(&mut menu.brewed),
            brewed_potion_takes: std::mem::take(&mut menu.brewed_potion_takes),
            crafted: std::mem::take(&mut menu.crafted),
            item_crafted: std::mem::take(&mut menu.item_crafted),
            crafted_counts: std::mem::take(&mut menu.crafted_counts),
        }
    }
}

/// An authoritative click reply, published only once the click's transaction has committed.
pub struct MenuClickReply {
    pub(in crate::game) conn: u32,
    pub(in crate::game) generation: u64,
    pub(in crate::game) reply: ClickReply,
}

pub enum ClickReply {
    Predicted(sunrise_protocol::packets::play::ContainerClick, Vec<usize>),
    Resend(i32),
    Close(i32),
    Inventory,
    Offhand,
}

/// Identity of a mount window across a tentative click and its settlement.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct MountClickWindow {
    pub dim: Dim,
    pub conn: u32,
    pub generation: u64,
    pub window: i32,
    pub mob: u32,
}

/// A guest's open workstation menu.
#[derive(Clone)]
pub struct GuestStation {
    pub window: i32,
    pub state: i32,
    pub kind: StationKind,
    pub pos: [i32; 3],
    pub menu: Box<Menu>,
    /// The data slots the client was last sent. `None` is not sent yet,
    /// which is not a cost: an anvil's signed cost may be `i32::MIN`.
    pub sent_data: Vec<Option<i32>>,
    /// The game time of the menu's last sound (`lastSoundTime`).
    pub last_sound: u64,
}

/// A guest's open trading screen.
#[derive(Clone)]
pub struct GuestMerchant {
    pub window: i32,
    pub state: i32,
    /// The payment slots and the result.
    pub slots: [Option<ItemStack>; 3],
    /// Persistent quick-craft (drag) state across rebuilt merchant menus.
    pub drag: QuickCraftDrag,
    pub merchant: Box<MerchantState>,
    /// The offers the client was last sent.
    pub sent: Vec<merchant::MerchantOffer>,
    pub sent_level: i32,
    pub sent_xp: i32,
    /// The result slot last synchronized to the client (`AbstractContainerMenu.remoteSlots`).
    pub sent_result: Option<ItemStack>,
}

pub struct KeptMerchantGuest {
    pub(in crate::game) merchant: GuestMerchant,
    pub(in crate::game) stats: Option<Stats>,
    pub(in crate::game) progress: Progress,
    pub(in crate::game) changed: BTreeSet<usize>,
    pub(in crate::game) unlocked: Vec<String>,
    pub(in crate::game) book: Settings,
}

pub struct KeptMerchantWindow {
    pub(in crate::game) window: Box<dyn FnOnce(&mut Guest)>,
    pub(in crate::game) heard: Option<HeardMenu>,
    pub(in crate::game) owned: Option<(u64, Box<Menu>)>,
}

pub struct KeptMerchantMob {
    pub(in crate::game) random: LegacyRandom,
    pub(in crate::game) timer: i32,
    pub(in crate::game) trade: Option<Box<Trader>>,
    pub(in crate::game) trading: bool,
    pub(in crate::game) profession: ProfessionState,
    pub(in crate::game) level: i32,
    pub(in crate::game) variant: u8,
    pub(in crate::game) villager: Option<Box<Ext>>,
}

#[derive(Default)]
pub struct Journal {
    pub guests: HashMap<u32, Box<KeptMerchantGuest>>,
    pub windows: HashMap<u32, KeptMerchantWindow>,
    pub mobs: HashMap<u32, Box<KeptMerchantMob>>,
    pub sequences: Option<sunrise_items::random_sequences::RandomSequences>,
    pub active: Option<u32>,
    pub correct_on_abort: std::collections::BTreeSet<(u32, i32)>,
    pub refused_clicks: std::collections::BTreeSet<(u32, i32)>,
}

pub enum Effect {
    Orb([f64; 3], u32),
    Drops(u32, Vec<sunrise_items::items::ItemStack>),
    Reward(u128, Advancement),
    Notice(u128, String, u32),
}

impl Journal {
    pub(in crate::game) fn merge(&mut self, child: Self) {
        for (conn, kept) in child.guests {
            self.guests.entry(conn).or_insert(kept);
        }
        for (conn, kept) in child.windows {
            self.windows.entry(conn).or_insert(kept);
        }
        for (id, kept) in child.mobs {
            self.mobs.entry(id).or_insert(kept);
        }
        if self.sequences.is_none() {
            self.sequences = child.sequences;
        }
        self.correct_on_abort.extend(child.correct_on_abort);
    }
}
