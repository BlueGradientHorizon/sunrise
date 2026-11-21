//! The typed inside-block effects and callbacks an `entity_inside` part queues.
//!
//! Vanilla's `Block.entityInside` runs mid-walk and queues its five
//! `InsideBlockEffectType`s and the `runBefore`/`runAfter` callbacks around them
//! (`InsideBlockEffectApplier.StepBasedCollector`). Callbacks run when the collector
//! flushes (`apply_and_clear`), each lent an [`InsideEntityView`] over the swept entity
//! and its live level, stopping as soon as the entity dies.
use crate::{BlockState, EffectKey, EntityRef};

/// Exclusive loan of the swept entity and its live level, lent to queued `run_before`
/// and `run_after` callbacks when `InsideBlockEffectApplier.StepBasedCollector.applyAndClear`
/// flushes them.
// exhaustive: only the engine constructs a view.
// no wire: an exclusive loan of the swept entity and live level during apply_and_clear, not a payload.
pub struct InsideEntityView<'a> {
    access: &'a mut dyn InsideEntityAccess,
}

impl<'a> InsideEntityView<'a> {
    // hidden: the engine constructs the view when flushing queued inside-block callbacks.
    #[doc(hidden)]
    pub fn new(access: &'a mut dyn InsideEntityAccess) -> Self {
        Self { access }
    }

    /// The swept entity's numeric id (`Entity.getId`).
    pub fn id(&self) -> u32 {
        self.access.id()
    }

    /// The swept entity's registry type id (for example `"minecraft:player"`).
    pub fn kind(&self) -> &str {
        self.access.kind()
    }

    /// Whether the swept entity is still alive (`Entity.isAlive`).
    pub fn is_alive(&self) -> bool {
        self.access.is_alive()
    }

    /// The swept entity's current world position (`Entity.position`).
    pub fn position(&self) -> [f64; 3] {
        self.access.position()
    }

    /// The swept entity's current velocity (`Entity.getDeltaMovement`).
    pub fn velocity(&self) -> [f64; 3] {
        self.access.velocity()
    }

    /// Remaining fire ticks (`Entity.getRemainingFireTicks`); positive values mean burning.
    pub fn fire_ticks(&self) -> i32 {
        self.access.fire_ticks()
    }

    /// Whether the swept entity currently has positive remaining fire ticks.
    pub fn is_on_fire(&self) -> bool {
        self.fire_ticks() > 0
    }

    /// The swept entity's accumulated fall distance (`Entity.fallDistance`).
    pub fn fall_distance(&self) -> f64 {
        self.access.fall_distance()
    }

    /// Read the block state at `pos` in the swept entity's live level, or `None` if unloaded
    /// or outside world height.
    pub fn block_state(&self, pos: [i32; 3]) -> Option<BlockState> {
        self.access.block_state(pos)
    }

    /// Add `delta` to the swept entity's velocity (`Entity.push`). Returns `false` if any
    /// component is non-finite.
    pub fn push(&mut self, delta: [f64; 3]) -> bool {
        self.access.push(delta)
    }

    /// Replace the swept entity's velocity (`Entity.setDeltaMovement`). Returns `false` if
    /// any component is non-finite.
    pub fn set_velocity(&mut self, velocity: [f64; 3]) -> bool {
        self.access.set_velocity(velocity)
    }

    /// Apply `Entity.makeStuckInBlock` with `speed_multiplier`, resetting fall distance as
    /// vanilla stuck blocks do. Returns `false` if any component is non-finite.
    pub fn make_stuck_in_block(&mut self, speed_multiplier: [f64; 3]) -> bool {
        self.access.make_stuck_in_block(speed_multiplier)
    }

    /// Reset the swept entity's accumulated fall distance (`Entity.resetFallDistance`).
    pub fn reset_fall_distance(&mut self) {
        self.access.reset_fall_distance();
    }

