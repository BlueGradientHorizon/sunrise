use super::*;
use std::sync::{Arc, Mutex};
use winit::dpi::PhysicalPosition;

#[derive(Debug, PartialEq)]
struct Frame {
    pointer: [i32; 2],
    size: [i32; 2],
    origin: [f32; 2],
}

#[derive(Default)]
struct ScreenEvents {
    /// The block and position captured by the drawn screen's factory.
    draw: Vec<(Option<String>, Option<[i32; 3]>)>,
    frames: Vec<Frame>,
    tooltip_at: Vec<[f32; 2]>,
    data: Vec<Vec<i32>>,
    extra: Vec<(i32, Vec<u8>)>,
    cancel_opening: bool,
}
type Seen = Arc<Mutex<ScreenEvents>>;
/// A mod menu type no block opens here.
const PACK: &str = "test:pack";

struct Position(Seen);
impl mod_api::MenuScreenFactory for Position {
    fn create(
        &self,
        _: &dyn mod_api::ClientHost,
        open: mod_api::MenuScreenOpen<'_>,
    ) -> Option<mod_api::MenuScreen> {
        // A block's screen leaves slot 4 out; without a block it shows none.
        self.0
            .lock()
            .unwrap()
            .extra
            .push((open.window, open.extra.to_vec()));
        let slots = match open.block {
            Some(_) => (0..open.backing_slots)
                .map(|i| (i != 4).then(mod_api::ScreenSlot::default))
                .collect(),
            None => Vec::new(),
        };
        let (block, pos) = (open.block.map(str::to_owned), open.pos);
        let draw_seen = self.0.clone();
        let tooltip_seen = self.0.clone();
        Some(
            mod_api::MenuScreen::new(mod_api::MenuScreenLayout {
                width: 176.0,
                height: if block.is_some() { 166.0 } else { 133.0 },
                inventory_x: 8.0,
                inventory_y: 84.0,
                slots,
                own_labels: true,
                own_background: true,
                ..Default::default()
            })
            .with_draw(move |_, frame, _| {
                let mut seen = draw_seen.lock().unwrap();
                seen.draw.push((block.clone(), pos));
                seen.data.push(frame.data.to_vec());
                seen.frames.push(Frame {
                    pointer: frame.pointer,
                    size: frame.gui_size,
                    origin: frame.origin,
                });
            })
            .with_tooltip(move |_, frame, _, _| {
                let mut seen = tooltip_seen.lock().unwrap();
                let draw = seen.frames.last().unwrap();
                assert_eq!(frame.pointer, draw.pointer);
                assert_eq!(frame.gui_size, draw.size);
                assert_eq!(frame.origin, draw.origin);
                seen.tooltip_at.push(frame.at);
            }),
        )
    }
}
struct Listener(Seen);
#[mod_api::__subscriptions]
impl mod_api::Hooks for Listener {
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        r.register_menu_screen(PACK, Arc::new(Position(self.0.clone())))
            .unwrap();
    }

    fn screen_opening(
        &self,
        _: &dyn mod_api::ClientHost,
        _: &mut mod_api::ScreenOpeningEvent<'_>,
    ) -> Option<mod_api::ScreenOpening> {
        self.0
            .lock()
            .unwrap()
            .cancel_opening
            .then_some(mod_api::ScreenOpening::Cancel)
    }
}
fn with_listener(test: impl FnOnce(&mut Game<'_>, &Seen)) {
    let seen = Seen::default();
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "test",
        Box::new(Listener(seen.clone())),
    )]
    .into();
    mod_platform::testing::with_mods(mods.clone(), || {
        mod_platform::__private::part_tests::install_parts_fixture(&mods).unwrap();
        crate::game::native_entity_refs::tests::with_client_game(|game| test(game, &seen));
    });
}
#[test]
#[ignore = "daily"]
fn slotless_screen_draw_sees_full_width_updates_only_for_its_window() {
    use sunrise_protocol::{
        connection::Pipe,
        menu_extra::{DATA_CHANNEL, DataSlot, LIVE_DATA_REVISION},
        packets::{
            configuration::CustomPayload,
            play::{Clientbound as C, ContainerClose, OpenScreen},
        },
    };
    sunrise_client::containers::with_mod_menu_types(
        [(
            PACK.into(),
            "test".into(),
            0,
            2,
            Vec::new(),
            Vec::new(),
            None,
            None,
            false,
        )],
        || {
            with_listener(|game, seen| {
                let (pipe, _server) = Pipe::pair();
                game.connect_local(pipe, "Slotless").unwrap();
                let remote = game.remote.as_mut().unwrap();
                Arc::make_mut(&mut remote.registries).menu_extra_revision = LIVE_DATA_REVISION;
                Arc::make_mut(&mut remote.builtin).registries.insert(
                    "minecraft:menu".into(),
                    sunrise_protocol::registry::Ids::from_names(
                        ["minecraft:anvil", PACK].map(str::to_owned).to_vec(),
                    ),
                );
                let open = |game: &mut Game<'_>, window, menu| {
                    game.handle_packet(C::OpenScreen(OpenScreen {
                        container: window,
                        menu,
                        title: sunrise_core::nbt::Tag::String("Test".into()),
                    }))
                    .unwrap();
                };
                let receive = |game: &mut Game<'_>, window, index, value| {
                    game.handle_packet(C::CustomPayload(CustomPayload {
                        channel: DATA_CHANNEL.into(),
                        data: DataSlot {
                            window,
                            index,
                            value,
                        }
                        .encode(),
                    }))
                    .unwrap();
                };
                let draw = |game: &Game<'_>| {
                    let mut batch = HudBatch::scratch([200, 200]);
                    assert!(game.draw_native_screen(&mut batch, 0.0, 0.0));
                };
                open(game, 1, 0); // Anvil
                assert!(game.station.is_some());
                assert!(!game.slots().is_empty());
                game.cursor = (25.0, 40.0);
                open(game, 2, 1); // No intervening close packet.
                assert_eq!(game.screen, Screen::Inventory);
                assert_eq!(game.cursor, (25.0, 40.0));
                assert!(game.station.is_none());
                assert!(game.slots().is_empty());
                assert_eq!(game.slot_count(), 0);
                assert_eq!(Game::inventory_screen_slot(PACK, 0), None);
                assert_eq!(
                    game.slot_at(&HudBatch::scratch([200, 200]), 25.0, 40.0),
                    None
                );
                seen.lock().unwrap().cancel_opening = true;
                game.set_screen(Screen::Paused);
                assert_eq!(game.screen, Screen::Inventory);
                assert!(
                    game.native_screen_shown(),
                    "an unrelated veto keeps the menu"
                );
                seen.lock().unwrap().cancel_opening = false;
                receive(game, 2, 0, -80_000);
                receive(game, 2, 1, 3);
                draw(game);
                receive(game, 2, 2, 9);
                receive(game, 2, 0, i32::MAX);
                draw(game);
                let first = game.remote.as_ref().unwrap().menu.generation;
                // Even before the old state is dropped, another generation
                // of the same window cannot see it or write to it.
                game.remote
                    .as_mut()
                    .unwrap()
                    .menu
                    .set(Some((2, PACK.into())));
                assert!(game.shown_native_screen().is_none());
                assert!(!game.native_screen_data(2, 0, 8));
                open(game, 2, 1); // Same id, new opening.
                assert_ne!(game.remote.as_ref().unwrap().menu.generation, first);
                receive(game, 3, 0, 8);
                draw(game);
                receive(game, 2, 0, -17);
                receive(game, 2, 1, i32::MIN);
                draw(game);
                game.handle_packet(C::ContainerClose(ContainerClose { container: 2 }))
                    .unwrap();
                receive(game, 2, 1, 18);
                assert!(game.shown_native_screen().is_none());
                seen.lock().unwrap().cancel_opening = true;
                open(game, 2, 1);
                assert_eq!(game.screen, Screen::Playing);
                assert!(game.mod_screen.is_none(), "a veto drops the staged screen");
                assert!(game.remote.as_ref().unwrap().menu.as_ref().is_none());
                assert_eq!(
                    seen.lock().unwrap().data,
                    [
                        vec![-80_000, 3],
                        vec![i32::MAX, 3],
                        vec![],
                        vec![-17, i32::MIN],
                    ]
                );
            })
        },
    );
}

