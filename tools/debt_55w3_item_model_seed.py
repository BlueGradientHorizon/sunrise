#!/usr/bin/env python3
"""Make the existing zero seed explicit at every GUI item model draw."""

import re
import subprocess
from pathlib import Path

GAME = "crates/sunrise-client/src/game"
ITEM_DRAW = f"{GAME}/interface/item_draw.rs"
FIXTURE = "tests/test_native_item_stack.py"
MODEL = "draw_item_model"
PARAMS = "&self, batch: &mut HudBatch, stack: &ItemStack, x: f32, y: f32"

# Exact argument lists and occurrence counts at the base of the rewrite.
INLINE = {
    "entities/pvp.rs": [("batch, stack, ix, iy", 1)],
    "interface/advancements/mod.rs": [
        ("batch, &ItemStack::new(station, 1), x + 3.0, y + 3.0", 1),
        ("batch, item, x + 8.0, y + 8.0", 1),
    ],
    "interface/dialogs.rs": [("batch, &stack, x, y", 1)],
    "interface/inventory_slots.rs": [("batch, stack, x, y", 2)],
    "interface/item_draw.rs": [("batch, &ItemStack::new(id, 1), x, y", 1)],
    "items/recipe_book.rs": [("batch, stack, *x as f32, *y as f32", 1)],
    "modding/gui_paint_items.rs": [("batch, stack, x as f32, y as f32", 1)],
    "modding/native_client_services.rs": [("batch, stack, at[0], at[1]", 1)],
    "native_hooks/base.rs": [
        ("batch, &ItemStack::new(item, 1), at[0], at[1]", 1),
        ("batch, stack, at[0], at[1]", 1),
        ("batch, &stack, at[0], at[1]", 1),
    ],
}
MULTILINE = {
    "interface/gamemode_switcher.rs": (
        "                batch,\n"
        "                &ItemStack::new(ICONS[index], 1),\n"
        "                (x + 5) as f32,\n"
        "                (y + 5) as f32,\n",
        "            ",
    ),
    "modding/gui_paint_items_tests.rs": (
        "                &mut plain,\n"
        '                &ItemStack::new("minecraft:stone", 5),\n'
        "                20.0,\n"
        "                20.0,\n",
        "            ",
    ),
}


def main():
    root = Path(__file__).resolve().parents[1]
    rules = {
        f"{GAME}/{name}": [
            (f"{MODEL}({args})", f"{MODEL}({args}, 0)", count) for args, count in calls
        ]
        for name, calls in INLINE.items()
    }
    for name, (args, indent) in MULTILINE.items():
        rules[f"{GAME}/{name}"] = [
            (f"{MODEL}(\n{args}{indent})", f"{MODEL}(\n{args}{indent}    0,\n{indent})", 1)
        ]

    rules[ITEM_DRAW].append((
        "    /// `GuiGraphics.renderItem(player, level, stack, x, y, 0)`.\n"
        f"    pub(super) fn {MODEL}({PARAMS}) {{\n"
        "        let owner = self.player_owner(Some(stack));\n"
        "        let state = self.resolve_item(stack, DisplayContext::Gui, Some(&owner), 0);",
        "    /// `GuiGraphics.renderItem(player, level, stack, x, y, seed)`.\n"
        f"    pub(super) fn {MODEL}({PARAMS}, seed: i32) {{\n"
        "        let owner = self.player_owner(Some(stack));\n"
        "        let state = self.resolve_item(stack, DisplayContext::Gui, Some(&owner), seed);",
        1,
    ))
    # This Rust stub stands in for Game in an existing Python test.
    rules[FIXTURE] = [(f"fn {MODEL}({PARAMS})", f"fn {MODEL}({PARAMS}, _seed: i32)", 1)]

    script = Path(__file__).resolve().relative_to(root).as_posix()
    matches = subprocess.check_output(
        ["git", "grep", "-lwz", MODEL, "--", ".", f":(exclude){script}"], cwd=root
    ).decode("utf-8")
    files = set(matches.rstrip("\0").split("\0"))
    if files != rules.keys():
        raise SystemExit(
            f"{MODEL} files changed: extra {sorted(files - rules.keys())}, "
            f"missing {sorted(rules.keys() - files)}"
        )

    edits = {}
    for name, replacements in rules.items():
        path = root / name
        source = path.read_text(encoding="utf-8")
        expected = sum(count for _, _, count in replacements)
        found = len(re.findall(rf"\b{MODEL}\b", source))
        if found != expected:
            raise SystemExit(f"{name}: expected {expected} {MODEL} references, found {found}")
        for old, new, expected in replacements:
            found = source.count(old)
            if found != expected:
                raise SystemExit(f"{name}: expected {expected} occurrences of {old!r}, found {found}")
            source = source.replace(old, new)
        edits[path] = source

    for path, source in edits.items():
        path.write_text(source, encoding="utf-8")
    subprocess.run(
        ["rustfmt", "--edition", "2024", "--config", "skip_children=true",
         *(str(path) for path in edits if path.suffix == ".rs")],
        cwd=root,
        check=True,
    )


if __name__ == "__main__":
    main()
