

use std::sync::Arc;
use std::sync::Mutex;
use sunrise_game_data::text::Text;
use sunrise_protocol::packets::play::Clientbound as C;
#[cfg(test)]
#[path = "native_server/command_failure_tests_server.rs"]
pub(crate) mod command_failure_tests;

#[cfg(test)]
#[path = "native_server/selector_tests_server.rs"]
pub(crate) mod selector_tests;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::game::native_server::tests::with_command_sources;

    #[test]
    #[ignore = "daily"]
    fn command_refresh_rebuilds_the_typed_dispatcher_only_on_request() {
        sunrise_gameplay_common::game::in_own_process(
            "game::server_side::native_server_server::tests::command_refresh_rebuilds_the_typed_dispatcher_only_on_request",
            || {
                use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
                struct ChangingCommands {
                    changed: Arc<AtomicBool>,
                    registrations: Arc<AtomicUsize>,
                    calls: Arc<Mutex<Vec<Option<&'static str>>>>,
                    replacement: bool,
                }
                #[mod_api::__subscriptions]
                impl mod_api::Hooks for ChangingCommands {
                    fn register_commands(
                        &self,
                        _: &mut dyn mod_api::ServerHost,
                        event: &mut mod_api::RegisterCommandsEvent<'_>,
                    ) {
                        let changed = self.changed.load(Ordering::Relaxed);
                        if !self.replacement {
                            self.registrations.fetch_add(1, Ordering::Relaxed);
                            let usage = if changed {
                                "after <n:integer>"
                            } else {
                                "before"
                            };
                            event
                                .commands
                                .push(mod_api::Command::new(usage).executes(|_, _, _| Ok(1)));
                        }
                        if changed == self.replacement {
                            let calls = self.calls.clone();
                            let result = if self.replacement { 37 } else { 0 };
                            event
                                .commands
                                .push(mod_api::Command::new("retained").executes(
                                    move |_, _, _| {
                                        calls
                                            .lock()
                                            .unwrap()
                                            .push(mod_api::content::crash::current_native_mod());
                                        Ok(result)
                                    },
                                ));
                        }
                    }
                    fn player_commands_sent(
                        &self,
                        _: &mut dyn mod_api::ServerHost,
                        event: &mut mod_api::PlayerCommandsSentEvent<'_>,
                    ) {
                        event.names.retain(|name| name != "before");
                    }
                }
                let changed = Arc::new(AtomicBool::new(false));
                let registrations = Arc::new(AtomicUsize::new(0));
                let calls = Arc::new(Mutex::new(Vec::new()));
                mod_platform::testing::with_mods(
                    [("refresh", false), ("replacement", true)]
                        .map(|(id, replacement)| {
                            mod_api::ModEntry::new(
                                id,
                                Box::new(ChangingCommands {
                                    changed: changed.clone(),
                                    registrations: registrations.clone(),
                                    calls: calls.clone(),
                                    replacement,
                                }),
                            )
                        })
                        .into(),
                    || {
                        super::super::native_guest_turn_tests::with_guests(|game| {
                            game.build_registries();
                            game.register_native_commands();
                            assert_eq!(registrations.load(Ordering::Relaxed), 1);
                            let old_parse =
                                game.ui.dispatcher.0.parse("retained", &game.ui.registries);
                            let old_context = old_parse.executable().unwrap().0;
                            let (result, outcome) = game.recording_command_outcome(|game| {
                                game.execute_line("retained", game.server_source())
                            });
                            assert_eq!(result, Ok(0));
                            assert_eq!(outcome, Some((true, 0)));
                            assert_eq!(*calls.lock().unwrap(), [Some("refresh")]);
                            changed.store(true, Ordering::Relaxed);
                            game.register_native_commands();
                            assert_eq!(registrations.load(Ordering::Relaxed), 1);
                            let registered =
                                game.side.native_commands.0.registered.as_ref().unwrap();
                            assert!(registered.levels.contains_key("before"));
                            mod_api::Commands::refresh_commands(game);
                            mod_api::Commands::refresh_commands(game);
                            assert_eq!(
                                registrations.load(Ordering::Relaxed),
                                1,
                                "requests are deferred"
                            );
                            game.register_native_commands();
                            assert!(!game.side.native_commands.0.refresh);
                            assert_eq!(
                                registrations.load(Ordering::Relaxed),
                                2,
                                "requests coalesce"
                            );
                            assert_eq!(
                                game.run_native_command(old_context.clone())
                                    .unwrap_err()
                                    .key,
                                "command.unknown.command"
                            );
                            assert_eq!(*calls.lock().unwrap(), [Some("refresh")]);
                            assert_eq!(game.execute_line("retained", game.server_source()), Ok(37));
                            assert_eq!(
                                *calls.lock().unwrap(),
                                [Some("refresh"), Some("replacement")]
                            );
                            assert!(mod_api::__private::set_mod_enabled("replacement", false));
                            assert_eq!(
                                game.execute_line("retained", game.server_source())
                                    .unwrap_err()
                                    .key,
                                "command.unknown.command"
                            );
                            assert_eq!(
                                *calls.lock().unwrap(),
                                [Some("refresh"), Some("replacement")]
                            );
                            assert!(mod_api::__private::set_mod_enabled("replacement", true));
                            let commands = game.side.native_commands.0.registered.as_ref().unwrap();
                            assert!(!commands.levels.contains_key("before"));
                            assert!(commands.levels.contains_key("after"));
                            let dispatcher = &game.ui.dispatcher.0;
                            assert!(
                                dispatcher
                                    .parse("after 7", &game.ui.registries)
                                    .executable()
                                    .is_ok()
                            );
                            assert!(
                                dispatcher
                                    .parse("after nope", &game.ui.registries)
                                    .executable()
                                    .is_err()
                            );
                            assert!(
                                dispatcher
                                    .parse("before", &game.ui.registries)
                                    .executable()
                                    .is_err()
                            );
                            game.register_native_commands();
                            assert_eq!(registrations.load(Ordering::Relaxed), 2);
                            mod_api::Commands::refresh_commands(game);
                            game.register_native_commands();
                            assert_eq!(registrations.load(Ordering::Relaxed), 3);
                            assert!(game.side.native_commands.0.level("after").is_some());
                            changed.store(false, Ordering::Relaxed);
                            game.reload_datapacks(false, &game.level.enabled_features.clone())
                                .unwrap();
                            let (sent, received) = std::sync::mpsc::channel();
                            game.side
                                .lan
                                .as_mut()
                                .unwrap()
                                .hub
                                .update_guest(12, |guest| {
                                    guest.sender = sunrise_sim::lan::PlayerSender::new(
                                        sent,
                                        Default::default(),
                                    );
                                })
                                .unwrap();
                            game.register_native_commands();
                            let packet = received
                                .try_iter()
                                .flat_map(|out| match out {
                                    sunrise_sim::lan::Out::Packet(body) => vec![body],
                                    sunrise_sim::lan::Out::Packets(bodies) => bodies,
                                    sunrise_sim::lan::Out::Disconnect(_) => {
                                        panic!("reload disconnected guest")
                                    }
                                })
                                .find_map(|body| {
                                    let mut reader = sunrise_protocol::buf::Reader::new(&body);
                                    let id = reader.varint().unwrap();
                                    match C::decode(id, &body[reader.position()..]).unwrap() {
                                        C::Commands(packet) => Some(packet),
                                        _ => None,
                                    }
                                })
                                .expect("reload must deliver a command tree");
                            let names: Vec<_> = packet.nodes[packet.root as usize]
                                .children
                                .iter()
                                .map(|&i| packet.nodes[i as usize].name.as_str())
                                .collect();
                            assert!(!names.is_empty());
                            assert!(
                                !names.contains(&"before"),
                                "new reload root must be filtered"
                            );
                            assert!(!names.contains(&"after"), "old root must be retired");
                        })
                    },
                );
            },
        );
    }

    #[test]
    #[ignore = "daily"]
    fn native_command_feedback_respects_track_output_and_silence() {
        use sunrise_gameplay::game::server_side::commands::execute::Source;
        use sunrise_level::chunk::block_entity::BlockEntity;
        with_command_sources(|game| {
            let source = Source {
                block: Some((1, 64, 1)),
                silent: false,
                ..game.server_source()
            };
            for track in [true, false] {
                if let Some(BlockEntity::CommandBlock(block)) =
                    game.world.block_entities.get_mut(&(1, 64, 1))
                {
                    block.track_output = track;
                    block.last_output = None;
                }
                game.with_source(source, |game| {
                    game.native_feedback(Text::literal("answer"), false)
                });
                let Some(BlockEntity::CommandBlock(block)) =
                    game.world.block_entities.get(&(1, 64, 1))
                else {
                    panic!("block missing")
                };
                assert_eq!(block.last_output.is_some(), track);
            }
            game.side.chat_capture = Some(Vec::new());
            game.with_source(
                Source {
                    silent: true,
                    block: None,
                    ..source
                },
                |game| game.native_feedback(Text::literal("silent"), false),
            );
            assert!(game.side.chat_capture.take().unwrap().is_empty());
            assert!(game.side.command_source.is_none());
            game.as_guest(12, |game| {
                let source = game.server_source();
                game.with_source(source, |game| {
                    assert!(matches!(
                        game.native_command_origin(4),
                        Ok(mod_api::CommandSource::Console { .. })
                    ));
                });
            })
            .unwrap();
            game.side.exec.0.functions.push("test:outer".into());
            assert!(game.native_run_command("list"));
            assert_eq!(game.side.exec.0.functions, ["test:outer"]);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn native_commands_keep_function_block_and_cart_sources() {
        sunrise_gameplay_common::game::in_own_process(
            "game::server_side::native_server_server::tests::native_commands_keep_function_block_and_cart_sources",
            || {
                use mod_api::{CommandSource, PermissionLevel as Permission};
                use sunrise_core::position::BlockPos;
                use sunrise_level::{
                    chunk::block_entity::BlockEntity,
                    datapack::{Function, FunctionLine},
                };
                struct Commands(Arc<Mutex<Vec<CommandSource>>>);
                #[mod_api::__subscriptions]
                impl mod_api::Hooks for Commands {
                    fn register_commands(
                        &self,
                        _: &mut dyn mod_api::ServerHost,
                        event: &mut mod_api::RegisterCommandsEvent<'_>,
                    ) {
                        let heard = self.0.clone();
                        let execute =
                            move |host: &mut dyn mod_api::ServerHost,
                                  source: &CommandSource,
                                  _: &mod_api::CommandInvocation| {
                                heard.lock().unwrap().push(source.clone());
                                host.commands()
                                    .feedback(mod_api::Text::literal("source answer"), false)
                                    .unwrap();
                                Ok(1)
                            };
                        event.commands.extend([
                            mod_api::Command::new("origin").executes(execute.clone()),
                            mod_api::Command::op("denied", 4).executes(execute),
                            // Replaced by the later accepted registration; failure must not fall back here.
                            mod_api::Command::new("unhandled").executes(|_, _, _| Ok(99)),
                            mod_api::Command::new("unhandled")
                                .executes(|_, _, _| Err(mod_api::Text::literal("refused").into())),
                        ]);
                    }
                }
                let heard = Arc::new(Mutex::new(Vec::new()));
                mod_platform::testing::with_mods(
                    vec![mod_api::ModEntry::new(
                        "origins",
                        Box::new(Commands(heard.clone())),
                    )]
                    .into(),
                    || {
                        with_command_sources(|game| {
                            game.command_block_tick(BlockPos::new(1, 64, 1));
                            let Some(BlockEntity::CommandBlock(block)) =
                                game.world.block_entities.get(&(1, 64, 1))
                            else {
                                panic!("block missing")
                            };
                            assert_eq!(block.success_count, 1);
                            let source = heard.lock().unwrap()[0].clone();
                            assert!(
                                matches!(source, CommandSource::CommandBlock { position: [1, 64, 1], level, .. } if level == game.dim.key())
                            );
                            assert_eq!(source.permission(), Permission::Gamemasters);
                            assert!(game.perform_cart_command(99));
                            assert_eq!(game.carts[0].command.success_count, 1);
                            assert!(game.carts[0].command.last_output.is_some());
                            assert!(
                                matches!(heard.lock().unwrap().last().unwrap(), CommandSource::CommandBlockMinecart { entity, .. } if entity.id() == 99)
                            );
                            game.side.chat_capture = Some(Vec::new());
                            let mut function = Function {
                                id: "test:inner".into(),
                                lines: vec![
                                    FunctionLine::Command(
                                        "execute positioned 2 64 3 run origin".into(),
                                    ),
                                    FunctionLine::Command("denied".into()),
                                ],
                                parameters: vec![],
                            };
                            game.ensure_datapacks();
                            game.side
                                .fns
                                .0
                                .store
                                .functions
                                .insert(function.id.clone(), function.clone());
                            function.id = "test:outer".into();
                            function.lines =
                                vec![FunctionLine::Command("function test:inner".into())];
                            let before = heard.lock().unwrap().len();
                            game.run_function(&function, None, game.server_source())
                                .unwrap();
                            assert_eq!(heard.lock().unwrap().len(), before + 1);
                            assert!(
                                matches!(heard.lock().unwrap().last().unwrap(), CommandSource::Function { id, permission } if id.to_string() == "test:inner" && *permission == Permission::Gamemasters)
                            );
                            assert!(
                                game.side.chat_capture.take().unwrap().is_empty(),
                                "functions suppress both success and failure"
                            );
                            assert!(game.side.exec.0.functions.is_empty());
                            game.ui.registries.permission = 4;
                            assert!(!game.run_command("unhandled"));
                            assert!(game.run_command(
                                "execute store result storage test:result value int 1 run origin"
                            ));
                            assert!(game.run_command(
                                "execute if data storage test:result {value:1} run origin"
                            ));
                        });
                    },
                );
            },
        );
    }

    #[test]
    #[ignore = "daily"]
    fn native_typed_commands_reach_listeners_after_parse_and_source_resolution() {
        sunrise_gameplay_common::game::in_own_process(
            "game::server_side::native_server_server::tests::native_typed_commands_reach_listeners_after_parse_and_source_resolution",
            || {
                use mod_api::{CommandArgumentValue as Value, CommandInvocation};
                struct Commands(Arc<Mutex<Vec<(bool, CommandInvocation)>>>);
                #[mod_api::__subscriptions]
                impl mod_api::Hooks for Commands {
                    fn register_commands(
                        &self,
                        _: &mut dyn mod_api::ServerHost,
                        event: &mut mod_api::RegisterCommandsEvent<'_>,
                    ) {
                        let heard = self.0.clone();
                        let record =
                            move |_: &mut dyn mod_api::ServerHost,
                                  source: &mod_api::CommandSource,
                                  command: &CommandInvocation| {
                                if command.literals == ["typed", "give"] {
                                    let Some(Value::Integer(amount)) = command.argument("amount")
                                    else {
                                        panic!("executor must receive an integer")
                                    };
                                    let Some(Value::Player(player)) = command.argument("who")
                                    else {
                                        panic!("executor must receive a resolved player")
                                    };
                                    assert_ne!(*amount, 0);
                                    assert_ne!(player.uuid(), 0);
                                }
                                let console =
                                    matches!(source, mod_api::CommandSource::Console { .. });
                                heard.lock().unwrap().push((console, command.clone()));
                                Ok(1)
                            };
                        event.commands.extend([
                            mod_api::Command::new("typed give <who:player> <amount:integer> [message:text]"),
                            mod_api::Command::new("typed data <word:word> <number:number> <enabled:boolean> <item:item> <pos:block_pos>"),
                            mod_api::Command::new("typed opt [value:integer]"),
                            mod_api::Command::new("/set <x:word>"),
                            mod_api::Command::new("typed repeat <value:player> <value:integer>"),
                            mod_api::Command::new("typed at <pos:vec3>"),
                            mod_api::Command::op("protected <who:player>", 4),
                        ].map(|command| command.executes(record.clone())));
                    }
                }
                let heard = Arc::new(Mutex::new(Vec::new()));
                mod_platform::testing::with_mods(
                    vec![mod_api::ModEntry::new(
                        "typed_commands",
                        Box::new(Commands(heard.clone())),
                    )]
                    .into(),
                    || {
                        super::super::native_guest_turn_tests::with_guests(|game| {
                            game.build_registries();
                            game.register_native_commands();
                            let other = game.player(13).unwrap();
                            let acting = game.player(12).unwrap();
                            game.as_guest(12, |game| {
                                for line in [
                                    "typed give guest13 nope",
                                    "typed give guest13",
                                    "typed give missing 7",
                                    "typed give @a 7",
                                    "typed data word 1 maybe stone ~ ~ ~",
                                    "typed data word NaN true stone ~ ~ ~",
                                    "typed data word 1 true does_not_exist ~ ~ ~",
                                    "typed opt 1 extra",
                                ] {
                                    assert_eq!(game.perform_native_command(12, 4, line, false), Some(false), "{line}");
                                }
                                let random = game.world.random.state();
                                assert_eq!(game.perform_native_command(12, 0, "protected @r", false), None);
                                assert_eq!(game.world.random.state(), random, "denied selectors must not resolve");
                                assert!(heard.lock().unwrap().is_empty(), "failed input must not reach listeners");
                                for line in [
                                    "typed give guest13 37 hello typed world",
                                    "typed give @s -4",
                                    "typed opt",
                                    "typed opt 9",
                                    "typed data word -1.25 true stone[minecraft:custom_name='\"Rock\"'] ~1 ~2 ~-3",
                                    "typed data word 2 false dirt ^0 ^0 ^0",
                                ] {
                                    assert_eq!(game.perform_native_command(12, 4, line, false), Some(true), "{line}");
                                }
                            }).unwrap();
                            game.spawn = Some(sunrise_core::geometry::Vec3::new(10.5, 70.0, -2.5));
                            game.yaw = 45.0;
                            game.pitch = 30.0;
                            for (line, key) in [
                                ("typed give guest13 nope", "parsing.int.expected"),
                                ("typed give missing 1", "argument.entity.notfound.player"),
                            ] {
                                game.side.chat_capture = Some(Vec::new());
                                assert!(game.perform_native_console_command(line));
                                let output = game.side.chat_capture.take().unwrap();
                                assert_eq!(
                                    output[0].plain(),
                                    Text::translate(key, &[]).plain(),
                                    "{line}"
                                );
                            }
                            assert!(game.perform_native_console_command("typed give @s 1"));
                            assert_eq!(
                                heard.lock().unwrap().len(),
                                6,
                                "console has no source player"
                            );
                            assert!(game.perform_native_console_command("typed give guest13 11"));
                            assert!(game.perform_native_console_command(
                                "typed data console 3 true dirt ~1 ~2 ~3"
                            ));
                            assert!(game.perform_native_console_command(
                                "typed data console 3 true dirt ^0.5 ^0.5 ^0.5"
                            ));
                            assert!(game.perform_native_console_command("typed repeat guest13 3"));
                            assert!(game.perform_native_console_command("typed at 1 64.25 ~1"));
                            // A point is three numbers: two, or text, is a parse error and is not heard
                            // (the count of 12 heard lines below holds neither).
                            game.perform_native_console_command("typed at 1 64.25");
                            game.perform_native_console_command("typed at 1 up 3");
                            assert!(
                                game.side.command_source.is_none(),
                                "console source restored"
                            );
                            game.as_guest(12, |game| {
                                assert!(
                                    game.native_run_command("execute as guest13 run typed opt 13")
                                )
                            })
                            .unwrap();
                            let observations = heard.clone();
                            let heard = heard.lock().unwrap();
                            assert_eq!(heard.len(), 12);
                            let first = &heard[0].1;
                            assert_eq!(first.literals, ["typed", "give"]);
                            assert_eq!(first.argument("who"), Some(&Value::Player(other)));
                            assert_eq!(first.argument("amount"), Some(&Value::Integer(37)));
                            assert_eq!(
                                first.argument("message"),
                                Some(&Value::String("hello typed world".into()))
                            );
                            assert_eq!(heard[1].1.argument("who"), Some(&Value::Player(acting)));
                            assert_eq!(heard[1].1.argument("amount"), Some(&Value::Integer(-4)));
                            assert!(heard[1].1.argument("message").is_none());
                            assert!(heard[2].1.arguments.is_empty());
                            assert_eq!(heard[3].1.argument("value"), Some(&Value::Integer(9)));
                            let data = &heard[4].1;
                            assert_eq!(data.argument("word"), Some(&Value::String("word".into())));
                            assert_eq!(data.argument("number"), Some(&Value::Number(-1.25)));
                            assert_eq!(data.argument("enabled"), Some(&Value::Boolean(true)));
                            assert_eq!(data.argument("pos"), Some(&Value::BlockPos([4, 66, 2])));
                            let Some(Value::Item(item)) = data.argument("item") else {
                                panic!("typed item")
                            };
                            assert_eq!(
                                mod_api::__private::item_key_id(item.item()),
                                Some("minecraft:stone")
                            );
                            assert_eq!(item.count(), 1);
                            assert!(item.components().is_some(), "command components retained");
                            assert_eq!(
                                heard[5].1.argument("enabled"),
                                Some(&Value::Boolean(false))
                            );
                            assert_eq!(
                                heard[5].1.argument("pos"),
                                Some(&Value::BlockPos([3, 64, 5]))
                            );
                            assert!(heard[..6].iter().all(|(console, _)| !console));
                            assert!(heard[6..].iter().all(|(console, _)| *console));
                            assert_eq!(heard[6].1.argument("who"), Some(&Value::Player(other)));
                            assert_eq!(heard[6].1.argument("amount"), Some(&Value::Integer(11)));
                            assert_eq!(
                                heard[7].1.argument("pos"),
                                Some(&Value::BlockPos([11, 72, 0]))
                            );
                            assert_eq!(
                                heard[8].1.argument("pos"),
                                Some(&Value::BlockPos([10, 70, -3]))
                            );
                            assert_eq!(heard[9].1.arguments[0].value, Value::Player(other));
                            assert_eq!(heard[9].1.argument("value"), Some(&Value::Integer(3)));
                            // `1 64.25 ~1`: a whole x is a block centre (1.5), y is as typed, and `~1`
                            // is one block from the console's z.
                            assert_eq!(
                                heard[10].1.argument("pos"),
                                Some(&Value::Vec3([1.5, 64.25, -2.0]))
                            );
                            drop(heard);
                            game.side
                                .lan
                                .as_mut()
                                .unwrap()
                                .hub
                                .update_guest(12, |g| g.permission = 4)
                                .unwrap();
                            let inventory = game
                                .side
                                .lan
                                .as_ref()
                                .unwrap()
                                .hub
                                .guest(12)
                                .unwrap()
                                .inventory
                                .clone();
                            game.guest_command(12, "/set stone", None);
                            game.guest_command(12, "typed opt 17", None);
                            game.guest_command(12, "/give guest12 minecraft:stone 1", None);
                            game.run_guest_commands();
                            let heard = observations.lock().unwrap();
                            assert_eq!(heard.len(), 14);
                            assert_eq!(heard[12].1.literals, ["/set"]);
                            assert_eq!(
                                heard[12].1.argument("x"),
                                Some(&Value::String("stone".into()))
                            );
                            assert_eq!(heard[13].1.literals, ["typed", "opt"]);
                            assert_eq!(heard[13].1.argument("value"), Some(&Value::Integer(17)));
                            assert_eq!(
                                game.side
                                    .lan
                                    .as_ref()
                                    .unwrap()
                                    .hub
                                    .guest(12)
                                    .unwrap()
                                    .inventory,
                                inventory
                            );
                        });
                    },
                );
            },
        );
    }
}

#[cfg(test)]
pub(crate) mod fake_action_tests {

use sunrise_gameplay_common::game::api_values::test_snapshot;

use sunrise_gameplay_common::game::native_server::fake_action_tests::on_top;

use sunrise_gameplay::game::native_server::fake_action_tests::fake;

use sunrise_gameplay::game::native_server::fake_action_tests::held;

use sunrise_gameplay::game::native_server::fake_action_tests::place;

use sunrise_gameplay::game::respawn_anchor::RespawnConfig;

use sunrise_gameplay_common::game::stations_common::{Kind, Station};

use sunrise_gameplay_common::game::native_server::Acting;

use mod_api::{ActionOutcome, ActionRefused, FakeEffect, PlayerAction};

use crate::game::native_server::fake_action_tests::fake;

use crate::game::native_server::fake_action_tests::held;

use crate::game::native_server::fake_action_tests::on_top;

use crate::game::native_server::fake_action_tests::place;

use mod_api::{BlockHit, Hand};

use sunrise_items::items::ItemStack;

#[test]
    #[ignore = "daily"]
    fn a_fakes_action_is_refused_before_anything_runs() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, ..) = fake(game, Some("minecraft:water_bucket"), 0);
            // Out of reach: nothing used, the bucket still full.
            let far = PlayerAction::UseOn {
                hand: Hand::Main,
                pos: [40, 63, 0],
                hit: BlockHit::new([40.5, 64.0, 0.5], [0, 1, 0], false),
            };
            assert_eq!(
                game.player_perform_action(player, far),
                Err(ActionRefused::OutOfReach)
            );
            assert_eq!(
                held(game, 2).map(|s| s.item.to_string()).as_deref(),
                Some("minecraft:water_bucket")
            );
            // A real player's handle.
            let real = game.player(1).unwrap();
            assert_eq!(
                game.player_perform_action(real, on_top([1, 63, 0])),
                Err(ActionRefused::NotFake)
            );
            // Inside another action, as a hook running during it would ask.
            let acting = Acting::start(&game.side.fake_action.0);
            assert_eq!(
                game.player_perform_action(player, on_top([1, 63, 0])),
                Err(ActionRefused::Busy)
            );
            drop(acting);
            assert!(!game.side.fake_action.0.acting());
        });
    }
