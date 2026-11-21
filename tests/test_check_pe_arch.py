import contextlib
import io
from pathlib import Path
import struct
import tempfile
import unittest

from tools import check_pe_arch


def pe(machine):
    image = bytearray(0x98)
    image[:2] = b"MZ"
    struct.pack_into("<I", image, 0x3C, 0x80)
    image[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<H", image, 0x84, machine)
    return image


class CheckPEArchTests(unittest.TestCase):
    def test_reads_x64_and_arm64_machine_values(self):
        self.assertEqual(check_pe_arch.pe_fields(pe(0x8664)), (0x8664, None))
        self.assertEqual(check_pe_arch.pe_fields(pe(0xAA64)), (0xAA64, None))

    def test_rejects_non_pe_and_truncated_headers(self):
        with self.assertRaisesRegex(ValueError, "MZ"):
            check_pe_arch.pe_fields(b"not PE")
        image = pe(0xAA64)
        struct.pack_into("<I", image, 0x3C, 0xFFFFFFF0)
        with self.assertRaisesRegex(ValueError, "PE signature"):
            check_pe_arch.pe_fields(image)

    def test_stack_check_requires_the_exact_reserve_not_the_commit(self):
        image = pe(0x8664)
        # x64 PE32+ optional header begins at 0x98. Give reserve and commit different values.
        image.extend(bytearray(0xF0))
        struct.pack_into("<H", image, 0x94, 0xF0)
        struct.pack_into("<H", image, 0x98, 0x20B)
        with tempfile.TemporaryDirectory() as folder:
            binary = Path(folder) / "game.exe"
            with contextlib.redirect_stdout(io.StringIO()):
                struct.pack_into("<QQ", image, 0xE0, 8388608, 4096)
                binary.write_bytes(image)
                self.assertTrue(check_pe_arch.check(binary, "x64", stack_reserve=8388608))
            with contextlib.redirect_stderr(io.StringIO()) as stderr:
                struct.pack_into("<QQ", image, 0xE0, 1048576, 4096)
                binary.write_bytes(image)
                self.assertFalse(check_pe_arch.check(binary, "x64", stack_reserve=8388608))
                self.assertIn("SizeOfStackReserve is 1048576, expected 8388608 bytes", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
