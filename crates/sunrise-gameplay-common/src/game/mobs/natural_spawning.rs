

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

#[cfg(test)]
/// Answers both spawn checks with one fixed result.
    struct ChecksMod(SpawnCheck);
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for ChecksMod {
        fn mob_spawn_placement_check(
            &self,
            _: &dyn mod_api::Host,
            event: &mut mod_api::MobSpawnPlacementCheckEvent<'_>,
        ) {
            *event.result = self.0;
        }
        fn mob_spawn_position_check(
            &self,
            _: &dyn mod_api::Host,
            event: &mut mod_api::MobSpawnPositionCheckEvent<'_>,
        ) {
            *event.result = self.0;
        }
    }
#[cfg(test)]
type MobReads = (
        Result<mod_api::Identifier, mod_api::UnspawnedMobError>,
        Result<[f64; 3], mod_api::UnspawnedMobError>,
        Result<bool, mod_api::UnspawnedMobError>,
    );
#[cfg(test)]
type Log = std::sync::Arc<std::sync::Mutex<Vec<Heard>>>;
#[cfg(test)]
/// Gives every natural monster position one entry: the sulfur cube, whose spawn predicate
    /// has no conditions, in packs of four.
    struct SulfurCubesOnly;
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for SulfurCubesOnly {
        fn potential_spawns(
            &self,
            _: &dyn mod_api::Host,
            event: &mut mod_api::PotentialSpawnsEvent<'_>,
        ) -> mod_api::Flow {
            if event.category == "monster" {
                *event.entries = vec![SpawnEntry {
                    entity: SULFUR_CUBE.into(),
                    weight: 1,
                    min: 4,
                    max: 4,
                    constant: true,
                }];
            }
            mod_api::Flow::Continue
        }
    }
pub const SULFUR_CUBE: &str = "minecraft:sulfur_cube";
#[cfg(test)]
/// A spawned mob as the draw comparisons see it: id, kind, position, yaw, persistence and
    /// the next draw of its own random (seeded from its id).
    type Snap = (u32, String, [f64; 3], f32, bool, i32);
#[cfg(test)]
fn snap(m: &Mob) -> Snap {
        (
            m.id,
            m.kind.id.clone(),
            [m.position.x, m.position.y, m.position.z],
            m.yaw,
            m.persistent,
            m.random.clone().next_int(),
        )
    }
/// Runs `go` with no mod (`None`), or with a position-check listener answering `answer`
    /// beside `extra` mods, and what that listener heard.
    pub fn with_checks<R>(
        answer: Option<SpawnCheck>,
        extra: impl Fn() -> Vec<mod_api::ModEntry>,
        go: impl FnOnce() -> R,
    ) -> (R, Vec<Heard>) {
        let log = Log::default();
        let mut mods = extra();
        if let Some(answer) = answer {
            mods.push(mod_api::ModEntry::new(
                "spawn-context",
                Box::new(ContextMod {
                    answer,
                    log: log.clone(),
                }),
            ));
        }
        let out = if mods.is_empty() {
            go()
        } else {
            mod_platform::testing::with_mods(mods.into(), go)
        };
        let heard = log.lock().unwrap().clone();
        (out, heard)
    }
#[cfg(test)]
/// Answers `finalize_spawn` the way a test asks.
    #[derive(Clone, Copy)]
    enum FinalizeAnswer {
        Pass,
        SkipFinalize,
        StopSpawn,
    }
#[cfg(test)]
struct FinalizeMod(FinalizeAnswer);
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for FinalizeMod {
        fn finalize_spawn(
            &self,
            _: &dyn mod_api::Host,
            event: &mut mod_api::FinalizeSpawnEvent<'_>,
        ) -> mod_api::Flow {
            match self.0 {
                FinalizeAnswer::Pass => {}
                FinalizeAnswer::SkipFinalize => return mod_api::Flow::Cancel,
                FinalizeAnswer::StopSpawn => event.spawn_cancelled = true,
            }
            mod_api::Flow::Continue
        }
    }
#[cfg(test)]
/// Counts the `finalize_spawn` events it hears.
    struct CountFinalize(std::sync::Arc<std::sync::atomic::AtomicUsize>);
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for CountFinalize {
        fn finalize_spawn(
            &self,
            _: &dyn mod_api::Host,
            _: &mut mod_api::FinalizeSpawnEvent<'_>,
        ) -> mod_api::Flow {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            mod_api::Flow::Continue
        }
    }
#[cfg(test)]
/// Records the kind and reason of every `finalize_spawn` event.
    struct RecordFinalize(std::sync::Arc<std::sync::Mutex<Vec<(String, mod_api::SpawnReason)>>>);
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for RecordFinalize {
        fn finalize_spawn(
            &self,
            _: &dyn mod_api::Host,
            event: &mut mod_api::FinalizeSpawnEvent<'_>,
        ) -> mod_api::Flow {
            self.0
                .lock()
                .unwrap()
                .push((event.kind.to_owned(), event.reason));
            mod_api::Flow::Continue
        }
    }
#[cfg(test)]
/// Answers every `mob_despawn` event with the value in its cell and counts them.
    struct Despawns(
        std::sync::Arc<std::sync::Mutex<mod_api::Despawn>>,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    );
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for Despawns {
        fn mob_despawn(&self, _: &dyn mod_api::Host, event: &mut mod_api::MobDespawnEvent<'_>) {
            self.1.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            *event.result = *self.0.lock().unwrap();
        }
    }
#[cfg(test)]
/// Answers the group size limit of a type, reading the mob it is lent.
    struct ZombieClusters;
#[cfg(test)]
#[mod_api::sunrise_part]
    impl mod_api::EntityPart for ZombieClusters {
        fn spawn_cluster_size(
            &self,
            host: &dyn mod_api::Host,
            mob: mod_api::UnspawnedMobRef,
            reason: mod_api::SpawnReason,
            _: Option<mod_api::SpawnSource>,
            vanilla: u32,
        ) -> Option<u32> {
            assert_eq!(mob.kind(host).unwrap().to_string(), "minecraft:zombie");
            Some(if reason == mod_api::SpawnReason::Natural {
                vanilla * 2
            } else {
                1
            })
        }
    }
#[cfg(test)]
/// A second part on the same type: asked only when the first does not answer.
    struct LaterClusters;
#[cfg(test)]
#[mod_api::sunrise_part]
    impl mod_api::EntityPart for LaterClusters {
        fn spawn_cluster_size(
            &self,
            _: &dyn mod_api::Host,
            _: mod_api::UnspawnedMobRef,
            _: mod_api::SpawnReason,
            _: Option<mod_api::SpawnSource>,
            _: u32,
        ) -> Option<u32> {
            Some(9)
        }
    }
#[cfg(test)]
/// A part that leaves the limit to vanilla.
    struct SilentClusters;
#[cfg(test)]
#[mod_api::sunrise_part]
    impl mod_api::EntityPart for SilentClusters {}
#[cfg(test)]
struct ClusterMod;
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for ClusterMod {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            r.entity_part("minecraft:zombie", ZombieClusters);
            r.entity_part("minecraft:zombie", LaterClusters);
            r.entity_part("minecraft:skeleton", SilentClusters);
        }
    }
}
