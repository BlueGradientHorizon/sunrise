#!/usr/bin/env python3
"""Require a checkpointable RNG only along the spawn placement call paths."""

import re
from pathlib import Path

MOBS = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src/mobs"

for filename, functions in {
    "spawn_rules.rs": [("check_spawn_rules", "R", 1)],
    "spawn_selection.rs": [
        ("spawn_for_chunk", "R", 1),
        ("spawn_category_for_chunk", "R", 1),
        ("spawn_category_for_position", "R", 1),
        ("valid_position_for_type", "R", 1),
        ("spawn_mobs_for_chunk_generation", "Q", 1),
    ],
    "spawner.rs": [("server_tick", "R", 2)],
    "trial_spawner.rs": [
        ("server_tick", "R", 2),
        ("tick_and_get_next", "R", 2),
        ("spawn_mob", "R", 2),
    ],
}.items():
    path = MOBS / filename
    text = path.read_text()
    for name, rng, expected in functions:
        pattern = rf"(fn {name}<[^>]*\b{rng}: RandomSource)(?! \+ Clone)(?=[,>])"
        text, count = re.subn(pattern, r"\1 + Clone", text)
        if count != expected:
            raise SystemExit(f"{path}: expected {expected} signatures of {name}, got {count}")
    path.write_text(text)