#[test]
#[ignore = "daily"]
fn screen_extra_data_borrows_only_the_open_windows_bytes() {
    use sunrise_protocol::{
        connection::Pipe,
        menu_extra::{self, CHANNEL, LIVE_DATA_REVISION, Message},
        packets::{
            configuration::CustomPayload,
            play::{Clientbound as C, ContainerClose, OpenScreen},
        },
        registry::Ids,
    };

    sunrise_client::containers::with_mod_menu_types(
        [(
            PACK.into(),
            "test".into(),
            0,
            0,
            Vec::new(),
            Vec::new(),
            None,
            None,
            false,
        )],
        || {
            with_listener(|game, seen| {
                let (pipe, _server) = Pipe::pair();
                game.connect_local(pipe, "Extra bytes").unwrap();
                let remote = game.remote.as_mut().unwrap();
                Arc::make_mut(&mut remote.registries).menu_extra_revision = LIVE_DATA_REVISION;
                Arc::make_mut(&mut remote.builtin)
                    .registries
                    .insert("minecraft:menu".into(), Ids::from_names(vec![PACK.into()]));

                let receive = |game: &mut Game<'_>, window, bytes: &[u8]| {
                    game.handle_packet(C::CustomPayload(CustomPayload {
                        channel: CHANNEL.into(),
                        data: menu_extra::encode(&Message {
                            window,
                            pos: None,
                            data: bytes.to_vec(),
                        })
                        .unwrap(),
                    }))
                    .unwrap();
                };
                let open = |game: &mut Game<'_>, window| {
                    game.handle_packet(C::OpenScreen(OpenScreen {
                        container: window,
                        menu: 0,
                        title: sunrise_core::nbt::Tag::String("Test".into()),
                    }))
                    .unwrap();
                };
                let expected = [0x12, 0x34, 0x56];
                receive(game, 1, &expected);
                assert!(seen.lock().unwrap().extra.is_empty());
                open(game, 1);
                assert_eq!(seen.lock().unwrap().extra, [(1, expected.to_vec())]);

                open(game, 2); // No payload at all.
                receive(game, 4, &[0x78]);
                assert_eq!(seen.lock().unwrap().extra.len(), 2);
                open(game, 3); // Discards the payload for window 4.
                open(game, 4); // The mismatched payload was not saved for later.
                let calls = [
                    (1, expected.to_vec()),
                    (2, vec![]),
                    (3, vec![]),
                    (4, vec![]),
                ];
                assert_eq!(seen.lock().unwrap().extra, calls);
                game.handle_packet(C::ContainerClose(ContainerClose { container: 4 }))
                    .unwrap();
                assert!(game.shown_native_screen().is_none());
                assert_eq!(seen.lock().unwrap().extra, calls);
            })
        },
    );
}

