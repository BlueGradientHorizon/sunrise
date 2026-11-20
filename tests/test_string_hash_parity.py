import unittest

from tools.test_string_hash_parity import parse


class StringHashOutputTests(unittest.TestCase):
    def test_requires_complete_chunk_schedule_and_rust_summary(self):
        expected = {0: 4, 1: 4}
        first = "R 0 4 0000000000000000"
        second = "R 1 4 1111111111111111"
        summary = "S 8 0"
        cases = [
            (False, [first], "missing R chunk 1"),
            (True, [first, "S 4 0"], "missing R chunk 1"),
            (False, [first, second.replace("1 4", "1 3")], "chunk 1 .*count 3"),
            (True, [first, second], "missing Rust S summary"),
        ]
        for rust, output, error in cases:
            with self.subTest(rust=rust, output=output):
                with self.assertRaisesRegex(ValueError, error):
                    parse(output, "R", expected, rust=rust)

        java, _ = parse([first, second], "R", expected)
        rust, counts = parse([first, second, summary], "R", expected, rust=True)
        self.assertEqual(java, rust)
        self.assertEqual(counts, (8, 0))
