//! Resumable borrowed mob-tick region; the caller owns final restoration.
use super::*;

/// Route each player event to the player selected when it was emitted.
/// Untagged events retain the previous per-tick guest binding.
pub(in crate::game) fn route_player_events(
    events: &mut Vec<MobEvent>,
    from: usize,
    seen_guest: Option<u32>,
    guest_ids: &[(u32, u32)],
    guest_events: &mut Vec<(u32, u32, MobEvent)>,
    mob: u32,
) {
    let mut k = from;
    while k < events.len() {
        let (conn, tagged) = match &events[k] {
            MobEvent::ForPlayer { conn, .. } => (*conn, true),
            _ => (seen_guest, false),
        };
        if tagged {
            let MobEvent::ForPlayer { event, .. } = events.remove(k) else {
                unreachable!();
            };
            events.insert(k, *event);
        }
        // A hoglin broadcast carries the source's player, not the receiving
        // hoglin's independently selected player.
        if let MobEvent::HoglinAttackTarget {
            target: Who::Player,
            player,
            ..
        } = &mut events[k]
        {
            *player = conn;
        }
        // ShulkerBullet stores a target for later projectile ticks. Preserve
        // the selected guest's entity ID instead of the host-player sentinel.
        if let Some(conn) = conn
            && let MobEvent::Projectile(p) = &mut events[k]
            && let ProjectileKind::ShulkerBullet { target, .. } = &mut p.kind
            && *target == Some(Who::Player)
        {
            *target = guest_ids
                .iter()
                .find(|g| g.0 == conn)
                .map(|g| Who::Mob(g.1));
        }
        if let Some(conn) = conn
            && super::super::lan_mobs::acts_on_player(&events[k])
        {
            guest_events.push((conn, mob, events.remove(k)));
        } else {
            k += 1;
        }
    }
}

/// Commit departures in the borrowed list, not when a tick merely proposes retention.
/// In particular an interrupted pass keeps the credit of every row it gives back.
pub(in crate::game) fn retain_mob_rows(
    mobs: &mut sunrise_sim::join_seq::JoinList<Mob>,
    player_credit: &mut std::collections::HashMap<u32, super::super::combat::PlayerCredit>,
    mut keep: impl FnMut(&Mob) -> bool,
) {
    mobs.retain(|mob| {
        let kept = keep(mob);
        if !kept {
            player_credit.remove(&mob.id);
        }
        kept
    });
}

