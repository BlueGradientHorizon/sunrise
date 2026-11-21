use super::setup_live_damage_world;
use crate::game::{
    Game,
    combat::PlayerCredit,
    native_entity_refs::tests::{run_fresh_cases, with_game},
    sculk::TEST_POSTED_EVENT_ORDER,
};
use mod_api::{
    BlockState, BlockUpdateFlags, Flow, Hooks, HostBlockStates, ServerHost, VanillaGameEventEvent,
};
use std::sync::{Arc, Mutex};
use sunrise_client::{
    containers::BlockEntity,
    mobs::{Who, creaking},
};
use sunrise_core::{
    geometry::Vec3,
    random::{LegacyRandom, RandomSource},
};

const HOME: [i32; 3] = [8, 64, 8];

#[derive(Clone, Copy)]
struct Reaction {
    position: [f64; 3],
    peer_velocity: Option<[f64; 3]>,
}

#[derive(Default)]
struct Heard {
    mob: u32,
    peer: Option<u32>,
    reactions: Vec<Reaction>,
}

struct Watch {
    heard: Arc<Mutex<Heard>>,
    replace_protector: bool,
}

#[mod_api::__subscriptions]
impl Hooks for Watch {
    fn vanilla_game_event(
        &self,
        host: &mut dyn ServerHost,
        event: &mut VanillaGameEventEvent<'_>,
    ) -> Flow {
        if event.event != "minecraft:entity_action" {
            return Flow::Continue;
        }
        let peer = {
            let mut heard = self.heard.lock().unwrap();
            if event.cause != Some(heard.mob) {
                return Flow::Continue;
            }
            let mob = host.entity(heard.mob).unwrap();
            let peer = heard.peer.and_then(|id| host.entity(id));
            let peer_velocity = peer.and_then(|peer| peer.velocity(host));
            heard.reactions.push(Reaction {
                position: mob.position(host).unwrap(),
                peer_velocity,
            });
            peer
        };
        if let Some(peer) = peer {
            // Move the already-selected peer to the opposite side, out of range.
            assert!(peer.teleport_to(host, [3.5, 64.0, 4.5], None));
        }
        if self.replace_protector {
            let air = mod_api::__private::block_key("minecraft:air").unwrap();
            let state = BlockState::of(&host.block_states(), air, &[]).unwrap();
            assert_eq!(
                host.level(event.dimension).unwrap().set_block(
                    host,
                    HOME,
                    state,
                    BlockUpdateFlags::SKIP_BLOCK_ENTITY_SIDE_EFFECTS,
                ),
                Ok(true),
            );
        }
        Flow::Continue
    }
}