const CRATE_MENU: &str = "test:crate_menu";

/// A mod that adds `test:crate`, a block whose container opens `test:crate_menu`,
/// with the `Position` screen registered for that menu type.
struct CrateMod(Seen);
#[mod_api::__subscriptions]
impl mod_api::Hooks for CrateMod {
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        let block = r.block_key("test:crate");
        r.block_container(
            &block,
            mod_api::ModContainer {
                slots: 27,
                menu: Some(CRATE_MENU.into()),
                ..Default::default()
            },
        );
        r.register_menu_screen(CRATE_MENU, Arc::new(Position(self.0.clone())))
            .unwrap();
    }
    fn blocks(&self, _: &mod_api::RegistrationContext, event: &mut mod_api::BlocksEvent<'_>) {
        event
            .register(mod_api::Block::new("test:crate", "minecraft:barrel").with_item(false))
            .unwrap();
    }
}
/// A mod block whose container opens a custom menu type: the client lays it
/// out for that opener, leaving slot 4 initially inactive (`None`).
#[test]
#[ignore = "daily"]
fn a_mod_blocks_custom_menu_gets_its_registered_screen() {
    // Content registries are process-global.
    if crate::game::native_entity_refs::tests::run_fresh_cases(
        "game::native_hooks::screen_position_tests::a_mod_blocks_custom_menu_gets_its_registered_screen",
        "SUNRISE_SCREEN_POSITION_CRATE_CHILD",
        &["crate"],
    ) {
        return;
    }
    let seen = Seen::default();
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "test",
        Box::new(CrateMod(seen.clone())),
    )]
    .into();
    mod_platform::testing::with_mods(mods.clone(), || {
        crate::game::api_values::install_parts(&mods).unwrap();
        mod_api::__private::mark_game_thread();
        let base = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let root = crate::game::mods::content_root(&base, &Default::default()).unwrap();
        sunrise_client::containers::with_mod_menu_types(
            [(
                CRATE_MENU.into(),
                "test".into(),
                27,
                0,
                Vec::new(),
                Vec::new(),
                None,
                None,
                true,
            )],
            || {
                crate::game::native_entity_refs::tests::with_game_role(
                    root.prepared().to_owned(),
                    crate::game::execution_role::ExecutionRole::Client,
                    |game| {
                        seed_native_menu(game, 1, CRATE_MENU);
                        game.player = Some(
                            sunrise_client::motion::PlayerMotion::new(
                                game.motion,
                                sunrise_core::geometry::Vec3::new(17.5, 64.0, -8.5),
                            )
                            .unwrap(),
                        );
                        let pos = (17, 63, -9);
                        game.world.chunks.insert(
                            (1, -1),
                            Arc::new(sunrise_client::chunk::Chunk::empty(1, -1)),
                        );
                        let state = game
                            .pack
                            .assets
                            .registry
                            .default_state("test:crate")
                            .unwrap();
                        game.world.set(pos.0, pos.1, pos.2, state).unwrap();
                        assert!(game.open_native_screen(pos, Some(pos), 27, None));
                        let open = game.mod_screen.as_ref().expect("the menu type's screen");
                        assert_eq!(open.menu.block.as_deref(), Some("test:crate"));
                        assert_eq!(open.layout().height, 166.0);
                        assert!(open.layout().slots[4].is_none());
                        game.container = Some(pos);
                        game.title = None;
                        game.ui.hide_gui = true;
                        game.settings.gui_scale = 2;
                        game.level.recipe_book.open = [false; 4];
                        game.world
                            .block_entities
                            .insert(pos, BlockEntity::Chest(vec![None; 27]));
                        game.set_screen(Screen::Inventory);
                        // No new motion: the odd-sized resize allows GUI scale 2.
                        let small = [638, 478];
                        let projection = game.pointer_projection(small);
                        let sample = projection
                            .window
                            .physical_sample(PhysicalPosition::new(321.75, 231.75));
                        game.set_window_cursor(sample, projection);
                        game.hud(small);
                        game.hud([641, 481]);

                        // A cursor supplied in GUI pixels directly still truncates toward zero.
                        game.window_cursor = None;
                        game.cursor = (-3.75, 129.75);
                        game.hud([641, 481]);
                        {
                            let seen = seen.lock().unwrap();
                            let block = Some("test:crate".to_owned());
                            assert_eq!(seen.draw, vec![(block, Some([17, 63, -9])); 3]);
                            assert_eq!(
                                seen.frames,
                                [
                                    Frame {
                                        pointer: [321, 231],
                                        size: [638, 478],
                                        origin: [231.0, 156.0],
                                    },
                                    Frame {
                                        pointer: [161, 116],
                                        size: [321, 241],
                                        origin: [72.0, 37.0],
                                    },
                                    Frame {
                                        pointer: [-3, 129],
                                        size: [321, 241],
                                        origin: [72.0, 37.0],
                                    },
                                ]
                            );
                            assert_eq!(
                                seen.tooltip_at,
                                [[90.0, 75.0], [89.0, 79.0], [-75.75, 92.75]]
                            );
                        }
                        assert_eq!(game.menu_opener_block(pos, CRATE_MENU), Some("test:crate"));
                    },
                );
            },
        );
    });
}
/// #1172: a mod menu type opened without a block (an item's use, an entity, a command) gets
/// its type's screen with omitted trailing slots starting inactive (`None`), while a vanilla
/// menu keeps its built-in screen.
#[test]
#[ignore = "daily"]
fn a_mod_menu_opened_without_a_block_gets_its_types_screen() {
    sunrise_client::containers::with_mod_menu_types(
        [(
            PACK.into(),
            "test".into(),
            9,
            0,
            Vec::new(),
            Vec::new(),
            None,
            None,
            true,
        )],
        || {
            with_listener(|game, seen| {
                let key = (i32::MIN, 0, 0);
                seed_native_menu(game, 1, PACK);
                assert!(game.open_native_screen(key, None, 9, None));
                let open = game.mod_screen.as_ref().expect("the menu type's screen");
                assert_eq!((open.layout().height, &open.menu.block), (133.0, &None));
                assert_eq!(open.layout().slots, vec![None; 9]);
                game.container = Some(key);
                game.set_screen(Screen::Inventory);
                let mut batch = HudBatch::scratch([128, 128]);
                assert!(game.draw_native_screen(&mut batch, 0.0, 0.0));
                assert_eq!(seen.lock().unwrap().draw, [(None, None)]);
                seed_native_menu(game, 2, "minecraft:generic_9x3");
                assert!(game.open_native_screen(key, None, 27, None));
                assert!(
                    game.mod_screen.is_none(),
                    "a vanilla chest keeps its screen"
                );
            });
        },
    );
}

