#!/usr/bin/env python3
"""Delete the mod-only GUI replay path; leave the engine's HUD draw list alone."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GAME = ROOT / "crates/sunrise-client/src/game"
VOCAB = ROOT / "crates/sunrise-mod-vocab/src"
GEN = ROOT / "crates/sunrise-mod-api-gen"
RENDER = ROOT / "crates/sunrise-client-render/src"


def change(path, old, new=""):
    source = path.read_text()
    if source.count(old) != 1:
        raise SystemExit(f"{path.relative_to(ROOT)}: expected one {old!r}")
    path.write_text(source.replace(old, new))


def cut(path, first, after):
    source = path.read_text()
    start = source.index(first)
    end = source.index(after, start + len(first))
    path.write_text(source[:start] + source[end:])


def main():
    cut(VOCAB / "client/hud_layout.rs", "/// One `GuiGraphicsExtractor` call.",
        "#[derive(Clone, Copy, PartialEq, Eq, Debug)]")
    change(VOCAB / "client/hud_layout.rs", "use std::{borrow::Cow, sync::Arc};", "use std::sync::Arc;")
    change(VOCAB / "exports/hud_layout.rs", "{GuiDraw, HealthHudReplacement, HealthInputs}",
           "{HealthHudReplacement, HealthInputs}")
    change(VOCAB / "client/mod.rs", "pub(crate) mod gui_paint;\n")
    cut(VOCAB / "host/draw.rs", "    /// Paint GUI extraction calls", "    /// Vanilla's nine-slice")
    cut(GAME / "host/draw.rs", "    fn gui_paint(", "    fn gui_nine_slice(")

    change(GAME / "modding/api_values.rs", '#[path = "api_values/gui_draw.rs"]\nmod gui_draw;\n')
    change(GAME / "modding/api_values.rs", "mod gui_draw_boundary_tests {", "mod text_boundary_tests {")
    cut(GAME / "modding/api_values.rs", "    use serde_json::json;\n    use sunrise_client::{\n        hud_layout::{Blend, GuiDraw as D, PoseOp},",
        "    #[test]\n    fn tooltip_pairs_round_trip_without_an_item_registry()")

    change(GEN / "src/dispatch.rs", '&["GuiDraw", "Text"]', '&["Text"]')
    cut(GEN / "src/dispatch.rs", "    #[test]\n    fn gui_snapshot_and_writeback_are_guarded_and_context_free()",
        "    #[test]\n    fn completion_hook_converts_only_after_subscription_check()")
    change(GEN / "src/host.rs", '&["Text", "GuiDraw"]', '&["Text"]')
    change(GEN / "dispatch-tests/src/fixtures/api/client.rs", "        pub struct GuiDraw;\n")

    hotbar = GAME / "modding/gui_paint_items.rs"
    cut(hotbar, "//! ", "use super::Game;")
    change(hotbar, "use mod_api::GuiPaintError;\n")
    change(hotbar, "use sunrise_client::hud_layout::{self, GuiDraw, GuiItems, Undrawable};",
           "use sunrise_client::hud_layout::GuiItems;")
    source = hotbar.read_text().split("/// The API's name for the op", 1)[0].rstrip()
    hotbar.with_name("hotbar_items.rs").write_text(
        "//! The hotbar's `GuiGraphicsExtractor.extractSlot`: the item model followed by decorations.\n"
        + source + "\n"
    )
    change(GAME.with_suffix(".rs"),
           "// `extractSlot` for each item, in order, on the path `gui_paint` takes.\n"
           "                    let mut items = crate::game::gui_paint_items::HotbarItems { game: g };",
           "// `extractSlot` for each item, in order.\n"
           "                    let mut items = crate::game::hotbar_items::HotbarItems { game: g };")
    change(GAME / "modding/gui_paint_items_tests.rs", "gui_paint_items::HotbarItems", "hotbar_items::HotbarItems")

    for path in (
        VOCAB / "client/gui_paint.rs",
        VOCAB / "exports/gui_paint.rs",
        GAME / "modding/api_values/gui_draw.rs",
        GAME / "modding/gui_pose.rs",
        hotbar,
    ):
        path.unlink()


if __name__ == "__main__":
    main()
