//! `Commands::feedback(line, broadcast_to_admins)`: vanilla's
//! `CommandSourceStack.sendSuccess(line, true)`, whose `broadcastToAdmins` tells the other
//! operators (`PlayerList.isOp`) under `sendCommandFeedback` and the server log under
//! `logAdminCommands` (#4632).
use super::Game;
use mod_api::{Command, PermissionLevel, RegisterCommandsEvent, ServerHost, Text};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicI32, AtomicUsize},
    mpsc,
};
use sunrise_client::gamerules::Value as Rule;
use sunrise_client::lan::{Joined, Out, PlayerSender, guest::Guest};
use sunrise_protocol::packets::{configuration::ClientInformation, play::Clientbound};

/// The source (an operator), the `execute as` target (an operator), an operator listed at
/// level 0 and a player who is not an operator.
const SOURCE: u32 = 20;
const TARGET: u32 = 21;
const LEVEL_0_OP: u32 = 22;
const NON_OP: u32 = 23;
const PLAYERS: [(u32, &str); 4] = [
    (SOURCE, "Alex"),
    (TARGET, "Bea"),
    (LEVEL_0_OP, "Cid"),
    (NON_OP, "Dan"),
];

struct Announce;

#[mod_api::__subscriptions]
impl mod_api::Hooks for Announce {
    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.extend([
            // FTB Teams' `source.sendSuccess(() -> msg, true)`.
            Command::new("announce").executes(|host, _, _| {
                host.commands()
                    .feedback(Text::literal("Disbanded team"), true)
                    .unwrap();
                Ok(1)
            }),
            // A query's `sendSuccess(msg, false)`.
            Command::new("announce_quietly").executes(|host, _, _| {
                host.commands()
                    .feedback(Text::literal("Disbanded team"), false)
                    .unwrap();
                Ok(1)
            }),
        ]);
    }
}

/// A dedicated server with the `announce` commands and the four players of `PLAYERS`, whose
/// chat each test reads (by conn).
fn with_operators(test: impl FnOnce(&mut Game<'_>, &[(u32, mpsc::Receiver<Out>)])) {
    mod_platform::testing::with_mods(
        vec![mod_api::ModEntry::new("announce", Box::new(Announce))].into(),
        || {
            super::native_guest_turn_tests::with_guests(|game| {
                let mut receivers = Vec::new();
                // A dedicated server: its players' commands run at their ops.json level.
                game.set_allow_commands(true);
                let host = game.lan.as_mut().unwrap();
                host.settings.guest_commands = true;
                let hub = &mut host.hub;
                hub.config.integrated = false;
                for (conn, name) in PLAYERS {
                    let (tx, rx) = mpsc::channel();
                    let mut guest = Guest::new(
                        Joined {
                            conn,
                            uuid: u128::from(conn),
                            name: name.into(),
                            properties: Vec::new(),
                            info: ClientInformation::new(8),
                            brand: String::new(),
                            sender: PlayerSender::new(tx, Arc::new(AtomicUsize::new(0))),
                            latency: Arc::new(AtomicI32::new(0)),
                            address: "127.0.0.1:1".parse().unwrap(),
                            expected: false,
                        },
                        conn as i32,
                        [3.0, 64.0, 5.0],
                        10,
                    );
                    guest.placed = true;
                    // ops.json: two level-4 operators and one listed at level 0.
                    let level = match conn {
                        SOURCE | TARGET => Some(4),
                        LEVEL_0_OP => Some(0),
                        _ => None,
                    };
                    guest.permission = level.unwrap_or(0);
                    if let Some(level) = level {
                        hub.ops.insert(u128::from(conn), level);
                    }
                    hub.guests.push(guest);
                    receivers.push((conn, rx));
                }
                game.next_entity = game.next_entity.max(NON_OP);
                game.ensure_datapacks();
                game.build_registries();
                game.register_native_commands();
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)));
                // A dedicated server's log (`dedicated::log`), which the tests read.
                game.options.execution_role =
                    crate::game::execution_role::ExecutionRole::DedicatedServer;
                assert!(
                    game.native_run_command("announce"),
                    "the mod's command is registered"
                );
                test(game, &receivers);
            });
        },
    );
}

