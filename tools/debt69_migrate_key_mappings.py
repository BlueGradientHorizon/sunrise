#!/usr/bin/env python3
"""Move the repetitive key-mapping fixture calls for debt item 69.

Use --dry-run to see the patch without modifying files. The four modes are
independent previews, but vanilla-writes is applied after vanilla-reads in the
migration. Shared handle declarations are reused. Semantic and fixture edits
listed by each mode belong in the subsequent hand-fix commit.
"""

import argparse
import re
import sys
from dataclasses import dataclass
from difflib import unified_diff
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
GAME = Path("crates/sunrise-client/src/game")
PAD = GAME / "interface/pad/parity_tests.rs"
PHYSICAL = GAME / "native_hooks/client_input_reads_tests.rs"
ENVIRONMENT = GAME / "native_hooks/shared/runtime_environment_tests.rs"
PRESSES = GAME / "network/remote/mapping_press_tests.rs"
BURSTS = GAME / "network/remote/input_burst_tests.rs"
STORAGE = Path("crates/sunrise-client-input/src/keymapping.rs")
MOD_TESTS = GAME / "modding/native_key_mappings_tests.rs"
MOD_NESTED = GAME / "modding/native_key_mappings.rs"


@dataclass(frozen=True)
class Edit:
    old: str
    new: str
    count: int = 1


@dataclass(frozen=True)
class Retain:
    anchor: str  # Insert before this unchanged line.
    declarations: tuple[str, ...]


@dataclass(frozen=True)
class Test:
    name: str
    retain: Retain
    edits: tuple[Edit, ...]


@dataclass(frozen=True)
class Recipe:
    tests: tuple[Test, ...] = ()
    edits: tuple[Edit, ...] = ()
    hand_fixes: tuple[str, ...] = ()


def vanilla(anchor, *keys):
    indent = anchor[: len(anchor) - len(anchor.lstrip())]
    return Retain(
        anchor,
        tuple(
            f'{indent}let {key}_mapping = game.vanilla_key_mapping("key.{key}").unwrap();\n'
            for key in keys
        ),
    )


def vanilla_down(key, count=1, *, snapshot=False):
    old = (
        f'game.mapping("key.{key}").unwrap().unwrap().down'
        if snapshot
        else f'game.mapping_state("key.{key}").unwrap().0'
    )
    return Edit(old, f"{key}_mapping.is_down(game).unwrap()", count)


def physical_down(key, expected, count):
    return Edit(
        f'assert_eq!(game.mapping_physical_down("key.{key}"), Ok(Some({expected})));',
        f"assert_eq!({key}_mapping.physical_down(game), Ok({expected}));",
        count,
    )


def vanilla_set(key, down, count=1):
    return Edit(
        f'game.set_mapping_down("key.{key}", {down})',
        f"{key}_mapping.set_down(game, {down})",
        count,
    )


def vanilla_click(key, count=1):
    return Edit(f'game.click_mapping("key.{key}")', f"{key}_mapping.click(game)", count)


def engine_keys(anchor, *ids):
    return Retain(
        anchor,
        tuple(f'        let {id_} = keys.resolve("m", "{id_}").unwrap();\n' for id_ in ids),
    )


def engine_down(id_, held, screen, count=1):
    return Edit(
        f'keys.is_down("m", "{id_}", {held}, {screen})',
        f"keys.is_down({id_}, {held}, {screen}).unwrap()",
        count,
    )


def engine_click(id_, count=1):
    # Inspect the actual pending count after release/open-screen input, so the
    # consume operation's screen-open guard cannot mask queued clicks.
    return Edit(f'keys.consume_click("m", "{id_}")', f"keys.consume_click({id_}, false).unwrap()", count)


def mod_keys(anchor, *ids):
    indent = anchor[: len(anchor) - len(anchor.lstrip())]
    return Retain(
        anchor,
        tuple(
            f"{indent}let {id_} =\n"
            f'{indent}    ClientInputClientHost::mod_key_mapping(game, "keys", "{id_}").unwrap();\n'
            for id_ in ids
        ),
    )


