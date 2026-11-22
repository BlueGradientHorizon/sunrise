//! HUD slots in a real client: placement, scoped wrapping and replacement, and layer cancellation.
use super::{
    execution_role::ExecutionRole,
    mods,
    native_entity_refs::tests::{fixture_root, run_fresh_cases, with_client_game, with_game_role},
};
use mod_api::{
    ClientHost, Flow, GuiLayerPlacement, GuiLayerPostEvent, GuiLayerPreEvent, Hooks, ModEntry,
    RegisterGuiLayersEvent, RegistrationContext, content::crash::current_native_mod,
};
use std::sync::Mutex;
use sunrise_client::{gui_layers::VanillaGuiLayer, hud::HudBatch};

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn log(entry: impl Into<String>) {
    LOG.lock().unwrap().push(entry.into());
}

fn assert_owner(id: &'static str) {
    assert_eq!(current_native_mod(), Some(id));
}

struct Compass;
#[mod_api::__subscriptions]
impl Hooks for Compass {
    fn register_gui_layers(&self, _: &RegistrationContext, event: &mut RegisterGuiLayersEvent<'_>) {
        use GuiLayerPlacement::*;
        // The layer namespace is deliberately different from the registering mod.
        for (id, placement) in [
            (
                "hud_test:under_crosshair",
                Below("minecraft:crosshair".into()),
            ),
            (
                "hud_test:over_crosshair",
                Above("minecraft:crosshair".into()),
            ),
            ("hud_test:bar", Above("minecraft:hotbar".into())),
            (
                "hud_test:above_health",
                Above("minecraft:player_health".into()),
            ),
            (
                "hud_test:below_contextual",
                Below("minecraft:contextual_info_bar".into()),
            ),
            ("hud_test:top", AboveAll),
        ] {
            assert!(event.layers.register(
                id.into(),
                placement,
                Box::new(move |_, draw| {
                    assert_owner("compass");
                    log(format!("draw {id}"));
                    if id == "hud_test:under_crosshair" {
                        draw.fill([0.0, 0.0, 4.0, 4.0], 0xFF00_FF00);
                    }
                }),
            ));
        }
    }
    fn gui_layer_pre(&self, _: &dyn ClientHost, event: &mut GuiLayerPreEvent<'_>) -> Flow {
        log(format!("pre {}", event.layer));
        Flow::Continue
    }
    fn gui_layer_post(&self, _: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
        log(format!("post {}", event.layer));
    }
}

struct Inner;
#[mod_api::__subscriptions]
impl Hooks for Inner {
    fn register_gui_layers(&self, _: &RegistrationContext, event: &mut RegisterGuiLayersEvent<'_>) {
        assert!(event.layers.wrap_layer(
            "hud_test:under_crosshair",
            Box::new(|ctx| {
                assert_owner("inner");
                log("inner before");
                ctx.draw_previous();
                assert_owner("inner");
                log("inner after");
            }),
        ));
        assert!(event.layers.wrap_layer(
            "minecraft:hotbar",
            Box::new(|ctx| {
                log("hotbar wrapper before");
                ctx.draw_previous();
                log("hotbar wrapper after");
            }),
        ));
        for (id, label) in [
            ("minecraft:crosshair", "crosshair wrapper"),
            ("minecraft:contextual_info_bar", "contextual wrapper"),
        ] {
            assert!(event.layers.wrap_layer(
                id,
                Box::new(move |ctx| {
                    log(label);
                    ctx.draw_previous();
                })
            ));
        }
    }
}

