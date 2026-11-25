//! A subscribed guest's merchant result click is one savepoint. Each take has its own
//! frozen menu plan, collect and apply; a later refusal aborts every earlier iteration.
#[allow(unused_imports)]
use super::{
    Game,
    decided_action::{DecidedTarget, Epoch},
    lan_trading::{MerchantRun, opaque_villager},
    stacked_on_other::Pickup,
    trading::MadeMaps,
    transactions::{
        After,
        decision::{Identity, OwnerStamp, Reply, Side},
    },
};
#[allow(unused_imports)]
use std::{cell::RefCell, collections::HashMap};
#[allow(unused_imports)]
use sunrise_client::{
    advancements::Advancement,
    items::ItemStack,
    lan::conn::transaction_packets::Scope,
    menu::{Input, Menu, StackedOutcome},
    merchant::Event,
};
#[allow(unused_imports)]
use sunrise_protocol::packets::play;

/// The live owner's generation covers its slots, offer list, selection, and packet snapshot.
/// The action's player epoch separately covers the cursor and player-owned destinations.
pub(in crate::game) struct TakeReads {
    pub(in crate::game) conn: u32,
    pub(in crate::game) window: i32,
    pub(in crate::game) identity: Identity,
    pub(in crate::game) stamp: OwnerStamp,
    pub(in crate::game) hub_generation: u64,
    pub(in crate::game) creative: bool,
    pub(in crate::game) map_id: Option<i32>,
}

pub(in crate::game) struct TakePlan {
    pub(in crate::game) menu: Menu,
    pub(in crate::game) made: MadeMaps,
    pub(in crate::game) player_slots: Vec<(usize, Option<ItemStack>)>,
    pub(in crate::game) offhand: Option<Option<ItemStack>>,
}