    /// Set the swept entity's remaining fire ticks (`Entity.setRemainingFireTicks`).
    pub fn set_remaining_fire_ticks(&mut self, ticks: i32) {
        self.access.set_remaining_fire_ticks(ticks);
    }

    /// Apply `LivingEntity.addEffect` to the swept entity. Returns `false` for non-living
    /// entities or if the effect could not be applied.
    pub fn add_effect(&mut self, effect: EffectKey, duration: i32, amplifier: u8) -> bool {
        self.access.add_effect(effect, duration, amplifier)
    }

    /// Hurt the swept entity with `damage_type` and `amount` (`Entity.hurt`), with no
    /// direct or causing entity source.
    pub fn hurt(&mut self, damage_type: &str, amount: f32) -> bool {
        self.hurt_with_source(damage_type, amount, None, None)
    }

    /// Hurt the swept entity with `damage_type`, `amount`, and optional `direct` / `causing`
    /// source entity handles (`Entity.hurt`).
    pub fn hurt_with_source(
        &mut self,
        damage_type: &str,
        amount: f32,
        direct: Option<EntityRef>,
        causing: Option<EntityRef>,
    ) -> bool {
        self.access
            .hurt_with_source(damage_type, amount, direct, causing)
    }

    /// Replace the block at `pos` with `state` in the live level (`Level.setBlockAndUpdate`).
    pub fn set_block(&mut self, pos: [i32; 3], state: BlockState) -> bool {
        self.access.set_block(pos, state)
    }

    /// Destroy the block at `pos` in the live level (`Level.destroyBlock`), optionally
    /// dropping its resources.
    pub fn destroy_block(&mut self, pos: [i32; 3], drop: bool) -> bool {
        self.access.destroy_block(pos, drop)
    }
}

/// Engine backing for [`InsideEntityView`] and queued [`InsideEffect`] flushing.
// hidden: engine backend behind InsideEntityView; mods call InsideEntityView methods.
#[doc(hidden)]
pub trait InsideEntityAccess {
    fn id(&self) -> u32;
    fn kind(&self) -> &str;
    fn is_alive(&self) -> bool;
    fn position(&self) -> [f64; 3];
    fn velocity(&self) -> [f64; 3];
    fn fire_ticks(&self) -> i32;
    fn fall_distance(&self) -> f64;
    fn block_state(&self, pos: [i32; 3]) -> Option<BlockState>;
    fn push(&mut self, delta: [f64; 3]) -> bool;
    fn set_velocity(&mut self, velocity: [f64; 3]) -> bool;
    fn make_stuck_in_block(&mut self, speed_multiplier: [f64; 3]) -> bool;
    fn reset_fall_distance(&mut self);
    fn set_remaining_fire_ticks(&mut self, ticks: i32);
    fn add_effect(&mut self, effect: EffectKey, duration: i32, amplifier: u8) -> bool;
    fn hurt_with_source(
        &mut self,
        damage_type: &str,
        amount: f32,
        direct: Option<EntityRef>,
        causing: Option<EntityRef>,
    ) -> bool;
    fn set_block(&mut self, pos: [i32; 3], state: BlockState) -> bool;
    fn destroy_block(&mut self, pos: [i32; 3], drop: bool) -> bool;
    fn apply_effect(&mut self, effect: InsideEffect);
}

