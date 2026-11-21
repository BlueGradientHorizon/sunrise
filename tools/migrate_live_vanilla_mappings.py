#!/usr/bin/env python3
"""Move Game's old mirrored held/click uses to its live mapping accessors."""
import re
from pathlib import Path

root = Path("crates/sunrise-client/src/game")
# These files define the routing or have other things called left/right; edit those by hand.
manual = {
    "interface/play_input.rs", "interface/play_keys.rs", "interface/chat.rs",
    "items/hand.rs", "modding/native_mapping_presses.rs", "modding/api_values.rs",
    "native_hooks/client_input_reads.rs", "network/lan_actions/turns.rs",
    "network/remote/mod.rs", "network/remote/input_burst_tests.rs",
    "network/remote/mapping_press_tests.rs", "network/viewed_worlds/worlds.rs",
}
for path in sorted(root.rglob("*.rs")):
    if path.relative_to(root).as_posix() in manual:
        continue
    old = code = path.read_text()
    for field, mapping in [("left", "key.attack"), ("right", "key.use")]:
        obj = rf"\b(self|game|g)\.{field}\b"
        code = re.sub(rf"{obj} = ([^;\n]+);", rf'\1.set_mapping_held("{mapping}", \2);', code)
        code = re.sub(obj, rf'\1.mapped_key_down("{mapping}")', code)
    for old_name, new_name in (
        ("held", "held_mapping"), ("press", "press_device_key"),
        ("release", "release_device_key"), ("insert", "insert_device_key"),
        ("remove", "remove_device_key"), ("clear", "clear_device_keys"),
    ):
        code = re.sub(rf"\b(self|game|g)\.keys\.{old_name}\(", rf"\1.{new_name}(", code)
    code = re.sub(
        r"game\.remote\.as_mut\(\)\.unwrap\(\)\.input\.press\(Button::(Attack|Use|Pick)\);",
        r"game.queue_mapping_click(Button::\1.mapping());", code,
    )
    if code != old:
        path.write_text(code)
