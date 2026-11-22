use super::*;
use mod_api::__private::game::Fields;
use mod_api::{Hand, HostTransaction, ItemPlace, ItemResource, MenuSlotPlace, ServerHost};
use sunrise_client::{
    containers::{BlockEntity, ModBlockEntity, ModMenuType},
    crafting::{Ingredient, Placement},
    lan::menu::{Menu as Open, Slots},
    recipe_book::{
        settings_connection::Connection, settings_sync::Authority, slot_layout::CheckedSlotLayout,
    },
};
/// A guest with a three-slot mod menu open on a block, and the catalogue & display its client would send.
fn open_mod_menu(
    game: &mut Game<'_>,
) -> (
    (i32, i32, i32),
    MenuSnapshot,
    sync::CatalogIdentity,
    sync::DisplayId,
) {
    open_mod_menu_named(game, "review:menu")
}

fn open_mod_menu_named(
    game: &mut Game<'_>,
    menu_id: &'static str,
) -> (
    (i32, i32, i32),
    MenuSnapshot,
    sync::CatalogIdentity,
    sync::DisplayId,
) {
    let pos = (3, 64, 5);
    let kind = Box::leak(Box::new(ModMenuType {
        id: menu_id,
        owner: "review",
        slots: 3,
        player_inventory: true,
        data_slots: 0,
        owned_slots: None,
        results: vec![2],
        inputs: Vec::new(),
        crafting: None,
        links: None,
    }));
    let mut block = ModBlockEntity::new("review:block".into(), Default::default());
    block.items = vec![
        Some(sunrise_client::items::ItemStack::new("minecraft:dirt", 1)),
        None,
        None,
    ];
    game.world
        .block_entities
        .insert(pos, BlockEntity::Mod(Box::new(block)));
    game.world.block_entities.set_mod_containers(
        [(
            game.world.block(pos.0, pos.1, pos.2).0,
            std::sync::Arc::new(sunrise_client::containers::ModContainer {
                slots: 3,
                ..Default::default()
            }),
        )]
        .into_iter()
        .collect(),
    );
    game.world.modified.clear();
    game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
        g.menu_generation = 1;
        g.menu = Some(Open {
            window: 1,
            kind: 0,
            registry_id: menu_id.into(),
            pos: Some(pos),
            slots: Slots::Block(pos),
            size: 3,
            state: 0,
            sent: vec![],
            drag: None,
            sent_data: vec![],
            reported_unsent_data_len: None,
            blockless_data: Vec::new(),
            hidden: vec![],
            mod_type: Some(kind),
            craft: None,
            links: vec![],
            asks_still_valid: false,
            opening_item: None,
        });
    });
    let before = game.mod_book_menu_snapshot(12, 1, menu_id).unwrap();
    let catalog = sync::CatalogIdentity {
        epoch: 1,
        number: 1,
        window: 1,
        nonce: 1,
        instance: 1,
        generation: 1,
        menu: menu_id.to_owned().try_into().unwrap(),
        book: "review:book".to_owned().try_into().unwrap(),
    };
    let display = sync::DisplayId {
        recipe: "review:recipe".to_owned().try_into().unwrap(),
        source: 0,
    };
    let mut server = super::super::native_book_placement::Server::new(1);
    server.peer.hello(1, sync::schema()).unwrap();
    server.peer.publish(catalog.clone()).unwrap();
    game.native_net.placement.servers.insert(12, server);
    let mut connection = Connection::default();
    let book = catalog.book.clone();
    connection.authority = Some(Authority::negotiate(1, vec![book.clone()], &[book].into()));
    game.native_net.book_connections.insert(12, connection);
    (pos, before, catalog, display)
}

