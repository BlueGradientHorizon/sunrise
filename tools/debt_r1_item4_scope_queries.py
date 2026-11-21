#!/usr/bin/env python3
"""Migrate the attachment-query tests and example to scoped value loans."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CLIENT = "crates/sunrise-client/src/game"
PATHS = (
    *(f"{CLIENT}/modding/attachment_index/{name}.rs" for name in (
        "boundary_tests", "mob_tests", "query_tests", "saved_entity_tests",
    )),
    f"{CLIENT}/modding/attachment_sync/transport_tests.rs",
    f"{CLIENT}/modding/entity_initialization/tests.rs",
    f"{CLIENT}/network/remote/attachment_query_tests.rs",
    f"{CLIENT}/network/remote/placement_prediction_tests.rs",
    "crates/sunrise-mod-vocab/src/attachment_queries/tests.rs",
    "crates/sunrise-mod-api/examples/attachment_query_bench.rs",
)


def main():
    for relative in PATHS:
        path = ROOT / relative
        source = path.read_text()
        migrated = re.sub(
            r"\*(row(?:\.unwrap\(\))?)\.value(?:\(\))?(?![\w(])",
            r"\1.with_value(|value| *value).unwrap()",
            source,
        )
        migrated = re.sub(
            r"\bcontribution\(row\.holder, row\.value\)",
            r"row.with_value(|value| contribution(row.holder, value)).unwrap()",
            migrated,
        )
        migrated = migrated.replace(
            "black_box(row.unwrap().value);", "row.unwrap().with_value(|value| { black_box(value); }).unwrap();"
        )
        migrated = re.sub(
            r"(?:mod_api::)?AttachmentWriteToken::from\(row\.write_token\(\)\)",
            "row.write_token()",
            migrated,
        )
        if migrated != source:
            path.write_text(migrated)


if __name__ == "__main__":
    main()