struct Editor;
#[mod_api::__subscriptions]
impl Hooks for Editor {
    fn register_gui_layers(&self, _: &RegistrationContext, event: &mut RegisterGuiLayersEvent<'_>) {
        assert!(
            !event
                .layers
                .wrap_layer("hud_test:missing", Box::new(|_| {}))
        );
        assert!(
            !event
                .layers
                .replace_layer("hud_test:missing", Box::new(|_, _| {}))
        );
        assert!(event.layers.wrap_layer(
            "hud_test:under_crosshair",
            Box::new(|ctx| {
                assert_owner("editor");
                log("outer before");
                ctx.with_draw(&mut |draw| draw.fill([0.0, 0.0, 4.0, 4.0], 0xFFFF_0000));
                ctx.draw_previous();
                assert_owner("editor");
                ctx.with_draw(&mut |draw| draw.fill([0.0, 0.0, 4.0, 4.0], 0xFF00_00FF));
                log("outer after");
            }),
        ));
        assert!(event.layers.replace_layer(
            "minecraft:contextual_info_bar",
            Box::new(|_, _| {
                assert_owner("editor");
                log("contextual replacement");
            }),
        ));
    }
    fn gui_layer_pre(&self, _: &dyn ClientHost, event: &mut GuiLayerPreEvent<'_>) -> Flow {
        if matches!(event.layer, "hud_test:bar" | "minecraft:effects") {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }
}

fn take() -> Vec<String> {
    std::mem::take(&mut *LOG.lock().unwrap())
}

#[test]
#[ignore = "daily"]
fn mod_layers_draw_in_their_place_and_hide_like_vanilla_layers() {
    // The layer order is kept once per process.
    if run_fresh_cases(
        "game::gui_layer_tests::mod_layers_draw_in_their_place_and_hide_like_vanilla_layers",
        "SUNRISE_GUI_LAYER_FIXTURE",
        &["run"],
    ) {
        return;
    }
    with_client_game(|game| {
        mod_platform::__private::install(
            vec![
                ModEntry::new("compass", Box::new(Compass)),
                ModEntry::new("inner", Box::new(Inner)),
                ModEntry::new("editor", Box::new(Editor)),
            ]
            .into(),
        );
        mods::register_gui_layers(ExecutionRole::Client);
        let mut batch = HudBatch::new([320, 240], game.settings.gui_scale_inputs());

        // Simulate the HUD paint sites with the crosshair skipped (third person).
        // Its wrapper doesn't draw, but both neighboring layers still do.
        game.hud_layer_next = Some(0);
        for (layer, paint) in [
            (VanillaGuiLayer::Hotbar, "hotbar paint"),
            (VanillaGuiLayer::Effects, "effects paint"),
            (VanillaGuiLayer::PlayerHealth, "health paint"),
            (VanillaGuiLayer::ContextualInfoBar, "contextual paint"),
            (VanillaGuiLayer::Chat, "chat paint"),
        ] {
            game.gui_layer(layer, &mut batch, &mut |_, _| log(paint));
        }
        game.draw_mod_gui_layers(None, &mut batch);
        game.hud_layer_next = None;
        assert_eq!(current_native_mod(), None);
        assert_eq!(
            take(),
            [
                "pre hud_test:under_crosshair",
                "outer before",
                "inner before",
                "draw hud_test:under_crosshair",
                "inner after",
                "outer after",
                "post hud_test:under_crosshair",
                "pre hud_test:over_crosshair",
                "draw hud_test:over_crosshair",
                "post hud_test:over_crosshair",
                "pre minecraft:hotbar",
                "hotbar wrapper before",
                "hotbar paint",
                "hotbar wrapper after",
                "post minecraft:hotbar",
                "pre hud_test:bar",
                "pre minecraft:effects",
                "pre minecraft:player_health",
                "health paint",
                "post minecraft:player_health",
                "pre hud_test:above_health",
                "draw hud_test:above_health",
                "post hud_test:above_health",
                "pre hud_test:below_contextual",
                "draw hud_test:below_contextual",
                "post hud_test:below_contextual",
                "pre minecraft:contextual_info_bar",
                "contextual replacement",
                "post minecraft:contextual_info_bar",
                "pre minecraft:chat",
                "chat paint",
                "post minecraft:chat",
                "pre hud_test:top",
                "draw hud_test:top",
                "post hud_test:top",
            ]
        );
        // The outer wrapper and the underlying painter append into the same batch in order.
        let mut colors: Vec<_> = batch
            .vertices
            .iter()
            .map(|v| v.color.to_le_bytes())
            .collect();
        colors.dedup();
        assert_eq!(
            colors,
            [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]]
        );

        // A vanilla layer still paints when the inserted-layer cursor is inactive.
        game.gui_layer(VanillaGuiLayer::SleepOverlay, &mut batch, &mut |_, _| {
            log("sleep paint")
        });
        game.draw_mod_gui_layers(None, &mut batch);
        assert_eq!(
            take(),
            [
                "pre minecraft:sleep_overlay",
                "sleep paint",
                "post minecraft:sleep_overlay"
            ]
        );
    });
}

