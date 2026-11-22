use crate::game::{in_own_process, native_guest_turn_tests::with_guests};
use sunrise_client::{gamerules, text};

struct FailingCommand;

#[mod_api::__subscriptions]
impl mod_api::Hooks for FailingCommand {
    fn register_commands(
        &self,
        _: &mut dyn mod_api::ServerHost,
        event: &mut mod_api::RegisterCommandsEvent<'_>,
    ) {
        event.commands.push(
            mod_api::Command::new("rejecttest")
                .executes(|_, _, _| Err(mod_api::Text::literal("No room for another item"))),
        );
    }
}

#[test]
#[ignore = "daily"]
fn executor_failure_respects_source_silence_and_stores_zero() {
    in_own_process(
        "game::native_server::command_failure_tests::executor_failure_respects_source_silence_and_stores_zero",
        || {
            mod_platform::testing::with_mods(
                vec![mod_api::ModEntry::new(
                    "failure_test",
                    Box::new(FailingCommand),
                )]
                .into(),
                || {
                    with_guests(|game| {
                        game.build_registries();
                        game.register_native_commands();
                        game.as_guest(12, |game| {
                            game.ui.registries.permission = 2;
                            assert!(game.ui.rules.set(
                                "send_command_feedback",
                                gamerules::Value::Bool(false),
                            ));
                            game.chat_capture = Some(Vec::new());

                            assert!(!game.run_command("rejecttest"));
                            let chat = game.chat_capture.as_mut().unwrap();
                            assert_eq!(chat.len(), 1);
                            assert_eq!(chat[0].plain(), "No room for another item");
                            assert_eq!(chat[0].spans[0].style.color, Some(text::RED));
                            chat.clear();

                            assert!(!game.run_command_quietly("rejecttest"));
                            assert!(game.chat_capture.as_ref().unwrap().is_empty());

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
