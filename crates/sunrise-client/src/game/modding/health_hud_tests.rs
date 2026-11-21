#![cfg(test)]
//! Integration tests for `health_hud` replacement selection, `minecraft:player_health` layer
//! cancellation, empty `Ok(())`, non-rollback/non-latching `Err`, and panic isolation + toast.
use super::native_entity_refs::tests::with_client_game;
use mod_api::content::crash::{ModCall, current_native_call};
use mod_api::{
    ClientHost, Flow, GuiDrawError, GuiLayerPostEvent, GuiLayerPreEvent, HealthHudEvent,
    HealthHudReplacement, Hooks, ModEntry,
};
use mod_platform::testing::with_mods;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use sunrise_client::hud_layout::abgr;
use sunrise_client::motion::PlayerMotion;
use sunrise_core::geometry::Vec3;

const SIZE: [u32; 2] = [640, 480];
const PLAYER_HEALTH: &str = "minecraft:player_health";

fn prepare_survival_hud(game: &mut super::Game<'_>) {
    game.title = None;
    game.screen = super::Screen::Playing;
    game.creative = false;
    game.ui.hide_gui = false;
    game.player = Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 0.5)).unwrap());
    game.mod_error_toasts.clear();
}

struct CancelPlayerHealth;

#[mod_api::__subscriptions]
impl Hooks for CancelPlayerHealth {
    fn gui_layer_pre(&self, _: &dyn ClientHost, event: &mut GuiLayerPreEvent<'_>) -> Flow {
        if event.layer == PLAYER_HEALTH {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }
}

#[test]
#[ignore = "daily"]
fn first_answering_health_hud_wins_between_layer_hooks_in_its_mod_scope() {
    let trace: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen_call: Arc<Mutex<Option<ModCall>>> = Arc::new(Mutex::new(None));

    struct Observer(Arc<Mutex<Vec<String>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Observer {
        fn gui_layer_pre(&self, _: &dyn ClientHost, event: &mut GuiLayerPreEvent<'_>) -> Flow {
            if event.layer == PLAYER_HEALTH {
                self.0.lock().unwrap().push("pre".into());
            }
            Flow::Continue
        }
        fn gui_layer_post(&self, _: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
            if event.layer == PLAYER_HEALTH {
                self.0.lock().unwrap().push("post".into());
            }
        }
    }

    struct Defer(Arc<Mutex<Vec<String>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Defer {
        fn health_hud(
            &self,
            _: &dyn ClientHost,
            _: &mut HealthHudEvent<'_>,
        ) -> Option<HealthHudReplacement> {
            self.0.lock().unwrap().push("select:defer".into());
            None
        }
    }

    struct First(Arc<Mutex<Vec<String>>>, Arc<Mutex<Option<ModCall>>>);
    #[mod_api::__subscriptions]
    impl Hooks for First {
        fn health_hud(
            &self,
            _: &dyn ClientHost,
            _: &mut HealthHudEvent<'_>,
        ) -> Option<HealthHudReplacement> {
            self.0.lock().unwrap().push("select:first".into());
            let trace = Arc::clone(&self.0);
            let seen_call = Arc::clone(&self.1);
            Some(HealthHudReplacement::new(move |_, _, canvas| {
                *seen_call.lock().unwrap() = current_native_call();
                trace.lock().unwrap().push("draw:first".into());
                canvas.fill([12.0, 14.0, 6.0, 6.0], 0xFF11_CC22)
            }))
        }
    }

    struct Second(Arc<Mutex<Vec<String>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Second {
        fn health_hud(
            &self,
            _: &dyn ClientHost,
            _: &mut HealthHudEvent<'_>,
        ) -> Option<HealthHudReplacement> {
            self.0.lock().unwrap().push("select:second".into());
            Some(HealthHudReplacement::new(|_, _, canvas| {
                canvas.fill([30.0, 14.0, 6.0, 6.0], 0xFFCC_1122)
            }))
        }
    }

    let mods: mod_api::Mods = vec![
        ModEntry::new("observer", Box::new(Observer(Arc::clone(&trace)))),
        ModEntry::new("defer", Box::new(Defer(Arc::clone(&trace)))),
        ModEntry::new(
            "first",
            Box::new(First(Arc::clone(&trace), Arc::clone(&seen_call))),
        ),
        ModEntry::new("second", Box::new(Second(Arc::clone(&trace)))),
    ]
    .into();

    let (batch, baseline_len) = {
        let mut batch = None;
        let mut baseline_len = 0;
        with_client_game(|game| {
            prepare_survival_hud(game);
            with_mods(
                vec![ModEntry::new("cancel", Box::new(CancelPlayerHealth))].into(),
                || {
                    baseline_len = game.hud_layers(SIZE).vertices.len();
                },
            );
            with_mods(mods, || {
                batch = Some(game.hud_layers(SIZE));
            });
        });
        (batch.unwrap(), baseline_len)
    };

    assert_eq!(
        *trace.lock().unwrap(),
        vec!["pre", "select:defer", "select:first", "draw:first", "post"]
    );
    assert_eq!(
        *seen_call.lock().unwrap(),
        Some(ModCall {
            id: "first",
            hook: "health_hud",
            part: None,
        })
    );
    assert_eq!(batch.vertices.len(), baseline_len + 6);
    assert!(batch.vertices.iter().any(|v| v.color == abgr(0xFF11_CC22)));
    assert!(batch.vertices.iter().all(|v| v.color != abgr(0xFFCC_1122)));
}

#[test]
#[ignore = "daily"]
fn cancelling_player_health_layer_skips_health_hud_and_vanilla_rows() {
    let selected = Arc::new(AtomicUsize::new(0));
    let post_seen = Arc::new(AtomicUsize::new(0));

    struct Canceller(Arc<AtomicUsize>, Arc<AtomicUsize>);
    #[mod_api::__subscriptions]
    impl Hooks for Canceller {
        fn gui_layer_pre(&self, _: &dyn ClientHost, event: &mut GuiLayerPreEvent<'_>) -> Flow {
            if event.layer == PLAYER_HEALTH {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        }
        fn health_hud(
            &self,
            _: &dyn ClientHost,
            _: &mut HealthHudEvent<'_>,
        ) -> Option<HealthHudReplacement> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Some(HealthHudReplacement::new(|_, _, canvas| {
                canvas.fill([12.0, 14.0, 6.0, 6.0], 0xFF11_2233)
            }))
        }
        fn gui_layer_post(&self, _: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
            if event.layer == PLAYER_HEALTH {
                self.1.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    let mods: mod_api::Mods = vec![ModEntry::new(
        "canceller",
        Box::new(Canceller(Arc::clone(&selected), Arc::clone(&post_seen))),
    )]
    .into();

    let (vanilla_len, cancelled_len) = {
        let mut vanilla_len = 0;
        let mut cancelled_len = 0;
        with_client_game(|game| {
            prepare_survival_hud(game);
            with_mods(Vec::<ModEntry>::new().into(), || {
                vanilla_len = game.hud_layers(SIZE).vertices.len();
            });
            with_mods(mods, || {
                cancelled_len = game.hud_layers(SIZE).vertices.len();
            });
        });
        (vanilla_len, cancelled_len)
    };

    assert_eq!(selected.load(Ordering::Relaxed), 0);
    assert_eq!(post_seen.load(Ordering::Relaxed), 0);
    assert!(
        vanilla_len >= cancelled_len + 240,
        "vanilla health + food rows ({vanilla_len}) must be skipped when cancelled ({cancelled_len})"
    );
}

#[test]
#[ignore = "daily"]
fn empty_ok_is_blank_and_err_keeps_prior_draws_without_vanilla_fallback_or_latching() {
    struct BlankHud;
    #[mod_api::__subscriptions]
    impl Hooks for BlankHud {
        fn health_hud(
            &self,
            _: &dyn ClientHost,
            _: &mut HealthHudEvent<'_>,
        ) -> Option<HealthHudReplacement> {
            Some(HealthHudReplacement::new(|_, _, _| Ok(())))
        }
    }

    let err_calls = Arc::new(AtomicUsize::new(0));
    struct ErrThenOk(Arc<AtomicUsize>);
    #[mod_api::__subscriptions]
    impl Hooks for ErrThenOk {
        fn health_hud(
            &self,
            _: &dyn ClientHost,
            _: &mut HealthHudEvent<'_>,
        ) -> Option<HealthHudReplacement> {
            let calls = Arc::clone(&self.0);
            Some(HealthHudReplacement::new(move |_, _, canvas| {
                let n = calls.fetch_add(1, Ordering::Relaxed);
                if n == 0 {
                    canvas.fill([10.0, 10.0, 4.0, 4.0], 0xFF12_3456)?;
                    Err(GuiDrawError::UnknownTexture)
                } else {
                    canvas.fill([20.0, 10.0, 4.0, 4.0], 0xFF65_4321)
                }
            }))
        }
    }

    let mut cancelled_len = 0;
    let mut blank_len = 0;
    let mut err_frame = None;
    let mut ok_frame = None;

    with_client_game(|game| {
        prepare_survival_hud(game);
        with_mods(
            vec![ModEntry::new("cancel", Box::new(CancelPlayerHealth))].into(),
            || {
                cancelled_len = game.hud_layers(SIZE).vertices.len();
            },
        );
        with_mods(
            vec![ModEntry::new("blank", Box::new(BlankHud))].into(),
            || {
                blank_len = game.hud_layers(SIZE).vertices.len();
            },
        );
        with_mods(
            vec![ModEntry::new(
                "err_then_ok",
                Box::new(ErrThenOk(Arc::clone(&err_calls))),
            )]
            .into(),
            || {
                err_frame = Some(game.hud_layers(SIZE));
                ok_frame = Some(game.hud_layers(SIZE));
            },
        );
    });

    assert_eq!(
        blank_len, cancelled_len,
        "Ok(()) without drawing must leave minecraft:player_health blank"
    );

    let err_frame = err_frame.unwrap();
    assert_eq!(
        err_frame.vertices.len(),
        cancelled_len + 6,
        "Err must keep prior draws without rolling back or falling back to vanilla"
    );
    assert!(
        err_frame
            .vertices
            .iter()
            .any(|v| v.color == abgr(0xFF12_3456))
    );

    let ok_frame = ok_frame.unwrap();
    assert_eq!(
        err_calls.load(Ordering::Relaxed),
        2,
        "non-panic Err must not be latched on the next frame"
    );
    assert_eq!(ok_frame.vertices.len(), cancelled_len + 6);
    assert!(
        ok_frame
            .vertices
            .iter()
            .any(|v| v.color == abgr(0xFF65_4321))
    );
}

#[test]
#[ignore = "daily"]
fn panicking_health_hud_disables_only_that_hook_and_queues_error_toast() {
    let panicker_selected = Arc::new(AtomicUsize::new(0));
    let panicker_post = Arc::new(AtomicUsize::new(0));
    let backup_ran = Arc::new(AtomicUsize::new(0));

    struct Panicker(Arc<AtomicUsize>, Arc<AtomicUsize>);
    #[mod_api::__subscriptions]
    impl Hooks for Panicker {
        fn health_hud(
            &self,
            _: &dyn ClientHost,
            _: &mut HealthHudEvent<'_>,
        ) -> Option<HealthHudReplacement> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Some(HealthHudReplacement::new(|_, _, canvas| {
                canvas.fill([10.0, 10.0, 4.0, 4.0], 0xFFAA_0000)?;
                panic!("boom inside health_hud replacement");
            }))
        }
        fn gui_layer_post(&self, _: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
            if event.layer == PLAYER_HEALTH {
                self.1.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    struct Backup(Arc<AtomicUsize>);
    #[mod_api::__subscriptions]
    impl Hooks for Backup {
        fn health_hud(
            &self,
            _: &dyn ClientHost,
            _: &mut HealthHudEvent<'_>,
        ) -> Option<HealthHudReplacement> {
            let ran = Arc::clone(&self.0);
            Some(HealthHudReplacement::new(move |_, _, canvas| {
                ran.fetch_add(1, Ordering::Relaxed);
                canvas.fill([20.0, 10.0, 4.0, 4.0], 0xFF00_AA55)
            }))
        }
    }

    with_client_game(|game| {
        prepare_survival_hud(game);
        let mut vanilla_len = 0;
        with_mods(Vec::<ModEntry>::new().into(), || {
            vanilla_len = game.hud_layers(SIZE).vertices.len();
        });

        // Case 1: second listener takes over on the next frame while panicker's other hooks stay live.
        let mods_with_backup: mod_api::Mods = vec![
            ModEntry::new(
                "panicker",
                Box::new(Panicker(
                    Arc::clone(&panicker_selected),
                    Arc::clone(&panicker_post),
                )),
            ),
            ModEntry::new("backup", Box::new(Backup(Arc::clone(&backup_ran)))),
        ]
        .into();
        with_mods(mods_with_backup, || {
            let frame1 = game.hud_layers(SIZE);
            assert_eq!(panicker_selected.load(Ordering::Relaxed), 1);
            assert_eq!(backup_ran.load(Ordering::Relaxed), 0);
            assert_eq!(panicker_post.load(Ordering::Relaxed), 1);
            assert!(
                frame1.vertices.iter().any(|v| v.color == abgr(0xFFAA_0000)),
                "draws emitted before the panic remain in the batch"
            );
            assert_eq!(game.mod_error_toasts.len(), 1);
            let toast = &game.mod_error_toasts[0];
            assert!(
                toast.title.plain().contains("panicker"),
                "error toast title must name the panicking mod: {:?}",
                toast.title.plain()
            );
            assert!(
                toast
                    .message
                    .as_ref()
                    .is_some_and(|m| m.plain().contains("health_hud")),
                "error toast body must name the hook"
            );

            let frame2 = game.hud_layers(SIZE);
            assert_eq!(
                panicker_selected.load(Ordering::Relaxed),
                1,
                "panicking mod's health_hud must stay disabled for the session"
            );
            assert_eq!(backup_ran.load(Ordering::Relaxed), 1);
            assert_eq!(
                panicker_post.load(Ordering::Relaxed),
                2,
                "panicking mod's other hooks (gui_layer_post) must keep running"
            );
            assert_eq!(game.mod_error_toasts.len(), 1);
            assert!(frame2.vertices.iter().any(|v| v.color == abgr(0xFF00_AA55)));
            assert!(frame2.vertices.iter().all(|v| v.color != abgr(0xFFAA_0000)));
        });

        // Case 2: with no second listener, vanilla health rows run on the next frame.
        let solo_selected = Arc::new(AtomicUsize::new(0));
        let solo_post = Arc::new(AtomicUsize::new(0));
        game.mod_error_toasts.clear();
        let solo_mods: mod_api::Mods = vec![ModEntry::new(
            "solo_panicker",
            Box::new(Panicker(Arc::clone(&solo_selected), Arc::clone(&solo_post))),
        )]
        .into();
        with_mods(solo_mods, || {
            let _frame1 = game.hud_layers(SIZE);
            assert_eq!(solo_selected.load(Ordering::Relaxed), 1);
            assert_eq!(solo_post.load(Ordering::Relaxed), 1);
            assert_eq!(game.mod_error_toasts.len(), 1);
            assert!(
                game.mod_error_toasts[0]
                    .title
                    .plain()
                    .contains("solo_panicker")
            );

            // Clear toast so frame2's vertex count reflects HUD layers alone.
            game.mod_error_toasts.clear();
            let frame2 = game.hud_layers(SIZE);
            assert_eq!(solo_selected.load(Ordering::Relaxed), 1);
            assert_eq!(solo_post.load(Ordering::Relaxed), 2);
            assert_eq!(
                frame2.vertices.len(),
                vanilla_len,
                "vanilla health rows must draw on the next frame once the sole listener is poisoned"
            );
        });
    });
}