#[test]
    #[ignore = "daily"]
    fn a_fakes_action_leaves_the_per_action_state_as_it_was() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, ..) = fake(game, Some("minecraft:oak_log"), 0);
            let before = sunrise_gameplay_common::game::Hit {
                pos: [7, 7, 7],
                face: [1, 0, 0],
                point: [7.0, 7.5, 7.5],
                inside: true,
            };
            game.side.forced_hit = Some(before);
            assert_eq!(
                game.player_perform_action(player, on_top([1, 63, 0])),
                Ok(ActionOutcome::Consumed)
            );
            let after = game.side.forced_hit.expect("the forced hit is put back");
            assert_eq!(
                (after.pos, after.face, after.point, after.inside),
                (before.pos, before.face, before.point, before.inside)
            );
            assert!(game.side.guest_use.is_none());
        });
    }
/// A fake player's click on a shulker box or an ender chest is refused
    /// at the open: no open count climbs, no stand-in chest enters the level
    /// and no lid opens.
    #[test]
    #[ignore = "daily"]
    fn a_shulker_box_and_an_ender_chest_a_fake_player_used_are_left_shut() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, ..) = fake(game, None, 0);
            let shulker = [1, 64, 0];
            place(game, shulker, "minecraft:shulker_box");
            let count = |game: &sunrise_gameplay::game::ServerGame<'_>| {
                game.server
                    .be_anims
                    .shulkers
                    .get(&shulker)
                    .map(|s| s.open_count)
            };
            assert_eq!(
                game.player_perform_action(player, on_top(shulker)),
                Ok(ActionOutcome::EffectBlocked(FakeEffect::Menu))
            );
            assert!(
                count(game).is_none_or(|c| c == 0),
                "the use is refused at the open, nothing opened"
            );
            assert!(game.container.is_none());
            assert_eq!(
                game.player_perform_action(player, on_top(shulker)),
                Ok(ActionOutcome::EffectBlocked(FakeEffect::Menu)),
                "the next use is refused the same way",
            );
            assert!(
                count(game).is_none_or(|c| c == 0),
                "the count doesn't climb"
            );

            let diamonds = Some(ItemStack::new("minecraft:diamond", 5));
            game.side.ender_items[0] = diamonds.clone();
            let ender = [2, 64, 0];
            place(game, ender, "minecraft:ender_chest");
            let key = (ender[0], ender[1], ender[2]);
            // A raw block state has no stand-in. Establish the closed state
            // whose preservation this refused action is meant to check.
            game.open_ender_chest(key);
            game.close_ender_chest();
            game.container = None;
            assert_eq!(
                game.player_perform_action(player, on_top(ender)),
                Ok(ActionOutcome::EffectBlocked(FakeEffect::Menu))
            );
            assert!(game.side.ender_open.is_none());
            match game.world.block_entities.get(&key) {
                Some(sunrise_level::chunk::block_entity::BlockEntity::Chest(items)) => assert!(
                    items.iter().all(Option::is_none),
                    "the player's items stayed"
                ),
                other => panic!("the ender chest lost its block entity: {other:?}"),
            }
            assert_eq!(
                game.side.ender_items[0], diamonds,
                "the server's own ender items changed"
            );
            assert!(
                !game.side.lid_openers.contains_key(&key),
                "its lid stayed open"
            );
        });
    }
