//! Server-side dropped-item handle reads and writes for native mods.
use super::commands::Target;
use mod_api::{EntityRef, ItemEntityRef, ItemStackView};
use sunrise_gameplay_common::game::api_values::{project_stack, stack_read_metadata};
use sunrise_items::items::ItemStack;
use sunrise_sim::lan::playerdata::{ints_uuid, uuid_ints};

impl crate::game::ServerGame<'_> {
    /// `ItemEntity.playerTouch(player)` now: see `ItemEntitiesBackendMut::player_touch_item`.
    pub(in crate::game) fn player_touch_item(
        &mut self,
        player: mod_api::PlayerRef,
        item: ItemEntityRef,
    ) -> Result<u32, mod_api::PickupError> {
        use mod_api::PickupError;
        let turn = self
            .native_resident_player_turn(player)
            .map_err(|refusal| {
                if matches!(refusal, mod_api::PlayerDataRefusal::Client) {
                    PickupError::Unavailable
                } else {
                    PickupError::Stale
                }
            })?;
        self.native_item_entity_index(item, None)
            .ok_or(PickupError::Stale)?;
        let id = item.entity().id();
        let mut taken = 0;
        turn.run(self, |game| taken = game.native_touch_item_now(id))
            .ok_or(PickupError::Stale)?;
        Ok(taken)
    }

    /// `ExperienceOrb.playerTouch(player)` now: see `ItemEntitiesBackendMut::player_touch_orb`.
    pub(in crate::game) fn player_touch_orb(
        &mut self,
        player: mod_api::PlayerRef,
        orb: mod_api::ExperienceOrbRef,
    ) -> Result<bool, mod_api::PickupError> {
        use mod_api::PickupError;
        let turn = self
            .native_resident_player_turn(player)
            .map_err(|refusal| {
                if matches!(refusal, mod_api::PlayerDataRefusal::Client) {
                    PickupError::Unavailable
                } else {
                    PickupError::Stale
                }
            })?;
        self.native_orb_mut(orb.entity())
            .map_err(|_| PickupError::Stale)?;
        let id = orb.entity().id();
        let mut taken = false;
        turn.run(self, |game| taken = game.native_touch_orb_now(id))
            .ok_or(PickupError::Stale)?;
        Ok(taken)
    }

    /// Resolve the stable item identity, reusing the caller's known slot while it still matches.
    pub(in crate::game) fn native_item_entity_index(
        &self,
        item: ItemEntityRef,
        hint: Option<usize>,
    ) -> Option<usize> {
        let (Target::Item(_), Some(location)) =
            self.native_entity_target_with_location(item.entity())?
        else {
            return None;
        };
        hint.filter(|&i| {
            self.common
                .dropped
                .get(i)
                .is_some_and(|row| row.join == location.join)
        })
        .or_else(|| self.common.dropped.locate(location.join))
    }

    pub(in crate::game) fn server_item_entity(&self, entity: EntityRef) -> Option<ItemEntityRef> {
        let item = ItemEntityRef::new(entity);
        self.native_item_entity_index(item, None).map(|_| item)
    }

    /// Lend `ItemEntity.getItem`. Encode components only when the reader projects the view.
    pub(in crate::game) fn item_entity_stack(
        &self,
        item: ItemEntityRef,
        read: &mut dyn for<'a> FnMut(ItemStackView<'a>),
    ) -> Option<()> {
        let stack = &self
            .common
            .dropped
            .get(self.native_item_entity_index(item, None)?)?
            .stack;
        let metadata = stack_read_metadata(stack).ok()?;
        let project = |metadata| project_stack(stack, metadata).ok();
        read(ItemStackView::resident(metadata, &project));
        Some(())
    }

    /// Writes a stack using `ItemEntity.setItem`; None clears it.
    pub(in crate::game) fn item_entity_set_stack(
        &mut self,
        item: ItemEntityRef,
        stack: Option<ItemStack>,
    ) -> bool {
        let Some(index) = self.native_item_entity_index(item, None) else {
            return false;
        };
        self.journal_set_item_entity_at(index, stack);
        true
    }

    /// The signed pickup delay; the infinite sentinel reads as vanilla's 32767.
    pub(in crate::game) fn item_entity_pickup_delay(&self, item: ItemEntityRef) -> Option<i32> {
        Some(self.common.dropped[self.native_item_entity_index(item, None)?].pickup_delay)
    }

    /// `ItemEntity.setPickUpDelay`.
    pub(in crate::game) fn item_entity_set_pickup_delay(
        &mut self,
        item: ItemEntityRef,
        ticks: i32,
    ) -> bool {
        let Some(index) = self.native_item_entity_index(item, None) else {
            return false;
        };
        self.journal_set_item_pickup_delay(index, ticks);
        true
    }

    /// `ItemEntity.setDefaultPickUpDelay`: ten ticks.
    pub(in crate::game) fn item_entity_set_default_pickup_delay(
        &mut self,
        item: ItemEntityRef,
    ) -> bool {
        self.item_entity_set_pickup_delay(item, 10)
    }

    /// `ItemEntity.setNoPickUpDelay`.
    pub(in crate::game) fn item_entity_set_no_pickup_delay(&mut self, item: ItemEntityRef) -> bool {
        self.item_entity_set_pickup_delay(item, 0)
    }

    /// `ItemEntity.setNeverPickUp`: the infinite-delay sentinel 32767.
    pub(in crate::game) fn item_entity_set_never_pickup(&mut self, item: ItemEntityRef) -> bool {
        self.item_entity_set_pickup_delay(
            item,
            sunrise_sim::entities::ItemEntity::INFINITE_PICKUP_DELAY,
        )
    }

    /// `ItemEntity.getAge`, including negative lifetime sentinels.
    pub(in crate::game) fn item_entity_age(&self, item: ItemEntityRef) -> Option<i32> {
        Some(
            self.common
                .dropped
                .get(self.native_item_entity_index(item, None)?)?
                .age,
        )
    }

    /// The UUID set by `ItemEntity.setTarget`, limiting pickup to that player.
    pub(in crate::game) fn item_entity_target(&self, item: ItemEntityRef) -> Option<u128> {
        self.common
            .dropped
            .get(self.native_item_entity_index(item, None)?)?
            .target
            .map(ints_uuid)
    }

    /// `ItemEntity.setTarget`; None allows any player to pick it up.
    pub(in crate::game) fn item_entity_set_target(
        &mut self,
        item: ItemEntityRef,
        target: Option<u128>,
    ) -> bool {
        let Some(index) = self.native_item_entity_index(item, None) else {
            return false;
        };
        self.journal_set_item_target(index, target.map(uuid_ints));
        true
    }

    /// `ItemEntity.getOwner`: the thrower as a resolved entity, not the pickup target.
    pub(in crate::game) fn item_entity_owner(&self, item: ItemEntityRef) -> Option<EntityRef> {
        let uuid = self
            .common
            .dropped
            .get(self.native_item_entity_index(item, None)?)?
            .thrower?;
        let target = self.target_by_uuid(ints_uuid(uuid))?;
        if self.target_level(target) != self.common.dim {
            return None;
        }
        let id = u32::try_from(self.network_entity_id(target)?).ok()?;
        self.entity(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::native_entity_refs::tests::with_game;
    use sunrise_core::geometry::Vec3;
    use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
    use sunrise_gameplay_common::game::native_item_entity_refs::NativeItemEntityRefsGameExt1 as _;
    use sunrise_gameplay_common::game::native_server::NativeServerGameExt1 as _;
    use sunrise_gameplay_common::game::test_fixtures::fixture_resident_column;

    /// `PlayerRef::touch_item` and `touch_orb` (#6126): a guest takes a dropped stack and an
    /// orb through the game's own pickup, honouring the pickup delay and `takeXpDelay`.
    #[test]
    #[ignore = "daily"]
    fn a_guest_touches_an_item_and_an_orb_through_the_game_pickup() {
        crate::game::native_guest_turn_tests::with_guests(|game| {
            fixture_resident_column(&mut game.world, (0, 0));
            let guest = game.player(12).unwrap();
            let held = |game: &mut crate::game::ServerGame<'_>| {
                game.as_guest(12, |g| {
                    g.inventory.iter().flatten().map(|s| s.count).sum::<u32>()
                })
                .unwrap()
            };
            let before = held(game);

            // The pickup delay refuses it, as `ItemEntity.playerTouch` does.
            game.spawn_item(
                ItemStack::new("minecraft:stone", 5),
                [3.0, 64.0, 5.0],
                [0.0; 3],
                10,
            );
            let item = game
                .item_entity(game.entity(game.dropped[0].id).unwrap())
                .unwrap();
            assert_eq!(guest.touch_item(game, item), Ok(0));
            assert_eq!(game.dropped.len(), 1);
            assert!(item.set_no_pickup_delay(game));
            assert_eq!(guest.touch_item(game, item), Ok(5));
            assert!(game.dropped.is_empty(), "the whole stack went in");
            assert_eq!(held(game), before + 5);
            assert_eq!(
                guest.touch_item(game, item),
                Err(mod_api::PickupError::Stale),
                "the item is gone"
            );

            // An orb: taken once, then `takeXpDelay` refuses the next.
            game.add_orb([3.0, 64.0, 5.0], 7);
            game.add_orb([3.0, 64.0, 5.0], 3);
            let orb = |game: &crate::game::ServerGame<'_>, i: usize| {
                mod_api::prelude::EntitiesServerHost::experience_orb(
                    game,
                    game.entity(game.orbs[i].id).unwrap(),
                )
                .unwrap()
                .unwrap()
            };
            let (first, second) = (orb(game, 0), orb(game, 1));
            assert_eq!(guest.touch_orb(game, first), Ok(true));
            assert_eq!(game.orbs.len(), 1);
            assert_eq!(guest.exp_cooldown(game), Some(2));
            assert_eq!(guest.touch_orb(game, second), Ok(false));
            assert_eq!(game.orbs.len(), 1, "the delay left it on the ground");
            assert_eq!(
                guest.touch_orb(game, first),
                Err(mod_api::PickupError::Stale)
            );
        });
    }

    /// `PlayerRef::touch_item` and `touch_orb` (#6126) for the host's own player: the same
    /// pickup delay, stack, `takeXpDelay` and experience as the pass gives it.
    #[test]
    #[ignore = "daily"]
    fn the_host_player_touches_an_item_and_an_orb_through_the_game_pickup() {
        with_game(|game| {
            fixture_resident_column(&mut game.world, (0, 0));
            game.player = Some(
                sunrise_sim::motion::PlayerMotion::new(game.motion, Vec3::new(3.0, 64.0, 5.0))
                    .unwrap(),
            );
            crate::game::native_entity_refs::tests::bind_local_player(game, 20, 123);
            let player = game.player(123).unwrap();
            let held = |game: &crate::game::ServerGame<'_>| {
                game.inventory
                    .iter()
                    .flatten()
                    .map(|s| s.count)
                    .sum::<u32>()
            };
            let before = held(game);

            // The pickup delay refuses it, as `ItemEntity.playerTouch` does.
            game.spawn_item(
                ItemStack::new("minecraft:stone", 5),
                [8.0, 64.0, 8.0],
                [0.0; 3],
                10,
            );
            let item = game
                .item_entity(game.entity(game.dropped[0].id).unwrap())
                .unwrap();
            assert_eq!(player.touch_item(game, item), Ok(0));
            assert_eq!(game.dropped.len(), 1);
            assert!(item.set_no_pickup_delay(game));
            assert_eq!(player.touch_item(game, item), Ok(5));
            assert!(game.dropped.is_empty(), "the whole stack went in");
            assert_eq!(held(game), before + 5);
            assert_eq!(
                player.touch_item(game, item),
                Err(mod_api::PickupError::Stale),
                "the item is gone"
            );

            // An orb out of reach: taken once, then `takeXpDelay` refuses the next.
            game.server.xp_delay = 0;
            game.experience = Default::default();
            game.add_orb([8.0, 64.0, 8.0], 7);
            game.add_orb([8.0, 64.0, 8.0], 3);
            let orb = |game: &crate::game::ServerGame<'_>, i: usize| {
                mod_api::prelude::EntitiesServerHost::experience_orb(
                    game,
                    game.entity(game.orbs[i].id).unwrap(),
                )
                .unwrap()
                .unwrap()
            };
            let (first, second) = (orb(game, 0), orb(game, 1));
            assert_eq!(player.touch_orb(game, first), Ok(true));
            assert_eq!(game.orbs.len(), 1);
            assert_eq!(game.experience.total, 7);
            assert_eq!(player.exp_cooldown(game), Some(2));
            assert_eq!(player.touch_orb(game, second), Ok(false));
            assert_eq!(game.orbs.len(), 1, "the delay left it on the ground");
            assert_eq!(game.experience.total, 7);
            assert_eq!(
                player.touch_orb(game, first),
                Err(mod_api::PickupError::Stale)
            );
        });
    }

    /// Vetoes every `pickup_xp`, counting the calls.
    struct VetoOrbs(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for VetoOrbs {
        fn pickup_xp(
            &self,
            _: &mut dyn mod_api::ServerHost,
            _: &mut mod_api::PickupXpEvent<'_>,
        ) -> mod_api::Flow {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            mod_api::Flow::Cancel
        }
    }

    /// `PlayerRef::touch_orb` fires `pickup_xp` first, for the host's own player and for a
    /// guest: a veto leaves the orb whole and `takeXpDelay` at 0, as in
    /// `ExperienceOrb.playerTouch` (NeoForge's `PlayerXpEvent.PickupXp`).
    #[test]
    #[ignore = "daily"]
    fn a_pickup_xp_veto_refuses_a_touched_orb_for_the_host_and_for_a_guest() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let mods = vec![mod_api::ModEntry::new(
            "veto-orbs",
            Box::new(VetoOrbs(Arc::clone(&calls))),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                fixture_resident_column(&mut game.world, (0, 0));
                game.player = Some(
                    sunrise_sim::motion::PlayerMotion::new(game.motion, Vec3::new(3.0, 64.0, 5.0))
                        .unwrap(),
                );
                crate::game::native_entity_refs::tests::bind_local_player(game, 20, 123);
                let player = game.player(123).unwrap();
                game.server.xp_delay = 0;
                game.experience = Default::default();
                game.add_orb([8.0, 64.0, 8.0], 7);
                let orb = mod_api::prelude::EntitiesServerHost::experience_orb(
                    game,
                    game.entity(game.orbs[0].id).unwrap(),
                )
                .unwrap()
                .unwrap();
                assert_eq!(player.touch_orb(game, orb), Ok(false));
                assert_eq!(calls.load(Ordering::Relaxed), 1);
                assert_eq!((game.orbs.len(), game.orbs[0].count), (1, 1));
                assert_eq!(game.experience.total, 0);
                assert_eq!(player.exp_cooldown(game), Some(0));
            });
            crate::game::native_guest_turn_tests::with_guests(|game| {
                // A guest's pickup asks the mods on a server.
                game.options.dedicated = Some(Vec::new());
                fixture_resident_column(&mut game.world, (0, 0));
                let guest = game.player(12).unwrap();
                game.add_orb([3.0, 64.0, 5.0], 7);
                let orb = mod_api::prelude::EntitiesServerHost::experience_orb(
                    game,
                    game.entity(game.orbs[0].id).unwrap(),
                )
                .unwrap()
                .unwrap();
                assert_eq!(guest.touch_orb(game, orb), Ok(false));
                assert_eq!(calls.load(Ordering::Relaxed), 2);
                assert_eq!((game.orbs.len(), game.orbs[0].count), (1, 1));
                assert_eq!(guest.exp_cooldown(game), Some(0));
            });
        });
    }
}

use sunrise_gameplay_common::game::commands_common::CommandsCommonGameExt2 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt5 as _;
