use super::*;

/// The hook point. It does nothing in Sunrise.
impl Game<'_> {
    /// A player or mob that fell onto farmland is about to turn it to its base block
    /// (`BlockEvent.FarmlandTrampleEvent`): `entity` is the landing living entity, `pos` the
    /// farmland and `state` what it is, `fall_distance` the fall. Cancel and the farmland and
    /// its crop stay as they are. Asked once the landing has returned, in one transaction with
    /// the apply: the block at `pos` must still be `state`, or the trample is refused.
    pub(in crate::game) fn native_farmland_trample(
        &mut self,
        entity: mod_api::LivingRef,
        pos: BlockPos,
        state: BlockStateId,
        fall_distance: f64,
        cancel: &mut bool,
    ) {
        self.fire_farmland_trample(entity, pos, state, fall_distance, cancel);
    }
}

impl Game<'_> {
    /// The last two steps of a trample the landing decided on (`FarmBlock.fallOn`: the chance,
    /// the checks and the size test already ran, in vanilla's order, and left the farmland
    /// alone): ask `farmland_trample` with the landing `entity` (`player`: the acting guest),
    /// then turn the farmland at `pos` into `base`, the block's write, the entities it lifts and
    /// its game event as `FarmlandBlock.turnToBaseBlock` does them. A listener's writes and the
    /// check that the block at `pos` is still `state` are one transaction. A block that
    /// changed, either before the listener or by it, or an entity that is gone, refuses the
    /// trample: it is never recomputed, and the listener's transactional writes are rolled back.
    pub(in crate::game) fn trample_farmland(
        &mut self,
        entity: u32,
        player: bool,
        pos: BlockPos,
        state: BlockStateId,
        base: BlockStateId,
        distance: f64,
    ) {
        let still = |game: &Self| game.world.block(pos.x, pos.y, pos.z) == state;
        let Some(living) = self.entity(entity).and_then(|e| self.living(e)) else {
            return;
        };
        if !still(self) {
            return;
        }
        let mut cancel = false;
        let kept: Result<(), ()> = mod_api::run_transaction(self, |game, _| {
            game.native_farmland_trample(living, pos, state, distance, &mut cancel);
            if still(game) { Ok(()) } else { Err(()) }
        });
        if kept.is_err() || cancel {
            return;
        }
        // The player's write asks `entity_change_block_pre` first; a mob's never did.
        if player {
            let acting = self.native_acting_id();
            if self.landing_change_cancelled(Some(acting), pos, state, base) {
                return;
            }
        }
        let at = [pos.x, pos.y, pos.z];
        if self.level_set(at, base).is_ok() {
            self.push_entities_up(pos, state);
            let who = if player {
                Who::Player
            } else {
                Who::Mob(entity)
            };
            self.post_block_game_event("minecraft:block_change", at, Some(who), Some(base));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::native_entity_refs::tests::{bind_local_player, with_game};
    use mod_api::*;
    use std::sync::Arc;
    use sunrise_core::geometry::Vec3;

    /// Cancels the trample of a sneaking or crouching entity, read through its handle; with
    /// `meddle` it also replaces the farmland under it first.
    struct Gardener {
        meddle: bool,
    }
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Gardener {
        fn farmland_trample(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::FarmlandTrampleEvent,
        ) -> Flow {
            let entity = event.entity.entity();
            if self.meddle {
                let level = entity.level(host).unwrap();
                let key = host.block_key("minecraft:stone").unwrap();
                let stone = mod_api::BlockState::of(&host.block_states(), key, &[]).unwrap();
                let flags = mod_api::BlockUpdateFlags::KNOWN_SHAPE;
                assert_eq!(
                    level.set_block(host, [event.pos.x, event.pos.y, event.pos.z], stone, flags),
                    Ok(true)
                );
            }
            if entity.is_shift_key_down(host) == Some(true)
                || entity.is_crouching(host) == Some(true)
            {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        }
    }

    /// The block at (1, 64, 1) after `entity` (a player or a cat) landed on farmland there and
    /// was told to a `Gardener`: farmland (kept or refused) or dirt (trampled), or stone.
    fn landing(player: bool, sneaking: bool, meddle: bool) -> String {
        let mut left = String::new();
        mod_platform::testing::with_mods(
            vec![ModEntry::new("gardener", Box::new(Gardener { meddle }))].into(),
            || {
                with_game(|game| {
                    game.options.dedicated = Some(Vec::new());
                    game.world
                        .chunks
                        .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
                    let registry = game.pack.assets.registry;
                    let farmland = registry.default_state("minecraft:farmland").unwrap();
                    let dirt = registry.default_state("minecraft:dirt").unwrap();
                    let pos = BlockPos::new(1, 64, 1);
                    game.world.set(pos.x, pos.y, pos.z, farmland).unwrap();
                    let id = if player {
                        let mut body =
                            sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::ZERO)
                                .unwrap();
                        body.shift_key_down = sneaking;
                        game.player = Some(body);
                        bind_local_player(game, 20, 123);
                        20
                    } else {
                        let kind = game.res.mob_types().get("minecraft:cat").unwrap().clone();
                        let mut cat = sunrise_client::mobs::Mob::new(
                            72,
                            kind,
                            Vec3::new(1.5, 64.0, 1.5),
                            0.0,
                        );
                        // `Cat.setPose`: 5 is `Pose.CROUCHING`.
                        cat.extra.pose = if sneaking { 5 } else { 0 };
                        game.mobs.push(cat);
                        72
                    };
                    game.trample_farmland(id, player, pos, farmland, dirt, 3.0);
                    let now = game.world.block(pos.x, pos.y, pos.z);
                    left = registry.state(now).unwrap().name.clone();
                });
            },
        );
        left
    }

    #[test]
    #[ignore = "daily"]
    fn a_sneaking_player_keeps_the_farmland_and_a_walking_one_tramples_it() {
        assert_eq!(landing(true, true, false), "minecraft:farmland");
        assert_eq!(landing(true, false, false), "minecraft:dirt");
    }

    #[test]
    #[ignore = "daily"]
    fn a_crouching_mob_keeps_the_farmland_and_a_standing_one_tramples_it() {
        assert_eq!(landing(false, true, false), "minecraft:farmland");
        assert_eq!(landing(false, false, false), "minecraft:dirt");
    }

    #[test]
    #[ignore = "daily"]
    fn a_listener_that_changes_the_block_refuses_the_trample() {
        // The block the listener left is kept, not recomputed into dirt.
        assert_eq!(landing(true, false, true), "minecraft:stone");
        assert_eq!(landing(false, false, true), "minecraft:stone");
    }
}
