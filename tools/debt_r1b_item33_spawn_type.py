#!/usr/bin/env python3
"""Move existing MobType-backed spawn-placement checks to SpawnEntityType."""
from pathlib import Path
import re

CLIENT = Path(__file__).resolve().parents[1] / "crates/sunrise-client"
# The spawner implementations need a separate, behavioural change. This
# conversion preserves the input of every existing MobType-backed check.
PATHS = (
    "examples/probe/spawn_rules_fixture.rs",
    "src/game/mobs/natural_spawning.rs",
    "src/game/mobs/raids/tests/spawn_placements.rs",
    "src/game/mobs/zombie_reinforcements.rs",
    "src/mobs/spawn_rules.rs",
    "src/mobs/spawn_selection.rs",
    "src/mobs/spawner.rs",
    "src/mobs/trial_spawner.rs",
)
FIRST_ARGUMENT = re.compile(
    r"(?P<call>\bcheck_spawn_rules\(\s*)"
    r"(?P<arg>kind|t|zombie|&guardian|&kind|&trader|"
    r'&mob_kind\("[^"\n]+", Species::\w+\)|'
    r'level\.mob_type\("minecraft:sulfur_cube"\)\.unwrap\(\))(?=,)'
)
for name in PATHS:
    path = CLIENT / name
    source = path.read_text()
    updated = FIRST_ARGUMENT.sub(
        lambda m: m["call"] + (
            f"({m['arg']}).into()" if m["arg"].startswith("&") else f"{m['arg']}.into()"
        ),
        source,
    )
    if updated != source:
        path.write_text(updated)
