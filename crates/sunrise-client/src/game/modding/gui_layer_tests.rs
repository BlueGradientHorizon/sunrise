//! HUD slots in a real client: placement, scoped wrapping and replacement, and layer cancellation.
use super::{
    execution_role::ExecutionRole,
    mods,
    native_entity_refs::tests::{run_fresh_cases, with_client_game},
};
use mod_api::{
    ClientHost, CustomizeChatOverlayEvent, Flow, GuiLayerPlacement, GuiLayerPostEvent,
    GuiLayerPreEvent, Hooks, ModEntry, PlayerHeartTypeEvent, RegisterGuiLayersEvent,
    RegistrationContext, RenderGuiPostEvent, RenderGuiPreEvent, ToastAddEvent,
    content::crash::current_native_mod,
};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
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
            (VanillaGuiLayer::PlayerHealth, "health paint"),
            (VanillaGuiLayer::ContextualInfoBar, "contextual paint"),
            (VanillaGuiLayer::Effects, "effects paint"),
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
                "pre minecraft:effects",
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

        // With no insertion cursor active, a standalone vanilla slot still fires its hooks.
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
fn focused_chat_is_separate_from_its_hud_slot_even_when_hud_is_canceled() {
    if run_fresh_cases(
        "game::gui_layer_tests::focused_chat_is_separate_from_its_hud_slot_even_when_hud_is_canceled",
        "SUNRISE_FOCUSED_CHAT_HUD_FIXTURE",
        &["run"],
    ) {
        return;
    }

    const BODY: u32 = 0xFF13_579B;
    const MESSAGE: u32 = 0x29_AD64;
    static CANCEL_HUD: AtomicBool = AtomicBool::new(false);
    struct ChatLayer;
    #[mod_api::__subscriptions]
    impl Hooks for ChatLayer {
        fn customize_chat_overlay(
            &self,
            _: &mut dyn ClientHost,
            _: &mut CustomizeChatOverlayEvent,
        ) {
            log("customize background chat");
        }

        fn register_gui_layers(
            &self,
            _: &RegistrationContext,
            event: &mut RegisterGuiLayersEvent<'_>,
        ) {
            assert!(event.layers.replace_layer(
                "minecraft:chat",
                Box::new(|_, draw| {
                    log("replacement");
                    draw.fill([0.0, 0.0, 4.0, 4.0], BODY);
                }),
            ));
        }
        fn gui_layer_pre(&self, _: &dyn ClientHost, event: &mut GuiLayerPreEvent<'_>) -> Flow {
            if event.layer == "minecraft:chat" {
                log("chat pre");
            }
            Flow::Continue
        }
        fn gui_layer_post(&self, _: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
            if event.layer == "minecraft:chat" {
                log("chat post");
            }
        }
        fn render_gui_post(&self, _: &dyn ClientHost, _: &mut RenderGuiPostEvent<'_>) {
            log("hud post");
        }
        fn render_gui_pre(&self, _: &dyn ClientHost, _: &mut RenderGuiPreEvent) -> Flow {
            if CANCEL_HUD.load(Ordering::Relaxed) {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        }
    }

    with_client_game(|game| {
        mod_platform::__private::install(
            vec![ModEntry::new("chat_layer", Box::new(ChatLayer))].into(),
        );
        mods::register_gui_layers(ExecutionRole::Client);
        game.title = None;
        game.screen = super::Screen::Playing;
        game.ui.hide_gui = false;
        game.player = Some(
            sunrise_client::motion::PlayerMotion::new(
                game.motion,
                sunrise_core::geometry::Vec3::new(0.5, 64.0, 0.5),
            )
            .unwrap(),
        );
        game.begin_chat_screen("", false);
        assert!(game.screen == super::Screen::Chat);
        game.system_message(
            sunrise_client::text::Text::literal("Focused chat").with_color(MESSAGE),
        );
        for canceled in [false, true] {
            CANCEL_HUD.store(canceled, Ordering::Relaxed);
            let batch = game.hud_layers([640, 480]);
            let (hud, screen) = batch.vertices.split_at(batch.blur_at as usize);
            let expected: &[&str] = if canceled {
                &[]
            } else {
                &["chat pre", "replacement", "chat post", "hud post"]
            };
            assert_eq!(take().as_slice(), expected);
            assert_eq!(
                hud.iter()
                    .filter(|v| v.color == sunrise_client::hud_layout::abgr(BODY))
                    .count(),
                if canceled { 0 } else { 6 },
            );
            assert!(
                screen
                    .iter()
                    .any(|v| v.color == sunrise_client::hud_layout::abgr(0xFF00_0000 | MESSAGE))
            );
        }
    });
}

#[test]
#[ignore = "daily"]
fn a_hidden_retained_layer_panic_discards_only_that_callback_and_reports_its_owner() {
    const FLAG: &str = "SUNRISE_PANICKING_HUD_LAYER_FIXTURE";
    if run_fresh_cases(
        "game::gui_layer_tests::a_hidden_retained_layer_panic_discards_only_that_callback_and_reports_its_owner",
        FLAG,
        &["replacement", "wrapper"],
    ) {
        return;
    }
    const FAILED: u32 = 0xFF67_1234;
    const HEALTHY: u32 = 0xFF25_EFC1;

    struct HiddenLayer(bool);
    #[mod_api::__subscriptions]
    impl Hooks for HiddenLayer {
        fn toast_add(&self, _: &dyn ClientHost, event: &mut ToastAddEvent<'_>) -> Flow {
            if event.title.plain().contains("hidden_layer") {
                log("diagnostic was offered to a toast listener");
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
            let fail = |draw: &mut dyn mod_api::DrawContext| {
                log("panic");
                draw.fill([0.0, 0.0, 4.0, 4.0], FAILED);
                panic!("hidden layer test");
            };
            if self.0 {
                assert!(event.layers.wrap_layer(
                    "minecraft:player_health",
                    Box::new(move |ctx| ctx.with_draw(&mut |draw| fail(draw))),
                ));
            } else {
                assert!(event.layers.replace_layer(
                    "minecraft:player_health",
                    Box::new(move |_, draw| fail(draw)),
                ));
            }
            assert!(event.layers.register(
                "hud_test:after".into(),
                GuiLayerPlacement::Above("minecraft:player_health".into()),
                Box::new(|_, draw| {
                    log("healthy");
                    draw.fill([0.0, 0.0, 4.0, 4.0], HEALTHY);
                }),
            ));
        }
    }

    with_client_game(|game| {
        let wrapper = std::env::var(FLAG).unwrap() == "wrapper";
        mod_platform::__private::install(
            vec![ModEntry::new(
                "hidden_layer",
                Box::new(HiddenLayer(wrapper)),
            )]
            .into(),
        );
        mods::register_gui_layers(ExecutionRole::Client);
        game.title = None;
        game.screen = super::Screen::Playing;
        game.ui.hide_gui = true;
        for (i, expected) in [&["panic", "healthy"][..], &["healthy"][..]]
            .into_iter()
            .enumerate()
        {
            let frame = game.hud_layers([640, 480]);
            assert_eq!(take().as_slice(), expected);
            assert!(
                frame
                    .vertices
                    .iter()
                    .all(|v| v.color != sunrise_client::hud_layout::abgr(FAILED))
            );
            assert!(
                frame
                    .vertices
                    .iter()
                    .any(|v| v.color == sunrise_client::hud_layout::abgr(HEALTHY))
            );
            assert_eq!(game.mod_error_toasts.len(), 1);
            assert!(
                game.mod_error_toasts[0]
                    .title
                    .plain()
                    .contains("hidden_layer")
            );
            let hook = if wrapper {
                "gui_layer_wrapper"
            } else {
                "gui_layer_callback"
            };
            assert!(
                game.mod_error_toasts[0]
                    .message
                    .as_ref()
                    .unwrap()
                    .plain()
                    .contains(hook)
            );
            if i == 0 {
                // Keep F1 on and advance the same toast past its slide-in without sleeping.
                game.mod_error_toasts[0].slide.origin =
                    Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
            } else {
                assert!(
                    frame.vertices.iter().any(|v| {
                        v.color == sunrise_client::hud::rgba(255, 85, 85, 255)
                            && v.pos[0] >= 0.0
                            && v.pos[0] < frame.width
                            && v.pos[1] >= 0.0
                            && v.pos[1] < frame.height
                    }),
                    "F1 must not hide the diagnostic's red title"
                );
            }
        }
    });
}

#[test]
#[ignore = "daily"]
fn a_panicking_wrapper_keeps_completed_hearts_with_their_armor_offset() {
    if run_fresh_cases(
        "game::gui_layer_tests::a_panicking_wrapper_keeps_completed_hearts_with_their_armor_offset",
        "SUNRISE_WRAPPED_HEARTS_PANIC_FIXTURE",
        &["run"],
    ) {
        return;
    }
    const FAILED: u32 = 0xFF86_1254;
    struct WrapHearts;
    #[mod_api::__subscriptions]
    impl Hooks for WrapHearts {
        fn player_heart_type(&self, _: &mut dyn ClientHost, _: &mut PlayerHeartTypeEvent) {
            log("original hearts body");
        }

        fn register_gui_layers(
            &self,
            _: &RegistrationContext,
            event: &mut RegisterGuiLayersEvent<'_>,
        ) {
            assert!(event.layers.wrap_layer(
                "minecraft:player_health",
                Box::new(|ctx| {
                    let paint = &mut |draw: &mut dyn mod_api::DrawContext| {
                        draw.fill([0.0, 0.0, 4.0, 4.0], FAILED);
                    };
                    ctx.with_draw(paint);
                    ctx.draw_previous();
                    ctx.with_draw(paint);
                    panic!("wrapper panicked after vanilla hearts drew");
                })
            ));
        }
    }
    with_client_game(|game| {
        mod_platform::__private::install(
            vec![ModEntry::new("wrap_hearts", Box::new(WrapHearts))].into(),
        );
        mods::register_gui_layers(ExecutionRole::Client);
        game.title = None;
        game.screen = super::Screen::Playing;
        game.ui.hide_gui = false;
        game.creative = false;
        let mut player = sunrise_client::motion::PlayerMotion::new(
            game.motion,
            sunrise_core::geometry::Vec3::new(0.5, 64.0, 0.5),
        )
        .unwrap();
        let key = player.attributes.key("minecraft:armor").unwrap();
        let armor = player.attributes.get_instance(Some(key)).unwrap().unwrap();
        player
            .attributes
            .edit(&armor, |a, o| a.set_base_value(20.0, o))
            .unwrap();
        game.player = Some(player);
        let corner = |sprite| {
            let rect = game.gui.atlas.gui_draw_rect(sprite).unwrap();
            [rect.u0, rect.v0]
        };
        let hearts = corner("hud/heart/container");
        let armor = corner("hud/armor_full");
        let frame = game.hud_layers([640, 480]);
        let has = |frame: &HudBatch, uv, from_bottom| {
            frame
                .vertices
                .iter()
                .any(|v| v.uv == uv && v.pos[1] == frame.height - from_bottom)
        };
        assert_eq!(take(), ["original hearts body"]);
        assert!(has(&frame, hearts, 39.0));
        assert!(has(&frame, armor, 49.0));
        assert!(
            frame
                .vertices
                .iter()
                .all(|v| v.color != sunrise_client::hud_layout::abgr(FAILED))
        );
        let next = game.hud_layers([640, 480]);
        assert!(take().is_empty());
        assert!(!has(&next, hearts, 39.0));
        assert!(has(&next, armor, 39.0));
    });
}

#[test]
#[ignore = "daily"]
fn a_retained_gui_callback_reports_two_refusals_without_being_disabled() {
    if run_fresh_cases(
        "game::gui_layer_tests::a_retained_gui_callback_reports_two_refusals_without_being_disabled",
        "SUNRISE_RETAINED_GUI_REFUSAL_FIXTURE",
        &["run"],
    ) {
        return;
    }
    struct Warnings;
    impl log::Log for Warnings {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool {
            true
        }
        fn log(&self, record: &log::Record<'_>) {
            if record.level() == log::Level::Warn {
                let message = record.args().to_string();
                if message.contains("refusing_layer hook gui_layer_callback") {
                    log(message);
                }
            }
        }
        fn flush(&self) {}
    }
    log::set_logger(&Warnings).unwrap();
    log::set_max_level(log::LevelFilter::Warn);

    struct Refusing;
    #[mod_api::__subscriptions]
    impl Hooks for Refusing {
        fn register_gui_layers(
            &self,
            _: &RegistrationContext,
            event: &mut RegisterGuiLayersEvent<'_>,
        ) {
            assert!(event.layers.replace_layer(
                "minecraft:player_health",
                Box::new(|_, _| {
                    mod_api::__private::refuse(
                        "gui_test_read",
                        mod_api::__private::current_phase(),
                    );
                })
            ));
        }
    }
    with_client_game(|game| {
        mod_platform::__private::install(
            vec![ModEntry::new("refusing_layer", Box::new(Refusing))].into(),
        );
        mods::register_gui_layers(ExecutionRole::Client);
        game.title = None;
        game.screen = super::Screen::Playing;
        for _ in 0..2 {
            game.hud_layers([640, 480]);
            let warnings = take();
            assert_eq!(warnings.len(), 1);
            assert!(warnings[0].contains("mod API gui_test_read is unavailable"));
            assert!(game.mod_error_toasts.is_empty());
        }
    });
}
