use super::*;

impl Game<'_> {
    /// Collect attributed world-preset editor factories once at client startup
    /// (NeoForge's `RegisterPresetEditorsEvent`).
    pub(in crate::game) fn native_register_preset_editors(
        context: &mod_api::RegistrationContext,
        editors: &mut Vec<Answered<PresetEditorFactoryRegistration>>,
    ) {
        Self::fire_register_preset_editors(context, editors);
    }

    pub(in crate::game) fn preset_editor_title(&self) -> Option<&sunrise_client::title::Title> {
        if self.screen != Screen::Playing || self.screen_over_playing() || self.remote_loading() {
            return None;
        }
        self.title
            .as_ref()
            .filter(|title| title.preset_editor_session().is_some())
    }

    pub(in crate::game) fn preset_editor_dimensions(&self) -> Option<mod_api::Compound> {
        let sunrise_core::nbt::Tag::Compound(dimensions) =
            self.preset_editor_title()?.preset_editor_dimensions()?
        else {
            return None;
        };
        Some(dimensions)
    }

    pub(in crate::game) fn set_preset_editor_dimensions(
        &self,
        dimensions: mod_api::Compound,
    ) -> bool {
        self.preset_editor_title().is_some_and(|title| {
            title.set_preset_editor_dimensions(sunrise_core::nbt::Tag::Compound(dimensions))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{
        DrawContext, Hooks, Identifier, ModEntry, PresetEditor, PresetEditorDraft,
        RegisterPresetEditorsEvent, ScreenCharTypedPostEvent, ScreenCharTypedPreEvent,
        ScreenKeyPressedPostEvent, ScreenKeyPressedPreEvent, ScreenRenderPostEvent,
    };
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use sunrise_client::title::{Mode, WorldOptions};

    struct IslandsEditor {
        clicks: Arc<AtomicUsize>,
        keys: Arc<AtomicUsize>,
        typed: Arc<Mutex<String>>,
    }

    impl PresetEditor for IslandsEditor {
        fn title(&self) -> Option<String> {
            Some("Islands Editor".into())
        }

        fn render(
            &mut self,
            draw: &mut dyn DrawContext,
            rect: [f32; 4],
            _draft: &mut PresetEditorDraft<'_>,
        ) {
            draw.fill([rect[0] + 10.0, rect[1] + 24.0, 40.0, 16.0], 0xFF336699);
        }

        fn mouse_clicked(
            &mut self,
            _x: f32,
            _y: f32,
            button: u8,
            draft: &mut PresetEditorDraft<'_>,
        ) -> bool {
            if button != 0 {
                return false;
            }
            self.clicks.fetch_add(1, Ordering::Relaxed);
            let mut dimensions = draft.dimensions().clone();
            if let Some(sunrise_core::nbt::Tag::Compound(overworld)) =
                dimensions.get_mut(&"minecraft:overworld".into())
            {
                overworld.insert(
                    "type".into(),
                    sunrise_core::nbt::Tag::String("minecraft:the_nether".into()),
                );
            }
            assert!(draft.set_dimensions(dimensions));
            true
        }

        fn key_pressed(
            &mut self,
            key_code: u32,
            _scan_code: u32,
            _modifiers: u32,
            _draft: &mut PresetEditorDraft<'_>,
        ) -> bool {
            // Consume Tab (GLFW 258) ahead of menu focus navigation.
            if key_code == 258 {
                self.keys.fetch_add(1, Ordering::Relaxed);
                return true;
            }
            false
        }

        fn char_typed(
            &mut self,
            ch: char,
            _modifiers: u32,
            _draft: &mut PresetEditorDraft<'_>,
        ) -> bool {
            if ch == 'x' {
                return false;
            }
            self.typed
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(ch);
            true
        }
    }

    struct EditorMod {
        clicks: Arc<AtomicUsize>,
        keys: Arc<AtomicUsize>,
        typed: Arc<Mutex<String>>,
        char_post: Arc<Mutex<String>>,
        key_post: Arc<Mutex<Vec<String>>>,
    }
    #[mod_api::__subscriptions]
    impl Hooks for EditorMod {
        fn screen_char_typed_pre(
            &self,
            _: &dyn mod_api::ClientHost,
            event: &mut ScreenCharTypedPreEvent<'_>,
        ) -> mod_api::Flow {
            if event.screen.id == "world_preset_editor" && event.c == '!' {
                mod_api::Flow::Cancel
            } else {
                mod_api::Flow::Continue
            }
        }
        fn screen_char_typed_post(
            &self,
            _: &dyn mod_api::ClientHost,
            event: &mut ScreenCharTypedPostEvent<'_>,
        ) -> mod_api::Flow {
            if event.screen.id == "world_preset_editor" {
                self.char_post.lock().unwrap().push(event.c);
            }
            mod_api::Flow::Continue
        }
        fn screen_key_pressed_post(
            &self,
            _: &dyn mod_api::ClientHost,
            event: &mut ScreenKeyPressedPostEvent<'_>,
        ) -> mod_api::Flow {
            if event.screen.id == "world_preset_editor" {
                self.key_post.lock().unwrap().push(event.key.to_owned());
            }
            mod_api::Flow::Continue
        }

        fn register_preset_editors(
            &self,
            _: &mod_api::RegistrationContext,
            event: &mut RegisterPresetEditorsEvent<'_>,
        ) {
            let clicks = Arc::clone(&self.clicks);
            let keys = Arc::clone(&self.keys);
            let typed = Arc::clone(&self.typed);
            event.register(Identifier::parse("example:islands").unwrap(), move |ctx| {
                assert_eq!(ctx.preset(), &Identifier::parse("example:islands").unwrap());
                assert_eq!(ctx.seed(), "1");
                Ok(Box::new(IslandsEditor {
                    clicks: Arc::clone(&clicks),
                    keys: Arc::clone(&keys),
                    typed: Arc::clone(&typed),
                }))
            });
        }
        fn screen_render_post(
            &self,
            host: &dyn mod_api::ClientHost,
            event: &mut ScreenRenderPostEvent<'_>,
        ) {
            if event.screen.id == "world_preset_editor" && host.preset_editor_dimensions().is_some()
            {
                event.draw.fill([20.0, 20.0, 50.0, 20.0], 0xFF00FF00);
            }
        }
        fn screen_key_pressed_pre(
            &self,
            host: &dyn mod_api::ClientHost,
            event: &mut ScreenKeyPressedPreEvent<'_>,
        ) -> mod_api::Flow {
            if event.key != "key.keyboard.space" || event.screen.id != "world_preset_editor" {
                return mod_api::Flow::Continue;
            }
            let mut dimensions = host.preset_editor_dimensions().unwrap();
            let sunrise_core::nbt::Tag::Compound(overworld) =
                dimensions.get_mut(&"minecraft:overworld".into()).unwrap()
            else {
                unreachable!()
            };
            overworld.insert(
                "type".into(),
                sunrise_core::nbt::Tag::String("minecraft:overworld_caves".into()),
            );
            assert!(host.set_preset_editor_dimensions(dimensions));
            mod_api::Flow::Cancel
        }
    }

    #[test]
    #[ignore = "daily"]
    fn preset_editor_test_mod_draws_and_edits_through_real_client_hooks() {
        struct NoopLoop;
        impl crate::game::event_loop::EventLoopCtl for NoopLoop {
            fn exit(&self) {}
            fn set_control_flow(&self, _: winit::event_loop::ControlFlow) {}
        }

        let clicks = Arc::new(AtomicUsize::new(0));
        let keys = Arc::new(AtomicUsize::new(0));
        let typed = Arc::new(Mutex::new(String::new()));
        let char_post = Arc::new(Mutex::new(String::new()));
        let key_post = Arc::new(Mutex::new(Vec::new()));
        mod_platform::testing::with_mods(
            vec![ModEntry::new(
                "example",
                Box::new(EditorMod {
                    clicks: Arc::clone(&clicks),
                    keys: Arc::clone(&keys),
                    typed: Arc::clone(&typed),
                    char_post: Arc::clone(&char_post),
                    key_post: Arc::clone(&key_post),
                }),
            )]
            .into(),
            || {
                crate::game::native_entity_refs::tests::with_client_game(|game| {
                    let _window =
                        crate::game::native_hooks::particle_count_tests::frame_scene(game);
                    use mod_api::__private::game::RegistrationContextNew;
                    let mut editors = Vec::new();
                    Game::native_register_preset_editors(
                        &mod_api::RegistrationContext::new(),
                        &mut editors,
                    );
                    assert_eq!(editors.len(), 1);
                    let mut title = game.new_title(game.save.root.join("editor-saves"));
                    title.set_preset_data_source(game.pack.data_changes.clone());
                    let dimensions = sunrise_core::nbt::Tag::compound([(
                        "minecraft:overworld",
                        sunrise_core::nbt::Tag::compound([
                            (
                                "type",
                                sunrise_core::nbt::Tag::String("minecraft:overworld".into()),
                            ),
                            (
                                "generator",
                                sunrise_core::nbt::Tag::compound([(
                                    "type",
                                    sunrise_core::nbt::Tag::String("minecraft:debug".into()),
                                )]),
                            ),
                        ]),
                    )]);
                    sunrise_client::title::preset_editors::register(editors);
                    title.mode = Mode::Create {
                        name: "Test".into(),
                        seed: "1".into(),
                        creative: false,
                        focus: 0,
                        options: WorldOptions {
                            custom_preset: Some("example:islands".into()),
                            presets: [("example:islands".into(), dimensions)].into(),
                            preset_order: vec!["example:islands".into()],
                            tab: 1,
                            ..Default::default()
                        },
                    };
                    title.focused_button = Some(26);
                    title.confirm_key_size((320.0, 240.0));
                    assert_eq!(title.mode.id(), "world_preset_editor");
                    assert_eq!(
                        title.preset_editor_session().unwrap().title(),
                        Some("Islands Editor")
                    );
                    game.title = Some(title);
                    game.screen = Screen::Playing;
                    let mut batch = HudBatch::new([640, 480], game.settings.gui_scale_inputs());
                    game.title.as_ref().unwrap().draw(
                        &game.gui.atlas,
                        &mut batch,
                        (0.0, 0.0),
                        false,
                        &mut |phase, batch| game.draw_screen_phase(batch, phase),
                    );
                    let stock_vertices = batch.vertices.len();
                    game.draw_preset_editor(&mut batch);
                    let after_editor_vertices = batch.vertices.len();
                    assert!(after_editor_vertices > stock_vertices);
                    let view = game.title_screen_render_view(&batch).unwrap();
                    assert_eq!(view.id, "world_preset_editor");
                    game.native_screen_render_post(&mut batch, &view);
                    assert!(batch.vertices.len() > after_editor_vertices);
                    assert!(game.preset_editor_mouse_clicked(MouseButton::Left));
                    // IME and the socket both send characters independently of the key path.
                    game.handle_window_event_inner(
                        &NoopLoop,
                        winit::event::WindowEvent::Ime(winit::event::Ime::Commit("é!x".into())),
                        None,
                    );
                    let (reply, _) = std::sync::mpsc::channel();
                    game.control_command("type Ωx", &reply, &NoopLoop).unwrap();
                    // Tab is consumed by the editor; Q is declined by both the editor and its widgets.
                    game.control_command("key tab tap", &reply, &NoopLoop)
                        .unwrap();
                    game.preset_editor_key_pressed(&NoopLoop, KeyCode::KeyQ, false)
                        .unwrap();
                    assert_eq!(*key_post.lock().unwrap(), ["key.keyboard.q"]);
                    assert_eq!(keys.load(Ordering::Relaxed), 1);
                    assert_eq!(clicks.load(Ordering::Relaxed), 1);
                    assert_eq!(char_post.lock().unwrap().as_str(), "xx");
                    assert_eq!(typed.lock().unwrap().as_str(), "éΩ");
                    let after_click = game.preset_editor_dimensions().unwrap();
                    assert_eq!(
                        after_click
                            .get(&"minecraft:overworld".into())
                            .unwrap()
                            .get("type"),
                        Some(&sunrise_core::nbt::Tag::String(
                            "minecraft:the_nether".into()
                        ))
                    );
                    game.preset_editor_key_pressed(&NoopLoop, KeyCode::Space, false)
                        .unwrap();
                    game.screen_chars_typed(Some("z"));
                    assert_eq!(typed.lock().unwrap().as_str(), "éΩz");
                    assert_eq!(*key_post.lock().unwrap(), ["key.keyboard.q"]);
                    let dimensions = game.preset_editor_dimensions().unwrap();
                    assert_eq!(
                        dimensions
                            .get(&"minecraft:overworld".into())
                            .unwrap()
                            .get("type"),
                        Some(&sunrise_core::nbt::Tag::String(
                            "minecraft:overworld_caves".into()
                        ))
                    );
                    game.screen = Screen::Options;
                    assert!(game.preset_editor_dimensions().is_none());
                    assert!(!game.set_preset_editor_dimensions(dimensions));
                    sunrise_client::title::preset_editors::register(Vec::new());
                });
            },
        );
    }
}