pub(in crate::game) struct BorrowedMobTick<'a> {
    pub(in crate::game) mobs: &'a mut sunrise_sim::join_seq::JoinList<Mob>,
    pub(in crate::game) snapshot: &'a mut Vec<Seen>,
    pub(in crate::game) passenger_objects: &'a [Seen],
    pub(in crate::game) blocked: &'a std::collections::HashSet<u32>,
    pub(in crate::game) events: &'a mut Vec<MobEvent>,
    pub(in crate::game) restocks: &'a mut Vec<sunrise_sim::mobs::villager_brain::RestockNotice>,
    pub(in crate::game) pushes: &'a mut sunrise_sim::mobs::PendingPushes,
    pub(in crate::game) dead: &'a mut Vec<Mob>,
    pub(in crate::game) corpses: &'a mut Vec<Mob>,
    pub(in crate::game) died: &'a mut sunrise_level::fast_hash::FxHashSet<u32>,
    pub(in crate::game) ticked: &'a mut sunrise_level::fast_hash::FxHashSet<u32>,
    pub(in crate::game) guests: &'a mut super::super::lan_mobs::GuestViews,
    pub(in crate::game) spare: &'a mut Option<PlayerView>,
    pub(in crate::game) bindings: &'a mut sunrise_level::fast_hash::FxHashMap<u32, u32>,
    pub(in crate::game) guest_ids: &'a mut Vec<(u32, u32)>,
    pub(in crate::game) guest_events: &'a mut Vec<(u32, u32, MobEvent)>,
    pub(in crate::game) player_credit:
        &'a mut std::collections::HashMap<u32, super::super::combat::PlayerCredit>,
    pub(in crate::game) path_writes: &'a mut Vec<((i32, i32, i32), u32, u32)>,
    pub(in crate::game) gone: &'a mut Vec<(u32, Vec3, f64, f64, Option<Mob>)>,
    pub(in crate::game) lent: sunrise_sim::health::LentLevel,
    pub(in crate::game) laps: &'a mut sunrise_level::spans::Laps,
    pub(in crate::game) uuid: Option<[i32; 4]>,
    pub(in crate::game) level_id: &'static str,
    pub(in crate::game) round: &'a mut super::super::native_ticks::TickRound,
    pub(in crate::game) stand_result: &'a mut Result<()>,
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    pub(in crate::game) fn retained_passenger_snapshot(&self, uuid: Option<[i32; 4]>) -> Vec<Seen> {
        if self.persist.off_level.is_empty() {
            Vec::new()
        } else {
            self.persist
                .off_level
                .values()
                .filter_map(crate::game::persist::OffLevel::as_mob)
                .map(|m| seen(m, uuid, self.items()))
                .collect()
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod off_level_tests {
    use super::*;

    #[test]
    #[ignore = "daily"]
    fn borrowed_tick_resolves_the_retained_controller() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            use sunrise_sim::mobs::goals::{JUMP, LOOK, MOVE};
            game.options.dedicated = Some(Vec::new());
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            let spider = game
                .res
                .mob_types()
                .get("minecraft:spider")
                .unwrap()
                .clone();
            let skeleton = game
                .res
                .mob_types()
                .get("minecraft:skeleton")
                .unwrap()
                .clone();
            let at = Vec3::new(1.0, 80.0, 0.5);
            let mut vehicle = Mob::new(71, spider, at, 0.0);
            vehicle.extra.rider = Some(72);
            let mut rider = Mob::new(72, skeleton, at, 0.0);
            rider.extra.mount = Some(71);
            rider.age = 19;
            game.mobs.push(vehicle);
            game.persist
                .off_level
                .insert(72, crate::game::persist::OffLevel::Mob(rider));
            game.tick_creature_entities().unwrap();
            assert_eq!(
                game.mob_object(71).unwrap().ai.goals.disabled & (MOVE | JUMP | LOOK),
                MOVE | JUMP | LOOK
            );
            assert_eq!(game.mob_object(72).unwrap().age, 19);
            assert_eq!(game.mobs.len(), 1);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn resident_descendant_does_not_tick_until_detached() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            let pig = game.res.mob_types().get("minecraft:pig").unwrap().clone();
            let mut parent = Mob::new(72, pig.clone(), Vec3::new(1.0, 80.0, 0.5), 0.0);
            parent.extra.mount = Some(71);
            let mut child = Mob::new(73, pig, parent.position, 0.0);
            child.extra.mount = Some(72);
            child.no_ai = true;
            child.age = 19;
            child.fire_ticks = 20;
            game.persist
                .off_level
                .insert(72, crate::game::persist::OffLevel::Mob(parent));
            game.mobs.push(child);
            game.tick_creature_entities().unwrap();
            let child = game.mob_object(73).unwrap();
            assert_eq!(child.age, 19);
            assert_eq!(child.fire_ticks, 20);
            game.mob_object_mut(73).unwrap().extra.mount = None;
            game.tick_creature_entities().unwrap();
            assert_eq!(game.mob_object(73).unwrap().age, 20);
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod hoglin_routing_tests {
    use super::*;

    #[test]
    fn broadcast_keeps_source_player_connection() {
        for (seen_guest, tag, expected) in [
            (Some(12), None, Some(12)),
            (None, None, None),
            (Some(12), Some(Some(13)), Some(13)),
            (Some(12), Some(None), None),
        ] {
            let event = MobEvent::HoglinAttackTarget {
                mob: 9001,
                target: Who::Player,
                player: None,
            };
            let mut events = vec![match tag {
                Some(conn) => MobEvent::ForPlayer {
                    conn,
                    event: Box::new(event),
                },
                None => event,
            }];
            route_player_events(&mut events, 0, seen_guest, &[], &mut Vec::new(), 9000);
            assert!(matches!(events[0], MobEvent::HoglinAttackTarget {
                player, ..
            } if player == expected));
        }
    }
}

#[cfg(test)]
#[path = "mob_tick_access_tests.rs"]
pub(in crate::game) mod mob_tick_access_tests;
