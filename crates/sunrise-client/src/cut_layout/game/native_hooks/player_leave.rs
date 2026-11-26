
#[cfg(test)]
pub(in crate::game) mod player_leave_tests {
    use sunrise_gameplay::game::native_entity_refs::tests::run_fresh_cases;
    use mod_api::{Hooks, PlayerBedLeaveEvent, PlayerLoggedOutEvent, Registrar, ServerHost};
    use std::sync::{Arc, Mutex};
    use sunrise_core::nbt::{Compound, Tag};
    use sunrise_mod_runtime::attachments::{
        AttachmentCodec, AttachmentError, AttachmentType, Holder,
    };

    /// Each case runs in a fresh child process: the fixture loads the prepared resources into
    /// the process-wide tables (languages, POIs, cauldron interactions).
    pub(in crate::game) const FLAG: &str = "SUNRISE_PLAYER_LEAVE_CHILD";
    pub(in crate::game) const SAVED: &str = "game::server_side::native_hooks_server::player_leave::player_leave_tests::what_a_listener_changes_on_a_leaving_player_is_saved";
    pub(in crate::game) const ASLEEP: &str = "game::native_hooks::player_leave::player_leave_tests::a_sleeping_player_wakes_before_logging_out";

    pub(in crate::game) const VALUE: AttachmentType<i32> = AttachmentType {
        namespace: "leave_test",
        name: "value",
        default: || 0,
        codec: Some(AttachmentCodec {
            encode: |n| Ok(Compound::from([("n".into(), Tag::Int(*n))])),
            decode: |data| match data.get(&"n".into()) {
                Some(Tag::Int(n)) => Ok(*n),
                _ => Err(AttachmentError::Decode),
            },
        }),
        copy_on_death: false,
        sync: false,
    };

    /// What a listener heard, in order.
    #[derive(Debug, PartialEq)]
    pub(in crate::game) enum Heard {
        /// `player_bed_leave`'s two flags.
        BedLeave(bool, bool),
        /// `player_logged_out`: the online players in their order, whether the player was found
        /// by UUID and still slept, the attachment write's answer and whether the health was set.
        LoggedOut {
            listed: Vec<u128>,
            found: bool,
            asleep: bool,
            attached: Result<(), AttachmentError>,
            healed: bool,
        },
    }

    pub(in crate::game) struct Leave(pub(in crate::game) Arc<Mutex<Vec<Heard>>>);
    #[mod_api::__subscriptions]
    impl Hooks for Leave {
        fn register(&self, registrar: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
            registrar
                .register_attachment_type("leave_test:value", false)
                .unwrap();
        }
        fn player_bed_leave(&self, _: &mut dyn ServerHost, event: &mut PlayerBedLeaveEvent<'_>) {
            self.0
                .lock()
                .unwrap()
                .push(Heard::BedLeave(event.wake_immediately, event.update_level));
        }
        fn player_logged_out(
            &self,
            host: &mut dyn ServerHost,
            event: &mut PlayerLoggedOutEvent<'_>,
        ) {
            let listed = host
                .players()
                .server_players()
                .iter()
                .map(|player| player.uuid())
                .collect();
            let player = host.player(event.player);
            let living = player.and_then(|player| host.living(player.entity()));
            let asleep = living.is_some_and(|living| living.sleeping_pos(host).is_some());
            let attached = VALUE.set(host, Holder::Player(event.player), 77);
            let healed = living.is_some_and(|living| living.set_health(host, 7.0));
            self.0.lock().unwrap().push(Heard::LoggedOut {
                listed,
                found: player.is_some(),
                asleep,
                attached,
                healed,
            });
        }
    }

    #[allow(unused_imports)]
pub(in crate::game) use crate::game::server_side::native_hooks_server::player_leave::player_leave_tests::first_of_two_leaves;

    /// `ServerPlayer.disconnect` wakes a sleeping player (`stopSleepInBed(true, false)`) before
    /// `PlayerList.remove`: the wake is heard first, and the logout finds them awake.
    #[test]
    #[ignore = "daily"]
    fn a_sleeping_player_wakes_before_logging_out() {
        if run_fresh_cases(ASLEEP, FLAG, &["asleep"]) {
            return;
        }
        first_of_two_leaves(
            |game| {
                game.side
                    .lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .update_guest(12, |g| {
                        g.bed = Some([5, 64, 5]);
                        g.sleeping = true;
                        g.sleep_timer = 10;
                    })
                    .unwrap();
            },
            |_, told| {
                assert_eq!(told.len(), 2, "{told:?}");
                assert_eq!(told[0], Heard::BedLeave(true, false));
                assert!(
                    matches!(
                        told[1],
                        Heard::LoggedOut {
                            asleep: false,
                            found: true,
                            ..
                        }
                    ),
                    "{told:?}"
                );
            },
        );
    }
}
use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_server::NativeServerGameExt1 as _;
