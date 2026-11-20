use super::*;
use sunrise_world::block_landing::FarmlandTrample;

/// The hook point. It does nothing in Sunrise.
impl Game<'_> {
    /// A player or mob that fell onto farmland is about to turn it to its base block
    /// (`BlockEvent.FarmlandTrampleEvent`): `entity` is the landing living entity, `pos` the
    /// farmland and `state` what it is, `fall_distance` the fall. Cancel and the farmland and
    /// its crop stay as they are. Asked inside the landing before anything is lifted, changed
    /// or hurt by the fall.
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
    /// Apply the owning-host decision inside `FarmlandBlock.fallOn`, while the
    /// landing is paused. The caller resumes fall damage even when this refuses.
    pub(in crate::game) fn trample_farmland(
        &mut self,
        entity: u32,
        player: bool,
        trample: FarmlandTrample,
    ) {
        let FarmlandTrample {
            position: pos,
            state,
            base,
            distance,
            ..
        } = trample;
        let still = |game: &Self| game.world.block(pos.x, pos.y, pos.z) == state;
        if !still(self) {
            return;
        }
        if !mod_api::__private::listened(mod_api::HookId::FarmlandTrample) {
            self.apply_farmland_trample(entity, player, pos, state, base);
            return;
        }
        let Some(living) = self.entity(entity).and_then(|e| self.living(e)) else {
            return;
        };
        let _: Result<(), ()> = mod_api::run_transaction(self, |game, _| {
            let mut cancel = false;
            game.native_farmland_trample(living, pos, state, distance, &mut cancel);
            if !still(game) || game.entity(entity) != Some(living.entity()) {
                return Err(());
            }
            if !cancel {
                game.apply_farmland_trample(entity, player, pos, state, base);
            }
            Ok(())
        });
    }

    fn apply_farmland_trample(
        &mut self,
        entity: u32,
        player: bool,
        pos: BlockPos,
        state: BlockStateId,
        base: BlockStateId,
    ) {
        // The player's write asks `entity_change_block_pre` first; a mob's never did.
        if player {
            let acting = self.native_acting_id();
            if self.landing_change_cancelled(Some(acting), pos, state, base) {
                return;
            }
        }
        let at = [pos.x, pos.y, pos.z];
        // `FarmlandBlock.turnToBaseBlock` lifts bodies while the old state is
        // still in the level, then writes the base and emits BLOCK_CHANGE.
        self.push_entities_up(pos, state, base);
        if self.level_set(at, base).is_ok() {
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
    use crate::game::native_entity_refs::tests::with_game;
    use crate::game::native_item_places::tests::{guest, open_to_lan};
    use mod_api::*;
    use std::sync::{Arc, Mutex};
    use sunrise_client::{chunk::Chunk, motion::PlayerMotion};
    use sunrise_core::geometry::Vec3;

    const FIELD: [i32; 3] = [1, 63, 1];
    const LANDING_Y: f64 = 63.9375;
    const LANDING: u32 = 20;
    const OTHER: u32 = 72;
    const OUTSIDE: u32 = 73;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Choice {
        Absent,
        Allow,
        Trap,
        Cancel,
        Replace,
    }

    #[derive(Debug, PartialEq)]
    enum Moment {
        Hook { y: f64, health: f32 },
        Lift(u32, f64, Option<mod_api::BlockState>),
        Write(mod_api::BlockState),
        Fall(f32, f64, mod_api::BlockState),
    }

    type Trace = Arc<Mutex<Vec<Moment>>>;

    // This observer listens to the effects, not to farmland_trample. It can be
    // installed on both sides of the allow/no-listener comparison.
    struct Observe(Trace);
    #[mod_api::__subscriptions]
    impl Hooks for Observe {
        fn entity_entering_section(
            &self,
            host: HostSide<'_>,
            event: &mut EntityEnteringSectionEvent,
        ) {
            if let HostSide::Server(host) = host
                && event.old_section[1] == 3
                && event.new_section[1] == 4
            {
                let state = event
                    .entity
                    .level(host)
                    .and_then(|level| level.block_state(host, FIELD));
                self.0.lock().unwrap().push(Moment::Lift(
                    event.entity.id(),
                    event.entity.position(host).unwrap()[1],
                    state,
                ));
            }
        }
        fn block_changed(&self, level: &dyn LevelHost, event: &mut BlockChangedEvent) {
            if [event.pos.x, event.pos.y, event.pos.z] == FIELD {
                assert_eq!(level.block_state(FIELD), Some(event.new));
                self.0.lock().unwrap().push(Moment::Write(event.new));
            }
        }
        fn living_incoming_damage(
            &self,
            level: &mut dyn HitHost,
            event: &mut LivingIncomingDamageEvent<'_>,
        ) -> Flow {
            if event.entity as u32 == LANDING && event.source.damage_type == "minecraft:fall" {
                self.0.lock().unwrap().push(Moment::Fall(
                    event.container.new_damage(),
                    event.victim.position[1],
                    level.block_state(FIELD).unwrap(),
                ));
            }
            Flow::Continue
        }
    }

    struct Gardener(Choice, Trace);
    #[mod_api::__subscriptions]
    impl Hooks for Gardener {
        fn farmland_trample(
            &self,
            host: &mut dyn ServerHost,
            event: &mut FarmlandTrampleEvent,
        ) -> Flow {
            let entity = event.entity.entity();
            assert_eq!(entity.id(), LANDING);
            assert_eq!([event.pos.x, event.pos.y, event.pos.z], FIELD);
            let level = entity.level(host).unwrap();
            assert_eq!(level.block_state(host, FIELD), Some(event.state));
            self.1.lock().unwrap().push(Moment::Hook {
                y: entity.position(host).unwrap()[1],
                health: event.entity.health(host).unwrap(),
            });
            match self.0 {
                Choice::Cancel => Flow::Cancel,
                Choice::Replace => {
                    let key = host.block_key("minecraft:stone").unwrap();
                    let stone = mod_api::BlockState::of(&host.block_states(), key, &[]).unwrap();
                    assert_eq!(
                        level.set_block(host, FIELD, stone, BlockUpdateFlags::KNOWN_SHAPE),
                        Ok(true)
                    );
                    Flow::Continue
                }
                Choice::Trap => {
                    event.cancelled = true;
                    panic!("trample listener trapped");
                }
                Choice::Allow => Flow::Continue,
                Choice::Absent => unreachable!(),
            }
        }
    }

    fn land(player: bool, choice: Choice) -> Vec<Moment> {
        crate::entry::install_client_hooks();
        let trace = Trace::default();
        let mut mods = vec![ModEntry::new(
            "trample-observer",
            Box::new(Observe(trace.clone())),
        )];
        if choice != Choice::Absent {
            mods.push(ModEntry::new(
                "gardener",
                Box::new(Gardener(choice, trace.clone())),
            ));
        }
        mod_platform::testing::with_mods(mods.into(), || {
            with_game(|game| {
                if player {
                    open_to_lan(game);
                } else {
                    game.options.dedicated = Some(Vec::new());
                }
                game.streaming
                    .insert_remote(&mut game.world, Chunk::empty(0, 0));
                let reg = game.pack.assets.registry;
                let [farmland, dirt, wheat, stone] = [
                    "minecraft:farmland",
                    "minecraft:dirt",
                    "minecraft:wheat",
                    "minecraft:stone",
                ]
                .map(|name| reg.default_state(name).unwrap());
                let [x, y, z] = FIELD;
                game.world.set(x, y, z, farmland).unwrap();
                game.world.set(x, y + 1, z, wheat).unwrap();
                let kind = game.res.mob_types().get("minecraft:pig").unwrap().clone();
                let initial_health = if player {
                    let mut player = guest(1);
                    player.loaded = true;
                    player.position = [1.5, LANDING_Y, 1.5];
                    player.moves.push(([0.0, -0.25, 0.0], true));
                    let mut body =
                        PlayerMotion::new(game.motion, Vec3::new(1.5, LANDING_Y, 1.5)).unwrap();
                    body.fall_distance = 6.0;
                    player.motion = Some(Box::new(body));
                    let health = player.health;
                    game.lan.as_mut().unwrap().hub.guests.push(player);
                    game.entity_section_joined(LANDING, [1.5, LANDING_Y, 1.5]);
                    health
                } else {
                    let mut mob = sunrise_client::mobs::Mob::new(
                        LANDING,
                        kind.clone(),
                        Vec3::new(1.5, 64.125, 1.5),
                        0.0,
                    );
                    mob.fall_distance = 6.0;
                    mob.velocity.y = -0.5;
                    let health = mob.health;
                    game.mobs.push(mob);
                    game.entity_section_joined(LANDING, [1.5, 64.125, 1.5]);
                    health
                };
                for (id, x) in [(OTHER, 1.8), (OUTSIDE, 3.5)] {
                    let mut other = sunrise_client::mobs::Mob::new(
                        id,
                        kind.clone(),
                        Vec3::new(x, LANDING_Y, 1.5),
                        0.0,
                    );
                    other.no_ai = true;
                    game.mobs.push(other);
                    game.entity_section_joined(id, [x, LANDING_Y, 1.5]);
                }
                if player {
                    game.guest_server_tick(1, true);
                } else {
                    game.tick_creature_entities().unwrap();
                }
                let (landing_y, health) = if player {
                    let g = game.lan.as_ref().unwrap().hub.guest(1).unwrap();
                    (g.position[1], g.health)
                } else {
                    let m = game.mob_object(LANDING).unwrap();
                    (m.position.y, m.health)
                };
                let converted = matches!(choice, Choice::Absent | Choice::Allow | Choice::Trap);
                let floor = match choice {
                    Choice::Replace => stone,
                    Choice::Cancel => farmland,
                    _ => dirt,
                };
                let y_after = if converted { 64.0 } else { LANDING_Y };
                assert_eq!(game.world.block(x, y, z), floor);
                if choice == Choice::Cancel {
                    assert_eq!(game.world.block(x, y + 1, z), wheat);
                }
                assert_eq!(landing_y, y_after);
                assert_eq!(game.mob_object(OTHER).unwrap().position.y, y_after);
                assert_eq!(game.mob_object(OUTSIDE).unwrap().position.y, LANDING_Y);
                assert_eq!(health, initial_health - 3.0);

                let api_state =
                    |state: sunrise_world::registry::BlockStateId| -> mod_api::BlockState {
                        use crate::game::api_values::TryIntoApi;
                        state.try_into_api().unwrap()
                    };
                let mut expected = Vec::new();
                if choice != Choice::Absent {
                    expected.push(Moment::Hook {
                        y: LANDING_Y,
                        health: initial_health,
                    });
                }
                if converted {
                    let farmland = Some(api_state(farmland));
                    expected.extend([
                        Moment::Lift(LANDING, 64.0, farmland),
                        Moment::Lift(OTHER, 64.0, farmland),
                    ]);
                }
                if choice != Choice::Cancel {
                    expected.push(Moment::Write(api_state(floor)));
                }
                expected.push(Moment::Fall(3.0, y_after, api_state(floor)));
                assert_eq!(*trace.lock().unwrap(), expected);
            });
        });
        std::mem::take(&mut *trace.lock().unwrap())
    }

    #[test]
    #[ignore = "daily"]
    fn an_allowing_listener_keeps_the_player_landing_order() {
        let baseline = land(true, Choice::Absent);
        let allowed = land(true, Choice::Allow);
        assert_eq!(allowed[1..], baseline);
    }

    #[test]
    #[ignore = "daily"]
    fn an_allowing_listener_keeps_the_mob_landing_order_and_lifts_all_bodies() {
        let baseline = land(false, Choice::Absent);
        let allowed = land(false, Choice::Allow);
        assert_eq!(allowed[1..], baseline);
    }

    #[test]
    #[ignore = "daily"]
    fn a_trapping_listener_does_not_change_the_landing_order() {
        for player in [true, false] {
            let baseline = land(player, Choice::Absent);
            let trapped = land(player, Choice::Trap);
            assert_eq!(trapped[1..], baseline);
        }
    }

    #[test]
    #[ignore = "daily"]
    fn cancelling_keeps_the_crop_without_preventing_fall_damage() {
        for player in [true, false] {
            land(player, Choice::Cancel);
        }
    }

    #[test]
    #[ignore = "daily"]
    fn a_listener_that_changes_the_block_refuses_the_trample() {
        for player in [true, false] {
            land(player, Choice::Replace);
        }
    }
}
