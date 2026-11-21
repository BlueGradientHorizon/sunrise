#!/usr/bin/env python3
"""Thread `seed: i32` through `Game::draw_item_model` and every call site."""

from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
GAME = "crates/sunrise-client/src/game"

CALL_COUNTS = {
    f"{GAME}/entities/pvp.rs": 1,
    f"{GAME}/interface/advancements/mod.rs": 2,
    f"{GAME}/interface/dialogs.rs": 1,
    f"{GAME}/interface/gamemode_switcher.rs": 1,
    f"{GAME}/interface/inventory_slots.rs": 2,
    f"{GAME}/interface/item_draw.rs": 1,
    f"{GAME}/items/recipe_book.rs": 1,
    f"{GAME}/modding/gui_paint_items.rs": 1,
    f"{GAME}/modding/gui_paint_items_tests.rs": 1,
    f"{GAME}/modding/native_client_services.rs": 1,
    f"{GAME}/native_hooks/base.rs": 3,
}

EDITS = {
    f"{GAME}/interface/item_draw.rs": [
        (
            "        self.game\n            .draw_item_model(batch, &ItemStack::new(id, 1), x, y);",
            "        self.game\n            .draw_item_model(batch, &ItemStack::new(id, 1), x, y, 0);",
            1,
        ),
        (
            "    /// `GuiGraphics.renderItem(player, level, stack, x, y, 0)`.\n"
            "    pub(super) fn draw_item_model(&self, batch: &mut HudBatch, stack: &ItemStack, x: f32, y: f32) {\n"
            "        let owner = self.player_owner(Some(stack));\n"
            "        let state = self.resolve_item(stack, DisplayContext::Gui, Some(&owner), 0);",
            "    /// `GuiGraphicsExtractor.item(player, level, stack, x, y, seed)`.\n"
            "    pub(super) fn draw_item_model(\n"
            "        &self,\n"
            "        batch: &mut HudBatch,\n"
            "        stack: &ItemStack,\n"
            "        x: f32,\n"
            "        y: f32,\n"
            "        seed: i32,\n"
            "    ) {\n"
            "        let owner = self.player_owner(Some(stack));\n"
            "        let state = self.resolve_item(stack, DisplayContext::Gui, Some(&owner), seed);",
            1,
        ),
    ],
    f"{GAME}/entities/pvp.rs": [
        (
            "self.draw_item_model(batch, stack, ix, iy);",
            "self.draw_item_model(batch, stack, ix, iy, 0);",
            1,
        ),
    ],
    f"{GAME}/interface/advancements/mod.rs": [
        (
            "self.draw_item_model(batch, &ItemStack::new(station, 1), x + 3.0, y + 3.0);",
            "self.draw_item_model(batch, &ItemStack::new(station, 1), x + 3.0, y + 3.0, 0);",
            1,
        ),
        (
            "self.draw_item_model(batch, item, x + 8.0, y + 8.0);",
            "self.draw_item_model(batch, item, x + 8.0, y + 8.0, 0);",
            1,
        ),
    ],
    f"{GAME}/interface/dialogs.rs": [
        (
            "self.draw_item_model(batch, &stack, x, y);",
            "self.draw_item_model(batch, &stack, x, y, 0);",
            1,
        ),
    ],
    f"{GAME}/interface/gamemode_switcher.rs": [
        (
            "            self.draw_item_model(\n"
            "                batch,\n"
            "                &ItemStack::new(ICONS[index], 1),\n"
            "                (x + 5) as f32,\n"
            "                (y + 5) as f32,\n"
            "            );",
            "            self.draw_item_model(\n"
            "                batch,\n"
            "                &ItemStack::new(ICONS[index], 1),\n"
            "                (x + 5) as f32,\n"
            "                (y + 5) as f32,\n"
            "                0,\n"
            "            );",
            1,
        ),
    ],
    f"{GAME}/interface/inventory_slots.rs": [
        (
            "self.draw_item_model(batch, stack, x, y);",
            "self.draw_item_model(batch, stack, x, y, 0);",
            2,
        ),
    ],
    f"{GAME}/items/recipe_book.rs": [
        (
            "self.draw_item_model(batch, stack, *x as f32, *y as f32)",
            "self.draw_item_model(batch, stack, *x as f32, *y as f32, 0)",
            1,
        ),
    ],
    f"{GAME}/modding/gui_paint_items.rs": [
        (
            "    fn item(&mut self, batch: &mut HudBatch, slot: u32, x: i32, y: i32, _seed: i32) {\n"
            "        if let Some(stack) = self.stack(slot) {\n"
            "            self.game.draw_item_model(batch, stack, x as f32, y as f32);\n"
            "        }\n"
            "    }",
            "    fn item(&mut self, batch: &mut HudBatch, slot: u32, x: i32, y: i32, seed: i32) {\n"
            "        if let Some(stack) = self.stack(slot) {\n"
            "            self.game\n"
            "                .draw_item_model(batch, stack, x as f32, y as f32, seed);\n"
            "        }\n"
            "    }",
            1,
        ),
    ],
    f"{GAME}/modding/gui_paint_items_tests.rs": [
        (
            "            game.draw_item_model(\n"
            "                &mut plain,\n"
            '                &ItemStack::new("minecraft:stone", 5),\n'
            "                20.0,\n"
            "                20.0,\n"
            "            );",
            "            game.draw_item_model(\n"
            "                &mut plain,\n"
            '                &ItemStack::new("minecraft:stone", 5),\n'
            "                20.0,\n"
            "                20.0,\n"
            "                0,\n"
            "            );",
            1,
        ),
    ],
    f"{GAME}/modding/native_client_services.rs": [
        (
            "self.draw_item_model(batch, stack, at[0], at[1]);",
            "self.draw_item_model(batch, stack, at[0], at[1], 0);",
            1,
        ),
    ],
    f"{GAME}/native_hooks/base.rs": [
        (
            "self.draw_item_model(batch, &ItemStack::new(item, 1), at[0], at[1]);",
            "self.draw_item_model(batch, &ItemStack::new(item, 1), at[0], at[1], 0);",
            1,
        ),
        (
            "self.draw_item_model(batch, stack, at[0], at[1]);",
            "self.draw_item_model(batch, stack, at[0], at[1], 0);",
            1,
        ),
        (
            "self.draw_item_model(batch, &stack, at[0], at[1]);",
            "self.draw_item_model(batch, &stack, at[0], at[1], 0);",
            1,
        ),
    ],
    "tests/test_native_item_stack.py": [
        (
            "    fn draw_item_model(&self, batch: &mut HudBatch, stack: &ItemStack, x: f32, y: f32) {",
            "    fn draw_item_model(&self, batch: &mut HudBatch, stack: &ItemStack, x: f32, y: f32, _seed: i32) {",
            1,
        ),
    ],
}

CALL = re.compile(r"\.draw_item_model\s*\(")


def main() -> None:
    paths = [
        p
        for p in subprocess.check_output(
            ["git", "grep", "-lwz", "draw_item_model", "--", "*.rs"],
            cwd=ROOT,
            text=True,
            encoding="utf-8",
        )
        .strip("\0")
        .split("\0")
        if p
    ]
    sources = {path: (ROOT / path).read_text(encoding="utf-8") for path in paths}
    for path in sources.keys() | CALL_COUNTS.keys():
        expected = CALL_COUNTS.get(path, 0)
        actual = len(CALL.findall(sources.get(path, "")))
        if actual != expected:
            raise SystemExit(
                f"{path}: expected {expected} draw_item_model calls, found {actual}"
            )

    for rel, edits in EDITS.items():
        path = ROOT / rel
        text = path.read_text(encoding="utf-8")
        for old, new, count in edits:
            if text.count(old) != count:
                raise SystemExit(
                    f"{rel}: expected {count} occurrence(s) of {old!r}, found {text.count(old)}"
                )
            text = text.replace(old, new)
        path.write_text(text, encoding="utf-8")


if __name__ == "__main__":
    main()
