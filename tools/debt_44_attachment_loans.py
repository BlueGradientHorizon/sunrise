#!/usr/bin/env python3
"""Migrate read projections and edit contexts for debt item 44.

After changing the read signatures, collect `cargo check --all-targets
--message-format=json` output, then pass it to this script. Repeat as the
compiler reaches downstream targets. Use rustc's missing-argument spans so a
similarly named method on an unrelated type is never rewritten.
"""

import json
import re
import sys
from collections import defaultdict
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VALUE_METHODS = {"get", "get_in", "attachment", "state", "state_in"}
OPTION_METHODS = {
    "get_existing",
    "get_existing_in",
    "existing_attachment",
    "state_existing",
    "state_existing_in",
    "read_state",
}
QUERY_DEFINITIONS = {
    ROOT / "crates/sunrise-mod-runtime/src/attachments/query.rs",
    ROOT / "crates/sunrise-mod-vocab/src/entity/attachment_query.rs",
}
DEFINITIONS = QUERY_DEFINITIONS | {
    ROOT / path
    for path in (
        "crates/sunrise-mod-runtime/src/attachments.rs",
        "crates/sunrise-mod-vocab/src/parts/attachment_types.rs",
        "crates/sunrise-mod-vocab/src/entity/original_player.rs",
        "crates/sunrise-mod-vocab/src/block/part_state.rs",
        "crates/sunrise-mod-vocab/src/entity/part_state.rs",
    )
}


def collect(messages):
    edits = defaultdict(dict)
    for item in messages:
        if item.get("reason") != "compiler-message":
            continue
        diagnostic = item["message"]
        code = (diagnostic.get("code") or {}).get("code")
        if code == "E0593":
            for span in diagnostic["spans"]:
                if span["label"] != "takes 2 arguments":
                    continue
                path = Path(span["file_name"]).resolve()
                if not path.is_relative_to(ROOT / "crates"):
                    continue
                start, end = span["byte_start"], span["byte_end"]
                source = path.read_bytes()
                call = next((s for s in diagnostic["spans"] if s["is_primary"]), None)
                if call is None or not re.search(
                    rb"\b(?:edit_state|edit_part_state)\(",
                    source[call["byte_start"] : start],
                ):
                    continue
                arguments = source[start:end]
                if arguments.startswith(b"|") and b"," in arguments:
                    comma = start + arguments.index(b",") + 1
                    edits[path][comma, comma] = b" _tx,"
            continue
        if code != "E0061":
            continue
        definition = next(
            (
                span
                for child in diagnostic["children"]
                if child["message"] in ("method defined here", "function defined here")
                for span in child["spans"]
                if span["is_primary"]
                and Path(span["file_name"]).resolve() in DEFINITIONS
            ),
            None,
        )
        if definition is None:
            continue
        match = re.search(r"\bfn (\w+)", definition["text"][0]["text"])
        if not match:
            continue
        name = match[1]
        if name in OPTION_METHODS or (
            name == "get" and Path(definition["file_name"]).resolve() in QUERY_DEFINITIONS
        ):
            projection = "|value| value.cloned()"
        elif name in VALUE_METHODS:
            projection = "Clone::clone"
        else:
            continue
        for child in diagnostic["children"]:
            for span in child["spans"]:
                fix = span["suggested_replacement"]
                if fix is None or "/* read */" not in fix:
                    continue
                path = Path(span["file_name"]).resolve()
                if path.is_relative_to(ROOT / "crates"):
                    edits[path][span["byte_start"], span["byte_end"]] = fix.replace(
                        "/* read */", projection
                    ).encode()
    return edits


def main():
    with open(sys.argv[1]) as stream:
        edits = collect(map(json.loads, stream))
    for path, changes in edits.items():
        contents = path.read_bytes()
        remaining = len(contents)
        for (start, end), replacement in sorted(changes.items(), reverse=True):
            if end > remaining:
                raise ValueError(f"overlapping suggestions in {path}")
            contents = contents[:start] + replacement + contents[end:]
            remaining = start
        path.write_bytes(contents)
    print(f"Migrated {sum(map(len, edits.values()))} calls across {len(edits)} files.")


if __name__ == "__main__":
    main()