#[test]
    #[ignore = "daily"]
    fn scoped_fake_use_hooks_see_the_setup_and_fail_is_not_consumed() {
        use mod_api::{Flow, Hooks, HostSide, PlayerActionSetup, RightClickBlockEvent};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        #[derive(Default)]
        struct Deny(Arc<AtomicBool>);
        #[mod_api::sunrise_mod]
        impl Hooks for Deny {
            fn right_click_block(&self, host: HostSide, event: &mut RightClickBlockEvent) -> Flow {
                let HostSide::Server(host) = host else {
                    panic!("server action");
                };
                assert_eq!(event.click.player, 2);
                let entity = host.entity(event.click.player_entity_id as u32).unwrap();
                assert_eq!(entity.rotation(host), Some([123.0, 17.0]));
                assert_eq!(
                    entity.eye_position(host),
                    Some([0.5, 64.0 + f64::from(1.62_f32), 0.5])
                );
                assert_eq!(entity.is_shift_key_down(host), Some(true));
                self.0.store(true, Ordering::Relaxed);
                *event.cancellation_result = mod_api::InteractionResult::Fail;
                Flow::Cancel
            }
        }
        let heard = Arc::new(AtomicBool::new(false));
        mod_platform::testing::with_mods(
            vec![mod_api::ModEntry::new(
                "deny",
                Box::new(Deny(heard.clone())),
            )]
            .into(),
            || {
                super::super::native_entity_refs::tests::with_game(|game| {
                    let (player, ..) = fake(game, Some("minecraft:diamond"), 0);
                    game.side.use_failed = true;
                    let result = player
                        .perform_action_scoped(
                            game,
                            on_top([1, 63, 0]),
                            PlayerActionSetup::new(
                                Some(sunrise_gameplay_common::game::api_values::test_snapshot(&ItemStack::new(
                                    "minecraft:oak_log",
                                    1,
                                ))),
                                [0.5, 64.0 + f64::from(1.62_f32), 0.5],
                                123.0,
                                17.0,
                                true,
                            ),
                        )
                        .unwrap();
                    assert!(
                        heard.load(Ordering::Relaxed),
                        "normal Host-bearing hook ran"
                    );
                    assert_eq!(result.outcome, ActionOutcome::Passed);
                    assert_eq!(
                        result.held,
                        Some(sunrise_gameplay_common::game::api_values::test_snapshot(&ItemStack::new(
                            "minecraft:oak_log",
                            1
                        )))
                    );
                    assert!(game.side.use_failed, "outer per-action flag restored");
                    let level = game.level(game.dim.id()).unwrap();
                    assert_eq!(
                        game.level_block(level, [1, 64, 0])
                            .and_then(mod_api::__private::block_key_id),
                        Some("minecraft:air")
                    );
                    assert_eq!(held(game, 2), Some(ItemStack::new("minecraft:diamond", 1)));
                });
            },
        );
    }
