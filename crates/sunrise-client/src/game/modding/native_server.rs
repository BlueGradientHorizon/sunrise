//! Server commands and players for native mods (docs/modding/src/how-to/native-mods.md,
//! "Server commands and players"): the commands mods add to the
//! server's command tree (`native_register_commands`), a player's command
//! reaching its bound executor, and the helpers a server mod
//! answers with: the players online, a line to one of them or to all, the
//! command's feedback and the world's folder; and the acting
//! player's plain items: count, take, give. They act on the server's own
//! players (the hub's: a dedicated server's, a LAN host's, single player's
//! server thread's), never on a client's.
#[path = "native_server/argument_kinds.rs"]
pub(in crate::game) mod argument_kinds;
#[cfg(test)]
#[path = "native_server/command_failure_tests.rs"]
pub(in crate::game) mod command_failure_tests;
#[cfg(test)]
#[path = "native_server/command_shape_tests.rs"]
pub(in crate::game) mod command_shape_tests;
#[cfg(test)]
#[path = "native_server/provider_tests.rs"]
pub(in crate::game) mod provider_tests;
#[path = "native_server/registered_commands.rs"]
pub(in crate::game) mod registered_commands;
#[cfg(test)]
#[path = "native_server/selector_tests.rs"]
pub(in crate::game) mod selector_tests;
#[cfg(test)]
#[path = "native_server/sidebar_tests.rs"]
pub(in crate::game) mod sidebar_tests;

#[allow(unused_imports)]
use super::api_values::{TryIntoApi, TryIntoEngine};
#[allow(unused_imports)]
use super::{Dim, Game, commands::execute, respawn_anchor::RespawnConfig};
#[allow(unused_imports)]
use crate::game::level_control_hooks::LevelControl;
#[allow(unused_imports)]
pub(in crate::game) use argument_kinds::{CommandArguments, command_arguments};
#[allow(unused_imports)]
use argument_kinds::{
    UNCENTERED, argument_type, bounded_parser, custom_parser, custom_value, dimension_value,
    nbt_value, parsed_type, parser, positional_value, resource_parser,
};
#[allow(unused_imports)]
use mod_api::__private::game::*;
#[allow(unused_imports)]
pub(in crate::game) use registered_commands::NativeCommandState;
// Their old home, for mixin code that names them here.
#[allow(unused_imports)]
use mod_api::LevelRef;
#[allow(unused_imports)]
use mod_api::{
    ActionOutcome, ActionRefused, FakeEffect, GameMode, GlobalPos, Identifier, PlayerAction,
    PlayerActionSetup, PlayerFood, PlayerRef, PlayerRespawnConfig, ScopedActionResult, Sidebar,
    TextFormError, VirtualHost,
};
#[allow(unused_imports)]
use serde_json::{Map, Value, json};
#[allow(unused_imports)]
use std::sync::{
    LazyLock, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};
#[allow(unused_imports)]
use std::{collections::HashMap, path::PathBuf, sync::Arc};
#[allow(unused_imports)]
use sunrise_client::{
    command::{Arg, ArgType, Context, Dispatcher},
    items::ItemStack,
    lan::{
        abilities::PlayerAbilities, data::component_nbt, guest::Guest, scoreboard::SidebarContents,
    },
    motion::PlayerMotion,
    text::Text,
};
#[allow(unused_imports)]
use sunrise_core::position::BlockPos;
#[allow(unused_imports)]
use sunrise_protocol::packets::play::{self, Clientbound as C};

/// The report's name of an operator level (`commands.json`'s
/// `permissions`).
pub(in crate::game) const LEVELS: [&str; 5] = ["", "moderators", "gamemasters", "admins", "owners"];

/// One parsed word of a usage: its key in the tree and its node, without
/// children.
pub(in crate::game) fn usage_nodes(
    usage: &str,
    custom: &CommandArguments,
    client: bool,
) -> Result<Vec<(String, Value, bool)>, String> {
    let mut out = Vec::new();
    let mut optional_seen = false;
    let mut greedy_seen = false;
    for (index, word) in usage.split_whitespace().enumerate() {
        if greedy_seen {
            return Err(format!("`{usage}`: a text argument must be last"));
        }
        let (inner, optional) = if let Some(w) = word.strip_prefix('<') {
            (w.strip_suffix('>'), false)
        } else if let Some(w) = word.strip_prefix('[') {
            (w.strip_suffix(']'), true)
        } else {
            if optional_seen {
                return Err(format!("`{usage}`: `{word}` follows an optional argument"));
            }
            if !argument_kinds::command_word(word, index == 0) {
                return Err(format!("`{usage}`: `{word}` is not a command word"));
            }
            out.push((word.to_owned(), json!({"type": "literal"}), false));
            continue;
        };
        let Some((name, kind)) = inner.and_then(|w| w.split_once(':')) else {
            return Err(format!("`{usage}`: `{word}` is not `<name:kind>`"));
        };
        if optional_seen && !optional {
            return Err(format!("`{usage}`: `{word}` follows an optional argument"));
        }
        let resource = kind
            .strip_prefix("resource:")
            .map(|registry| resource_parser(registry).map_err(|e| format!("`{usage}`: {e}")))
            .transpose()?;
        let bounded = bounded_parser(kind)
            .transpose()
            .map_err(|e| format!("`{usage}`: {e}"))?;
        let builtin = resource
            .or(bounded)
            .or_else(|| parser(kind).map(|(parser, properties, _)| (parser, properties)));
        let (parser, properties, greedy, custom) = match builtin {
            Some((parser, properties)) => (parser, properties, kind == "text", None),
            None => {
                let Some(argument) = custom.get(kind) else {
                    return Err(format!("`{usage}`: unknown kind `{kind}`"));
                };
                // A declared provider answers on the server alone: whoever types the argument asks
                // the server, and no mod code runs in a client. A client tree has no server behind
                // it, so it refuses the usage by name rather than run the provider locally.
                if client && declares_provider(kind, argument) {
                    return Err(format!(
                        "`{usage}`: argument type `{kind}` declares a suggestion provider, which \
                         only a server command can offer; a client command completes without one"
                    ));
                }
                let declared =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| argument.kind()))
                        .map_err(|_| format!("`{usage}`: argument type `{kind}` panicked"))?;
                let Some((parser, properties)) = custom_parser(declared) else {
                    return Err(format!(
                        "`{usage}`: argument type `{kind}` uses a kind this build cannot read"
                    ));
                };
                let greedy = properties["type"] == "greedy";
                (parser, properties, greedy, Some(kind))
            }
        };
        optional_seen |= optional;
        greedy_seen |= greedy;
        let mut node = json!({"type": "argument", "parser": parser});
        if !properties.is_null() {
            node["properties"] = properties;
        }
        if kind == "vec3_uncentered" {
            node[UNCENTERED] = true.into();
        }
        // A registered type goes out as the Brigadier type its kind names, which an unmodded
        // client can read; it and the free `id`, which has no list of its own, ask the server
        // (the mods' `tab_complete`).
        if custom.is_some() || kind == "id" {
            if let Some(id) = custom {
                node["custom_argument"] = id.into();
            }
            node["suggestions"] = "minecraft:ask_server".into();
        }
        out.push((name.to_owned(), node, optional));
    }
    if out.first().is_none_or(|n| n.1["type"] != "literal") {
        return Err(format!("`{usage}`: a command starts with its name"));
    }
    Ok(out)
}

/// The tree a mod's usages meet at their root word.
#[derive(Clone, Copy)]
pub(in crate::game) enum BaseTree<'a> {
    /// The client's own chat tree: a client command whose root it has (`has` answers) is left
    /// out, so it never hides a server command.
    Reserved(&'a dyn Fn(&str) -> bool),
    /// The server's vanilla tree. A usage whose root vanilla registers merges into that root, as
    /// NeoForge's `RegisterCommandsEvent` registers into vanilla's `CommandDispatcher`, whose
    /// `register` adds the literal with Brigadier's `CommandNode.addChild`: vanilla's usages
    /// stay, the mod's children are added beside them, and vanilla's nodes keep their
    /// permissions. Sunrise's own tools (`/modprofile`, `/pregen`) are not merged into.
    Merged(&'a Dispatcher),
}

/// Native mods' commands as a root in the commands report's format
/// (`lan::commands`, which sends each player what their level allows), and
/// the traversal level of each command a mod owns: the lowest of its usages. A node is runnable
/// where only optional arguments follow it; like Brigadier's `CommandNode.addChild`, each
/// later accepted runnable registration overwrites the node's executor and execution level,
/// a vanilla node's executor too. A root merged into vanilla's keeps vanilla's level, so it has
/// none here. A usage that doesn't parse, meets a vanilla node it cannot share or names a root
/// `base` keeps is left out, with why.
#[cfg(test)]
pub(in crate::game) fn command_tree(
    commands: &[mod_api::Command],
    vanilla: &Dispatcher,
    custom: &CommandArguments,
) -> (Value, HashMap<String, u8>, Vec<String>) {
    command_tree_for(commands, BaseTree::Merged(vanilla), custom, false, None)
}

/// Build the display tree for the client's own commands. An argument type that declares a
/// suggestion provider is refused by name because a client command has no server to answer it.
/// A root the chat tree already has (`vanilla` answers) is left out. Every other argument kind
/// builds exactly as on the server.
pub(in crate::game) fn client_command_tree(
    commands: &[mod_api::Command],
    vanilla: impl Fn(&str) -> bool,
    custom: &CommandArguments,
) -> (Value, HashMap<String, u8>, Vec<String>) {
    command_tree_for(commands, BaseTree::Reserved(&vanilla), custom, true, None)
}

/// Whether a usage merges into a vanilla root of `base`, or why it cannot go into the tree. Each
/// word under a merged root that vanilla has at the same place must be a literal on a literal:
/// Brigadier would keep vanilla's argument type for the mod's argument of the same name, and the
/// mod's executor would read a value of another kind.
pub(in crate::game) fn base_root(
    base: BaseTree<'_>,
    usage: &str,
    nodes: &[(String, Value, bool)],
) -> Result<bool, String> {
    let root = nodes[0].0.as_str();
    let vanilla = match base {
        BaseTree::Reserved(has) if has(root) => {
            return Err(format!("`{usage}`: vanilla has /{root}"));
        }
        BaseTree::Reserved(_) => return Ok(false),
        BaseTree::Merged(vanilla) => vanilla,
    };
    let Some(mut at) = vanilla.find(root) else {
        return Ok(false);
    };
    if sunrise_client::command::vanilla::is_session_tool(root) {
        return Err(format!("`{usage}`: /{root} is the game's own command"));
    }
    for (name, node, _) in &nodes[1..] {
        let Some(child) = vanilla.nodes[at]
            .children
            .iter()
            .copied()
            .find(|&child| vanilla.nodes[child].name() == name)
        else {
            break;
        };
        let literal = matches!(
            vanilla.nodes[child].kind,
            sunrise_client::command::NodeKind::Literal(_)
        );
        if !literal || node["type"] != "literal" {
            return Err(format!(
                "`{usage}`: `{name}` is a node of vanilla's /{root} that a mod argument cannot share"
            ));
        }
        at = child;
    }
    Ok(true)
}

/// Whether `argument` declares a suggestion provider, read once per tree. A panic is logged and
/// counts as no provider, as `argument_type` reads it when it builds the node.
pub(in crate::game) fn declares_provider(
    kind: &str,
    argument: &Arc<dyn mod_api::CommandArgumentType>,
) -> bool {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        argument.suggestion_provider()
    })) {
        Ok(provider) => provider.is_some(),
        Err(_) => {
            eprintln!(
                "mod command argument type {kind} panicked declaring its suggestion provider; \
                 its arguments complete by `suggest`"
            );
            false
        }
    }
}

// Executable paths (including argument names) mapped to the input registration slot.
pub(in crate::game) type CommandExecutorSlots = HashMap<Vec<String>, usize>;

pub(in crate::game) fn command_tree_for(
    commands: &[mod_api::Command],
    base: BaseTree<'_>,
    custom: &CommandArguments,
    client: bool,
    mut executors: Option<&mut CommandExecutorSlots>,
) -> (Value, HashMap<String, u8>, Vec<String>) {
    let mut root: Map<String, Value> = Map::new();
    let mut levels: HashMap<String, u8> = HashMap::new();
    let mut problems = Vec::new();
    for (slot, command) in commands.iter().enumerate() {
        let nodes = match usage_nodes(&command.usage, custom, client) {
            Ok(n) => n,
            Err(e) => {
                problems.push(e);
                continue;
            }
        };
        let merged = match base_root(base, &command.usage, &nodes) {
            Ok(merged) => merged,
            Err(e) => {
                problems.push(e);
                continue;
            }
        };
        // A rejected usage must not open an earlier executable prefix or lower its level.
        let mut children = &root;
        let mut conflict = None;
        for (name, node, _) in &nodes {
            let Some(entry) = children.get(name) else {
                break;
            };
            // `word` and `text` share a parser; their properties tell them apart, and a custom
            // kind shares the word (or greedy) properties with them.
            if entry["type"] != node["type"]
                || entry["parser"] != node["parser"]
                || entry["properties"] != node["properties"]
                || entry["custom_argument"] != node["custom_argument"]
                || entry[UNCENTERED] != node[UNCENTERED]
            {
                conflict = Some(name);
                break;
            }
            children = entry["children"]
                .as_object()
                .expect("working tree children");
        }
        if let Some(name) = conflict {
            problems.push(format!(
                "`{}`: `{name}` is another kind in an earlier usage",
                command.usage
            ));
            continue;
        }
        let execution_level = u64::from(command.level.min(4));
        let mut children = &mut root;
        for (i, (name, node, _)) in nodes.iter().enumerate() {
            let runnable = nodes.get(i + 1).is_none_or(|n| n.2);
            let entry = children.entry(name.clone()).or_insert_with(|| {
                let mut n = node.clone();
                n["executable"] = false.into();
                n["level"] = 4.into();
                n
            });
            // Like Brigadier's `CommandNode.addChild`, each later accepted runnable
            // registration overwrites the node's executor and execution level.
            if runnable {
                entry["executable"] = true.into();
                entry["execution_level"] = execution_level.into();
                if let Some(executors) = executors.as_deref_mut() {
                    let path = nodes[..=i]
                        .iter()
                        .map(|(name, _, _)| name.clone())
                        .collect();
                    executors.insert(path, slot);
                }
            }
            let level = entry["level"]
                .as_u64()
                .unwrap_or(4)
                .min(command.level.into());
            entry["level"] = level.into();
            if !entry["children"].is_object() {
                entry["children"] = Value::Object(Map::new());
            }
            children = entry["children"].as_object_mut().expect("made above");
        }
        if !merged {
            let level = levels.entry(nodes[0].0.clone()).or_insert(4);
            *level = (*level).min(command.level);
        }
    }
    let tree = if root.is_empty() {
        Value::Null
    } else {
        let mut root = Value::Object(root);
        finish(&mut root);
        json!({"type": "root", "children": root})
    };
    (tree, levels, problems)
}

/// Turns the working `level` into the report's `permissions`, and drops
/// empty child lists.
pub(in crate::game) fn finish(children: &mut Value) {
    let Some(map) = children.as_object_mut() else {
        return;
    };
    for node in map.values_mut() {
        let level = node["level"].as_u64().unwrap_or(0) as usize;
        let object = node.as_object_mut().expect("a node");
        object.remove("level");
        if level > 0 {
            object.insert(
                "permissions".into(),
                json!({"type": "require", "permission": {"level": LEVELS[level.min(4)]}}),
            );
        }
        if object
            .get("children")
            .is_some_and(|c| c.as_object().is_some_and(Map::is_empty))
        {
            object.remove("children");
        } else if let Some(c) = object.get_mut("children") {
            finish(c);
        }
    }
}

/// Add a validated native report for descriptor-only consumers.
pub(in crate::game) fn register_dispatcher(
    dispatcher: &mut Dispatcher,
    tree: &Value,
    handler: &'static str,
    arguments: &CommandArguments,
) {
    register_command_nodes(dispatcher, tree, handler, arguments, None);
}

/// Add the report's nodes with the same grammar on both sides. The authoritative
/// server also associates each runnable node ID with its registration path.
pub(in crate::game) fn register_command_nodes(
    dispatcher: &mut Dispatcher,
    tree: &Value,
    handler: &'static str,
    arguments: &CommandArguments,
    executors: Option<&CommandExecutorSlots>,
) {
    fn add(
        dispatcher: &mut Dispatcher,
        parent: usize,
        children: &Map<String, Value>,
        handler: &'static str,
        arguments: &CommandArguments,
        path: &mut Vec<String>,
        on_executable: &mut impl FnMut(&[String], usize),
    ) {
        for (name, node) in children {
            path.push(name.clone());
            let before = dispatcher.nodes.len();
            let id = if node["type"] == "literal" {
                dispatcher.literal(parent, name)
            } else {
                dispatcher.argument(parent, name, argument_type(node, arguments))
            };
            // `CommandNode.addChild` merges into a node vanilla already has, which keeps its
            // own requirement; a node the mod adds takes the usage's level.
            if dispatcher.nodes.len() > before {
                let level = node["permissions"]["permission"]["level"]
                    .as_str()
                    .and_then(|level| LEVELS.iter().position(|known| *known == level))
                    .unwrap_or(0) as u8;
                dispatcher.requires(id, level);
            }
            if node["executable"] == true {
                dispatcher.executes(id, handler);
                dispatcher.requires_execution(
                    id,
                    node["execution_level"]
                        .as_u64()
                        .expect("runnable usage level") as u8,
                );
                on_executable(path, id);
            }
            if let Some(children) = node["children"].as_object() {
                add(
                    dispatcher,
                    id,
                    children,
                    handler,
                    arguments,
                    path,
                    on_executable,
                );
            }
            path.pop();
        }
    }
    let mut bindings = HashMap::new();
    if let Some(children) = tree["children"].as_object() {
        add(
            dispatcher,
            0,
            children,
            handler,
            arguments,
            &mut Vec::new(),
            &mut |path: &[String], id| {
                if let Some(executors) = executors {
                    bindings.insert(id, executors[path]);
                }
            },
        );
    }
    if executors.is_some() {
        dispatcher.replace_executor_bindings(bindings);
    }
}

/// The live player body, or a guest record outside a player swap.
pub(in crate::game) enum PlayerRead<'a> {
    Acting {
        /// Server authoritative mode, or the local player's resolved `PlayerInfo` mode on a client.
        mode: Option<i32>,
        body: Option<&'a PlayerMotion>,
        /// The player's stored abilities, when it is a server player's turn.
        abilities: Option<PlayerAbilities>,
        /// Where the player last died: the acting guest's record, swapped
        /// into the level for their turn.
        last_death: Option<&'a (String, [i32; 3])>,
    },
    Guest(&'a Guest),
}

