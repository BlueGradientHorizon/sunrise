use super::Game;
use crate::game::chat_common::SessionDispatcher;
use crate::game::chat_common::command_export;
use std::sync::Arc;
use sunrise_client::{command::Registries, text::Text};
use sunrise_gameplay::world_clock::OVERWORLD_CLOCK;
use sunrise_gameplay::world_clock::WorldClock;

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn server_sound_event_names(&self) -> Vec<String> {
        // Server sound events come from command-registries.json, not client audio.
        for dir in [&self.options.resources, &self.ui.vanilla_root] {
            if let Some(export) = command_export(dir, self.options.dedicated.is_none()) {
                return export.sounds.clone();
            }
        }
        Vec::new()
    }
    pub(in crate::game) fn server_sync_pack_tag_ids(&mut self) {
        self.sync_pack_tag_ids();
    }
    pub(in crate::game) fn server_sync_hub_clocks(&mut self) {
        if self.lan.is_none() {
            return;
        }
        let snap = self.hub_clock_snapshot();
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        for (name, total, rate) in snap.others {
            host.hub.set_clock_state(&name, total, rate);
        }
        host.hub.overworld_day = snap.overworld_day;
        host.hub.level_clock = snap.level_clock;
    }
    /// Commands the game issues itself (the server console, harnesses, the
    /// control socket) run at the owner's level whatever Allow Commands says.
    pub(in crate::game) fn as_console<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let level = std::mem::replace(&mut self.ui.registries.permission, 4);
        let out = f(self);
        self.ui.registries.permission = level;
        out
    }
    /// `ServerClockManager.ServerClockInstance.loadFrom`: restore the existing
    /// Overworld state when a world opens; clear the other command-state cache.
    /// The carrier store's open-time state settles here too, before the
    /// first tick and any player: a refusal fails the open.
    pub(in crate::game::server_side) fn load_world_state(&mut self) -> crate::Result<()> {
        self.ui.dispatcher = SessionDispatcher(Arc::new(
            sunrise_client::command::vanilla::session_dispatcher(
                &self.level.rule_registry,
                &self.level.enabled_features,
            ),
        ));
        self.restore_native_commands();
        self.ui.clock = WorldClock::default();
        self.ui.end_clock = WorldClock::default();
        self.ui.command_clocks.clear();
        // PrimaryLevelData: the difficulty comes back from level.dat's difficulty_settings.
        self.ui.difficulty = self.level.difficulty_id();
        self.ui.active_clock = OVERWORLD_CLOCK.to_owned();
        if let Ok(bytes) =
            std::fs::read(self.side.world_root.join("data/minecraft/world_clocks.dat"))
            && let Ok(raw) = sunrise_world::compression::Compression::Gzip.decode(&bytes, 1 << 20)
            && let Ok((root, _)) =
                sunrise_core::nbt::read_named(&raw, sunrise_core::nbt::Limits::default())
            && let Some(sunrise_core::nbt::Tag::Compound(clocks)) = root.tag.get("data")
        {
            use sunrise_core::nbt::Tag;
            for (name, c) in clocks {
                let mut state = WorldClock::default();
                if let Some(Tag::Float(r)) = c.get("rate") {
                    state.rate = f32::from_bits(*r);
                }
                if let Some(Tag::Float(p)) = c.get("partial_tick") {
                    state.partial = f32::from_bits(*p);
                }
                if let Some(Tag::Byte(p)) = c.get("paused") {
                    state.paused = *p != 0;
                }
                let total = match c.get("total_ticks") {
                    Some(Tag::Long(t)) => *t,
                    _ => 0,
                };
                let name = name.to_string_lossy();
                if name == OVERWORLD_CLOCK {
                    self.ui.clock = state;
                } else if name == "minecraft:the_end" {
                    self.ui.end_clock = state;
                } else {
                    self.ui.command_clocks.insert(name, (total, state));
                }
            }
        }
        // The level the world opens in follows its own default clock; the Overworld's total
        // (level.dat's day time, already in World.day_time) goes to the table.
        let clock = self.level_clock_name(self.dim);
        self.world.day_time = self.switch_clock(&clock);
        self.build_registries();
        // The world opens with its test instances (`Registries.TEST_INSTANCE`).
        self.snapshot_test_defs();
        self.sync_rules();
        self.carrier_world_opened()
    }
    /// Save the clock beside level.dat (Level.write saves the game rules: a
    /// second writer of game_rules.dat could leave it with the rules loaded
    /// when the world opened).
    pub(in crate::game::server_side) fn save_world_state(&mut self) {
        if self.side.world_root.as_os_str().is_empty() {
            return;
        }
        use sunrise_core::nbt::Tag;
        // Every clock the world has met is saved; the fields of a clock Sunrise does not model
        // stay as saved, and (like vanilla's codec) rate, partial tick and pause are omitted
        // while they have their defaults.
        let path = self.side.world_root.join("data/minecraft/world_clocks.dat");
        let existing = sunrise_client::save::read_dat(&path);
        let mut data = match existing.as_ref().and_then(|r| r.get("data")) {
            Some(d @ Tag::Compound(_)) => d.clone(),
            _ => Tag::Compound(Default::default()),
        };
        let mut clocks: Vec<(String, i64, _)> = vec![(
            OVERWORLD_CLOCK.to_owned(),
            self.command_clock_time(OVERWORLD_CLOCK),
            self.ui.clock,
        )];
        clocks.push((
            "minecraft:the_end".to_owned(),
            self.command_clock_time("minecraft:the_end"),
            self.ui.end_clock,
        ));
        for (name, (_, state)) in &self.ui.command_clocks {
            if name != OVERWORLD_CLOCK && name != "minecraft:the_end" {
                clocks.push((name.clone(), self.command_clock_time(name), *state));
            }
        }
        for (name, total, c) in clocks {
            let mut clock = match data.get(&name) {
                Some(c @ Tag::Compound(_)) => c.clone(),
                _ => Tag::Compound(Default::default()),
            };
            if let Tag::Compound(map) = &mut clock {
                map.insert("total_ticks".into(), Tag::Long(total));
                for (key, value, default) in [
                    (
                        "partial_tick",
                        Tag::Float(c.partial.to_bits()),
                        c.partial == 0.0,
                    ),
                    ("rate", Tag::Float(c.rate.to_bits()), c.rate == 1.0),
                    ("paused", Tag::Byte(i8::from(c.paused)), !c.paused),
                ] {
                    if default {
                        map.remove(&key.into());
                    } else {
                        map.insert(key.into(), value);
                    }
                }
            }
            if let Tag::Compound(map) = &mut data {
                map.insert(name.as_str().into(), clock);
            }
        }
        let root = Tag::compound([
            ("data", data),
            ("DataVersion", Tag::Int(sunrise_world::DATA_VERSION)),
        ]);
        if let Err(e) = sunrise_client::save::write_dat(&path, root) {
            log::warn!("the world clocks were not written: {e}");
        }
    }
    /// A server-side system message: captured, queued to the acting player, or logged.
    pub(in crate::game) fn server_system_message(&mut self, text: Text) {
        assert!(
            self.is_logical_server(),
            "server_system_message called on Client role"
        );
        if let Some(capture) = self.side.chat_capture.as_mut() {
            capture.push(text);
            return;
        }
        if self.side.lan_acting.is_some() {
            self.side
                .guest_packets
                .push(sunrise_protocol::packets::play::Clientbound::SystemChat(
                    sunrise_protocol::packets::play::SystemChat {
                        content: sunrise_client::lan::data::component_nbt(&text.component()),
                        overlay: false,
                    },
                ));
            return;
        }
        if self.is_dedicated_server() {
            super::dedicated::log("INFO", &format!("System chat: {}", text.plain()));
        } else {
            log::info!("System chat: {}", text.plain());
        }
    }
    /// Command suggestions include the world's currently named random sequences.
    pub(in crate::game::server_side) fn suggestion_registries(&mut self) -> &mut Registries {
        self.ui.registries.random_sequences = self.side.rand_sequences.ids();
        &mut self.ui.registries
    }
    pub(in crate::game) fn server_block_entity_nbt(
        &self,
        pos: [i32; 3],
    ) -> Option<sunrise_core::nbt::Tag> {
        self.block_entity_nbt(pos)
    }
    pub(in crate::game) fn server_send_chat_command(&mut self, _command: &str) {
        debug_assert!(false, "server never sends client chat commands");
    }
    pub(in crate::game) fn server_apply_item_splash(
        &mut self,
        splash: sunrise_client::entities::ItemSplash,
    ) {
        if let sunrise_client::entities::ItemSplash::Sound { at, volume, pitch } = splash {
            self.server_play_sound("minecraft:entity.generic.splash", at, volume, pitch);
        }
    }
    pub(in crate::game) fn server_level_min_y(&self) -> i32 {
        self.world.shape.min_y
    }
}

