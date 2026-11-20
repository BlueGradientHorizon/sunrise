#!/usr/bin/env python3
"""Give the audited workers and game fixture their existing 136 MiB stacks explicitly.

Run once, then rustfmt the changed Rust files. No source is written unless the
full callsite inventory and every replacement match the pre-migration tree.
"""

import re
import subprocess
from pathlib import Path
from textwrap import indent

ROOT = Path(__file__).resolve().parents[1]
CLIENT = "crates/sunrise-client/src"
MODDING = f"{CLIENT}/game/modding"
CRASH = "crates/sunrise-client-render/src/crash.rs"
INTEGRATED = f"{CLIENT}/game/world/integrated.rs"
GUEST_LIB = "crates/sunrise-guest-stack/src/lib.rs"
STACK_BYTES = "136 * 1024 * 1024"
NEW_BUILDER = "std::thread::Builder::new()"
STACK_SIZE = f".stack_size({STACK_BYTES})"
OLD_BUILDER = "sunrise_guest_stack::builder()"

# Identify each builder call by its next line, including that line's indentation.
BUILDER_SITES = {
    f"{CLIENT}/game/interface/startup_menu.rs": (
        '        .name("resource-loader".into())',  # spawn_loader
    ),
    f"{CLIENT}/game/runtime/resource_load.rs": (
        # named_scope
        "        // Linux keeps 15 bytes of a thread name: a short prefix keeps them apart",
        '            .name("world-rest-loader".into())',  # load_resources_for_mods
        '        .name("Prepared data file".into())',  # write_prepared_files
    ),
    f"{CLIENT}/game/world/packs/mod.rs": (
        '            .name("pack-reload".into())',  # PackReload::spawn_holding
    ),
    f"{CLIENT}/game/world/world_entry.rs": (
        '                .name("world-creation".into())',  # Game::poll_world_creation
    ),
    f"{CLIENT}/game/world/world_loading.rs": (
        '            .name("World initialization".into())',  # WorldLoader::start
        '                .name("World data preload".into())',  # preload_world_data
        '        .name("World data preload".into())',  # preload_launch_world
    ),
    f"{CLIENT}/save/writes.rs": (
        '            .name("sunrise-region-writer".into())',  # SaveDir::start_writer
    ),
    f"{CLIENT}/vanillagen.rs": (
        '            .name("Ring search".into())',  # search_rings_behind
    ),
    f"{CLIENT}/vanillagen/lod.rs": (
        '            .name("Far decoration".into())',  # Builder::spawn
    ),
    "crates/sunrise-threads/src/jobs.rs": (
        '                .name(format!("{name}-{i}"))',  # Pool::with_idle_release
    ),
    "crates/sunrise-threads/src/lib.rs": (
        "                    .spawn_scoped(scope, work)",  # par_chunks_mut_with
    ),
}

# Match the complete first argument; replace only the call prefix, not the closure.
# Indents are for the new builder continuation lines.
SPAWN_SITES = {
    CRASH: ((8, '"Crash report lines".into()'),),  # mod_lines
    f"{CLIENT}/game/network/lan/sync.rs": (
        (8, '"Server data preload".into()'),  # preload_sync_data
    ),
    f"{CLIENT}/lan/io_pool.rs": ((12, 'format!("sunrise-net-{n}")'),),  # start
    f"{CLIENT}/lan/mod.rs": (
        (12, '"sunrise-local-conn".into()'),  # connect_local
        (20, '"sunrise-lan-conn".into()'),  # accept_loop
    ),
    f"{CLIENT}/net.rs": ((12, '"sunrise-net".into()'),),  # start
}

