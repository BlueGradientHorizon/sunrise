#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_entity_refs::tests::{fixture_root, with_client_game, with_game_role};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{execution_role::ExecutionRole, lan::LanHost};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::prelude::ItemsHost;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{BuildCreativeModeTabContentsEvent, Hooks, HostSide, ModEntry};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Mutex;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::lan::{
    Lan, NetConfig,
    chunks::ChunkEncoder,
    data::SyncData,
    hub::{Hub, HubConfig},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::{Compound, Tag};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::loot::PackOverlay;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::Seen;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::Edit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::stack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::stone_key;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::Observe;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::definition;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::replace_client_definition;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::build_creative_mode_tab_contents::tests::book_level;

#[test]
#[ignore = "daily"]
fn creative_catalog_builds_lazily_and_edits_before_filtering_on_both_sides() {
    use crate::game::native_client_session::tests::logged_client;
    use sunrise_client::creative::Creative;
    let row = |item: &str, visibility: &str| {
        serde_json::json!({
            "kind":"stack", "snbt":format!(r#"{{id:"{item}",count:1}}"#), "visibility":visibility,
        })
    };
    let both = "PARENT_AND_SEARCH_TABS";
    let catalog = serde_json::json!({
        "schema":4, "item_features":{"minecraft:barrier":["test:disabled"]}, "potion_features":{},
        "tabs":[
            {"id":"test:a","title":"test:a","type":"CATEGORY","build":[
                row("minecraft:stone", "PARENT_TAB_ONLY"), row("minecraft:stick", "SEARCH_TAB_ONLY"), row("minecraft:barrier", both)
            ]},
            {"id":"minecraft:hotbar","title":"minecraft:hotbar","type":"HOTBAR","build":[]},
            {"id":"test:b","title":"test:b","type":"CATEGORY","build":[row("minecraft:sand", both)],"operator_build":[row("minecraft:glass", both)]},
            {"id":"test:holders","title":"test:holders","type":"CATEGORY","build":[{"kind":"enchantments","all_levels":false,"visibility":both}]},
            {"id":"minecraft:search","title":"minecraft:search","type":"SEARCH","build":[]},
            {"id":"minecraft:inventory","title":"minecraft:inventory","type":"INVENTORY","build":[]},
        ]
    });
    let seen = Arc::new(Mutex::new(Seen::default()));
    let setups = vec![
        ModEntry::new("first_edit", Box::new(Edit(seen.clone()))),
        ModEntry::new("later_edit", Box::new(Observe)),
    ]
    .into();
    let check = |game: &mut Game<'_>, side| {
        game.creative_screen.0 = Creative::from_json(&catalog).unwrap();
        assert!(!game.creative_screen.0.has_contents());
        let flags = |game: &mut Game<'_>, ids: [&str; 2]| {
            let flags = ids.map(str::to_owned);
            if let Some(remote) = &mut game.remote {
                remote.features = flags.into();
            } else {
                game.level.enabled_features = flags.into();
            }
        };
        flags(game, ["test:alpha", "test:beta"]);
        assert_eq!(game.stock_sort_order()[0].item.as_ref(), "minecraft:stick");
        assert!(seen.lock().unwrap().events.is_empty());
        {
            // A plain read before any build answers nothing and builds nothing.
            let reader: &dyn mod_api::Host = game;
            assert_eq!(reader.built_creative_tabs(), Ok(None));
            assert!(seen.lock().unwrap().events.is_empty());
        }
        game.build_creative(CreativeTabPermissions::Normal).unwrap();
        let built_events = seen.lock().unwrap().events.len();
        let reader: &dyn mod_api::Host = game;
        let normal = reader.built_creative_tabs().unwrap().unwrap();
        assert_eq!(normal.permissions, CreativeTabPermissions::Normal);
        let cached = ItemsHost::creative_tabs(game, CreativeTabPermissions::Normal).unwrap();
        assert!(Arc::ptr_eq(&normal, &cached));
        assert_eq!(seen.lock().unwrap().events.len(), built_events);
        assert_eq!(game.stock_sort_order()[0].item.as_ref(), "minecraft:torch");
        let a = normal.tab("test:a").unwrap();
        assert_eq!(
            a.display_items,
            [
                stack("minecraft:dirt", 1),
                stack("minecraft:stone", 1),
                stack("minecraft:stick", 1)
            ]
        );
        assert_eq!(
            a.search_items,
            [stack("minecraft:torch", 1), stack("minecraft:stone", 1)]
        );
        flags(game, ["test:beta", "test:alpha"]);
        let host: &mut dyn mod_api::Host = game;
        assert!(!a.contains(host, &stack("minecraft:dirt", 1)));
        assert!(a.contains(host, &stack("minecraft:torch", 8)));
        assert_eq!(
            normal
                .tabs_showing(host, &stack("minecraft:dirt", 1))
                .next()
                .unwrap()
                .id,
            "test:a"
        );
        assert!(
            normal
                .tabs_showing(host, &stack("minecraft:torch", 1))
                .next()
                .is_none()
        );
        assert!(a.contains(host, &stone_key(64)));
        assert_eq!(
            normal.tabs_showing(host, &stone_key(64)).next().unwrap().id,
            "test:a"
        );
        assert!(!a.contains(host, &stone_key(32)));
        assert!(normal.tabs_showing(host, &stone_key(32)).next().is_none());
        let cached = host.creative_tabs(CreativeTabPermissions::Normal).unwrap();
        assert!(Arc::ptr_eq(&normal, &cached));
        let old = seen.lock().unwrap().token.unwrap();
        assert_eq!(
            old.entries(host, CreativeTabList::Search),
            Err(CreativeTabEditError::Stale)
        );
        assert_eq!(
            old.remove(
                host,
                &stack("minecraft:stone", 1),
                CreativeTabVisibility::Both
            ),
            Err(CreativeTabEditError::Stale)
        );
        let operator = host
            .creative_tabs(CreativeTabPermissions::Operator)
            .unwrap();
        assert!(!Arc::ptr_eq(&normal, &operator));
        assert_eq!(
            operator.tab("minecraft:search").unwrap().display_items[..3],
            [
                stack("minecraft:torch", 1),
                stack("minecraft:stone", 1),
                stack("minecraft:glass", 1)
            ]
        );
        assert_eq!(
            normal.tab("test:b").unwrap().display_items,
            [stack("minecraft:sand", 1)]
        );
        let actual = std::mem::take(&mut seen.lock().unwrap().events);
        let expected: Vec<_> = [
            CreativeTabPermissions::Normal,
            CreativeTabPermissions::Operator,
        ]
        .into_iter()
        .flat_map(|permissions| {
            [
                "test:a",
                "test:b",
                "test:holders",
                "minecraft:hotbar",
                "minecraft:search",
                "minecraft:inventory",
            ]
            .map(|tab| (side, permissions, tab.to_owned()))
        })
        .collect();
        assert_eq!(actual, expected);

        assert_eq!(book_level(&normal), 2);
        normal
    };
    let check_replacement = |game: &mut Game<'_>, old: &CreativeModeTabs| {
        // Registry installation must not dispatch; the next demand uses the changed definition.
        assert!(seen.lock().unwrap().events.is_empty());
        let fresh = ItemsHost::creative_tabs(game, CreativeTabPermissions::Normal).unwrap();
        assert_eq!(book_level(&fresh), 4);
        assert_eq!(book_level(old), 2);
        assert_eq!(seen.lock().unwrap().events.len(), 6);
        seen.lock().unwrap().events.clear();
    };
    mod_platform::testing::with_mods(setups, || {
        let mut dedicated = None;
        with_game_role(fixture_root(), ExecutionRole::DedicatedServer, |game| {
            install_dedicated_holders(game, 2);
            dedicated = Some(check(game, LogicalSide::Server));
        });
        with_game_role(fixture_root(), ExecutionRole::DedicatedServer, |game| {
            install_dedicated_holders(game, 4);
            game.creative_screen.0 = Creative::from_json(&catalog).unwrap();
            check_replacement(game, dedicated.as_deref().unwrap());
        });
        with_client_game(|game| {
            let _server = logged_client(game);
            game.native_lifecycle
                .begin_open(mod_api::Authority::RemoteClient);
            replace_client_definition(game, 2);
            let old = check(game, LogicalSide::Client);
            replace_client_definition(game, 4);
            check_replacement(game, &old);
            game.native_lifecycle.begin_close();
            game.native_lifecycle_world_closed();
            assert!(game.native_lifecycle.epoch().is_none());
            assert!(game.creative_contents.published.is_some());
            let host: &dyn mod_api::Host = game;
            assert_eq!(
                host.built_creative_tabs(),
                Err(CreativeTabReadError::Unavailable)
            );
        });
    });
}