/// What the hopper screens heard, as (the screen's mod, the callback).
type Heard = Arc<Mutex<Vec<(&'static str, &'static str)>>>;
/// Mod `owner`'s screen for `menu`: it leaves slot `HIDDEN` inactive at open,
/// its button is `HIDDEN` and its tooltip names the owner.
#[derive(Clone)]
struct HopperScreen<const HIDDEN: usize> {
    owner: &'static str,
    menu: &'static str,
    heard: Heard,
}
impl<const HIDDEN: usize> mod_api::MenuScreenFactory for HopperScreen<HIDDEN> {
    fn create(
        &self,
        _: &dyn mod_api::ClientHost,
        open: mod_api::MenuScreenOpen<'_>,
    ) -> Option<mod_api::MenuScreen> {
        assert_eq!(open.backing_slots, 5);
        assert_eq!(open.player_slots, 36);
        assert_eq!(open.menu, self.menu);
        let owner = self.owner;
        let menu = self.menu;
        self.heard.lock().unwrap().push((owner, "create"));
        let draw = self.heard.clone();
        let click = self.heard.clone();
        let tooltip = self.heard.clone();
        Some(
            mod_api::MenuScreen::new(mod_api::MenuScreenLayout {
                width: 176.0,
                height: 133.0,
                slots: (0..5)
                    .map(|i| (i != HIDDEN).then(mod_api::ScreenSlot::default))
                    .collect(),
                own_labels: true,
                own_background: true,
                ..Default::default()
            })
            .with_draw(move |host, _, _| {
                assert_eq!(mod_api::content::crash::current_native_mod(), Some(owner));
                let slot = usize::from(HIDDEN == 0);
                assert_eq!(host.screen_slot(menu, slot), Some([0.0, 0.0]));
                draw.lock().unwrap().push((owner, "draw"));
            })
            .with_click(move |_, _, _| {
                click.lock().unwrap().push((owner, "click"));
                Some(HIDDEN as i32)
            })
            .with_tooltip(move |_, _, _, lines| {
                tooltip.lock().unwrap().push((owner, "tooltip"));
                lines.push((mod_api::Text::literal(owner), 0));
            }),
        )
    }
}
/// Mod `owner`: its block `<owner>:machine` opens `<owner>:hopper`, while it registers
/// its screen factory for `screen.menu` (testing cross-mod registration).
struct HopperMod<const HIDDEN: usize> {
    block_menu: &'static str,
    screen: HopperScreen<HIDDEN>,
}
#[mod_api::__subscriptions]
impl<const HIDDEN: usize> mod_api::Hooks for HopperMod<HIDDEN> {
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        let id = format!("{}:machine", self.screen.owner);
        let block = r.block_key(&id);
        r.block_container(
            &block,
            mod_api::ModContainer {
                slots: 5,
                menu: Some(self.block_menu.into()),
                ..Default::default()
            },
        );
        r.register_menu_screen(self.screen.menu, Arc::new(self.screen.clone()))
            .unwrap();
    }
    fn blocks(&self, _: &mod_api::RegistrationContext, event: &mut mod_api::BlocksEvent<'_>) {
        let id = format!("{}:machine", self.screen.owner);
        event
            .register(mod_api::Block::new(&id, "minecraft:barrel").with_item(false))
            .unwrap();
    }
}
/// Screen factories route directly by menu type (including cross-mod registration),
/// any mod may read/move/activate slots of the open screen, and a vanilla hopper keeps
/// vanilla's built-in screen.
#[test]
#[ignore = "daily"]
fn custom_menu_screens_route_by_menu_type_and_allow_cross_mod_slot_controls() {
    // Content registries are process-global.
    if crate::game::native_entity_refs::tests::run_fresh_cases(
        "game::native_hooks::screen_position_tests::custom_menu_screens_route_by_menu_type_and_allow_cross_mod_slot_controls",
        "SUNRISE_SCREEN_POSITION_HOPPER_CHILD",
        &["hoppers"],
    ) {
        return;
    }
    let heard = Heard::default();
    let alpha_screen = HopperScreen::<0> {
        owner: "alpha",
        menu: "beta:hopper",
        heard: heard.clone(),
    };
    let beta_screen = HopperScreen::<4> {
        owner: "beta",
        menu: "alpha:hopper",
        heard: heard.clone(),
    };
    let mods: mod_api::Mods = vec![
        mod_api::ModEntry::new(
            "alpha",
            Box::new(HopperMod {
                block_menu: "alpha:hopper",
                screen: alpha_screen,
            }),
        ),
        mod_api::ModEntry::new(
            "beta",
            Box::new(HopperMod {
                block_menu: "beta:hopper",
                screen: beta_screen,
            }),
        ),
    ]
    .into();
    mod_platform::testing::with_mods(mods.clone(), || {
        crate::game::api_values::install_parts(&mods).unwrap();
        mod_api::__private::mark_game_thread();
        let base = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let root = crate::game::mods::content_root(&base, &Default::default()).unwrap();
        sunrise_client::containers::with_mod_menu_types(
            [
                (
                    "alpha:hopper".into(),
                    "alpha".into(),
                    5,
                    0,
                    Vec::new(),
                    Vec::new(),
                    None,
                    None,
                    true,
                ),
                (
                    "beta:hopper".into(),
                    "beta".into(),
                    5,
                    0,
                    Vec::new(),
                    Vec::new(),
                    None,
                    None,
                    true,
                ),
            ],
            || {
                crate::game::native_entity_refs::tests::with_game_role(
                    root.prepared().to_owned(),
                    crate::game::execution_role::ExecutionRole::Client,
                    |game| {
                        use sunrise_protocol::packets::play::Serverbound;
                        let (mut peer, _writer) =
                            crate::game::remote::tests::connect_menu_test(game);
                        game.world
                            .chunks
                            .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
                        for (x, block, menu_name, hidden, screen_owner) in [
                            (1, "alpha:machine", "alpha:hopper", 4, "beta"),
                            (2, "beta:machine", "beta:hopper", 0, "alpha"),
                        ] {
                            let pos = (x, 64, 0);
                            let state = game.pack.assets.registry.default_state(block).unwrap();
                            game.world.set(pos.0, pos.1, pos.2, state).unwrap();
                            heard.lock().unwrap().clear();
                            seed_native_menu(game, x, menu_name);
                            assert!(game.open_native_screen(pos, Some(pos), 5, None));
                            game.container = Some(pos);
                            game.set_screen(Screen::Inventory);
                            let open = game.mod_screen.as_ref().expect("the machine's screen");
                            let (menu, open_block, base) = (
                                open.menu.name.clone(),
                                open.menu.block.clone().unwrap(),
                                open.container_slot_base(),
                            );
                            assert_eq!(menu, menu_name);
                            assert_eq!(open_block, block);
                            let shown: Vec<_> = (0..5).filter(|&i| i != hidden).collect();
                            let ids: Vec<_> = shown.iter().map(|i| base + i).collect();
                            assert_eq!(game.native_screen_slots(), Some(ids));
                            let mut batch = HudBatch::scratch([512, 512]);
                            let (ox, oy) = game.inventory_origin(&batch);
                            game.cursor = (ox + 90.0, oy + 105.0);
                            assert!(game.draw_native_screen(&mut batch, ox, oy));
                            assert!(game.native_screen_press_at(&batch, false));
                            let click = (0..32).find_map(|_| {
                                let (id, body) = peer.recv().ok()?;
                                match Serverbound::decode(id, &body).unwrap() {
                                    Serverbound::ContainerButtonClick(click) => Some(click),
                                    _ => None,
                                }
                            });
                            let click = click.expect("the active hopper's button");
                            assert_eq!((click.container, click.button), (x, hidden as i32));
                            let mut lines = Vec::new();
                            game.native_screen_tooltip_lines(&batch, None, &mut lines);
                            assert_eq!(
                                lines,
                                [(sunrise_client::text::Text::literal(screen_owner), 0)]
                            );
                            let hooks = ["create", "draw", "click", "tooltip"];
                            let only_owner: Vec<_> =
                                hooks.iter().map(|&hook| (screen_owner, hook)).collect();
                            assert_eq!(*heard.lock().unwrap(), only_owner);
                            // Any mod may read, move, activate, and hide slots on the open screen.
                            for calling in ["alpha", "beta"] {
                                let _call = mod_api::content::crash::enter_native_mod(
                                    mod_api::content::crash::ModCall {
                                        id: calling,
                                        hook: "menu_screen_draw",
                                        part: None,
                                    },
                                );
                                assert!(game.screen_slot(&menu, shown[0]).is_some());
                                assert_eq!(
                                    game.set_screen_slot(&menu, shown[0], Some([1.0, 1.0])),
                                    Ok(())
                                );
                                assert_eq!(
                                    game.set_screen_slot(&menu, hidden, Some([20.0, 20.0])),
                                    Ok(())
                                );
                                assert_eq!(game.screen_slot(&menu, hidden), Some([20.0, 20.0]));
                                assert_eq!(game.set_screen_slot(&menu, hidden, None), Ok(()));
                                assert_eq!(game.screen_slot(&menu, hidden), None);
                            }
                        }
                        // A vanilla hopper asks neither mod.
                        let pos = (3, 64, 0);
                        let state = game
                            .pack
                            .assets
                            .registry
                            .default_state("minecraft:hopper")
                            .unwrap();
                        game.world.set(pos.0, pos.1, pos.2, state).unwrap();
                        heard.lock().unwrap().clear();
                        seed_native_menu(game, 3, "minecraft:hopper");
                        assert!(game.open_native_screen(pos, Some(pos), 5, None));
                        assert!(
                            game.mod_screen.is_none(),
                            "a vanilla hopper keeps its screen"
                        );
                        assert_eq!(game.menu_opener_block(pos, "minecraft:hopper"), None);
                        assert!(heard.lock().unwrap().is_empty());
                    },
                );
            },
        );
    });
}
