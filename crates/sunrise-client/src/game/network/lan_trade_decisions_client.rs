#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
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
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions::Effect;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions::Journal;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions::Restore;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions::TakePlan;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions::TakeReads;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{cell::RefCell, collections::HashMap};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    advancements::Advancement,
    items::ItemStack,
    lan::conn::transaction_packets::Scope,
    menu::{Input, Menu, StackedOutcome},
    merchant::Event,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::play;

impl Game<'_> {
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
