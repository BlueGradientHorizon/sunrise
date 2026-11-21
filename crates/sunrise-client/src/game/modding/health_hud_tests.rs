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
fn f1_hides_only_original_health_and_hud_post_follows_every_slot() {
    use crate::game::native_entity_refs::tests::run_fresh_cases;
    use mod_api::{GuiLayerPlacement, GuiLayerRegistrar, RenderGuiPostEvent, RenderGuiPreEvent};
    use std::sync::atomic::AtomicBool;

    const FLAG: &str = "SUNRISE_HIDDEN_HEALTH_HUD_FIXTURE";
    const BODY: u32 = 0xFF11_2233;
    const LAST: u32 = 0xFF33_4455;
    const POST: u32 = 0xFF55_6677;
    if run_fresh_cases(
        "game::health_hud_tests::f1_hides_only_original_health_and_hud_post_follows_every_slot",
        FLAG,
        &["original", "replacement", "wrapper"],
    ) {
        return;
    }
    let case = std::env::var(FLAG).unwrap();
    let mut layers = crate::gui_layers::GuiLayers::default();
    match case.as_str() {
        "original" => {}
        "replacement" => assert!(layers.replace_layer(
            PLAYER_HEALTH,
            Box::new(|_, draw| {
                draw.fill([3.0, 3.0, 2.0, 2.0], BODY);
            })
        )),
        "wrapper" => assert!(layers.wrap_layer(
            PLAYER_HEALTH,
            Box::new(|ctx| {
                ctx.draw_previous();
                ctx.with_draw(&mut |draw| draw.fill([3.0, 3.0, 2.0, 2.0], BODY));
            })
        )),
        _ => panic!("unknown layer test case: {case}"),
    }
    assert!(layers.register(
        "test:last".into(),
        GuiLayerPlacement::AboveAll,
        Box::new(|_, draw| {
            draw.fill([3.0, 3.0, 2.0, 2.0], LAST);
        })
    ));
    crate::gui_layers::freeze(layers);

    #[derive(Default)]
    struct Trace {
        pre: AtomicUsize,
        post: AtomicUsize,
        selected: AtomicUsize,
        cancel_health: AtomicBool,
        cancel_hud: AtomicBool,
    }
    struct Observe(Arc<Trace>);
    #[mod_api::__subscriptions]
    impl Hooks for Observe {
        fn render_gui_pre(&self, _: &dyn ClientHost, _: &mut RenderGuiPreEvent) -> Flow {
            if self.0.cancel_hud.load(Ordering::Relaxed) {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        }
        fn gui_layer_pre(&self, _: &dyn ClientHost, event: &mut GuiLayerPreEvent<'_>) -> Flow {
            if event.layer == PLAYER_HEALTH {
                self.0.pre.fetch_add(1, Ordering::Relaxed);
                if self.0.cancel_health.load(Ordering::Relaxed) {
                    return Flow::Cancel;
                }
            }
            Flow::Continue
        }
        fn gui_layer_post(&self, _: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
            if event.layer == PLAYER_HEALTH {
                self.0.post.fetch_add(1, Ordering::Relaxed);
            }
        }
        fn health_hud(
            &self,
            _: &dyn ClientHost,
            _: &mut HealthHudEvent<'_>,
        ) -> Option<HealthHudReplacement> {
            self.0.selected.fetch_add(1, Ordering::Relaxed);
            None
        }
        fn render_gui_post(&self, _: &dyn ClientHost, event: &mut RenderGuiPostEvent<'_>) {
            event.draw.fill([3.0, 3.0, 2.0, 2.0], POST);
        }
    }
    let trace = Arc::new(Trace::default());
    with_client_game(|game| {
        prepare_survival_hud(game);
        game.ui.hide_gui = true;
        with_mods(
            vec![ModEntry::new(
                "observer",
                Box::new(Observe(Arc::clone(&trace))),
            )]
            .into(),
            || {
                for (skip_hud, skip_health, pre, post) in [
                    (false, false, 1, 1),
                    (false, true, 2, 1),
                    (true, false, 2, 1),
                ] {
                    trace.cancel_hud.store(skip_hud, Ordering::Relaxed);
                    trace.cancel_health.store(skip_health, Ordering::Relaxed);
                    let batch = game.hud_layers(SIZE);
                    assert_eq!(trace.pre.load(Ordering::Relaxed), pre);
                    assert_eq!(trace.post.load(Ordering::Relaxed), post);
                    assert_eq!(
                        trace.selected.load(Ordering::Relaxed),
                        0,
                        "hidden vanilla health ran"
                    );
                    let colors: Vec<_> = batch
                        .vertices
                        .iter()
                        .filter_map(|vertex| {
                            [BODY, LAST, POST]
                                .into_iter()
                                .find(|&color| abgr(color) == vertex.color)
                        })
                        .collect();
                    let expected = if skip_hud {
                        Vec::new()
                    } else if skip_health || case == "original" {
                        vec![LAST; 6].into_iter().chain([POST; 6]).collect()
                    } else {
                        vec![BODY; 6]
                            .into_iter()
                            .chain([LAST; 6])
                            .chain([POST; 6])
                            .collect()
                    };
                    assert_eq!(colors, expected);
                    if !skip_hud {
                        assert_eq!(batch.vertices[batch.blur_at as usize - 1].color, abgr(POST));
                    }
                }
            },
        );
    });
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
    assert_eq!(
        vanilla_len,
        cancelled_len + 120,
        "cancelling minecraft:player_health must skip only player hearts (20 sprites = 120 verts) while food stays drawn"
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
        let player = game.player.as_mut().unwrap();
        let key = player.attributes.key("minecraft:armor").unwrap();
        let armor = player.attributes.get_instance(Some(key)).unwrap().unwrap();
        player
            .attributes
            .edit(&armor, |a, o| a.set_base_value(20.0, o))
            .unwrap();
        let check_armor = |game: &super::Game<'_>, batch: &crate::hud::HudBatch| {
            let r = game.gui.atlas.gui_draw_rect("hud/armor_full").unwrap();
            let top = batch.height - 39.0;
            assert!(
                batch
                    .vertices
                    .iter()
                    .any(|v| v.uv == [r.u0, r.v0] && v.pos[1] == top)
            );
            assert!(
                !batch
                    .vertices
                    .iter()
                    .any(|v| v.uv == [r.u0, r.v0] && v.pos[1] == top - 10.0)
            );
        };
        with_mods(
            vec![ModEntry::new("cancel", Box::new(CancelPlayerHealth))].into(),
            || {
                let batch = game.hud_layers(SIZE);
                check_armor(game, &batch);
                cancelled_len = batch.vertices.len();
            },
        );
        with_mods(
            vec![ModEntry::new("blank", Box::new(BlankHud))].into(),
            || {
                let batch = game.hud_layers(SIZE);
                check_armor(game, &batch);
                blank_len = batch.vertices.len();
            },
        );
        with_mods(
            vec![ModEntry::new(
                "err_then_ok",
                Box::new(ErrThenOk(Arc::clone(&err_calls))),
            )]
            .into(),
            || {
                let batch = game.hud_layers(SIZE);
                check_armor(game, &batch);
                err_frame = Some(batch);
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

#[test]
#[ignore = "daily"]
fn status_layers_fire_independently_in_neoforge_order_and_stack_heights() {
    const STATUS_LAYERS: [&str; 10] = [
        "minecraft:hotbar",
        "minecraft:player_health",
        "minecraft:armor_level",
        "minecraft:food_level",
        "minecraft:vehicle_health",
        "minecraft:air_level",
        "minecraft:contextual_info_bar",
        "minecraft:selected_item_name",
        "minecraft:effects",
        "minecraft:boss_overlay",
    ];
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    struct StatusObserver(Arc<Mutex<Vec<String>>>, Option<&'static str>);
    #[mod_api::__subscriptions]
    impl Hooks for StatusObserver {
        fn gui_layer_pre(&self, _: &dyn ClientHost, event: &mut GuiLayerPreEvent<'_>) -> Flow {
            if STATUS_LAYERS.contains(&event.layer) {
                self.0.lock().unwrap().push(event.layer.into());
            }
            if self.1 == Some(event.layer) {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        }
    }

    with_client_game(|game| {
        prepare_survival_hud(game);
        let mut full_len = 0;
        let mut no_food_len = 0;

        let observe_all: mod_api::Mods = vec![ModEntry::new(
            "observer",
            Box::new(StatusObserver(Arc::clone(&seen), None)),
        )]
        .into();
        with_mods(observe_all, || {
            full_len = game.hud_layers(SIZE).vertices.len();
        });
        assert_eq!(
            *seen.lock().unwrap(),
            STATUS_LAYERS,
            "the layers around the status HUD must fire in NeoForge order"
        );

        // Cancelling minecraft:food_level skips only the 10 food icons (20 sprites = 120 verts).
        seen.lock().unwrap().clear();
        let cancel_food: mod_api::Mods = vec![ModEntry::new(
            "cancel_food",
            Box::new(StatusObserver(
                Arc::clone(&seen),
                Some("minecraft:food_level"),
            )),
        )]
        .into();
        with_mods(cancel_food, || {
            no_food_len = game.hud_layers(SIZE).vertices.len();
        });
        assert_eq!(full_len, no_food_len + 120);

        // Status layers still fire their pre/post hooks in creative mode (matching NeoForge),
        // while their vanilla bodies skip drawing.
        seen.lock().unwrap().clear();
        game.creative = true;
        let observe_creative: mod_api::Mods = vec![ModEntry::new(
            "observer_creative",
            Box::new(StatusObserver(Arc::clone(&seen), None)),
        )]
        .into();
        with_mods(observe_creative, || {
            let _ = game.hud_layers(SIZE);
        });
        assert_eq!(*seen.lock().unwrap(), STATUS_LAYERS);
    });
}

#[test]
#[ignore = "daily"]
fn panicking_selector_and_item_decorator_contain_only_offending_mod_and_hook() {
    crate::game::in_own_process(
        "game::health_hud_tests::panicking_selector_and_item_decorator_contain_only_offending_mod_and_hook",
        panicking_selector_and_item_decorator_isolated,
    );
}

fn panicking_selector_and_item_decorator_isolated() {
    use mod_api::{
        Identifier, ItemBarEvent, ItemDecorationsEvent, ItemPart, ItemStack, Registrar,
        RegistrationContext,
    };

    static SELECTOR_CALLS: AtomicUsize = AtomicUsize::new(0);
    static HEALTHY_HUD_CALLS: AtomicUsize = AtomicUsize::new(0);
    static BAD_BAR_CALLS: AtomicUsize = AtomicUsize::new(0);
    static BAD_DECORATOR_CALLS: AtomicUsize = AtomicUsize::new(0);

    struct PanickingSelector;
    #[mod_api::__subscriptions]
    impl Hooks for PanickingSelector {
        fn health_hud(
            &self,
            _: &dyn ClientHost,
            _: &mut HealthHudEvent<'_>,
        ) -> Option<HealthHudReplacement> {
            SELECTOR_CALLS.fetch_add(1, Ordering::Relaxed);
            panic!("boom inside health_hud selector");
        }
    }

    struct BadItemPart;
    #[mod_api::__subscriptions]
    impl ItemPart for BadItemPart {
        fn item_bar(&self, _: &dyn ClientHost, _: &mut ItemBarEvent<'_>) {
            BAD_BAR_CALLS.fetch_add(1, Ordering::Relaxed);
            panic!("boom inside item_bar");
        }
        fn item_decorations(&self, _: &dyn ClientHost, event: &mut ItemDecorationsEvent<'_>) {
            BAD_DECORATOR_CALLS.fetch_add(1, Ordering::Relaxed);
            let _ = event.draw.fill([40.0, 10.0, 4.0, 4.0], 0xFFDD_2244);
            panic!("boom inside item_decorations");
        }
    }

    struct BadItemMod;
    #[mod_api::__subscriptions]
    impl Hooks for BadItemMod {
        fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
            let key = r.item_key("minecraft:iron_sword");
            r.item_part(key, BadItemPart);
        }
    }

    struct HealthyHudMod;
    #[mod_api::__subscriptions]
    impl Hooks for HealthyHudMod {
        fn health_hud(
            &self,
            _: &dyn ClientHost,
            _: &mut HealthHudEvent<'_>,
        ) -> Option<HealthHudReplacement> {
            Some(HealthHudReplacement::new(|host, _, canvas| {
                HEALTHY_HUD_CALLS.fetch_add(1, Ordering::Relaxed);
                canvas.fill([12.0, 12.0, 4.0, 4.0], 0xFF22_CC44)?;
                let sword_key = host
                    .item_key(&Identifier::parse("minecraft:iron_sword").unwrap())
                    .unwrap();
                let mut sword = ItemStack::new(sword_key, 1).unwrap();
                sword.set_damage(10);
                canvas.item_decorations(&sword, [16.0, 16.0], None)
            }))
        }
    }

    let mods: mod_api::Mods = vec![
        ModEntry::new("bad_selector", Box::new(PanickingSelector)),
        ModEntry::new("healthy_hud", Box::new(HealthyHudMod)),
        ModEntry::new("bad_item", Box::new(BadItemMod)),
    ]
    .into();

    with_mods(mods.clone(), || {
        crate::game::api_values::install_parts(&mods).unwrap();
        with_client_game(|game| {
            prepare_survival_hud(game);
            let frame1 = game.hud_layers(SIZE);
            assert_eq!(SELECTOR_CALLS.load(Ordering::Relaxed), 1);
            assert_eq!(HEALTHY_HUD_CALLS.load(Ordering::Relaxed), 1);
            assert_eq!(BAD_BAR_CALLS.load(Ordering::Relaxed), 1);
            assert_eq!(BAD_DECORATOR_CALLS.load(Ordering::Relaxed), 1);
            // Both healthy HUD's draw and bad_item's draw before its panic stay in the batch.
            assert!(frame1.vertices.iter().any(|v| v.color == abgr(0xFF22_CC44)));
            assert!(frame1.vertices.iter().any(|v| v.color == abgr(0xFFDD_2244)));
            // Three separate error toasts: bad_selector/health_hud, bad_item/item_bar,
            // bad_item/item_decorations. Healthy HUD is never blamed.
            assert_eq!(game.mod_error_toasts.len(), 3);
            let pairs: Vec<(String, String)> = game
                .mod_error_toasts
                .iter()
                .map(|t| {
                    (
                        t.title.plain(),
                        t.message.as_ref().map(|m| m.plain()).unwrap_or_default(),
                    )
                })
                .collect();
            assert!(
                pairs
                    .iter()
                    .any(|(t, m)| t.contains("bad_selector") && m.contains("health_hud")),
                "missing selector panic toast: {pairs:?}"
            );
            assert!(
                pairs
                    .iter()
                    .any(|(t, m)| t.contains("bad_item") && m.contains("item_bar")),
                "missing item_bar panic toast: {pairs:?}"
            );
            assert!(
                pairs
                    .iter()
                    .any(|(t, m)| t.contains("bad_item") && m.contains("item_decorations")),
                "missing item_decorations panic toast: {pairs:?}"
            );
            assert!(
                pairs.iter().all(|(t, _)| !t.contains("healthy_hud")),
                "healthy_hud must not be blamed for bad_item's panic: {pairs:?}"
            );

            // Next frame: all three panicking hooks stay poisoned, while healthy_hud keeps running.
            let frame2 = game.hud_layers(SIZE);
            assert_eq!(SELECTOR_CALLS.load(Ordering::Relaxed), 1);
            assert_eq!(BAD_BAR_CALLS.load(Ordering::Relaxed), 1);
            assert_eq!(BAD_DECORATOR_CALLS.load(Ordering::Relaxed), 1);
            assert_eq!(HEALTHY_HUD_CALLS.load(Ordering::Relaxed), 2);
            assert_eq!(game.mod_error_toasts.len(), 3);
            assert!(frame2.vertices.iter().any(|v| v.color == abgr(0xFF22_CC44)));
            assert!(frame2.vertices.iter().all(|v| v.color != abgr(0xFFDD_2244)));
        });
    });
}

#[test]
#[ignore = "daily"]
fn only_the_original_health_body_observes_heart_type_and_reseeds_food_jitter() {
    crate::game::in_own_process(
        "game::health_hud_tests::only_the_original_health_body_observes_heart_type_and_reseeds_food_jitter",
        || {
            use mod_api::{GuiLayerRegistrar, PlayerHeartTypeEvent};
            use std::sync::atomic::AtomicBool;
            static CALL_PREVIOUS: AtomicBool = AtomicBool::new(false);
            static CANCEL: AtomicBool = AtomicBool::new(true);
            static HEART_TYPE_CALLS: AtomicUsize = AtomicUsize::new(0);
            struct Observe;
            #[mod_api::__subscriptions]
            impl Hooks for Observe {
                fn gui_layer_pre(&self, _: &dyn ClientHost, e: &mut GuiLayerPreEvent<'_>) -> Flow {
                    if e.layer == PLAYER_HEALTH && CANCEL.load(Ordering::Relaxed) {
                        Flow::Cancel
                    } else {
                        Flow::Continue
                    }
                }
                fn player_heart_type(&self, _: &mut dyn ClientHost, _: &mut PlayerHeartTypeEvent) {
                    HEART_TYPE_CALLS.fetch_add(1, Ordering::Relaxed);
                }
            }
            let mut layers = crate::gui_layers::GuiLayers::default();
            assert!(layers.wrap_layer(
                PLAYER_HEALTH,
                Box::new(|ctx| {
                    if CALL_PREVIOUS.load(Ordering::Relaxed) {
                        ctx.draw_previous();
                    }
                })
            ));
            crate::gui_layers::freeze(layers);
            with_client_game(|game| {
                prepare_survival_hud(game);
                game.ticks = 0;
                game.world.game_time = 0;
                let food = &mut game.player.as_mut().unwrap().health.food;
                food.food = 0;
                food.saturation = 0.0;
                with_mods(
                    vec![ModEntry::new("observer", Box::new(Observe))].into(),
                    || {
                        // java.util.Random(0).nextInt(3) - 1 for ten icons, then ten more.
                        let first = [-1, 0, 0, 1, 1, 1, 1, -1, -1, 1];
                        let second = [1, 1, 1, 0, 1, 1, 0, -1, 1, 1];
                        for (cancel, previous, calls, jitter) in [
                            (true, true, 0, first),
                            (false, false, 0, second),
                            (false, true, 1, first),
                        ] {
                            CANCEL.store(cancel, Ordering::Relaxed);
                            CALL_PREVIOUS.store(previous, Ordering::Relaxed);
                            let frame = game.hud_layers(SIZE);
                            assert_eq!(HEART_TYPE_CALLS.load(Ordering::Relaxed), calls);
                            let r = game.gui.atlas.gui_draw_rect("hud/food_empty").unwrap();
                            let base = frame.height as i32 - 39;
                            let right = frame.width as i32 / 2 + 91 - 9;
                            for (slot, dy) in jitter.into_iter().enumerate() {
                                assert!(frame.vertices.iter().any(|v| v.uv == [r.u0, r.v0]
                                    && v.pos
                                        == [(right - slot as i32 * 8) as f32, (base + dy) as f32]));
                            }
                        }
                    },
                );
            });
        },
    );
}

#[test]
#[ignore = "daily"]
fn a_panicking_item_model_hook_does_not_disable_the_health_renderer() {
    use mod_api::{Identifier, ItemModelUpdateEvent, ItemStack};
    struct DrawItem;
    #[mod_api::__subscriptions]
    impl Hooks for DrawItem {
        fn health_hud(
            &self,
            _: &dyn ClientHost,
            _: &mut HealthHudEvent<'_>,
        ) -> Option<HealthHudReplacement> {
            Some(HealthHudReplacement::new(|host, _, canvas| {
                let key = host
                    .item_key(&Identifier::parse("minecraft:stick").unwrap())
                    .unwrap();
                canvas.fill([8.0, 8.0, 4.0, 4.0], 0xFF44_8800)?;
                canvas.item(&ItemStack::of(key), [20.0, 8.0], 0)?;
                canvas.fill([32.0, 8.0, 4.0, 4.0], 0xFF88_0044)
            }))
        }
    }
    struct BadModel(Arc<AtomicUsize>);
    #[mod_api::__subscriptions]
    impl Hooks for BadModel {
        fn item_model_update(&self, _: &dyn ClientHost, event: &mut ItemModelUpdateEvent<'_>) {
            assert_eq!(event.kind, "bad_model:stick");
            self.0.fetch_add(1, Ordering::Relaxed);
            panic!("item model failed");
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    with_mods(
        vec![
            ModEntry::new("hud", Box::new(DrawItem)),
            ModEntry::new("bad_model", Box::new(BadModel(Arc::clone(&calls)))),
        ]
        .into(),
        || {
            with_client_game(|game| {
                prepare_survival_hud(game);
                use sunrise_assets::{item_model::ItemModelNode, resource_source::ResourceRoot};
                let root = ResourceRoot::directory(game.options.resources.clone());
                let mut renderer =
                    sunrise_client::item_render::ItemRenderer::load(&root, &game.res.item_data)
                        .unwrap();
                renderer
                    .models
                    .definitions
                    .get_mut("minecraft:stick")
                    .unwrap()
                    .model = ItemModelNode::Registered {
                    kind: "bad_model:stick".into(),
                    transformation: None,
                    config: Arc::new(sunrise_core::nbt::Tag::Compound(Default::default())),
                    models: Vec::new(),
                };
                game.pack.item_renderer = Arc::new(renderer);
                for _ in 0..2 {
                    let frame = game.hud_layers(SIZE);
                    for color in [0xFF44_8800, 0xFF88_0044] {
                        assert!(frame.vertices.iter().any(|v| v.color == abgr(color)));
                    }
                    assert_eq!(calls.load(Ordering::Relaxed), 1);
                    assert_eq!(game.mod_error_toasts.len(), 1);
                    let toast = &game.mod_error_toasts[0];
                    assert!(toast.title.plain().contains("bad_model"));
                    assert!(
                        toast
                            .message
                            .as_ref()
                            .unwrap()
                            .plain()
                            .contains("item_model_update")
                    );
                }
            });
        },
    );
}
