#!/usr/bin/env python3
"""Update attachment value loans to pass a borrow of the capturing host's read scope."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
GAME = "crates/sunrise-client/src/game/"
INDEX = GAME + "modding/attachment_index/"
LOAN = ".with_value(|"

# Most fixtures name the game directly; a few borrow it as a Host or enter a
# transaction closure. The per-file counts make this a one-shot migration.
for path, argument, count in [
    (INDEX + "boundary_tests.rs", "game.attachment_read_scope()", 1),
    (INDEX + "mob_tests.rs", "game.attachment_read_scope()", 6),
    (INDEX + "query_tests.rs", "game.attachment_read_scope()", 8),
    (INDEX + "saved_entity_tests.rs", "game.attachment_read_scope()", 2),
    (GAME + "modding/attachment_sync/transport_tests.rs", "game.attachment_read_scope()", 1),
    (GAME + "modding/entity_initialization/tests.rs", "game.attachment_read_scope()", 4),
    (GAME + "network/remote/attachment_query_tests.rs", "game.attachment_read_scope()", 2),
    (GAME + "network/remote/placement_prediction_tests.rs", "host.attachment_read_scope()", 1),
    ("crates/sunrise-mod-vocab/src/attachment_queries/tests.rs", "None", 3),
    ("crates/sunrise-mod-vocab/src/entity/attachment_query.rs", "None", 2),
    ("crates/sunrise-mod-api/examples/attachment_query_bench.rs", "scope", 5),
]:
    file = ROOT / path
    source = file.read_text()
    assert source.count(LOAN) == count, path
    source = source.replace(LOAN, f".with_value({argument}, |")
    if path == INDEX + "mob_tests.rs":
        old = "row.with_value(game.attachment_read_scope(), |value| *value), Ok(13)"
        assert source.count(old) == 1
        source = source.replace(old, old.replace("game.", "host."))
    if path.endswith("attachment_query_bench.rs"):
        for start in [
            "fn api(host: &dyn Host) -> u64 {",
            "fn warm(host: &BenchHost) -> f64 {",
            "let host = BenchHost::new(true, true);",
        ]:
            spaces = " " * (12 if start.startswith("let") else 4)
            source = source.replace(start, f"{start}\n{spaces}let scope = host.attachment_read_scope();")
    file.write_text(source)