FIXTURE_MODULE = "crate::game::native_entity_refs::tests"
OLD_FIXTURE = "run_on_guest_stack"
NEW_FIXTURE = "run_with_game_stack"
QUALIFIED_FIXTURE = f"{FIXTURE_MODULE}::{OLD_FIXTURE}"
DIRECT_FIXTURE = f"sunrise_guest_stack::{OLD_FIXTURE}"
FIXTURE_USERS = {
    f"{CLIENT}/game/mobs/creatures/death_weapon_review_tests.rs": (QUALIFIED_FIXTURE, 4),
    f"{CLIENT}/game/mobs/creatures/death_weapon_tests.rs": (QUALIFIED_FIXTURE, 4),
    f"{MODDING}/block_entity_part_state_tests.rs": (DIRECT_FIXTURE, 1),
    f"{MODDING}/block_entity_tick_items_tests.rs": (QUALIFIED_FIXTURE, 1),
    f"{MODDING}/block_entity_tick_reply_tests.rs": (DIRECT_FIXTURE, 1),
    f"{MODDING}/block_use_completion_tests.rs": (QUALIFIED_FIXTURE, 1),
    # For bare names the count includes the import.
    f"{MODDING}/native_explosion_destroy_tests.rs": (OLD_FIXTURE, 2),
    f"{MODDING}/native_explosion_resistance_tests.rs": (OLD_FIXTURE, 2),
    f"{MODDING}/native_explosions_tests.rs": (OLD_FIXTURE, 7),
    f"{MODDING}/native_general_renderers_tests.rs": (DIRECT_FIXTURE, 1),
    f"{MODDING}/original_player.rs": (QUALIFIED_FIXTURE, 2),
}

FIXTURE_ARGS = "(test: impl FnOnce() + Send + 'static)"
FIXTURE_BODY = """{
    let mut worker = builder();
    if let Some(name) = std::thread::current().name() {
        worker = worker.name(name.to_owned());
    }
    if let Err(payload) = worker.spawn(test).unwrap().join() {
        std::panic::resume_unwind(payload);
    }
}
"""
OLD_FIXTURE_DEF = (
    "/// Runs `test` on a guest-sized worker named like the calling thread, so libtest reports it under\n"
    "/// its own name, and resumes its panic with the original payload. For tests that enter a guest:\n"
    "/// libtest's own threads are too small.\n"
    f"pub fn {OLD_FIXTURE}{FIXTURE_ARGS} {FIXTURE_BODY}\n"
)
NEW_FIXTURE_DEF = indent(
    "/// Run `test` on a game-sized stack, preserving libtest's thread name and panic payload.\n"
    f"pub(in crate::game) fn {NEW_FIXTURE}{FIXTURE_ARGS} "
    + FIXTURE_BODY.replace("builder()", NEW_BUILDER + STACK_SIZE),
    "    ",
)

OLD_AUDIT_TEST = r"""#[cfg(test)]
#[test]
fn audited_guest_workers_use_the_stack_policy() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for file in [
        "crates/sunrise-threads/src/jobs.rs",
        "crates/sunrise-threads/src/lib.rs",
        "crates/sunrise-client/src/lan/io_pool.rs",
        "crates/sunrise-client/src/vanillagen.rs",
        "crates/sunrise-client/src/vanillagen/lod.rs",
        "crates/sunrise-client/src/game/runtime/resource_load.rs",
        "crates/sunrise-client/src/game/interface/startup_menu.rs",
        "crates/sunrise-client/src/game/world/world_loading.rs",
        "crates/sunrise-client/src/game/world/world_entry.rs",
        "crates/sunrise-client/src/game/world/packs/mod.rs",
    ] {
        let source = std::fs::read_to_string(root.join(file)).unwrap();
        let source = source.split("\n#[cfg(test)]").next().unwrap();
        assert!(
            ![
                "thread::Builder::new()",
                "thread::spawn(",
                "scope.spawn(",
                "s.spawn("
            ]
            .iter()
            .any(|form| source.contains(form)),
            "{file}: route guest workers through sunrise_guest_stack::builder"
        );
    }
    let server =
        std::fs::read_to_string(root.join("crates/sunrise-client/src/game/world/integrated.rs"))
            .unwrap();
    // Production code only: a test may start its own server thread with the same policy.
    let server = server.split("\n#[cfg(test)]").next().unwrap();
    assert_eq!(
        server.matches(".stack_size(server_thread_stack())").count(),
        2
    );
    assert_eq!(server.matches("thread::Builder::new()").count(), 2);
    for form in ["thread::spawn(", "scope.spawn(", "s.spawn("] {
        assert!(
            !server.contains(form),
            "server: route guest workers through the stack policy"
        );
    }
}

"""