impl PlayerRead<'_> {
    pub(in crate::game) fn last_death(&self) -> Option<&(String, [i32; 3])> {
        match self {
            Self::Acting { last_death, .. } => *last_death,
            Self::Guest(guest) => guest.last_death.as_ref(),
        }
    }

    pub(in crate::game) fn mode(&self) -> Option<i32> {
        match self {
            Self::Acting { mode, .. } => *mode,
            Self::Guest(guest) => Some(guest.game_mode),
        }
    }

    pub(in crate::game) fn creative(&self) -> bool {
        self.mode() == Some(1)
    }

    pub(in crate::game) fn may_build(&self) -> bool {
        match self {
            Self::Acting {
                abilities: Some(a), ..
            } => a.may_build,
            Self::Guest(guest) => guest.abilities.may_build,
            Self::Acting { mode, .. } => mode.is_some_and(|m| m < 2),
        }
    }

    pub(in crate::game) fn body(&self) -> Option<&PlayerMotion> {
        match self {
            Self::Acting { body, .. } => *body,
            Self::Guest(guest) => guest.motion.as_deref(),
        }
    }

    pub(in crate::game) fn food(&self) -> Option<PlayerFood> {
        let food = &self.body()?.health.food;
        let (level, saturation) = match self {
            Self::Acting { .. } => (food.food, food.saturation),
            Self::Guest(guest) => (guest.food, guest.saturation),
        };
        Some(PlayerFood::from(mod_api::__private::built::PlayerFood {
            level,
            saturation,
            exhaustion: food.exhaustion,
        }))
    }

    pub(in crate::game) fn abilities(&self) -> Option<mod_api::PlayerAbilities> {
        let a = match self {
            Self::Acting {
                abilities: Some(a), ..
            } => *a,
            Self::Acting { mode, body, .. } => {
                // No server turn: what the mode gives, with the body's flight.
                let body = body.as_ref()?;
                let mut a = PlayerAbilities {
                    flying: body.flying,
                    flying_speed: body.flying_speed,
                    ..Default::default()
                };
                a.update_for_mode(mode.unwrap_or(0));
                a.flying = body.flying;
                a
            }
            Self::Guest(guest) => guest.abilities,
        };
        Some(mod_api::PlayerAbilities::from(
            mod_api::__private::built::PlayerAbilities {
                invulnerable: a.invulnerable,
                flying: a.flying,
                may_fly: a.may_fly,
                instabuild: a.instabuild,
                may_build: a.may_build,
                fly_speed: a.flying_speed,
                walk_speed: a.walking_speed,
            },
        ))
    }
}

/// Helpers for mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
#[allow(dead_code)]
impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// Resolve a real online player by UUID, never a fake.
    pub(in crate::game) fn player(&self, uuid: u128) -> Option<PlayerRef> {
        self.side_player(uuid)
    }

    /// The dimension id and block of the player's last death, through the shared player resolver.
    pub(in crate::game) fn native_player_last_death_record(
        &self,
        player: PlayerRef,
    ) -> Option<(String, [i32; 3])> {
        self.player_read_both(player)?.last_death().cloned()
    }

    /// `player_handle_read` for the reads that work on both sides.
    pub(in crate::game) fn player_read_both(&self, player: PlayerRef) -> Option<PlayerRead<'_>> {
        self.side_player_read_both(player)
    }

    /// The dimension key of an online player (`ServerPlayer.level().dimension()`).
    pub(in crate::game) fn player_dimension_key(
        &self,
        player: PlayerRef,
    ) -> Option<mod_api::DimensionKey> {
        self.side_player_dimension_key(player)
    }

    pub(in crate::game) fn player_collision_flags(
        &self,
        player: PlayerRef,
    ) -> Result<Option<mod_api::CollisionFlags>, mod_api::LevelReadError> {
        self.side_player_collision_flags(player)
    }

    pub(in crate::game) fn player_supporting_block(
        &self,
        player: PlayerRef,
    ) -> Result<Option<mod_api::BlockPos>, mod_api::LevelReadError> {
        Ok(self
            .player_read_both(player)
            .and_then(|read| read.body()?.ground.supporting_block))
    }

    pub(in crate::game) fn player_on_block(
        &self,
        player: PlayerRef,
    ) -> Result<Option<mod_api::BlockPos>, mod_api::LevelReadError> {
        Ok(self.player_read_both(player).and_then(|read| {
            let body = read.body();
            let position = match read {
                PlayerRead::Acting { .. } => body?.position,
                PlayerRead::Guest(guest) => {
                    let [x, y, z] = guest.position;
                    sunrise_core::geometry::Vec3::new(x, y, z)
                }
            };
            let [x, y, z] = super::native_entity_motion::on_block_position(
                body.and_then(|motion| motion.ground.supporting_block),
                position,
            );
            Some(mod_api::BlockPos::new(x, y, z))
        }))
    }

    /// `ServerPlayerGameMode.getGameModeForPlayer` on a server; `AbstractClientPlayer.gameMode` on a client.
    pub(in crate::game) fn player_game_mode(&self, player: PlayerRef) -> Option<GameMode> {
        self.player_read_both(player)?
            .mode()
            .and_then(GameMode::from_id)
    }

    /// `Player.isCreative`; Paper `HumanEntity#getGameMode`.
    pub(in crate::game) fn player_is_creative(&self, player: PlayerRef) -> Option<bool> {
        Some(self.player_read_both(player)?.creative())
    }

    /// `Inventory.getSelectedSlot`: the hotbar slot the player holds, 0..9.
    pub(in crate::game) fn player_selected_slot(&self, player: PlayerRef) -> Option<usize> {
        Some(match self.player_read_both(player)? {
            PlayerRead::Acting { .. } => self.selected,
            PlayerRead::Guest(guest) => guest.selected,
        })
    }

    /// `Player.isSpectator`.
    pub(in crate::game) fn player_is_spectator(&self, player: PlayerRef) -> Option<bool> {
        Some(self.player_read_both(player)?.mode() == Some(3))
    }

    /// `Player.mayBuild`; Paper `HumanEntity#getGameMode`.
    pub(in crate::game) fn player_may_build(&self, player: PlayerRef) -> Option<bool> {
        Some(self.player_read_both(player)?.may_build())
    }

    /// `FoodData.getFoodLevel/getSaturationLevel/getExhaustionLevel`.
    pub(in crate::game) fn player_food(&self, player: PlayerRef) -> Option<PlayerFood> {
        self.player_read_both(player)?.food()
    }

    /// The level the online player is in (`ServerPlayer.level`).
    pub(in crate::game) fn player_level(&self, player: PlayerRef) -> Option<LevelRef> {
        let dim = self.player_dimension_key(player)?;
        Some(self.side_level_ref(player.fields().entity.epoch(), dim))
    }

    /// The dimension the online player is in (`ServerPlayer.level().dimension()`).
    pub(in crate::game) fn player_dimension(&self, player: PlayerRef) -> Option<Identifier> {
        let level = self.player_level(player)?;
        Identifier::parse(level.dimension_id()).ok()
    }

    /// The brand the player's client announced when it connected (`ServerPlayer.clientBrand`).
    pub(in crate::game) fn player_client_brand(&self, player: PlayerRef) -> Option<&str> {
        self.side_player_client_brand(player)
    }

    /// The server's tick rate and average tick duration in milliseconds.
    pub(in crate::game) fn tick_stats(&self) -> (f32, f64) {
        (
            self.clock.rate(),
            self.clock.average_nanos() as f64 / 1_000_000.0,
        )
    }

    /// `TickRateManager.isFrozen`; None on a client, which follows its server's.
    pub(in crate::game) fn tick_rate_is_frozen(&self) -> Option<bool> {
        self.is_logical_server().then(|| self.clock.frozen())
    }
}

/// Whether an orb at `position` (its box: 0.5 wide and high, standing on
/// it) meets the box `min`..`max`, touching faces excluded (`AABB.intersects`).
pub(in crate::game) fn orb_meets(position: [f64; 3], min: [f64; 3], max: [f64; 3]) -> bool {
    position[0] - 0.25 < max[0]
        && position[0] + 0.25 > min[0]
        && position[1] < max[1]
        && position[1] + 0.5 > min[1]
        && position[2] - 0.25 < max[2]
        && position[2] + 0.25 > min[2]
}

/// The box `min`..`max` with each axis's pair in order, as vanilla's `AABB` takes it;
/// None when a bound is not finite (NaN or infinite).
pub(in crate::game) fn ordered_box(min: [f64; 3], max: [f64; 3]) -> Option<([f64; 3], [f64; 3])> {
    if !min.iter().chain(&max).all(|v| v.is_finite()) {
        return None;
    }
    Some((
        std::array::from_fn(|i| min[i].min(max[i])),
        std::array::from_fn(|i| min[i].max(max[i])),
    ))
}

pub(in crate::game) fn system_chat(line: &Text) -> C {
    C::SystemChat(play::SystemChat {
        content: component_nbt(&line.component()),
        overlay: false,
    })
}

/// The temporary state of the same persistent guest, not another player or action engine.
/// Swapped back after the ordinary guest action has restored the server's outer state.
pub(in crate::game) struct ScopedPlayerState {
    pub(in crate::game) motion: Option<Box<PlayerMotion>>,
    pub(in crate::game) position: [f64; 3],
    pub(in crate::game) yaw: f32,
    pub(in crate::game) pitch: f32,
    pub(in crate::game) head_yaw: f32,
    pub(in crate::game) on_ground: bool,
    pub(in crate::game) game_mode: i32,
    pub(in crate::game) previous_game_mode: Option<i32>,
    pub(in crate::game) abilities: PlayerAbilities,
    pub(in crate::game) sneaking: bool,
    pub(in crate::game) sprinting: bool,
    pub(in crate::game) fall_flying: bool,
    pub(in crate::game) inventory: Vec<Option<sunrise_items::items::ItemStack>>,
    pub(in crate::game) selected: usize,
    pub(in crate::game) armor: [Option<sunrise_items::items::ItemStack>; 4],
    pub(in crate::game) offhand: Option<sunrise_items::items::ItemStack>,
    pub(in crate::game) carried: Option<sunrise_items::items::ItemStack>,
    pub(in crate::game) equipment: sunrise_items::equipment::Equipment,
    pub(in crate::game) effects: sunrise_client::effects::PlayerEffects,
    pub(in crate::game) health: f32,
    pub(in crate::game) food: i32,
    pub(in crate::game) saturation: f32,
}
impl ScopedPlayerState {
    pub(in crate::game) fn new(
        resources: &sunrise_client::motion::MotionResources,
        items: &sunrise_items::items::Items,
        setup: PlayerActionSetup,
    ) -> Result<Self, ActionRefused> {
        // Standing eye height even for secondary use, just as a freshly positioned fake.
        let position = [
            setup.eye[0],
            setup.eye[1] - f64::from(1.62_f32),
            setup.eye[2],
        ];
        let mut motion = PlayerMotion::new(
            resources,
            sunrise_core::geometry::Vec3::new(position[0], position[1], position[2]),
        )
        .map_err(|_| ActionRefused::InvalidSetup)?;
        motion.shift_key_down = setup.secondary_use;
        let mut inventory = vec![None; 36];
        inventory[0] = setup
            .stack
            .try_into_engine(items)
            .map_err(ActionRefused::InvalidStack)?;
        Ok(Self {
            motion: Some(Box::new(motion)),
            position,
            yaw: setup.yaw,
            pitch: setup.pitch,
            head_yaw: setup.yaw,
            on_ground: false,
            game_mode: 0,
            previous_game_mode: None,
            abilities: PlayerAbilities::default(),
            sneaking: setup.secondary_use,
            sprinting: false,
            fall_flying: false,
            inventory,
            selected: 0,
            armor: Default::default(),
            offhand: None,
            carried: None,
            equipment: Default::default(),
            effects: Default::default(),
            health: 20.0,
            food: 20,
            saturation: 5.0,
        })
    }

    pub(in crate::game) fn swap(&mut self, guest: &mut Guest) {
        std::mem::swap(&mut self.motion, &mut guest.motion);
        std::mem::swap(&mut self.position, &mut guest.position);
        std::mem::swap(&mut self.yaw, &mut guest.yaw);
        std::mem::swap(&mut self.pitch, &mut guest.pitch);
        std::mem::swap(&mut self.head_yaw, &mut guest.head_yaw);
        std::mem::swap(&mut self.on_ground, &mut guest.on_ground);
        std::mem::swap(&mut self.game_mode, &mut guest.game_mode);
        std::mem::swap(&mut self.previous_game_mode, &mut guest.previous_game_mode);
        std::mem::swap(&mut self.abilities, &mut guest.abilities);
        std::mem::swap(&mut self.sneaking, &mut guest.sneaking);
        std::mem::swap(&mut self.sprinting, &mut guest.sprinting);
        std::mem::swap(&mut self.fall_flying, &mut guest.fall_flying);
        std::mem::swap(&mut self.inventory, &mut guest.inventory);
        std::mem::swap(&mut self.selected, &mut guest.selected);
        std::mem::swap(&mut self.armor, &mut guest.armor);
        std::mem::swap(&mut self.offhand, &mut guest.offhand);
        std::mem::swap(&mut self.carried, &mut guest.carried);
        std::mem::swap(&mut self.equipment, &mut guest.equipment);
        std::mem::swap(&mut self.effects, &mut guest.effects);
        std::mem::swap(&mut self.health, &mut guest.health);
        std::mem::swap(&mut self.food, &mut guest.food);
        std::mem::swap(&mut self.saturation, &mut guest.saturation);
    }
}

/// A fake player's `perform_action` in progress: set for its length by [`Acting`], whose drop
/// clears it on every exit, an early return or a panic unwinding through a mod's hook, and the
/// effect it stopped, cleared as each action starts.
#[derive(Default)]
pub(in crate::game) struct FakeAction {
    pub(in crate::game) acting: AtomicBool,
    /// The first effect stopped since the action started.
    pub(in crate::game) blocked: Mutex<Option<FakeEffect>>,
}
impl FakeAction {
    /// Whether a fake player's action is running.
    pub(in crate::game) fn acting(&self) -> bool {
        self.acting.load(Ordering::Relaxed)
    }
    pub(in crate::game) fn block(&self, effect: FakeEffect) {
        // The first effect stopped is the one the action answers.
        self.blocked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert(effect);
    }
}
/// Marks a fake player's action as running until it drops.
pub(in crate::game) struct Acting(pub(in crate::game) Arc<FakeAction>);
impl Acting {
    pub(in crate::game) fn start(action: &Arc<FakeAction>) -> Self {
        *action
            .blocked
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        action.acting.store(true, Ordering::Relaxed);
        Self(Arc::clone(action))
    }
    /// The effect this action stopped, if any.
    pub(in crate::game) fn blocked(&self) -> Option<FakeEffect> {
        *self
            .0
            .blocked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}
impl Drop for Acting {
    fn drop(&mut self) {
        self.0.acting.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use sunrise_client::items::ItemStack;

    /// Box bounds come from mod code: one that is not finite reads no orb and never
    /// panics, and an inverted box reads what the ordered one does.
    #[test]
    #[ignore = "daily"]
    fn experience_orbs_refuse_bounds_that_are_not_finite_and_swap_an_inverted_box() {
        let mut resources = None;
        let mut game = crate::game::game_tests::tests::server_game(&mut resources);
        game.options.dedicated = Some(Vec::new());
        let at = sunrise_core::geometry::Vec3::new(1.0, 70.0, 1.0);
        game.orbs.push(sunrise_client::entities::ExperienceOrb {
            size_answer: None,
            attachment_lifetime: Default::default(),
            fall_distance: 0.0,
            rotation: [0.0, 0.0],
            join: Default::default(),
            id: 40,
            position: at,
            previous: at,
            velocity: sunrise_core::geometry::Vec3::ZERO,
            value: 3,
            age: 0,
            ground: sunrise_world::ground_support::GroundState::from_ground_flag(true),
            count: 1,
            fire_ticks: 0,
            ticks: 0,
            health: 5,
            air: 300,
            no_gravity: true,
            following: None,
        });
        let dimension = game.dim.id().to_owned();
        let ids = |game: &crate::game::ServerGame<'_>, min, max| -> Vec<u32> {
            game.experience_orbs(&dimension, min, max)
                .into_iter()
                .map(|orb| orb.id)
                .collect()
        };
        assert_eq!(ids(&game, [0.0, 69.0, 0.0], [2.0, 71.0, 2.0]), [40]);
        for (min, max) in [
            ([f64::NAN, 69.0, 0.0], [2.0, 71.0, 2.0]),
            ([0.0, 69.0, 0.0], [2.0, f64::NAN, 2.0]),
            ([f64::NEG_INFINITY, 69.0, 0.0], [2.0, 71.0, 2.0]),
            ([0.0, 69.0, 0.0], [2.0, 71.0, f64::INFINITY]),
            ([f64::NEG_INFINITY; 3], [f64::INFINITY; 3]),
        ] {
            assert_eq!(ids(&game, min, max), Vec::<u32>::new(), "{min:?} {max:?}");
        }
        assert_eq!(ids(&game, [2.0, 69.0, 0.0], [0.0, 71.0, 2.0]), [40]);
        assert_eq!(ids(&game, [2.0, 71.0, 2.0], [0.0, 69.0, 0.0]), [40]);
    }

    /// A mod's `announce` reaches the dedicated server's event stream and nothing else's.
    /// The integrated server thread keeps the launch configuration (`options.dedicated`) too,
    /// so it cannot be what decides.
    #[test]
    #[ignore = "daily"]
    fn server_events_are_posted_only_on_the_dedicated_role() {
        use crate::game::execution_role::ExecutionRole;
        let mut resources = None;
        let mut game = crate::game::game_tests::tests::server_game(&mut resources);
        game.options.dedicated = Some(Vec::new());
        for (role, posted) in [
            (ExecutionRole::DedicatedServer, true),
            (ExecutionRole::IntegratedServer, false),
            (ExecutionRole::Client, false),
        ] {
            game.options.execution_role = role;
            assert_eq!(
                game.native_server_event("Alex", 7, "hello"),
                posted,
                "{role:?}"
            );
        }
    }

