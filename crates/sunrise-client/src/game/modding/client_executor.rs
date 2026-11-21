//! Mod-owned work submitted to the client application's packet/task drain.
use super::Game;
use mod_api::ClientExecutor;
use sunrise_client::{
    crash::{self, ModCall},
    fast_hash::FxHashSet,
    hud::{self, HudBatch},
    net::ClientTask,
    text::Text,
    toasts::SystemToast,
};

#[derive(Default)]
pub(super) struct State {
    disabled: FxHashSet<&'static str>,
    pub(super) errors: Vec<SystemToast>,
}

impl Game<'_> {
    pub(in crate::game) fn client_executor(&self) -> Option<ClientExecutor> {
        if !self.native_window() {
            return None;
        }
        self.client_inbox.executor(crash::current_native_mod()?)
    }

    pub(super) fn close_client_tasks(&mut self) {
        self.client_inbox.close();
        self.client_backlog.clear();
    }

    pub(super) fn run_client_task(&mut self, task: ClientTask) {
        let owner = task.owner;
        if self.client_executor.disabled.contains(owner) {
            return;
        }
        let guard = crash::enter_native_mod(ModCall {
            id: owner,
            hook: "client_executor",
            part: None,
        });
        let result =
            crash::contain(|| mod_api::__private::contain_refusal(owner, || task.run(self)));
        drop(guard);
        match result {
            Ok(Ok(())) => {}
            Ok(Err(refusal)) => refusal.report(),
            Err(error) => {
                self.client_executor.disabled.insert(owner);
                log::error!(
                    "Mod {owner} panicked in client executor work; its executor callbacks are disabled for this session: {error}"
                );
                self.client_executor.errors.push(
                    SystemToast::new(
                        Text::tr("sunrise.toast.client_task_failed", &[]),
                        Some(Text::literal(owner)),
                    )
                    .with_title_color(hud::rgba(255, 85, 85, 255)),
                );
            }
        }
    }

    pub(super) fn draw_client_task_errors(&mut self, batch: &mut HudBatch) {
        if self.client_executor.errors.is_empty() {
            return;
        }
        let mut used = self.toast_slots_used()
            | u32::from(self.title.as_ref().is_some_and(|title| title.has_toast()));
        let multiplier = self.settings.notification_display_time;
        let mut sound = None;
        self.client_executor.errors.retain_mut(|toast| {
            if toast.slot.is_none() {
                toast.slot = (0..Self::TOAST_SLOTS).find(|slot| used & (1 << slot) == 0);
            }
            let Some(slot) = toast.slot else {
                return true;
            };
            let (change, finished) = toast.update(multiplier);
            sound = sound.or(change);
            if !finished {
                used |= 1 << slot;
                if !self.ui.hide_gui {
                    toast.draw(&self.gui.atlas, batch, 32.0 * slot as f32);
                }
            }
            !finished
        });
        if let Some(show) = sound {
            self.audio.play(
                if show {
                    "minecraft:ui.toast.in"
                } else {
                    "minecraft:ui.toast.out"
                },
                None,
                1.0,
                1.0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::native_entity_refs::tests::with_client_game;
    use mod_api::ClientHost;
    use std::{
        sync::{Arc, Mutex},
        thread,
    };

    fn executor(host: &dyn ClientHost, id: &'static str) -> ClientExecutor {
        let _call = crash::enter_native_mod(ModCall {
            id,
            hook: "client_setup",
            part: None,
        });
        host.client_executor().unwrap()
    }

    #[test]
    #[ignore = "daily"]
    fn worker_tasks_run_in_order_on_the_client_with_its_live_host() {
        with_client_game(|game| {
            let client = thread::current().id();
            let executor = executor(game, "producer");
            let calls = Arc::new(Mutex::new(Vec::new()));
            let worker_calls = calls.clone();
            let worker = executor.clone();
            thread::spawn(move || {
                for ordinal in [2, 0, 1] {
                    let calls = worker_calls.clone();
                    worker.execute(Box::new(move |host| {
                        calls.lock().unwrap().push((
                            ordinal,
                            thread::current().id(),
                            host.window_focused(),
                            crash::current_native_mod(),
                        ));
                    }));
                }
            })
            .join()
            .unwrap();
            assert!(calls.lock().unwrap().is_empty());
            assert!(game.remote.is_none());
            game.focused = true;
            game.poll_remote().unwrap();
            assert_eq!(
                *calls.lock().unwrap(),
                [
                    (2, client, true, Some("producer")),
                    (0, client, true, Some("producer")),
                    (1, client, true, Some("producer")),
                ]
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn shutdown_discards_pending_and_late_client_tasks() {
        with_client_game(|game| {
            let executor = executor(game, "producer");
            let captured = Arc::new(());
            let pending = captured.clone();
            executor.execute(Box::new(move |_| {
                drop(pending);
                panic!("pending client task ran after shutdown");
            }));
            assert_eq!(Arc::strong_count(&captured), 2);

            game.saved_for_exit = true;
            game.begin_closing_save(true).unwrap();
            assert_eq!(Arc::strong_count(&captured), 1);
            let late = captured.clone();
            thread::spawn(move || {
                executor.execute(Box::new(move |_| {
                    drop(late);
                    panic!("late client task ran after shutdown");
                }));
            })
            .join()
            .unwrap();
            assert_eq!(Arc::strong_count(&captured), 1);
            game.poll_remote().unwrap();
            assert!(game.client_executor.errors.is_empty());
        });
    }

    #[test]
    #[ignore = "daily"]
    fn panicking_executor_disables_only_its_mod_and_shows_an_error() {
        with_client_game(|game| {
            let failed = executor(game, "failed");
            let other = executor(game, "other");
            let (record, calls) = std::sync::mpsc::channel();
            failed.execute(Box::new(|_| panic!("mod's task panicked")));
            let skipped = record.clone();
            failed.execute(Box::new(move |_| skipped.send("failed").unwrap()));
            other.execute(Box::new(move |_| record.send("other").unwrap()));

            game.poll_remote().unwrap();
            assert_eq!(calls.try_iter().collect::<Vec<_>>(), ["other"]);
            assert_eq!(game.client_executor.errors.len(), 1);
            let error = &game.client_executor.errors[0];
            assert_eq!(error.message.as_ref().unwrap().plain(), "failed");
        });
    }
}
