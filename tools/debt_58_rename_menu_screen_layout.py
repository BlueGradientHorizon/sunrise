#!/usr/bin/env python3
"""Rename the mod menu layout record without touching the engine's Screen."""

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GAME = "crates/sunrise-client/src/game"
VOCAB = "crates/sunrise-mod-vocab/src"
API = "crates/sunrise-mod-api/src"
FIXTURES = "crates/sunrise-mod-api-gen/dispatch-tests/src/fixtures"
BOOK = "docs/modding/src"
QUALIFIED = re.compile(rb"\b(mod_api|crate)::Screen\b")
QUALIFIED_FILES = {
    f"{GAME}/native_hooks/base.rs": 1,
    f"{GAME}/native_hooks/shared.rs": 1,
    f"{GAME}/native_hooks/screen_slot_tests.rs": 1,
    f"{GAME}/native_hooks/shared/book_screen/open.rs": 2,
    f"{GAME}/native_hooks/shared/book_screen/lifecycle_tests.rs": 4,
    f"{GAME}/native_hooks/shared/label_tests.rs": 1,
    f"{GAME}/native_hooks/shared/screen_position_tests.rs": 4,
    f"{GAME}/native_hooks/shared/slot_icon_tests.rs": 3,
    f"{GAME}/native_hooks/shared/slot_tooltip_tests.rs": 1,
    f"{GAME}/modding/mods/menu_books.rs": 4,
    f"{GAME}/modding/native_menu_places.rs": 2,
    f"{GAME}/modding/native_menu_refs.rs": 2,
    f"{GAME}/modding/native_menus.rs": 1,
    f"{VOCAB}/item/recipe_book_layout.rs": 2,
    f"{VOCAB}/parts/menus.rs": 2,
    f"{API}/parts/menus.rs": 1,
    f"{API}/parts/menus/book_tests.rs": 2,
    "docs/design/mod-api/recipe-book-types.md": 2,
    f"{BOOK}/how-to/world/machine-menu-data.md": 2,
}
EXACT_EDITS = {
    f"{VOCAB}/client/types.rs": (
        (b"pub struct Screen {", b"pub struct MenuScreenLayout {"),
        (
            b"A menu type's screen, a mod's (`screen`)",
            b"Layout for a mod menu type's screen (`screen`)",
        ),
    ),
    f"{FIXTURES}/base.rs": (
        (b"pub(super) struct Screen;", b"pub(super) struct MenuScreenLayout;"),
    ),
    f"{FIXTURES}/shared.rs": (
        (b"MapInfo, Screen, Zoom,", b"MapInfo, MenuScreenLayout, Zoom,"),
    ),
    f"{GAME}/native_hooks/shared.rs": (
        (b"positions its `Screen` declared", b"positions its `MenuScreenLayout` declared"),
    ),
    f"{VOCAB}/host/screens.rs": ((b"`Screen::slots`", b"`MenuScreenLayout::slots`"),),
    f"{VOCAB}/item/recipe_book_ghost.rs": (
        (b"`Screen::recipe_book_big_result`", b"`MenuScreenLayout::recipe_book_big_result`"),
    ),
    "docs/design/mod-api/struct-kinds.md": (
        (
            b"let screen = Screen { width: 176.0, height: 166.0, ..Screen::default() };",
            b"let screen = MenuScreenLayout { width: 176.0, height: 166.0, ..MenuScreenLayout::default() };",
        ),
    ),
    "docs/fabric-ledger.md": ((b"return a `Screen` layout", b"return a `MenuScreenLayout`"),),
    f"{BOOK}/concepts/recipe-book-types.md": (
        (b"`Screen::recipe_book_toggle`", b"`MenuScreenLayout::recipe_book_toggle`"),
    ),
    f"{BOOK}/how-to/client/menu-gauges.md": (
        (b"machine's `Screen`", b"machine's `MenuScreenLayout`"),
    ),
    f"{BOOK}/how-to/client/menu-slot-icons.md": ((b"`Screen.slots`", b"`MenuScreenLayout.slots`"),),
    f"{BOOK}/how-to/items/recipe-book-categories.md": (
        (b"`Screen::recipe_book_big_result`", b"`MenuScreenLayout::recipe_book_big_result`"),
    ),
}
# Only the audited inline references to the layout in these two pages.
LAYOUT_REFS = {
    "docs/design/mod-api/recipe-book-types.md": (
        rb"`Screen(?=`|\.(?:recipe_book(?:_toggle)?|slots)`)", 7,
    ),
    f"{BOOK}/how-to/client/screens.md": (
        rb"`Screen(?=` literal|::(?:slots|title_label|inventory_label)`)", 9,
    ),
}


def main():
    paths = sorted(QUALIFIED_FILES.keys() | EXACT_EDITS.keys() | LAYOUT_REFS.keys())
    subprocess.run(
        ["git", "ls-files", "--error-unmatch", "--", *paths],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        check=True,
    )
    # Validate every anchor before writing any files. This is a one-time rewrite.
    updates = {}
    for name in paths:
        path = ROOT / name
        source = path.read_bytes()
        if name in QUALIFIED_FILES:
            source, count = QUALIFIED.subn(rb"\1::MenuScreenLayout", source)
            if count != QUALIFIED_FILES[name]:
                raise SystemExit(f"{name}: unexpected qualified Screen reference count: {count}")
        for old, new in EXACT_EDITS.get(name, ()):
            if source.count(old) != 1:
                raise SystemExit(f"{name}: expected one {old.decode()!r}")
            source = source.replace(old, new)
        if name in LAYOUT_REFS:
            pattern, expected = LAYOUT_REFS[name]
            source, count = re.subn(pattern, b"`MenuScreenLayout", source)
            if count != expected:
                raise SystemExit(f"{name}: unexpected inline layout reference count: {count}")
        updates[path] = source

    for path, source in updates.items():
        path.write_bytes(source)


if __name__ == "__main__":
    main()
