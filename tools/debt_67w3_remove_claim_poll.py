#!/usr/bin/env python3
"""Remove the claim/poll surface and route existing capture ends by intent."""

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
VOCAB = "crates/sunrise-mod-vocab/src"
GAME = "crates/sunrise-client/src/game"


def replace(path, old, new="", count=1):
    file = ROOT / path
    data = changes.get(file, file.read_text())
    if data.count(old) != count:
        raise SystemExit(f"{path}: expected {count} occurrences of {old!r}")
    changes[file] = data.replace(old, new)


changes = {}
replace(
    f"{VOCAB}/host/client_input.rs",
    "    /// Side: client.\n"
    "    /// Right-clicks with these items (at most 64 ids) are mod `mod_id`'s; a later call replaces the list.\n"
    "    #[access(WorldOnly)]\n"
    "    fn claim_items(&mut self, mod_id: &str, ids: &[String]);\n"
    "    /// Side: client.\n"
    "    /// Mod `mod_id`'s events since its last poll (at most 256 wait).\n"
    "    #[access(WorldOnly)]\n"
    "    fn poll(&mut self, mod_id: &str) -> Vec<ModEvent>;\n",
)
for method in ("claim_items", "poll"):
    replace("crates/sunrise-mod-api-gen/unwinding-methods.txt", f"client_input::{method}\n")
for name, kind, doc in (
    ("HeldItemSlot", "struct", "A stack as a client mod's use events carry it."),
    ("ModEvent", "enum", "What happened to a mod's claimed item, oldest first."),
):
    path = f"{VOCAB}/game.rs"
    block = re.search(
        rf"^/// {re.escape(doc)}\n(?:#\[.*\]|//[^\n]*)\n"
        rf"(?:#\[.*\]|//[^\n]*)\n(?:#\[.*\]|//[^\n]*)\n"
        rf"pub {kind} {name} \{{[\s\S]*?^\}}\n",
        (ROOT / path).read_text(),
        re.MULTILINE,
    )
    if not block:
        raise SystemExit(f"{path}: could not find {name}")
    replace(path, block[0])
    replace(f"{VOCAB}/exports/game.rs", f"{name}, ")

for line in (
    "pub use native_claimed_items::{HeldItemSlot, ModEvent};\n",
    "    native_input: native_claimed_items::ModInput,\n",
    "            native_input: Default::default(),\n",
):
    replace(f"{GAME}.rs", line)

for path, count in (
    ("entities/player_residency.rs", 1),
    ("modding/native_claimed_items.rs", 1),
    ("network/remote/mod.rs", 7),
    ("network/remote/recipe_properties.rs", 1),
    ("runtime/replay.rs", 1),
):
    replace(
        f"{GAME}/{path}",
        "self.end_client_item_use();",
        "self.reset_client_item_use();",
        count=count,
    )
replace(
    f"{GAME}/network/remote/mod.rs",
    "!self.0.end_client_item_use()",
    "!self.0.end_client_item_use(true)",
)

for path, data in changes.items():
    path.write_text(data)
