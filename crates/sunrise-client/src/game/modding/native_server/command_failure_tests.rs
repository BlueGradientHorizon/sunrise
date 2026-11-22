use crate::game::{
    in_own_process, lan_actions::GuestCommand, native_guest_turn_tests::with_guests,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sunrise_client::{
    gamerules,
    lan::{Out, PlayerSender},
    text,
};
use sunrise_protocol::{
    buf::Reader, ids::play::clientbound::SYSTEM_CHAT, packets::play::Clientbound,
};

struct FailingCommand(Arc<AtomicUsize>);

#[mod_api::__subscriptions]
impl mod_api::Hooks for FailingCommand {
    fn register_commands(
        &self,
        _: &mut dyn mod_api::ServerHost,
        event: &mut mod_api::RegisterCommandsEvent<'_>,
    ) {
        let calls = self.0.clone();
        event.commands.push(
            mod_api::Command::new("rejecttest").executes(move |_, _, _| {
                calls.fetch_add(1, Ordering::Relaxed);
                Err(mod_api::Text::literal("No room for another item").into())
            }),
        );
        for (usage, depth) in [
            ("rejectinvalid", mod_api::text::MAX_TEXT_DEPTH + 1),
            ("rejectformatted", mod_api::text::MAX_TEXT_DEPTH - 1),
        ] {
            event
                .commands
                .push(mod_api::Command::new(usage).executes(move |_, _, _| {
                    let mut message = mod_api::Text::literal("unserializable failure");
                    for _ in 1..depth {
                        message = mod_api::Text::literal("").append(message);
                    }
                    Err(message.into())
                }));
        }
    }
}

#[test]
#[ignore = "daily"]
fn executor_failure_respects_source_silence_and_stores_zero() {
    in_own_process(
        "game::native_server::command_failure_tests::executor_failure_respects_source_silence_and_stores_zero",
        || {
            let calls = Arc::new(AtomicUsize::new(0));
            mod_platform::testing::with_mods(
                vec![mod_api::ModEntry::new(
                    "failure_test",
                    Box::new(FailingCommand(calls.clone())),
                )]
                .into(),
                || {
                    with_guests(|game| {
                        game.build_registries();
                        game.register_native_commands();
                        assert!(
                            game.ui
                                .rules
                                .set("send_command_feedback", gamerules::Value::Bool(false),)
                        );
                        let command = |line: &str, quiet| GuestCommand {
                            conn: 12,
                            command: line.into(),
                            permission: 2,
                            integrated: true,
                            quiet,
                        };

                        let (ok, chat) = game
                            .run_guest_command_told(command("rejecttest", false))
                            .unwrap();
                        assert!(!ok);
                        assert_eq!(chat.len(), 1);
                        assert_eq!(chat[0].plain(), "No room for another item");
                        assert_eq!(chat[0].spans[0].style.color, Some(text::RED));
                        assert_eq!(calls.load(Ordering::Relaxed), 1);

                        let (ok, chat) = game
                            .run_guest_command_told(command("rejecttest", true))
                            .unwrap();
                        assert!(!ok);
                        assert!(chat.is_empty());
                        assert_eq!(calls.load(Ordering::Relaxed), 2);

                        // The guest route serializes these lines to SystemChat.
                        let ((ok, chat), outcome) = game.recording_command_outcome(|game| {
                            game.run_guest_command_told(command("rejectinvalid", false))
                                .unwrap()
                        });
                        assert!(!ok);
                        assert_eq!(outcome, Some((false, 0)));
                        assert_eq!(chat.len(), 1);
                        assert_eq!(
                            chat[0].plain(),
                            text::Text::tr("command.failed", &[]).plain()
                        );

                        // The raw API component at this depth passes validation, but
                        // its encoded `extra` lists and the red parent exceed wire depth.
                        let (tx, rx) = std::sync::mpsc::channel();
                        game.lan.as_mut().unwrap().hub.update_guest(12, |guest| {
                            guest.sender = PlayerSender::new(tx, Arc::default());
                        });
                        let (ok, outcome) = game.recording_command_outcome(|game| {
                            game.run_guest_command(command("rejectformatted", false))
                        });
                        assert!(!ok);
                        assert_eq!(outcome, Some((false, 0)));
                        let received: Vec<_> = rx
                            .try_iter()
                            .flat_map(|out| match out {
                                Out::Packet(body) => vec![body],
                                Out::Packets(bodies) => bodies,
                                Out::Disconnect(_) => Vec::new(),
                            })
                            .filter_map(|body| {
                                let mut reader = Reader::new(&body);
                                (reader.varint().unwrap() == SYSTEM_CHAT).then(|| {
                                    let Clientbound::SystemChat(chat) =
                                        Clientbound::decode(SYSTEM_CHAT, reader.rest()).unwrap()
                                    else {
                                        unreachable!()
                                    };
                                    crate::game::remote::remote_text(&chat.content).plain()
                                })
                            })
                            .collect();
                        assert_eq!(received, [text::Text::tr("command.failed", &[]).plain()]);

                        assert!(game.run_guest_command(command(
                            "data merge storage test:out {success:42,result:42}",
                            true,
                        )));
                        assert!(!game.run_guest_command(command(
                            "execute store success storage test:out success int 1 \
                             store result storage test:out result int 1 run rejecttest",
                            true,
                        )));
                        assert!(game.run_guest_command(command(
                            "execute if data storage test:out {success:0,result:0}",
                            true,
                        )));
                    });
                },
            );
        },
    );
}