// Keep typed snapshots in closures so no storage's private representation escapes its owner.
pub(in crate::game) type Restore =
    Box<dyn FnOnce(&mut Game<'_, crate::game::server_side::ServerState>)>;
#[derive(Default)]
pub(in crate::game) struct Journal {
    pub(in crate::game) guests: HashMap<u32, Restore>,
    pub(in crate::game) windows: HashMap<u32, Restore>,
    pub(in crate::game) mobs: HashMap<u32, Restore>,
    pub(in crate::game) sequences: Option<sunrise_client::random_sequences::RandomSequences>,
    pub(in crate::game) active: Option<u32>,
    pub(in crate::game) correct_on_abort: std::collections::BTreeSet<(u32, i32)>,
    pub(in crate::game) refused_clicks: std::collections::BTreeSet<(u32, i32)>,
}

pub(in crate::game) enum Effect {
    Orb([f64; 3], u32),
    Drops(u32, Vec<sunrise_client::items::ItemStack>),
    Reward(u128, Advancement),
    Notice(u128, String, u32),
}

impl Game<'_, crate::game::server_side::ServerState> {
    /// Any write to an open merchant menu, including a MerchantRef edit to another guest,
    /// belongs to the transaction that first changed it.
    pub(in crate::game) fn journal_guest_merchant(&mut self, conn: u32) {
        let Some(level) = self.transactions.last() else {
            return;
        };
        if level.trades.guests.contains_key(&conn) {
            return;
        }
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let Some(g) = host.hub.guest(conn) else {
            return;
        };
        let Some(merchant) = host
            .menus
            .get(&conn)
            .and_then(|menus| menus.merchant.clone())
        else {
            return;
        };
        // A refresh can run during the buyer's loan; it never mutates their stats.
        let stats = host.play.get(&conn).map(|play| play.stats.clone());
        let progress = g.advancements.clone();
        let changed = g.advancements_changed.clone();
        let unlocked = g.recipes_unlocked.clone();
        let book = g.recipe_book.clone();
        self.journal_guest_click(conn);
        let level = self.transactions.last_mut().expect("the action is open");
        level.trades.guests.insert(
            conn,
            Box::new(move |game| {
                if let Some(host) = game.lan.as_mut() {
                    host.menus
                        .entry(conn)
                        .or_default()
                        .merchant
                        .restore(Some(merchant));
                    if let Some(stats) = stats {
                        host.play.entry(conn).or_default().stats = stats;
                    }
                    if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
                        host.hub.guests.update(gi, |g| {
                            g.advancements = progress;
                            g.advancements_changed = changed;
                            g.recipes_unlocked = unlocked;
                            g.recipe_book = book;
                        });
                    }
                }
            }),
        );
    }

    /// A replacement can close a funded window from a listener's transaction.
    /// Keep the player's wire window with the trade and hold any open/close
    /// packets until the frame commits.
    pub(in crate::game) fn journal_merchant_replacement(&mut self, conn: u32) {
        let Some(level) = self.transactions.last_mut() else {
            return;
        };
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let Some(menus) = host.menus.get(&conn).filter(|m| m.merchant.is_some()) else {
            return;
        };
        let Some(guest) = host.hub.guest(conn) else {
            return;
        };
        level.trades.windows.entry(conn).or_insert_with(|| {
            let window = guest.menu_before_replacement();
            let heard = menus.menu_heard.clone();
            let owned = menus.owned_menu.clone();
            Box::new(move |game| {
                if let Some(host) = game.lan.as_mut() {
                    host.hub.guests.update_connection(conn, window);
                    let menus = host.menus.entry(conn).or_default();
                    menus.menu_heard = heard;
                    menus.owned_menu = owned;
                }
            })
        });
        level
            .packets
            .get_or_insert_with(sunrise_client::lan::conn::transaction_packets::Scope::begin);
    }

    /// Keep the fields merchant operations own, not the mob's health, equipment or effects,
    /// which belong to their owning journals.
    pub(in crate::game) fn journal_merchant_mob(&mut self, id: u32) {
        let Some(level) = self.transactions.last_mut() else {
            return;
        };
        let std::collections::hash_map::Entry::Vacant(entry) = level.trades.mobs.entry(id) else {
            return;
        };
        let Some(m) = self.mobs.iter().find(|m| m.id == id) else {
            return;
        };
        let random = m.random.clone();
        let timer = m.ambient_timer;
        let trade = m.extra.trade.clone();
        let trading = m.extra.flag2;
        let profession = m.extra.profession.clone();
        let level = m.extra.villager_level;
        let variant = m.extra.variant;
        let villager = m.extra.villager.clone();
        entry.insert(Box::new(move |game| {
            if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
                m.random = random;
                m.ambient_timer = timer;
                m.extra.trade = trade;
                m.extra.flag2 = trading;
                m.extra.profession = profession;
                m.extra.villager_level = level;
                m.extra.variant = variant;
                m.extra.villager = villager;
            }
        }));
    }

    pub(in crate::game) fn merchant_take_active(&self) -> bool {
        self.transactions.iter().any(|l| l.packets.is_some())
    }

    pub(in crate::game) fn merchant_drops(
        &mut self,
        conn: u32,
        stacks: Vec<sunrise_client::items::ItemStack>,
    ) {
        if self.merchant_take_active() {
            if !stacks.is_empty() {
                self.after_commit(After::Trade(Box::new(Effect::Drops(conn, stacks))));
            }
        } else if let Some(gi) = self
            .lan
            .as_ref()
            .and_then(|h| h.hub.guests.iter().position(|g| g.conn == conn))
        {
            self.drop_guest_items(gi, stacks);
        }
    }

    pub(in crate::game) fn merchant_orb(&mut self, at: [f64; 3], value: u32) {
        if self.merchant_take_active() {
            self.after_commit(After::Trade(Box::new(Effect::Orb(at, value))));
        } else {
            self.add_orb(at, value);
        }
    }

    // The criterion and its listener run tentatively. Rewards can spawn entities and run
    // arbitrary functions, so their publication belongs to the successful root commit.
    pub(in crate::game) fn defer_merchant_reward(&mut self, advancement: &Advancement) -> bool {
        if !self.merchant_take_active() {
            return false;
        }
        let uuid = self.native_acting_id();
        self.after_commit(After::Trade(Box::new(Effect::Reward(
            uuid,
            advancement.clone(),
        ))));
        true
    }
    pub(in crate::game) fn merchant_sound(
        &mut self,
        sound: &'static str,
        at: [f64; 3],
        pitch: f32,
    ) {
        if self.merchant_take_active() {
            self.defer_effect(super::transactions::DeferredEffect::Sound {
                sound,
                at,
                volume: 1.0,
                pitch,
            });
        } else {
            self.server_play_sound(sound, at, 1.0, pitch);
        }
    }
}
