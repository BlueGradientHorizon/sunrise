#!/usr/bin/env python3
"""One-shot removal of the retired crosshair, outline and hurt-flash strings."""

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
KEY = re.compile(
    r"sunrise\.pvp\.(?:crosshair(?:_(?:shape|size|gap|thickness|colour|outline|dot|hover))?"
    r"|outline(?:_(?:colour|opacity|thickness|fill))?|hit(?:_(?:colour|opacity))?"
    r"|header\.(?:crosshair|outline|hit)|shape\.(?:cross|t|x|circle|square|dot)"
    r"|colour\.inverted)(?:\.tooltip)?"
)
RECORD = re.compile(rb'^\s*"([^"]+)":')


def main():
    folders = ("crates/sunrise-client/assets/sunrise/lang", "tools/lang_review/game")
    for folder in folders:
        for path in sorted((ROOT / folder).glob("*.json")):
            original = path.read_bytes()
            records = json.loads(original, object_pairs_hook=list)
            expected = [(key, value) for key, value in records if not KEY.fullmatch(key)]
            if len(expected) == len(records):
                continue
            kept = []
            for line in original.splitlines(keepends=True):
                entry = RECORD.match(line)
                if entry is None or not KEY.fullmatch(entry[1].decode()):
                    kept.append(line)
            updated = b"".join(kept)
            if json.loads(updated, object_pairs_hook=list) != expected:
                raise ValueError(f"unexpected language record layout: {path}")
            path.write_bytes(updated)


if __name__ == "__main__":
    main()
