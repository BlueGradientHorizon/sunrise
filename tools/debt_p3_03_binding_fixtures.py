#!/usr/bin/env python3
"""Migrate scoped loot binding fixtures to the generation's normalization input."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = {
    "sunrise-client/src/game/modding/api_values/loot_query_tests.rs": 1,
    "sunrise-client/src/game/modding/api_values/loot_values.rs": 2,
    "sunrise-items/src/loot_query_tests.rs": 1,
    "sunrise-world/src/loot_query_tests.rs": 1,
    "sunrise-world/tests/loot_type_logging.rs": 1,
}
OLD = re.compile(
    r"(\.bind_generation\(\n(?:[^\n]*\n){3}\s*Vec::new\(\),\n[ ]*)"
    r"(?:std::collections::)?HashMap::new\(\),"
)
for name, expected in FIXTURES.items():
    path = ROOT / "crates" / name
    before = path.read_text()
    after, changes = OLD.subn(r"\g<1>Default::default(),", before)
    if changes != expected:
        raise SystemExit(f"{name}: expected {expected} fixture calls, found {changes}")
    path.write_text(after)
