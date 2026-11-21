#!/usr/bin/env python3
"""Initialize the repeat attributes on the existing HUD vertex constructors."""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
COUNTS = {
    "crates/sunrise-client-render/src/hud.rs": 5,
    "crates/sunrise-client-render/src/hud/pack_text.rs": 1,
    "crates/sunrise-client-render/src/hud_kit/shapes.rs": 1,
    "crates/sunrise-client/src/debug_overlay.rs": 1,
}

for name, expected in COUNTS.items():
    path = ROOT / name
    before = path.read_bytes()
    if name.endswith("/hud.rs"):
        before = before.replace(
            b"    pub text: u32,\n}",
            b"    pub text: u32,\n    /// Fixed-point repeat phase per device pixel, zero for an ordinary vertex.\n    pub repeat_x: [u32; 4],\n    pub repeat_y: [u32; 3],\n}",
            1,
        )
    after, count = re.subn(
        rb"(?m)^( +)(glint_uv: [^\n]+,\n\1text(?:: [01])?,)$",
        rb"\1\2\n\1repeat_x: [0; 4],\n\1repeat_y: [0; 3],",
        before,
    )
    assert count == expected, (name, count)
    path.write_bytes(after)