    pub(in crate::game) fn with_command_sources(
        test: impl FnOnce(&mut crate::game::ServerGame<'_>),
    ) {
        use sunrise_client::{
            chunk::Chunk, chunk::block_entity::BlockEntity, command_block::CommandBlock,
        };
        use sunrise_core::geometry::Vec3;
        super::super::native_guest_turn_tests::with_guests(|game| {
            game.build_registries();
            game.register_native_commands();
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let state = game
                .pack
                .assets
                .registry
                .default_state("minecraft:command_block")
                .unwrap();
            game.world.set(1, 64, 1, state).unwrap();
            game.world.block_entities.insert(
                (1, 64, 1),
                BlockEntity::CommandBlock(Box::new(CommandBlock {
                    command: "execute in minecraft:the_nether run origin".into(),
                    condition_met: true,
                    ..Default::default()
                })),
            );
            let mut cart = super::super::minecarts::Minecart::new(
                99,
                super::super::minecarts::CartKind::CommandBlock,
                Vec3::new(1.0, 64.0, 1.0),
            );
            cart.command.command = "origin".into();
            game.carts.push(cart);
            test(game);
        });
    }

    pub(in crate::game) type Heard = Vec<(&'static str, u32, u128, &'static str, [i32; 2])>;
    /// Logs each of the watch trio: (hook, player id, UUID, level, chunk).
    pub(in crate::game) struct Watcher(pub(in crate::game) std::sync::Arc<std::sync::Mutex<Heard>>);
    impl Watcher {
        pub(in crate::game) fn log(
            &self,
            hook: &'static str,
            player: PlayerRef,
            chunk: mod_api::ChunkRef,
        ) {
            self.0.lock().unwrap().push((
                hook,
                player.entity().id(),
                player.uuid(),
                chunk.level().dimension_id(),
                chunk.pos(),
            ));
        }
    }
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Watcher {
        fn chunk_watch(&self, _: &mut dyn mod_api::ServerHost, e: &mut mod_api::ChunkWatchEvent) {
            self.log("watch", e.player, e.chunk);
        }
        fn chunk_sent(&self, _: &mut dyn mod_api::ServerHost, e: &mut mod_api::ChunkSentEvent) {
            self.log("sent", e.player, e.chunk);
        }
        fn chunk_unwatch(
            &self,
            _: &mut dyn mod_api::ServerHost,
            e: &mut mod_api::ChunkUnwatchEvent,
        ) {
            self.log("unwatch", e.player, e.chunk);
        }
    }

    /// `ChunkWatchEvent`: the hub's records reach the mods in order, each
    /// with the player (entity id and UUID, even after they left) and the
    /// chunk in its level; a level the server doesn't hold is skipped.
    #[test]
    #[ignore = "daily"]
    fn the_hubs_chunk_watches_reach_the_mods_in_order() {
        use sunrise_client::lan::chunk_watch::{
            ChunkWatch, ChunkWatchKind, ChunkWatchLog, LevelWatches,
        };
        let heard = std::sync::Arc::default();
        let mods = vec![mod_api::ModEntry::new(
            "watch",
            Box::new(Watcher(std::sync::Arc::clone(&heard))),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_game(|game| {
                super::super::native_item_places::tests::open_to_lan(game);
                let here = game.dim.id();
                let watch = |kind, chunk| ChunkWatch {
                    kind,
                    uuid: 127,
                    entity_id: 42,
                    chunk,
                };
                let run = |dimension: &str, watches| LevelWatches {
                    dimension: dimension.into(),
                    watches,
                };
                game.lan.as_mut().unwrap().hub.chunk_watches = ChunkWatchLog::new(vec![
                    run(
                        here,
                        vec![
                            watch(ChunkWatchKind::Watch, [1, 2]),
                            watch(ChunkWatchKind::Sent, [1, 2]),
                        ],
                    ),
                    run(
                        "minecraft:nowhere",
                        vec![watch(ChunkWatchKind::Unwatch, [0, 0])],
                    ),
                    run(here, vec![watch(ChunkWatchKind::Unwatch, [1, 2])]),
                ]);
                game.run_chunk_watches();
                assert!(game.lan.as_ref().unwrap().hub.chunk_watches.is_empty());
                assert_eq!(
                    *heard.lock().unwrap(),
                    [
                        ("watch", 42, 127, here, [1, 2]),
                        ("sent", 42, 127, here, [1, 2]),
                        ("unwatch", 42, 127, here, [1, 2]),
                    ]
                );
            });
        });
    }

    pub(in crate::game) fn player_guest() -> Guest {
        use std::sync::{
            Arc,
            atomic::{AtomicI32, AtomicUsize},
        };
        use sunrise_client::lan::{Joined, PlayerSender};
        use sunrise_protocol::packets::configuration::ClientInformation;
        let (tx, _rx) = std::sync::mpsc::channel();
        Guest::new(
            Joined {
                conn: 1,
                uuid: 123,
                name: "guest".into(),
                properties: Vec::new(),
                info: ClientInformation::new(8),
                brand: String::new(),
                sender: PlayerSender::new(tx, Arc::new(AtomicUsize::new(0))),
                latency: Arc::new(AtomicI32::new(0)),
                address: "127.0.0.1:1".parse().unwrap(),
                expected: false,
            },
            1,
            [0.0; 3],
            10,
        )
    }

    #[test]
    fn player_reads_use_guest_mode_and_abilities() {
        let mut guest = player_guest();
        guest.abilities.flying_speed = 0.08;
        let read = PlayerRead::Guest(&guest);
        assert_eq!(read.mode(), Some(0));
        assert_eq!(
            read.food(),
            None,
            "food is unavailable before body creation"
        );
        assert_eq!(
            read.abilities(),
            Some(mod_api::PlayerAbilities::from(
                mod_api::__private::built::PlayerAbilities {
                    invulnerable: false,
                    flying: false,
                    may_fly: false,
                    instabuild: false,
                    may_build: true,
                    fly_speed: 0.08,
                    walk_speed: 0.1,
                }
            ))
        );
        guest.game_mode = 1;
        guest.abilities.update_for_mode(1);
        guest.abilities.flying = true;
        let read = PlayerRead::Guest(&guest);
        assert_eq!(read.mode(), Some(1));
        assert_eq!(
            read.abilities(),
            Some(mod_api::PlayerAbilities::from(
                mod_api::__private::built::PlayerAbilities {
                    invulnerable: true,
                    flying: true,
                    may_fly: true,
                    instabuild: true,
                    may_build: true,
                    fly_speed: 0.08,
                    walk_speed: 0.1,
                }
            ))
        );
        guest.game_mode = 3;
        guest.abilities.update_for_mode(3);
        let abilities = PlayerRead::Guest(&guest).abilities().unwrap();
        assert!(!abilities.may_build && !abilities.instabuild && abilities.may_fly);
    }

    pub(in crate::game) fn tree(
        commands: &[mod_api::Command],
    ) -> (Value, HashMap<String, u8>, Vec<String>) {
        command_tree(commands, &Dispatcher::new(), &HashMap::new())
    }

    pub(in crate::game) fn stack(item: &str, count: u32) -> Option<ItemStack> {
        Some(ItemStack {
            item: item.into(),
            count,
            damage: 0,
            extra: None,
        })
    }

    #[test]
    fn orbs_are_found_by_their_half_block_box() {
        // A block's cell, as a mod asks for a broken block's drops.
        let (min, max) = ([10.0, 64.0, -3.0], [11.0, 65.0, -2.0]);
        assert!(orb_meets([10.5, 64.2, -2.5], min, max), "inside");
        assert!(
            orb_meets([11.2, 64.0, -2.5], min, max),
            "its box reaches in"
        );
        assert!(
            orb_meets([10.5, 63.6, -2.5], min, max),
            "standing just below"
        );
        assert!(!orb_meets([11.25, 64.0, -2.5], min, max), "a face touching");
        assert!(!orb_meets([10.5, 63.5, -2.5], min, max), "its top touching");
        assert!(!orb_meets([10.5, 65.0, -2.5], min, max), "standing on top");
    }

    #[test]
    fn whole_stacks_are_given_onto_equal_stacks_only() {
        use super::super::inventory_add;
        let mut inv = vec![stack("minecraft:dirt", 64); 36];
        inv[2] = stack("minecraft:iron_sword", 1);
        inv[2].as_mut().unwrap().damage = 5;
        inv[5] = stack("minecraft:arrow", 60);
        inv[7] = None;
        inv[20] = None;
        let mut off = stack("minecraft:arrow", 62);

        // A damaged sword never joins another stack: the first free slot.
        let mut sword = stack("minecraft:iron_sword", 1).unwrap();
        sword.damage = 40;
        assert!(!inventory_add(&mut inv, &mut off, 5, 1, &mut sword));
        assert_eq!(inv[7].as_ref().unwrap().damage, 40, "its damage is kept");
        assert_eq!(inv[2].as_ref().unwrap().damage, 5);

        // Arrows: the selected slot, then the off hand, then a free slot,
        // and what is left over comes back.
        let mut arrows = stack("minecraft:arrow", 80).unwrap();
        assert!(inventory_add(&mut inv, &mut off, 5, 64, &mut arrows));
        assert_eq!(inv[5].as_ref().unwrap().count, 64);
        assert_eq!(off.as_ref().unwrap().count, 64);
        assert_eq!(inv[20].as_ref().unwrap().count, 64);
        assert_eq!(arrows.count, 80 - 4 - 2 - 64);

        // A full inventory with an empty off hand takes nothing.
        let mut off = None;
        let mut apple = stack("minecraft:apple", 3).unwrap();
        assert!(inventory_add(&mut inv, &mut off, 0, 64, &mut apple));
        assert!(off.is_none(), "never into an empty off hand (getFreeSlot)");
        assert_eq!(apple.count, 3);
    }

    /// The game's `Inventory.add` (pickups, `/give`, a bucket's or a bottle's result) takes
    /// its slots from `getSlotWithRemainingSpace` and `getFreeSlot`
    /// (`sunrise_client::menu::{slot_with_remaining_space, free_slot}`): one dirt at a time
    /// lands where they answer and nowhere else, in vanilla's order (the selected slot, the
    /// off hand, the main slots, then the first empty one).
    #[test]
    fn every_added_item_lands_where_the_slot_pickers_answer() {
        use super::super::inventory_add;
        use sunrise_client::menu::{OFFHAND, free_slot, slot_with_remaining_space};
        let mut inv = vec![stack("minecraft:stone", 64); 36];
        for (slot, have) in [
            (0, stack("minecraft:stone", 5)),
            (1, None),
            (2, None),
            (3, stack("minecraft:dirt", 63)),
            (7, stack("minecraft:dirt", 63)),
            (20, None),
        ] {
            inv[slot] = have;
        }
        let mut off = stack("minecraft:dirt", 62);
        let selected = 3;
        let dirt = stack("minecraft:dirt", 1).unwrap();
        let mut landed = Vec::new();
        for _ in 0..6 {
            let answer = slot_with_remaining_space(&inv, off.as_ref(), selected, &dirt, 64)
                .or_else(|| free_slot(&inv))
                .unwrap();
            let (mut inv_after, mut off_after) = (inv.clone(), off.clone());
            let cell = if answer == OFFHAND {
                &mut off_after
            } else {
                &mut inv_after[answer]
            };
            match cell {
                Some(have) => have.count += 1,
                empty => *empty = Some(dirt.clone()),
            }
            let mut one = dirt.clone();
            assert!(!inventory_add(&mut inv, &mut off, selected, 64, &mut one));
            assert_eq!((&inv, &off), (&inv_after, &off_after), "into {answer}");
            landed.push(answer);
        }
        assert_eq!(landed, [3, OFFHAND, OFFHAND, 7, 1, 1]);
    }

    /// A mod's rows that keep hotbar slots 6 to 8 free change the slot the game's
    /// `Inventory.add` takes (replaced on this thread through `rule_replacements::ADD_SLOT`):
    /// a dirt passes the stack in slot 6 with room and the selected slot 7 for slot 0.
    #[test]
    fn a_replaced_slot_choice_reaches_the_games_inventory_add() {
        use super::super::inventory_add;
        use super::super::rule_replacements::{ADD_SLOT, replaced};
        use sunrise_client::menu::OFFHAND;
        fn open(i: usize) -> bool {
            !(6..=8).contains(&i)
        }
        fn locked(
            items: &[Option<ItemStack>],
            offhand: Option<&ItemStack>,
            selected: usize,
            stack: &ItemStack,
            max: u32,
        ) -> Option<usize> {
            let has = |have: Option<&ItemStack>| {
                have.is_some_and(|have| have.same_item_same_components(stack) && have.count < max)
            };
            if open(selected) && has(items[selected].as_ref()) {
                Some(selected)
            } else if has(offhand) {
                Some(OFFHAND)
            } else {
                (0..items.len())
                    .find(|&i| open(i) && has(items[i].as_ref()))
                    .or_else(|| (0..items.len()).find(|&i| open(i) && items[i].is_none()))
            }
        }
        let mut inv = vec![None; 36];
        inv[6] = stack("minecraft:dirt", 10);
        let mut vanilla = inv.clone();
        let mut off = None;
        let mut one = stack("minecraft:dirt", 1).unwrap();
        let left = replaced(&ADD_SLOT, locked, || {
            inventory_add(&mut inv, &mut off, 7, 64, &mut one)
        });
        assert!(!left);
        assert_eq!(
            (&inv[0], &inv[6], &inv[7]),
            (
                &stack("minecraft:dirt", 1),
                &stack("minecraft:dirt", 10),
                &None
            )
        );
        // Without the rows, vanilla's slot: the stack in slot 6 with room.
        let mut one = stack("minecraft:dirt", 1).unwrap();
        assert!(!inventory_add(&mut vanilla, &mut off, 7, 64, &mut one));
        assert_eq!(vanilla[6], stack("minecraft:dirt", 11));
    }

    #[test]
    fn usages_merge_into_one_tree_with_runnable_nodes() {
        let (t, levels, problems) = tree(&[
            mod_api::Command::new("balance [player:player]"),
            mod_api::Command::new("pay <player:player> <amount:integer>"),
            mod_api::Command::op("eco give <player:player> <amount:integer>", 3),
            mod_api::Command::op("eco take <player:player> <amount:integer>", 3),
        ]);
        assert!(problems.is_empty(), "{problems:?}");
        let c = &t["children"];
        assert_eq!(c["balance"]["executable"], true);
        assert_eq!(c["balance"]["children"]["player"]["executable"], true);
        assert_eq!(
            c["balance"]["children"]["player"]["parser"],
            "minecraft:entity"
        );
        assert_eq!(c["pay"]["executable"], false);
        assert_eq!(c["pay"]["children"]["player"]["executable"], false);
        let amount = &c["pay"]["children"]["player"]["children"]["amount"];
        assert_eq!(amount["executable"], true);
        assert_eq!(amount["parser"], "brigadier:integer");
        assert!(amount.get("children").is_none());
        assert_eq!(c["eco"]["permissions"]["permission"]["level"], "admins");
        assert!(c["eco"]["children"]["give"].is_object());
        assert!(c["eco"]["children"]["take"].is_object());
        assert!(c["pay"].get("permissions").is_none());
        assert_eq!(levels["eco"], 3);
        assert_eq!(levels["pay"], 0);
    }

    #[test]
    fn a_usage_with_another_string_kind_is_left_out_in_either_order() {
        for (first, second, line, other) in [
            (
                "foo <value:word>",
                "foo <value:text>",
                "foo hello",
                "foo hello world",
            ),
            (
                "foo <value:text>",
                "foo <value:word>",
                "foo hello world",
                "foo hello",
            ),
        ] {
            let (tree, _, problems) =
                tree(&[mod_api::Command::new(first), mod_api::Command::new(second)]);
            assert_eq!(problems.len(), 1, "{first} then {second}: {problems:?}");
            assert!(problems[0].starts_with(&format!("`{second}`")));
            let mut dispatcher = Dispatcher::new();
            register_dispatcher(
                &mut dispatcher,
                &tree,
                "native_mod_command",
                &HashMap::new(),
            );
            let registries = sunrise_client::command::Registries::default();
            assert!(dispatcher.parse(line, &registries).executable().is_ok());
            // The kept kind decides: `word` stops at the space, `text` takes it all.
            let kept_greedy = first.contains(":text");
            assert_eq!(
                dispatcher.parse(other, &registries).executable().is_ok(),
                kept_greedy,
                "{first} then {second}: {other}"
            );
        }
    }

    pub(in crate::game) const POSITIONED: &str = "kinds <at:vec3> <by:vec3_uncentered> <column:column_pos> \
         <look:rotation> <level:dimension> <tag:nbt>";

    #[test]
    fn position_rotation_dimension_and_nbt_kinds_are_vanillas_parsers() {
        let (tree, _, problems) = tree(&[mod_api::Command::new(POSITIONED)]);
        assert!(problems.is_empty(), "{problems:?}");
        let mut node = &tree["children"]["kinds"];
        for (name, parser) in [
            ("at", "minecraft:vec3"),
            ("by", "minecraft:vec3"),
            ("column", "minecraft:column_pos"),
            ("look", "minecraft:rotation"),
            ("level", "minecraft:dimension"),
            ("tag", "minecraft:nbt_compound_tag"),
        ] {
            node = &node["children"][name];
            assert_eq!(node["parser"], parser, "{name}");
            assert_eq!(node["uncentered"] == true, name == "by", "{name}");
        }
    }

    #[test]
    fn vec3_and_vec3_uncentered_do_not_share_a_node() {
        let (_, _, problems) = tree(&[
            mod_api::Command::new("go <to:vec3>"),
            mod_api::Command::new("go <to:vec3_uncentered>"),
        ]);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("another kind"), "{problems:?}");
    }

    #[test]
    fn positions_and_rotations_resolve_against_the_source() {
        let (tree, _, problems) = tree(&[mod_api::Command::new(POSITIONED)]);
        assert!(problems.is_empty(), "{problems:?}");
        let mut dispatcher = Dispatcher::new();
        register_dispatcher(
            &mut dispatcher,
            &tree,
            "native_mod_command",
            &HashMap::new(),
        );
        let registries = sunrise_client::command::Registries::default();
        let (origin, rotation) = ([10.0, 70.0, -2.0], [45.0, 30.0]);
        let value = |line: &str, name: &str, kind: &str| {
            let parsed = dispatcher.parse(line, &registries);
            let (context, _) = parsed.executable().unwrap();
            let Some(Arg::Coords(coordinates)) = context.get(name) else {
                panic!("{name} is a position")
            };
            let range = context.args.iter().find(|a| a.name == name).unwrap().range;
            let ty = parsed_type(&dispatcher, &context.nodes, range);
            assert_eq!(
                format!("{ty:?}"),
                format!("{:?}", Some(parser(kind).unwrap().2)),
                "{name}: the type comes from the parsed node"
            );
            positional_value(
                ty.as_ref(),
                coordinates,
                coordinates.position(origin, rotation),
                rotation,
            )
        };
        let relative = "kinds ~1 ~ ~-1 ~1 ~ ~-1 ~3 ~-4 ~10 ~-5 minecraft:the_nether {}";
        assert_eq!(
            value(relative, "at", "vec3"),
            mod_api::CommandArgumentValue::Vec3([11.0, 70.0, -3.0])
        );
        assert_eq!(
            value(relative, "column", "column_pos"),
            mod_api::CommandArgumentValue::ColumnPos([13, -6])
        );
        assert_eq!(
            value(relative, "look", "rotation"),
            mod_api::CommandArgumentValue::Rotation([55.0, 25.0])
        );
        // A whole `x` and `z` is centred by `vec3` and kept by `vec3_uncentered`.
        let absolute = "kinds 5 64 -7 5 64 -7 0 0 0 0 minecraft:overworld {}";
        assert_eq!(
            value(absolute, "at", "vec3"),
            mod_api::CommandArgumentValue::Vec3([5.5, 64.0, -6.5])
        );
        assert_eq!(
            value(absolute, "by", "vec3_uncentered"),
            mod_api::CommandArgumentValue::Vec3([5.0, 64.0, -7.0])
        );
    }