/// The chat components each player was sent since the last read, by conn.
fn chats(game: &Game<'_>, receivers: &[(u32, mpsc::Receiver<Out>)]) -> Vec<(u32, Vec<Value>)> {
    for g in &game.lan.as_ref().unwrap().hub.guests {
        g.sender.flush();
    }
    receivers
        .iter()
        .map(|(conn, rx)| {
            let mut lines = Vec::new();
            for sent in rx.try_iter() {
                let bodies = match sent {
                    Out::Packet(p) => vec![p],
                    Out::Packets(p) => p,
                    Out::Disconnect(_) => panic!("unexpected disconnect"),
                };
                for body in bodies {
                    let mut reader = sunrise_protocol::buf::Reader::new(&body);
                    let id = reader.varint().unwrap();
                    if let Clientbound::SystemChat(p) =
                        Clientbound::decode(id, reader.rest()).unwrap()
                    {
                        lines.push(sunrise_client::scoreboard::tag_to_json(&p.content));
                    }
                }
            }
            (*conn, lines)
        })
        .collect()
}

/// The `[name: Disbanded team]` line every operator gets: `chat.type.admin`, gray and italic,
/// with the display name of the `execute as` entity, reduced to the fields compared.
fn admin(name: &str) -> Value {
    json!({
        "translate": "chat.type.admin",
        "name": name,
        "message": {"text": "Disbanded team"},
        "color": "gray",
        "italic": true,
    })
}

/// A sent line as compared: an admin line's display name as its text alone (a player's name
/// also carries its hover and click events), any other line as sent.
fn shape(line: &Value) -> Value {
    if line["translate"] != "chat.type.admin" {
        return line.clone();
    }
    let with = line["with"]
        .as_array()
        .expect("chat.type.admin has two arguments");
    assert_eq!(with.len(), 2, "{line}");
    json!({
        "translate": line["translate"],
        "name": with[0]["text"],
        "message": with[1],
        "color": line["color"],
        "italic": line["italic"],
    })
}

fn set_rules(game: &mut Game<'_>, send_command_feedback: bool, log_admin_commands: bool) {
    assert!(
        game.ui
            .rules
            .set("send_command_feedback", Rule::Bool(send_command_feedback))
    );
    assert!(
        game.ui
            .rules
            .set("log_admin_commands", Rule::Bool(log_admin_commands))
    );
}

/// The `broadcastToAdmins` lines the server logged while `f` ran.
fn admin_log(f: impl FnOnce()) -> Vec<String> {
    crate::game::dedicated::capture_log(f)
        .into_iter()
        .filter(|line| line.starts_with("System chat: ["))
        .collect()
}

/// `execute as` another operator: the source hears the line under `sendCommandFeedback`;
/// every operator but the source (the `execute as` player and the level-0 operator, never the
/// non-operator) gets `[Bea: ...]` under `sendCommandFeedback`; the log gets it under
/// `logAdminCommands`; neither rule gates the other. `broadcast_to_admins` false and a silent
/// source tell no one.
#[test]
#[ignore = "daily"]
fn feedback_tells_the_other_operators_and_the_log_under_their_rules() {
    crate::game::in_own_process(
        "game::native_success_broadcast_tests::feedback_tells_the_other_operators_and_the_log_under_their_rules",
        || {
            with_operators(|game, receivers| {
                chats(game, receivers);
                for (feedback, log) in [(true, true), (true, false), (false, true), (false, false)]
                {
                    set_rules(game, feedback, log);
                    let logged = admin_log(|| {
                        game.guest_command(SOURCE, "execute as Bea run announce", None);
                        game.run_guest_commands();
                    });
                    let told = |admin_line: bool| {
                        if admin_line && feedback {
                            vec![admin("Bea")]
                        } else {
                            Vec::new()
                        }
                    };
                    let own = if feedback {
                        vec![json!({"text": "Disbanded team"})]
                    } else {
                        Vec::new()
                    };
                    let sent: Vec<(u32, Vec<Value>)> = chats(game, receivers)
                        .into_iter()
                        .map(|(conn, lines)| (conn, lines.iter().map(shape).collect()))
                        .collect();
                    assert_eq!(
                        sent,
                        vec![
                            (SOURCE, own),
                            (TARGET, told(true)),
                            (LEVEL_0_OP, told(true)),
                            (NON_OP, told(false)),
                        ],
                        "sendCommandFeedback {feedback}, logAdminCommands {log}"
                    );
                    let expected_log = if log {
                        vec!["System chat: [Bea: Disbanded team]".to_owned()]
                    } else {
                        Vec::new()
                    };
                    assert_eq!(
                        logged, expected_log,
                        "sendCommandFeedback {feedback}, logAdminCommands {log}"
                    );
                }

                // The player's own command: `[Alex: ...]`, never back to Alex.
                set_rules(game, true, true);
                let logged = admin_log(|| {
                    game.guest_command(SOURCE, "announce", None);
                    game.run_guest_commands();
                });
                assert_eq!(
                    logged,
                    vec!["System chat: [Alex: Disbanded team]".to_owned()]
                );
                let sent: Vec<(u32, Vec<Value>)> = chats(game, receivers)
                    .into_iter()
                    .map(|(conn, lines)| (conn, lines.iter().map(shape).collect()))
                    .collect();
                assert_eq!(
                    sent,
                    vec![
                        (SOURCE, vec![json!({"text": "Disbanded team"})]),
                        (TARGET, vec![admin("Alex")]),
                        (LEVEL_0_OP, vec![admin("Alex")]),
                        (NON_OP, Vec::new()),
                    ]
                );

                // `broadcast_to_admins` false: the source alone, nothing logged.
                let logged = admin_log(|| {
                    game.guest_command(SOURCE, "announce_quietly", None);
                    game.run_guest_commands();
                });
                assert_eq!(logged, Vec::<String>::new());
                assert_eq!(
                    chats(game, receivers),
                    vec![
                        (SOURCE, vec![json!({"text": "Disbanded team"})]),
                        (TARGET, Vec::new()),
                        (LEVEL_0_OP, Vec::new()),
                        (NON_OP, Vec::new()),
                    ]
                );

                // A silent source (`withSuppressedOutput`) sends neither part.
                let alex = game.player(u128::from(SOURCE)).unwrap().entity();
                let logged = admin_log(|| {
                    assert!(alex.run_command(game, "announce", PermissionLevel::Owners, true));
                });
                assert_eq!(logged, Vec::<String>::new());
                assert!(
                    chats(game, receivers)
                        .iter()
                        .all(|(_, lines)| lines.is_empty()),
                    "a silent source told someone"
                );
            });
        },
    );
}

