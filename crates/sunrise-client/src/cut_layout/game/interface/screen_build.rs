

#[cfg(test)]
#[cfg(feature = "client")]
pub(crate) mod tests {

use sunrise_client_gameplay::game::client_side::native_client_session::tests::logged_client;

use sunrise_client_gameplay::game::client_side::screen_build::{
        BuildLayout, ContainerLayout, PauseInputs, PauseLayout, ScreenBuildState, ScreenLayout,
    };

use sunrise_client::inventory_effects::EffectsLayout;

use crate::game::Screen;

use sunrise_client::hud::{GuiScale, HudBatch};

/// The chat box is re-made at the build's size when the GUI scale
    /// changes, with no draw in between (`ChatScreen.resize`), and the Leave
    /// Bed button is laid out once per build. The rectangles are
    /// `ChatScreen.init` and `InBedChatScreen.init` worked by hand for the
    /// test's 1280x720 window.
    #[test]
    #[ignore = "daily"]
    fn the_chat_box_is_resized_by_the_build_not_the_draw() {
        crate::game::server_side::native_entity_refs::tests::with_client_game(|game| {
            use sunrise_client::chat::{ChatAbilities, ChatScreen, ChatVisibility, Commands};
            game.side.title.0 = None;
            // 1280x720 at GUI scale 3 is 427x240.
            game.side.settings.gui_scale = 3;
            let m = sunrise_client::screen_draw::Measure(&game.side.gui.atlas.font);
            let c = Commands {
                dispatcher: sunrise_command::vanilla::game_dispatcher(),
                registries: &game.ui.registries,
            };
            let abilities = ChatAbilities::new(ChatVisibility::Full);
            let screen = ChatScreen::new(&m, &c, "/say hi", false, 0, 427, 240, abilities);
            game.side.chat_ui.chat_screen = Some(screen);
            game.side.screen = Screen::Chat;
            game.build_screen();
            let input = |game: &sunrise_client_gameplay::game::ClientGame<'_>| {
                let i = &game.client.chat_ui.chat_screen.as_ref().unwrap().input;
                [i.x, i.y, i.width, i.height]
            };
            assert_eq!(input(game), [4, 228, 423, 12]);
            let leave_bed = |game: &sunrise_client_gameplay::game::ClientGame<'_>, w, h| {
                game.built_chat(w, h).map(|layout| layout.leave_bed)
            };
            assert_eq!(leave_bed(game, 427.0, 240.0), Some((113, 200, 200, 20)));
            let builds = game.side.screen_state.0.build.builds;
            game.build_screen();
            assert_eq!(
                game.side.screen_state.0.build.builds, builds,
                "an unchanged frame"
            );
            // GUI scale 1: the build re-makes the box, keeping its text.
            game.side.settings.gui_scale = 1;
            game.build_screen();
            assert_eq!(game.side.screen_state.0.build.builds, builds + 1);
            assert_eq!(input(game), [4, 708, 1276, 12]);
            assert_eq!(
                game.side.chat_ui.chat_screen.as_ref().unwrap().value(),
                "/say hi"
            );
            assert_eq!(leave_bed(game, 1280.0, 720.0), Some((540, 680, 200, 20)));
            assert_eq!(leave_bed(game, 427.0, 240.0), None);
        });
    }
