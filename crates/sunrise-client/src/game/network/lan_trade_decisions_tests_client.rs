#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, native_guest_turn_tests::with_guests};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::Both;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::CALLS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::COMPLETED;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::EditInPost;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::EffectWrite;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::FishBreath;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::IN_ACTION;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::Mod;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::PlayerWrite;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::RemoveHero;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::SCRIPT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::SPEED;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::STACKED;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::STAGED_WRITES;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::Script;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::Stacked;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::USES;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::WORK_TICK;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::open;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::prepare_work;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::tick_work;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::work_screen;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::work_screen_mut;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan_trade_decisions_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    Flow, Hooks, HostSide, HostTransaction, ItemPlace, ItemResource, ItemStackedOnOtherEvent,
    ModEntry, Mods, PlayerTradeEvent, ServerHost, TradeWithVillagerEvent,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::cell::{Cell, RefCell};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::{Arc, mpsc};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::lan::{Out, PlayerSender};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    items::ItemStack,
    merchant::{Session, Trader},
    mobs::finalize::{self, Attr, Op},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::play::{
    Clientbound, ContainerClick,
};

pub(in crate::game) fn restock_after_hero_change(test: &str, before_work: bool) {
    use sunrise_client::effects::EffectInstance;
    super::in_own_process(test, || {
        let mods: Mods = vec![ModEntry::new(
            "remove-hero",
            Box::new(RemoveHero { before_work }),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_guests(|game| {
                let save = tempfile::tempdir().unwrap();
                open(game, 1);
                prepare_work(game, save.path());
                game.as_guest(12, |g| {
                    g.potions
                        .effects
                        .add(EffectInstance::new("hero_of_the_village", 1000, 0));
                });
                let screen = work_screen_mut(game);
                screen.slots = [Some(ItemStack::new("minecraft:wheat", 4)), None, None];
                let offer = &mut screen.merchant.session.offers[0];
                (offer.base_cost_a.stack.count, offer.uses) = (5, 10);
                let trader = game.mobs[0].extra.trade.as_mut().unwrap();
                (
                    trader.offers[0].base_cost_a.stack.count,
                    trader.offers[0].uses,
                ) = (5, 10);
                // The voice from opening the screen has finished its cooldown by work time.
                game.mobs[0].ambient_timer = 0;
                game.start_audio_capture();

                tick_work(game);

                game.as_guest(12, |g| assert_eq!(g.hero_amplifier(), None));
                let screen = work_screen(game);
                assert_eq!(
                    screen.slots[2],
                    (!before_work).then(|| ItemStack::new("minecraft:emerald", 1))
                );
                assert_eq!(screen.merchant.session.hero, (!before_work).then_some(0));
                let feedback: Vec<_> = game
                    .side
                    .pending_sounds
                    .iter()
                    .map(|sound| sound.event.as_str())
                    .filter(|&event| {
                        matches!(
                            event,
                            "minecraft:entity.villager.yes" | "minecraft:entity.villager.no"
                        )
                    })
                    .collect();
                assert_eq!(
                    feedback,
                    [if before_work {
                        "minecraft:entity.villager.no"
                    } else {
                        "minecraft:entity.villager.yes"
                    }]
                );
            })
        });
    });
}
