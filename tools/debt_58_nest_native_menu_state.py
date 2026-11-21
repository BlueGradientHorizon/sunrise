#!/usr/bin/env python3
"""Nest the existing native screen's menu state, without changing its lifetime."""

import re
from pathlib import Path

GAME = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src/game/native_hooks"
MENU_FIELDS = ("key", "window", "menu", "block", "mod_type", "data", "extra")
FIELD = re.compile(r"^ *(\w+)(:|,)")


def nest_menu_fields(text):
    # All these literals have one-line menu fields; the layout can be multiline.
    pattern = re.compile(r"^([ ]*)([^\n]*\bOpenNativeScreen \{\n)(.*?)(^\1\})", re.M | re.S)

    def nest(literal):
        indent = literal[1] + "    "
        menu = []
        screen = []
        fields = []
        for line in literal[3].splitlines(keepends=True):
            field = FIELD.match(line) if line.startswith(indent) and not line.startswith(indent + " ") else None
            if field and field[1] in MENU_FIELDS:
                fields.append(field[1])
                menu.append("    " + line)
            else:
                screen.append(line)
        if set(fields) != set(MENU_FIELDS) or len(fields) != len(MENU_FIELDS):
            raise ValueError(f"unexpected menu fields: {fields}")
        return (
            literal[1] + literal[2] + indent + "menu: OpenNativeMenu {\n"
            + "".join(menu) + indent + "},\n" + "".join(screen) + literal[4]
        )

    return pattern.sub(nest, text)


def replace(path, *edits, nest=False):
    path = GAME / path
    text = path.read_text()
    if nest:
        text = nest_menu_fields(text)
    for old, new in edits:
        if old not in text:
            raise ValueError(f"{path.name}: missing {old!r}")
        text = text.replace(old, new)
    path.write_text(text)


replace(
    "shared.rs",
    ("""/// The mod screen open on this client: its menu type and block, the key its
/// slots are under, the window id of its menu, the menu's data slots as
/// the server sent them, and what the server sent before the menu opened
/// (`sunrise:menu_extra`), when it did.
pub(super) struct OpenNativeScreen {
    pub(super) layout_revision: u64,
    slot_moves: SlotMoves,
    key: Pos,""", """/// The layout and local slot state of the mod screen on this client.
pub(super) struct OpenNativeScreen {
    pub(super) layout_revision: u64,
    slot_moves: SlotMoves,
    screen: mod_api::MenuScreenLayout,
    menu: OpenNativeMenu,
}

/// The menu the screen presents: where its slots are stored, the opener,
/// the window's live data, and the bytes supplied when it opened.
struct OpenNativeMenu {
    key: Pos,"""),
    ("    screen: mod_api::MenuScreenLayout,\n    mod_type:", "    mod_type:"),
    ("screen.window == window && screen.menu == name", "screen.menu.window == window && screen.menu.name == name"),
    ("            .mod_type\n    }\n}\n\nimpl OpenNativeScreen", "            .menu\n            .mod_type\n    }\n}\n\nimpl OpenNativeScreen"),
    ("    menu: String,\n    /// The mod block", "    name: String,\n    /// The mod block"),
    ("match &self.extra {", "match &self.menu.extra {"),
    ("None if self.key.0 != i32::MIN => Some([self.key.0, self.key.1, self.key.2]),", "None if self.menu.key.0 != i32::MIN => {\n                Some([self.menu.key.0, self.menu.key.1, self.menu.key.2])\n            }"),
    ("if window != self.window {", "if window != self.menu.window {"),
    ("self.mod_type.map_or(256,", "self.menu.mod_type.map_or(256,"),
    ("if self.data.len() <= index {", "if self.menu.data.len() <= index {"),
    ("self.data.resize(index + 1, 0);", "self.menu.data.resize(index + 1, 0);"),
    ("self.data[index] = value;", "self.menu.data[index] = value;"),
    ("        self.mod_type\n", "        self.menu.mod_type\n"),
    ("filter(|s| s.menu == menu)", "filter(|s| s.menu.name == menu)"),
    ("&s.menu, s.block.as_deref()", "&s.menu.name, s.menu.block.as_deref()"),
    ("menu: &s.menu,", "menu: &s.menu.name,"),
    ("block: s.block.as_deref(),", "block: s.menu.block.as_deref(),"),
    ("data: &s.data,", "data: &s.menu.data,"),
    ("            &screen.menu,\n            screen.block.as_deref(),\n            &screen.data,", "            &screen.menu.name,\n            screen.menu.block.as_deref(),\n            &screen.menu.data,"),
    ("menu: &screen.menu,\n            block: screen.block.as_deref(),\n            data: &screen.data,", "menu: &screen.menu.name,\n            block: screen.menu.block.as_deref(),\n            data: &screen.menu.data,"),
)
replace("shared/book_screen.rs", ("Some(s.key)", "Some(s.menu.key)"),
        ("Some(&self.shown_native_screen()?.menu)", "Some(&self.shown_native_screen()?.menu.name)"),
        ("self.shown_native_screen()?.block", "self.shown_native_screen()?.menu.block"))
replace("shared/book_screen/placement.rs", ("screen.window", "screen.menu.window"))
replace("menu_extra_data.rs", ("            screen\n                .extra", "            screen\n                .menu\n                .extra"))
replace("shared/book_screen/open.rs", ("menu: menu.to_owned()", "name: menu.to_owned()"), nest=True)
replace("screen_slot_tests.rs", ("menu: \"gtmod:menu\"", "name: \"gtmod:menu\""),
        ("menu: OpenNativeMenu", "menu: super::OpenNativeMenu"), ("assert_eq!(s.data", "assert_eq!(s.menu.data"), nest=True)
replace("shared/book_screen/lifecycle_tests.rs", ("menu: \"bound:menu\"", "name: \"bound:menu\""),
        (" menu: \"geometry:menu\"", " name: \"geometry:menu\""),
        ("menu: \"minecraft:generic_9x1\"", "name: \"minecraft:generic_9x1\""),
        ("menu: MENU.into()", "name: MENU.into()"), ("screen.block.as_deref()", "screen.menu.block.as_deref()"), nest=True)
replace("shared/label_tests.rs", ("menu: \"test:menu\"", "name: \"test:menu\""), nest=True)
replace("shared/slot_icon_tests.rs", ("menu: \"test:menu\"", "name: \"test:menu\""), nest=True)
replace("shared/slot_tooltip_tests.rs", ("menu: \"minecraft:generic_9x3\"", "name: \"minecraft:generic_9x3\""), nest=True)
replace("shared/screen_position_tests.rs", (".unwrap().extra.as_ref()", ".unwrap().menu.extra.as_ref()"),
        ("open.block", "open.menu.block"), ("open.menu.clone()", "open.menu.name.clone()"))