/// Vanilla `InsideBlockEffectType`: what a block does to the entity inside it, beyond
/// the writes it makes directly. The collector flushes them per step in this order.
///
/// The effects apply to the swept entity inside its own tick: `Freeze` is
/// `setIsInPowderSnow` plus a frozen tick, `ClearFreeze` thaws, `FireIgnite` is
/// `BaseFireBlock.fireIgnite` (with the level's random draw for a player), `LavaIgnite`
/// ignites for 15 seconds, `Extinguish` is `clearFire`.
// exhaustive: the closed serialized effect vocabulary and case order are a link contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
pub enum InsideEffect {
    /// `InsideBlockEffectType.FREEZE`: powder snow's frost, one frozen tick a step.
    Freeze,
    /// `InsideBlockEffectType.CLEAR_FREEZE`: fire and lava thaw what powder snow froze.
    ClearFreeze,
    /// `InsideBlockEffectType.FIRE_IGNITE`: `BaseFireBlock.fireIgnite`, 8 seconds.
    FireIgnite,
    /// `InsideBlockEffectType.LAVA_IGNITE`: `Entity.lavaIgnite`, 15 seconds.
    LavaIgnite,
    /// `InsideBlockEffectType.EXTINGUISH`: water and powder snow put the fire out.
    Extinguish,
}

/// The order `StepBasedCollector` flushes a step's effects in, `InsideBlockEffectType.values()`.
const EFFECT_ORDER: [InsideEffect; 5] = [
    InsideEffect::Freeze,
    InsideEffect::ClearFreeze,
    InsideEffect::FireIgnite,
    InsideEffect::LavaIgnite,
    InsideEffect::Extinguish,
];

struct QueuedCallback {
    owner: Option<&'static str>,
    callback: Box<dyn FnOnce(&mut InsideEntityView<'_>)>,
}

enum PlannedAction {
    Effect(InsideEffect),
    Callback(QueuedCallback),
}

/// `InsideBlockEffectApplier.StepBasedCollector`: the effects and owned callbacks of one
/// block sweep. The walk advances a step per block it reaches; a step's effects flush
/// once each, in [`InsideEffect`] order, with the callbacks queued via `run_before`
/// before their effect and those queued via `run_after` after it. Steps flush in the
/// order the walk reached them, and `apply_and_clear` runs the ordered plan while the
/// swept entity remains alive, with per-callback panic isolation.
// no wire: holds owned Rust callbacks queued for the current sweep's flush; not a serializable payload.
// exhaustive: the collector's state is private to the crate; a part only calls its methods.
pub struct InsideEffects {
    /// `lastStep`: the step the walk last advanced, `-1` before the first.
    last_step: i32,
    /// `effectsInStep`: the effect types queued for the current step.
    in_step: [bool; 5],
    /// `beforeEffectsInStep`: the callbacks asked before each effect this step.
    before: [Vec<QueuedCallback>; 5],
    /// `afterEffectsInStep`: the callbacks asked after each effect this step.
    after: [Vec<QueuedCallback>; 5],
    /// `finalEffects`: the flushed plan, steps in the order they were reached.
    planned: Vec<PlannedAction>,
}

impl Default for InsideEffects {
    fn default() -> Self {
        Self {
            last_step: -1,
            in_step: [false; 5],
            before: std::array::from_fn(|_| Vec::new()),
            after: std::array::from_fn(|_| Vec::new()),
            planned: Vec::new(),
        }
    }
}

impl std::fmt::Debug for InsideEffects {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let before: usize = self.before.iter().map(Vec::len).sum();
        let after: usize = self.after.iter().map(Vec::len).sum();
        f.debug_struct("InsideEffects")
            .field("last_step", &self.last_step)
            .field("in_step", &self.in_step)
            .field("before_count", &before)
            .field("after_count", &after)
            .field("planned_count", &self.planned.len())
            .finish()
    }
}

impl InsideEffects {
    /// `advanceStep`: flush the current step when the walk reaches a new one. The walk
    /// calls this before a block's effects, as vanilla does.
    pub fn advance_step(&mut self, step: i32) {
        if self.last_step != step {
            self.last_step = step;
            self.flush_step();
        }
    }

    /// `apply`: queue an effect for the current step. A type queued twice in one step
    /// still applies once.
    pub fn apply(&mut self, effect: InsideEffect) {
        self.in_step[effect as usize] = true;
    }

