#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::native_entity_refs::tests::{fixture_root, with_client_game, with_game_role};
#[allow(unused_imports)]
use crate::game::{execution_role::ExecutionRole, lan::LanHost};
#[allow(unused_imports)]
use mod_api::prelude::ItemsHost;
#[allow(unused_imports)]
use mod_api::{BuildCreativeModeTabContentsEvent, Hooks, HostSide, ModEntry};
#[allow(unused_imports)]
use std::sync::Mutex;
#[allow(unused_imports)]
use sunrise_client::lan::{
    Lan, NetConfig,
    chunks::ChunkEncoder,
    data::SyncData,
    hub::{Hub, HubConfig},
};
#[allow(unused_imports)]
use sunrise_core::nbt::{Compound, Tag};
#[allow(unused_imports)]
use sunrise_world::loot::PackOverlay;

#[derive(Default)]
pub(in crate::game) struct Seen {
    pub(in crate::game) events: Vec<(LogicalSide, CreativeTabPermissions, String)>,
    pub(in crate::game) token: Option<CreativeTabEdit>,
}

pub(in crate::game) struct Edit(pub(in crate::game) Arc<Mutex<Seen>>);

pub(in crate::game) fn stack(id: &str, count: u32) -> mod_api::ItemStack {
    mod_api::ItemStack::new(mod_api::__private::item_key(id).unwrap(), count).unwrap()
}

pub(in crate::game) fn stone_key(max_stack: i32) -> mod_api::ItemStack {
    let mut stone = stack("minecraft:stone", 42);
    stone
        .set_components(Compound::from([(
            "minecraft:max_stack_size".into(),
            Tag::Int(max_stack),
        )]))
        .unwrap();
    stone
}

#[mod_api::__subscriptions]
impl Hooks for Edit {
    fn build_creative_mode_tab_contents(
        &self,
        host: HostSide,
        event: &mut BuildCreativeModeTabContentsEvent<'_>,
    ) {
        let (host, side): (&mut dyn mod_api::Host, _) = match host {
            HostSide::Client(host) => (host, LogicalSide::Client),
            HostSide::Server(host) => (host, LogicalSide::Server),
        };
        {
            let mut seen = self.0.lock().unwrap();
            seen.events
                .push((side, event.permissions, event.tab.to_owned()));
            seen.token = Some(event.edit);
        }
        if event.tab != "test:a" {
            return;
        }
        let previous = host.creative_tabs(event.permissions).unwrap();
        let b = &previous.tab("test:b").unwrap().display_items;
        match event.permissions {
            CreativeTabPermissions::Normal => assert!(b.is_empty()),
            CreativeTabPermissions::Operator => {
                assert_eq!(b, &[stack("minecraft:sand", 1)]);
                let published = host.built_creative_tabs().unwrap().unwrap();
                assert_eq!(published.permissions, CreativeTabPermissions::Normal);
                assert_eq!(
                    published.tab("test:b").unwrap().display_items,
                    [stack("minecraft:sand", 1)]
                );
            }
        }
        let other = match event.permissions {
            CreativeTabPermissions::Normal => CreativeTabPermissions::Operator,
            CreativeTabPermissions::Operator => CreativeTabPermissions::Normal,
        };
        assert!(matches!(
            host.creative_tabs(other),
            Err(CreativeTabReadError::Reentrant)
        ));
        assert_eq!(
            event.edit.insert(
                host,
                CreativeTabPlacement::Last,
                stack("minecraft:dirt", 2),
                CreativeTabVisibility::Both
            ),
            Err(CreativeTabEditError::Count),
        );
        assert_eq!(
            event.edit.insert(
                host,
                CreativeTabPlacement::Before(stone_key(64)),
                stack("minecraft:dirt", 1),
                CreativeTabVisibility::Both
            ),
            Err(CreativeTabEditError::MissingAnchor(CreativeTabList::Search)),
        );
        let mut absent = stack("minecraft:iron_sword", 50);
        absent.set_damage(1000);
        assert_eq!(
            event.edit.insert(
                host,
                CreativeTabPlacement::After(absent.clone()),
                stack("minecraft:glass", 1),
                CreativeTabVisibility::Parent,
            ),
            Err(CreativeTabEditError::MissingAnchor(CreativeTabList::Parent))
        );
        event
            .edit
            .remove(host, &absent, CreativeTabVisibility::Both)
            .unwrap();
        absent.set_damage(u32::MAX);
        assert_eq!(
            event
                .edit
                .remove(host, &absent, CreativeTabVisibility::Both),
            Err(CreativeTabEditError::Stack(
                mod_api::StackWriteError::InvalidComponents {
                    reason: mod_api::StackError::InvalidDamage,
                },
            )),
        );
        assert_eq!(
            event.edit.insert(
                host,
                CreativeTabPlacement::Last,
                stack("minecraft:stick", 1),
                CreativeTabVisibility::Both,
            ),
            Err(CreativeTabEditError::Duplicate(CreativeTabList::Search))
        );
        // The disabled barrier still anchors the insertion during the event.
        event
            .edit
            .insert(
                host,
                CreativeTabPlacement::Before(stack("minecraft:barrier", 2)),
                stack("minecraft:torch", 1),
                CreativeTabVisibility::Search,
            )
            .unwrap();
        event
            .edit
            .remove(
                host,
                &stack("minecraft:stick", 47),
                CreativeTabVisibility::Search,
            )
            .unwrap();
    }
}

pub(in crate::game) struct Observe;

