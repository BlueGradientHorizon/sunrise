#!/usr/bin/env python3
"""Rename the spawn placement registrar method and its porting references."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OLD = "declare_spawn_placement"
NEW = "register_spawn_placement"
EXPECTED = {
    "crates/sunrise-mod-api/src/parts/spawn_placements.rs": 7,
    "crates/sunrise-mod-vocab/src/parts/spawn_placements.rs": 1,
    "crates/sunrise-mod-vocab/src/world/types.rs": 4,
    "crates/sunrise-port-pack/src/events.rs": 1,
    "crates/sunrise-port-pack/tests/it/events.rs": 2,
    "docs/design/mod-api/direction.md": 1,
    "docs/modding/src/how-to/world/spawning.md": 2,
}


def main():
    changes = {}
    for name, count in EXPECTED.items():
        path = ROOT / name
        source = path.read_text(encoding="utf-8")
        if source.count(OLD) != count:
            raise SystemExit(f"{name}: expected {count} occurrences of {OLD}")
        changes[path] = source.replace(OLD, NEW)
    for path, source in changes.items():
        path.write_text(source, encoding="utf-8")


if __name__ == "__main__":
    main()
