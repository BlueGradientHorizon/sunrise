//! Evoker fangs (`EvokerFangs`): rising out of the floor after a delay,
//! biting what stands on them for 6 damage, sinking back after 22 ticks.
use super::Game;
use crate::game::fangs_common::{Fangs, fang_attack_pitch};
use sunrise_core::geometry::Vec3;
use sunrise_sim::mobs::Species;

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn add_fangs(&mut self, at: [f64; 3], yaw: f32, delay: i32, owner: u32) {
        self.side.next_entity += 1;
        self.insert_fangs(self.side.next_entity, at, yaw, delay, owner);
    }

    pub(in crate::game::server_side) fn fangs_to_nbt(
        &self,
        f: &Fangs,
        uuid: [i32; 4],
    ) -> sunrise_core::nbt::Tag {
        sunrise_sim::entity_nbt::evoker_fangs_to_nbt(
            Vec3::new(f.pos[0], f.pos[1], f.pos[2]),
            f.motion,
            f.fire_ticks,
            f.yaw,
            f.warmup,
            self.persist.uuids.get(&f.owner).copied(),
            uuid,
            self.persist.raw.get(&f.id),
        )
    }
    pub(in crate::game::server_side) fn loaded_fangs_owner(
        &self,
        tag: &sunrise_core::nbt::Tag,
    ) -> u32 {
        match tag.get("Owner") {
            Some(sunrise_core::nbt::Tag::IntArray(v)) => v.as_slice().try_into().ok(),
            _ => None,
        }
        .and_then(|u| self.mob_by_uuid(u))
        .unwrap_or(0)
    }

    /// `EvokerFangs.tick`.
    pub(in crate::game) fn tick_fangs(&mut self) {
        sunrise_threads::work::clear_work();
        // ServerLevel.tick: only entities in entity-ticking chunks that joined before the pass tick.
        let ticking = self.entity_ticking_area();
        let through = self.side.ticked_entities_through;
        let mut list = self.fangs.list.lift();
        #[cfg(test)]
        crate::game::server_side::entity_server::taken_pass_tests::after_take(self);
        list.retain_mut(|f| {
            if f.id > through || !ticking(f.pos[0].floor() as i32, f.pos[2].floor() as i32) {
                return true;
            }
            self.fangs_fluid_step(f);
            // Entity.baseTick: the fangs burn down; their hurt does nothing.
            if f.fire_ticks > 0 {
                f.fire_ticks -= 1;
            }
            f.synced_fire = f.fire_ticks > 0 || f.has_visual_fire;
            f.warmup = f.warmup.wrapping_sub(1);
            if f.warmup >= 0 {
                return true;
            }
            if f.warmup == -8 {
                self.fangs_bite(f);
            }
            if !f.started {
                f.started = true;
                self.lan_entity_event(f.id as i32, 4, f.pos);
                let _pitch = fang_attack_pitch(&mut f.random);
            }
            f.life -= 1;
            f.life >= 0
        });
        self.fangs
            .list
            .restore(list, sunrise_sim::join_seq::RestoreOrder::SurvivorsFirst);
    }
    /// `Entity.baseTick`'s `updateFluidInteraction` for fangs (0.5F x 0.8F): the
    /// currents of the water and lava the box touches add to the motion.
    pub(in crate::game::server_side) fn fangs_fluid_step(&self, f: &mut Fangs) {
        let at = Vec3::new(f.pos[0], f.pos[1], f.pos[2]);
        let bounds = sunrise_core::geometry::Aabb::new(
            Vec3::new(at.x - 0.25, at.y, at.z - 0.25),
            Vec3::new(at.x + 0.25, at.y + f64::from(0.8_f32), at.z + 0.25),
        );
        // EntityDimensions.scalable: the eye at 0.85 of the height.
        let eye = at.y + f64::from(0.8_f32 * 0.85);
        sunrise_sim::entities::update_entity_fluids(
            bounds,
            at,
            eye,
            &mut f.motion,
            &self.world,
            &self.res.motion().fluids,
            &self.loop_context,
        );
    }
    /// `dealDamageTo` everything alive in the fangs' box (0.5 x 0.8,
    /// inflated 0.2 sideways) but the evoker's allies.
    pub(in crate::game::server_side) fn fangs_bite(&mut self, f: &Fangs) {
        let half = 0.25 + 0.2;
        let lo = Vec3::new(f.pos[0] - half, f.pos[1], f.pos[2] - half);
        let hi = Vec3::new(f.pos[0] + half, f.pos[1] + 0.8, f.pos[2] + half);
        let b = sunrise_core::geometry::Aabb::new(lo, hi);
        // Every player in the box, the server's players too.
        let pos = f.pos;
        // `EvokerFangs.dealDamageTo`: indirectMagic(fangs, owner) while the
        // owner lives, plain magic (nobody to name) once it is gone.
        let owner = self
            .mobs
            .iter()
            .any(|m| m.id == f.owner && m.alive())
            .then_some(f.owner);
        let bite = move |g: &mut Self| {
            if let Some(p) = g.player_view()
                && p.alive
                && p.bounds.intersects(b)
            {
                let kind = match owner {
                    Some(_) => "minecraft:indirect_magic",
                    None => "minecraft:magic",
                };
                g.mob_hits_player_caused(6.0, pos, kind, owner);
            }
        };
        bite(self);
        self.each_guest_hit(bite);
        // `Entity.isAlliedTo`: the owner's or the target's
        // `considersEntityAsAlly`, teams included.
        let teams = self.mob_teams();
        let illagers: Vec<u32> = self
            .mobs
            .iter()
            .filter(|m| is_illager(m.species()))
            .map(|m| m.id)
            .collect();
        // A missing owner hurts everyone (`currentOwner == null`).
        let owner_known = illagers.contains(&f.owner);
        let team_of = |id: u32| teams.of(sunrise_sim::mobs::Who::Mob(id));
        let allies: Vec<u32> = if owner_known {
            self.mobs
                .iter()
                .filter(|m| {
                    let other = Party {
                        illager: is_illager(m.species()),
                        team: team_of(m.id),
                    };
                    let root = (m.species() == Species::Vex)
                        .then_some(m.extra.counter2 as u32)
                        .filter(|r| illagers.contains(r))
                        .map(|r| {
                            (
                                r == f.owner,
                                Party {
                                    illager: true,
                                    team: team_of(r),
                                },
                            )
                        });
                    let owner = Party {
                        illager: true,
                        team: team_of(f.owner),
                    };
                    fangs_allied(owner, other, root)
                })
                .map(|m| m.id)
                .collect()
        } else {
            Vec::new()
        };
        let attacker = owner.map(sunrise_sim::mobs::Who::Mob);
        // A hurt can run server hooks that change the mob list. Keep the selected
        // victims by identity, as `EvokerFangs.tick` does before `dealDamageTo`.
        let victims: Vec<_> = self
            .mobs
            .iter()
            .filter(|m| m.id != f.owner && !allies.contains(&m.id) && m.bounds().intersects(b))
            .map(|m| m.id)
            .collect();
        for id in victims {
            let Some(index) = self.mob_index.find(&self.mobs, id) else {
                continue;
            };
            if !self.mobs[index].alive() {
                continue;
            }
            // So `living_knockback` sees who the fangs' owner is and what they hold.
            let attack = self.attack_of_who(attacker);
            let plan = self.prepare_knockback_credit(id as i32, attacker, &attack);
            // The hit's causing entity, lent to its listeners beside the victim.
            let causing = self.causer_id(attacker);
            let decision = crate::game::entity_server::causer_decision!(self, causing);
            let mut credit = plan.lend(&mut self.side.last_hurt_by_player);
            let (victim, lend) =
                crate::game::entity_server::victim_and_causer!(self, index, causing, decision);
            let hurt = victim.damage(
                crate::game::entity_server::damage_level_of!(self, &attack, &mut credit, lend),
                6.0,
                sunrise_sim::mobs::Damage {
                    kind: if owner.is_some() {
                        sunrise_sim::mobs::DamageKind::IndirectMagic
                    } else {
                        sunrise_sim::mobs::DamageKind::Magic
                    },
                    attacker,
                    from: Some(f.pos),
                    attacker_species: None,
                },
            );
            let Some((index, hurt)) = self.complete_mob_hurt(index, hurt) else {
                continue;
            };
            self.finish_mob_damage_at(index);
            self.settle_mob_hurt(id);
            if hurt && owner.is_some() {
                self.mob_post_attack(
                    id,
                    attacker,
                    Some("minecraft:evoker_fangs"),
                    Some("minecraft:indirect_magic"),
                );
            }
        }
    }
}
/// `EntityTypeTags.ILLAGER` members, the types of `ILLAGER_FRIENDS`.
pub(in crate::game::server_side) fn is_illager(s: Species) -> bool {
    matches!(
        s,
        Species::Evoker | Species::Illusioner | Species::Pillager | Species::Vindicator
    )
}
/// One side of an ally check: whether it is an illager and its team.
#[derive(Clone, Copy)]
pub(in crate::game) struct Party<'a> {
    pub(in crate::game) illager: bool,
    pub(in crate::game) team: Option<&'a sunrise_game_data::scoreboard::Team>,
}
/// `AbstractIllager.considersEntityAsAlly(other)`: the base rule (the teams
/// are allied), or an illager friend while both have no team.
pub(in crate::game::server_side) fn illager_considers(me: Party, other: Party) -> bool {
    sunrise_game_data::scoreboard::teams_allied(me.team, other.team)
        || other.illager && me.team.is_none() && other.team.is_none()
}
/// `owner.isAlliedTo(other)` for an evoker owner: `Evoker.considersEntityAsAlly`
/// either way round. `vex_root` is the root owner of a vex target (whether it
/// is the owner itself, and its party); the owner treats such a vex as an ally
/// when its root owner is.
pub(in crate::game::server_side) fn fangs_allied(
    owner: Party,
    other: Party,
    vex_root: Option<(bool, Party)>,
) -> bool {
    illager_considers(owner, other)
        || other.illager && illager_considers(other, owner)
        || !other.illager && sunrise_game_data::scoreboard::teams_allied(other.team, owner.team)
        || vex_root.is_some_and(|(is_owner, root)| is_owner || illager_considers(owner, root))
}
#[cfg(test)]
#[path = "fangs/current_tests.rs"]
pub(in crate::game) mod current_tests;

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::{Party, fangs_allied};
    use crate::game::client_side::fangs_client::fang_crit;
    use crate::game::fangs_common::fang_attack_pitch;
    use sunrise_core::random::LegacyRandom;

    #[test]
    #[ignore = "daily"]
    fn taken_fangs_tick_keeps_survivors_before_both_spawns() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.fangs.list.clear();
            game.add_fangs([0.0, 80.0, 0.0], 0.0, 30, 0);
            game.add_fangs([1.0, 80.0, 0.0], 0.0, 30, 0);
            let first = game.fangs.list[0].id;
            let second = game.fangs.list[1].id;
            let next = game.side.next_entity;
            crate::game::server_side::entity_server::taken_pass_tests::with_spawn(
                |game| {
                    assert!(game.fangs.list.is_empty());
                    game.add_fangs([2.0, 80.0, 0.0], 0.0, 40, 0);
                    game.add_fangs([3.0, 80.0, 0.0], 0.0, 50, 0);
                },
                || game.tick_fangs(),
            );
            assert_eq!(
                game.fangs
                    .list
                    .iter()
                    .map(|f| (f.id, f.warmup))
                    .collect::<Vec<_>>(),
                [(first, 29), (second, 29), (next + 1, 40), (next + 2, 50)]
            );
        });
    }

    #[test]
    fn fang_attack_pitch_matches_the_vanilla_span() {
        // Java Random seed 1, first nextFloat() * 0.2 + 0.85, in f32.
        let mut random = LegacyRandom::new(1);
        let pitch = fang_attack_pitch(&mut random);
        assert!((pitch - 0.996_175_65).abs() < 1.0e-6, "{pitch}");
        assert!((0.85..1.05).contains(&pitch));
    }

    #[test]
    fn fang_crit_sits_above_the_jaws() {
        // Java Random seed 1, the first crit of the twelve. The y is
        // getY() + 0.05 + nextDouble(), then addParticle adds 1.
        let mut random = LegacyRandom::new(1);
        let (at, velocity) = fang_crit(&mut random, [2.0, 70.0, -4.0]);
        let expected_at = [
            2.115_439_095_351_645_5,
            71.460_080_811_492_2,
            -4.146_142_579_345_142,
        ];
        let expected_velocity = [
            -0.100_369_766_424_293_3,
            0.590_326_772_827_236_1,
            -0.296_329_690_640_543_2,
        ];
        for i in 0..3 {
            assert!((at[i] - expected_at[i]).abs() < 1.0e-12, "{at:?}");
            assert!(
                (velocity[i] - expected_velocity[i]).abs() < 1.0e-12,
                "{velocity:?}"
            );
        }
    }

    fn party(illager: bool, team: Option<&sunrise_game_data::scoreboard::Team>) -> Party<'_> {
        Party { illager, team }
    }

    #[test]
    fn fangs_allies_follow_teams_and_the_illager_tag() {
        let mut board = sunrise_game_data::scoreboard::Scoreboard::default();
        board.add_team("red");
        board.add_team("blue");
        let red = board.team("red").unwrap();
        let blue = board.team("blue").unwrap();
        let evoker = |t| party(true, t);
        // Teamless illagers are friends.
        assert!(fangs_allied(evoker(None), party(true, None), None));
        // Different teams: the illager tag no longer applies.
        assert!(!fangs_allied(
            evoker(Some(red)),
            party(true, Some(blue)),
            None
        ));
        // One side with a team: still not allied.
        assert!(!fangs_allied(evoker(Some(red)), party(true, None), None));
        // The same team allies a non-illager mob.
        assert!(fangs_allied(
            evoker(Some(red)),
            party(false, Some(red)),
            None
        ));
        assert!(!fangs_allied(evoker(None), party(false, None), None));
        // A vex follows its root owner.
        assert!(fangs_allied(
            evoker(None),
            party(false, None),
            Some((true, evoker(None)))
        ));
        assert!(!fangs_allied(
            evoker(Some(red)),
            party(false, None),
            Some((false, party(true, Some(blue))))
        ));
    }
}

#[cfg(test)]
#[path = "fangs/ticking_tests.rs"]
pub(in crate::game) mod ticking_tests;
