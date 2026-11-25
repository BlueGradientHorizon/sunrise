use super::native_guest_turn_tests::with_guests;
use crate::game::server_side::lan_trade_decisions_tests::RemoveHero;
use crate::game::server_side::lan_trade_decisions_tests::open;
use crate::game::server_side::lan_trade_decisions_tests::prepare_work;
use crate::game::server_side::lan_trade_decisions_tests::tick_work;
use crate::game::server_side::lan_trade_decisions_tests::work_screen;
use crate::game::server_side::lan_trade_decisions_tests::work_screen_mut;
use mod_api::{ModEntry, Mods};
use sunrise_client::items::ItemStack;

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
