#!/usr/bin/env python3
"""Route the existing entity collection fixtures through test-only collectors."""

import re
import subprocess
from collections import Counter
from pathlib import Path

GAME = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src/game"
HANDLE_CALL = re.compile(
    r"\b(?P<level>level|viewed)(?:\n[ \t]+)?\."
    r"(?P<method>all_entities|entities_in)\((?P<host>host|&?game)(?=[,)])"
)
GAME_CALL = re.compile(
    r"\bgame(?:\n[ \t]+)?\.level_(?P<method>all_entities|entities_in)"
    r"\((?P<indent>\n[ \t]+)?"
)

COLLECTORS = """
    // Keep the handle and direct Game routes separate when collecting for assertions.
    pub(in crate::game) fn collect_all_entities(
        host: &dyn mod_api::Host,
        level: LevelRef,
    ) -> Option<Vec<EntityRef>> {
        level.all_entities(host)
    }

    pub(in crate::game) fn collect_entities_in(
        host: &dyn mod_api::Host,
        level: LevelRef,
        min: [f64; 3],
        max: [f64; 3],
    ) -> Option<Vec<EntityRef>> {
        level.entities_in(host, min, max)
    }

    pub(in crate::game) fn collect_game_all_entities(
        game: &super::Game<'_>,
        level: LevelRef,
    ) -> Option<Vec<EntityRef>> {
        game.level_all_entities(level)
    }

    pub(in crate::game) fn collect_game_entities_in(
        game: &super::Game<'_>,
        level: LevelRef,
        min: [f64; 3],
        max: [f64; 3],
    ) -> Option<Vec<EntityRef>> {
        game.level_entities_in(level, min, max)
    }
"""


def rewrite(relative, expected, anchor, replacement, *, scope=""):
    path = GAME / relative
    source = path.read_text(encoding="utf-8")
    prefix = ""
    if scope:
        if source.count(scope) != 1:
            raise SystemExit(f"{relative}: expected one test scope {scope!r}")
        prefix, marker, tail = source.partition(scope)
        source = marker + tail

    seen = Counter()

    def handle_call(match):
        method, host, level = match.group("method", "host", "level")
        seen[method] += 1
        return f"collect_{method}({host}, {level}"

    def game_call(match):
        method, indent = match.group("method", "indent")
        seen[f"game_{method}"] += 1
        if indent:
            return f"collect_game_{method}({indent}game,{indent}"
        return f"collect_game_{method}(game, "

    source = GAME_CALL.sub(game_call, HANDLE_CALL.sub(handle_call, source))
    if seen != expected:
        raise SystemExit(f"{relative}: expected {expected}, found {dict(seen)}")
    if source.count(anchor) != 1:
        raise SystemExit(f"{relative}: expected one fixture anchor {anchor!r}")
    return path, prefix + source.replace(anchor, replacement)


def main():
    fixtures = [
        rewrite(
            "entities/lightning.rs",
            {"entities_in": 1},
            "    use mod_api::{self, ServerHost};\n",
            "    use crate::game::native_entity_refs::tests::collect_entities_in;\n"
            "    use mod_api::{self, ServerHost};\n",
            scope="#[cfg(test)]\nmod tests {\n",
        ),
        rewrite(
            "entities/passengers.rs",
            {"all_entities": 1, "entities_in": 1},
            "mod nested_vehicle_discard_tests {\n    use super::*;\n",
            "mod nested_vehicle_discard_tests {\n    use super::*;\n"
            "    use crate::game::native_entity_refs::tests::{collect_all_entities, collect_entities_in};\n",
            scope="#[cfg(test)]\nmod nested_vehicle_discard_tests {\n",
        ),
        rewrite(
            "modding/block_entity_tick_items_tests.rs",
            {"all_entities": 1},
            "use super::api_values::TryIntoApi;\n",
            "use super::{api_values::TryIntoApi, native_entity_refs::tests::collect_all_entities};\n",
        ),
        rewrite(
            "modding/native_entities.rs",
            {"entities_in": 4},
            "mod tests {\n    use super::*;\n",
            "mod tests {\n    use super::*;\n"
            "    use crate::game::native_entity_refs::tests::collect_entities_in;\n",
            scope="#[cfg(test)]\nmod tests {\n",
        ),
        rewrite(
            "modding/native_entity_refs.rs",
            {"entities_in": 3, "game_entities_in": 3},
            "    use mod_api::__private::game::*;\n",
            "    use mod_api::__private::game::*;\n"
            "    use mod_api::{EntityRef, LevelRef};\n"
            + COLLECTORS
            + "\n",
            scope="#[cfg(test)]\npub(super) mod tests {\n",
        ),
        rewrite(
            "modding/native_entity_refs/dragon_query.rs",
            {"entities_in": 1, "game_all_entities": 6, "game_entities_in": 2},
            "    use crate::game::{dragon::Dragon, native_entity_refs::tests::with_game};\n",
            "    use crate::game::{\n"
            "        dragon::Dragon,\n"
            "        native_entity_refs::tests::{\n"
            "            collect_entities_in, collect_game_all_entities, collect_game_entities_in, with_game,\n"
            "        },\n"
            "    };\n",
            scope="#[cfg(test)]\nmod tests {\n",
        ),
        rewrite(
            "modding/native_fishing_hook_refs.rs",
            {"game_entities_in": 1},
            "    use super::super::{fishing::Hook, native_entity_refs::tests::with_game};\n",
            "    use super::super::{\n"
            "        fishing::Hook,\n"
            "        native_entity_refs::tests::{collect_game_entities_in, with_game},\n"
            "    };\n",
            scope="#[cfg(test)]\nmod tests {\n",
        ),
        rewrite(
            "modding/native_move_by_tests.rs",
            {"game_entities_in": 1},
            "use super::native_entity_refs::tests::with_game;\n",
            "use super::native_entity_refs::tests::{collect_game_entities_in, with_game};\n",
        ),
        rewrite(
            "modding/native_player_destroyed_block_tests.rs",
            {"all_entities": 1},
            "use super::native_entity_refs::tests::with_game;\n",
            "use super::native_entity_refs::tests::{collect_all_entities, with_game};\n",
        ),
        rewrite(
            "modding/native_viewed_levels.rs",
            {"entities_in": 1},
            "        api_values::TryIntoApi,\n",
            "        api_values::TryIntoApi,\n"
            "        native_entity_refs::tests::collect_entities_in,\n",
            scope="#[cfg(test)]\nmod tests {\n",
        ),
        rewrite(
            "network/lan_actions/tests/pickup_post_tests.rs",
            {"all_entities": 1},
            "use crate::game::Game;\n",
            "use crate::game::{Game, native_entity_refs::tests::collect_all_entities};\n",
        ),
        rewrite(
            "world/persist/lookup_order_tests.rs",
            {"game_all_entities": 1},
            "use crate::game::native_entity_refs::tests::with_game;\n",
            "use crate::game::native_entity_refs::tests::{collect_game_all_entities, with_game};\n",
        ),
    ]
    # Validate every fixture before touching any file.
    for path, source in fixtures:
        path.write_text(source, encoding="utf-8")
    subprocess.run(
        [
            "rustfmt", "--edition", "2024", "--config", "skip_children=true",
            *(path for path, _ in fixtures),
        ],
        check=True,
    )


if __name__ == "__main__":
    main()
