use std::sync::Mutex;
pub use tempfile;

/// Bounds concurrent full resource loads and fresh-process children to keep peak memory low.
static HEAVY_GATE: std::sync::LazyLock<(Mutex<(usize, usize, usize)>, std::sync::Condvar)> =
    std::sync::LazyLock::new(|| (Mutex::new((0, 0, 0)), std::sync::Condvar::new()));

pub struct HeavyLoadPermit {
    in_process: bool,
}

impl Drop for HeavyLoadPermit {
    fn drop(&mut self) {
        let (lock, cv) = &*HEAVY_GATE;
        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
        if self.in_process {
            state.0 -= 1;
        }
        state.1 -= 1;
        cv.notify_all();
    }
}

pub fn acquire_in_process_load_permit() -> HeavyLoadPermit {
    const MAX_IN_PROCESS: usize = 1;
    const MAX_TOTAL: usize = 2;
    let (lock, cv) = &*HEAVY_GATE;
    let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
    state.2 += 1;
    while state.0 >= MAX_IN_PROCESS || state.1 >= MAX_TOTAL {
        state = cv.wait(state).unwrap_or_else(|e| e.into_inner());
    }
    state.2 -= 1;
    state.0 += 1;
    state.1 += 1;
    HeavyLoadPermit { in_process: true }
}

pub fn acquire_fresh_child_permit() -> HeavyLoadPermit {
    const MAX_TOTAL: usize = 2;
    let (lock, cv) = &*HEAVY_GATE;
    let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
    while state.1 >= MAX_TOTAL || state.2 > 0 {
        state = cv.wait(state).unwrap_or_else(|e| e.into_inner());
    }
    state.1 += 1;
    HeavyLoadPermit { in_process: false }
}

/// The longest a fresh child may run: a hang in its startup or shutdown fails the parent
/// with the child killed, instead of waiting for the whole test run's own limit.
pub const FRESH_CASE_LIMIT: std::time::Duration =
    std::time::Duration::from_secs(900);


/// Run `test` on a game-sized stack, preserving libtest's thread name and panic payload.
pub fn run_with_game_stack(test: impl FnOnce() + Send + 'static) {
    let mut worker =
        std::thread::Builder::new().stack_size(sunrise_threads::GAME_THREAD_STACK_BYTES);
    if let Some(name) = std::thread::current().name() {
        worker = worker.name(name.to_owned());
    }
    if let Err(payload) = worker.spawn(test).unwrap().join() {
        std::panic::resume_unwind(payload);
    }
}

pub fn fixture_root() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap())
}

/// Keep a key that a test mod's `register` declared. The game fixture registers the
/// installed mods again (`register_game_keys`), and a key's slot is stable for the process,
/// so the repeat must declare the same key.
pub fn record_key<K: Copy + PartialEq + std::fmt::Debug>(
    slot: &std::sync::OnceLock<K>,
    key: K,
) {
    assert_eq!(
        *slot.get_or_init(|| key),
        key,
        "a repeat registration declared another key"
    );
}

/// Isolate declarations and frozen registries without changing another test's resources.
pub fn run_fresh_cases(name: &str, flag: &str, cases: &[&str]) -> bool {
    run_fresh_cases_at_once(name, flag, cases, 1)
}

/// [`run_fresh_cases`] with up to `at_once` children running together, for a test that runs
/// alone in its job: each child still has its own process and folders.
pub fn run_fresh_cases_at_once(
    name: &str,
    flag: &str,
    cases: &[&str],
    at_once: usize,
) -> bool {
    if std::env::var_os(flag).is_some() {
        return false;
    }
    let resources = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").expect(
        "SUNRISE_TEST_RESOURCES must name the prepared resources (.local/sunrise-client/<version>)",
    ))
    .canonicalize()
    .expect("SUNRISE_TEST_RESOURCES names a folder that does not exist");
    let scratch_owner = tempfile::tempdir().unwrap();
    let target = scratch_owner.path();
    let run = |case: &str| {
        let _permit = acquire_fresh_child_permit();
        let scratch = target.join(format!("fresh-case-{case}"));
        std::fs::create_dir(&scratch).unwrap();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                name,
                "--nocapture",
                "--test-threads=4",
            ])
            .current_dir(&scratch)
            .env(flag, case)
            .env("SUNRISE_TEST_RESOURCES", &resources)
            .env("SUNRISE_NO_AUDIO", "1")
            .env("SUNRISE_LAUNCHER_NO_SELF_UPDATE", "1");
        for key in [
            "SUNRISE_HOME",
            "SUNRISE_LAUNCHER_HOME",
            "XDG_DATA_HOME",
            "XDG_CONFIG_HOME",
            "XDG_RUNTIME_DIR",
            "SUNRISE_LAUNCHER_TRASH",
            "SUNRISE_LAUNCHER_CLIPBOARD",
            "SUNRISE_LAUNCHER_DESKTOP",
        ] {
            let home = scratch.join(key);
            std::fs::create_dir(&home).unwrap();
            command.env(key, home);
        }
        let output = output_within(&mut command, name, case);
        std::fs::remove_dir_all(scratch).unwrap();
        output
    };
    for group in cases.chunks(at_once.max(1)) {
        let run = &run;
        let outputs: Vec<_> = std::thread::scope(|scope| {
            let running: Vec<_> = group
                .iter()
                .map(|&case| scope.spawn(move || run(case)))
                .collect();
            running
                .into_iter()
                .map(|child| child.join().unwrap())
                .collect()
        });
        for (case, output) in group.iter().zip(outputs) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // The child's log, captured with this test's own output.
            print!("{stdout}");
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
            assert!(output.status.success(), "{name}: {case}");
            assert!(
                ran_one_test(&stdout),
                "{name}: {case}: the child matched no test of that name"
            );
        }
    }
    true
}

