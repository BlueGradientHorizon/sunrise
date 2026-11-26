//! World epochs for native handles, and lifecycle observations on authoritative servers.
//! A paused pass
//! observes nothing; a frozen world reports `world_running: false` except during a step.

use crate::game::native_lifecycle_common::OpenState;

use super::{
    Game,
    execution_role::ExecutionRole,
    scheduled_tasks::{ServerTask, dispose_pending_task, report_failure, run_all},
};
use mod_api::{ScheduleError, TaskId};
// Their old home, for mixin code that names them here.
#[allow(unused_imports)]
pub(in crate::game) use mod_api::{
    Authority, LifecycleContext, ServerTickPhase, TickId, WorldEpoch, WorldPhase,
};

impl Game<'_, crate::game::server_side::ServerState> {
    /// The role is fixed at construction.
    pub(in crate::game::server_side) fn native_lifecycle_authority(&self) -> Authority {
        match self.options.execution_role {
            ExecutionRole::Client => Authority::RemoteClient,
            ExecutionRole::IntegratedServer => Authority::IntegratedServer,
            ExecutionRole::DedicatedServer => Authority::DedicatedServer,
        }
    }

    pub(in crate::game) fn server_lifecycle_open_started(&mut self) {
        self.clear_composter_randoms();
        self.reset_entity_residency();
        self.entity_initialization = Default::default();
        self.attachment_indexes.clear();
        self.passages.retire_world();
        self.native_lifecycle
            .begin_open(self.native_lifecycle_authority());
    }

    pub(in crate::game) fn server_lifecycle_world_ready(&mut self) {
        // The saved scoreboard is there for the server-started hooks, before any tick.
        self.load_scoreboard();
        if let Some(context) = self.native_lifecycle.world_open_ready() {
            self.native_server_started(context.authority == Authority::DedicatedServer);
        }
    }

    /// After the final save, before the lock release on the server.
    pub(in crate::game) fn server_lifecycle_world_closed(&mut self) {
        // Each server level closes after the final save (`LevelEvent.Unload`).
        self.native_levels_closing();
        if let Some(context) = self.native_lifecycle.world_close_final() {
            self.native_world_lifecycle(&context, WorldPhase::WorldCloseFinal, "");
        }
        self.clear_composter_randoms();
        self.finish_world_closed_common();
    }

    /// `error_text` is the engine's own error text, unchanged.
    pub(in crate::game) fn native_lifecycle_world_failed(&mut self, error_text: &str) {
        // The server levels that opened before the failure close first.
        if self.is_logical_server() {
            self.native_levels_closing();
        }
        if let Some(context) = self.native_lifecycle.world_open_failed() {
            self.native_world_lifecycle(&context, WorldPhase::WorldOpenFailed, error_text);
        }
        self.attachment_indexes.clear();
        self.passages.retire_world();
        self.resident_entities.table.get_mut().reset();
    }

    /// Report a server-loop or shutdown failure once and retire its epoch.
    pub(in crate::game::server_side) fn native_lifecycle_close_failed(&mut self, error_text: &str) {
        if self.is_logical_server() {
            self.native_levels_closing();
        }
        if let Some(context) = self.native_lifecycle.world_close_failed() {
            self.native_world_lifecycle(&context, WorldPhase::WorldCloseFailed, error_text);
        }
        self.attachment_indexes.clear();
        self.passages.retire_world();
        self.resident_entities.table.get_mut().reset();
    }

    pub(in crate::game::server_side) fn drain_due_tasks(
        &mut self,
    ) -> Option<Box<dyn std::any::Any + Send>> {
        run_all(
            self,
            |game| game.native_lifecycle.tasks.pop_due(),
            |game, (owner, task)| {
                use mod_api::__private::game::CallbackOwner;

                let Some(mod_id) = owner else {
                    // Engine-owned work still follows the server's own failure policy.
                    let _: Result<(), std::convert::Infallible> =
                        game.storage_transaction(|game, _| {
                            task(game);
                            Ok(())
                        });
                    return;
                };
                if let Err(panic) =
                    CallbackOwner::call_scheduled_task(mod_id, game, |game| task(game))
                {
                    report_failure(mod_id, &panic);
                    if let Some(serving) = &game.side.serving {
                        serving.reports().report_scheduled_task_failure(mod_id);
                    }
                }
            },
        )
    }

    pub(in crate::game) fn native_lifecycle_server_tick_start(&mut self, world_running: bool) {
        if let Some((context, phase)) = self.native_lifecycle.server_tick_start(world_running) {
            self.native_console_lines();
            self.native_lifecycle.read_scope = true;
            self.native_server_tick(&context, phase);
            // Tasks come due after the start phase, still inside the tick's read scope. Only
            // an unattributed engine panic resumes once the scope is restored.
            let panic = self.drain_due_tasks();
            self.native_lifecycle.read_scope = false;
            if let Some(payload) = panic {
                std::panic::resume_unwind(payload);
            }
        }
    }

    /// `schedule_task`: a delayed callable attributed to the current mod when called inside one.
    pub(in crate::game) fn schedule_task(
        &mut self,
        delay_ticks: u32,
        task: ServerTask,
    ) -> Result<TaskId, ScheduleError> {
        let owner = mod_api::content::crash::current_native_mod();
        if self.native_lifecycle.attempt.is_none_or(|attempt| {
            attempt.authority == Authority::RemoteClient || attempt.session == OpenState::Closing
        }) {
            return Err(ScheduleError::Unavailable);
        }
        self.native_lifecycle
            .tasks
            .schedule(owner, delay_ticks, task)
    }

    /// `cancel_task`: withdraws a waiting task by its opaque `TaskId`, disposing of its captures
    /// under the scheduling owner's panic policy when attributed.
    pub(in crate::game) fn cancel_task(&mut self, id: TaskId) -> bool {
        let Some(task) = self.native_lifecycle.tasks.cancel(id) else {
            return false;
        };
        dispose_pending_task(task);
        true
    }

    /// An errored tick returned at `result?` before this site.
    pub(in crate::game) fn native_lifecycle_server_tick_end(&mut self, paused: bool) {
        if let Some((context, phase)) = self.native_lifecycle.server_tick_end(paused) {
            self.native_lifecycle.read_scope = true;
            self.native_server_tick(&context, phase);
            // Drain tasks due this tick (including delay 0 scheduled during the tick body or end
            // hook), still inside the tick's read scope.
            let panic = self.drain_due_tasks();
            self.native_lifecycle.read_scope = false;
            if let Some(payload) = panic {
                std::panic::resume_unwind(payload);
            }
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;
    use crate::game::native_entity_refs::tests::with_game;
    use crate::game::native_lifecycle_common::NativeLifecycle;
    use mod_api::{Hooks, ModEntry, ServerHost};

    struct TaskOwner;
    #[mod_api::__subscriptions]
    impl Hooks for TaskOwner {}

    #[test]
    fn client_epochs_never_emit_server_lifecycle_events() {
        let mut lifecycle = NativeLifecycle::default();
        lifecycle.begin_open(Authority::RemoteClient);
        let epoch = lifecycle.epoch().unwrap();
        assert!(lifecycle.world_open_ready().is_none());
        assert_eq!(lifecycle.epoch(), Some(epoch));
        assert!(lifecycle.server_tick_start(true).is_none());
        assert!(lifecycle.server_tick_end(false).is_none());
        assert!(!lifecycle.permits_read(&lifecycle.attempt.unwrap().context()));
        lifecycle.begin_close();
        assert!(lifecycle.world_close_final().is_none());
        assert!(lifecycle.epoch().is_none());

        lifecycle.begin_open(Authority::RemoteClient);
        assert_ne!(lifecycle.epoch(), Some(epoch));
        assert!(lifecycle.world_open_failed().is_none());
        assert!(lifecycle.epoch().is_none());

        lifecycle.begin_open(Authority::RemoteClient);
        lifecycle.world_open_ready();
        assert!(lifecycle.world_close_failed().is_none());
        assert!(lifecycle.epoch().is_none());
    }

    #[test]
    fn frozen_steps_are_observed_without_consuming_them() {
        let mut lifecycle = opening();
        lifecycle.world_open_ready();
        let mut clock = sunrise_sim::clock::FixedStep::default();
        clock.set_frozen(true);
        clock.set_frozen_ticks_to_run(4);
        let mut observed = 0;
        for _ in 0..6 {
            let running = clock.world_runs();
            let (_, start) = lifecycle.server_tick_start(running).unwrap();
            let (_, end) = lifecycle.server_tick_end(false).unwrap();
            assert_eq!(
                start,
                ServerTickPhase::ServerTickStart {
                    world_running: running
                }
            );
            assert_eq!(
                end,
                ServerTickPhase::ServerTickEnd {
                    world_running: running
                }
            );
            observed += usize::from(running);
        }
        assert_eq!(observed, 4);
    }

    /// A task a mod schedules from the tick's start hook (after `server_tick_start` moved "now")
    /// fires on the same tick for delay 0, on the next tick for delay 1, and on the Nth tick
    /// after for delay N. A paused pass moves nothing, and closing the world drops what waits.
    #[test]
    fn tasks_scheduled_from_the_start_hook_fire_on_a_later_tick() {
        let owner = Some("mymod");
        let mut lifecycle = opening();
        lifecycle.world_open_ready();
        let mut fired = Vec::new();
        for tick in 1..=6u64 {
            assert!(lifecycle.server_tick_start(true).is_some());
            // The start hook runs here: it schedules on the first tick only.
            if tick == 1 {
                for delay in [0, 1, 3] {
                    lifecycle
                        .tasks
                        .schedule(owner, delay, Box::new(|_| {}))
                        .unwrap();
                }
            }
            while lifecycle.tasks.pop_due().is_some() {
                fired.push(tick);
            }
            lifecycle.server_tick_end(false);
        }
        assert_eq!(fired, [1, 2, 4]);

        lifecycle
            .tasks
            .schedule(owner, 1, Box::new(|_| {}))
            .unwrap();
        assert!(lifecycle.server_tick_end(true).is_none());
        assert!(lifecycle.tasks.pop_due().is_none());
        lifecycle.begin_close();
        lifecycle.world_close_final();
        lifecycle.begin_open(Authority::IntegratedServer);
        lifecycle.world_open_ready();
        lifecycle.server_tick_start(true);
        assert!(lifecycle.tasks.pop_due().is_none());
    }

    #[test]
    #[ignore = "daily"]
    fn due_tasks_run_only_for_enabled_owners_under_their_identity() {
        use mod_api::{__private as api, content::crash::current_native_mod};
        use std::{cell::Cell, rc::Rc};

        let mods = vec![
            ModEntry::new("task_owner", Box::new(TaskOwner)),
            ModEntry::new("task_disabled", Box::new(TaskOwner)),
        ];
        mod_platform::testing::with_mods(mods.into(), || {
            with_game(|game| {
                game.native_lifecycle.world_open_ready().unwrap();
                let capture = Rc::new(());
                let retained = Rc::clone(&capture);
                {
                    let host: &mut dyn ServerHost = game;
                    let _owner = api::enter("task_disabled", "schedule fixture");
                    host.schedule_task(
                        1,
                        Box::new(move |_| {
                            drop(retained);
                            panic!("a disabled owner's callback must not run");
                        }),
                    )
                    .unwrap();
                }
                // TaskId is an opaque capability: another caller can cancel a waiting task, and
                // its captures still drop under the scheduling owner's identity.
                struct DropObserver(Rc<Cell<Option<&'static str>>>);
                impl Drop for DropObserver {
                    fn drop(&mut self) {
                        self.0.set(current_native_mod());
                    }
                }
                let dropped_under = Rc::new(Cell::new(None));
                let cancelled_id = {
                    let host: &mut dyn ServerHost = game;
                    let _owner = api::enter("task_disabled", "schedule fixture");
                    let observer = DropObserver(Rc::clone(&dropped_under));
                    host.schedule_task(
                        1,
                        Box::new(move |_| {
                            let _keep = &observer;
                            panic!("a cancelled task must not run");
                        }),
                    )
                    .unwrap()
                };
                let hit = Rc::new(Cell::new(0));
                {
                    let host: &mut dyn ServerHost = game;
                    let _owner = api::enter("task_owner", "schedule fixture");
                    assert!(host.cancel_task(cancelled_id));
                    assert_eq!(
                        dropped_under.get(),
                        Some("task_disabled"),
                        "cancelled captures drop under the scheduling owner's attribution"
                    );
                    assert_eq!(current_native_mod(), Some("task_owner"));
                    assert!(!host.cancel_task(cancelled_id));
                    let hit = Rc::clone(&hit);
                    host.schedule_task(
                        1,
                        Box::new(move |host| {
                            assert_eq!(current_native_mod(), Some("task_owner"));
                            hit.set(1);
                            let same_tick = Rc::clone(&hit);
                            host.schedule_task(
                                0,
                                Box::new(move |_| {
                                    assert_eq!(current_native_mod(), Some("task_owner"));
                                    same_tick.set(2);
                                }),
                            )
                            .unwrap();
                            let next_tick = Rc::clone(&hit);
                            host.schedule_task(
                                1,
                                Box::new(move |_| {
                                    assert_eq!(current_native_mod(), Some("task_owner"));
                                    next_tick.set(3);
                                }),
                            )
                            .unwrap();
                        }),
                    )
                    .unwrap();
                }

                assert!(api::set_mod_enabled("task_disabled", false));
                game.native_lifecycle_server_tick_start(true);
                assert_eq!(
                    hit.get(),
                    2,
                    "a nested delay-0 task completes in the active start drain"
                );
                assert_eq!(Rc::strong_count(&capture), 1, "disabled captures are gone");
                assert_eq!(current_native_mod(), None);

                // A finite delay-0 chain scheduled during the end drain completes without
                // advancing the tick.
                let chain = Rc::new(Cell::new(0));
                {
                    let host: &mut dyn ServerHost = game;
                    let _owner = api::enter("task_owner", "schedule fixture");
                    let chain_a = Rc::clone(&chain);
                    host.schedule_task(
                        0,
                        Box::new(move |host| {
                            assert_eq!(chain_a.get(), 0);
                            chain_a.set(1);
                            let chain_b = Rc::clone(&chain_a);
                            host.schedule_task(
                                0,
                                Box::new(move |host| {
                                    assert_eq!(chain_b.get(), 1);
                                    chain_b.set(2);
                                    let chain_c = Rc::clone(&chain_b);
                                    host.schedule_task(
                                        0,
                                        Box::new(move |_| {
                                            assert_eq!(chain_c.get(), 2);
                                            chain_c.set(3);
                                        }),
                                    )
                                    .unwrap();
                                }),
                            )
                            .unwrap();
                        }),
                    )
                    .unwrap();
                }
                game.native_lifecycle_server_tick_end(false);
                assert_eq!(
                    chain.get(),
                    3,
                    "a finite delay-0 chain scheduled during the end drain completes in the same tick"
                );
                assert_eq!(current_native_mod(), None);

                assert!(api::set_mod_enabled("task_disabled", true));
                game.native_lifecycle_server_tick_start(true);
                assert_eq!(hit.get(), 3, "a delay-1 task runs on the next tick");
                game.native_lifecycle_server_tick_end(false);
            });
        });
    }

    #[test]
    #[ignore = "daily"]
    fn panicking_scheduled_write_rolls_back_before_the_next_task() {
        use mod_api::{ObjectiveRenderType, Text, TransactionLifecycle};
        use std::{cell::Cell, rc::Rc};

        const FAILING: &str = "task_owner";
        const HEALTHY: &str = "other_task_owner";
        let mods = vec![
            ModEntry::new(FAILING, Box::new(TaskOwner)),
            ModEntry::new(HEALTHY, Box::new(TaskOwner)),
        ];
        mod_platform::testing::with_mods(mods.into(), || {
            with_game(|game| {
                game.native_lifecycle.world_open_ready().unwrap();
                let skipped = Rc::new(Cell::new(false));
                let after_reset = Rc::new(Cell::new(false));
                let unrelated = Rc::new(Cell::new(false));
                {
                    let host: &mut dyn ServerHost = game;
                    let _owner = mod_api::__private::enter(FAILING, "schedule fixture");
                    host.schedule_task(
                        1,
                        Box::new(|host| {
                            host.add_objective(
                                "discarded",
                                "dummy",
                                Text::literal("Discarded"),
                                ObjectiveRenderType::Integer,
                            )
                            .unwrap();
                            panic!("scheduled callback panicked");
                        }),
                    )
                    .unwrap();
                    let skipped = Rc::clone(&skipped);
                    host.schedule_task(1, Box::new(move |_| skipped.set(true)))
                        .unwrap();
                }
                {
                    let host: &mut dyn ServerHost = game;
                    let _owner = mod_api::__private::enter(HEALTHY, "schedule fixture");
                    let unrelated = Rc::clone(&unrelated);
                    host.schedule_task(
                        1,
                        Box::new(move |host| {
                            assert!(host.objective("discarded").is_none());
                            host.add_objective(
                                "kept",
                                "dummy",
                                Text::literal("Kept"),
                                ObjectiveRenderType::Integer,
                            )
                            .unwrap();
                            unrelated.set(true);
                        }),
                    )
                    .unwrap();
                }
                game.native_lifecycle_server_tick_start(true);
                game.native_lifecycle_server_tick_end(false);
                assert!(!skipped.get(), "the failed owner's next task ran");
                assert!(unrelated.get(), "the other owner's task did not finish");
                assert!(game.objective("discarded").is_none());
                assert!(game.objective("kept").is_some());
                assert_eq!(game.transaction_lifecycle(), TransactionLifecycle::None);
                // The poison outlives the world's queue without turning off the mod itself.
                let index = mod_api::__private::mod_index(FAILING).unwrap();
                assert!(mod_api::__private::mod_index_on(index));
                game.native_lifecycle
                    .begin_open(Authority::IntegratedServer);
                game.native_lifecycle.world_open_ready().unwrap();
                {
                    let host: &mut dyn ServerHost = game;
                    let _owner = mod_api::__private::enter(FAILING, "schedule fixture");
                    let after_reset = Rc::clone(&after_reset);
                    host.schedule_task(0, Box::new(move |_| after_reset.set(true)))
                        .unwrap();
                }
                game.native_lifecycle_server_tick_start(true);
                game.native_lifecycle_server_tick_end(false);
                assert!(!after_reset.get(), "reopening cleared the session poison");
            });
        });
    }

    #[test]
    #[ignore = "daily"]
    fn a_scheduled_transaction_refusal_rolls_back_without_disabling_later_tasks() {
        use crate::game::native_entity_refs::tests::run_fresh_cases;
        use mod_api::{
            __private::enter, HostTransaction, ObjectiveError, ObjectiveRenderType, Text,
        };

        if run_fresh_cases(
            "game::server_side::native_lifecycle::tests::a_scheduled_transaction_refusal_rolls_back_without_disabling_later_tasks",
            "SUNRISE_SCHEDULED_TRANSACTION_REFUSAL_CHILD",
            &["refused"],
        ) {
            return;
        }

        const OWNER: &str = "task_owner";
        let mods = vec![ModEntry::new(OWNER, Box::new(TaskOwner))];
        mod_platform::testing::with_mods(mods.into(), || {
            with_game(|game| {
                game.native_lifecycle.world_open_ready().unwrap();
                game.native_lifecycle_server_tick_start(true);
                {
                    let host: &mut dyn ServerHost = game;
                    let _owner = enter(OWNER, "schedule fixture");
                    host.schedule_task(
                        0,
                        Box::new(|host| {
                            let result: Result<(), mod_api::TransactionError<ObjectiveError>> =
                                host.transaction(|host, _| {
                                    // The second write refuses the duplicate, rolling back the first.
                                    for _ in 0..2 {
                                        host.add_objective(
                                            "discarded",
                                            "dummy",
                                            Text::literal("Discarded"),
                                            ObjectiveRenderType::Integer,
                                        )?;
                                    }
                                    Ok(())
                                });
                            assert_eq!(
                                result,
                                Err(mod_api::TransactionError::Aborted(
                                    ObjectiveError::NameTaken
                                ))
                            );
                        }),
                    )
                    .unwrap();
                    host.schedule_task(
                        0,
                        Box::new(|host| {
                            assert!(host.objective("discarded").is_none());
                            host.add_objective(
                                "kept",
                                "dummy",
                                Text::literal("Kept"),
                                ObjectiveRenderType::Integer,
                            )
                            .unwrap();
                        }),
                    )
                    .unwrap();
                }
                game.native_lifecycle_server_tick_end(false);
                assert!(game.objective("discarded").is_none());
                assert!(
                    game.objective("kept").is_some(),
                    "the owner's next task did not commit"
                );
            });
        });
    }

    #[test]
    #[ignore = "daily"]
    fn panicking_scheduled_task_captures_do_not_interrupt_cancellation_or_close() {
        use crate::game::native_entity_refs::tests::run_fresh_cases;
        use mod_api::{
            __private::{HookPanicReport, enter, take_hook_panics},
            content::crash::current_native_mod,
        };
        use std::{cell::RefCell, rc::Rc};
        use sunrise_sim::{join_seq::residency::table::EntityLookup, mobs::Mob};

        if run_fresh_cases(
            "game::server_side::native_lifecycle::tests::panicking_scheduled_task_captures_do_not_interrupt_cancellation_or_close",
            "SUNRISE_SCHEDULED_TASK_DROP_CHILD",
            &["dispose"],
        ) {
            return;
        }
        let home = std::path::PathBuf::from(std::env::var_os("SUNRISE_HOME").unwrap());
        sunrise_level::logging::init(&home, "Server thread");
        sunrise_level::crash::install(home, "server", || None);

        const CANCELLED: &str = "cancelled_task_owner";
        const CLOSED: &str = "closed_task_owner";
        const HEALTHY: &str = "healthy_task_owner";
        struct PanickingCapture(&'static str);
        impl Drop for PanickingCapture {
            fn drop(&mut self) {
                panic!("{}", self.0);
            }
        }

        let mods: Vec<_> = [CANCELLED, CLOSED, HEALTHY]
            .into_iter()
            .map(|id| ModEntry::new(id, Box::new(TaskOwner)))
            .collect();
        mod_platform::testing::with_mods(mods.into(), || {
            with_game(|game| {
                game.native_lifecycle.world_open_ready().unwrap();
                let cancelled = {
                    let _owner = enter(CANCELLED, "schedule fixture");
                    let capture = PanickingCapture("cancelled capture panicked");
                    game.schedule_task(20, Box::new(move |_| drop(capture)))
                        .unwrap()
                };
                {
                    let _caller = enter(HEALTHY, "cancel fixture");
                    assert!(game.cancel_task(cancelled));
                    assert_eq!(current_native_mod(), Some(HEALTHY));
                    assert!(!game.cancel_task(cancelled));
                }
                let closed = {
                    let _owner = enter(CLOSED, "schedule fixture");
                    let capture = PanickingCapture("closing capture panicked");
                    game.schedule_task(20, Box::new(move |_| drop(capture)))
                        .unwrap()
                };
                let healthy_capture = Rc::new(());
                let pending = Rc::clone(&healthy_capture);
                let later = game
                    .schedule_task(20, Box::new(move |_| drop(pending)))
                    .unwrap();

                let pig = game.res.mob_types().get("minecraft:pig").unwrap().clone();
                game.mobs.push(Mob::new(410, pig, Default::default(), 0.0));
                assert!(matches!(
                    game.server_resident_entity(410),
                    EntityLookup::Found { .. }
                ));
                game.native_lifecycle.begin_close();
                game.native_lifecycle_world_closed();
                assert!(game.native_lifecycle.epoch().is_none());
                assert!(matches!(
                    game.server_resident_entity(410),
                    EntityLookup::Missing
                ));
                assert_eq!(Rc::strong_count(&healthy_capture), 1);

                assert_eq!(
                    take_hook_panics(),
                    [CANCELLED, CLOSED].map(|mod_id| HookPanicReport {
                        mod_id,
                        hook: "schedule_task_drop",
                    }),
                );
                let logs = sunrise_level::logging::recent_lines();
                for (owner, message) in [
                    (CANCELLED, "cancelled capture panicked"),
                    (CLOSED, "closing capture panicked"),
                ] {
                    assert!(
                        logs.iter()
                            .any(|line| line.contains(owner) && line.contains(message))
                    );
                }

                game.native_lifecycle
                    .begin_open(Authority::IntegratedServer);
                game.native_lifecycle.world_open_ready().unwrap();
                let ran = Rc::new(RefCell::new(Vec::new()));
                for owner in [CANCELLED, CLOSED, HEALTHY] {
                    let _owner = enter(owner, "schedule fixture");
                    let ran = Rc::clone(&ran);
                    let fresh = game
                        .schedule_task(0, Box::new(move |_| ran.borrow_mut().push(owner)))
                        .unwrap();
                    assert!(![cancelled, closed, later].contains(&fresh));
                }
                for stale in [cancelled, closed, later] {
                    assert!(!game.cancel_task(stale));
                }
                game.native_lifecycle_server_tick_start(true);
                game.native_lifecycle_server_tick_end(false);
                assert_eq!(*ran.borrow(), [HEALTHY]);
            });
        });
    }

    /// A lifecycle mid-open on the server thread.
    fn opening() -> NativeLifecycle {
        let mut lifecycle = NativeLifecycle::default();
        lifecycle.begin_open(Authority::IntegratedServer);
        lifecycle
    }

    #[test]
    fn block_reads_require_the_current_hook_scope() {
        let mut life = NativeLifecycle::default();
        life.begin_open(Authority::IntegratedServer);
        let ready = life.world_open_ready().unwrap();
        assert!(!life.permits_read(&ready));
        let (tick, _) = life.server_tick_start(true).unwrap();
        life.read_scope = true;
        assert!(life.permits_read(&tick));
        assert!(!life.permits_read(&ready));
        life.read_scope = false;
        assert!(!life.permits_read(&tick));
        life.server_tick_end(false).unwrap();
        life.begin_close();
        life.world_close_final().unwrap();
        life.begin_open(Authority::IntegratedServer);
        life.world_open_ready().unwrap();
        life.read_scope = true;
        assert!(!life.permits_read(&tick));
    }

    #[test]
    fn world_ready_fires_once_per_epoch() {
        let mut lifecycle = opening();
        let context = lifecycle.world_open_ready().expect("the first ready fires");
        assert_eq!(context.authority, Authority::IntegratedServer);
        assert!(context.epoch.get() >= 1, "the epoch is globally allocated");
        assert!(lifecycle.world_open_ready().is_none(), "at most once");
    }

    #[test]
    fn a_failed_open_invalidates_and_a_retry_is_fresh() {
        let mut lifecycle = opening();
        let context = lifecycle.world_open_failed().expect("the failure fires");
        let failed_epoch = context.epoch;
        assert!(lifecycle.world_open_ready().is_none(), "the epoch is gone");
        assert!(lifecycle.server_tick_start(true).is_none());
        lifecycle.begin_open(Authority::IntegratedServer);
        let context = lifecycle.world_open_ready().expect("the retry opens fresh");
        assert!(context.epoch.get() > failed_epoch.get(), "a fresh epoch");
    }

    #[test]
    fn close_final_fires_once_and_retires_the_epoch() {
        let mut lifecycle = opening();
        lifecycle.world_open_ready();
        lifecycle.begin_close();
        let context = lifecycle.world_close_final().expect("the close fires");
        assert!(context.epoch.get() >= 1);
        assert!(
            lifecycle.world_close_final().is_none(),
            "a repeated close request is a no-op"
        );
        assert!(lifecycle.server_tick_start(true).is_none());
        assert!(lifecycle.world_open_ready().is_none());
    }

    #[test]
    fn a_close_during_opening_still_closes_once() {
        let mut lifecycle = opening();
        lifecycle.begin_close();
        assert!(lifecycle.world_close_final().is_some());
    }

    #[test]
    fn close_failure_retires_only_ready_epochs() {
        let mut lifecycle = opening();
        assert!(lifecycle.world_close_failed().is_none());
        lifecycle.world_open_ready();
        lifecycle.begin_close();
        assert!(lifecycle.world_close_failed().is_some());
        assert!(lifecycle.world_close_failed().is_none());
        assert!(lifecycle.world_close_final().is_none());
    }
}
