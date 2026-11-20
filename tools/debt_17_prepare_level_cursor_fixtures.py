#!/usr/bin/env python3
"""Route the level-admission tests through a shared collector before swapping its source."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src/join_seq/residency"
CALL = re.compile(r"\b(f\.table|table)\.level_entities\((HOME\.dimension|DimensionKey::NETHER)\)")
MARKER = '#[cfg(test)]\n#[path = "table/completion_tests.rs"]\n'
COLLECTOR = """#[cfg(test)]
fn collect_level_entities(table: &mut EntityTable, dimension: DimensionKey) -> Vec<u32> {
    table.level_entities(dimension)
}

"""


def main():
    edits = {}
    for name, count in (("table.rs", 9), ("table/section_tests.rs", 5)):
        path = ROOT / name
        source = path.read_text()
        changed, found = CALL.subn(r"collect_level_entities(&mut \1, \2)", source)
        if found != count:
            raise SystemExit(f"{path}: expected {count} calls, found {found}")
        if name == "table.rs":
            if changed.count(MARKER) != 1:
                raise SystemExit(f"{path}: expected one test boundary")
            changed = changed.replace(MARKER, COLLECTOR + MARKER)
        edits[path] = changed
    for path, changed in edits.items():
        path.write_text(changed)


if __name__ == "__main__":
    main()