#[mod_api::__subscriptions]
impl Hooks for Observe {
    fn build_creative_mode_tab_contents(
        &self,
        host: HostSide,
        event: &mut BuildCreativeModeTabContentsEvent<'_>,
    ) {
        if event.tab != "test:a" {
            return;
        }
        let host: &dyn mod_api::Host = match host {
            HostSide::Client(host) => host,
            HostSide::Server(host) => host,
        };
        assert_eq!(
            event.edit.entries(host, CreativeTabList::Parent).unwrap(),
            [
                stack("minecraft:dirt", 1),
                stack("minecraft:stone", 1),
                stack("minecraft:barrier", 1),
                stack("minecraft:stick", 1)
            ]
        );
        assert_eq!(
            event.edit.entries(host, CreativeTabList::Search).unwrap(),
            [stack("minecraft:torch", 1), stack("minecraft:barrier", 1)]
        );
        let mut stone = stone_key(64);
        stone.set_count(1).unwrap();
        event
            .edit
            .insert(
                host,
                CreativeTabPlacement::Last,
                stone,
                CreativeTabVisibility::Search,
            )
            .unwrap();
    }
}

pub(in crate::game) fn definition(max_level: i32) -> serde_json::Value {
    serde_json::json!({
        "description": {"text":"Fixture levels"}, "supported_items":"minecraft:iron_sword",
        "weight": 1, "anvil_cost": 1, "max_level": max_level,
        "min_cost": {"base": 1,"per_level_above_first": 1},
        "max_cost": {"base": 10,"per_level_above_first": 1}, "slots": ["mainhand"], "effects": {},
    })
}

#[allow(unused_imports)]
pub(in crate::game) use crate::game::server_side::native_hooks_server::build_creative_mode_tab_contents::tests::install_dedicated_holders;

pub(in crate::game) fn replace_client_definition(game: &mut crate::game::ClientGame<'_>, max_level: i32) {
    let mut registries = (*game.remote.as_ref().unwrap().registries).clone();
    registries.registries.insert(
        "minecraft:enchantment".into(),
        vec![(
            "test:levels".into(),
            Some(sunrise_client::scoreboard::json_to_tag(&definition(
                max_level,
            ))),
        )],
    );
    game.set_registries(Arc::new(registries));
}

pub(in crate::game) fn book_level(snapshot: &CreativeModeTabs) -> i32 {
    let books = &snapshot.tab("test:holders").unwrap().display_items;
    let levels: Vec<_> = books
        .iter()
        .filter_map(|book| {
            let Tag::Int(level) = book
                .components()?
                .as_nbt()
                .get(&"minecraft:stored_enchantments".into())?
                .get("test:levels")?
            else {
                return None;
            };
            Some(*level)
        })
        .collect();
    let [level] = levels.as_slice() else {
        panic!("expected one highest-level fixture book, got {levels:?}")
    };
    *level
}

#[test]
fn a_tab_reads_back_its_title_as_a_translatable_with_the_title_as_fallback() {
    let catalog = Catalog::parse(&serde_json::json!({
        "schema":4, "item_features":{}, "potion_features":{},
        "tabs":[
            {"id":"minecraft:building_blocks","title":"Building Blocks","type":"CATEGORY","build":[]},
            {"id":"ruby:gems","title":"Ruby Gems","type":"CATEGORY","build":[]},
        ]
    }))
    .unwrap();
    let lists = [Arc::default(), Arc::default()];
    let tabs = snapshot(&catalog, CreativeTabPermissions::Normal, &lists);
    let name = |tab: &str| match &tabs.tab(tab).unwrap().display_name.content {
        mod_api::text::TextContent::Translatable {
            key,
            fallback,
            arguments,
        } => (key.clone(), fallback.clone(), arguments.is_empty()),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        name("minecraft:building_blocks"),
        (
            "itemGroup.buildingBlocks".to_owned(),
            Some("Building Blocks".to_owned()),
            true
        )
    );
    assert_eq!(
        name("ruby:gems"),
        (
            "itemGroup.ruby.gems".to_owned(),
            Some("Ruby Gems".to_owned()),
            true
        )
    );
}

/// A dedicated server's `creative_tabs` builds vanilla's tabs from the prepared catalog: the
/// building blocks tab starts with oak log, as the client's creative screen shows it.
#[test]
#[ignore = "daily"]
fn a_servers_creative_tabs_hold_vanillas_items() {
    mod_platform::testing::with_mods(Vec::new().into(), || {
        with_game_role(fixture_root(), ExecutionRole::DedicatedServer, |game| {
            use sunrise_client::save::{Level, write_dat};

            let save = tempfile::tempdir().unwrap();
            write_dat(
                &save.path().join("level.dat"),
                Tag::compound([(
                    "Data",
                    Tag::compound([("DataVersion", Tag::Int(sunrise_core::version::DATA_VERSION))]),
                )]),
            )
            .unwrap();
            game.level = Level::read(save.path()).unwrap();
            install_dedicated_holders(game, 2);
            for permissions in [
                CreativeTabPermissions::Normal,
                CreativeTabPermissions::Operator,
            ] {
                let tabs = ItemsHost::creative_tabs(game, permissions).unwrap();
                let building = tabs.tab("minecraft:building_blocks").unwrap();
                let oak_log = stack("minecraft:oak_log", 1);
                assert_eq!(building.display_items.first(), Some(&oak_log));
                assert!(building.search_items.contains(&oak_log));
                assert!(
                    !tabs
                        .tab("minecraft:natural_blocks")
                        .unwrap()
                        .display_items
                        .is_empty()
                );
                assert!(
                    !tabs
                        .tab("minecraft:search")
                        .unwrap()
                        .display_items
                        .is_empty()
                );
            }
        });
    });
}
