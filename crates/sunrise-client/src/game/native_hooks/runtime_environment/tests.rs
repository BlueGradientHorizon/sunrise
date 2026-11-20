//! Bootstrap capture -> real registration -> real Game hosts, with the compiled wasm guest.
use crate::game::{Game, Options, api_values, execution_role::ExecutionRole, mods};
use mod_api::{Dist, Hooks, Host, ModEntry, PackageInfo, RegistrationContext, RuntimeEnvironment};
use std::sync::{Arc, Mutex};
use sunrise_mod_package::selection::LaunchArguments;

#[derive(Default)]
struct Observed {
    registration: Vec<RuntimeEnvironment>,
    later: Vec<RuntimeEnvironment>,
    messages: Vec<mod_api::Text>,
    tests: usize,
}
struct Probe(Arc<Mutex<Observed>>);
fn from_host(host: &dyn Host, tests: bool) -> RuntimeEnvironment {
    RuntimeEnvironment::new(
        host.dist(),
        host.game_version().to_owned(),
        host.sunrise_version().to_owned(),
        tests,
    )
}
#[mod_api::__subscriptions]
impl Hooks for Probe {
    fn register(&self, _: &mut dyn mod_api::Registrar, cx: &RegistrationContext) {
        self.0
            .lock()
            .unwrap()
            .registration
            .push(RuntimeEnvironment::new(
                cx.dist(),
                cx.game_version().to_owned(),
                cx.sunrise_version().to_owned(),
                cx.game_tests_enabled(),
            ));
    }
    fn client_tick_pre(
        &self,
        host: &mut dyn mod_api::ClientHost,
        _: &mut mod_api::ClientTickPreEvent,
    ) {
        // The flag has only a server Host method; registration already pinned it.
        let mut observed = self.0.lock().unwrap();
        let tests = observed.registration[0].game_tests_enabled();
        observed.later.push(from_host(host, tests));
    }
    fn server_started(
        &self,
        host: &mut dyn mod_api::ServerHost,
        _: &mut mod_api::ServerStartedEvent,
    ) {
        self.0
            .lock()
            .unwrap()
            .later
            .push(from_host(host, host.game_tests_enabled()));
    }
    fn register_game_tests(&self, _: &dyn Host, _: &mut mod_api::RegisterGameTestsEvent<'_>) {
        self.0.lock().unwrap().tests += 1;
    }
    fn broadcast_message(
        &self,
        _: &dyn Host,
        event: &mut mod_api::BroadcastMessageEvent<'_>,
    ) -> mod_api::Flow {
        self.0.lock().unwrap().messages.push(event.line.clone());
        mod_api::Flow::Continue
    }
}

#[test]
#[ignore = "daily"]
fn native_process_environment_in_registration_and_game_hosts() {
    run(false);
}

#[test]
#[ignore = "requires prepared SUNRISE_TEST_RESOURCES and rebuilt SUNRISE_WASM_FIXTURE/SUNRISE_WASM_STD"]
fn native_and_wasm_process_environment_in_registration_and_game_hosts() {
    sunrise_guest_stack::run_on_guest_stack(|| run(true));
}

#[test]
fn normal_options_preserve_dedicated_flag_valued_operands() {
    let cwd = std::env::current_dir().unwrap();
    for option in ["world", "universe", "port", "serverId", "pidFile"] {
        let tail = vec![format!("--{option}"), "--game-tests".to_owned()];
        let launch = LaunchArguments::parse(
            std::iter::once("--dedicated".to_owned()).chain(tail.clone()),
            &cwd,
            &cwd,
            None,
        )
        .unwrap();
        let options = Options::from_launch(launch).unwrap().unwrap();
        assert_eq!(options.dedicated.as_ref(), Some(&tail));
        assert_eq!(
            crate::entry::dedicated_arguments::parse(options.dedicated.as_ref().unwrap())
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            [(option, Some("--game-tests"))],
        );
    }
    let options = Options::parse(["--world", "--game-tests"].map(str::to_owned).into_iter())
        .unwrap()
        .unwrap();
    assert_eq!(options.world.as_deref(), Some("--game-tests"));
}
