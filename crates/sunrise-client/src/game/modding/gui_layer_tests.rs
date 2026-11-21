//! HUD slots in a real client: placement, scoped wrapping and replacement, and layer cancellation.
use super::{
    execution_role::ExecutionRole,
    mods,
    native_entity_refs::tests::{run_fresh_cases, with_client_game},
};
use mod_api::{
    ClientHost, Flow, GuiLayerPlacement, GuiLayerPostEvent, GuiLayerPreEvent, Hooks, ModEntry,
    RegisterGuiLayersEvent, RegistrationContext, RenderGuiPostEvent, RenderGuiPreEvent,
    content::crash::current_native_mod,
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
fn mod_layers_draw_in_their_place_and_can_be_canceled() {
    // The layer order is kept once per process.
    if run_fresh_cases(
        "game::gui_layer_tests::mod_layers_draw_in_their_place_and_can_be_canceled",
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

        // Simulate a vanilla crosshair body whose own guard hides its drawing.
        game.hud_layer_next = Some(0);
        game.gui_layer(VanillaGuiLayer::Crosshair, &mut batch, &mut |_, _| {});
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
                "pre minecraft:crosshair",
                "crosshair wrapper",
                "post minecraft:crosshair",
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
    });
}

#[test]
#[ignore = "daily"]
fn f1_hides_vanilla_paint_without_skipping_the_hud_walk() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let test = "game::gui_layer_tests::f1_hides_vanilla_paint_without_skipping_the_hud_walk";
    crate::game::in_own_process(test, || {
        const MOD_PAINT: u32 = 0xFF13_579B;
        static CANCEL_HUD: AtomicBool = AtomicBool::new(false);

        struct Observer;
        #[mod_api::__subscriptions]
        impl Hooks for Observer {
            fn render_gui_pre(&self, _: &dyn ClientHost, _: &mut RenderGuiPreEvent) -> Flow {
                log("hud pre");
                if CANCEL_HUD.load(Ordering::Relaxed) {
                    Flow::Cancel
                } else {
                    Flow::Continue
                }
            }

            fn register_gui_layers(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterGuiLayersEvent<'_>,
            ) {
                assert!(event.layers.register(
                    "hud_test:indicator".into(),
                    GuiLayerPlacement::AboveAll,
                    Box::new(|_, draw| {
                        log("indicator paint");
                        draw.fill([14.0, 18.0, 4.0, 4.0], MOD_PAINT);
                    }),
                ));
            }

            fn gui_layer_pre(&self, _: &dyn ClientHost, event: &mut GuiLayerPreEvent<'_>) -> Flow {
                log(format!("pre {}", event.layer));
                Flow::Continue
            }

            fn gui_layer_post(&self, _: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
                log(format!("post {}", event.layer));
            }

            fn render_gui_post(&self, _: &dyn ClientHost, _: &mut RenderGuiPostEvent<'_>) {
                log("hud post");
            }
        }

        with_client_game(|game| {
            mod_platform::__private::install(
                vec![ModEntry::new("observer", Box::new(Observer))].into(),
            );
            mods::register_gui_layers(ExecutionRole::Client);
            game.title = None;
            game.screen = super::Screen::Playing;
            game.player = Some(
                sunrise_client::motion::PlayerMotion::new(
                    game.motion,
                    sunrise_core::geometry::Vec3::new(0.5, 64.0, 0.5),
                )
                .unwrap(),
            );
            let rect = game.gui.atlas.gui_rect("hud/hotbar").unwrap();
            let hotbar_uv = [rect.u0, rect.v0];
            let has_hotbar = |frame: &HudBatch| {
                frame
                    .vertices
                    .iter()
                    .any(|v| v.uv == hotbar_uv && v.pos[1] == frame.height - 22.0)
            };
            let has_indicator = |frame: &HudBatch| {
                frame
                    .vertices
                    .iter()
                    .any(|v| v.color == sunrise_client::hud_layout::abgr(MOD_PAINT))
            };
            let mut expected = vec!["hud pre".to_owned()];
            // NeoForge visits hidden and visible slots the same way. The literal IDs also
            // cover normally absent vanilla bodies, e.g. vehicle health with no mount.
            for id in [
                "crosshair",
                "hotbar",
                "effects",
                "boss_overlay",
                "player_health",
                "armor_level",
                "food_level",
                "vehicle_health",
                "air_level",
                "contextual_info_bar",
                "selected_item_name",
                "sleep_overlay",
                "overlay_message",
                "title",
                "scoreboard_sidebar",
                "subtitle_overlay",
                "chat",
                "tab_list",
            ] {
                expected.push(format!("pre minecraft:{id}"));
                expected.push(format!("post minecraft:{id}"));
            }
            expected.extend(
                [
                    "pre hud_test:indicator",
                    "indicator paint",
                    "post hud_test:indicator",
                    "hud post",
                ]
                .map(str::to_owned),
            );

            for hidden in [false, true] {
                game.ui.hide_gui = hidden;
                let frame = game.hud_layers([640, 480]);
                assert_eq!(take(), expected);
                assert_eq!(has_hotbar(&frame), !hidden);
                assert!(has_indicator(&frame));
            }
            CANCEL_HUD.store(true, Ordering::Relaxed);
            let frame = game.hud_layers([640, 480]);
            assert_eq!(take(), ["hud pre"]);
            assert!(!has_hotbar(&frame));
            assert!(!has_indicator(&frame));
        });
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
