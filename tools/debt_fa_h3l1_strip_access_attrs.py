#!/usr/bin/env python3
"""Strip #[access(...)] attributes from host area trait declarations (debt fa-h3l1, [43])."""

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
HOST_DIR = ROOT / "crates/sunrise-mod-vocab/src/host"
ACCESS_LINE = re.compile(
    r"^[ \t]*#\[access\((?:WorldOnly|Metadata|ClientSetup|PublishedResources)\)\]\n",
    re.MULTILINE,
)
EXPECTED_FILES = 97
EXPECTED_COUNT = 1189


def main() -> None:
    stripped = 0
    touched_files = 0
    for path in sorted(HOST_DIR.glob("*.rs")):
        original = path.read_text()
        updated, count = ACCESS_LINE.subn("", original)
        if count:
            path.write_text(updated)
            stripped += count
            touched_files += 1
        if "#[access(" in updated:
            raise SystemExit(f"{path}: unexpected remaining #[access( attribute")
    if touched_files != EXPECTED_FILES or stripped != EXPECTED_COUNT:
        raise SystemExit(
            f"expected {EXPECTED_COUNT} attributes across {EXPECTED_FILES} files, "
            f"stripped {stripped} across {touched_files}"
        )


if __name__ == "__main__":
    main()