    #[test]
    fn a_dimension_the_world_lacks_is_refused_and_an_nbt_argument_is_a_compound() {
        let refusal = dimension_value("minecraft:nowhere", |_| None).unwrap_err();
        assert_eq!(refusal.key, "argument.dimension.invalid");
        let mut expected = sunrise_core::nbt::Compound::new();
        expected.insert("answer".into(), sunrise_core::nbt::Tag::Byte(42));
        assert_eq!(
            nbt_value("{answer:42b}").unwrap(),
            mod_api::CommandArgumentValue::Nbt(expected)
        );
    }

    #[test]
    fn every_supported_kind_has_a_parser() {
        assert!(
            argument_kinds::KINDS
                .into_iter()
                .all(|kind| parser(kind).is_some())
        );
    }

    /// A test mod's argument type: named machines, one word, completed by prefix.
    pub(in crate::game) struct MachineArgument;

    impl mod_api::CommandArgumentType for MachineArgument {
        fn kind(&self) -> mod_api::CommandArgumentKind {
            mod_api::CommandArgumentKind::Word
        }

        fn parse(&self, input: &str) -> Result<String, mod_api::CommandArgumentError> {
            match input {
                "press" | "furnace" => Ok(input.to_owned()),
                _ => Err(mod_api::CommandArgumentError::new(
                    mod_api::Text::translate(
                        "test.commands.machine.unknown",
                        vec![mod_api::Text::literal(input)],
                    ),
                )),
            }
        }

        fn suggest(
            &self,
            _: Option<&mod_api::CommandSender>,
            prefix: &str,
        ) -> Vec<mod_api::CommandSuggestion> {
            ["press", "furnace"]
                .into_iter()
                .filter(|machine| machine.starts_with(prefix))
                .map(mod_api::CommandSuggestion::new)
                .collect()
        }
    }

    pub(in crate::game) struct HostileArgument;

    impl mod_api::CommandArgumentType for HostileArgument {
        fn kind(&self) -> mod_api::CommandArgumentKind {
            mod_api::CommandArgumentKind::Word
        }
        fn parse(&self, input: &str) -> Result<String, mod_api::CommandArgumentError> {
            if input == "panic" {
                panic!("parser")
            }
            Ok(input.into())
        }
        fn suggest(
            &self,
            _: Option<&mod_api::CommandSender>,
            prefix: &str,
        ) -> Vec<mod_api::CommandSuggestion> {
            match prefix {
                "panic" => panic!("completer"),
                "text" => vec![mod_api::CommandSuggestion::new("a".repeat(32768))],
                "tip" => vec![mod_api::CommandSuggestion::with_tooltip(
                    "ok",
                    "a".repeat(65536),
                )],
                "unicode" => vec![mod_api::CommandSuggestion::with_tooltip(
                    "ok",
                    "\0".repeat(32768),
                )],
                _ => vec![mod_api::CommandSuggestion::with_tooltip(
                    "a".repeat(32767),
                    "a".repeat(65535),
                )],
            }
        }
    }

