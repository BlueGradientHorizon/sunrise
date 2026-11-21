#!/usr/bin/env python3
"""Wrap the existing per-opening layout and its fixture literals in MenuScreen."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VOCAB = ROOT / "crates/sunrise-mod-vocab/src/client/mod.rs"
HOOKS = ROOT / "crates/sunrise-client/src/game/native_hooks"


def migrate() -> None:
    src = VOCAB.read_text()
    anchor = "pub(crate) mod mod_key_refusal;"
    assert src.count(anchor) == 1
    VOCAB.write_text(src.replace(anchor, "pub(crate) mod menu_screen;\n" + anchor))

    for name in (
        "shared.rs",
        "screen_slot_tests.rs",
        "shared/book_screen.rs",
        "shared/book_screen/lifecycle_tests.rs",
        "shared/book_screen/open.rs",
        "shared/book_screen/placement.rs",
        "shared/label_tests.rs",
        "shared/screen_position_tests.rs",
        "shared/slot_icon_tests.rs",
        "shared/slot_tooltip_tests.rs",
    ):
        path = HOOKS / name
        src = path.read_text()
        src = src.replace("screen: mod_api::MenuScreenLayout,", "screen: mod_api::MenuScreen,")
        src = src.replace("            screen,\n            shown: false,", "            screen: mod_api::MenuScreen::new(screen),\n            shown: false,")
        src = src.replace("screen: mod_api::MenuScreenLayout::default(),", "screen: mod_api::MenuScreen::new(mod_api::MenuScreenLayout::default()),")
        src = re.sub(
            r"^( +)screen: (mod_api::MenuScreenLayout \{.*?^\1\}),$",
            r"\1screen: mod_api::MenuScreen::new(\2),",
            src,
            flags=re.DOTALL | re.MULTILINE,
        )
        src = src.replace("&mut game.mod_screen.as_mut().unwrap().screen", "game.mod_screen.as_mut().unwrap().layout_mut()")
        src = src.replace("game.mod_screen.as_mut().unwrap().screen.slots", "game.mod_screen.as_mut().unwrap().layout_mut().slots")
        src = re.sub(
            r"\.screen(?=\s*\.(slots|own_background|own_labels|width|height|inventory_x|inventory_y|inventory_label|title_label|recipe_book\w*)\b)",
            ".layout()",
            src,
        )
        if name == "shared.rs":
            anchor = "impl OpenNativeScreen {\n"
            assert src.count(anchor) == 1
            src = src.replace(anchor, anchor + """    fn layout(&self) -> &mod_api::MenuScreenLayout {
        mod_api::__private::menu_screen_layout(&self.screen)
    }

    #[cfg(test)]
    fn layout_mut(&mut self) -> &mut mod_api::MenuScreenLayout {
        mod_api::__private::menu_screen_layout_mut(&mut self.screen)
    }

""")
        path.write_text(src)


if __name__ == "__main__":
    migrate()