#[test]
#[ignore = "daily"]
fn actual_game_stage_refuses_nested_frame_and_mutated_actor() {
    super::super::native_guest_turn_tests::with_guests(|game| {
        let (pos, before, catalog, display) = open_mod_menu(game);
        let layout = CheckedSlotLayout::new(
            mod_api::RecipeBookLayout::Furnace {
                input: 0,
                fuel: 1,
                result: 2,
            },
            3,
        )
        .unwrap();
        let plan = MenuPlacement::new(
            "review:menu".into(),
            Placement::of(
                vec![Some(Ingredient::of(vec!["minecraft:iron_ore".into()]))],
                None,
            ),
            &layout,
        )
        .unwrap();
        let root = game.transaction_open();
        assert!(game.mod_book_placement_context(&before, &catalog));
        let child = game.transaction_open();
        assert_eq!(
            game.stage_mod_book_place(&before, &catalog, &display, &plan, false),
            Err(sync::Refusal::Changed)
        );
        game.transaction_close(&child, false);
        assert!(game.mod_book_placement_context(&before, &catalog));
        game.lan
            .as_mut()
            .unwrap()
            .hub
            .guests
            .update(0, |g| g.health = 1.0);
        assert_eq!(
            game.stage_mod_book_place(&before, &catalog, &display, &plan, false),
            Err(sync::Refusal::Changed)
        );
        assert_eq!(
            game.world.block_entities.get(&pos).unwrap().slots()[0]
                .as_ref()
                .unwrap()
                .count,
            1
        );
        assert!(game.world.modified.is_empty());
        game.transaction_close(&root, false);
    });
}
#[test]
#[ignore = "daily"]
fn a_mod_layout_without_a_listening_part_refuses_and_changes_nothing() {
    super::super::native_guest_turn_tests::with_guests(|game| {
        let (pos, before, catalog, display) = open_mod_menu(game);
        let plan = MenuPlacement::for_mod(
            "review:menu".into(),
            Placement::of(
                vec![Some(Ingredient::of(vec!["minecraft:iron_ore".into()]))],
                None,
            ),
        )
        .unwrap();
        let root = game.transaction_open();
        assert_eq!(
            game.stage_mod_book_place(&before, &catalog, &display, &plan, false),
            Err(sync::Refusal::Unsupported)
        );
        assert_eq!(
            game.world.block_entities.get(&pos).unwrap().slots()[0]
                .as_ref()
                .unwrap()
                .count,
            1
        );
        assert!(game.world.modified.is_empty());
        game.transaction_close(&root, false);
    });
}
struct CountingPlacer(std::sync::Arc<std::sync::atomic::AtomicUsize>);
#[mod_api::sunrise_part]
impl mod_api::MenuPart for CountingPlacer {
    fn recipe_book_place(
        &self,
        _: &mut dyn mod_api::ServerHost,
        event: &mut mod_api::RecipeBookPlaceEvent<'_>,
    ) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        *event.placement = mod_api::RecipeBookPlacement::Grid {
            width: 1,
            height: 1,
            slots: vec![0],
            clear: vec![0],
        };
    }
}

struct PanickingPlacer(std::sync::Arc<std::sync::atomic::AtomicUsize>);
#[mod_api::sunrise_part]
impl mod_api::MenuPart for PanickingPlacer {
    fn recipe_book_place(
        &self,
        _: &mut dyn mod_api::ServerHost,
        event: &mut mod_api::RecipeBookPlaceEvent<'_>,
    ) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        *event.placement = mod_api::RecipeBookPlacement::Grid {
            width: 1,
            height: 1,
            slots: vec![0],
            clear: vec![0],
        };
        panic!("menu placer exploded");
    }
}

#[test]
#[ignore = "daily"]
fn observed_mod_click_supplies_grid_once_and_second_click_acts_on_changed_grid() {
    struct CountingMod(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for CountingMod {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            r.menu_part("review:menu", CountingPlacer(self.0.clone()));
        }
    }
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "review",
        Box::new(CountingMod(calls.clone())),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        super::super::native_guest_turn_tests::with_guests(|game| {
            let (pos, _, catalog, display) = open_mod_menu(game);
            game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                g.inventory[9] = Some(sunrise_client::items::ItemStack::new(
                    "minecraft:iron_ore",
                    3,
                ));
            });
            let before = game.mod_book_menu_snapshot(12, 1, "review:menu").unwrap();
            let plan = MenuPlacement::for_mod(
                "review:menu".into(),
                Placement::of(
                    vec![Some(Ingredient::of(vec!["minecraft:iron_ore".into()]))],
                    None,
                ),
            )
            .unwrap();
            let root = game.transaction_open();
            assert_eq!(
                game.stage_mod_book_place(&before, &catalog, &display, &plan, false),
                Ok(PostPlace::Nothing)
            );
            game.transaction_close(&root, true);
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
            let first = game.world.block_entities.get(&pos).unwrap().slots()[0]
                .as_ref()
                .unwrap();
            assert_eq!(first.item.as_ref(), "minecraft:iron_ore");
            assert_eq!(first.count, 1);

            let after_first = game.mod_book_menu_snapshot(12, 1, "review:menu").unwrap();
            let root = game.transaction_open();
            assert_eq!(
                game.stage_mod_book_place(&after_first, &catalog, &display, &plan, false),
                Ok(PostPlace::Nothing)
            );
            game.transaction_close(&root, true);
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
            let second = game.world.block_entities.get(&pos).unwrap().slots()[0]
                .as_ref()
                .unwrap();
            assert_eq!(second.item.as_ref(), "minecraft:iron_ore");
            assert_eq!(second.count, 2);
            let left = game.lan.as_ref().unwrap().hub.guest(12).unwrap().inventory[9]
                .as_ref()
                .unwrap()
                .count;
            assert_eq!(left, 1);
        });
    });
}