#[cfg(test)]
pub(in crate::game) mod enchantment_snapshot_tests {
    use crate::game::{Options, Progress, Settings, load_resources};

    #[test]
    #[ignore = "daily"]
    fn synchronized_definitions_refresh_existing_enchantment_views() {
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            None,
        )
        .unwrap();
        game.build_registries();

        let dir = std::env::temp_dir().join(format!("sunrise-h024-model-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let pack = dir.join("datapacks/model");
        let tags = pack.join("data/h024/tags/enchantment");
        std::fs::create_dir_all(&tags).unwrap();
        std::fs::write(
            pack.join("pack.mcmeta"),
            r#"{"pack":{"description":"fixture"}}"#,
        )
        .unwrap();
        std::fs::write(tags.join("empty.json"), r#"{"values":[]}"#).unwrap();
        std::fs::write(
            dir.join("datapacks.json"),
            r#"{"enabled":["file/model"],"disabled":[]}"#,
        )
        .unwrap();
        game.side.world_root = dir;
        let stack = game.reload_datapacks(false, &[]).unwrap();
        let root = game.options.resources.clone();
        game.side.world_tables.0 = game
            .stage_world_tables(&stack, false, &root)
            .unwrap()
            .tables;
        assert!(
            game.side.world_tables.0.is_some(),
            "the real active table container must be available"
        );
        let base = game.stations().enchantments.clone();
        assert!(base.get("minecraft:sharpness").is_some());
        // Same definition independently admitted by fresh Java. This is controlled
        // synchronized model input, NOT a world-pack or packet-delivery claim.
        let definition = serde_json::json!({"description":"H024 audit","supported_items":"minecraft:diamond_sword",
            "weight":1,"max_level":1,"min_cost":{"base":1,"per_level_above_first":0},
            "max_cost":{"base":1,"per_level_above_first":0},"anvil_cost":1,"slots":["mainhand"],"effects":{}});
        let synced = std::sync::Arc::new(
            base.with_entries([("h024:audit", &definition)])
                .unwrap()
                .unwrap(),
        );
        game.side
            .world_tables
            .0
            .as_mut()
            .unwrap()
            .stations
            .enchantments = synced.clone();
        std::sync::Arc::make_mut(&mut game.side.world_tables.0.as_mut().unwrap().items)
            .enchantments = synced;
        assert!(
            game.active_enchantment_definitions()
                .get("h024:audit")
                .is_some(),
            "actual authority adopts the definition first"
        );
        let tags = vec!["h024:retained_tag".to_owned()];
        game.ui
            .registries
            .by_registry
            .entry("minecraft:enchantment".into())
            .or_default()
            .tags = tags.clone();
        let item_ids = game.ui.registries.ids("minecraft:item").to_vec();
        let effects = game.ui.registries.effects.clone();
        game.sync_function_registries(&stack);
        for phase in [0, 1] {
            if phase == 1 {
                game.build_registries();
            }
            let reg = &game.ui.registries;
            assert!(
                reg.ids("minecraft:enchantment")
                    .contains(&"h024:audit".to_owned()),
                "active custom ID reaches actual Game sync/build snapshot"
            );
            assert!(
                reg.ids("minecraft:enchantment")
                    .contains(&"minecraft:sharpness".to_owned())
            );
            assert!(
                !reg.ids("minecraft:enchantment")
                    .contains(&"h024:missing".to_owned())
            );
            assert_eq!(
                reg.ids("minecraft:enchantment"),
                reg.enchantments.as_slice()
            );
            assert_eq!(reg.by_registry["minecraft:enchantment"].tags, tags);
            assert_eq!(reg.ids("minecraft:item"), item_ids);
            assert_eq!(reg.effects, effects);
        }
        game.side
            .world_tables
            .0
            .as_mut()
            .unwrap()
            .stations
            .enchantments = base.clone();
        std::sync::Arc::make_mut(&mut game.side.world_tables.0.as_mut().unwrap().items)
            .enchantments = base;
        assert!(
            game.active_enchantment_definitions()
                .get("h024:audit")
                .is_none()
        );
        game.sync_function_registries(&stack);
        assert!(
            !game
                .ui
                .registries
                .ids("minecraft:enchantment")
                .contains(&"h024:audit".to_owned()),
            "refresh replaces rather than accumulating stale IDs"
        );
        assert_eq!(
            game.ui.registries.ids("minecraft:enchantment"),
            game.ui.registries.enchantments.as_slice()
        );
        assert_eq!(
            game.ui.registries.by_registry["minecraft:enchantment"].tags,
            tags
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod damage_type_registry_tests {
    use crate::game::{Options, Progress, Settings, load_resources};

    /// A world pack's damage type is a `/damage` type once the world opens,
    /// beside vanilla's, and the registry keeps vanilla's tags.
    #[test]
    #[ignore = "daily"]
    fn a_world_packs_damage_type_parses_in_damage() {
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            None,
        )
        .unwrap();

        let dir = std::path::PathBuf::from(std::env::var_os("SUNRISE_HOME").unwrap())
            .join("damage-type-ids");
        let pack = dir.join("datapacks/model");
        let types = pack.join("data/example/damage_type");
        std::fs::create_dir_all(&types).unwrap();
        std::fs::write(
            pack.join("pack.mcmeta"),
            r#"{"pack":{"description":"test","pack_format":121}}"#,
        )
        .unwrap();
        std::fs::write(
            types.join("test.json"),
            r#"{"message_id":"example.test","scaling":"never","exhaustion":0.0}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("datapacks.json"),
            r#"{"enabled":["file/model"],"disabled":[]}"#,
        )
        .unwrap();
        game.side.world_root = dir;
        let stack = game.reload_datapacks(false, &[]).unwrap();
        let staged = game.stage_world_tables(&stack, false, &root).unwrap();
        game.side.world_tables.0 = staged.tables;
        game.health_resources = staged.health;
        game.level.allow_commands = Some(true);
        game.build_registries();

        let reg = &game.ui.registries;
        assert!(
            reg.damage_types.contains(&"example:test".to_owned()),
            "the damage catalog holds the pack's type"
        );
        let parse = |input: &str| {
            sunrise_client::command::vanilla::dispatcher()
                .parse(input, reg)
                .executable()
                .map(|_| ())
                .map_err(|e| format!("{e:?}"))
        };
        assert_eq!(parse("damage @s 1 example:test"), Ok(()));
        assert_eq!(parse("damage @s 1 minecraft:magic"), Ok(()));
        assert!(parse("damage @s 1 example:missing").is_err());
        assert!(
            reg.tags("minecraft:damage_type")
                .contains(&"minecraft:is_fire".to_owned()),
            "vanilla's damage type tags stay"
        );
    }
}

#[cfg(test)]
#[path = "tests/end_clock_tests_server.rs"]
pub(in crate::game) mod end_clock_tests;
