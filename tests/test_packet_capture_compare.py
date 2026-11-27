"""Pure-Python unit tests for tools/packet_capture_compare.py."""
from __future__ import annotations

import io
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tools import packet_capture_compare as cmp_tool  # noqa: E402


def write_capture(directory: Path, filename: str, conn: int, player: str, rows: list[tuple[int, int, int, int, str]]) -> Path:
    path = directory / filename
    lines = [f"# v1\t{conn}\t{player}"]
    for seq, tick, pkt_id, length, digest in rows:
        lines.append(f"{seq}\t{tick}\t{pkt_id}\t{length}\t{digest}")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


class PacketCaptureCompareTests(unittest.TestCase):
    def test_equal_recordings_match_by_player_name(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            a, b = Path(tmp) / "a", Path(tmp) / "b"
            a.mkdir()
            b.mkdir()
            alice_rows = [
                (0, 1, 50, 83, "0" * 32),
                (1, 2, 46, 4096, "a" * 32),
            ]
            bob_rows = [
                (0, 1, 50, 83, "1" * 32),
            ]
            # Different connection ids in filenames; matching uses the player header.
            write_capture(a, "Alice-1.pkt", 1, "Alice", alice_rows)
            write_capture(a, "Bob-2.pkt", 2, "Bob", bob_rows)
            write_capture(b, "Alice-9.pkt", 9, "Alice", alice_rows)
            write_capture(b, "Bob-4.pkt", 4, "Bob", bob_rows)

            out = io.StringIO()
            with redirect_stdout(out):
                rc = cmp_tool.main([str(a), str(b)])
            self.assertEqual(rc, 0)
            text = out.getvalue()
            self.assertIn("Alice: identical (2 packets)", text)
            self.assertIn("Bob: identical (1 packets)", text)
            self.assertTrue(text.strip().endswith("IDENTICAL"))

    def test_one_byte_different_reports_first_mismatch_details(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            a, b = Path(tmp) / "a", Path(tmp) / "b"
            a.mkdir()
            b.mkdir()
            hash_a = "11" * 16
            hash_b = "22" * 16
            write_capture(
                a,
                "Alice-1.pkt",
                1,
                "Alice",
                [
                    (0, 10, 50, 80, "aa" * 16),
                    (1, 11, 8, 12, hash_a),
                    (2, 12, 46, 512, "cc" * 16),
                ],
            )
            write_capture(
                b,
                "Alice-1.pkt",
                1,
                "Alice",
                [
                    (0, 10, 50, 80, "aa" * 16),
                    (1, 11, 8, 12, hash_b),
                    (2, 12, 46, 999, "dd" * 16),
                ],
            )

            out = io.StringIO()
            with redirect_stdout(out):
                rc = cmp_tool.main([str(a), str(b)])
            self.assertNotEqual(rc, 0)
            text = out.getvalue()
            self.assertIn("Alice: DIFFERS", text)
            self.assertIn("seq=1", text)
            self.assertIn("tick=11", text)
            self.assertIn("id=8 (block_update)", text)
            self.assertIn("len=12 vs 12", text)
            self.assertIn(f"hash={hash_a} vs {hash_b}", text)
            # Only the first difference per connection is printed.
            self.assertNotIn("seq=2", text)
            self.assertTrue(text.strip().endswith("DIFFERENT"))

    def test_reordered_packets_fail_with_both_ids(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            a, b = Path(tmp) / "a", Path(tmp) / "b"
            a.mkdir()
            b.mkdir()
            write_capture(
                a,
                "Alice-1.pkt",
                1,
                "Alice",
                [
                    (0, 5, 8, 10, "aa" * 16),
                    (1, 5, 115, 24, "bb" * 16),
                ],
            )
            write_capture(
                b,
                "Alice-1.pkt",
                1,
                "Alice",
                [
                    (0, 5, 115, 24, "bb" * 16),
                    (1, 5, 8, 10, "aa" * 16),
                ],
            )

            out = io.StringIO()
            with redirect_stdout(out):
                rc = cmp_tool.main([str(a), str(b)])
            self.assertNotEqual(rc, 0)
            text = out.getvalue()
            self.assertIn("seq=0", text)
            self.assertIn("tick=5", text)
            self.assertIn("8 (block_update) vs 115 (set_time)", text)
            self.assertIn("len=10 vs 24", text)
            self.assertIn(f"hash={'aa' * 16} vs {'bb' * 16}", text)

    def test_missing_connection_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            a, b = Path(tmp) / "a", Path(tmp) / "b"
            a.mkdir()
            b.mkdir()
            write_capture(a, "Alice-1.pkt", 1, "Alice", [(0, 1, 50, 10, "aa" * 16)])
            write_capture(a, "Bob-2.pkt", 2, "Bob", [(0, 1, 50, 10, "bb" * 16)])
            write_capture(b, "Alice-1.pkt", 1, "Alice", [(0, 1, 50, 10, "aa" * 16)])
            write_capture(b, "Carol-3.pkt", 3, "Carol", [(0, 1, 50, 10, "cc" * 16)])

            out = io.StringIO()
            with redirect_stdout(out):
                rc = cmp_tool.main([str(a), str(b)])
            self.assertNotEqual(rc, 0)
            text = out.getvalue()
            self.assertIn("Alice: identical (1 packets)", text)
            self.assertIn("Bob: DIFFERS only in A", text)
            self.assertIn("Carol: DIFFERS only in B", text)

    def test_unequal_counts_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            a, b = Path(tmp) / "a", Path(tmp) / "b"
            a.mkdir()
            b.mkdir()
            write_capture(a, "Alice-1.pkt", 1, "Alice", [(0, 1, 50, 10, "aa" * 16)])
            write_capture(
                b,
                "Alice-1.pkt",
                1,
                "Alice",
                [
                    (0, 1, 50, 10, "aa" * 16),
                    (1, 2, 8, 14, "bb" * 16),
                ],
            )

            out = io.StringIO()
            with redirect_stdout(out):
                rc = cmp_tool.main([str(a), str(b)])
            self.assertNotEqual(rc, 0)
            text = out.getvalue()
            self.assertIn("count=1 vs 2", text)
            self.assertIn("seq=1", text)
            self.assertIn("tick=2", text)
            self.assertIn("8 (block_update)", text)

    def test_ignored_id_off_by_default_and_skips_when_requested(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            a, b = Path(tmp) / "a", Path(tmp) / "b"
            a.mkdir()
            b.mkdir()
            # Keep-alive (45) appears at a different position with a different wall-clock body.
            write_capture(
                a,
                "Alice-1.pkt",
                1,
                "Alice",
                [
                    (0, 1, 50, 80, "aa" * 16),
                    (1, 1, 45, 9, "11" * 16),
                    (2, 2, 8, 12, "bb" * 16),
                ],
            )
            write_capture(
                b,
                "Alice-1.pkt",
                1,
                "Alice",
                [
                    (0, 1, 50, 80, "aa" * 16),
                    (1, 2, 8, 12, "bb" * 16),
                ],
            )

            # Default: keep_alive is NOT ignored.
            out_default = io.StringIO()
            with redirect_stdout(out_default):
                rc_default = cmp_tool.main([str(a), str(b)])
            self.assertNotEqual(rc_default, 0)
            self.assertIn("45 (keep_alive)", out_default.getvalue())

            # With --ignore keep_alive: shifted sequence numbers still compare equal.
            out_named = io.StringIO()
            with redirect_stdout(out_named):
                rc_named = cmp_tool.main([str(a), str(b), "--ignore", "keep_alive"])
            self.assertEqual(rc_named, 0)
            self.assertIn("Alice: identical (2 packets) (1/0 ignored)", out_named.getvalue())

            # Numeric packet id works too.
            out_num = io.StringIO()
            with redirect_stdout(out_num):
                rc_num = cmp_tool.main([str(a), str(b), "--ignore", "45"])
            self.assertEqual(rc_num, 0)

    def test_tick_difference_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            a, b = Path(tmp) / "a", Path(tmp) / "b"
            a.mkdir()
            b.mkdir()
            write_capture(a, "Alice-1.pkt", 1, "Alice", [(0, 4, 8, 12, "aa" * 16)])
            write_capture(b, "Alice-1.pkt", 1, "Alice", [(0, 5, 8, 12, "aa" * 16)])

            out = io.StringIO()
            with redirect_stdout(out):
                rc = cmp_tool.main([str(a), str(b)])
            self.assertNotEqual(rc, 0)
            self.assertIn("tick=4 vs 5", out.getvalue())

    def test_unknown_ignore_name_errors(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            a, b = Path(tmp) / "a", Path(tmp) / "b"
            a.mkdir()
            b.mkdir()
            err = io.StringIO()
            with redirect_stderr(err):
                rc = cmp_tool.main([str(a), str(b), "--ignore", "not_a_real_packet"])
            self.assertEqual(rc, 2)
            self.assertIn("not_a_real_packet", err.getvalue())

    def test_help_lists_wall_clock_packets_and_default_off(self) -> None:
        help_text = cmp_tool.build_parser().format_help()
        self.assertIn("none ignored by default", help_text)
        for name, pid, _ in cmp_tool.WALL_CLOCK_PACKETS:
            self.assertIn(name, help_text)
            self.assertIn(str(pid), help_text)


if __name__ == "__main__":
    unittest.main()
