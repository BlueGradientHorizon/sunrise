//! Mod-owned work submitted to the client application's packet/task drain.
use super::Game;
use mod_api::ClientExecutor;
use sunrise_client::{
    crash::{self, ModCall},
    hud::{self, HudBatch},
    net::ClientTask,
    text::Text,
    toasts::SystemToast,
};

#[derive(Default)]
pub(super) struct State {
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
                log::error!("Mod {owner} panicked in client executor work: {error}");
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
    use crate::game::{
        EventLoopCtl, native_entity_refs::tests::with_client_game, raw_backlog::Received,
    };
    use mod_api::ClientHost;
    use std::{
        sync::{Arc, Mutex},
        thread,
    };
    use sunrise_client::net::Event;

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
    fn closing_window_during_pending_save_discards_pending_and_late_client_tasks() {
        use crate::game::input_trace::InputOrigin;
        use winit::{event::WindowEvent, event_loop::ControlFlow, window::WindowId};
        struct TestLoop;
        impl EventLoopCtl for TestLoop {
            fn exit(&self) {
                panic!("the quit must wait for the pending save");
            }
            fn set_control_flow(&self, _: ControlFlow) {}
        }

        with_client_game(|game| {
            let executor = executor(game, "producer");
            let captured = Arc::new(());
            let pending = captured.clone();
            executor.execute(Box::new(move |_| {
                drop(pending);
                panic!("pending client task ran after shutdown");
            }));
            assert_eq!(Arc::strong_count(&captured), 2);

            let (reply, completion) = std::sync::mpsc::channel();
            game.begin_control_save(false, reply).unwrap();
            assert!(!game.pending_save.as_ref().unwrap().quit);
            game.options.headless = true;
            game.handle_window_event(
                &TestLoop,
                WindowId::dummy(),
                WindowEvent::CloseRequested,
                InputOrigin::User,
            );
            assert!(game.pending_save.as_ref().unwrap().quit);
            assert!(matches!(
                completion.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ));
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
    fn executor_reports_a_panic_and_runs_later_tasks_from_any_mod() {
        with_client_game(|game| {
            let failed = executor(game, "failed");
            let other = executor(game, "other");
            let (record, calls) = std::sync::mpsc::channel();
            failed.execute(Box::new(|_| panic!("mod's task panicked")));
            let same_mod = record.clone();
            failed.execute(Box::new(move |_| same_mod.send("failed").unwrap()));
            other.execute(Box::new(move |_| record.send("other").unwrap()));

            game.poll_remote().unwrap();
            assert_eq!(calls.try_iter().collect::<Vec<_>>(), ["failed", "other"]);
            assert_eq!(game.client_executor.errors.len(), 1);
            let error = &game.client_executor.errors[0];
            assert_eq!(error.message.as_ref().unwrap().plain(), "failed");
        });
    }

    #[test]
    #[ignore = "daily"]
    fn executor_drains_the_current_turn_even_when_a_packet_disconnects() {
        with_client_game(|game| {
            game.init_remote_for_replay().unwrap();
            game.client_backlog_serial = Some(game.remote.as_ref().unwrap().session.serial());
            game.client_backlog.push_back(Received::Event(
                Event::Disconnected {
                    reason: "fixture disconnected".into(),
                    key: None,
                    from_server: true,
                },
                None,
            ));
            let executor = executor(game, "producer");
            let (record, calls) = std::sync::mpsc::channel();
            let first = record.clone();
            executor.execute(Box::new(move |_| first.send(2).unwrap()));
            executor.execute(Box::new(move |_| record.send(1).unwrap()));

            game.poll_remote().unwrap();
            assert!(game.remote.is_none());
            assert_eq!(calls.try_iter().collect::<Vec<_>>(), [2, 1]);
        });
    }
}
