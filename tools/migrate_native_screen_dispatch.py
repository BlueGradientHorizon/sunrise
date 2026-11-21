#!/usr/bin/env python3
"""Rename the menu opener route and bind direct screen fixtures to an instance owner."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"


def replace(path, old, new):
    path = CRATES / path
    source = path.read_text()
    assert old in source, (path, old)
    path.write_text(source.replace(old, new))


replace("sunrise-mod-api-gen/src/dispatch.rs", "screen_routed", "opener_routed")
replace("sunrise-mod-api-gen/src/dispatch.rs", "menu_screen({key}, block)", "menu_for_opener({key}, block)")
replace("sunrise-mod-api-gen/src/dispatch.rs", "HookParts::menu_screen", "HookParts::menu_for_opener")
replace("sunrise-mod-vocab/src/parts/menus.rs", "fn menu_screen(", "fn menu_for_opener(")
replace("sunrise-mod-api-gen/dispatch-tests/src/fixtures/shared.rs", "fn menu_screen(", "fn menu_for_opener(")

for path in (
    "shared/label_tests.rs",
    "shared/slot_icon_tests.rs",
    "shared/book_screen/lifecycle_tests.rs",
    "screen_slot_tests.rs",
):
    path = CRATES / "sunrise-client/src/game/native_hooks" / path
    source = path.read_text()
    # The direct fixtures have no mod-owned callables. Bind the layout to the
    # fixture's owner and an unused dispatch index; production uses the registered index.
    # Work on each literal separately, leaving the neighboring factory fixtures alone.
    def bind(match):
        opening = match[0]
        owner = re.search(r'(?m)^ *owner: ("\w+"),\n', opening)
        assert owner is not None, path
        opening = opening[:owner.start()] + opening[owner.end():]
        return re.sub(
            r"(?ms)^( +)screen: (mod_api::MenuScreen::new\((?:[^\n]*\)|.*?^\1\}\)))(?=,\n)",
            lambda m: f"{m[1]}screen: mod_api::__private::OwnedMenuScreen::new({owner[1]}, 0, {m[2]})",
            opening,
        )

    after, count = re.subn(r"(?ms)^( +)[^\n]*\bOpenNativeScreen \{\n.*?^\1\}\)?;", bind, source)
    assert count > 0 and after.count("OwnedMenuScreen::new") == count, path
    path.write_text(after)