#[test]
    #[ignore = "daily"]
    fn scoped_fake_use_uses_viewpoint_secondary_use_and_restores_on_refusal() {
        use mod_api::PlayerActionSetup;
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, conn, _) = fake(game, Some("minecraft:diamond"), 1);
            let setup = |item, sneaking| {
                PlayerActionSetup::new(
                    Some(sunrise_gameplay_common::game::api_values::test_snapshot(&ItemStack::new(
                        item, 1,
                    ))),
                    [0.5, 64.0 + f64::from(1.62_f32), 0.5],
                    0.0,
                    90.0,
                    sneaking,
                )
            };
            // Air use rays from the supplied eyes/look, not the persistent fake's view.
            let mut viewpoint = setup("minecraft:water_bucket", false);
            viewpoint.eye = [2.5, 64.0 + f64::from(1.62_f32), 2.5];
            viewpoint.yaw = 117.0;
            let result = game
                .player_perform_action_scoped(
                    player,
                    PlayerAction::UseItem { hand: Hand::Main },
                    viewpoint,
                )
                .unwrap();
            assert_eq!(result.outcome, ActionOutcome::Consumed);
            assert_eq!(
                result.held,
                Some(sunrise_gameplay_common::game::api_values::test_snapshot(&ItemStack::new(
                    "minecraft:bucket",
                    1
                )))
            );
            let level = game.level(game.dim.id()).unwrap();
            assert_eq!(
                game.level_block(level, [2, 64, 2])
                    .and_then(mod_api::__private::block_key_id),
                Some("minecraft:water")
            );
            place(game, [1, 64, 0], "minecraft:chest");
            let result = game
                .player_perform_action_scoped(
                    player,
                    on_top([1, 64, 0]),
                    setup("minecraft:oak_log", false),
                )
                .unwrap();
            assert_eq!(
                result.outcome,
                ActionOutcome::EffectBlocked(FakeEffect::Menu)
            );
            let result = game
                .player_perform_action_scoped(
                    player,
                    on_top([1, 64, 0]),
                    setup("minecraft:oak_log", true),
                )
                .unwrap();
            assert_eq!(result.outcome, ActionOutcome::Consumed);
            assert_eq!(
                result.held, None,
                "survival placement consumes the temporary stack"
            );
            assert_eq!(
                game.level_block(level, [1, 65, 0])
                    .and_then(mod_api::__private::block_key_id),
                Some("minecraft:oak_log")
            );
            assert_eq!(
                game.player_perform_action_scoped(
                    player,
                    on_top([40, 63, 0]),
                    setup("minecraft:oak_log", false)
                ),
                Err(ActionRefused::OutOfReach)
            );
            let mut invalid = setup("minecraft:oak_log", false);
            invalid.eye[0] = f64::NAN;
            assert_eq!(
                game.player_perform_action_scoped(player, on_top([1, 63, 0]), invalid),
                Err(ActionRefused::InvalidSetup)
            );
            let real = game.player(1).unwrap();
            assert_eq!(
                game.player_perform_action_scoped(
                    real,
                    on_top([1, 63, 0]),
                    setup("minecraft:oak_log", false)
                ),
                Err(ActionRefused::NotFake)
            );
            let acting = Acting::start(&game.side.fake_action.0);
            assert_eq!(
                game.player_perform_action_scoped(
                    player,
                    on_top([1, 63, 0]),
                    setup("minecraft:oak_log", false)
                ),
                Err(ActionRefused::Busy)
            );
            drop(acting);
            assert_eq!(held(game, 2), Some(ItemStack::new("minecraft:diamond", 1)));
            let guest = game.side.lan.as_ref().unwrap().hub.guest(conn).unwrap();
            assert_eq!(guest.game_mode, 1);
            assert_eq!(guest.position, [0.5, 64.0, 0.5]);
        });
    }