#[test]
#[ignore = "daily"]
fn panicking_recipe_book_place_discards_answer_disables_only_that_callback_and_keeps_other_menus_running()
 {
    struct TwoMenuMod {
        panic_calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        other_calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for TwoMenuMod {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            r.menu_part("review:menu", PanickingPlacer(self.panic_calls.clone()));
            r.menu_part("review:other", CountingPlacer(self.other_calls.clone()));
        }
    }
    let panic_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let other_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "review",
        Box::new(TwoMenuMod {
            panic_calls: panic_calls.clone(),
            other_calls: other_calls.clone(),
        }),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        super::super::native_guest_turn_tests::with_guests(|game| {
            let _ = mod_api::__private::game::take_callback_panic_notices();
            let (pos, _, catalog, display) = open_mod_menu_named(game, "review:menu");
            game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                g.inventory[9] = Some(sunrise_client::items::ItemStack::new(
                    "minecraft:iron_ore",
                    3,
                ));
            });
            let before = game.mod_book_menu_snapshot(12, 1, "review:menu").unwrap();
            let plan = MenuPlacement::for_mod(
                "review:menu".into(),
                Placement::of(
                    vec![Some(Ingredient::of(vec!["minecraft:iron_ore".into()]))],
                    None,
                ),
            )
            .unwrap();

            let root = game.transaction_open();
            assert_eq!(
                game.stage_mod_book_place(&before, &catalog, &display, &plan, false),
                Ok(PostPlace::Nothing)
            );
            game.transaction_close(&root, true);
            assert_eq!(panic_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
            let untouched = game.world.block_entities.get(&pos).unwrap().slots()[0]
                .as_ref()
                .unwrap();
            assert_eq!(untouched.item.as_ref(), "minecraft:dirt");
            assert_eq!(untouched.count, 1);
            let notices = mod_api::__private::game::take_callback_panic_notices();
            assert!(
                notices
                    .iter()
                    .any(|n| n.mod_id == "review" && n.operation == "recipe_book_place"),
                "expected panic notice naming mod review and recipe_book_place, got {notices:?}"
            );

            let before2 = game.mod_book_menu_snapshot(12, 1, "review:menu").unwrap();
            let root = game.transaction_open();
            assert_eq!(
                game.stage_mod_book_place(&before2, &catalog, &display, &plan, false),
                Ok(PostPlace::Nothing)
            );
            game.transaction_close(&root, true);
            assert_eq!(
                panic_calls.load(std::sync::atomic::Ordering::SeqCst),
                1,
                "disabled callback must not run again in the session"
            );
            assert!(
                !mod_api::__private::game::take_callback_panic_notices()
                    .iter()
                    .any(|n| n.mod_id == "review" && n.operation == "recipe_book_place")
            );

            let (other_pos, _, other_catalog, other_display) =
                open_mod_menu_named(game, "review:other");
            let other_before = game.mod_book_menu_snapshot(12, 1, "review:other").unwrap();
            let other_plan = MenuPlacement::for_mod(
                "review:other".into(),
                Placement::of(
                    vec![Some(Ingredient::of(vec!["minecraft:iron_ore".into()]))],
                    None,
                ),
            )
            .unwrap();
            let root = game.transaction_open();
            assert_eq!(
                game.stage_mod_book_place(
                    &other_before,
                    &other_catalog,
                    &other_display,
                    &other_plan,
                    false
                ),
                Ok(PostPlace::Nothing)
            );
            game.transaction_close(&root, true);
            assert_eq!(other_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
            let placed = game.world.block_entities.get(&other_pos).unwrap().slots()[0]
                .as_ref()
                .unwrap();
            assert_eq!(placed.item.as_ref(), "minecraft:iron_ore");
            assert_eq!(placed.count, 1);
        });
    });
}

struct BackedPack;
#[mod_api::__subscriptions]
impl mod_api::Hooks for BackedPack {
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        let item = r.item_key("minecraft:shulker_box");
        r.item_container(&item, |_: &dyn mod_api::Host| Some(27));
    }
}

