#!/usr/bin/env python3
"""Route food and XP setters through the shared player turn instead of a second lookup."""

from pathlib import Path

path = Path(__file__).resolve().parents[1] / (
    "crates/sunrise-client/src/game/modding/native_player_food.rs"
)
source = path.read_text()
path.write_text(source.replace("self.native_player_write(", "self.native_as_player_handle("))
