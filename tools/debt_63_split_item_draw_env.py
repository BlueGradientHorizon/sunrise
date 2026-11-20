#!/usr/bin/env python3
"""Separate the client draw environment from the asset resolver's environment (debt 63)."""

from pathlib import Path

SRC = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src"


def change(path: str, old: str, new: str, count: int = 1) -> None:
    target = SRC / path
    text = target.read_text(encoding="utf-8")
    assert text.count(old) == count, (path, old, text.count(old))
    target.write_text(text.replace(old, new), encoding="utf-8")


def main() -> None:
    change(
        "item_render.rs",
        "/// Data special renderers read from the game (registries, skins).",
        "/// Client drawing services in addition to the asset resolver's host services.\n"
        "pub trait ItemDrawEnv: ItemEnv {}\n"
        "impl ItemDrawEnv for sunrise_assets::item_model::NoEnv {}\n\n"
        "/// Data special renderers read from the game (registries, skins).",
    )
    change("item_render.rs", "env: &dyn ItemEnv", "env: &dyn ItemDrawEnv", 3)
    change("game/interface/item_draw.rs", "impl ItemEnv for Env", "impl item_render::ItemDrawEnv for Env<'_, '_> {}\n\nimpl ItemEnv for Env")
    change("item_render/block_light_tests.rs", "impl ItemEnv for Plain {}", "impl ItemEnv for Plain {}\nimpl ItemDrawEnv for Plain {}")
    change("item_render/special_parts_tests.rs", "impl ItemEnv for PanEnv", "impl ItemDrawEnv for PanEnv<'_> {}\nimpl ItemEnv for PanEnv")
    for name in ("Charge", "Gas"):
        change("item_render/tint_tests.rs", f"impl ItemEnv for {name}", f"impl ItemDrawEnv for {name} {{}}\nimpl ItemEnv for {name}")
    change("title/flat.rs", "impl ItemEnv for GrassEnv", "impl item_render::ItemDrawEnv for GrassEnv<'_> {}\nimpl ItemEnv for GrassEnv")


if __name__ == "__main__":
    main()