/// The menu's part: it places the clicked recipe in a one-slot grid, as a mod would.
struct Placer;
#[mod_api::sunrise_part]
impl mod_api::MenuPart for Placer {
    fn recipe_book_place(
        &self,
        _: &mut dyn mod_api::ServerHost,
        event: &mut mod_api::RecipeBookPlaceEvent<'_>,
    ) {
        *event.placement = mod_api::RecipeBookPlacement::Grid {
            width: 1,
            height: 1,
            slots: vec![0],
            clear: vec![0],
        };
    }
}
struct PlacerMod;
#[mod_api::__subscriptions]
impl mod_api::Hooks for PlacerMod {
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        r.menu_part("review:menu", Placer);
    }
}
#[test]
#[ignore = "daily"]
fn a_listening_part_places_the_recipe_through_the_real_stage() {
    let mods: mod_api::Mods = vec![
        mod_api::ModEntry::new("backed_pack_fixture", Box::new(BackedPack)),
        mod_api::ModEntry::new("review", Box::new(PlacerMod)),
    ]
    .into();
    mod_platform::testing::with_mods(mods, || {
        super::super::native_guest_turn_tests::with_guests(|game| {
            let (_, opened, catalog, display) = open_mod_menu(game);
            game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                let open = g.menu.as_mut().unwrap();
                open.slots = Slots::Native;
                open.pos = None;
                g.native_menu = vec![None; open.size];
                g.inventory[0] = Some(sunrise_client::items::ItemStack::new(
                    "minecraft:shulker_box",
                    1,
                ));
                g.inventory[9] = Some(sunrise_client::items::ItemStack::new(
                    "minecraft:iron_ore",
                    3,
                ));
            });
            game.player_menu_opening(opened.conn);
            let menu = game.menu(opened.uuid, opened.window).unwrap();
            let places = (0..3)
                .map(|slot| MenuSlotPlace::HeldItem {
                    hand: Hand::Main,
                    slot,
                })
                .collect();
            assert_eq!(
                game.player_back_mod_menu_slots(menu.fields().player, 1, places),
                Ok(())
            );
            let before = game.mod_book_menu_snapshot(12, 1, "review:menu").unwrap();
            let plan = MenuPlacement::for_mod(
                "review:menu".into(),
                Placement::of(
                    vec![Some(Ingredient::of(vec!["minecraft:iron_ore".into()]))],
                    None,
                ),
            )
            .unwrap();
            let root = game.transaction_open();
            assert_eq!(
                game.stage_mod_book_place(&before, &catalog, &display, &plan, false),
                Ok(PostPlace::Nothing)
            );
            game.transaction_close(&root, true);
            let placed = {
                let host: &mut dyn ServerHost = game;
                ItemPlace::MenuSlot { menu, slot: 0 }
                    .with_stack(host, |stack| stack.project())
                    .flatten()
                    .unwrap()
            };
            assert_eq!(
                placed.item(),
                mod_api::__private::item_key("minecraft:iron_ore").unwrap()
            );
            assert_eq!(placed.count(), 1);
            let left = game.lan.as_ref().unwrap().hub.guest(12).unwrap().inventory[9]
                .as_ref()
                .unwrap()
                .count;
            assert_eq!(left, 2);

            let mut stale_contents = game.mod_book_menu_snapshot(12, 1, "review:menu").unwrap();
            let nested = ItemPlace::Nested {
                parent: Box::new(ItemPlace::PlayerHand {
                    player: stale_contents.uuid,
                    hand: Hand::Main,
                }),
                slot: 0,
            };
            let dirt =
                mod_api::ItemStack::new(mod_api::__private::item_key("minecraft:dirt").unwrap(), 1)
                    .unwrap();
            let dirt = ItemResource::of(&dirt);
            let host: &mut dyn ServerHost = game;
            assert_eq!(
                host.transaction(|host, tx| {
                    let old = nested.resource(host).ok_or(())?;
                    let count = nested.amount(host);
                    if nested.extract(host, tx, &old, count) != count
                        || nested.insert(host, tx, &dirt, 1) != 1
                    {
                        return Err(());
                    }
                    Ok(())
                }),
                Ok(())
            );
            stale_contents.inventory = game
                .lan
                .as_ref()
                .unwrap()
                .hub
                .guest(12)
                .unwrap()
                .inventory
                .clone();
            let root = game.transaction_open();
            assert_eq!(
                game.stage_mod_book_place(&stale_contents, &catalog, &display, &plan, false),
                Err(sync::Refusal::Changed)
            );
            game.transaction_close(&root, false);

            let stale_identity = game.mod_book_menu_snapshot(12, 1, "review:menu").unwrap();
            let places = [4, 1, 2]
                .into_iter()
                .map(|slot| MenuSlotPlace::HeldItem {
                    hand: Hand::Main,
                    slot,
                })
                .collect();
            assert_eq!(
                game.player_back_mod_menu_slots(menu.fields().player, 1, places),
                Ok(())
            );
            let root = game.transaction_open();
            assert_eq!(
                game.stage_mod_book_place(&stale_identity, &catalog, &display, &plan, false),
                Err(sync::Refusal::Changed)
            );
            game.transaction_close(&root, false);
        });
    });
}

