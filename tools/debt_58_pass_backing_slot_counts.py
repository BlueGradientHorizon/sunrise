#!/usr/bin/env python3
"""Pass the physical backing-slot count at each native screen open call site."""

from pathlib import Path

GAME = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src/game"


def rewrite(path, *edits):
    path = GAME / path
    text = path.read_text()
    for old, new, count in edits:
        if text.count(old) != count:
            raise ValueError(f"{path}: expected {count} occurrences of {old!r}")
        text = text.replace(old, new)
    path.write_text(text)


rewrite(
    "native_hooks/shared/book_screen/open.rs",
    ("        target: Option<Pos>,\n        extra:", "        target: Option<Pos>,\n        backing_slots: usize,\n        extra:", 1),
    ("        if let Some(kind) = mod_type {\n            screen.slots.truncate(kind.slots);\n        }", "        if mod_type.is_some() {\n            screen.slots.truncate(backing_slots);\n        }", 1),
)
rewrite(
    "network/remote/menu_interactions.rs",
    ("            self.world.block_entities.insert(key, entity);", "            let backing_slots = entity.slots().len();\n            self.world.block_entities.insert(key, entity);", 1),
    ("self.open_native_screen(key, target, extra)", "self.open_native_screen(key, target, backing_slots, extra)", 1),
)
rewrite(
    "native_hooks/shared/book_screen/lifecycle_tests.rs",
    ("                    None,\n                    Some(sunrise_protocol::menu_extra::Message", "                    None,\n                    3,\n                    Some(sunrise_protocol::menu_extra::Message", 1),
)
rewrite(
    "native_hooks/shared/screen_position_tests.rs",
    ("                game.open_native_screen(pos, Some(pos), None);\n                let open", "                game.open_native_screen(pos, Some(pos), 27, None);\n                let open", 1),
    ("                game.open_native_screen(key, None, None);\n                let open", "                game.open_native_screen(key, None, 9, None);\n                let open", 1),
    ("game.open_native_screen(key, None, None);", "game.open_native_screen(key, None, 27, None);", 1),
    ("game.open_native_screen(pos, Some(pos), None);", "game.open_native_screen(pos, Some(pos), 5, None);", 2),
)
rewrite(
    "native_hooks/shared/slot_icon_tests.rs",
    ("game.open_native_screen(pos, Some(pos), None);", "game.open_native_screen(pos, Some(pos), 9, None);", 2),
)