    #[test]
    fn hostile_argument_callbacks_are_refused_without_losing_the_dispatcher() {
        let arguments: CommandArguments = HashMap::from([(
            "test:hostile".into(),
            Arc::new(HostileArgument) as Arc<dyn mod_api::CommandArgumentType>,
        )]);
        let (tree, _, problems) = super::command_tree(
            &[mod_api::Command::new("probe <value:test:hostile>")],
            &sunrise_client::command::Dispatcher::new(),
            &arguments,
        );
        assert!(problems.is_empty());
        let mut dispatcher = Dispatcher::new();
        register_dispatcher(&mut dispatcher, &tree, "native_mod_command", &arguments);
        let registries = sunrise_client::command::Registries::default();
        for prefix in ["panic", "text", "tip", "unicode"] {
            let input = format!("probe {prefix}");
            let parsed = dispatcher.parse(&input, &registries);
            assert_eq!(parsed.executable().is_err(), prefix == "panic");
            let (_, suggestions) = dispatcher.suggestions(&parsed, input.len(), &registries);
            assert!(suggestions.is_empty(), "{prefix}");
        }
        let input = "probe valid";
        let parsed = dispatcher.parse(input, &registries);
        let (_, suggestions) = dispatcher.suggestions(&parsed, input.len(), &registries);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].text.len(), 32767);
    }

    #[test]
    fn panicking_argument_kind_is_left_out_of_the_tree() {
        struct PanicKind;
        impl mod_api::CommandArgumentType for PanicKind {
            fn kind(&self) -> mod_api::CommandArgumentKind {
                panic!("kind")
            }
            fn parse(&self, _: &str) -> Result<String, mod_api::CommandArgumentError> {
                unreachable!()
            }
            fn suggest(
                &self,
                _: Option<&mod_api::CommandSender>,
                _: &str,
            ) -> Vec<mod_api::CommandSuggestion> {
                unreachable!()
            }
        }
        let arguments: CommandArguments = HashMap::from([(
            "test:panic".into(),
            Arc::new(PanicKind) as Arc<dyn mod_api::CommandArgumentType>,
        )]);
        let (_, _, problems) = super::command_tree(
            &[mod_api::Command::new("probe <value:test:panic>")],
            &sunrise_client::command::Dispatcher::new(),
            &arguments,
        );
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("panicked"));
    }

    pub(in crate::game) struct Machines;

    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Machines {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            r.register_command_argument("machine", MachineArgument);
        }
    }

    #[test]
    fn a_registered_argument_type_parses_and_completes_its_values() {
        mod_platform::__private::part_tests::install_parts_fixture(&[mod_api::ModEntry::new(
            "test",
            Box::new(Machines),
        )])
        .unwrap();
        let arguments = command_arguments();
        assert!(arguments.contains_key("test:machine"));

        let (tree, _, problems) = super::command_tree(
            &[mod_api::Command::new("machine use <machine:test:machine>")],
            &sunrise_client::command::Dispatcher::new(),
            &arguments,
        );
        assert!(problems.is_empty(), "{problems:?}");
        let machine = &tree["children"]["machine"]["children"]["use"]["children"]["machine"];
        assert_eq!(machine["custom_argument"], "test:machine");
        assert_eq!(machine["suggestions"], "minecraft:ask_server");
        assert_eq!(machine["properties"], json!({"type": "word"}));

        let mut dispatcher = Dispatcher::new();
        register_dispatcher(&mut dispatcher, &tree, "native_mod_command", &arguments);
        let registries = sunrise_client::command::Registries::default();
        let parsed = dispatcher.parse("machine use press", &registries);
        let (context, _) = parsed.executable().unwrap();
        assert_eq!(
            context.get("machine"),
            Some(&Arg::Custom {
                kind: "test:machine".into(),
                value: "press".into(),
            })
        );
        assert!(
            dispatcher
                .parse("machine use banana", &registries)
                .executable()
                .is_err(),
            "the mod's parser rejects what it does not know"
        );

        let typed = "machine use pr";
        let parse = dispatcher.parse(typed, &registries);
        let (_, suggestions) = dispatcher.suggestions(&parse, typed.len(), &registries);
        assert_eq!(
            suggestions
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>(),
            ["press"]
        );
    }

    #[test]
    fn the_id_kind_reads_a_free_identifier_with_no_registry() {
        let (tree, _, problems) = tree(&[mod_api::Command::new("map fullrender <world:id>")]);
        assert!(problems.is_empty(), "{problems:?}");
        let world = &tree["children"]["map"]["children"]["fullrender"]["children"]["world"];
        assert_eq!(world["parser"], "minecraft:resource_location");
        assert!(
            world.get("properties").is_none(),
            "ResourceLocationArgument carries no properties"
        );
        // The free `id` has no list of its own: its node asks the server, which answers from the
        // mods' own suggestions (`tab_complete`), so an unmodded client is not left with nothing.
        assert_eq!(world["suggestions"], "minecraft:ask_server");

        let mut dispatcher = Dispatcher::new();
        register_dispatcher(
            &mut dispatcher,
            &tree,
            "native_mod_command",
            &HashMap::new(),
        );
        let registries = sunrise_client::command::Registries::default();
        for (line, id) in [
            ("map fullrender minecraft:overworld", "minecraft:overworld"),
            ("map fullrender mymod:custom", "mymod:custom"),
            ("map fullrender overworld", "minecraft:overworld"),
        ] {
            let parsed = dispatcher.parse(line, &registries);
            let (context, _) = parsed.executable().unwrap_or_else(|e| {
                panic!("{line}: {}", e.message.plain());
            });
            assert_eq!(context.get("world"), Some(&Arg::Str(id.into())), "{line}");
        }
        for refused in [
            "map fullrender Minecraft:overworld",
            "map fullrender minecraft:overworld:again",
            "map fullrender 50%stone,50%dirt",
        ] {
            assert!(
                dispatcher.parse(refused, &registries).executable().is_err(),
                "{refused}"
            );
        }
    }

    /// A test mod's argument type for a WorldEdit-style pattern list: the greedy text holds
    /// several tokens, and the completion is for the one being typed.
    pub(in crate::game) struct PatternArgument;

    impl mod_api::CommandArgumentType for PatternArgument {
        fn kind(&self) -> mod_api::CommandArgumentKind {
            mod_api::CommandArgumentKind::Greedy
        }

        fn parse(&self, input: &str) -> Result<String, mod_api::CommandArgumentError> {
            Ok(input.trim().to_owned())
        }

        fn suggest(
            &self,
            _: Option<&mod_api::CommandSender>,
            prefix: &str,
        ) -> Vec<mod_api::CommandSuggestion> {
            let start = prefix.rfind(' ').map_or(0, |space| space + 1);
            ["stone", "dirt"]
                .into_iter()
                .filter(|block| block.starts_with(&prefix[start..]))
                .map(|block| mod_api::CommandSuggestion::new(block).replacing_from(start))
                .collect()
        }
    }

    #[test]
    fn a_greedy_argument_completes_the_token_being_typed() {
        let arguments: CommandArguments = HashMap::from([(
            "test:pattern".into(),
            Arc::new(PatternArgument) as Arc<dyn mod_api::CommandArgumentType>,
        )]);
        let (tree, _, problems) = super::command_tree(
            &[mod_api::Command::new("replace <pattern:test:pattern>")],
            &sunrise_client::command::Dispatcher::new(),
            &arguments,
        );
        assert!(problems.is_empty(), "{problems:?}");
        let mut dispatcher = Dispatcher::new();
        register_dispatcher(&mut dispatcher, &tree, "native_mod_command", &arguments);
        let registries = sunrise_client::command::Registries::default();

        // The second token: the range covers it, and the text in front of it is kept.
        let typed = "replace stone st";
        let parse = dispatcher.parse(typed, &registries);
        let (range, suggestions) = dispatcher.suggestions(&parse, typed.len(), &registries);
        assert_eq!(
            suggestions
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>(),
            ["stone"]
        );
        assert_eq!(range, (14, typed.len()), "the token, not the whole text");
        assert_eq!(suggestions[0].apply(typed), "replace stone stone");

        // The first token of the text replaces it whole, as before.
        let typed = "replace dir";
        let parse = dispatcher.parse(typed, &registries);
        let (range, suggestions) = dispatcher.suggestions(&parse, typed.len(), &registries);
        assert_eq!(range, (8, typed.len()));
        assert_eq!(suggestions[0].apply(typed), "replace dirt");
    }

    /// A type whose completion mixes one the text can hold with one it cannot.
    pub(in crate::game) struct MixedOffsets;

    impl mod_api::CommandArgumentType for MixedOffsets {
        fn kind(&self) -> mod_api::CommandArgumentKind {
            mod_api::CommandArgumentKind::Greedy
        }

        fn parse(&self, input: &str) -> Result<String, mod_api::CommandArgumentError> {
            Ok(input.to_owned())
        }

        fn suggest(
            &self,
            _: Option<&mod_api::CommandSender>,
            prefix: &str,
        ) -> Vec<mod_api::CommandSuggestion> {
            vec![
                mod_api::CommandSuggestion::new("stone").replacing_from(0),
                mod_api::CommandSuggestion::new("dirt").replacing_from(prefix.len() + 1),
            ]
        }
    }

    #[test]
    fn a_suggestion_with_an_offset_past_the_text_is_refused_and_the_others_stay() {
        let arguments: CommandArguments = HashMap::from([(
            "test:mixed".into(),
            Arc::new(MixedOffsets) as Arc<dyn mod_api::CommandArgumentType>,
        )]);
        let (tree, _, problems) = super::command_tree(
            &[mod_api::Command::new("fill <blocks:test:mixed>")],
            &sunrise_client::command::Dispatcher::new(),
            &arguments,
        );
        assert!(problems.is_empty(), "{problems:?}");
        let mut dispatcher = Dispatcher::new();
        register_dispatcher(&mut dispatcher, &tree, "native_mod_command", &arguments);
        let registries = sunrise_client::command::Registries::default();
        let typed = "fill st";
        let parse = dispatcher.parse(typed, &registries);
        let (_, suggestions) = dispatcher.suggestions(&parse, typed.len(), &registries);
        assert_eq!(
            suggestions
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>(),
            ["stone"],
            "the suggestion past the typed text is left out, never applied"
        );
    }

    #[test]
    fn resource_and_mod_id_kinds_accept_only_known_ids() {
        let mut arguments = command_arguments();
        // The test process loads no package: stand in for one the way `command_arguments` does.
        arguments.insert(
            argument_kinds::MOD_ID.into(),
            Arc::new(mod_api::ChoiceArgument::new(["demo"])),
        );
        let (tree, _, problems) = super::command_tree(
            &[
                mod_api::Command::new("spawnof <kind:resource:minecraft:entity_type>"),
                mod_api::Command::new("about <id:mod_id>"),
                mod_api::Command::new("bad <kind:resource:NotOk>"),
                mod_api::Command::new("worse <kind:resource:a:b:c>"),
                mod_api::Command::new("worst <kind:resource:a/b:c>"),
            ],
            &sunrise_client::command::Dispatcher::new(),
            &arguments,
        );
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(
            problems
                .iter()
                .all(|p| p.contains("is not a `namespace:path` resource location")),
            "{problems:?}"
        );
        for refused in ["bad", "worse", "worst"] {
            assert!(tree["children"].get(refused).is_none(), "{refused}");
        }
        let kind = &tree["children"]["spawnof"]["children"]["kind"];
        assert_eq!(kind["parser"], "minecraft:resource_key");
        assert_eq!(
            kind["properties"],
            json!({"registry": "minecraft:entity_type"})
        );
        assert_eq!(
            tree["children"]["about"]["children"]["id"]["custom_argument"],
            argument_kinds::MOD_ID
        );

        let mut dispatcher = Dispatcher::new();
        register_dispatcher(&mut dispatcher, &tree, "native_mod_command", &arguments);
        let registries = sunrise_client::command::Registries {
            entity_types: vec!["minecraft:zombie".into()],
            ..Default::default()
        };
        let parsed = dispatcher.parse("spawnof minecraft:zombie", &registries);
        let (context, _) = parsed.executable().unwrap();
        assert_eq!(
            context.get("kind"),
            Some(&Arg::Resource("minecraft:zombie".into()))
        );
        for refused in ["spawnof minecraft:nope", "about no_such_mod"] {
            assert!(
                dispatcher.parse(refused, &registries).executable().is_err(),
                "{refused}"
            );
        }
        let parsed = dispatcher.parse("about demo", &registries);
        let (context, _) = parsed.executable().unwrap();
        let Some(Arg::Custom { kind, value }) = context.get("id").cloned() else {
            panic!("a mod id parses as the built-in custom kind");
        };
        assert_eq!(
            custom_value(&kind, value),
            mod_api::CommandArgumentValue::String("demo".into()),
            "a loaded mod id reaches the mod as its plain string"
        );
    }

    #[test]
    fn native_usages_parse_with_their_permissions_in_the_server_dispatcher() {
        let (tree, _, problems) = tree(&[
            mod_api::Command::new("warp [name:word]"),
            mod_api::Command::op("warp set <amount:integer>", 2),
            mod_api::Command::new("toggle <enabled:boolean>"),
        ]);
        assert!(problems.is_empty(), "{problems:?}");
        let mut dispatcher = Dispatcher::new();
        register_dispatcher(
            &mut dispatcher,
            &tree,
            "native_mod_command",
            &HashMap::new(),
        );
        let mut registries = sunrise_client::command::Registries::default();
        assert!(dispatcher.parse("warp", &registries).executable().is_ok());
        assert!(
            dispatcher
                .parse("warp home", &registries)
                .executable()
                .is_ok()
        );
        assert!(
            dispatcher
                .parse("warp set 7", &registries)
                .executable()
                .is_err()
        );
        registries.permission = 2;
        assert!(
            dispatcher
                .parse("warp set 7", &registries)
                .executable()
                .is_ok()
        );
        assert!(
            dispatcher
                .parse("warp set nope", &registries)
                .executable()
                .is_err()
        );
        assert!(
            dispatcher
                .parse("warp set", &registries)
                .executable()
                .is_err()
        );
        assert!(
            dispatcher
                .parse("toggle true", &registries)
                .executable()
                .is_ok()
        );
        assert!(
            dispatcher
                .parse("toggle maybe", &registries)
                .executable()
                .is_err()
        );
    }

    #[test]
    #[ignore = "daily"]
    fn panicking_command_executors_keep_other_usages_and_only_undo_journaled_writes() {
        crate::game::in_own_process(
            "game::native_server::tests::panicking_command_executors_keep_other_usages_and_only_undo_journaled_writes",
            || {
                use std::sync::atomic::AtomicUsize;
                const JOURNALED: [i32; 3] = [2, 70, 2];
                const UNJOURNALED: [i32; 3] = [3, 70, 2];
                const ABANDONED: [i32; 3] = [4, 70, 2];
                const REFUSED: [i32; 3] = [5, 70, 2];
                struct Executors(Arc<AtomicUsize>);
                #[mod_api::__subscriptions]
                impl mod_api::Hooks for Executors {
                    fn register_commands(
                        &self,
                        _: &mut dyn mod_api::ServerHost,
                        event: &mut mod_api::RegisterCommandsEvent<'_>,
                    ) {
                        for (usage, pos, opens_root) in [
                            ("journalboom", JOURNALED, false),
                            ("plainboom", UNJOURNALED, false),
                            ("rootboom", ABANDONED, true),
                            ("ordinaryfailure", REFUSED, true),
                        ] {
                            let refuses = self.0.clone();
                            event.commands.push(mod_api::Command::new(usage).executes(
                                move |host, _, _| {
                                    let mutate = |host: &mut dyn mod_api::ServerHost| {
                                        let level = host.level("minecraft:overworld").unwrap();
                                        let state = host.block_state_id("minecraft:stone").unwrap();
                                        if level.set_block(
                                            host,
                                            pos,
                                            state,
                                            mod_api::BlockUpdateFlags::NONE,
                                        ) != Ok(true)
                                        {
                                            return Err(
                                                mod_api::Text::literal("block not placed").into()
                                            );
                                        }
                                        if usage == "ordinaryfailure" {
                                            refuses.fetch_add(1, Ordering::Relaxed);
                                            return Err(
                                                mod_api::Text::literal("ordinary failure").into()
                                            );
                                        }
                                        panic!("executor regression panic");
                                    };
                                    if opens_root {
                                        let res = mod_api::HostTransaction::transaction(
                                            host,
                                            |host, _| {
                                                mod_api::HostTransaction::transaction(
                                                    host,
                                                    |host, _| mutate(host),
                                                )
                                                .map_err(|e| match e {
                                                    mod_api::TransactionError::Aborted(msg) => msg,
                                                    _ => mod_api::Text::literal("tx failed").into(),
                                                })
                                            },
                                        );
                                        match res {
                                            Ok(()) => Ok(0),
                                            Err(mod_api::TransactionError::Aborted(msg)) => {
                                                Err(msg)
                                            }
                                            Err(_) => {
                                                Err(mod_api::Text::literal("tx failed").into())
                                            }
                                        }
                                    } else {
                                        mutate(host).map(|()| 0)
                                    }
                                },
                            ));
                        }
                        event
                            .commands
                            .push(mod_api::Command::new("worksagain").executes(|_, _, _| Ok(7)));
                    }
                }
                let refuses = Arc::new(AtomicUsize::new(0));
                mod_platform::testing::with_mods(
                    vec![mod_api::ModEntry::new(
                        "executor_owner",
                        Box::new(Executors(refuses.clone())),
                    )]
                    .into(),
                    || {
                        super::super::native_entity_refs::tests::with_game(|game| {
                            game.options.dedicated = Some(Vec::new());
                            game.world.chunks.insert(
                                (0, 0),
                                Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
                            );
                            let air = game.world.block(JOURNALED[0], JOURNALED[1], JOURNALED[2]);
                            let stone = game
                                .pack
                                .assets
                                .registry
                                .default_state("minecraft:stone")
                                .unwrap();
                            game.build_registries();
                            game.build_native_commands(&[]);

                            game.storage_transaction(|game, outer| {
                                let (failure, outcome) = game.recording_command_outcome(|game| {
                                    game.execute_line("journalboom", game.server_source())
                                });
                                assert_eq!(failure.unwrap_err().key, "command.failed");
                                assert_eq!(outcome, None);
                                assert_eq!(
                                    game.world.block(JOURNALED[0], JOURNALED[1], JOURNALED[2]),
                                    air
                                );
                                assert!(mod_api::ServerHost::transaction_matches(
                                    game,
                                    Some(outer)
                                ));
                                Ok::<_, ()>(())
                            })
                            .unwrap();
                            let failure = game
                                .execute_line("plainboom", game.server_source())
                                .unwrap_err();
                            assert!(failure.unexpected);
                            assert_eq!(
                                game.world
                                    .block(UNJOURNALED[0], UNJOURNALED[1], UNJOURNALED[2]),
                                stone
                            );

                            let failure = game
                                .execute_line("rootboom", game.server_source())
                                .unwrap_err();
                            assert!(failure.unexpected);
                            assert!(!game.has_open_transaction());
                            assert_eq!(
                                game.world.block(ABANDONED[0], ABANDONED[1], ABANDONED[2]),
                                air
                            );

                            game.build_native_commands(&[]);
                            for usage in ["journalboom", "rootboom"] {
                                assert_eq!(
                                    game.execute_line(usage, game.server_source())
                                        .unwrap_err()
                                        .key,
                                    "command.unknown.command"
                                );
                            }
                            assert_eq!(
                                game.execute_line("worksagain", game.server_source()),
                                Ok(7)
                            );
                            for _ in 0..2 {
                                assert_eq!(
                                    game.execute_line("ordinaryfailure", game.server_source())
                                        .unwrap_err()
                                        .message
                                        .plain(),
                                    "ordinary failure"
                                );
                                assert!(!game.has_open_transaction());
                                assert_eq!(
                                    game.world.block(REFUSED[0], REFUSED[1], REFUSED[2]),
                                    air
                                );
                            }
                            assert_eq!(refuses.load(Ordering::Relaxed), 2);
                            assert_eq!(
                                mod_api::__private::take_hook_panics(),
                                [mod_api::__private::HookPanicReport {
                                    mod_id: "executor_owner",
                                    hook: "command_executor",
                                }; 3]
                            );
                        })
                    },
                );
            },
        );
    }

    #[test]
    #[ignore = "daily"]
    fn native_unauthorized_commands_never_reach_the_listener() {
        crate::game::in_own_process(
            "game::native_server::tests::native_unauthorized_commands_never_reach_the_listener",
            || {
                struct Commands(Arc<Mutex<Vec<mod_api::CommandInvocation>>>);
                #[mod_api::__subscriptions]
                impl mod_api::Hooks for Commands {
                    fn register_commands(
                        &self,
                        _: &mut dyn mod_api::ServerHost,
                        event: &mut mod_api::RegisterCommandsEvent<'_>,
                    ) {
                        event.commands.extend(
                            [
                                mod_api::Command::op("foo [value:integer]", 4),
                                mod_api::Command::new("foo bar"),
                                mod_api::Command::op("choice <a:integer>", 4),
                                mod_api::Command::new("choice <a:integer> bar"),
                                mod_api::Command::new("choice <b:word>"),
                            ]
                            .map(|command| {
                                let heard = self.0.clone();
                                command.executes(move |_, _, invocation| {
                                    heard.lock().unwrap().push(invocation.clone());
                                    Ok(1)
                                })
                            }),
                        );
                    }
                }
                let heard = Arc::new(Mutex::new(Vec::new()));
                let mods = vec![mod_api::ModEntry::new(
                    "commands",
                    Box::new(Commands(heard.clone())),
                )]
                .into();
                mod_platform::testing::with_mods(mods, || {
                    super::super::native_entity_refs::tests::with_game(|game| {
                        super::super::native_item_places::tests::open_to_lan(game);
                        game.register_native_commands();
                        game.lan.as_mut().unwrap().hub.guests.push(player_guest());
                        for line in ["foo", "foo 7", "foo nope"] {
                            assert_eq!(game.perform_native_command(1, 0, line, false), Some(false));
                        }
                        assert!(
                            heard.lock().unwrap().is_empty(),
                            "denied usages must not call a mod"
                        );
                        assert_eq!(
                            game.perform_native_command(1, 0, "foo bar", false),
                            Some(true)
                        );
                        assert_eq!(game.perform_native_command(1, 4, "foo", false), Some(true));
                        assert_eq!(
                            game.perform_native_command(1, 4, "foo 7", false),
                            Some(true)
                        );
                        for line in ["choice 7", "choice 7 bar", "choice text"] {
                            assert_eq!(game.perform_native_command(1, 0, line, false), Some(true));
                        }
                        let heard = heard.lock().unwrap();
                        assert_eq!(heard.len(), 6);
                        assert_eq!(heard[0].literals, ["foo", "bar"]);
                        assert_eq!(heard[1].literals, ["foo"]);
                        assert!(heard[1].arguments.is_empty());
                        assert_eq!(heard[2].literals, ["foo"]);
                        assert_eq!(
                            heard[2].argument("value"),
                            Some(&mod_api::CommandArgumentValue::Integer(7))
                        );
                        assert_eq!(heard[3].literals, ["choice"]);
                        assert!(heard[3].argument("a").is_none());
                        assert_eq!(
                            heard[3].argument("b"),
                            Some(&mod_api::CommandArgumentValue::String("7".into()))
                        );
                        assert_eq!(heard[4].literals, ["choice", "bar"]);
                        assert_eq!(
                            heard[4].argument("a"),
                            Some(&mod_api::CommandArgumentValue::Integer(7))
                        );
                        assert_eq!(heard[5].literals, ["choice"]);
                        assert_eq!(
                            heard[5].argument("b"),
                            Some(&mod_api::CommandArgumentValue::String("text".into()))
                        );
                    });
                });
            },
        );
    }

    #[test]
    fn native_executable_permissions_do_not_follow_public_descendants() {
        for optional in [false, true] {
            for reverse in [false, true] {
                let admin = if optional {
                    "foo [value:integer]"
                } else {
                    "foo"
                };
                let mut commands = vec![
                    mod_api::Command::op(admin, 4),
                    mod_api::Command::new("foo bar"),
                ];
                if reverse {
                    commands.reverse();
                }
                let (tree, _, problems) = tree(&commands);
                assert!(problems.is_empty(), "{problems:?}");
                let mut dispatcher = Dispatcher::new();
                register_dispatcher(
                    &mut dispatcher,
                    &tree,
                    "native_mod_command",
                    &HashMap::new(),
                );
                for permission in 0..=4 {
                    let registries = sunrise_client::command::Registries {
                        permission,
                        ..Default::default()
                    };
                    for line in ["foo", "foo bar", "foo 7"] {
                        let allowed =
                            line == "foo bar" || permission == 4 && (line == "foo" || optional);
                        assert_eq!(
                            dispatcher.parse(line, &registries).executable().is_ok(),
                            allowed,
                            "{commands:?}, level {permission}, {line}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn native_public_prefixes_do_not_open_admin_children() {
        for reverse in [false, true] {
            let mut commands = vec![
                mod_api::Command::new("foo"),
                mod_api::Command::op("foo bar", 4),
            ];
            if reverse {
                commands.reverse();
            }
            let (tree, _, problems) = tree(&commands);
            assert!(problems.is_empty(), "{problems:?}");
            let mut dispatcher = Dispatcher::new();
            register_dispatcher(
                &mut dispatcher,
                &tree,
                "native_mod_command",
                &HashMap::new(),
            );
            for permission in 0..=4 {
                let registries = sunrise_client::command::Registries {
                    permission,
                    ..Default::default()
                };
                assert!(dispatcher.parse("foo", &registries).executable().is_ok());
                assert_eq!(
                    dispatcher
                        .parse("foo bar", &registries)
                        .executable()
                        .is_ok(),
                    permission == 4
                );
            }
        }
    }

    #[test]
    fn native_shared_executable_prefix_uses_last_accepted_registration() {
        let cases = [
            (
                vec![
                    mod_api::Command::op("foo [value:integer]", 4),
                    mod_api::Command::op("foo", 2),
                    mod_api::Command::op("foo", 2), // Later registration wins on ties.
                    mod_api::Command::new("foo [value:word]"), // Rejected: parser conflict.
                ],
                (2, 2),
                (4, 0),
            ),
            (
                vec![
                    mod_api::Command::op("foo", 2),
                    mod_api::Command::op("foo [value:integer]", 4), // Later runnable overwrites earlier lower level.
                    mod_api::Command::new("foo [value:word]"),      // Rejected: parser conflict.
                ],
                (4, 1),
                (4, 1),
            ),
        ];
        for (commands, (prefix_level, prefix_slot), (argument_level, argument_slot)) in cases {
            let arguments = HashMap::new();
            let mut executors = HashMap::new();
            let (tree, _, problems) = command_tree_for(
                &commands,
                BaseTree::Merged(&Dispatcher::new()),
                &arguments,
                false,
                Some(&mut executors),
            );
            assert_eq!(problems.len(), 1, "{problems:?}");
            let mut dispatcher = Dispatcher::new();
            register_command_nodes(
                &mut dispatcher,
                &tree,
                "native_mod_command",
                &arguments,
                Some(&executors),
            );
            for permission in 0..=4 {
                let registries = sunrise_client::command::Registries {
                    permission,
                    ..Default::default()
                };
                for (input, level, slot) in [
                    ("foo", prefix_level, prefix_slot),
                    ("foo 7", argument_level, argument_slot),
                ] {
                    let parsed = dispatcher.parse(input, &registries);
                    if permission >= level {
                        let (context, _) = parsed.executable().unwrap();
                        assert_eq!(
                            context.executor.unwrap().slot(),
                            slot,
                            "{commands:?}, level {permission}, {input}"
                        );
                    } else {
                        assert!(
                            parsed.executable().is_err(),
                            "{commands:?}, level {permission}, {input}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn native_rejected_usage_cannot_open_an_executable_prefix() {
        let (tree, _, problems) = tree(&[
            mod_api::Command::op("foo <value:integer>", 4),
            mod_api::Command::new("foo [value:word]"),
        ]);
        assert_eq!(problems.len(), 1);
        let mut dispatcher = Dispatcher::new();
        register_dispatcher(
            &mut dispatcher,
            &tree,
            "native_mod_command",
            &HashMap::new(),
        );
        for permission in 0..=4 {
            let registries = sunrise_client::command::Registries {
                permission,
                ..Default::default()
            };
            assert!(dispatcher.parse("foo", &registries).executable().is_err());
            assert!(
                dispatcher
                    .parse("foo hello", &registries)
                    .executable()
                    .is_err()
            );
            assert_eq!(
                dispatcher.parse("foo 7", &registries).executable().is_ok(),
                permission == 4
            );
        }
    }

    #[test]
    fn a_command_takes_its_lowest_level() {
        let (t, levels, _) = tree(&[
            mod_api::Command::new("warp [name:word]"),
            mod_api::Command::op("warp set <name:word>", 2),
        ]);
        assert_eq!(levels["warp"], 0);
        assert!(t["children"]["warp"].get("permissions").is_none());
        let set = &t["children"]["warp"]["children"]["set"];
        assert_eq!(set["permissions"]["permission"]["level"], "gamemasters");
    }

    #[test]
    fn bad_usages_are_left_out() {
        let (t, levels, problems) = tree(&[
            mod_api::Command::new("a [x:integer] <y:integer>"),
            mod_api::Command::new("b <x:text> <y:word>"),
            mod_api::Command::new("c <x:colour>"),
            mod_api::Command::new("<x:integer>"),
            mod_api::Command::new("Bad"),
            mod_api::Command::new("d <x:integer>"),
            mod_api::Command::new("d <x:word>"),
        ]);
        assert_eq!(problems.len(), 6, "{problems:?}");
        assert_eq!(levels.keys().collect::<Vec<_>>(), ["d"]);
        assert_eq!(
            t["children"]["d"]["children"]["x"]["parser"],
            "brigadier:integer"
        );
        let (none, levels, _) = tree(&[]);
        assert!(none.is_null() && levels.is_empty());
    }

    /// `integer(min,max)` and `long(min,max)` go out as vanilla's `IntegerArgumentInfo` and
    /// `LongArgumentInfo` write them (a bound at the type's extreme is left out), and the server
    /// refuses a value out of range with Brigadier's message before any executor runs.
    #[test]
    fn bounded_numbers_keep_their_bounds_in_the_tree_and_the_dispatcher() {
        let (t, _, problems) = tree(&[
            mod_api::Command::new("pay <amount:integer(0,1000)>"),
            mod_api::Command::new("bank <total:long(-5,9000000000)>"),
            mod_api::Command::new("floor <level:integer(0,)>"),
            mod_api::Command::new("whole <n:integer(-2147483648,2147483647)>"),
            mod_api::Command::new("plain <n:long>"),
            mod_api::Command::new("bad <n:integer(5,1)>"),
            mod_api::Command::new("bad <n:integer(0,2147483648)>"),
            mod_api::Command::new("bad <n:long(1)>"),
            mod_api::Command::new("bad <n:integer(a,)>"),
        ]);
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert!(
            problems.iter().all(|p| p.starts_with("`bad ")),
            "{problems:?}"
        );
        let c = &t["children"];
        let node = |root: &str, name: &str| c[root]["children"][name].clone();
        assert_eq!(node("pay", "amount")["parser"], "brigadier:integer");
        assert_eq!(
            node("pay", "amount")["properties"],
            json!({"min": 0, "max": 1000})
        );
        assert_eq!(node("bank", "total")["parser"], "brigadier:long");
        assert_eq!(
            node("bank", "total")["properties"],
            json!({"min": -5, "max": 9_000_000_000_i64})
        );
        assert_eq!(node("floor", "level")["properties"], json!({"min": 0}));
        assert!(node("whole", "n").get("properties").is_none());
        assert_eq!(node("plain", "n")["parser"], "brigadier:long");
        assert!(node("plain", "n").get("properties").is_none());

        let mut dispatcher = Dispatcher::new();
        register_dispatcher(&mut dispatcher, &t, "native_mod_command", &HashMap::new());
        let registries = sunrise_client::command::Registries::default();
        let value = |line: &str, name: &str| {
            dispatcher
                .parse(line, &registries)
                .executable()
                .unwrap()
                .0
                .get(name)
                .cloned()
        };
        assert_eq!(value("pay 1000", "amount"), Some(Arg::Int(1000)));
        assert_eq!(
            value("bank 9000000000", "total"),
            Some(Arg::Long(9_000_000_000))
        );
        assert_eq!(
            value("plain -9223372036854775808", "n"),
            Some(Arg::Long(i64::MIN))
        );
        for (line, expected) in [
            (
                "pay 1001",
                Text::tr("argument.integer.big", &[&1000, &1001]),
            ),
            ("pay -1", Text::tr("argument.integer.low", &[&0, &-1])),
            ("floor -1", Text::tr("argument.integer.low", &[&0, &-1])),
            (
                "bank 9000000001",
                Text::tr(
                    "argument.long.big",
                    &[&9_000_000_000_i64, &9_000_000_001_i64],
                ),
            ),
            ("bank -6", Text::tr("argument.long.low", &[&-5, &-6])),
            ("bank 1.5", Text::tr("parsing.long.invalid", &[&"1.5"])),
        ] {
            let error = dispatcher
                .parse(line, &registries)
                .executable()
                .unwrap_err();
            assert_eq!(error.message, expected, "{line}");
        }
    }

    /// A mod argument type that declares `CommandArgumentKind::Integer` goes out as vanilla's
    /// bounded `brigadier:integer`, so a client refuses what the server refuses; the server checks
    /// the bounds before the mod's parser.
    #[test]
    fn a_registered_integer_kind_goes_out_bounded() {
        struct Stack;
        impl mod_api::CommandArgumentType for Stack {
            fn kind(&self) -> mod_api::CommandArgumentKind {
                mod_api::CommandArgumentKind::Integer {
                    min: Some(1),
                    max: Some(64),
                }
            }
            fn parse(&self, input: &str) -> Result<String, mod_api::CommandArgumentError> {
                Ok(format!("{input} items"))
            }
            fn suggest(
                &self,
                _: Option<&mod_api::CommandSender>,
                _: &str,
            ) -> Vec<mod_api::CommandSuggestion> {
                Vec::new()
            }
        }
        let arguments: CommandArguments = HashMap::from([(
            "test:stack".to_owned(),
            Arc::new(Stack) as Arc<dyn mod_api::CommandArgumentType>,
        )]);
        let (t, _, problems) = command_tree(
            &[mod_api::Command::new("stack <count:test:stack>")],
            &Dispatcher::new(),
            &arguments,
        );
        assert!(problems.is_empty(), "{problems:?}");
        let count = &t["children"]["stack"]["children"]["count"];
        assert_eq!(count["parser"], "brigadier:integer");
        assert_eq!(count["properties"], json!({"min": 1, "max": 64}));
        assert_eq!(count["custom_argument"], "test:stack");
        let mut dispatcher = Dispatcher::new();
        register_dispatcher(&mut dispatcher, &t, "native_mod_command", &arguments);
        let registries = sunrise_client::command::Registries::default();
        let parsed = dispatcher.parse("stack 64", &registries);
        assert_eq!(
            parsed.executable().unwrap().0.get("count"),
            Some(&Arg::Custom {
                kind: "test:stack".into(),
                value: "64 items".into(),
            })
        );
        let error = dispatcher
            .parse("stack 65", &registries)
            .executable()
            .unwrap_err();
        assert_eq!(error.message, Text::tr("argument.integer.big", &[&64, &65]));
    }

    /// `<name:string>` and a type declaring `CommandArgumentKind::Quotable` read Brigadier's
    /// `StringArgumentType.string()`: a word or a quoted phrase, decoded, with an argument after
    /// it. Both go to clients as `brigadier:string` in its quotable mode; only the mod's type asks
    /// the server for completions, as `word` and a `Word` type do.
    #[test]
    fn a_string_usage_and_a_quotable_type_read_a_quoted_phrase_before_the_next_argument() {
        struct QuestObject;
        impl mod_api::CommandArgumentType for QuestObject {
            fn kind(&self) -> mod_api::CommandArgumentKind {
                mod_api::CommandArgumentKind::Quotable
            }
            fn parse(&self, input: &str) -> Result<String, mod_api::CommandArgumentError> {
                Ok(input.to_lowercase())
            }
            fn suggest(
                &self,
                _: Option<&mod_api::CommandSender>,
                _: &str,
            ) -> Vec<mod_api::CommandSuggestion> {
                Vec::new()
            }
        }
        let arguments: CommandArguments = HashMap::from([(
            "test:quest_object".to_owned(),
            Arc::new(QuestObject) as Arc<dyn mod_api::CommandArgumentType>,
        )]);
        let (t, _, problems) = command_tree(
            &[
                mod_api::Command::new("quests open <object:test:quest_object> <count:integer>"),
                mod_api::Command::new("quests rename <title:string> <count:integer>"),
            ],
            &Dispatcher::new(),
            &arguments,
        );
        assert!(problems.is_empty(), "{problems:?}");
        let quests = &t["children"]["quests"]["children"];
        let object = &quests["open"]["children"]["object"];
        assert_eq!(object["parser"], "brigadier:string");
        assert_eq!(object["properties"], json!({"type": "phrase"}));
        assert_eq!(object["custom_argument"], "test:quest_object");
        assert_eq!(object["suggestions"], "minecraft:ask_server");
        let title = &quests["rename"]["children"]["title"];
        assert_eq!(title["parser"], "brigadier:string");
        assert_eq!(title["properties"], json!({"type": "phrase"}));
        assert_eq!(
            title["suggestions"],
            Value::Null,
            "vanilla's string suggests nothing"
        );

        let mut dispatcher = Dispatcher::new();
        register_dispatcher(&mut dispatcher, &t, "native_mod_command", &arguments);
        let registries = sunrise_client::command::Registries::default();
        for (line, value) in [
            (r##"quests open "#Chapter_One" 3"##, "#chapter_one"),
            ("quests open 1A2B3C4D5E6F7A8B 3", "1a2b3c4d5e6f7a8b"),
            (r#"quests open "" 3"#, ""),
        ] {
            let parsed = dispatcher.parse(line, &registries);
            let (context, _) = parsed
                .executable()
                .unwrap_or_else(|e| panic!("{line}: {e:?}"));
            assert_eq!(
                context.get("object"),
                Some(&Arg::Custom {
                    kind: "test:quest_object".into(),
                    value: value.into(),
                }),
                "{line}"
            );
            assert_eq!(context.get("count"), Some(&Arg::Int(3)), "{line}");
        }
        for (line, value) in [
            (
                r#"quests rename "Getting \"Started\"" 2"#,
                r#"Getting "Started""#,
            ),
            (r"quests rename 'back\\slash' 2", r"back\slash"),
            ("quests rename '' 2", ""),
            ("quests rename Plain 2", "Plain"),
        ] {
            let parsed = dispatcher.parse(line, &registries);
            let (context, _) = parsed
                .executable()
                .unwrap_or_else(|e| panic!("{line}: {e:?}"));
            assert_eq!(
                context.get("title"),
                Some(&Arg::Str(value.into())),
                "{line}"
            );
            assert_eq!(context.get("count"), Some(&Arg::Int(2)), "{line}");
        }
        for line in [r#"quests rename "Getting 2"#, r#"quests open "Getting 2"#] {
            let error = dispatcher
                .parse(line, &registries)
                .executable()
                .unwrap_err();
            assert_eq!(
                error.message,
                Text::tr("parsing.quote.expected.end", &[]),
                "{line}"
            );
            assert_eq!(error.cursor, line.len(), "{line}");
        }
    }

    /// NeoForge's `RegisterCommandsEvent` registers into vanilla's dispatcher, whose literal
    /// merges by Brigadier's `CommandNode.addChild`: a mod usage under `/time` sits beside
    /// vanilla's, which stay; vanilla's node keeps its permission; an executor at a vanilla node
    /// replaces vanilla's, and a later one replaces that. A mod argument at a vanilla node's
    /// place, a mod argument on a vanilla literal and Sunrise's own tools are refused.
    #[test]
    fn usages_under_a_vanilla_root_merge_beside_its_usages() {
        let vanilla = sunrise_client::command::vanilla::session_dispatcher(
            &sunrise_client::gamerules::RuleRegistry::vanilla(),
            &[],
        );
        let commands = [
            mod_api::Command::new("time season <name:word>"),
            mod_api::Command::new("seed"),
            mod_api::Command::op("seed", 3),
            mod_api::Command::new("time set <time:integer>"),
            mod_api::Command::new("time <set:word>"),
            mod_api::Command::new("modprofile extra"),
            mod_api::Command::new("raidplus go"),
        ];
        let arguments = HashMap::new();
        let mut executors = HashMap::new();
        let (t, levels, problems) = command_tree_for(
            &commands,
            BaseTree::Merged(&vanilla),
            &arguments,
            false,
            Some(&mut executors),
        );
        assert_eq!(problems.len(), 3, "{problems:?}");
        for (problem, usage) in problems.iter().zip([
            "time set <time:integer>",
            "time <set:word>",
            "modprofile extra",
        ]) {
            assert!(problem.starts_with(&format!("`{usage}`")), "{problem}");
        }
        // A merged root keeps vanilla's level: only the mod's own root routes as a mod command.
        assert_eq!(levels.keys().collect::<Vec<_>>(), ["raidplus"]);
        let mut dispatcher = vanilla.clone();
        register_command_nodes(
            &mut dispatcher,
            &t,
            "native_mod_command",
            &arguments,
            Some(&executors),
        );
        let at = |permission| sunrise_client::command::Registries {
            permission,
            ..Default::default()
        };
        let run = |line: &str, permission| {
            dispatcher
                .parse(line, &at(permission))
                .executable()
                .map(|(context, command)| (command, context.executor.map(|key| key.slot())))
        };
        let vanilla_time = vanilla
            .parse("time set day", &at(2))
            .executable()
            .unwrap()
            .1;
        assert_eq!(run("time set day", 2), Ok((vanilla_time, None)));
        assert_eq!(
            run("time season spring", 2),
            Ok(("native_mod_command", Some(0)))
        );
        assert!(
            run("time season spring", 0).is_err(),
            "vanilla's /time keeps its permission"
        );
        assert!(run("seed", 2).is_err(), "the later usage's level runs it");
        assert_eq!(run("seed", 3), Ok(("native_mod_command", Some(2))));
        assert_eq!(run("raidplus go", 0), Ok(("native_mod_command", Some(6))));
    }

    /// WorldEdit-style `//set`: the usage `/set <x:word>` (typed `//set` with the chat slash) is a
    /// root literal `/set` with its argument, and the server dispatches it as typed.
    #[test]
    fn a_first_word_may_start_with_one_slash() {
        let (t, levels, problems) = tree(&[mod_api::Command::new("/set <x:word>")]);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(levels.keys().collect::<Vec<_>>(), ["/set"]);
        assert_eq!(t["children"]["/set"]["type"], "literal");
        assert_eq!(t["children"]["/set"]["children"]["x"]["type"], "argument");
        for refused in ["//x", "/", "/ <x:word>", "a /b", "a /"] {
            let (_, levels, problems) = tree(&[mod_api::Command::new(refused)]);
            assert!(levels.is_empty(), "{refused}");
            assert!(
                problems.len() == 1 && problems[0].contains("is not a command word"),
                "{refused}: {problems:?}"
            );
        }
        let arguments = command_arguments();
        let mut dispatcher = Dispatcher::new();
        register_dispatcher(&mut dispatcher, &t, "native_mod_command", &arguments);
        let registries = sunrise_client::command::Registries::default();
        let parsed = dispatcher.parse("/set stone", &registries);
        let (context, _) = parsed.executable().expect("`//set stone` runs");
        assert_eq!(context.literals, ["/set"]);
        assert_eq!(context.get("x"), Some(&Arg::Str("stone".into())));
        assert!(
            dispatcher
                .parse("set stone", &registries)
                .executable()
                .is_err()
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod fake_action_tests {
    #[allow(unused_imports)]
    use super::super::Game;
    #[allow(unused_imports)]
    use super::{Acting, ActionOutcome, ActionRefused, FakeEffect, PlayerAction};
    #[allow(unused_imports)]
    use mod_api::{BlockHit, Hand, PlayerRef};
    #[allow(unused_imports)]
    use sunrise_client::items::ItemStack;

    /// A server with a real guest (uuid 1) and a fake (uuid 2) standing at (0.5, 64, 0.5) on
    /// a stone floor, holding `item`, in game mode `mode`: the fake's handle and both conns.
    pub(in crate::game) fn fake(
        game: &mut crate::game::ServerGame<'_>,
        item: Option<&str>,
        mode: i32,
    ) -> (PlayerRef, u32, u32) {
        super::super::bossbars::audience_tests::add_real_and_fake(game);
        game.options.dedicated = Some(Vec::new());
        for (x, z) in (-1..=1).flat_map(|x| (-1..=1).map(move |z| (x, z))) {
            let mut chunk = sunrise_client::chunk::Chunk::empty(x, z);
            chunk.lit = true;
            game.world.chunks.insert((x, z), std::sync::Arc::new(chunk));
        }
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        for x in -3..=3 {
            for z in -3..=3 {
                game.world.set(x, 63, z, stone).unwrap();
            }
        }
        let level = game.level(game.dim.id()).unwrap();
        let player = game.fake_player(level, 2, "fake").unwrap();
        let host = game.lan.as_mut().unwrap();
        let conn = |uuid| {
            host.hub
                .guests
                .iter()
                .find(|g| g.uuid == uuid)
                .unwrap()
                .conn
        };
        let (fake, real) = (conn(2), conn(1));
        host.hub.guests.update_all(|g| {
            g.position = [0.5, 64.0, 0.5];
            g.game_mode = mode;
            g.abilities.update_for_mode(mode);
            g.inventory[g.selected] = item.map(|i| ItemStack::new(i, 1));
        });
        (player, fake, real)
    }

    pub(in crate::game) fn place(
        game: &mut crate::game::ServerGame<'_>,
        pos: [i32; 3],
        state: &str,
    ) {
        // `name[key=value,...]`, as a command writes a state.
        let (name, props) = state.split_once('[').unwrap_or((state, "]"));
        let properties = props
            .trim_end_matches(']')
            .split(',')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        let state = sunrise_world::chunk::BlockState {
            name: name.into(),
            properties,
        };
        let id = game.pack.assets.registry.resolve(&state).unwrap();
        game.world.set(pos[0], pos[1], pos[2], id).unwrap();
    }

    /// A click on the top of the block at `pos`.
    pub(in crate::game) fn on_top(pos: [i32; 3]) -> PlayerAction {
        let [x, y, z] = pos.map(f64::from);
        PlayerAction::UseOn {
            hand: Hand::Main,
            pos,
            hit: BlockHit::new([x + 0.5, y + 1.0, z + 0.5], [0, 1, 0], false),
        }
    }

    #[allow(unused_imports)]
    use super::super::{
        boats::Boat, minecarts::CART_KEY_Y, minecarts::CartKind, minecarts::Minecart,
        respawn_anchor::RespawnConfig, stations::Kind, stations::Station,
    };

    pub(in crate::game) fn held(
        game: &crate::game::ServerGame<'_>,
        uuid: u128,
    ) -> Option<ItemStack> {
        let host = game.lan.as_ref().unwrap();
        let g = host.hub.guests.iter().find(|g| g.uuid == uuid).unwrap();
        g.inventory[g.selected].clone()
    }

    /// A wandering trader at `at`, entity id `id`, and its handle.
    pub(in crate::game) fn trader(
        game: &mut crate::game::ServerGame<'_>,
        id: u32,
        at: [f64; 3],
    ) -> mod_api::EntityRef {
        let kind = game
            .res
            .mob_types()
            .get("minecraft:wandering_trader")
            .unwrap()
            .clone();
        let [x, y, z] = at;
        game.mobs.push(sunrise_client::mobs::Mob::new(
            id,
            kind,
            sunrise_core::geometry::Vec3::new(x, y, z),
            0.0,
        ));
        mod_api::EntityRef::new(game.native_lifecycle.epoch().unwrap(), id)
    }

    /// A fake player's use on a trader frees it again: the session the use
    /// opened closes through the merchant's own close path, so the trader is
    /// not left trading and the next use opens it as the first did.
    #[test]
    #[ignore = "daily"]
    fn a_trader_a_fake_player_used_trades_with_the_next_one() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, ..) = fake(game, Some("minecraft:stick"), 0);
            let id = 6000;
            let target = trader(game, id, [2.5, 64.0, 0.5]);
            let interact = PlayerAction::InteractOn {
                hand: Hand::Main,
                target,
                location: [0.0, 0.5, 0.0],
            };
            assert_eq!(
                game.player_perform_action(player, interact),
                Ok(ActionOutcome::Consumed)
            );
            let mob = game.mobs.iter().find(|m| m.id == id).unwrap();
            // With no offers the menu never opens, and this test would say nothing.
            let offers = mob.extra.trade.as_ref().map_or(0, |t| t.offers.len());
            assert!(offers > 0, "a wandering trader has offers");
            assert!(!mob.extra.flag2, "the trader is not left trading");
            assert!(game.station.is_none(), "the session is not left open");
            assert_eq!(
                game.player_perform_action(player, interact),
                Ok(ActionOutcome::Consumed),
                "the next use opens it again",
            );
        });
    }

    /// A chest minecart, a hopper minecart and a chest boat: the items stay
    /// on the vehicle, and the menu's stand-in is out of the level, where a
    /// transfer would edit it instead of the vehicle.
    #[test]
    #[ignore = "daily"]
    fn a_vehicles_chest_a_fake_player_used_keeps_its_items_and_no_stand_in() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, fake_conn, _) = fake(game, None, 0);
            let at = sunrise_core::geometry::Vec3::new(2.5, 64.0, 0.5);
            let diamonds = Some(ItemStack::new("minecraft:diamond", 5));
            let mut vehicles = Vec::new();
            for (id, kind) in [(6100, CartKind::Chest), (6101, CartKind::Hopper)] {
                let mut cart = Minecart::new(id, kind, at);
                cart.items[3] = diamonds.clone();
                vehicles.push((id, false));
                game.carts.push(cart);
            }
            let mut boat = Boat::new(6102, "oak".into(), at, 0.0).with_chest();
            boat.items[3] = diamonds.clone();
            // ChestBoat.interact opens the chest for a sneaking player.
            vehicles.push((6102, true));
            game.boats.push(boat);

            for (id, sneaking) in vehicles {
                let target = mod_api::EntityRef::new(game.native_lifecycle.epoch().unwrap(), id);
                let interact = PlayerAction::InteractOn {
                    hand: Hand::Main,
                    target,
                    location: [0.0, 0.5, 0.0],
                };
                // Twice: the second use is the next one finding it as it was.
                for _ in 0..2 {
                    game.lan
                        .as_mut()
                        .unwrap()
                        .hub
                        .update_guest(fake_conn, |guest| guest.sneaking = sneaking)
                        .unwrap();
                    assert_eq!(
                        game.player_perform_action(player, interact),
                        Ok(ActionOutcome::Consumed)
                    );
                    let key = (i32::MAX - 15, CART_KEY_Y, id as i32);
                    assert!(
                        game.world.block_entities.get(&key).is_none(),
                        "vehicle {id} left a stand-in",
                    );
                    assert!(game.container.is_none());
                    let items = game
                        .carts
                        .iter()
                        .find(|c| c.id == id)
                        .map(|c| &c.items)
                        .or_else(|| game.boats.iter().find(|b| b.id == id).map(|b| &b.items))
                        .unwrap();
                    assert_eq!(items[3], diamonds, "vehicle {id} lost its items");
                }
            }
        });
    }

    /// A real guest keeps its vehicle menu over a cart a fake player then
    /// uses: the refused open closes only a stand-in nobody is looking at,
    /// and the staging does not reset what the viewer's menu is editing.
    #[test]
    #[ignore = "daily"]
    fn a_viewer_keeps_the_vehicle_menu_a_fake_player_used() {
        use super::super::minecarts::{CART_KEY_Y, CartKind, Minecart};
        use sunrise_client::containers::BlockEntity;
        use sunrise_client::lan::menu::Slots;
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, fake_conn, real_conn) = fake(game, None, 0);
            let at = sunrise_core::geometry::Vec3::new(2.5, 64.0, 0.5);
            let mut cart = Minecart::new(6110, CartKind::Chest, at);
            cart.items[3] = Some(ItemStack::new("minecraft:diamond", 5));
            game.carts.push(cart);
            let gi = game
                .lan
                .as_ref()
                .unwrap()
                .hub
                .guests
                .iter()
                .position(|g| g.conn == real_conn)
                .unwrap();
            game.lan
                .as_mut()
                .unwrap()
                .hub
                .update_guest(real_conn, |guest| guest.position = [0.5, 64.0, 0.5])
                .unwrap();
            assert_eq!(
                game.guest_interact(real_conn, gi, 6110, Hand::Main, [2.5, 64.5, 0.5]),
                Ok(true),
                "the guest's use opens the vehicle's menu",
            );
            let key = (i32::MAX - 15, CART_KEY_Y, 6110);
            let viewer = |game: &crate::game::ServerGame<'_>| {
                game.lan
                    .as_ref()
                    .unwrap()
                    .hub
                    .guest(real_conn)
                    .unwrap()
                    .menu
                    .as_ref()
                    .is_some_and(|m| m.slots == Slots::Block(key))
            };
            assert!(viewer(game), "the guest's use opened the vehicle's menu");
            assert!(game.world.block_entities.contains_key(&key));
            // The viewer's live edit: its menu's clicks write through to the
            // stand-in (`source.write` after every click).
            let emerald = Some(ItemStack::new("minecraft:emerald", 2));
            if let Some(sunrise_client::containers::BlockEntity::Chest(items)) =
                game.world.block_entities.get_mut(&key)
            {
                items[4] = emerald.clone();
            }
            let target = mod_api::EntityRef::new(game.native_lifecycle.epoch().unwrap(), 6110);
            assert_eq!(
                game.player_perform_action(
                    player,
                    PlayerAction::InteractOn {
                        hand: Hand::Main,
                        target,
                        location: [0.0, 0.5, 0.0],
                    },
                ),
                Ok(ActionOutcome::Consumed)
            );
            assert!(viewer(game), "the fake's use kept the viewer's menu");
            assert!(
                game.world.block_entities.contains_key(&key),
                "the fake's use kept the stand-in the menu backs",
            );
            let BlockEntity::Chest(items) = &game.world.block_entities[&key] else {
                unreachable!()
            };
            assert_eq!(items[3], Some(ItemStack::new("minecraft:diamond", 5)));
            assert_eq!(items[4], emerald, "the staging kept the viewer's edit");
            game.guest_close_all(real_conn);
            assert!(!game.world.block_entities.contains_key(&key));
            let items = &game.carts.iter().find(|c| c.id == 6110).unwrap().items;
            assert_eq!(
                items[3],
                Some(ItemStack::new("minecraft:diamond", 5)),
                "the viewer's close put the items back",
            );
            assert_eq!(items[4], emerald, "the viewer's edit reached the vehicle");
            assert!(
                game.lan
                    .as_ref()
                    .unwrap()
                    .hub
                    .guest(fake_conn)
                    .unwrap()
                    .menu
                    .is_none()
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn a_fake_uses_items_through_the_players_own_path() {
        use super::super::native_entity_refs::tests::with_game;
        // A log on stone: placed, one used up.
        with_game(|game| {
            let (player, ..) = fake(game, Some("minecraft:oak_log"), 0);
            assert_eq!(
                game.player_perform_action(player, on_top([1, 63, 0])),
                Ok(ActionOutcome::Consumed)
            );
            let level = game.level(game.dim.id()).unwrap();
            let log = game.level_block_property(level, [1, 64, 0], "axis");
            assert_eq!(log.as_deref(), Some("y"), "placed on a top face, standing");
            assert_eq!(held(game, 2), None);
        });
        // A bucket clicked on a block passes, as vanilla's does (BucketItem has no useOn), and
        // empties when used, by its ray from the eyes (looking straight down here).
        with_game(|game| {
            let (player, ..) = fake(game, Some("minecraft:water_bucket"), 0);
            game.lan.as_mut().unwrap().hub.guests.update_all(|g| {
                g.pitch = 90.0;
            });
            assert_eq!(
                game.player_perform_action(player, on_top([1, 63, 0])),
                Ok(ActionOutcome::Passed)
            );
            let water = PlayerAction::UseItem { hand: Hand::Main };
            assert_eq!(
                game.player_perform_action(player, water),
                Ok(ActionOutcome::Consumed)
            );
            let level = game.level(game.dim.id()).unwrap();
            assert_eq!(
                game.level_block(level, [0, 64, 0])
                    .and_then(mod_api::__private::block_key_id),
                Some("minecraft:water")
            );
            assert_eq!(
                held(game, 2).map(|s| s.item.to_string()).as_deref(),
                Some("minecraft:bucket")
            );
        });
        // A pearl thrown into the air: one used up.
        with_game(|game| {
            let (player, ..) = fake(game, Some("minecraft:ender_pearl"), 0);
            let pearl = PlayerAction::UseItem { hand: Hand::Main };
            assert_eq!(
                game.player_perform_action(player, pearl),
                Ok(ActionOutcome::Consumed)
            );
            assert_eq!(held(game, 2), None);
        });
    }

    /// A zombie standing at `at`, entity id `id`, and its handle.
    pub(in crate::game) fn zombie(
        game: &mut crate::game::ServerGame<'_>,
        id: u32,
        at: [f64; 3],
    ) -> mod_api::EntityRef {
        let kind = game
            .res
            .mob_types()
            .get("minecraft:zombie")
            .unwrap()
            .clone();
        let [x, y, z] = at;
        game.mobs.push(sunrise_client::mobs::Mob::new(
            id,
            kind,
            sunrise_core::geometry::Vec3::new(x, y, z),
            0.0,
        ));
        mod_api::EntityRef::new(game.native_lifecycle.epoch().unwrap(), id)
    }

    pub(in crate::game) fn health(game: &crate::game::ServerGame<'_>, id: u32) -> f32 {
        game.mobs.iter().find(|m| m.id == id).unwrap().health
    }

    #[test]
    #[ignore = "daily"]
    fn scoped_fake_use_returns_items_and_restores_persistent_state() {
        use mod_api::PlayerActionSetup;
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, conn, _) = fake(game, Some("minecraft:diamond"), 1);
            game.lan
                .as_mut()
                .unwrap()
                .hub
                .update_guest(conn, |guest| {
                    guest.yaw = 31.0;
                    guest.pitch = 12.0;
                    guest.selected = 3;
                    guest.inventory[3] = Some(ItemStack::new("minecraft:emerald", 4));
                    guest.offhand = Some(ItemStack::new("minecraft:apple", 2));
                    guest.sneaking = true;
                    guest.previous_game_mode = Some(3);
                    guest.motion.as_mut().unwrap().shift_key_down = true;
                })
                .unwrap();
            game.lan
                .as_mut()
                .unwrap()
                .play
                .get_mut(&conn)
                .unwrap()
                .cooldowns
                .add("minecraft:bucket".into(), 100);
            let kind = game.res.mob_types().get("minecraft:cow").unwrap().clone();
            game.mobs.push(sunrise_client::mobs::Mob::new(
                5000,
                kind,
                sunrise_core::geometry::Vec3::new(2.0, 64.0, 0.5),
                0.0,
            ));
            let target = game.entity(5000).unwrap();
            let setup = PlayerActionSetup::new(
                Some(crate::game::api_values::test_snapshot(&ItemStack::new(
                    "minecraft:bucket",
                    2,
                ))),
                [0.5, 64.0 + f64::from(1.62_f32), 0.5],
                170.0,
                -15.0,
                false,
            );
            let action = PlayerAction::InteractOn {
                hand: Hand::Main,
                target,
                location: [0.0, 0.5, 0.0],
            };
            let result = player.perform_action_scoped(game, action, setup).unwrap();
            assert_eq!(result.outcome, ActionOutcome::Consumed);
            assert_eq!(
                result.held,
                Some(crate::game::api_values::test_snapshot(&ItemStack::new(
                    "minecraft:bucket",
                    1
                )))
            );
            assert_eq!(
                result.other,
                vec![crate::game::api_values::test_snapshot(&ItemStack::new(
                    "minecraft:milk_bucket",
                    1
                ))]
            );
            let guest = game.lan.as_ref().unwrap().hub.guest(conn).unwrap();
            assert_eq!(guest.position, [0.5, 64.0, 0.5]);
            assert_eq!(
                (guest.yaw, guest.pitch, guest.selected, guest.game_mode),
                (31.0, 12.0, 3, 1)
            );
            assert_eq!(guest.previous_game_mode, Some(3));
            assert_eq!(guest.offhand, Some(ItemStack::new("minecraft:apple", 2)));
            assert_eq!(
                guest.inventory[3],
                Some(ItemStack::new("minecraft:emerald", 4))
            );
            assert!(guest.sneaking && guest.motion.as_ref().unwrap().shift_key_down);
            let level = game.level(game.dim.id()).unwrap();
            assert_eq!(game.fake_player(level, 2, "fake"), Some(player));
            // The original cooldown is still in force, not the temporary action's timer.
            assert!(
                game.lan.as_ref().unwrap().play[&conn]
                    .cooldowns
                    .is_on_cooldown("minecraft:bucket")
            );
            assert_eq!(game.lan.as_ref().unwrap().hub.guests.len(), 2);
        });
    }

    /// An armor stand holding a saved item this build doesn't have hands it to a scoped action:
    /// the stack can't cross, so it goes back into the world where the fake stood, unchanged,
    /// with the default pickup delay, and the result counts it instead of panicking.
    #[test]
    #[ignore = "daily"]
    fn scoped_fake_use_returns_an_unrepresentable_stack_to_the_world() {
        use mod_api::PlayerActionSetup;
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, _, _) = fake(game, None, 0);
            let mut stand = sunrise_client::entities::ArmorStand::new(
                5001,
                sunrise_core::geometry::Vec3::new(2.0, 64.0, 0.5),
                0.0,
            );
            stand.equipment[0] = Some(ItemStack::new("test:missing", 1));
            game.armor_stands.push(stand);
            let target = game.entity(5001).unwrap();
            let setup = PlayerActionSetup::new(
                None,
                [0.5, 64.0 + f64::from(1.62_f32), 0.5],
                90.0,
                0.0,
                false,
            );
            let action = PlayerAction::InteractOn {
                hand: Hand::Main,
                target,
                location: [0.0, 1.5, 0.0],
            };
            let dropped = game.dropped.len();
            let result = player.perform_action_scoped(game, action, setup).unwrap();
            assert_eq!(result.outcome, ActionOutcome::Consumed);
            assert_eq!((result.held, result.other.len()), (None, 0));
            assert_eq!(result.unrepresentable, 1);
            assert_eq!(game.armor_stands.last().unwrap().equipment[0], None);
            assert_eq!(game.dropped.len(), dropped + 1);
            let item = game.dropped.last().unwrap();
            assert_eq!(item.stack, ItemStack::new("test:missing", 1));
            assert_eq!(item.pickup_delay, 10);
            assert_eq!((item.position.x, item.position.z), (0.5, 0.5));
        });
    }

    /// A join listener that keeps item entities out discards the returned stack as it would any
    /// drop: no item entity is left, and the result does not count it.
    #[test]
    #[ignore = "daily"]
    fn a_vetoed_return_is_not_counted() {
        use mod_api::{EntityJoinLevelEvent, Flow, Hooks, PlayerActionSetup, ServerHost};
        struct KeepItemsOut;
        #[mod_api::__subscriptions]
        impl Hooks for KeepItemsOut {
            fn entity_join_level(
                &self,
                _: &mut dyn ServerHost,
                event: &mut EntityJoinLevelEvent<'_>,
            ) -> Flow {
                if event.kind == "minecraft:item" {
                    Flow::Cancel
                } else {
                    Flow::Continue
                }
            }
        }
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "keep-items-out",
            Box::new(KeepItemsOut),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_game(|game| {
                let (player, _, _) = fake(game, None, 0);
                let mut stand = sunrise_client::entities::ArmorStand::new(
                    5001,
                    sunrise_core::geometry::Vec3::new(2.0, 64.0, 0.5),
                    0.0,
                );
                stand.equipment[0] = Some(ItemStack::new("test:missing", 1));
                game.armor_stands.push(stand);
                let target = game.entity(5001).unwrap();
                let setup = PlayerActionSetup::new(
                    None,
                    [0.5, 64.0 + f64::from(1.62_f32), 0.5],
                    90.0,
                    0.0,
                    false,
                );
                let action = PlayerAction::InteractOn {
                    hand: Hand::Main,
                    target,
                    location: [0.0, 1.5, 0.0],
                };
                let dropped = game.dropped.len();
                let result = player.perform_action_scoped(game, action, setup).unwrap();
                assert_eq!(result.outcome, ActionOutcome::Consumed);
                assert_eq!((result.held, result.other.len()), (None, 0));
                assert_eq!(result.unrepresentable, 0, "the vetoed stack is not counted");
                assert_eq!(game.armor_stands.last().unwrap().equipment[0], None);
                assert_eq!(game.dropped.len(), dropped, "no item entity is left");
            });
        });
    }

    #[test]
    #[ignore = "daily"]
    fn scoped_fake_use_restores_the_persistent_menu_owner() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, conn, _) = fake(game, Some("minecraft:diamond"), 1);
            let mut menu = Box::new(sunrise_client::menu::Menu::new(
                sunrise_client::menu::Kind::Inventory,
            ));
            menu.carried = Some(ItemStack::new("minecraft:stick", 3));
            let owner = (&*menu) as *const sunrise_client::menu::Menu;
            *game
                .lan
                .as_mut()
                .unwrap()
                .menus
                .entry(conn)
                .or_default()
                .crafting
                .inventory = Some(menu);
            place(game, [1, 64, 0], "minecraft:chest");
            let setup = mod_api::PlayerActionSetup::new(
                Some(crate::game::api_values::test_snapshot(&ItemStack::new(
                    "minecraft:oak_log",
                    1,
                ))),
                [0.5, 64.0 + f64::from(1.62_f32), 0.5],
                0.0,
                90.0,
                false,
            );
            let result = game
                .player_perform_action_scoped(player, on_top([1, 64, 0]), setup)
                .unwrap();
            assert_eq!(
                result.outcome,
                ActionOutcome::EffectBlocked(FakeEffect::Menu)
            );
            let restored = game.lan.as_ref().unwrap().menus[&conn]
                .crafting
                .inventory
                .as_ref()
                .unwrap();
            assert_eq!((&**restored) as *const sunrise_client::menu::Menu, owner);
            assert_eq!(restored.carried, Some(ItemStack::new("minecraft:stick", 3)));
        });
    }

    #[test]
    #[ignore = "daily"]
    fn fake_entity_interaction_checks_targets_and_does_not_use_items_in_air() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, ..) = fake(game, Some("minecraft:ender_pearl"), 0);
            let near = zombie(game, 5000, [2.0, 64.0, 0.5]);
            let far = zombie(game, 5001, [20.0, 64.0, 0.5]);
            let interact = |target| PlayerAction::InteractOn {
                hand: Hand::Main,
                target,
                location: [0.0; 3],
            };
            assert_eq!(
                game.player_perform_action(player, interact(near)),
                Ok(ActionOutcome::Passed)
            );
            assert_eq!(
                held(game, 2),
                Some(ItemStack::new("minecraft:ender_pearl", 1))
            );
            assert!(game.creatures.projectiles.is_empty());
            assert_eq!(
                game.player_perform_action(player, interact(far)),
                Err(ActionRefused::OutOfReach)
            );
            assert_eq!(
                game.player_perform_action(player, interact(player.entity())),
                Err(ActionRefused::InvalidTarget)
            );
            let stale = mod_api::EntityRef::new(mod_api::WorldEpoch::new(0), near.id());
            assert_eq!(
                game.player_perform_action(player, interact(stale)),
                Err(ActionRefused::InvalidTarget)
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn mob_target_setters_refuse_persistent_and_acting_fakes() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, conn, _) = fake(game, None, 0);
            let entity = zombie(game, 5000, [2.0, 64.0, 0.5]);
            let mob = game.mob(entity).unwrap();
            let target = game.living(player.entity()).unwrap();
            assert!(!mob.set_target(game, Some(target)));
            assert_eq!(mob.target(game), None);
            assert_eq!(
                game.as_guest(conn, |g| mob.set_target(g, Some(target))),
                Some(false)
            );
            assert_eq!(mob.target(game), None);
            assert!(!game.lan.as_ref().unwrap().mob_players.contains_key(&5000));
        });
    }

    #[test]
    #[ignore = "daily"]
    fn a_fake_attacks_through_the_players_own_path() {
        use super::super::native_entity_refs::tests::with_game;
        with_game(|game| {
            let (player, fake_conn, _) = fake(game, None, 0);
            game.lan.as_mut().unwrap().hub.guests.update_all(|g| {
                g.yaw = -90.0;
            });
            // A fist does 1 (the attack damage attribute's base), scaled by the attack strength
            // of a player that hasn't waited (`getAttackStrengthScale(0.5)`: 0.5 of a 5-tick
            // delay, 0.1, so 0.2 + 0.1² × 0.8 = 0.208), then by a zombie's 2 armour
            // (`CombatRules.getDamageAfterAbsorb`: 1 - (2 - 0.208 / 2) / 25).
            let near = zombie(game, 5000, [2.0, 64.0, 0.5]);
            let full = health(game, 5000);
            assert_eq!(
                game.player_perform_action(player, PlayerAction::Attack { target: near }),
                Ok(ActionOutcome::Consumed)
            );
            let dealt = 0.208f32 * (1.0 - (2.0 - 0.208 / 2.0) / 25.0);
            assert!((full - health(game, 5000) - dealt).abs() < 1e-4);
            // No mob takes the fake as the player to answer.
            let bound = game.lan.as_ref().unwrap().mob_players.get(&5000).copied();
            assert_ne!(bound, Some(fake_conn));
            assert_eq!(bound, None);
            // Ten blocks off is past the attack range; the fake itself isn't a target, nor is
            // an id no entity has.
            let far = zombie(game, 5001, [10.5, 64.0, 0.5]);
            assert_eq!(
                game.player_perform_action(player, PlayerAction::Attack { target: far }),
                Err(ActionRefused::OutOfReach)
            );
            assert_eq!(health(game, 5001), full);
            assert_eq!(
                game.player_perform_action(
                    player,
                    PlayerAction::Attack {
                        target: player.entity()
                    }
                ),
                Err(ActionRefused::InvalidTarget)
            );
            let nobody = mod_api::EntityRef::new(game.native_lifecycle.epoch().unwrap(), 6000);
            assert_eq!(
                game.player_perform_action(player, PlayerAction::Attack { target: nobody }),
                Err(ActionRefused::InvalidTarget)
            );
        });
    }

    /// A listener on `attack_entity` records the target's health where the hook
    /// runs; with `cancel` set it vetoes the blow.
    pub(in crate::game) struct AttackWatcher {
        pub(in crate::game) seen: std::sync::Arc<std::sync::Mutex<Vec<f32>>>,
        pub(in crate::game) cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    #[mod_api::__subscriptions]
    impl mod_api::Hooks for AttackWatcher {
        fn attack_entity(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::AttackEntityEvent<'_>,
        ) -> mod_api::Flow {
            let health = u32::try_from(event.entity)
                .ok()
                .and_then(|id| host.entity(id))
                .and_then(|entity| host.living(entity))
                .and_then(|living| living.health(host))
                .expect("the watcher's target is a live zombie");
            self.seen.lock().unwrap().push(health);
            if self.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return mod_api::Flow::Cancel;
            }
            mod_api::Flow::Continue
        }
    }

    /// A listener on `attack_entity` reads the attacking player's attack
    /// strength where the hook runs.
    pub(in crate::game) struct ChargeWatcher(
        pub(in crate::game) std::sync::Arc<std::sync::Mutex<Vec<Option<f32>>>>,
    );

    #[mod_api::__subscriptions]
    impl mod_api::Hooks for ChargeWatcher {
        fn attack_entity(
            &self,
            host: &mut dyn mod_api::ServerHost,
            _event: &mut mod_api::AttackEntityEvent<'_>,
        ) -> mod_api::Flow {
            let player = host.players().acting_player().expect("an attacking player");
            let scale = player.attack_strength_scale(host, 0.5);
            self.0.lock().unwrap().push(scale);
            mod_api::Flow::Continue
        }
    }

    #[test]
    #[ignore = "daily"]
    fn a_fake_breaks_blocks_at_once_by_the_game_modes_rules() {
        use super::super::native_entity_refs::tests::with_game;
        let level_block = |game: &mut crate::game::ServerGame<'_>, pos| {
            let level = game.level(game.dim.id()).unwrap();
            game.level_block(level, pos)
        };
        // Survival, bare-handed: stone breaks at once, with no digging time.
        with_game(|game| {
            let (player, ..) = fake(game, None, 0);
            let pos = [1, 63, 0];
            assert_eq!(
                game.player_perform_action(player, PlayerAction::BreakBlock { pos }),
                Ok(ActionOutcome::Consumed)
            );
            assert_eq!(
                level_block(game, pos).and_then(mod_api::__private::block_key_id),
                Some("minecraft:air")
            );
            // Air breaks nothing; bedrock, which no digging breaks (destroy time -1), stays;
            // ten blocks off is out of reach.
            assert_eq!(
                game.player_perform_action(player, PlayerAction::BreakBlock { pos }),
                Ok(ActionOutcome::Passed)
            );
            place(game, [-1, 63, 0], "minecraft:bedrock");
            assert_eq!(
                game.player_perform_action(player, PlayerAction::BreakBlock { pos: [-1, 63, 0] }),
                Ok(ActionOutcome::Passed)
            );
            assert_eq!(
                level_block(game, [-1, 63, 0]).and_then(mod_api::__private::block_key_id),
                Some("minecraft:bedrock")
            );
            place(game, [3, 64, 3], "minecraft:stone");
            game.lan.as_mut().unwrap().hub.guests.update_all(|g| {
                g.position = [-6.5, 64.0, -6.5];
            });
            assert_eq!(
                game.player_perform_action(player, PlayerAction::BreakBlock { pos: [3, 64, 3] }),
                Err(ActionRefused::OutOfReach)
            );
        });
        // Creative breaks what survival can't dig.
        with_game(|game| {
            let (player, ..) = fake(game, None, 1);
            place(game, [1, 63, 0], "minecraft:bedrock");
            assert_eq!(
                game.player_perform_action(player, PlayerAction::BreakBlock { pos: [1, 63, 0] }),
                Ok(ActionOutcome::Consumed)
            );
            assert_eq!(
                level_block(game, [1, 63, 0]).and_then(mod_api::__private::block_key_id),
                Some("minecraft:air")
            );
        });
        // Adventure mode can't break a block (blockActionRestricted), and a sword in creative
        // breaks nothing (canDestroyBlock).
        for (mode, item) in [(2, None), (1, Some("minecraft:iron_sword"))] {
            with_game(|game| {
                let (player, ..) = fake(game, item, mode);
                let pos = [1, 63, 0];
                assert_eq!(
                    game.player_perform_action(player, PlayerAction::BreakBlock { pos }),
                    Ok(ActionOutcome::Passed),
                    "game mode {mode}, holding {item:?}"
                );
                assert_eq!(
                    level_block(game, pos).and_then(mod_api::__private::block_key_id),
                    Some("minecraft:stone")
                );
            });
        }
    }

    /// `as_guest_at` starts a turn only for the guest it names: another `conn` at that place,
    /// a place past the list's end, and a guest already in a turn are refused, not a panic.
    #[test]
    #[ignore = "daily"]
    fn a_turn_at_a_place_is_refused_for_another_guest() {
        use super::super::lan_actions::TurnRefused;
        super::super::native_entity_refs::tests::with_game(|game| {
            let (_, fake_conn, real_conn) = fake(game, None, 0);
            let gi = game
                .lan
                .as_ref()
                .unwrap()
                .hub
                .guests
                .iter()
                .position(|g| g.conn == fake_conn)
                .unwrap();
            assert_eq!(
                game.as_guest_at(gi, real_conn, |_| ()),
                Err(TurnRefused::Gone)
            );
            assert_eq!(
                game.as_guest_at(999, fake_conn, |_| ()),
                Err(TurnRefused::Gone)
            );
            let nested = game.as_guest_at(gi, fake_conn, |g| g.as_guest_at(gi, fake_conn, |_| ()));
            assert_eq!(nested, Ok(Err(TurnRefused::Nested)));
            assert_eq!(game.as_guest_at(gi, fake_conn, |_| 7), Ok(7));
        });
    }

    #[test]
    #[ignore = "daily"]
    fn a_fakes_effects_are_stopped_where_they_start() {
        use super::super::native_entity_refs::tests::with_game;
        // A chest's menu (Game::open_menu).
        with_game(|game| {
            let (player, ..) = fake(game, None, 0);
            place(game, [1, 64, 0], "minecraft:chest");
            assert_eq!(
                game.player_perform_action(player, on_top([1, 64, 0])),
                Ok(ActionOutcome::EffectBlocked(FakeEffect::Menu))
            );
        });
        // A bed (Game::try_sleep).
        with_game(|game| {
            let (player, ..) = fake(game, None, 0);
            place(
                game,
                [1, 64, 0],
                "minecraft:red_bed[facing=north,part=foot]",
            );
            place(
                game,
                [1, 64, -1],
                "minecraft:red_bed[facing=north,part=head]",
            );
            assert_eq!(
                game.player_perform_action(player, on_top([1, 64, 0])),
                Ok(ActionOutcome::EffectBlocked(FakeEffect::Sleep))
            );
        });
        // A spectator's chest, the menu that bypasses Game::open_menu (spectator_open_menu).
        with_game(|game| {
            let (player, ..) = fake(game, None, 3);
            place(game, [1, 64, 0], "minecraft:chest");
            assert_eq!(
                game.player_perform_action(player, on_top([1, 64, 0])),
                Ok(ActionOutcome::EffectBlocked(FakeEffect::Menu))
            );
        });
        // A sign placed opens no editor (open_sign_editor), but the sign is placed.
        with_game(|game| {
            let (player, ..) = fake(game, Some("minecraft:oak_sign"), 0);
            assert_eq!(
                game.player_perform_action(player, on_top([1, 63, 0])),
                Ok(ActionOutcome::EffectBlocked(FakeEffect::Menu))
            );
            let level = game.level(game.dim.id()).unwrap();
            assert_eq!(
                game.level_block(level, [1, 64, 0])
                    .and_then(mod_api::__private::block_key_id),
                Some("minecraft:oak_sign")
            );
        });
        // Boarding (native_may_board, every boarding path's gate): refused for the fake only.
        with_game(|game| {
            let (_, fake_conn, real_conn) = fake(game, None, 0);
            assert_eq!(
                game.as_guest(fake_conn, |g| g.native_may_board(99)),
                Some(false)
            );
            assert_eq!(
                game.as_guest(real_conn, |g| g.native_may_board(99)),
                Some(true)
            );
        });
    }

    /// 1, 64 and 256 fakes each clicking the stone floor with an empty hand every tick, the full
    /// admission and use path (vanilla passes). Run with `-- --ignored --nocapture`, in release.
    #[test]
    #[ignore = "timing"]
    fn fakes_acting_each_tick_cost() {
        #[derive(Clone, Copy, Debug)]
        enum Kind {
            UseOn,
            Attack,
            BreakBlock,
        }
        for kind in [Kind::UseOn, Kind::Attack, Kind::BreakBlock] {
            for count in [1u128, 64, 256] {
                super::super::native_entity_refs::tests::with_game(|game| {
                    let (first, ..) = fake(game, None, 0);
                    let level = game.level(game.dim.id()).unwrap();
                    let mut players = vec![first];
                    for i in 1..count {
                        players.push(
                            game.fake_player(level, 100 + i, &format!("fake{i}"))
                                .unwrap(),
                        );
                    }
                    game.lan.as_mut().unwrap().hub.guests.update_all(|g| {
                        g.position = [0.5, 64.0, 0.5];
                        g.yaw = -90.0;
                    });
                    let target = zombie(game, 5000, [2.0, 64.0, 0.5]);
                    let stone = game
                        .pack
                        .assets
                        .registry
                        .default_state("minecraft:stone")
                        .unwrap();
                    let (action, expected) = match kind {
                        Kind::UseOn => (on_top([1, 63, 0]), ActionOutcome::Passed),
                        Kind::Attack => (PlayerAction::Attack { target }, ActionOutcome::Consumed),
                        Kind::BreakBlock => (
                            PlayerAction::BreakBlock { pos: [1, 63, 0] },
                            ActionOutcome::Consumed,
                        ),
                    };
                    let ticks = 200;
                    let start = std::time::Instant::now();
                    for _ in 0..ticks {
                        // The zombie never dies, and each break has a stone to break.
                        game.mobs[0].health = 20.0;
                        for &player in &players {
                            if let Kind::BreakBlock = kind {
                                game.world.set(1, 63, 0, stone).unwrap();
                            }
                            let outcome = game.player_perform_action(player, action);
                            assert_eq!(outcome, Ok(expected));
                        }
                    }
                    let elapsed = start.elapsed();
                    let per_tick = elapsed / ticks;
                    let per_action = elapsed / (ticks * players.len() as u32);
                    eprintln!(
                        "fake_action kind={kind:?} players={count} ns_per_action={}",
                        per_action.as_nanos()
                    );
                    println!(
                        "{kind:?}, {count} fakes: {per_tick:?} a tick, {per_action:?} an action"
                    );
                });
            }
        }
    }

    #[test]
    fn the_acting_mark_clears_on_a_panic_and_each_action_starts_unblocked() {
        let action = std::sync::Arc::new(super::FakeAction::default());
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _acting = Acting::start(&action);
            panic!("a mod's hook panics");
        }));
        assert!(unwound.is_err());
        assert!(!action.acting());
        {
            let acting = Acting::start(&action);
            action.block(FakeEffect::Sleep);
            action.block(FakeEffect::Menu);
            // The first effect stopped is the answer.
            assert_eq!(acting.blocked(), Some(FakeEffect::Sleep));
        }
        assert_eq!(Acting::start(&action).blocked(), None);
    }

    #[test]
    #[ignore = "daily"]
    fn a_real_player_is_unaffected_at_every_effect_site() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let (_, _, real_conn) = fake(game, None, 0);
            for effect in [
                FakeEffect::Menu,
                FakeEffect::Sleep,
                FakeEffect::Ride,
                FakeEffect::ChangeDimension,
            ] {
                assert_eq!(
                    game.as_guest(real_conn, |g| g.fake_blocks(effect)),
                    Some(false)
                );
            }
            // A real guest's click on a chest is the vanilla click, nothing stopped.
            place(game, [1, 64, 0], "minecraft:chest");
            let gi = game
                .lan
                .as_ref()
                .unwrap()
                .hub
                .guests
                .iter()
                .position(|g| g.conn == real_conn)
                .unwrap();
            let hit = super::super::Hit {
                pos: [1, 64, 0],
                face: [0, 1, 0],
                point: [1.5, 65.0, 0.5],
                inside: false,
            };
            assert_eq!(
                game.guest_use_item_on(real_conn, gi, hit, Hand::Main, None),
                Ok(true)
            );
            assert_eq!(
                game.as_guest(real_conn, |g| g.native_may_board(99)),
                Some(true)
            );
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod player_rule_tests {
    use super::super::native_guest_turn_tests::with_guests;
    use super::*;

    /// The guests of `with_guests` are rows 0 and 1; both stand in the level in place.
    fn in_this_level(game: &mut crate::game::ServerGame<'_>) {
        let level = game.dim.id().to_owned();
        game.lan.as_mut().unwrap().hub.guests.update_all(|g| {
            g.dimension = level.clone();
        });
    }

    fn at(dimension: &str, [x, y, z]: [i32; 3]) -> GlobalPos {
        GlobalPos::new(
            Identifier::parse(dimension).unwrap(),
            BlockPos::new(x, y, z),
        )
    }

    fn respawn(dimension: &str, pos: [i32; 3], yaw: f32, forced: bool) -> PlayerRespawnConfig {
        PlayerRespawnConfig {
            position: at(dimension, pos),
            yaw,
            forced,
        }
    }

    /// `/spawnpoint`'s write and `getRespawnConfig`'s read, on each player's own record.
    #[test]
    #[ignore = "daily"]
    fn a_respawn_point_is_read_and_written_in_the_players_own_record() {
        with_guests(|game| {
            let (a, b) = (game.player(12).unwrap(), game.player(13).unwrap());
            assert_eq!(a.respawn_config(&*game), Some(None), "none until set");

            // The yaw wraps as `RespawnData.of` does: 270 degrees is -90.
            let nether = "minecraft:the_nether";
            assert!(a.set_respawn_config(game, Some(respawn(nether, [1, 2, 3], 270.0, true))));
            assert_eq!(
                a.respawn_config(&*game),
                Some(Some(respawn(nether, [1, 2, 3], -90.0, true)))
            );
            assert_eq!(
                game.lan.as_ref().unwrap().hub.guests[0].respawn_elsewhere,
                Some((nether.into(), [1, 2, 3], -90.0, 0.0, true)),
                "the record a respawn reads"
            );
            assert_eq!(
                b.respawn_config(&*game),
                Some(None),
                "the other is untouched"
            );

            // The acting player reads the point swapped into place.
            let seen = game.as_guest(12, |game| a.respawn_config(&*game)).unwrap();
            assert_eq!(seen, Some(Some(respawn(nether, [1, 2, 3], -90.0, true))));

            // Refused writes change nothing: a dimension the game does not have, a yaw that is not a number.
            let nowhere = respawn("test:nowhere", [9, 9, 9], 0.0, false);
            assert!(!a.set_respawn_config(game, Some(nowhere)));
            let not_a_number = respawn(nether, [9, 9, 9], f32::NAN, false);
            assert!(!a.set_respawn_config(game, Some(not_a_number)));
            assert_eq!(
                a.respawn_config(&*game),
                Some(Some(respawn(nether, [1, 2, 3], -90.0, true)))
            );

            // None forgets it.
            assert!(a.set_respawn_config(game, None));
            assert_eq!(a.respawn_config(&*game), Some(None));
            assert_eq!(
                game.lan.as_ref().unwrap().hub.guests[0].respawn_elsewhere,
                None
            );
        });
    }

    /// The write goes through `spawn_point_change`, so a mod's `player_set_spawn` veto refuses it, as it does `/spawnpoint`.
    #[test]
    #[ignore = "daily"]
    fn a_mods_veto_refuses_the_respawn_write() {
        struct Veto;
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Veto {
            fn player_set_spawn(
                &self,
                _: &mut dyn mod_api::ServerHost,
                _: &mut mod_api::PlayerSetSpawnEvent,
            ) -> mod_api::Flow {
                mod_api::Flow::Cancel
            }
        }
        let mods = vec![mod_api::ModEntry::new("veto", Box::new(Veto))].into();
        mod_platform::testing::with_mods(mods, || {
            with_guests(|game| {
                let a = game.player(12).unwrap();
                let overworld = "minecraft:overworld";
                let config = respawn(overworld, [1, 2, 3], 0.0, false);
                assert!(!a.set_respawn_config(game, Some(config)));
                assert_eq!(a.respawn_config(&*game), Some(None));
            });
        });
    }

    /// The client's text-filtering option, as its `ClientInformation` said.
    #[test]
    #[ignore = "daily"]
    fn text_filtering_is_what_the_clients_information_said() {
        with_guests(|game| {
            let (a, b) = (game.player(12).unwrap(), game.player(13).unwrap());
            game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                g.info.text_filtering = true;
            });
            assert_eq!(a.text_filtering(&*game), Some(true));
            assert_eq!(b.text_filtering(&*game), Some(false));
            // A later `ClientInformation` replaces it.
            game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                g.info.text_filtering = false;
            });
            assert_eq!(a.text_filtering(&*game), Some(false));
        });
    }

    /// The sleep reads and the counter write, with the night skip's own counts as the oracle.
    #[test]
    #[ignore = "daily"]
    fn sleep_eligibility_and_the_timer_follow_the_players_state() {
        with_guests(|game| {
            in_this_level(game);
            let (a, b) = (game.player(12).unwrap(), game.player(13).unwrap());
            let (active, sleeping, deep) = game.sleep_counts();
            assert_eq!(a.sleep_timer(&*game), Some(0));
            assert_eq!(a.counts_for_sleep(&*game), Some(true));
            assert_eq!(a.sleep_ignored(&*game), Some(false));

            // A player in bed at 100 ticks is one of the deep sleepers the night waits for.
            game.lan.as_mut().unwrap().hub.guests.update(0, |g| {
                g.bed = Some([5, 64, 5]);
            });
            assert!(a.set_sleep_timer(game, 100));
            assert_eq!(a.sleep_timer(&*game), Some(100));
            assert_eq!(game.lan.as_ref().unwrap().hub.guests[0].sleep_timer, 100);
            assert_eq!(game.sleep_counts(), (active, sleeping + 1, deep + 1));
            // The wake-up fade's last value is allowed; one past it is not, and changes nothing.
            assert!(a.set_sleep_timer(game, 109));
            assert!(!a.set_sleep_timer(game, 110));
            assert_eq!(a.sleep_timer(&*game), Some(109));

            // Ignoring leaves an out-of-bed player out of the count, and an in-bed one in.
            assert!(b.set_sleep_ignored(game, true));
            assert_eq!(b.sleep_ignored(&*game), Some(true));
            assert_eq!(b.counts_for_sleep(&*game), Some(false));
            assert_eq!(game.sleep_counts().0, active - 1);
            assert!(a.set_sleep_ignored(game, true));
            assert_eq!(a.counts_for_sleep(&*game), Some(true), "in bed");
            game.lan
                .as_mut()
                .unwrap()
                .hub
                .guests
                .update(0, |g| g.bed = None);
            assert_eq!(a.counts_for_sleep(&*game), Some(false), "out of bed");
            assert!(a.set_sleep_ignored(game, false));
            assert!(b.set_sleep_ignored(game, false));
            assert_eq!(a.counts_for_sleep(&*game), Some(true));

            // A spectator is never counted.
            game.lan.as_mut().unwrap().hub.guests.update(1, |g| {
                g.game_mode = 3;
            });
            assert_eq!(b.counts_for_sleep(&*game), Some(false));

            // The acting player's state is the one in place.
            let seen = game
                .as_guest(12, |game| {
                    (a.sleep_timer(&*game), a.counts_for_sleep(&*game))
                })
                .unwrap();
            assert_eq!(seen, (Some(109), Some(true)));
        });
    }
}

/// The online mode a dedicated server's network config carries: it has an
/// `OnlineMode` key (and so signs its logins) only in online mode.
pub(in crate::game) fn online_mode_of(config: &sunrise_client::lan::NetConfig) -> bool {
    config.online.is_some()
}

#[cfg(test)]
pub(in crate::game) mod network_settings_tests {
    use super::*;
    #[test]
    #[ignore = "daily"]
    fn server_settings_follow_the_network_config() {
        use sunrise_client::lan::{NetConfig, OnlineMode};
        let mut config = NetConfig::loopback(25565, String::new(), 20);
        // online-mode=false: no key.
        assert!(!online_mode_of(&config));
        // online-mode=true: the server's key.
        config.online = Some(std::sync::Arc::new(OnlineMode::generate(false).unwrap()));
        assert!(online_mode_of(&config));
    }
}