static ROUTER_BOOK_KEY: std::sync::OnceLock<mod_api::RecipeBookTypeKey> =
    std::sync::OnceLock::new();

fn router_display(ingredient: &str, result: &str, experience: f32) -> mod_api::RecipeBookDisplay {
    mod_api::RecipeBookDisplay::Furnace {
        ingredient: mod_api::RecipeSlotDisplay::Item(ingredient.into()),
        fuel: mod_api::RecipeSlotDisplay::AnyFuel,
        result: mod_api::RecipeSlotDisplay::Item(result.into()),
        station: mod_api::RecipeSlotDisplay::Item("minecraft:furnace".into()),
        duration: 200,
        experience,
    }
}

struct RouterMod;
#[mod_api::__subscriptions]
impl mod_api::Hooks for RouterMod {
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        r.register_recipe_type("review:furnace");
        r.register_raw_recipe_serializer("review:furnace", "review:furnace");
        let key = r.recipe_book_type("review:book").unwrap();
        let _ = ROUTER_BOOK_KEY.set(key.clone());
        r.menu_recipe_book(
            "review:menu",
            &key,
            "review:furnace",
            mod_api::RecipeBookLayout::Furnace {
                input: 0,
                fuel: 1,
                result: 2,
            },
        )
        .unwrap();
    }
    fn recipe_book_categories(
        &self,
        _: mod_api::HostSide,
        event: &mut mod_api::RecipeBookCategoriesEvent<'_>,
    ) {
        event.categories.push(mod_api::RecipeBookCategory {
            id: "review:cat".into(),
            recipe_type: "review:furnace".into(),
            icon: "minecraft:furnace".into(),
            secondary_icon: None,
            target_menu: "review:menu".into(),
            book: ROUTER_BOOK_KEY.get().unwrap().clone(),
            search_group: "review:search".into(),
            displays: vec![mod_api::RecipeBookDisplayEntry {
                recipe: "review:recipe".into(),
                displays: vec![
                    router_display("minecraft:iron_ore", "minecraft:iron_ingot", 0.7),
                    router_display("minecraft:gold_ore", "minecraft:gold_ingot", 1.0),
                ],
                placement: Some(mod_api::PlacementInfo {
                    cells: vec![Some(mod_api::Ingredient::of(vec![
                        "minecraft:iron_ore".into(),
                    ]))],
                    shape: None,
                }),
            }],
        });
    }
}

fn drain_placement_frames(
    rx: &std::sync::mpsc::Receiver<sunrise_client::lan::Out>,
) -> Vec<sync::Frame> {
    use sunrise_client::recipe_book::{settings_codec, settings_sync::Message};
    use sunrise_protocol::packets::play::Clientbound as C;
    let mut frames = Vec::new();
    for out in rx.try_iter() {
        let bodies = match out {
            sunrise_client::lan::Out::Packet(body) => vec![body],
            sunrise_client::lan::Out::Packets(bodies) => bodies,
            sunrise_client::lan::Out::Disconnect(_) => panic!("unexpected disconnect"),
        };
        for body in bodies {
            let mut reader = sunrise_protocol::buf::Reader::new(&body);
            let id = reader.varint().unwrap();
            if let Ok(C::CustomPayload(payload)) = C::decode(id, reader.rest())
                && payload.channel == "sunrise:recipe_book_settings"
                && let Some(Message::Placement(frame)) = settings_codec::decode(&payload.data)
            {
                frames.push(frame);
            }
        }
    }
    frames
}