/// A command block that tracks output informs the operators under `commandBlockOutput`: the
/// log records `[@: ...]` while `sendCommandFeedback` is off (and the block keeps no output);
/// with `commandBlockOutput` off only the block's own output has the line.
#[test]
#[ignore = "daily"]
fn a_tracking_command_block_is_logged_while_feedback_is_off() {
    crate::game::in_own_process(
        "game::native_success_broadcast_tests::a_tracking_command_block_is_logged_while_feedback_is_off",
        || {
            use sunrise_client::{chunk::block_entity::BlockEntity, command_block::CommandBlock};
            use sunrise_core::position::BlockPos;
            with_operators(|game, receivers| {
                let state = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:command_block")
                    .unwrap();
                game.world.set(1, 64, 1, state).unwrap();
                let last_output = |game: &Game<'_>| match game.world.block_entities.get(&(1, 64, 1))
                {
                    Some(BlockEntity::CommandBlock(block)) => {
                        block.last_output.as_ref().map(|line| line.plain())
                    }
                    _ => panic!("the command block is gone"),
                };
                let run = |game: &mut Game<'_>| {
                    game.world.block_entities.insert(
                        (1, 64, 1),
                        BlockEntity::CommandBlock(Box::new(CommandBlock {
                            command: "announce".into(),
                            condition_met: true,
                            track_output: true,
                            ..Default::default()
                        })),
                    );
                    admin_log(|| game.command_block_tick(BlockPos::new(1, 64, 1)))
                };
                chats(game, receivers);

                set_rules(game, false, true);
                assert!(game.ui.rules.set("command_block_output", Rule::Bool(true)));
                assert_eq!(
                    run(game),
                    vec!["System chat: [@: Disbanded team]".to_owned()]
                );
                assert_eq!(last_output(game), None);
                game.send_admin_feedback(None);
                assert!(
                    chats(game, receivers)
                        .iter()
                        .all(|(_, lines)| lines.is_empty()),
                    "an operator heard the block with sendCommandFeedback off"
                );

                // The block does not inform the operators: its own output only.
                set_rules(game, true, true);
                assert!(game.ui.rules.set("command_block_output", Rule::Bool(false)));
                assert_eq!(run(game), Vec::<String>::new());
                assert!(
                    last_output(game).is_some_and(|line| line.ends_with("Disbanded team")),
                    "{:?}",
                    last_output(game)
                );
                game.send_admin_feedback(None);
                assert!(
                    chats(game, receivers)
                        .iter()
                        .all(|(_, lines)| lines.is_empty())
                );
            });
        },
    );
}
