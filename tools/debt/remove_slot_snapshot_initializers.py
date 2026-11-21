#!/usr/bin/env python3
"""Remove obsolete SlotSnapshot initializers for mod API debt item 6."""

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[2] / "crates/sunrise-client/src"
INITIALIZER = re.compile(
    r"^ +slot_snapshot: (?:Default|SlotSnapshot)::default\(\),\n", re.MULTILINE
)

for name in ("menu.rs", "lan/menu.rs", "lan/containers.rs"):
    path = ROOT / name
    old = path.read_text(encoding="utf-8")
    new = INITIALIZER.sub("", old)
    if old != new:
        path.write_text(new, encoding="utf-8")
