#!/usr/bin/env python3
"""Convert the load and loop metadata getters on the item 30 base, before rustfmt."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class Source:
    def __init__(self, path: str):
        self.path = ROOT / path
        self.text = self.path.read_text(encoding="utf-8")

    def block(self, header: str, *edits):
        # These declarations close at their opening indentation. Do not match
        # similarly named getters in a different trait, impl or fixture.
        indent = header[: len(header) - len(header.lstrip())]
        pattern = rf"^{re.escape(header)}\n.*?^{indent}}}$"
        matches = list(re.finditer(pattern, self.text, re.MULTILINE | re.DOTALL))
        if len(matches) != 1:
            raise ValueError(f"{self.path}: expected one block {header!r}, got {len(matches)}")
        match = matches[0]
        text = match[0]
        for old, new, *count in edits:
            expected = count[0] if count else 1
            actual = text.count(old)
            if actual != expected:
                raise ValueError(
                    f"{self.path}: {header!r}: expected {expected} of {old!r}, got {actual}"
                )
            text = text.replace(old, new)
        self.text = self.text[: match.start()] + text + self.text[match.end() :]


REGISTRY = ("Option<Arc<[String]>>", "Option<&[String]>")
SETTING = ("Option<String>", "Option<&str>")
MODS = ("fn mods(&self) -> LoadedMods", "fn mods(&self) -> &LoadedMods")
MODS_BODY = ("self.mods.clone()", "&self.mods")


def no_deref(call: str, count: int = 1):
    return (call + ".as_deref()", call, count)


def main():
    sources = []

    def source(path: str) -> Source:
        result = Source(f"crates/{path}")
        sources.append(result)
        return result

    phase = source("sunrise-mod-vocab/src/world/phase.rs")
    for trait, registry_doc, setting_doc in (
        (
            "DataLoadContext",
            "Registry names in ID order, from this resource generation's snapshot.",
            "A loaded mod's setting captured at the owning load boundary, without file I/O.",
        ),
        (
            "LoopContext",
            "Registry names in ID order, from the resource generation's snapshot.",
            "A loaded mod's setting captured when this context was built, without file I/O.",
        ),
    ):
        phase.block(
            f"pub trait {trait} {{",
            REGISTRY,
            SETTING,
            MODS,
            (registry_doc, "Borrows registry names in ID order from this resource generation."),
            (setting_doc, "Borrows a setting captured for this generation, without file I/O."),
            (
                "The same loaded-mod view available during registration.",
                "Borrows the loaded-mod inventory in dispatch order.",
            ),
        )
    for trait in ("LoadReader", "LoopReader"):
        phase.block(
            f"pub trait {trait}: Sync {{",
            REGISTRY,
            SETTING,
            ("Registry names in ID order.", "Borrows registry names in ID order."),
            ("A captured setting, without file I/O.", "Borrows a captured setting, without file I/O."),
        )
    for impl in ("DataLoadContext for OwningLoad", "LoopContext for OwningLoop"):
        phase.block(f"impl {impl}<'_> {{", REGISTRY, SETTING, MODS, MODS_BODY)
    phase.block(
        "pub trait WorldgenHost {",
        MODS,
        (
            "The same loaded-mod view available during registration.",
            "Borrows the loaded-mod inventory in dispatch order.",
        ),
    )
    phase.block("impl WorldgenHost for OwningWorldgen<'_> {", MODS, MODS_BODY)

    world = source("sunrise-world/src/native_load_context.rs")
    world.block(
        "impl Snapshot {",
        ("Option<Arc<[String]>>", "Option<&Arc<[String]>>"),
        ("self.inputs.registries.get(name).cloned()", "self.inputs.registries.get(name)"),
        SETTING,
        ("Some(values[at].1.clone())", "Some(values[at].1.as_str())"),
    )
    for context in ("LoadContext", "LoopContext"):
        world.block(
            f"impl {context} {{",
            REGISTRY,
            SETTING,
            ("self.snapshot.registry(name)", "self.snapshot.registry(name).map(Arc::as_ref)"),
        )
    world.block(
        "impl WorldgenContext {",
        ("self.snapshot.registry(name)", "self.snapshot.registry(name).cloned()"),
        (
            "self.snapshot.setting(mod_id, key)",
            "self.snapshot.setting(mod_id, key).map(str::to_owned)",
        ),
    )
    world.block(
        "mod tests {",
        no_deref('audio.setting("reader", "value")'),
        no_deref('loop_context.setting("reader", "mode")', 2),
        no_deref('next.setting("reader", "mode")'),
        no_deref('next.setting("reader", "late")'),
        no_deref('before.setting("active", "mode")', 2),
        no_deref('before.setting("active", "empty")'),
        no_deref('after.setting("active", "mode")', 2),
        no_deref('after.setting("active", "late")'),
        no_deref('client.registry("item")'),
        no_deref('client.setting("active", "mode")'),
        no_deref('server.setting("active", "mode")'),
        no_deref('load.setting("active", "mode")'),
        no_deref('load.setting("active", "ground")'),
    )

    renderer = source("sunrise-client-render/src/api_context.rs")
    for reader in ("LoadReader", "LoopReader"):
        renderer.block(f"impl mod_api::{reader} for {reader}<'_> {{", REGISTRY, SETTING)
    renderer.block(
        "    fn phase_readers_preserve_origins_sides_and_shared_resource_members() {",
        no_deref('context.registry("item")', 2),
        no_deref('context.setting("active", "mode")', 2),
        (
            'Arc::ptr_eq(\n                    &context.registry("item").unwrap(),\n'
            '                    &load.registry("item").unwrap()',
            'std::ptr::eq(\n                    context.registry("item").unwrap(),\n'
            '                    load.registry("item").unwrap()',
        ),
    )

    source("sunrise-client/src/crafting/mod_kinds.rs").block(
        "impl mod_api::LoopContext for CraftingRemainderView<'_> {",
        REGISTRY,
        SETTING,
        ("-> mod_api::LoadedMods", "-> &mod_api::LoadedMods"),
    )

    load_state = source("sunrise-mod-api/src/load_state.rs")
    testing = source("sunrise-mod-vocab/src/testing.rs")
    registry_body = (
        "self.registries.get(name).cloned()",
        "self.registries.get(name).map(AsRef::as_ref)",
    )
    for reader in ("LoadReader", "LoopReader"):
        load_state.block(f"impl {reader} for TestReader {{", REGISTRY, SETTING, registry_body)
        testing.block(
            f"impl crate::{reader} for TestReader {{",
            ("Option<std::sync::Arc<[String]>>", "Option<&[String]>"),
            SETTING,
            registry_body,
        )
    load_state.block(
        "    fn a_loop_context_reads_its_side_registries_and_mods() {",
        no_deref('context.registry("item")'),
    )
    source("sunrise-mod-vocab/src/loot_modifiers/tests.rs").block(
        "impl LoadReader for Reader {", REGISTRY, SETTING
    )
    source("sunrise-mod-api/src/parts/crafting_recipes.rs").block(
        "    impl crate::LoopContext for Context {",
        REGISTRY,
        SETTING,
        ("-> crate::LoadedMods", "-> &crate::LoadedMods"),
    )

    source("sunrise-client/src/game/modding/mods/item_data_load.rs").block(
        "    fn startup_keeps_registration_settings_and_later_load_captures_changes() {",
        (
            '.setting("reader", "count")\n                .as_deref()',
            '.setting("reader", "count")',
            3,
        ),
        no_deref('fallback.setting("reader", "count")'),
    )
    source("sunrise-client/tests/load_defaults.rs").block(
        "fn default_components_reader_baseline() -> client::Result<()> {",
        (
            '*context.registry("minecraft:item").unwrap()',
            'context.registry("minecraft:item").unwrap()',
        ),
    )
    source("sunrise-client/src/native_client_hooks_tests.rs").block(
        "pub(crate) fn install() {",
        ("= crate::text::Text::literal(&value);", "= crate::text::Text::literal(value);"),
        (".map(|value| crate::text::Text::literal(&value))", ".map(crate::text::Text::literal)"),
    )

    # Write nothing until all scopes and replacement counts have been checked.
    for src in sources:
        src.path.write_text(src.text, encoding="utf-8")


if __name__ == "__main__":
    main()
