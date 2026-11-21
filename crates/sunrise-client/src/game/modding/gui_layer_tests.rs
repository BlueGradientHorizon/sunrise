//! HUD slots in a real client: placement, scoped wrapping and replacement, and layer cancellation.
use super::{
    execution_role::ExecutionRole,
    mods,
    native_entity_refs::tests::{run_fresh_cases, with_client_game},
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

        // With the HUD hidden (F1), vanilla's sleep overlay still draws, and no mod layer.
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
    use mod_api::{LevelReadError, RenderGuiPostEvent, RenderGuiPreEvent, ScreenRenderPostEvent};
    use std::sync::atomic::{AtomicBool, Ordering};

    let test = "game::gui_layer_tests::render_gui_post_runs_after_hud_layers_respects_hiding_and_exposes_layer_bounds";
    crate::game::in_own_process(test, || {
        static CANCEL_PRE: AtomicBool = AtomicBool::new(false);
        static SEEN_BOUNDS: Mutex<Option<Result<Option<[f32; 4]>, LevelReadError>>> =
            Mutex::new(None);
        static SEEN_CUSTOM_BOUNDS: Mutex<Option<Result<Option<[f32; 4]>, LevelReadError>>> =
            Mutex::new(None);
        static SEEN_UNKNOWN_BOUNDS: Mutex<Option<Result<Option<[f32; 4]>, LevelReadError>>> =
            Mutex::new(None);

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
                    "minecraft:boss_overlay" | "minecraft:tab_list" | "post_test:top"
                ) {
                    log(format!("layer_post {}", event.layer));
                }
            }
            fn render_gui_post(&self, host: &dyn ClientHost, event: &mut RenderGuiPostEvent<'_>) {
                log("render_gui_post");
                *SEEN_BOUNDS.lock().unwrap() = Some(host.hud_layer_bounds("minecraft:effects"));
                *SEEN_CUSTOM_BOUNDS.lock().unwrap() = Some(host.hud_layer_bounds("post_test:top"));
                *SEEN_UNKNOWN_BOUNDS.lock().unwrap() =
                    Some(host.hud_layer_bounds("post_test:missing"));
                event.draw.fill([4.0, 4.0, 8.0, 8.0], 0xFFFF_00FF);
            }
            fn screen_render_post(
                &self,
                _: &dyn ClientHost,
                event: &mut ScreenRenderPostEvent<'_>,
            ) {
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

            // 1. Visible HUD with pause screen open: render_gui_post runs once after all
            //    visible HUD layers (including AboveAll) and before screen_render_post.
            let _batch = game.hud([320, 240]);
            let events = take();
            let pre_idx = events
                .iter()
                .position(|e| e == "render_gui_pre")
                .expect("render_gui_pre fired");
            let boss_idx = events
                .iter()
                .position(|e| e == "layer_post minecraft:boss_overlay")
                .expect("boss overlay layer fired");
            let tab_idx = events
                .iter()
                .position(|e| e == "layer_post minecraft:tab_list")
                .expect("tab list layer fired");
            let top_idx = events
                .iter()
                .position(|e| e == "layer_post post_test:top")
                .expect("AboveAll mod layer fired");
            let post_indices: Vec<_> = events
                .iter()
                .enumerate()
                .filter_map(|(i, e)| (e == "render_gui_post").then_some(i))
                .collect();
            let screen_idx = events
                .iter()
                .position(|e| e.starts_with("screen_post "))
                .expect("screen_render_post fired");
            assert_eq!(
                post_indices.len(),
                1,
                "render_gui_post must fire once: {events:?}"
            );
            let post_idx = post_indices[0];
            assert!(
                pre_idx < boss_idx
                    && boss_idx < tab_idx
                    && tab_idx < top_idx
                    && top_idx < post_idx
                    && post_idx < screen_idx,
                "unexpected order: {events:?}"
            );
            assert!(
                matches!(*SEEN_BOUNDS.lock().unwrap(), Some(Ok(Some([_, _, w, h]))) if w > 0.0 && h > 0.0),
                "expected drawn minecraft:effects bounds, got {:?}",
                *SEEN_BOUNDS.lock().unwrap()
            );
            assert_eq!(
                *SEEN_CUSTOM_BOUNDS.lock().unwrap(),
                Some(Ok(Some([12.0, 18.0, 20.0, 10.0])))
            );
            assert_eq!(*SEEN_UNKNOWN_BOUNDS.lock().unwrap(), Some(Ok(None)));
            // Bounds are cleared after the HUD pass finishes.
            assert_eq!(game.hud_layer_bounds("post_test:top"), Ok(None));

            // 2. Canceling render_gui_pre suppresses render_gui_post.
            CANCEL_PRE.store(true, Ordering::Relaxed);
            let _batch = game.hud([320, 240]);
            let events = take();
            assert!(events.iter().any(|e| e == "render_gui_pre"));
            assert!(!events.iter().any(|e| e == "render_gui_post"));
            CANCEL_PRE.store(false, Ordering::Relaxed);

            // 3. F1 hiding suppresses both render_gui_pre and render_gui_post.
            game.ui.hide_gui = true;
            let _batch = game.hud([320, 240]);
            let events = take();
            assert!(!events.iter().any(|e| e == "render_gui_pre"));
            assert!(!events.iter().any(|e| e == "render_gui_post"));
            game.ui.hide_gui = false;

            // 4. Outside a client window (dedicated server), hud_layer_bounds refuses explicitly.
            game.options.dedicated = true;
            assert_eq!(
                game.hud_layer_bounds("minecraft:effects"),
                Err(LevelReadError::Unavailable)
            );
        });
    });
}