/// Each field holds a value the use can't have written, so a swap that
    /// forgets one fails whether the use changed it or not.
    #[test]
    #[ignore = "daily"]
    fn the_servers_own_player_state_comes_back_after_a_fake_use() {
        use mod_api::PlayerActionSetup;
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, ..) = fake(game, None, 0);
            let dim = game.dim;
            let config = RespawnConfig {
                dim,
                pos: [1, 2, 3],
                yaw: 0.0,
                pitch: 0.0,
                forced: false,
            };
            let spawn = sunrise_core::geometry::Vec3::new(1.0, 2.0, 3.0);
            game.station = Some(Station::new_kind(Kind::Anvil, [4, 5, 6]));
            game.side.craft_table = Some([7, 8, 9]);
            game.experience.level = 7;
            game.side.respawn_config = Some(config);
            game.spawn = Some(spawn);
            // An ender inventory left open goes back while the fake player
            // acts, and its stand-in returns afterwards, as a guest's turn does.
            let stand_in = (12, 64, 8);
            game.open_ender_chest(stand_in);
            game.container = None;

            place(game, [1, 64, 0], "minecraft:crafting_table");
            assert_eq!(
                game.player_perform_action(player, on_top([1, 64, 0])),
                Ok(ActionOutcome::EffectBlocked(FakeEffect::Menu))
            );
            assert_eq!(
                game.station.as_ref().map(|s| (s.kind, s.pos)),
                Some((Kind::Anvil, [4, 5, 6])),
            );
            assert_eq!(game.side.craft_table, Some([7, 8, 9]));
            assert_eq!(game.experience.level, 7);
            assert_eq!(game.side.respawn_config, Some(config));
            assert_eq!(game.spawn, Some(spawn));
            assert_eq!(game.side.ender_open, Some(stand_in));
            assert!(game.world.block_entities.get(&stand_in).is_some());

            let setup = PlayerActionSetup::new(
                Some(sunrise_gameplay_common::game::api_values::test_snapshot(&ItemStack::new(
                    "minecraft:fishing_rod",
                    1,
                ))),
                [0.5, 64.0 + f64::from(1.62_f32), 0.5],
                0.0,
                0.0,
                false,
            );
            let rod = player
                .perform_action_scoped(game, PlayerAction::UseItem { hand: Hand::Main }, setup)
                .unwrap();
            assert_eq!(rod.outcome, ActionOutcome::Consumed, "the rod casts");
            assert!(game.hook.is_none(), "no hook is left for nobody to reel in");
        });
    }
}

use sunrise_gameplay_common::game::chat_common::ChatCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_server::NativeServerGameExt1 as _;