def mod_click(id_, count):
    return Edit(f'click(game, "{id_}")', f"{id_}.consume_click(game).unwrap()", count)


def mod_set(id_, key, bits):
    return Edit(
        f'set(game, "{id_}", "{key}", {bits})',
        f'{id_}.set_binding(game, "{key}", ClientModifiers::new({bits}))',
    )


PAD_FOCUS = "        game.focused = true;\n"
PLAYER = "            game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());\n"
FORWARD_EVENT = "        game.mapped_key_event(KeyCode::KeyW, true, false).unwrap();\n"
STONE = "        stone_under_the_crosshair(game);\n"
EMPTY_HIT = "        game.forced_hit = None;\n"
Z_PRESS = '        keys.input("key.keyboard.z", KeyAction::Press, false, false, 0);\n'

# These paths are the full write allowlist. Function-scoped replacements also
# check their exact occurrence counts, so similar calls elsewhere are not moved.
RECIPES = {
    "vanilla-reads": {
        PAD: Recipe(
            tests=(
                Test(
                    "readers_of_the_movement_keys_see_the_stick",
                    vanilla(PAD_FOCUS, "forward", "left", "back", "right", "jump"),
                    (
                        vanilla_down("forward", 3, snapshot=True),
                        *(vanilla_down(key, snapshot=True) for key in ("left", "back", "right", "jump")),
                        vanilla_down("forward", 3),
                        vanilla_down("left", 2),
                    ),
                ),
                Test(
                    "the_pads_attack_and_use_reach_what_the_mouse_tells_mods",
                    vanilla("        assert_eq!(game.native_clicks(), (0, 0));\n", "attack", "use"),
                    (vanilla_down("attack", 2, snapshot=True), vanilla_down("use", snapshot=True)),
                ),
            ),
            hand_fixes=(
                "Set the migrated pad fixtures' execution role to Client before resolving.",
                "mapping_state_reads_what_mapping_reads_and_follows_the_binding: use one ref, "
                "assert the exact live binding with binding(game), and move the unknown-name check to resolution.",
            ),
        ),
        PHYSICAL: Recipe(
            tests=(
                Test(
                    "mapping_physical_down_follows_the_binding_and_ignores_the_logical_state",
                    vanilla(PLAYER, "forward", "attack"),
                    (
                        physical_down("forward", "false", 2),
                        physical_down("forward", "true", 3),
                        physical_down("attack", "false", 2),
                        physical_down("attack", "true", 1),
                        vanilla_down("forward"),
                    ),
                ),
            ),
            hand_fixes=(
                "Use with_client_game; replace both unknown-mapping assertions with one refusal on resolution.",
            ),
        ),
        ENVIRONMENT: Recipe(
            tests=(
                Test(
                    "client_host_reads_playing_and_key_mappings",
                    Retain(
                        "        game.keys.insert(KeyCode::KeyW);\n",
                        (
                            "        let forward_mapping =\n"
                            "            mod_api::prelude::ClientInputClientHost::vanilla_key_mapping"
                            '(game, "key.forward")\n'
                            "                .unwrap();\n",
                        ),
                    ),
                    (
                        Edit(
                            """        assert!(
            mod_api::prelude::ClientInputClientHost::mapping(game, "key.forward")
                .unwrap()
                .unwrap()
                .down
        );""",
                            "        assert!(forward_mapping.is_down(game).unwrap());",
                        ),
                    ),
                ),
            ),
            hand_fixes=(
                "Use the retained forward ref for the initial down assertion too; replace the forward/attack "
                "label checks with translated_key_message(game) and move the unknown-name check to resolution.",
            ),
        ),
        PRESSES: Recipe(
            tests=(
                Test(
                    "opening_a_screen_releases_mod_holds",
                    vanilla(FORWARD_EVENT, "forward", "sneak"),
                    (vanilla_down("forward", 4), vanilla_down("sneak", 2)),
                ),
            ),
        ),
    },
    "vanilla-writes": {
        PAD: Recipe(
            tests=(
                Test(
                    "readers_of_the_movement_keys_see_the_stick",
                    vanilla(PAD_FOCUS, "forward"),
                    (vanilla_set("forward", "false", 2), vanilla_set("forward", "true")),
                ),
            ),
            hand_fixes=(
                "the_mouse_letting_go_leaves_what_the_pad_holds_and_back: hoist the forward ref before the loop.",
            ),
        ),
        PHYSICAL: Recipe(
            tests=(
                Test(
                    "mapping_physical_down_follows_the_binding_and_ignores_the_logical_state",
                    vanilla(PLAYER, "forward"),
                    (vanilla_set("forward", "false"),),
                ),
            ),
        ),
        PRESSES: Recipe(
            edits=(
                Edit(
                    "//! A mod presses the vanilla gameplay mappings (`set_mapping_down`, `click_mapping`) on a server's\n"
                    "//! client: the client tick's key step acts on them with its checks and sends the packets the\n"
                    "//! player's own keys send.",
                    "//! A mod presses vanilla gameplay mappings with `VanillaKeyMappingRef::set_down` and\n"
                    "//! `VanillaKeyMappingRef::click`. The client tick's key step acts on them with its checks\n"
                    "//! and sends the packets the player's own keys send.",
                ),
            ),
            tests=(
                Test(
                    "holding_use_draws_a_bow_once_and_letting_go_releases_it",
                    vanilla('        game.inventory[9] = Some(ItemStack::new("minecraft:arrow", 64));\n', "use"),
                    (vanilla_set("use", "true"), vanilla_set("use", "false")),
                ),
                Test(
                    "clicking_attack_on_a_block_starts_the_break_and_punches",
                    vanilla(STONE, "attack"),
                    (vanilla_click("attack", 2), vanilla_set("attack", "true"), vanilla_set("attack", "false")),
                ),
                Test(
                    "clicking_attack_on_nothing_punches_and_waits_out_the_miss_cooldown",
                    vanilla(EMPTY_HIT, "attack"),
                    (vanilla_click("attack", 3), vanilla_set("attack", "true")),
                ),
                Test(
                    "a_screen_drops_the_clicks_and_holds_nothing",
                    vanilla(STONE, "attack"),
                    (vanilla_click("attack", 2), vanilla_set("attack", "true")),
                ),
                Test(
                    "the_host_presses_by_name_and_refuses_the_others",
                    Retain(
                        "        assert_eq!(\n"
                        '            ClientInputClientHost::click_mapping(game, "key.hotbar.3"),\n',
                        (
                            "        let hotbar =\n"
                            "            ClientInputClientHost::vanilla_key_mapping"
                            '(game, "key.hotbar.3").unwrap();\n',
                        ),
                    ),
                    (
                        Edit(
                            """        assert_eq!(
            ClientInputClientHost::click_mapping(game, "key.hotbar.3"),
            Ok(())
        );""",
                            "        assert_eq!(hotbar.click(game), Ok(()));",
                        ),
                    ),
                ),
                Test(
                    "a_callback_click_survives_until_the_next_tick",
                    vanilla(STONE, "attack"),
                    (vanilla_click("attack"),),
                ),
                Test(
                    "opening_a_screen_releases_mod_holds",
                    vanilla(FORWARD_EVENT, "forward", "attack", "use", "sneak"),
                    (
                        vanilla_set("forward", "false"),
                        vanilla_set("forward", "true", 2),
                        *(vanilla_set(key, "true") for key in ("attack", "use", "sneak")),
                    ),
                ),
            ),
            hand_fixes=(
                "the_host_presses_by_name_and_refuses_the_others: separate unknown resolution from write "
                "refusals; reuse one pickItem ref for click and set_down.",
                "Heard::interaction_key_mapping_triggered: supply a retained attack ref in with_callback_game "
                "before input handling; do not resolve on each callback.",
            ),
        ),
        BURSTS: Recipe(
            edits=(
                Edit(
                    "//! order they came, and a mod's loop of `click_mapping` acts 64 entries of each queue a tick, oldest\n"
                    "//! first, and one click of each number key, with nothing dropped below the queue's maximum.",
                    "//! order they came. A mod's loop of `VanillaKeyMappingRef::click` acts on 64 entries of each\n"
                    "//! queue a tick, oldest first, and one click of each number key, with nothing dropped below\n"
                    "//! the queue's maximum.",
                ),
            ),
            hand_fixes=(
                "a_mods_burst_acts_64_entries_of_each_queue_a_tick and "
                "a_full_queue_refuses_the_entry_and_the_call_returns: hoist attack/drop/hotbar refs before the "
                "loops; use the retained hotbar refs instead of formatting names in the loop.",
            ),
        ),
    },
    "mod-storage": {
        STORAGE: Recipe(
            tests=(
                Test(
                    "a_mapping_on_the_sixth_mouse_button_hears_it",
                    engine_keys("        let sixth = winit::event::MouseButton::Other(5);\n", "dash"),
                    (engine_down("dash", 0, "false", 2), engine_click("dash")),
                ),
                Test(
                    "a_tap_adds_one_click",
                    engine_keys(Z_PRESS, "zoom", "boost", "grab"),
                    (
                        engine_down("zoom", 0, "false", 2), engine_click("zoom", 2),
                        engine_click("boost"), engine_click("grab"),
                    ),
                ),
                Test(
                    "a_held_key_adds_one_click_per_repeat",
                    engine_keys(
                        '        keys.input("key.keyboard.z", KeyAction::Press, false, false, 1);\n',
                        "boost",
                    ),
                    (engine_down("boost", 1, "false"), engine_down("boost", 0, "false"), engine_click("boost")),
                ),
                Test(
                    "a_screen_opening_zeroes_the_clicks",
                    engine_keys(Z_PRESS, "zoom", "grab"),
                    (
                        engine_down("zoom", 0, "true"), engine_down("grab", 0, "true"),
                        engine_click("zoom", 2), engine_click("grab", 2),
                    ),
                ),
            ),
            hand_fixes=(
                "a_tap_adds_one_click: assert the unknown owner on resolve.",
                "a_rebinding_moves_the_mapping: handle resolution around the roster change separately; "
                "the existing test removes other rows at rebind, which can stale a ref.",
            ),
        ),
    },
    "mod-api": {
        MOD_TESTS: Recipe(
            edits=(
                Edit(
                    """fn down(game: &Game<'_>, id: &str) -> bool {
    ClientInputClientHost::is_down(game, "keys", id)
}
fn click(game: &mut Game<'_>, id: &str) -> bool {
    ClientInputClientHost::consume_click(game, "keys", id)
}
""",
                    "",
                ),
            ),
            tests=(
                Test(
                    "a_control_socket_key_counts_as_a_window_key",
                    mod_keys("        heard();\n", "zoom", "boost", "grab"),
                    (
                        Edit('down(game, "zoom")', "zoom.is_down(game).unwrap()", 6),
                        mod_click("zoom", 11),
                        mod_click("boost", 3),
                        mod_click("grab", 2),
                        Edit(
                            'click(game, "missing")',
                            'ClientInputClientHost::consume_click(game, "keys", "missing")',
                        ),
                    ),
                ),
                Test(
                    "a_mod_rebinds_and_resets_its_own_mapping",
                    mod_keys("            // Rebound to X with control:", "zoom", "boost", "grab"),
                    (
                        mod_click("zoom", 3),
                        mod_set("zoom", "key.keyboard.x", 2),
                        mod_set("boost", "key.keyboard.left.shift", 1),
                        mod_set("zoom", "key.keyboard.y", 0),
                        mod_set("grab", "key.mouse.right", 0),
                        mod_set("grab", "key.keyboard.unknown", 0),
                        Edit(
                            """            assert_eq!(
                ClientInputClientHost::reset_mod_key(game, "keys", "zoom"),
                Ok(())
            );""",
                            "            assert_eq!(zoom.reset_binding(game), Ok(()));",
                        ),
                    ),
                ),
            ),
            hand_fixes=(
                "a_control_socket_key_counts_as_a_window_key: move missing owner/id assertions to "
                "mod_key_mapping.",
                "a_mod_rebinds_and_resets_its_own_mapping: switch the remaining snapshots to binding(game) "
                "without holding its borrow across mutations; migrate the refusal assertions and remove the "
                "key/set closures. Check translated_key_message(game) separately.",
            ),
        ),
        MOD_NESTED: Recipe(
            tests=(
                Test(
                    "opening_and_closing_a_direct_screen_discards_pending_clicks",
                    Retain(
                        "            game.native_input.mappings.input(\n",
                        (
                            "            let pending =\n"
                            "                mod_api::prelude::ClientInputClientHost::mod_key_mapping"
                            '(game, "test", "pending")\n'
                            "                    .unwrap();\n",
                        ),
                    ),
                    (Edit('game.consume_click("test", "pending")', "pending.consume_click(game).unwrap()"),),
                ),
            ),
        ),
    },
}


