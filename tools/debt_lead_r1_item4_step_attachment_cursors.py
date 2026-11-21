#!/usr/bin/env python3
"""Migrate direct attachment iterator steps to Result<Option<Row>, Error>."""

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
GAME = "crates/sunrise-client/src/game/"
INDEX = GAME + "modding/attachment_index/"

# These files only use iter().next() on attachment queries. Leave unrelated
# loops and iterator adapters for the caller-specific migration.
for path, steps, empty, refused in [
    (INDEX + "mob_tests.rs", 6, 1, 4),
    (INDEX + "query_tests.rs", 7, 0, 2),
    (INDEX + "saved_entity_tests.rs", 1, 1, 0),
    (GAME + "modding/entity_initialization/tests.rs", 1, 2, 0),
    (GAME + "network/remote/attachment_query_tests.rs", 1, 0, 0),
    ("crates/sunrise-mod-api/examples/attachment_query_bench.rs", 1, 0, 0),
    ("crates/sunrise-mod-runtime/src/attachments/query.rs", 1, 1, 0),
    ("crates/sunrise-mod-vocab/src/attachment_queries/tests.rs", 0, 1, 0),
    ("crates/sunrise-mod-vocab/src/entity/attachment_query.rs", 1, 0, 0),
]:
    file = ROOT / path
    source = file.read_text()
    source, count = re.subn(
        r"(\.iter\(\)\s*\.next)(\(\)\s*\.unwrap\(\)\s*\.unwrap\(\))",
        lambda m: m[1].removesuffix("next") + "try_next" + m[2],
        source,
    )
    assert count == steps, (path, count)
    source, count = re.subn(
        r"\.iter\(\)\.next\(\)\.is_none\(\)",
        ".iter().try_next().unwrap().is_none()",
        source,
    )
    assert count == empty, (path, count)
    if path == INDEX + "query_tests.rs":
        source = source.replace("retained_iter.next()", "retained_iter.try_next()")
    source, count = re.subn(
        r"(\.iter\(\)\.next\(\)|retained_iter\.try_next\(\)),(\s*)"
        r"Some\(Err\(((?:mod_api::AttachmentRowError::)?OutsideTransaction)\)\)",
        lambda m: f"{m[1].replace('.iter().next()', '.iter().try_next()')},{m[2]}Err({m[3]})",
        source,
    )
    assert count == refused, (path, count)
    file.write_text(source)