#[test]
#[ignore = "daily"]
fn a_refused_gui_layer_does_not_leave_partial_paint() {
    crate::game::in_own_process(
        "game::gui_layer_tests::a_refused_gui_layer_does_not_leave_partial_paint",
        || {
            struct RefusedLayer;
            #[mod_api::__subscriptions]
            impl Hooks for RefusedLayer {
                fn register_gui_layers(
                    &self,
                    _: &RegistrationContext,
                    event: &mut RegisterGuiLayersEvent<'_>,
                ) {
                    assert!(event.layers.register(
                        "hud_test:refused".into(),
                        GuiLayerPlacement::AboveAll,
                        Box::new(|_, draw| {
                            draw.fill([0.0, 0.0, 4.0, 4.0], 0xFFFF_0000);
                            log("refused");
                            mod_api::__private::refuse(
                                "test layer refusal",
                                mod_api::__private::ModPhase::Setup,
                            );
                        }),
                    ));
                }
            }
            with_client_game(|game| {
                mod_platform::__private::install(
                    vec![ModEntry::new("refused_layer", Box::new(RefusedLayer))].into(),
                );
                mods::register_gui_layers(ExecutionRole::Client);
                for _ in 0..2 {
                    let mut batch = HudBatch::scratch([320, 240]);
                    game.hud_layer_next = Some(0);
                    game.draw_mod_gui_layers(None, &mut batch);
                    assert!(batch.vertices.is_empty());
                }
                assert_eq!(take(), ["refused", "refused"]);
            });
        },
    );
}

#[test]
#[ignore = "daily"]
fn a_gui_wrapper_panic_is_contained_but_delegated_engine_paint_is_fatal() {
    use sunrise_client::hud::{Blend, rgba};

    let test = "game::gui_layer_tests::a_gui_wrapper_panic_is_contained_but_delegated_engine_paint_is_fatal";
    crate::game::in_own_process(test, || {
        struct Wrapper(&'static str, bool);
        #[mod_api::__subscriptions]
        impl Hooks for Wrapper {
            fn register_gui_layers(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterGuiLayersEvent<'_>,
            ) {
                let Self(id, broken) = *self;
                assert!(event.layers.wrap_layer(
                    "minecraft:hotbar",
                    Box::new(move |ctx| {
                        assert_owner(id);
                        log(id);
                        if broken {
                            ctx.with_draw(&mut |draw| draw.fill([0.0, 0.0, 4.0, 4.0], 0xFFFF_0000));
                            ctx.draw_previous();
                            ctx.with_draw(&mut |draw| draw.fill([0.0, 0.0, 4.0, 4.0], 0xFF00_00FF));
                            ctx.draw_previous();
                            ctx.with_draw(&mut |draw| draw.fill([0.0, 0.0, 4.0, 4.0], 0xFFFF_0000));
                            panic!("broken wrapper");
                        }
                        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            ctx.draw_previous();
                        }))
                        .is_err()
                        {
                            log(format!("caught by {id}"));
                        }
                        assert_owner(id);
                    }),
                ));
            }
        }
        with_client_game(|game| {
            mod_platform::__private::install(
                vec![
                    ModEntry::new("inner", Box::new(Wrapper("inner", false))),
                    ModEntry::new("broken", Box::new(Wrapper("broken", true))),
                    ModEntry::new("outer", Box::new(Wrapper("outer", false))),
                ]
                .into(),
            );
            mods::register_gui_layers(ExecutionRole::Client);
            let mut batch = HudBatch::scratch([320, 240]);
            let white = rgba(255, 255, 255, 255);
            let green = rgba(0, 255, 0, 255);
            let yellow = rgba(255, 255, 0, 255);
            let mut paint_calls = 0;
            batch.fill(&game.gui.atlas, 0.0, 0.0, 4.0, 4.0, white);
            game.gui_layer(VanillaGuiLayer::Hotbar, &mut batch, &mut |g, batch| {
                log("vanilla paint");
                let start = batch.vertices.len();
                let color = [green, yellow][paint_calls];
                paint_calls += 1;
                batch.fill(&g.gui.atlas, 0.0, 0.0, 4.0, 4.0, color);
                batch.invert_since(start);
            });
            assert_eq!(
                take(),
                [
                    "outer",
                    "broken",
                    "inner",
                    "vanilla paint",
                    "inner",
                    "vanilla paint"
                ]
            );
            assert_eq!(
                batch.vertices.iter().map(|v| v.color).collect::<Vec<_>>(),
                [[white; 6], [green; 6], [yellow; 6]].concat()
            );
            assert_eq!(
                batch.blends,
                [(6..12, Blend::Invert), (12..18, Blend::Invert)]
            );

            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                game.gui_layer(VanillaGuiLayer::Hotbar, &mut batch, &mut |_, _| {
                    assert_eq!(current_native_mod(), None);
                    assert!(!mod_api::__private::game::retained_callback_active());
                    panic!("vanilla paint failed");
                });
            }))
            .unwrap_err();
            assert_eq!(panic.downcast_ref::<&str>(), Some(&"vanilla paint failed"));
            assert_eq!(
                take(),
                ["outer", "inner", "caught by inner", "caught by outer"]
            );
            assert_eq!(current_native_mod(), None);

            game.gui_layer(VanillaGuiLayer::Hotbar, &mut batch, &mut |_, _| {
                log("vanilla paint");
            });
            assert_eq!(take(), ["outer", "inner", "vanilla paint"]);
        });
    });
}