#[test]
    #[ignore = "daily"]
    fn production_screen_build_geometry() {
        crate::game::server_side::native_entity_refs::tests::with_client_game(|game| {
            game.side.screen = Screen::Inventory;
            game.container = None;
            game.craft_size = 2;
            game.side.title.0 = None;
            game.potions.effects.list =
                vec![sunrise_level::effects::EffectInstance::new("speed", 200, 0)];
            let batch = HudBatch::new(
                [320, 240],
                GuiScale {
                    requested: 1,
                    unicode: false,
                },
            );
            let layout = game.container_layout(&batch);
            assert_eq!(layout.origin, (72.0, 37.0));
            assert_eq!(layout.effects, Some([250.0, 37.0, 32.0, 32.0]));
            game.side.screen = Screen::Creative;
            let layout = game.container_layout(&batch);
            assert_eq!(
                layout.tabs,
                Some([[62.0, 24.0, 195.0, 28.0], [62.0, 188.0, 195.0, 28.0]])
            );
            game.build_screen();
            let before = game.side.screen_state.0.build.builds;
            let constructions =
                sunrise_client_gameplay::game::client_side::container_layout::LAYOUT_CONSTRUCTIONS.with(|count| count.get());
            let slots = match &game
                .side
                .screen_state
                .0
                .build
                .current
                .as_ref()
                .unwrap()
                .layout
            {
                BuildLayout::Container(layout) => layout.slots.as_ptr(),
                _ => panic!("container"),
            };
            let positions = sunrise_gameplay_common::game::inventory_slots::SLOT_POSITIONS.with(|count| count.get());
            game.build_screen();
            assert_eq!(
                crate::game::inventory_slots::SLOT_POSITIONS.with(|count| count.get()),
                positions
            );
            assert_eq!(game.side.screen_state.0.build.builds, before);
            assert_eq!(
                sunrise_client_gameplay::game::client_side::container_layout::LAYOUT_CONSTRUCTIONS.with(|count| count.get()),
                constructions
            );
            let BuildLayout::Container(layout) = &game
                .side
                .screen_state
                .0
                .build
                .current
                .as_ref()
                .unwrap()
                .layout
            else {
                panic!("container")
            };
            assert_eq!(layout.slots.as_ptr(), slots);
            // Each changed input builds once; its next frame does no slot work.
            let check = |game: &mut sunrise_client_gameplay::game::ClientGame<'_>| {
                let builds = sunrise_client_gameplay::game::client_side::container_layout::LAYOUT_CONSTRUCTIONS.with(|c| c.get());
                game.build_screen();
                assert_eq!(
                    sunrise_client_gameplay::game::client_side::container_layout::LAYOUT_CONSTRUCTIONS.with(|c| c.get()),
                    builds + 1
                );
                let positions = sunrise_gameplay_common::game::inventory_slots::SLOT_POSITIONS.with(|c| c.get());
                game.build_screen();
                assert_eq!(
                    crate::game::inventory_slots::SLOT_POSITIONS.with(|c| c.get()),
                    positions
                );
            };
            game.side.settings.gui_scale = 1;
            check(game);
            game.side.screen = Screen::Inventory;
            check(game);
            game.level.recipe_book.open[0] = true;
            check(game);
            game.potions.effects.list.clear();
            check(game);
            game.craft_size = 3;
            check(game);
            game.open_station_client(sunrise_client_gameplay::game::client_side::stations::Station::new_kind(
                sunrise_client_gameplay::game::client_side::stations::Kind::Merchant,
                [0, 0, 0],
            ));
            check(game);
            game.clear_station_client();
            game.side.screen = Screen::Creative;
            check(game);
            let inventory = game
                .side
                .creative_screen
                .0
                .tab_headers()
                .iter()
                .position(|tab| tab.inventory)
                .unwrap();
            game.side.creative_screen.0.select(inventory);
            check(game);
        });
    }