def main() -> None:
    edits = {
        CRASH: [
            (
                "under [`contain`], on a guest-sized thread of its own: this runs inside the panic hook, where a\n"
                "/// second panic would end the process before any catch could.",
                "under [`contain`], on a dedicated worker: this runs inside the panic hook, where a second\n"
                "/// panic would end the process before any catch could.",
                1,
            ),
        ],
        INTEGRATED: [
            (
                "/// The server thread holds its `Game` by value and runs every tick on it.\n"
                "/// It reserves what every guest-capable thread does under the engine's stack policy.\n"
                "fn server_thread_stack_floor() -> usize {\n"
                "    sunrise_guest_stack::reservation()\n"
                "}",
                "/// The minimum stack for the integrated server, which holds its `Game` by value\n"
                "/// and runs every tick on it.\n"
                "fn server_thread_stack_floor() -> usize {\n"
                f"    {STACK_BYTES}\n"
                "}",
                1,
            ),
            (
                "    /// The server thread keeps guest headroom, and a larger `RUST_MIN_STACK` still\n"
                "    /// reaches it (#2430).\n"
                "    #[test]\n"
                "    fn the_server_thread_stack_keeps_guest_headroom_and_larger_overrides() {",
                "    /// The integrated server keeps its stack floor and honors a larger\n"
                "    /// `RUST_MIN_STACK` override (#2430).\n"
                "    #[test]\n"
                "    fn integrated_server_stack_honors_its_floor_and_larger_overrides() {",
                1,
            ),
            (
                '"smaller asks keep guest headroom"',
                '"smaller values keep the integrated server floor"',
                1,
            ),
        ],
        f"{MODDING}/native_entity_refs.rs": [
            (
                "    /// Keep guest setup and calls together, preserving libtest identity and panic payloads.\n"
                f"    pub(in crate::game) use {DIRECT_FIXTURE};\n",
                NEW_FIXTURE_DEF,
                1,
            ),
        ],
        GUEST_LIB: [(OLD_FIXTURE_DEF, "", 1), (OLD_AUDIT_TEST, "", 1)],
    }
    for path, sites in BUILDER_SITES.items():
        for next_line in sites:
            padding = next_line[: len(next_line) - len(next_line.lstrip())]
            old = f"{OLD_BUILDER}\n{next_line}"
            new = f"{NEW_BUILDER}\n{padding}{STACK_SIZE}\n{next_line}"
            edits.setdefault(path, []).append((old, new, 1))
    for path, sites in SPAWN_SITES.items():
        for indent_width, name in sites:
            old = f"sunrise_guest_stack::spawn({name}, move ||"
            new = ("\n" + " " * indent_width).join(
                (NEW_BUILDER, STACK_SIZE, f".name({name})", ".spawn(move ||")
            )
            edits.setdefault(path, []).append((old, new, 1))
    for path, (old, count) in FIXTURE_USERS.items():
        new = NEW_FIXTURE if old == OLD_FIXTURE else f"{FIXTURE_MODULE}::{NEW_FIXTURE}"
        edits.setdefault(path, []).append((old, new, count))

    old_refs = (
        r"sunrise_guest_stack::(builder|spawn|reservation)"
        r"|run_on_guest_stack|audited_guest_workers_use_the_stack_policy"
    )
    found = subprocess.run(
        ["git", "grep", "-l", "-w", "-E", old_refs, "--", "*.rs"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if found.returncode not in (0, 1):
        raise SystemExit(found.stderr)
    paths = set(found.stdout.splitlines())
    missing, extra = sorted(edits.keys() - paths), sorted(paths - edits.keys())
    if missing or extra:
        raise SystemExit(f"unexpected stack callsite inventory: missing={missing}, extra={extra}")

    updates = {}
    for name, replacements in edits.items():
        path = ROOT / name
        source = path.read_bytes().decode("utf-8")
        for old, new, expected in replacements:
            count = source.count(old)
            if count != expected:
                marker = old.splitlines()[0]
                raise SystemExit(f"{name}: expected {expected} matches, found {count}: {marker}")
            source = source.replace(old, new)
        if leftover := re.search(rf"\b(?:{old_refs})\b", source):
            raise SystemExit(f"{name}: unexpected remaining reference: {leftover[0]}")
        updates[path] = source.encode("utf-8")

    for path, source in updates.items():
        path.write_bytes(source)


if __name__ == "__main__":
    main()
