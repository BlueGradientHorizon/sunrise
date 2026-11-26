

#![cfg(test)]
use sunrise_items::items::ItemStack;
use sunrise_sim::mobs::Mob;
#[cfg(test)]
pub(crate) mod tests {

use mod_api::{AgeableGroup, EquipmentSlot, Identifier, SpawnGroup, UnspawnedMobError, UnspawnedMobRef};

use sunrise_gameplay_common::game::server_side::native_unspawned_mob::{KeptGoalMod, pumpkin};

use sunrise_gameplay_common::game::server_side::native_unspawned_mob::tests::AT;

use sunrise_gameplay_common::game::server_side::native_unspawned_mob::tests::id;

type Listen = dyn Fn(&dyn Host, &mut FinalizeSpawnEvent<'_>) -> Flow + Send + Sync;
/// Answers `finalize_spawn` with the test's closure.
    struct Listener(Box<Listen>);
#[mod_api::__subscriptions]
    impl mod_api::Hooks for Listener {
        fn finalize_spawn(&self, host: &dyn Host, event: &mut FinalizeSpawnEvent<'_>) -> Flow {
            (self.0)(host, event)
        }
    }
fn listening<R>(
        listen: impl Fn(&dyn Host, &mut FinalizeSpawnEvent<'_>) -> Flow + Send + Sync + 'static,
        run: impl FnOnce() -> R,
    ) -> R {
        let listener = Listener(Box::new(listen));
        mod_platform::testing::with_mods(
            vec![mod_api::ModEntry::new("unspawned", Box::new(listener))].into(),
            run,
        )
    }

use sunrise_gameplay::game::server_side::native_unspawned_mob::tests::spawn_pig;

use sunrise_gameplay_common::game::commands_common::Target;

use super::super::native_entity_refs::tests::with_game;

use super::*;

use mod_api::{FinalizeSpawnEvent, Flow, Host};

use std::sync::{Arc, Mutex};

use sunrise_sim::mobs::finalize::{self, Attr};

/// What a listener edits through the editor is on the mob that joins the
    /// level, and the handle reads the edits back inside the call.
    #[test]
    #[ignore = "daily"]
    fn edits_through_the_editor_are_on_the_mob_that_joins() {
        type Seen = (
            Result<Identifier, UnspawnedMobError>,
            Result<f64, UnspawnedMobError>,
            Result<Option<ItemStack>, UnspawnedMobError>,
        );
        let seen: Arc<Mutex<Option<Seen>>> = Arc::new(Mutex::new(None));
        let saw = seen.clone();
        let max_health = id("minecraft:max_health");
        listening(
            move |host, event| {
                let (mob, edits) = (event.mob, event.edits);
                edits
                    .set_equipment(mob, EquipmentSlot::Head, Some(pumpkin()))
                    .unwrap();
                let mut oversized = pumpkin();
                oversized.set_count(100).unwrap();
                assert_eq!(
                    edits.set_equipment(mob, EquipmentSlot::Head, Some(oversized)),
                    Err(UnspawnedMobError::Invalid)
                );
                assert_eq!(
                    mob.equipment(host, EquipmentSlot::Head),
                    Ok(Some(pumpkin()))
                );
                edits.set_equipment(mob, EquipmentSlot::Head, None).unwrap();
                assert_eq!(mob.equipment(host, EquipmentSlot::Head), Ok(None));
                edits
                    .set_equipment(mob, EquipmentSlot::Head, Some(pumpkin()))
                    .unwrap();
                edits.set_attribute_base(mob, &max_health, 20.0).unwrap();
                assert_eq!(edits.add_tag(mob, "test:tagged"), Ok(true));
                edits
                    .set_custom_name(mob, Some(mod_api::Text::literal("Bacon")))
                    .unwrap();
                *saw.lock().unwrap() = Some((
                    mob.kind(host),
                    mob.attribute_base(host, &max_health),
                    mob.equipment(host, EquipmentSlot::Head),
                ));
                Flow::Continue
            },
            || {
                with_game(|game| {
                    let spawned = spawn_pig(game).unwrap();
                    let m = game.mobs.iter().find(|m| m.id == spawned).unwrap();
                    let head = &m.equipment_stacks()[EquipmentSlot::Head as usize];
                    assert_eq!(
                        head.as_ref().map(|s| &*s.item),
                        Some("minecraft:carved_pumpkin")
                    );
                    assert_eq!(finalize::base_value(m, Attr::MaxHealth), 20.0);
                    assert_eq!(m.custom_name.as_deref(), Some("Bacon"));
                    assert_eq!(game.tags_of(Target::Mob(spawned)), ["test:tagged"]);
                });
                // Item keys resolve with the game inside this scope, so the stack is read here.
                let (kind, base, head) = seen.lock().unwrap().take().expect("the hook ran");
                assert_eq!(kind, Ok(id("minecraft:pig")));
                assert_eq!(base, Ok(20.0));
                assert_eq!(head, Ok(Some(pumpkin())));
            },
        );
    }
/// A mod's base written through the editor stays in the attribute's range, as documented.
    #[test]
    #[ignore = "daily"]
    fn a_base_written_through_the_editor_stays_in_range() {
        let seen: Arc<Mutex<Option<Result<f64, UnspawnedMobError>>>> = Arc::new(Mutex::new(None));
        let saw = seen.clone();
        let max_health = id("minecraft:max_health");
        listening(
            move |host, event| {
                let (mob, edits) = (event.mob, event.edits);
                edits.set_attribute_base(mob, &max_health, 2000.0).unwrap();
                *saw.lock().unwrap() = Some(mob.attribute_base(host, &max_health));
                Flow::Continue
            },
            || {
                with_game(|game| {
                    let spawned = spawn_pig(game).unwrap();
                    let m = game.mobs.iter().find(|m| m.id == spawned).unwrap();
                    assert_eq!(finalize::base_value(m, Attr::MaxHealth), 1024.0);
                });
                assert_eq!(seen.lock().unwrap().take(), Some(Ok(1024.0)));
            },
        );
    }
/// A handle kept past its hook reads `Expired`, and the next call's
    /// editor refuses it the same way: it reaches only its own lend.
    #[test]
    #[ignore = "daily"]
    fn a_handle_kept_past_its_hook_is_expired() {
        let kept: Arc<Mutex<Vec<UnspawnedMobRef>>> = Arc::default();
        let refused = Arc::new(Mutex::new(Vec::new()));
        let (keep, refuse) = (kept.clone(), refused.clone());
        listening(
            move |_, event| {
                let earlier = keep.lock().unwrap().last().copied();
                if let Some(earlier) = earlier {
                    refuse
                        .lock()
                        .unwrap()
                        .push(event.edits.add_tag(earlier, "test:late"));
                }
                keep.lock().unwrap().push(event.mob);
                Flow::Continue
            },
            || {
                with_game(|game| {
                    let first = spawn_pig(game).unwrap();
                    spawn_pig(game).unwrap();
                    let old = kept.lock().unwrap()[0];
                    assert_eq!(old.kind(game), Err(UnspawnedMobError::Expired));
                    assert_eq!(old.position(game), Err(UnspawnedMobError::Expired));
                    assert!(game.tags_of(Target::Mob(first)).is_empty());
                });
            },
        );
        assert_eq!(*refused.lock().unwrap(), [Err(UnspawnedMobError::Expired)]);
        let kept = kept.lock().unwrap();
        assert_ne!(kept[0], kept[1], "each call lends anew");
    }
/// `Flow::Cancel` skips finalization and keeps the listener's edits on
    /// the mob that still joins; `spawn_cancelled` adds no mob at all.
    #[test]
    #[ignore = "daily"]
    fn both_cancels_keep_their_contract_with_edits() {
        let max_health = id("minecraft:max_health");
        listening(
            move |_, event| {
                event
                    .edits
                    .set_attribute_base(event.mob, &max_health, 20.0)
                    .unwrap();
                Flow::Cancel
            },
            || {
                with_game(|game| {
                    let spawned = spawn_pig(game).expect("a skipped finalization still spawns");
                    let m = game.mobs.iter().find(|m| m.id == spawned).unwrap();
                    assert_eq!(finalize::base_value(m, Attr::MaxHealth), 20.0);
                    assert!(
                        !finalize::has_modifier(
                            m,
                            Attr::FollowRange,
                            "minecraft:random_spawn_bonus"
                        ),
                        "no finalizeSpawn ran"
                    );
                });
            },
        );
        listening(
            |_, event| {
                assert_eq!(event.edits.add_tag(event.mob, "test:gone"), Ok(true));
                event.spawn_cancelled = true;
                Flow::Continue
            },
            || {
                with_game(|game| {
                    let before = game.mobs.len();
                    assert_eq!(spawn_pig(game), None);
                    assert_eq!(game.mobs.len(), before, "a stopped spawn adds no mob");
                });
            },
        );
    }
/// Every live site hands `group` over empty today; group data a listener
    /// leaves there is what finalization reads: an ageable group whose later
    /// members are all babies makes this pig one.
    #[test]
    #[ignore = "daily"]
    fn the_group_a_listener_leaves_is_what_finalization_reads() {
        with_game(|game| {
            let alone = spawn_pig(game).unwrap();
            let m = game.mobs.iter().find(|m| m.id == alone).unwrap();
            assert!(!m.is_baby(), "a pack's first member is never a baby");
        });
        let given = Arc::new(Mutex::new(Vec::new()));
        let heard = given.clone();
        listening(
            move |_, event| {
                heard.lock().unwrap().push(event.group.clone());
                let mut ageable = AgeableGroup::new(true, 1.0);
                ageable.size = 1;
                *event.group = Some(SpawnGroup::Ageable(ageable));
                Flow::Continue
            },
            || {
                with_game(|game| {
                    let spawned = spawn_pig(game).unwrap();
                    let m = game.mobs.iter().find(|m| m.id == spawned).unwrap();
                    assert!(m.is_baby());
                });
            },
        );
        assert_eq!(*given.lock().unwrap(), [None]);
    }
/// A listener that panics still ends its lend: the handle is withdrawn
    /// and the mob in the caller's slot is the one that was lent, mod goal
    /// and all, not a copy (whose mod goal would have no goal) or a default.
    #[test]
    #[ignore = "daily"]
    fn a_panicking_listener_still_ends_the_lend() {
        let installed = || vec![mod_api::ModEntry::new("kept-goal", Box::new(KeptGoalMod))];
        mod_platform::testing::with_mods(installed().into(), || {
            sunrise_gameplay_common::game::api_values::install_parts(&installed()).unwrap();
            with_game(|game| {
                let kind = game
                    .res
                    .mob_types()
                    .get("minecraft:zombie")
                    .unwrap()
                    .clone();
                let mut zombie = Mob::new(70, kind, sunrise_core::geometry::Vec3::ZERO, 0.0);
                zombie.custom_name = Some("Lent".to_owned());
                let goals = |m: &Mob| -> Vec<&'static str> {
                    m.ai.goals.entries().map(|(_, name, _)| name).collect()
                };
                assert!(
                    goals(&zombie).contains(&"Kept"),
                    "the part attached its goal"
                );
                let mut kept = None;
                let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    game.lend_unspawned("minecraft:zombie", AT, Some(&mut zombie), |mob, _| {
                        kept = Some(mob);
                        panic!("a listener's bug");
                    })
                }));
                assert!(panicked.is_err());
                assert!(
                    game.server.lent_mobs.0.borrow().lends.is_empty(),
                    "the lend is withdrawn"
                );
                let kept = kept.expect("the call ran");
                assert_eq!(kept.kind(game), Err(UnspawnedMobError::Expired));
                assert_eq!(zombie.id, 70);
                assert_eq!(zombie.custom_name.as_deref(), Some("Lent"));
                let back = goals(&zombie);
                assert!(
                    back.contains(&"Kept"),
                    "the mod goal keeps its goal: {back:?}"
                );
                assert!(!back.contains(&"ModGoal"), "no copied mod goal: {back:?}");
            });
            sunrise_gameplay_common::game::api_values::install_parts(&[]).unwrap();
        });
    }
}
