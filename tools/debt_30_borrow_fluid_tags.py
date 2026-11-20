#!/usr/bin/env python3
"""Migrate fluid-tag trait signatures and reader fixtures on the item 30 step 2 base."""

from pathlib import Path

CRATES = Path(__file__).resolve().parents[1] / "crates"
ROWS = "[(String, Arc<[String]>)]"
BORROWED = f"Option<&{ROWS}>"
SIGNATURE = "fn fluid_tags(&self) -> "


def main():
    edits = {}

    def replace(file, old, new, count=1):
        path = CRATES / file
        source = edits[path] if path in edits else path.read_text(encoding="utf-8")
        if source.count(old) != count:
            raise ValueError(f"{file}: expected {count} occurrences of {old!r}")
        edits[path] = source.replace(old, new)

    phase = "sunrise-mod-vocab/src/world/phase.rs"
    replace(phase, f"{SIGNATURE}Option<Arc<{ROWS}>>", f"{SIGNATURE}{BORROWED}", 2)
    replace(phase, f"{SIGNATURE}Arc<{ROWS}>;", f"{SIGNATURE}{BORROWED};")
    replace(phase, "Some(self.source.fluid_tags())", "self.source.fluid_tags()")

    renderer = "sunrise-client-render/src/api_context.rs"
    replace(renderer, f"{SIGNATURE}Arc<{ROWS}>", f"{SIGNATURE}{BORROWED}")
    replace(renderer, "self.0.fluid_tags().clone()", "self.0.fluid_tags()")

    load_state = "sunrise-mod-api/src/load_state.rs"
    for file, arc in (
        (load_state, "Arc"),
        ("sunrise-mod-vocab/src/testing.rs", "std::sync::Arc"),
    ):
        rows = ROWS.replace("Arc", arc)
        replace(file, f"pub tags: {arc}<{rows}>,", f"pub tags: Option<{arc}<{rows}>>,")
        replace(file, f"{SIGNATURE}{arc}<{rows}>", f"{SIGNATURE}Option<&{rows}>")
        replace(file, "self.tags.clone()", "self.tags.as_deref()")
    replace(load_state, "tags: Arc::from([]),", "tags: None,")

    replace(
        "sunrise-mod-api/src/parts/registration_fluid_capability_tests.rs",
        """tags: tags
                .into_iter()
                .map(|(k, v)| (k, Arc::from(v)))
                .collect::<Vec<_>>()
                .into(),""",
        """tags: Some(tags.into_iter().map(|(k, v)| (k, Arc::from(v))).collect()),""",
    )
    replace(
        "sunrise-mod-vocab/src/loot_modifiers/tests.rs",
        f"{SIGNATURE}Arc<{ROWS}> {{\n        Arc::from([])",
        f"{SIGNATURE}{BORROWED} {{\n        None",
    )

    for path, source in edits.items():
        path.write_text(source, encoding="utf-8")


if __name__ == "__main__":
    main()