#[test]
#[ignore = "daily"]
fn borrowed_creaking_hurt_finishes_before_the_tick_continues() {
    const CASE: &str = "SUNRISE_BORROWED_CREAKING_CASE";
    if run_fresh_cases(
        "game::commands::world_cmds::registered_damage_tests::creaking_borrowed::borrowed_creaking_hurt_finishes_before_the_tick_continues",
        CASE,
        &[
            "expiry",
            "offline",
            "protector",
            "mismatched_protector",
            "cramming",
        ],
    ) {
        return;
    }
    let case = std::env::var(CASE).unwrap();
    let heard = Arc::new(Mutex::new(Heard::default()));
    let mods = vec![mod_api::ModEntry::new(
        "watch",
        Box::new(Watch {
            heard: heard.clone(),
            replace_protector: case == "protector",
        }),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_game(|game| {
            setup_live_damage_world(game);
            for command in [
                "forceload add 0 0",
                "time set night",
                "summon creaking 4.5 64 4.5 {UUID:[I;86,0,0,1],NoAI:1b,NoGravity:1b}",
                "setblock 8 63 8 minecraft:pale_oak_log[axis=y]",
                "setblock 8 64 8 minecraft:creaking_heart[axis=y,creaking_heart_state=awake]{creaking:[I;86,0,0,1]}",
                "setblock 8 65 8 minecraft:pale_oak_log[axis=y]",
            ] {
                assert!(game.run_command(command), "{command}");
            }
            game.load_ticking_neighbourhood(0, 0);
            game.update_chunk_tickets();
            assert!(game.entity_ticking_area()(4, 4));
            let creaking = game
                .mobs
                .iter_mut()
                .find(|m| creaking::is_creaking(m))
                .unwrap();
            let mob = creaking.id;
            creaking::set_transient(creaking, HOME);
            creaking.ai.last_hurt_by_player = 1;
            heard.lock().unwrap().mob = mob;
            game.remember_mob_player_credit(mob);
            assert!(game.last_hurt_by_player.contains_key(&mob));
            if case == "offline" {
                game.last_hurt_by_player.insert(mob, PlayerCredit(0x23));
            }
            if case == "mismatched_protector" {
                let Some(BlockEntity::CreakingHeart(heart)) =
                    game.world.block_entities.get_mut(&(8, 64, 8))
                else {
                    panic!("missing heart");
                };
                heart.creaking = Some((None, [86, 0, 0, 99]));
            }
            if case == "expiry" {
                // Vanilla keeps the reference when the clock goes 1 -> 0. The
                // following tick's in-wall check is still before reference expiry.
                game.tick_creature_entities().unwrap();
                assert_eq!(game.mob_object(mob).unwrap().ai.last_hurt_by_player, 0);
                assert!(game.last_hurt_by_player.contains_key(&mob));
            }
            let peer = if case == "cramming" {
                game.set_difficulty_forced(2);
                assert!(game.run_command("gamerule minecraft:max_entity_cramming 1"));
                assert_eq!(game.mob_world_base(&[]).max_entity_cramming, 1);
                let peer = game
                    .spawn_mob_raw("minecraft:pig", [5.65, 64.0, 4.5], true)
                    .unwrap();
                let pig = game.mob_object_mut(peer).unwrap();
                pig.no_ai = true;
                pig.no_gravity = true;
                let pig_bounds = pig.bounds();
                assert!(
                    !game
                        .mob_object(mob)
                        .unwrap()
                        .bounds()
                        .intersects(pig_bounds)
                );
                heard.lock().unwrap().peer = Some(peer);
                // Mob.baseTick's ambient draw, then the pushEntities cramming draw.
                let seed = (0..100)
                    .find(|&seed| {
                        let mut rng = LegacyRandom::new(seed);
                        let _ = rng.next_int_bound(1000);
                        rng.next_int_bound(4) == Ok(0)
                    })
                    .unwrap();
                let creaking = game.mob_object_mut(mob).unwrap();
                creaking.random = LegacyRandom::new(seed);
                // Keep authoritative travel but skip brain/navigation draws so
                // the ambient and cramming rolls are the only mob RNG work.
                creaking.no_ai = false;
                creaking.ai.ai_paused = true;
                creaking.velocity = Vec3::new(0.8, 0.0, 0.0);
                Some(peer)
            } else {
                let y = game.mob_object(mob).unwrap().eye_y().floor() as i32;
                assert!(game.run_command(&format!("setblock 4 {y} 4 minecraft:stone")));
                None
            };

            game.lan_entity_events.clear();
            game.audio.capture = Some(Vec::new());
            TEST_POSTED_EVENT_ORDER.with(|order| *order.borrow_mut() = Some(Vec::new()));
            let live_reaction = |game: &Game<'_>| {
                let flap = (mob as i32, 66);
                let flashes = game
                    .lan_entity_events
                    .iter()
                    .filter(|&&entry| entry == flap)
                    .count();
                let sways = game
                    .audio
                    .capture
                    .as_ref()
                    .unwrap()
                    .iter()
                    .filter(|sound| sound.event == "minecraft:entity.creaking.sway")
                    .count();
                (flashes, sways)
            };
            game.tick_creature_entities().unwrap();
            let reacted = usize::from(!matches!(case.as_str(), "offline" | "mismatched_protector"));
            let swayed = if case == "protector" { 0 } else { reacted };
            let reaction = live_reaction(game);
            let creaking = game.mob_object(mob).unwrap_or_else(|| {
                let residents: Vec<_> = game.mobs.iter().map(|m| (m.id, &m.kind.id, m.position)).collect();
                panic!("{case}: Creaking {mob} left the level, reaction {reaction:?}, resident mobs {residents:?}");
            });
            assert_eq!(
                reaction,
                (reacted, swayed),
                "{case}: at {:?}, velocity {:?}",
                creaking.position,
                creaking.velocity
            );
            assert_eq!(creaking.alive(), case != "mismatched_protector");
            let phase = if peer.is_some() { 8 } else { 7 };
            assert_eq!(creaking.extra.ints[0], if reacted != 0 { phase } else { 0 });
            let first = heard.lock().unwrap().reactions.clone();
            assert_eq!(first.len(), reacted);
            if let Some(peer) = peer {
                let hit = first[0];
                assert!(
                    hit.position[0] > 5.0,
                    "the mob has travelled when cramming hurts it"
                );
                assert_eq!(
                    hit.peer_velocity,
                    Some([0.0; 3]),
                    "pushes happen after the hurt"
                );
                assert!(
                    game.mob_object(peer).unwrap().velocity.x < 0.0,
                    "resume pushes the selected peer using its new position"
                );
            }
            let heart = game.world.block_entities.get(&(8, 64, 8));
            if case == "protector" {
                let state = game.world.block(8, 64, 8);
                assert_eq!(
                    game.pack.assets.registry.state(state).unwrap().name,
                    "minecraft:air"
                );
                // The owning block-entity tick can still hold the stale row;
                // replacing the block already stopped it from responding.
                if let Some(BlockEntity::CreakingHeart(heart)) = heart {
                    assert_eq!(heart.emitter, 0);
                }
            } else {
                let Some(BlockEntity::CreakingHeart(heart)) = heart else {
                    panic!("the protecting heart is gone");
                };
                assert_eq!(heart.emitter, 100 * swayed as i32);
            }
            TEST_POSTED_EVENT_ORDER.with(|order| {
                let order = order.borrow();
                let action: Vec<_> = order
                    .as_ref()
                    .unwrap()
                    .iter()
                    .filter(|(event, source, ..)| {
                        *event == "minecraft:entity_action" && *source == Some(Who::Mob(mob))
                    })
                    .collect();
                assert_eq!(action.len(), reacted);
                if let Some((_, _, flashes, sounds)) = action.first() {
                    assert_eq!((*flashes, *sounds), (1, 0));
                }
            });
            if case == "expiry" {
                assert!(!game.last_hurt_by_player.contains_key(&mob));
                // Remove cooldown, but leave the wall in place. With no reference
                // there is neither a replay nor another environmental reaction.
                game.mob_object_mut(mob).unwrap().extra.ints[0] = 0;
            }
            game.tick_creature_entities().unwrap();
            assert_eq!(live_reaction(game), (reacted, swayed));
            assert_eq!(heard.lock().unwrap().reactions.len(), reacted);
            TEST_POSTED_EVENT_ORDER.with(|order| *order.borrow_mut() = None);
        })
    });
}
