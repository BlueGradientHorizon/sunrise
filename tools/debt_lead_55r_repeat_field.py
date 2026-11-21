#!/usr/bin/env python3
"""Rename the HUD repeat field at the lead-55r base, before changing its payload."""

import argparse
from pathlib import Path
import re


SOURCES = (
    "crates/sunrise-client-render/src/hud.rs",
    "crates/sunrise-client-render/src/hud/pack_text.rs",
    "crates/sunrise-client-render/src/hud_kit/shapes.rs",
    "crates/sunrise-client-render/src/ui.rs",
    "crates/sunrise-client/src/debug_overlay.rs",
    "crates/sunrise-client/src/game/host/draw/gui_canvas.rs",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path, help="checkout of the lead-55r base to update")
    root = parser.parse_args().root
    updates = []
    for source in SOURCES:
        path = root / source
        text, changes = re.subn(rb"\brepeat_x\b", b"repeat", path.read_bytes())
        if not changes:
            parser.error(f"{path}: expected the lead-55r base field spelling")
        updates.append((path, text))
    for path, text in updates:
        path.write_bytes(text)


if __name__ == "__main__":
    main()