#[test]
#[ignore = "daily"]
fn render_gui_post_runs_after_hud_layers_respects_hiding_and_exposes_layer_bounds() {
    use mod_api::{
        LevelReadError, RenderGuiPostEvent, RenderGuiPreEvent, ScreenRenderPostEvent,
        ScreenRenderPreEvent,
    };
    use std::sync::atomic::{AtomicBool, Ordering};

    let test = "game::gui_layer_tests::render_gui_post_runs_after_hud_layers_respects_hiding_and_exposes_layer_bounds";
    crate::game::in_own_process(test, || {
        static CANCEL_PRE: AtomicBool = AtomicBool::new(false);
        type Bounds = Result<Option<[f32; 4]>, LevelReadError>;
        static HUD_BOUNDS: Mutex<Option<[Bounds; 4]>> = Mutex::new(None);
        static SCREEN_BOUNDS: Mutex<Option<Bounds>> = Mutex::new(None);

        struct PostHudObserver;
        #[mod_api::__subscriptions]
        impl Hooks for PostHudObserver {
            fn register_gui_layers(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterGuiLayersEvent<'_>,
            ) {
                assert!(event.layers.register(
                    "post_test:top".into(),
                    GuiLayerPlacement::AboveAll,
                    Box::new(|_, draw| {
                        log("layer post_test:top");
                        draw.fill([12.0, 18.0, 20.0, 10.0], 0xFF00_FF00);
                    }),
                ));
            }
            fn render_gui_pre(&self, _: &dyn ClientHost, _: &mut RenderGuiPreEvent<'_>) -> Flow {
                log("render_gui_pre");
                if CANCEL_PRE.load(Ordering::Relaxed) {
                    Flow::Cancel
                } else {
                    Flow::Continue
                }
            }
            fn gui_layer_post(&self, _: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
                if matches!(
                    event.layer,
                    "minecraft:boss_overlay"
                        | "minecraft:sleep_overlay"
                        | "minecraft:subtitle_overlay"
                        | "minecraft:tab_list"
                        | "post_test:top"
                ) {
                    log(format!("layer_post {}", event.layer));
                }
            }
            fn render_gui_post(&self, host: &dyn ClientHost, event: &mut RenderGuiPostEvent<'_>) {
                log("render_gui_post");
                *HUD_BOUNDS.lock().unwrap() = Some([
                    host.hud_layer_bounds("minecraft:effects"),
                    host.hud_layer_bounds("minecraft:sleep_overlay"),
                    host.hud_layer_bounds("post_test:top"),
                    host.hud_layer_bounds("post_test:missing"),
                ]);
                event.draw.fill([4.0, 4.0, 8.0, 8.0], 0xFFFF_00FF);
            }
            fn screen_render_pre(
                &self,
                _: &dyn ClientHost,
                event: &mut ScreenRenderPreEvent<'_>,
            ) -> Flow {
                log(format!("screen_pre {}", event.screen.id));
                Flow::Continue
            }
            fn screen_render_post(
                &self,
                host: &dyn ClientHost,
                event: &mut ScreenRenderPostEvent<'_>,
            ) {
                *SCREEN_BOUNDS.lock().unwrap() = Some(host.hud_layer_bounds("post_test:top"));
                log(format!("screen_post {}", event.screen.id));
            }
        }

        with_client_game(|game| {
            mod_platform::__private::install(
                vec![ModEntry::new("post_hud", Box::new(PostHudObserver))].into(),
            );
            mods::register_gui_layers(ExecutionRole::Client);
            game.title = None;
            game.screen = super::Screen::Paused;
            let speed = sunrise_client::brewing::effect("speed")
                .expect("vanilla speed effect")
                .id;
            game.potions
                .effects
                .add(sunrise_client::effects::EffectInstance::new(speed, 200, 0));

            // Post comes after every HUD layer, but before even ScreenEvent.Render.Pre.
            game.hud([320, 240]);
            assert_eq!(
                take(),
                [
                    "render_gui_pre",
                    "layer_post minecraft:boss_overlay",
                    "layer_post minecraft:sleep_overlay",
                    "layer_post minecraft:subtitle_overlay",
                    "layer_post minecraft:tab_list",
                    "layer post_test:top",
                    "layer_post post_test:top",
                    "render_gui_post",
                    "screen_pre paused",
                    "screen_post paused",
                ]
            );
            let [effects, sleep, custom, missing] = HUD_BOUNDS.lock().unwrap().take().unwrap();
            assert_eq!(effects, Ok(Some([295.0, 1.0, 24.0, 24.0])));
            assert_eq!(sleep, Ok(None));
            assert_eq!(custom, Ok(Some([12.0, 18.0, 20.0, 10.0])));
            assert_eq!(missing, Ok(None));
            assert_eq!(SCREEN_BOUNDS.lock().unwrap().take(), Some(Ok(None)));

            // F1 stops ordinary HUD paint, but not the manager, the sleep slot, or the
            // subtitle slot on a screen that keeps deferred subtitles.
            game.ui.hide_gui = true;
            game.sleep_timer = 50;
            game.screen = super::Screen::BlockEditor;
            game.hud([320, 240]);
            assert_eq!(
                take(),
                [
                    "render_gui_pre",
                    "layer_post minecraft:sleep_overlay",
                    "layer_post minecraft:subtitle_overlay",
                    "layer post_test:top",
                    "layer_post post_test:top",
                    "render_gui_post",
                    "screen_pre block_editor",
                    "screen_post block_editor",
                ]
            );
            let [effects, sleep, _, _] = HUD_BOUNDS.lock().unwrap().take().unwrap();
            assert_eq!(effects, Ok(None));
            assert_eq!(sleep, Ok(Some([0.0, 0.0, 320.0, 240.0])));

            // A canceled Pre skips even those two slots, but still draws the screen.
            CANCEL_PRE.store(true, Ordering::Relaxed);
            let canceled = game.hud([320, 240]);
            assert_eq!(canceled.blur_at, 0, "HUD paint leaked into a canceled pass");
            assert_eq!(
                take(),
                [
                    "render_gui_pre",
                    "screen_pre block_editor",
                    "screen_post block_editor"
                ]
            );
            assert!(HUD_BOUNDS.lock().unwrap().take().is_none());
            CANCEL_PRE.store(false, Ordering::Relaxed);
            game.ui.hide_gui = false;

            // The same Options screen also exists without a world. Only the in-world route
            // runs the HUD pass, before the screen; a screen hook never sees its bounds.
            game.open_options(false);
            game.hud([320, 240]);
            let events = take();
            assert!(
                events.iter().map(String::as_str).rev().take(3).eq([
                    "screen_post options",
                    "screen_pre options",
                    "render_gui_post",
                ]),
                "unexpected Options order: {events:?}"
            );
            assert_eq!(SCREEN_BOUNDS.lock().unwrap().take(), Some(Ok(None)));

            let saves = tempfile::tempdir().unwrap();
            game.title = Some(game.new_title(saves.path().to_path_buf()));
            game.open_options(true);
            game.hud([320, 240]);
            assert_eq!(take(), ["screen_pre options", "screen_post options"]);
        });
    });
}

#[test]
#[ignore = "daily"]
fn hud_layer_bounds_rejects_dedicated_server_role() {
    crate::game::in_own_process(
        "game::gui_layer_tests::hud_layer_bounds_rejects_dedicated_server_role",
        || {
            with_game_role(fixture_root(), ExecutionRole::DedicatedServer, |game| {
                assert_eq!(
                    game.hud_layer_bounds("minecraft:effects"),
                    Err(mod_api::LevelReadError::Unavailable)
                );
            });
        },
    );
}
