#!/usr/bin/env python3
"""Move the reload anchor tests from first/last aliases to the real managers."""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "crates/sunrise-client/src/game/modding/model_baking_completed_tests.rs": (
        "VanillaClientListeners", "model_manager",
    ),
    "crates/sunrise-client/src/game/world/server_reload/server_reload_tests.rs": (
        "VanillaServerListeners", "functions",
    ),
    "crates/sunrise-mod-vocab/src/client/client_reload_tests.rs": (
        "VanillaClientListeners", "model_manager",
    ),
    "crates/sunrise-mod-vocab/src/world/server_reload_tests.rs": (
        "VanillaServerListeners", "functions",
    ),
}

for path, (side, manager) in TARGETS.items():
    fixture = ROOT / path
    before = fixture.read_text()
    after = re.sub(rf"^ *assert_eq!\({side}::(?:first|last)\(\), .*\);\n", "", before, flags=re.M)
    after = re.sub(rf"{side}::(?:first|last)\(\)", f"{side}::{manager}()", after)
    if before != after:
        fixture.write_text(after)
