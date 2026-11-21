#!/usr/bin/env python3
"""One-shot debt 56w3 migration: borrow each vanilla HUD paint closure for the frame."""

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GAME = "crates/sunrise-client/src/game"
HELPERS = f"{GAME}/native_hooks/shared/world_helpers.rs"
CALL_COUNTS = {
    f"{GAME}.rs": 15,
    f"{GAME}/interface/chat.rs": 1,
    f"{GAME}/mobs/mounts.rs": 1,
    f"{GAME}/modding/gui_layer_tests.rs": 1,
}
CALL = re.compile(r"\.gui_layer\s*\(")
PAINT = re.compile(r"(\.gui_layer\(\s*[^,]+,\s*(?:&mut )?batch,\s*)(\|(?:g, batch|_, _)\|)")


def main():
    paths = subprocess.check_output(
        ["git", "grep", "-lwz", "gui_layer", "--", "*.rs"],
        cwd=ROOT, text=True, encoding="utf-8",
    ).strip("\0").split("\0")
    sources = {path: (ROOT / path).read_text(encoding="utf-8") for path in paths}
    edited = {}
    for path in sources.keys() | CALL_COUNTS.keys():
        source = sources.get(path, "")
        expected = CALL_COUNTS.get(path, 0)
        if len(CALL.findall(source)) != expected:
            raise SystemExit(f"{path}: expected {expected} HUD paint calls")
        if expected:
            new, count = PAINT.subn(r"\1&mut \2", source)
            if count != expected:
                raise SystemExit(f"{path}: expected {expected} unit-returning HUD paint closures")
            edited[path] = new

    helpers = sources[HELPERS]
    for old, new in (
        ("impl Game<'_> {", "impl<'g> Game<'g> {"),
        ("/// may hide it), the layer, then `native_gui_layer_post`. `None` when a mod hid\n"
         "    /// the layer.",
         "/// may hide it), the layer, then `native_gui_layer_post`."),
        ("fn gui_layer<R>(", "fn gui_layer("),
        ("        draw: impl FnOnce(&mut Self, &mut HudBatch) -> R,\n    ) -> Option<R> {",
         "        draw: &mut dyn FnMut(&mut Game<'g>, &mut HudBatch),\n    ) {"),
        ("        let drawn = (!canceled).then(|| draw(self, batch));\n"
         "        self.native_gui_layer_post(layer, batch);\n        drawn\n",
         "        if !canceled {\n            draw(self, batch);\n        }\n"
         "        self.native_gui_layer_post(layer, batch);\n"),
    ):
        if helpers.count(old) != 1:
            raise SystemExit(f"{HELPERS}: expected one occurrence of {old!r}")
        helpers = helpers.replace(old, new)
    edited[HELPERS] = helpers
    for path, source in sorted(edited.items()):
        (ROOT / path).write_text(source, encoding="utf-8")


if __name__ == "__main__":
    main()
