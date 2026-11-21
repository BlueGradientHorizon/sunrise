#!/usr/bin/env python3
"""Compare WritableBookContent CODEC/equality and BookEditScreen Done with vanilla 26.3.

VanillaWritableBookOracle runs the real 26.3 WritableBookContent.CODEC,
ItemStack.OPTIONAL_STREAM_CODEC, ItemStack.isSameItemSameComponents, and
BookEditScreen's Done button (saveChanges) on the pinned client jar. Its
recorded output lives in tools/witness/26.3/writable_book.json and is replayed
by sunrise-items and sunrise-client tests.
"""
import argparse
import os
from pathlib import Path

try:
    from . import lab, oracle_harness, test_rust_parity as base, test_model_baking_parity as baking
except ImportError:
    import lab
    import oracle_harness
    import test_rust_parity as base
    import test_model_baking_parity as baking

WITNESS = lab.ROOT / "tools" / "witness" / "26.3" / "writable_book.json"
ITEMS_TEST = "item_data::ingredient_components::tests::writable_book_pages_keep_their_filtered_text"
CLIENT_TEST = "game::native_client_screens::client_screen_tests::done_on_an_unedited_book_sends_it_and_drops_its_filtered_text"


def run_oracle(build, output_path):
    lock, jars, _ = base.verify_artifacts()
    classes, dependencies, _ = oracle_harness.compile_oracle(
        build, "VanillaWritableBookOracle", jars, baking.client_libraries(), lock["minecraft"]["java_major"])
    oracle_harness.java_run(
        build, classes, dependencies, "VanillaWritableBookOracle", [output_path], "vanilla")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--record", action="store_true",
                        help="Run only the vanilla 26.3 JVM oracle and overwrite tools/witness/26.3/writable_book.json.")
    args = parser.parse_args()
    build = Path(os.environ.get("SUNRISE_WRITABLE_BOOK_PARITY_DIR", lab.LOCAL / "writable-book-parity"))
    build.mkdir(parents=True, exist_ok=True)
    if args.record:
        run_oracle(build, WITNESS)
        print(f"Recorded vanilla 26.3 writable book witness: {WITNESS}")
        return
    expected = build / "vanilla.json"
    run_oracle(build, expected)
    if expected.read_text(encoding="utf-8") != WITNESS.read_text(encoding="utf-8"):
        raise AssertionError(f"vanilla 26.3 oracle output differs from {WITNESS}; rerun with --record")
    oracle_harness.run_logged(
        ["cargo", "test", "--locked", "-p", "sunrise-items", "--lib", ITEMS_TEST, "--", "--exact"],
        build, "rust-items", env=lab.game_env(),
    )
    if f"test {ITEMS_TEST} ... ok" not in (build / "rust-items.log").read_text(encoding="utf-8"):
        raise AssertionError(f"Rust witness replay did not run: {ITEMS_TEST}")
    oracle_harness.run_logged(
        ["cargo", "test", "--locked", "-p", "sunrise-client", "--lib", CLIENT_TEST, "--", "--exact", "--ignored"],
        build, "rust-client", env=lab.game_env(SUNRISE_TEST_RESOURCES=lab.RESOURCES.resolve()),
    )
    if f"test {CLIENT_TEST} ... ok" not in (build / "rust-client.log").read_text(encoding="utf-8"):
        raise AssertionError(f"Rust witness replay did not run: {CLIENT_TEST}")
    print("Vanilla 26.3 writable book codec and BookEditScreen parity passed")


if __name__ == "__main__":
    main()