#[test]
#[ignore = "daily"]
fn mod_book_router_places_without_ack_and_ghosts_selected_variant() {
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new("review", Box::new(RouterMod))].into();
    mod_platform::testing::with_mods(mods, || {
        sunrise_client::containers::with_mod_menu_types(
            [(
                "review:menu".into(),
                "review".into(),
                3,
                0,
                vec![2],
                vec![],
                None,
                None,
                true,
                None,
            )],
            || {
                super::super::native_guest_turn_tests::with_guests(|game| {
                    mod_api::__private::resolve_menus(["review:menu"]).unwrap();
                    let recipes = sunrise_client::crafting::Recipes::from_sources(
                        &game.options.resources,
                        game.items(),
                        &sunrise_world::loot::PackOverlay::NONE,
                        vec![(
                            "review:recipe".into(),
                            serde_json::json!({"type": "review:furnace"}),
                        )],
                        game.res.core.item_data.context(),
                    )
                    .unwrap();
                    game.recipe_tables.install_test_recipes(recipes);
                    game.recipe_tables.books = None;

                    let (pos, _, mut catalog, display) = open_mod_menu(game);
                    game.ensure_mod_book_displays();
                    let generation = game
                        .recipe_tables
                        .books
                        .as_ref()
                        .unwrap()
                        .as_ref()
                        .unwrap()
                        .generation();
                    catalog.generation = generation;
                    {
                        let mut server = super::super::native_book_placement::Server::new(1);
                        server.peer.hello(1, sync::schema()).unwrap();
                        server.peer.publish(catalog.clone()).unwrap();
                        server.displays.insert("review:recipe".into(), 2);
                        game.native_net.placement.servers.insert(12, server);
                    }
                    let (tx, rx) = std::sync::mpsc::channel();
                    game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                        g.sender = sunrise_client::lan::PlayerSender::new(tx, Default::default());
                        g.recipe_book.recipes.insert("review:recipe".into());
                        g.inventory[9] = Some(sunrise_client::items::ItemStack::new(
                            "minecraft:iron_ore",
                            2,
                        ));
                    });

                    // Ordinary placement updates real slots and emits no ACK or Ghost frame.
                    game.guest_mod_book_placement(
                        12,
                        sync::Frame::Place {
                            catalog: catalog.clone(),
                            display: display.clone(),
                            use_max: false,
                        },
                    );
                    assert!(drain_placement_frames(&rx).is_empty());
                    assert_eq!(
                        game.world.block_entities.get(&pos).unwrap().slots()[0]
                            .as_ref()
                            .unwrap()
                            .item
                            .as_ref(),
                        "minecraft:iron_ore"
                    );

                    // Missing ingredients sends one Ghost frame for the clicked non-first variant.
                    if let Some(BlockEntity::Mod(block)) = game.world.block_entities.get_mut(&pos) {
                        block.items[0] = None;
                    }
                    game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                        g.inventory[9] = None;
                    });
                    let second_variant = sync::DisplayId {
                        recipe: display.recipe.clone(),
                        source: 1,
                    };
                    game.guest_mod_book_placement(
                        12,
                        sync::Frame::Place {
                            catalog: catalog.clone(),
                            display: second_variant,
                            use_max: false,
                        },
                    );
                    assert_eq!(
                        drain_placement_frames(&rx),
                        vec![sync::Frame::Ghost {
                            catalog: catalog.clone(),
                            recipe: display.recipe.clone(),
                            display: router_display(
                                "minecraft:gold_ore",
                                "minecraft:gold_ingot",
                                1.0,
                            ),
                        }]
                    );

                    // A refused placement emits no Ghost/ACK and does not block later placement.
                    game.guest_mod_book_placement(
                        12,
                        sync::Frame::Place {
                            catalog: catalog.clone(),
                            display: sync::DisplayId {
                                recipe: display.recipe.clone(),
                                source: 99,
                            },
                            use_max: false,
                        },
                    );
                    assert!(drain_placement_frames(&rx).is_empty());
                    game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                        g.inventory[9] = Some(sunrise_client::items::ItemStack::new(
                            "minecraft:iron_ore",
                            1,
                        ));
                    });
                    game.guest_mod_book_placement(
                        12,
                        sync::Frame::Place {
                            catalog,
                            display,
                            use_max: false,
                        },
                    );
                    assert!(drain_placement_frames(&rx).is_empty());
                    assert_eq!(
                        game.world.block_entities.get(&pos).unwrap().slots()[0]
                            .as_ref()
                            .unwrap()
                            .item
                            .as_ref(),
                        "minecraft:iron_ore"
                    );
                });
            },
        );
    });
}