/// `command`'s output, killing the child if it outlives [`FRESH_CASE_LIMIT`].
pub fn output_within(
    command: &mut std::process::Command,
    name: &str,
    case: &str,
) -> std::process::Output {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let deadline = std::time::Instant::now() + FRESH_CASE_LIMIT;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{name}: {case}: the child did not finish within {FRESH_CASE_LIMIT:?}");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    std::process::Output {
        status,
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    }
}

/// Whether libtest's summary says exactly one test passed: `--exact` with a name no test
/// has runs nothing and still exits 0.
pub fn ran_one_test(stdout: &str) -> bool {
    stdout
        .lines()
        .any(|line| line.starts_with("test result: ok. 1 passed;"))
}

/// Cached unique block names of `root`'s `facts.json` so `with_game` skips re-parsing 9.7 MB.
pub fn fixture_block_names(root: &std::path::Path) -> std::sync::Arc<Vec<String>> {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::SystemTime;
    type Stamp = Option<(u64, Option<SystemTime>)>;
    type Slot = Arc<OnceLock<Arc<Vec<String>>>>;
    static NAMES: Mutex<Vec<((PathBuf, Stamp), Slot)>> = Mutex::new(Vec::new());

    let canon = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let stamp = std::fs::metadata(canon.join("facts.json"))
        .ok()
        .map(|m| (m.len(), m.modified().ok()));
    let key = (canon, stamp);
    let slot = {
        let mut entries = NAMES.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pos) = entries.iter().position(|(k, _)| *k == key) {
            let entry = entries.remove(pos);
            let slot = entry.1.clone();
            entries.push(entry);
            slot
        } else {
            if entries.len() >= 4 {
                entries.remove(0);
            }
            let slot = Arc::new(OnceLock::new());
            entries.push((key, slot.clone()));
            slot
        }
    };
    slot.get_or_init(|| {
        let (registry, _) = sunrise_world::level::data::load_registry_with_flags(root).unwrap();
        let ids: std::collections::HashSet<_> = (0..registry.len())
            .map(|i| {
                registry
                    .state(sunrise_world::registry::BlockStateId(i as u32))
                    .unwrap()
                    .name
                    .clone()
            })
            .collect();
        Arc::new(ids.into_iter().collect())
    })
    .clone()
}

struct OfflineProfiles;

impl sunrise_level::profile_lookup::ProfileResolver for OfflineProfiles {
    fn lookup(
        &self,
        _: &sunrise_level::profile_lookup::NameOrId,
    ) -> sunrise_level::profile_lookup::Resolve {
        sunrise_level::profile_lookup::Resolve::Done(None)
    }
}

pub fn offline_profiles() -> Box<dyn sunrise_level::profile_lookup::ProfileResolver> {
    Box::new(OfflineProfiles)
}

pub fn resolve_parts<'g>(game: &impl super::SideGame<'g>) -> Result<(), String> {
    use super::tables::{GameItems as _, WorldTablesExt as _};
    super::api_values::resolve_block_definitions(game.pack.assets.registry)?;
    mod_api::__private::resolve_items(game.items().registry_order.iter().map(|id| &**id))
        .map_err(|errors| format!("fixture item keys refused: {errors:?}"))?;
    game.resolve_parts().map_err(|error| error.to_string())
}

pub fn fixture_facts_mut(
    resources: &mut std::sync::Arc<super::Resources>,
) -> &mut sunrise_items::blocks::BlockFacts {
    &mut std::sync::Arc::get_mut(resources).unwrap().core.facts
}
