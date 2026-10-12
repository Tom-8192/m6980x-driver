import json
import os
import tempfile
import unittest

from m6980x_configtool import backup
from m6980x_configtool.functions import FUNCTIONS_BY_KEY
from m6980x_configtool.protocol import M6980X
from test_protocol import FakeMouse


class BackupTest(unittest.TestCase):
    def setUp(self):
        self.mouse = FakeMouse()
        self.g = M6980X(self.mouse, poll_interval=0)

    def test_round_trip_restores_everything(self):
        snap = backup.create(self.g)
        rom_before = bytes(self.mouse.rom)

        # change a bit of everything, like a factory reset or experiments would
        self.g.write_macro(3, bytes.fromhex("04 83 04 84 04 00"), "Test", icon=7)
        self.g.assign_function(1, 3, FUNCTIONS_BY_KEY["kopieren"])
        self.g.reset_button(0, 4)
        self.g.set_led_color(2, 0, 0, 0)
        self.g.set_dpi(0, 900)
        self.g.set_report_interval(8)
        self.g.set_active_dpi_stage(2)
        self.g.set_active_profile(2)

        left = backup.restore(self.g, snap, log=lambda *_: None)
        self.assertEqual(left, [])
        self.assertEqual(bytes(self.mouse.rom), rom_before)
        self.assertEqual(backup.create(self.g) | {"created": snap["created"]}, snap)

    def test_meb_round_trip_and_import(self):
        rom = bytes((i * 7) & 0xFF for i in range(8192))
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "x.meb")
            backup.write_meb(rom, path)
            with open(path, "rb") as f:
                head = f.read(30)
            self.assertTrue(head.startswith(b"[Mem]\r\ncount=8192\r\n0=0\r\n1=7\r\n"))
            self.assertEqual(backup.read_meb(path), rom)
            with open(path, "w") as f:
                f.write("[Mem]\ncount=8192\n0=1\n")
            with self.assertRaises(ValueError):
                backup.read_meb(path)
        wanted = bytearray(self.mouse.rom)
        wanted[0x0100:0x0104] = b"\x04\x83\x05\x00"
        wanted[backup.TABLE_START] = 0x12
        left = backup.import_rom(self.g, bytes(wanted), log=lambda *_: None)
        self.assertEqual(left, [])
        self.assertEqual(bytes(self.mouse.rom), bytes(wanted))

    def test_meb_import_applies_button_assignments(self):
        # like the original's restore: flags 0x1F01 + p*12 + b decide, the table entry gives the address
        wanted = bytearray(self.mouse.rom)
        wanted[0x1F01:0x1F25] = bytes(36)                                  # everything default ...
        wanted[0x1F01 + 2 * 12 + 3] = 0x80                                 # ... except profile 3, rear button
        a = 0x1F3D + 2 * 0x18 + 3 * 2
        wanted[a:a + 2] = (0x1D98).to_bytes(2, "little")                   # -> Mittelklick
        stale = 0x1F3D + 0 * 0x18 + 4 * 2
        wanted[stale:stale + 2] = (0x0000).to_bytes(2, "little")           # stale entry, flag 00 -> stays default
        self.assertEqual(self.g.button(0, 4), 0x1D98)                      # FakeMouse starts with this assigned
        backup.import_rom(self.g, bytes(wanted), log=lambda *_: None)
        self.assertEqual(self.g.button(2, 3), 0x1D98)
        self.assertIsNone(self.g.button(0, 4))

    def test_save_load_and_reject_foreign_files(self):
        snap = backup.create(self.g)
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "b.json")
            backup.save(snap, path)
            self.assertEqual(backup.load(path), snap)
            with open(path, "w") as f:
                json.dump({"format": "etwas anderes"}, f)
            with self.assertRaises(ValueError):
                backup.load(path)


class MacroFileTest(unittest.TestCase):
    """Macro export file of the original's editor (export 0x4148a0, import 0x418408)."""

    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.path = os.path.join(self.dir, "XY.ini")

    def test_writes_original_layout(self):
        backup.write_macro_ini(bytes.fromhex("04 83 04 88 64 84 04 00"), self.path)
        with open(self.path, "rb") as f:
            self.assertEqual(f.read(), b"[Macro]\r\ncount=8\r\n0=4\r\n1=131\r\n2=4\r\n3=136\r\n4=100\r\n"
                                       b"5=132\r\n6=4\r\n7=0\r\n")

    def test_round_trip_keeps_zero_inside_events(self):
        record = bytes.fromhex("05 85 e9 00 85 00 00 89 00 01 21 41 00")     # 00 inside consumer/wait arguments
        backup.write_macro_ini(record, self.path)
        self.assertEqual(backup.read_macro_ini(self.path), record)

    def test_reads_like_original(self):
        with open(self.path, "w") as f:      # case-insensitive keys, trailing bytes after 00 ignored, no end marker added twice
            f.write("[MACRO]\nCount=5\n0=6\n1=33\n2=65\n3=0\n4=255\n")
        self.assertEqual(backup.read_macro_ini(self.path), bytes.fromhex("06 21 41 00"))
        with open(self.path, "w") as f:
            f.write("[Macro]\ncount=3\n0=4\n1=97\n2=127\n[SpecialMacro]\nindex=2\n")
        self.assertEqual(backup.read_macro_ini(self.path), bytes.fromhex("04 00"))   # original keeps only the mode

    def test_rejects_empty_and_foreign_files(self):
        with self.assertRaises(ValueError):
            backup.write_macro_ini(bytes.fromhex("04 00"), self.path)
        for content in ("[Mem]\ncount=8192\n", "[Macro]\ncount=3\n0=9\n1=0\n2=0\n", "[Macro]\ncount=999\n"):
            with open(self.path, "w") as f:
                f.write(content)
            with self.assertRaises(ValueError):
                backup.read_macro_ini(self.path)


if __name__ == "__main__":
    unittest.main()
