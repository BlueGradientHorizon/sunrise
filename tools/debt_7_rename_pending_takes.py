#!/usr/bin/env python3
"""Rename the engine snapshots without changing the mod API records or hooks."""

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src"
RENAMES = {
    "AnvilCraft": "AnvilCraftSnapshot",
    "GrindstoneTake": "PendingGrindstoneTake",
}
# (old identifier, required prefix in mixed files, expected match count)
FILES = {
    "menu.rs": [("AnvilCraft", "", 1), ("GrindstoneTake", "", 3)],
    "menu/models.rs": [("AnvilCraft", "", 1), ("GrindstoneTake", "", 2)],
    "menu/storage.rs": [("AnvilCraft", "", 1), ("GrindstoneTake", "", 1)],
    "menu/crafting_take.rs": [("AnvilCraft", "", 1), ("GrindstoneTake", "", 1)],
    "game/network/lan_stations.rs": [
        ("AnvilCraft", "menu::{", 1),
        ("AnvilCraft", "craft: Option<&", 1),
        ("GrindstoneTake", "GrindstonePlace, ", 1),
        ("GrindstoneTake", "RefCell::new(None::<", 1),
    ],
    "game/modding/api_values.rs": [
        ("AnvilCraft", "crate::menu::", 5),
        ("GrindstoneTake", "crate::menu::", 4),
    ],
}

# Check every count before writing any file, so a drifted source leaves no partial rename.
updates = {}
for name, replacements in FILES.items():
    path = ROOT / name
    source = path.read_text(encoding="utf-8")
    for old, prefix, expected in replacements:
        context = rf"(?<={re.escape(prefix)})" if prefix else ""
        pattern = rf"{context}\b{old}\b"
        source, count = re.subn(pattern, RENAMES[old], source)
        if count != expected:
            raise SystemExit(f"{name}: expected {expected} matches for {pattern!r}, found {count}")
    updates[path] = source

for path, source in updates.items():
    path.write_text(source, encoding="utf-8")

subprocess.run(
    ["rustfmt", "--edition", "2024", "--config", "skip_children=true", *updates],
    check=True,
)
