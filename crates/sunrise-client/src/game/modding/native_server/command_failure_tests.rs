use crate::game::{
    in_own_process, lan_actions::GuestCommand, native_guest_turn_tests::with_guests,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sunrise_client::{gamerules, text};

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
                        let command = |quiet| GuestCommand {
                            conn: 12,
                            command: "rejecttest".into(),
                            permission: 2,
                            integrated: true,
                            quiet,
                        };

                        let (ok, chat) = game.run_guest_command_told(command(false)).unwrap();
                        assert!(!ok);
                        assert_eq!(chat.len(), 1);
                        assert_eq!(chat[0].plain(), "No room for another item");
                        assert_eq!(chat[0].spans[0].style.color, Some(text::RED));
                        assert_eq!(calls.load(Ordering::Relaxed), 1);

                        let (ok, chat) = game.run_guest_command_told(command(true)).unwrap();
                        assert!(!ok);
                        assert!(chat.is_empty());
                        assert_eq!(calls.load(Ordering::Relaxed), 2);

                        game.as_guest(12, |game| {
                            game.ui.registries.permission = 2;
                            assert!(game.run_command("data modify storage test:out success set value 42"));
                            assert!(!game.run_command_quietly(
                                "execute store success storage test:out success int 1 run rejecttest",
                            ));
                            assert!(game.run_command("execute if data storage test:out {success:0}"));
                        })
                        .unwrap();
                    });
                },
            );
        },
    );
}