def checked_count(source, old, expected, location):
    actual = source.count(old)
    if actual != expected:
        raise ValueError(f"{location}: expected {expected} match(es), found {actual}: {old.strip()!r}")


def replace(source, edits, location):
    for edit in edits:
        checked_count(source, edit.old, edit.count, location)
        source = source.replace(edit.old, edit.new, edit.count)
    return source


def migrate(source, recipe, path):
    source = replace(source, recipe.edits, path)
    for test in recipe.tests:
        location = f"{path}::{test.name}"
        # Scope the exact replacements to this Rust fixture's signature and its
        # closing brace at signature indentation, including nested test modules.
        pattern = rf"(?ms)^( *)fn {re.escape(test.name)}\([^\n]*\) \{{\n.*?^\1\}}(?=\n|\Z)"
        matches = list(re.finditer(pattern, source))
        if len(matches) != 1:
            raise ValueError(f"{location}: expected one test function, found {len(matches)}")
        match = matches[0]
        body = match[0]
        anchor = "\n" + test.retain.anchor
        checked_count(body, anchor, 1, location)
        missing = []
        for declaration in test.retain.declarations:
            # Reuse a fixture-local declaration from vanilla reads. Require
            # the same indentation so a declaration inside a loop cannot match.
            count = body.count("\n" + declaration.rstrip("\n"))
            if count > 1:
                raise ValueError(f"{location}: duplicate declaration: {declaration.strip()}")
            if not count:
                missing.append(declaration)
        body = body.replace(anchor, "\n" + "".join(missing) + test.retain.anchor, 1)
        body = replace(body, test.edits, location)
        source = source[:match.start()] + body + source[match.end():]
    return source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=RECIPES)
    parser.add_argument("--dry-run", action="store_true", help="validate all matches and print a unified diff")
    args = parser.parse_args()
    recipes = RECIPES[args.mode]
    updated = {}
    # Validate the entire mode before writing any file.
    try:
        for path, recipe in recipes.items():
            target = ROOT / path
            if target.resolve() != target:
                raise ValueError(f"{path}: refusing to follow a symlink outside the allowlisted path")
            before = target.read_bytes().decode("utf-8")
            updated[path] = before, migrate(before, recipe, path)
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")

    for path, (before, after) in updated.items():
        if args.dry_run:
            sys.stdout.writelines(
                unified_diff(
                    before.splitlines(keepends=True),
                    after.splitlines(keepends=True),
                    fromfile=f"a/{path}",
                    tofile=f"b/{path}",
                )
            )
        else:
            (ROOT / path).write_bytes(after.encode("utf-8"))
        for note in recipes[path].hand_fixes:
            print(f"Hand fix {path}: {note}", file=sys.stderr)


if __name__ == "__main__":
    main()