    /// `runBefore`: queue a callback that runs just before `effect` in the flush order.
    /// It runs even when nothing queued `effect` itself that step, as vanilla's do.
    pub fn run_before(
        &mut self,
        effect: InsideEffect,
        callback: impl FnOnce(&mut InsideEntityView<'_>) + 'static,
    ) {
        self.before[effect as usize].push(QueuedCallback {
            owner: crate::content::crash::current_native_mod(),
            callback: Box::new(callback),
        });
    }

    /// `runAfter`: queue a callback that runs just after `effect` in the flush order.
    pub fn run_after(
        &mut self,
        effect: InsideEffect,
        callback: impl FnOnce(&mut InsideEntityView<'_>) + 'static,
    ) {
        self.after[effect as usize].push(QueuedCallback {
            owner: crate::content::crash::current_native_mod(),
            callback: Box::new(callback),
        });
    }

    /// Commit another collector's current step into this one: a staged callback
    /// answered, so what it queued joins this step. The staged collector must not
    /// have flushed (its plan stays empty); a panicking callback is never absorbed.
    // hidden: engine-internal step commit from a completed part dispatch.
    #[doc(hidden)]
    pub fn absorb_step(&mut self, mut staged: Self) {
        for index in 0..5 {
            self.in_step[index] |= staged.in_step[index];
            let before = std::mem::take(&mut staged.before[index]);
            self.before[index].extend(before);
            let after = std::mem::take(&mut staged.after[index]);
            self.after[index].extend(after);
        }
    }

    /// Whether nothing is queued: the walk builds no collector work when no part listens.
    pub fn is_empty(&self) -> bool {
        self.planned.is_empty()
            && self.in_step == [false; 5]
            && self.before.iter().all(Vec::is_empty)
            && self.after.iter().all(Vec::is_empty)
    }

    /// `applyAndClear`: flush the last step and run every queued effect and callback in
    /// order while the swept entity remains alive, isolating panics per callback owner.
    // hidden: engine sweep flush matching vanilla StepBasedCollector.applyAndClear.
    #[doc(hidden)]
    pub fn apply_and_clear(&mut self, access: &mut dyn InsideEntityAccess) {
        self.flush_step();
        self.last_step = -1;
        let planned = std::mem::take(&mut self.planned);
        let mut panicked_owners: Vec<&'static str> = Vec::new();
        for action in planned {
            if !access.is_alive() {
                break;
            }
            match action {
                PlannedAction::Effect(effect) => access.apply_effect(effect),
                PlannedAction::Callback(QueuedCallback { owner, callback }) => {
                    if owner.is_some_and(|id| {
                        crate::runtime::poisoned(crate::HookId::EntityInside, id)
                            || panicked_owners.contains(&id)
                    }) {
                        continue;
                    }
                    let _guard = owner.map(|id| crate::runtime::mark_mod(id, "entity_inside"));
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        crate::world::phase_refusal::contain_refusal(
                            owner.unwrap_or("unknown"),
                            || callback(&mut InsideEntityView::new(access)),
                        )
                    })) {
                        Ok(Ok(())) => {}
                        Ok(Err(refusal)) => refusal.report(),
                        Err(payload) => {
                            crate::runtime::record_callback_panic(
                                crate::HookId::EntityInside,
                                owner.unwrap_or("unknown"),
                                "entity_inside",
                                crate::runtime::panic_summary(payload.as_ref()),
                            );
                            if let Some(id) = owner {
                                panicked_owners.push(id);
                            }
                        }
                    }
                }
            }
        }
    }

    /// `flushStep`: move this step's callbacks and effects into the plan, in `InsideEffect`
    /// order: before the effect, the effect itself, then after it.
    fn flush_step(&mut self) {
        for (index, effect) in EFFECT_ORDER.into_iter().enumerate() {
            for callback in self.before[index].drain(..) {
                self.planned.push(PlannedAction::Callback(callback));
            }
            if std::mem::take(&mut self.in_step[index]) {
                self.planned.push(PlannedAction::Effect(effect));
            }
            for callback in self.after[index].drain(..) {
                self.planned.push(PlannedAction::Callback(callback));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RecordingAccess {
        alive: bool,
        log: Vec<String>,
    }

    impl RecordingAccess {
        fn new() -> Self {
            Self {
                alive: true,
                log: Vec::new(),
            }
        }
    }

    impl InsideEntityAccess for RecordingAccess {
        fn id(&self) -> u32 {
            7
        }
        fn kind(&self) -> &str {
            "minecraft:zombie"
        }
        fn is_alive(&self) -> bool {
            self.alive
        }
        fn position(&self) -> [f64; 3] {
            [0.0, 64.0, 0.0]
        }
        fn velocity(&self) -> [f64; 3] {
            [0.0, 0.0, 0.0]
        }
        fn fire_ticks(&self) -> i32 {
            0
        }
        fn fall_distance(&self) -> f64 {
            0.0
        }
        fn block_state(&self, _: [i32; 3]) -> Option<BlockState> {
            None
        }
        fn push(&mut self, delta: [f64; 3]) -> bool {
            self.log.push(format!("push:{}", delta[0]));
            true
        }
        fn set_velocity(&mut self, _: [f64; 3]) -> bool {
            true
        }
        fn make_stuck_in_block(&mut self, _: [f64; 3]) -> bool {
            self.log.push("stuck".to_owned());
            true
        }
        fn reset_fall_distance(&mut self) {
            self.log.push("reset_fall".to_owned());
        }
        fn set_remaining_fire_ticks(&mut self, ticks: i32) {
            self.log.push(format!("fire:{ticks}"));
        }
        fn add_effect(&mut self, _: EffectKey, duration: i32, amplifier: u8) -> bool {
            self.log.push(format!("effect:{duration}:{amplifier}"));
            true
        }
        fn hurt_with_source(
            &mut self,
            damage_type: &str,
            amount: f32,
            _: Option<EntityRef>,
            _: Option<EntityRef>,
        ) -> bool {
            self.log.push(format!("hurt:{damage_type}:{amount}"));
            if amount >= 100.0 {
                self.alive = false;
            }
            true
        }
        fn set_block(&mut self, _: [i32; 3], _: BlockState) -> bool {
            true
        }
        fn destroy_block(&mut self, _: [i32; 3], _: bool) -> bool {
            true
        }
        fn apply_effect(&mut self, effect: InsideEffect) {
            self.log.push(format!("builtin:{effect:?}"));
        }
    }

    /// `StepBasedCollector` flushes each step once, in `InsideBlockEffectType` order,
    /// with `run_before` callbacks before their effect and `run_after` callbacks after it,
    /// and a type queued twice in one step still applies once.
    #[test]
    fn steps_flush_in_vanilla_order() {
        let mut effects = InsideEffects::default();
        effects.advance_step(0);
        effects.apply(InsideEffect::Freeze);
        effects.run_after(InsideEffect::Freeze, |view| {
            view.make_stuck_in_block([0.25, 0.05, 0.25]);
        });
        effects.apply(InsideEffect::FireIgnite);
        effects.run_after(InsideEffect::FireIgnite, |view| {
            view.hurt("minecraft:in_fire", 1.0);
        });
        effects.advance_step(2);
        effects.apply(InsideEffect::Extinguish);
        effects.apply(InsideEffect::Extinguish);

        let mut access = RecordingAccess::new();
        effects.apply_and_clear(&mut access);
        assert_eq!(
            access.log,
            vec![
                "builtin:Freeze",
                "stuck",
                "builtin:FireIgnite",
                "hurt:minecraft:in_fire:1",
                "builtin:Extinguish",
            ]
        );
        assert!(effects.is_empty(), "applyAndClear empties the collector");
    }

    /// A `run_before` callback runs even when its effect was not queued that step, as
    /// vanilla's before-callbacks do.
    #[test]
    fn a_before_callback_runs_without_its_effect() {
        let mut effects = InsideEffects::default();
        effects.advance_step(3);
        effects.run_before(InsideEffect::LavaIgnite, |view| {
            view.reset_fall_distance();
        });
        let mut access = RecordingAccess::new();
        effects.apply_and_clear(&mut access);
        assert_eq!(access.log, vec!["reset_fall"]);
    }

    /// Two steps queue the same type twice: once each, in step order.
    #[test]
    fn the_same_effect_can_queue_in_two_steps() {
        let mut effects = InsideEffects::default();
        effects.advance_step(0);
        effects.apply(InsideEffect::ClearFreeze);
        effects.advance_step(1);
        effects.apply(InsideEffect::ClearFreeze);
        let mut access = RecordingAccess::new();
        effects.apply_and_clear(&mut access);
        assert_eq!(
            access.log,
            vec!["builtin:ClearFreeze", "builtin:ClearFreeze"]
        );
    }

    /// Once an action kills the swept entity, later queued effects and callbacks stop.
    #[test]
    fn apply_and_clear_stops_when_entity_dies() {
        let mut effects = InsideEffects::default();
        effects.advance_step(0);
        effects.run_before(InsideEffect::Freeze, |view| {
            view.hurt("minecraft:generic", 100.0);
        });
        effects.apply(InsideEffect::Freeze);
        effects.run_after(InsideEffect::Freeze, |view| {
            view.push([1.0, 0.0, 0.0]);
        });
        let mut access = RecordingAccess::new();
        effects.apply_and_clear(&mut access);
        assert_eq!(access.log, vec!["hurt:minecraft:generic:100"]);
    }

    /// A panicking callback poisons only its owner for `entity_inside`, preserves prior
    /// writes, and lets other mods' queued callbacks and built-in effects continue.
    #[test]
    fn a_panicking_callback_poisons_only_its_owner_and_preserves_prior_writes() {
        struct Quiet;
        #[mod_api::__subscriptions]
        impl crate::Hooks for Quiet {}
        crate::testing::with_dispatch(
            vec![
                crate::ModEntry::new("bad-cb", Box::new(Quiet)),
                crate::ModEntry::new("good-cb", Box::new(Quiet)),
            ]
            .into(),
            || {
                let _ = crate::runtime::take_callback_panics();
                let mut effects = InsideEffects::default();
                effects.advance_step(0);
                {
                    let _g = crate::runtime::mark_mod("bad-cb", "entity_inside");
                    effects.run_before(InsideEffect::Freeze, |view| {
                        view.push([1.0, 0.0, 0.0]);
                        panic!("callback boom");
                    });
                    effects.run_after(InsideEffect::Freeze, |view| {
                        view.push([2.0, 0.0, 0.0]);
                    });
                }
                {
                    let _g = crate::runtime::mark_mod("good-cb", "entity_inside");
                    effects.run_after(InsideEffect::Freeze, |view| {
                        view.push([3.0, 0.0, 0.0]);
                    });
                }
                effects.apply(InsideEffect::Freeze);

                let mut access = RecordingAccess::new();
                effects.apply_and_clear(&mut access);
                assert_eq!(
                    access.log,
                    vec!["push:1", "builtin:Freeze", "push:3"],
                    "prior write stays, second bad-cb callback is skipped, built-in and good-cb continue"
                );
                assert!(crate::runtime::poisoned(
                    crate::HookId::EntityInside,
                    "bad-cb"
                ));
                assert!(!crate::runtime::poisoned(
                    crate::HookId::EntityInside,
                    "good-cb"
                ));
                let notices = crate::runtime::take_callback_panics();
                assert!(notices.iter().any(|n| {
                    n.mod_id == "bad-cb" && n.hook == "entity_inside" && n.detail == "callback boom"
                }));
            },
        );
    }
}
