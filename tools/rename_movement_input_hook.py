#!/usr/bin/env python3
"""Rename the movement input hook and its generated symbols for mod-API debt 68."""

import re
import subprocess
from pathlib import Path


RENAMES = {
    "fn movement_input": "fn movement_input_update",
    "`movement_input`": "`movement_input_update`",
    "HookId::MovementInput": "HookId::MovementInputUpdate",
    "MovementInputCall": "MovementInputUpdateCall",
    "MovementInputEvent": "MovementInputUpdateEvent",
    "dispatch_movement_input": "dispatch_movement_input_update",
    "fire_movement_input": "fire_movement_input_update",
    "native_movement_input": "native_movement_input_update",
    "mod_movement_keys": "update_movement_input",
}
PATTERN = "|".join(
    rf"\b{re.escape(name)}\b" if not name.startswith("`") else re.escape(name)
    for name in RENAMES
)


def main():
    root = Path(__file__).resolve().parents[1]
    files = subprocess.check_output(
        ["git", "ls-files", "-z", "crates/*.rs", "docs/*.md"], cwd=root
    ).decode().split("\0")
    for name in filter(None, files):
        path = root / name
        old = path.read_text(encoding="utf-8")
        new = re.sub(PATTERN, lambda match: RENAMES[match[0]], old)
        if old != new:
            path.write_text(new, encoding="utf-8")


if __name__ == "__main__":
    main()