/// The display's change builds twice in one frame (`apply_ui_settings`
    /// then `observe_screen`): the effects hook still fires once, and the next
    /// frame fires it again. A mod's once-per-frame side effect stays once.
    #[test]
    #[ignore = "daily"]
    fn the_effects_answer_is_taken_once_a_frame() {
        use mod_api::{ClientHost, Hooks, ModEntry, ScreenRenderInventoryMobEffectsEvent};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        /// A mod listening to the effects hook, counting its dispatches.
        struct Effects(Arc<AtomicUsize>);
        #[mod_api::__subscriptions]
        impl Hooks for Effects {
            /// Counts the dispatch the build asks for.
            fn screen_render_inventory_mob_effects(
                &self,
                _: &dyn ClientHost,
                _: &mut ScreenRenderInventoryMobEffectsEvent<'_>,
            ) -> mod_api::Flow {
                self.0.fetch_add(1, Ordering::Relaxed);
                mod_api::Flow::Continue
            }
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let mods: mod_api::Mods = vec![ModEntry::new(
            "effects",
            Box::new(Effects(Arc::clone(&calls))),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            crate::game::server_side::native_entity_refs::tests::with_client_game(|game| {
                game.side.screen = Screen::Inventory;
                game.container = None;
                game.craft_size = 2;
                game.potions.effects.list =
                    vec![sunrise_level::effects::EffectInstance::new("speed", 200, 0)];
                game.side.frames = 11;
                game.build_screen();
                assert_eq!(
                    calls.load(Ordering::Relaxed),
                    1,
                    "the frame's first build asks the mods"
                );
                game.build_screen();
                assert_eq!(
                    calls.load(Ordering::Relaxed),
                    1,
                    "a second build in the same frame reuses the answer"
                );
                game.side.frames = 12;
                game.build_screen();
                assert_eq!(
                    calls.load(Ordering::Relaxed),
                    2,
                    "the next frame asks again"
                );
                assert_eq!(calls.load(Ordering::Relaxed), 2, "and only once");
            });
        });
    }
#[test]
    #[ignore = "daily"]
    fn a_mods_room_moves_the_window_and_is_asked_once_a_frame() {
        use mod_api::{ClientHost, Hooks, ModEntry, ScreenRoomEvent};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use sunrise_client::hud::HudBatch;

        struct Panel {
            calls: Arc<AtomicUsize>,
            width: Arc<AtomicUsize>,
        }
        #[mod_api::__subscriptions]
        impl Hooks for Panel {
            fn screen_room(&self, _: &dyn ClientHost, event: &mut ScreenRoomEvent<'_>) {
                // The query sees the unreserved window, not the prior frame's shifted one.
                assert_eq!(event.screen.rect[0], 125.0);
                self.calls.fetch_add(1, Ordering::Relaxed);
                event.right = event.right.max(self.width.load(Ordering::Relaxed) as f32);
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let width = Arc::new(AtomicUsize::new(149));
        let mods: mod_api::Mods = vec![
            ModEntry::new(
                "small_panel",
                Box::new(Panel {
                    calls: calls.clone(),
                    width: Arc::new(AtomicUsize::new(80)),
                }),
            ),
            ModEntry::new(
                "large_panel",
                Box::new(Panel {
                    calls: calls.clone(),
                    width: width.clone(),
                }),
            ),
        ]
        .into();
        mod_platform::testing::with_mods(mods, || {
            crate::game::server_side::native_entity_refs::tests::with_client_game(|game| {
                game.side.screen = Screen::Inventory;
                game.container = None;
                game.craft_size = 2;
                game.level.recipe_book.open.fill(false);
                game.side.settings.gui_scale = 3;
                game.side.frames = 11;
                game.build_screen();
                let batch = HudBatch::new([1280, 720], game.side.settings.gui_scale_inputs());
                assert_eq!(game.inventory_origin(&batch), (51.0, 37.0));
                assert_eq!(
                    game.screen_view(&batch, Screen::Inventory).rect,
                    [51.0, 37.0, 176.0, 166.0]
                );
                let slot = game
                    .built_container(&batch)
                    .unwrap()
                    .slots
                    .iter()
                    .find(|(id, _)| *id == 0)
                    .unwrap()
                    .1;
                assert_eq!(slot, [58.0, 178.0, 18.0, 18.0]);
                assert_eq!(game.slot_at(&batch, 59.0, 179.0), Some(0));
                assert_eq!(game.slot_at(&batch, 133.0, 179.0), Some(4));
                let builds = game.side.screen_state.0.build.builds;
                assert_eq!(calls.load(Ordering::Relaxed), 2);
                game.build_screen();
                assert_eq!(calls.load(Ordering::Relaxed), 2);
                assert_eq!(game.side.screen_state.0.build.builds, builds);
                game.side.frames += 1;
                game.build_screen();
                assert_eq!(calls.load(Ordering::Relaxed), 4);
                assert_eq!(game.side.screen_state.0.build.builds, builds);
                width.store(100, Ordering::Relaxed);
                game.side.frames += 1;
                game.build_screen();
                assert_eq!(game.inventory_origin(&batch), (75.0, 37.0));
                assert_eq!(game.side.screen_state.0.build.builds, builds + 1);
                assert_eq!(game.slot_at(&batch, 83.0, 179.0), Some(0));
                assert_eq!(
                    game.screen_view(&batch, Screen::Inventory).rect,
                    [75.0, 37.0, 176.0, 166.0]
                );
            });
        });
    }
#[test]
    #[ignore = "daily"]
    fn room_preserves_vanilla_thresholds_without_listeners_and_on_fit_refusal() {
        use mod_api::{ClientHost, Hooks, ModEntry, ScreenRoomEvent};
        use sunrise_client::hud::HudBatch;
        use sunrise_sim::recipe_book::BookType;
        struct Oversized;
        #[mod_api::__subscriptions]
        impl Hooks for Oversized {
            fn screen_room(&self, _: &dyn ClientHost, event: &mut ScreenRoomEvent<'_>) {
                event.right = event.right.max(1000.0);
            }
        }
        // Both branches use the real game geometry and slots, not room_shift as an oracle.
        for refuses in [false, true] {
            let mods: mod_api::Mods = if refuses {
                vec![ModEntry::new("oversized", Box::new(Oversized))].into()
            } else {
                Vec::<ModEntry>::new().into()
            };
            mod_platform::testing::with_mods(mods, || {
                crate::game::server_side::native_entity_refs::tests::with_client_game(|game| {
                    game.side.screen = Screen::Inventory;
                    game.container = None;
                    game.craft_size = 2;
                    game.side.settings.gui_scale = 1;
                    for (width, closed_x, open_x, book_x) in [
                        (378, 101.0, 101.0, 115.0),
                        (379, 101.0, 178.0, 30.0),
                        (380, 102.0, 179.0, 30.0),
                    ] {
                        for open in [false, true] {
                            game.side.frames += 1;
                            game.level.recipe_book.open[BookType::Crafting.index()] = open;
                            let batch =
                                HudBatch::new([width, 240], game.side.settings.gui_scale_inputs());
                            let layout = game.container_layout(&batch);
                            let x = if open { open_x } else { closed_x };
                            assert_eq!(layout.origin, (x, 37.0));
                            assert_eq!(layout.book, open.then_some([book_x, 37.0, 147.0, 166.0]));
                            assert_eq!(
                                layout.slots.iter().find(|(id, _)| *id == 0).unwrap().1,
                                [x + 7.0, 178.0, 18.0, 18.0]
                            );
                            assert_eq!(
                                game.screen_view(&batch, Screen::Inventory).rect,
                                [x, 37.0, 176.0, 166.0]
                            );
                            assert_eq!(
                                game.slot_at(&batch, x + 8.0, 179.0),
                                if open && width == 378 { None } else { Some(0) }
                            );
                        }
                    }
                });
            });
        }
    }
/// Each `ScreenView::regions` kind where vanilla 26.3 draws it, from its
    /// constants, as the hooks' view of the screen carries it: the effects
    /// beside the inventory, every creative tab, the recipe book's panel and
    /// the tab column it drew, and none for a book drawn over the window. In
    /// its own process: the game it opens publishes process-wide tables.
    #[test]
    #[ignore = "daily"]
    fn each_screen_region_is_where_vanilla_draws_it() {
        use mod_api::{
            ClientHost, Flow, Hooks, ModEntry, ScreenRegion, ScreenRegionKind as Kind,
            ScreenRenderInventoryMobEffectsEvent, ScreenRenderPostEvent, ScreenRenderPreEvent,
        };
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        };

        if sunrise_gameplay::game::native_entity_refs::tests::run_fresh_cases(
            "game::client_side::screen_build::tests::each_screen_region_is_where_vanilla_draws_it",
            "SUNRISE_SCREEN_REGIONS_CHILD",
            &["regions"],
        ) {
            return;
        }

        #[derive(Default)]
        struct Observer {
            canceled: AtomicBool,
            posts: Mutex<Vec<Vec<ScreenRegion>>>,
        }

        struct Observe(Arc<Observer>);
        #[mod_api::__subscriptions]
        impl Hooks for Observe {
            fn screen_render_inventory_mob_effects(
                &self,
                _: &dyn ClientHost,
                event: &mut ScreenRenderInventoryMobEffectsEvent<'_>,
            ) -> Flow {
                event.horizontal_offset += 5;
                Flow::Continue
            }

            fn screen_render_pre(
                &self,
                _: &dyn ClientHost,
                _: &mut ScreenRenderPreEvent<'_>,
            ) -> Flow {
                if self.0.canceled.load(Ordering::Relaxed) {
                    Flow::Cancel
                } else {
                    Flow::Continue
                }
            }

            fn screen_render_post(
                &self,
                _: &dyn ClientHost,
                event: &mut ScreenRenderPostEvent<'_>,
            ) {
                self.0
                    .posts
                    .lock()
                    .unwrap()
                    .push(event.screen.regions.to_vec());
            }
        }

        let observer = Arc::new(Observer::default());
        let mods: mod_api::Mods = vec![ModEntry::new(
            "regions",
            Box::new(Observe(observer.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            crate::game::server_side::native_entity_refs::tests::with_client_game(|game| {
                // The inventory and creative screen belong to a client in a world. An
                // in-memory connection supplies the client registry holder for creative tabs.
                let _server = sunrise_client_gameplay::game::client_side::native_client_session::tests::logged_client(game);
                game.native_lifecycle
                    .begin_open(mod_api::Authority::RemoteClient);
                game.player = Some(
                    sunrise_sim::motion::PlayerMotion::new(
                        game.motion,
                        sunrise_core::geometry::Vec3::ZERO,
                    )
                    .unwrap(),
                );
                game.side.title.0 = None;
                game.container = None;
                game.craft_size = 2;
                game.side.screen = Screen::Inventory;
                game.side.settings.gui_scale = 1;
                game.side.settings.force_unicode = false;
                let crafting = sunrise_sim::recipe_book::BookType::Crafting.index();
                game.level.recipe_book.recipes.clear();
                game.level.recipe_book.open[crafting] = false;

                let render = |game: &mut sunrise_client_gameplay::game::ClientGame<'_>, size| {
                    game.hud_layers(size);
                    let mut posts = observer.posts.lock().unwrap();
                    assert_eq!(posts.len(), 1, "one Render.Post per draw");
                    posts.pop().unwrap()
                };
                let small = [320, 240];
                game.build_screen_at(small);
                assert!(render(game, small).is_empty());

                // EffectsInInventory at 320 x 240: leftPos + imageWidth + 2,
                // shifted five pixels right by the subscriber, one compact row.
                game.potions.effects.list =
                    vec![sunrise_level::effects::EffectInstance::new("speed", 200, 0)];
                game.build_screen_at(small);
                let compact = ScreenRegion::new(Kind::Effects, [255.0, 37.0, 32.0, 32.0]);
                assert_eq!(render(game, small), [compact]);

                // Build twice in the same frame, opening a book whose only tab
                // is Search. At 427 x 240 the book and shifted compact effect
                // fit on opposite sides of the inventory.
                let wide = [854, 480];
                game.side.settings.gui_scale = 2;
                game.build_screen_at(wide);
                game.level.recipe_book.open[crafting] = true;
                game.build_screen_at(wide);
                assert_eq!(
                    render(game, wide),
                    [
                        ScreenRegion::new(Kind::RecipeBook, [54.0, 37.0, 147.0, 166.0]),
                        ScreenRegion::new(Kind::RecipeBookTabs, [22.0, 40.0, 35.0, 27.0]),
                        ScreenRegion::new(Kind::Effects, [385.0, 37.0, 32.0, 32.0]),
                    ]
                );

                observer.canceled.store(true, Ordering::Relaxed);
                assert!(render(game, wide).is_empty());
                assert!(game.screen_regions(Screen::Inventory).is_empty());
                observer.canceled.store(false, Ordering::Relaxed);

                // The narrow book covers the inventory, so its panel is not a side region.
                game.side.settings.gui_scale = 1;
                game.build_screen_at(small);
                assert_eq!(
                    render(game, small),
                    [
                        ScreenRegion::new(Kind::RecipeBookTabs, [54.0, 40.0, 35.0, 27.0]),
                        compact
                    ]
                );

                // Switch directly from drawing the book to drawing Creative.
                // Vanilla's painted 26 x 32 tabs are in two rows. The player
                // can use the operator utilities, including the tab in column five.
                game.potions.effects.list.clear();
                game.set_local_mode(sunrise_command::args::GameMode::Creative);
                game.ui.registries.permission = 2;
                game.side.settings.operator_items_tab = true;
                game.side.screen = Screen::Creative;
                game.refresh_creative_tabs();
                game.build_screen_at(small);
                let mut tabs = render(game, small);
                let by_origin = |a: &ScreenRegion, b: &ScreenRegion| {
                    a.rect[..2].partial_cmp(&b.rect[..2]).unwrap()
                };
                tabs.sort_by(by_origin);
                let mut expected = Vec::new();
                for x in [62.0, 89.0, 116.0, 143.0, 170.0, 204.0, 231.0] {
                    for y in [24.0, 184.0] {
                        expected.push(ScreenRegion::new(Kind::CreativeTab, [x, y, 26.0, 32.0]));
                    }
                }
                assert_eq!(tabs, expected);

                // AnvilScreen does not paint the inventory's EffectsInInventory panel.
                game.set_local_mode(sunrise_command::args::GameMode::Survival);
                game.potions.effects.list =
                    vec![sunrise_level::effects::EffectInstance::new("speed", 200, 0)];
                game.remote_open_station(
                    1,
                    "minecraft:anvil".into(),
                    sunrise_client_gameplay::game::client_side::stations::Kind::Anvil,
                    sunrise_game_data::text::Text::literal("Anvil"),
                    [0; 3],
                );
                game.build_screen_at(small);
                assert!(render(game, small).is_empty());
            });
        });
    }
}
