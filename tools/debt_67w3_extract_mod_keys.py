#!/usr/bin/env python3
"""Move mod key mappings out of claimed-item state without changing either route."""

import re
from pathlib import Path

GAME = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src/game"

fields = {
    GAME.with_suffix(".rs"): (
        (
            b"    /// Native client mods' claimed items, unpolled events and key mappings (game/modding/native_claimed_items.rs).\n"
            b"    native_input: native_claimed_items::ModInput,\n",
            b"    /// Native client mods' claimed items and unpolled events (game/modding/native_claimed_items.rs).\n"
            b"    native_input: native_claimed_items::ModInput,\n"
            b"    /// Native client mods' key mappings (game/modding/native_key_mappings.rs).\n"
            b"    mod_keys: sunrise_client::keymapping::ModKeys,\n",
        ),
        (
            b"            native_input: Default::default(),\n",
            b"            native_input: Default::default(),\n"
            b"            mod_keys: Default::default(),\n",
        ),
    ),
    GAME / "modding/native_claimed_items.rs": (
        (
            b"/// The items client mods claim, their events waiting for a poll, the use under way, and the\n"
            b"/// keys they register (`native_key_mappings.rs`).\n",
            b"/// The items client mods claim, their events waiting for a poll, and the use under way.\n",
        ),
        (b"    pub(super) mappings: sunrise_client::keymapping::ModKeys,\n", b""),
    ),
}

access = re.compile(rb"\b(self|game)\s*\.\s*native_input\s*\.\s*mappings\b")
callers = {
    "modding/native_key_mappings.rs": 8,
    "interface/hud_editor.rs": 1,
    "interface/screen_build.rs": 1,
    "runtime/control.rs": 1,
}

# Check all anchors before writing any file, so a shifted base cannot leave a partial edit.
updates = {}
for path, replacements in fields.items():
    source = path.read_bytes()
    for old, new in replacements:
        if source.count(old) != 1:
            raise SystemExit(f"{path}: expected one occurrence of {old!r}")
        source = source.replace(old, new)
    updates[path] = source

for name, expected in callers.items():
    path = GAME / name
    source, count = access.subn(rb"\1.mod_keys", path.read_bytes())
    if count != expected:
        raise SystemExit(f"{path}: expected {expected} mapping accesses, found {count}")
    updates[path] = source

for path, source in updates.items():
    path.write_bytes(source)
