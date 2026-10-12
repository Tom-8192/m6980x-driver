import unittest

from m6980x_configtool import functions as fn


class FunctionTableTest(unittest.TestCase):
    def test_addresses_match_original_captures(self):
        seen = {"mittelklick": 0x1D98, "kopieren": 0x1DD8, "einfuegen": 0x1DE2, "lauter": 0x1E46, "strg": 0x1E64}
        for key, addr in seen.items():
            self.assertEqual(fn.FUNCTIONS_BY_KEY[key].address, addr, key)

    def test_records_fit_their_size(self):
        for f in fn.FUNCTIONS:
            if f.record:
                self.assertEqual(len(f.record), f.size, f.key)
                self.assertEqual(f.record[-1], fn.EV_END, f.key)

    def test_derived_patterns_reproduce_known_records(self):
        # the generators must rebuild records that exist on the real mouse
        self.assertEqual(fn._combo(fn.KEYS["win"], fn.KEYS["d"]), bytes.fromhex("04 83 e3 83 07 84 07 84 e3 00"))
        self.assertEqual(fn._combo(fn.KEYS["strg"], fn.KEYS["v"]), fn.FUNCTIONS_BY_KEY["einfuegen"].record)
        self.assertEqual(fn._consumer(0x00EA), bytes.fromhex("04 85 ea 00 85 00 00 00"))
        self.assertEqual(fn._hold_key(fn.KEYS["strg"]), fn.FUNCTIONS_BY_KEY["strg"].record)
        self.assertEqual(fn._hold_mouse(0x04), fn.FUNCTIONS_BY_KEY["mittelklick"].record)


class IconTest(unittest.TestCase):
    def test_picture_byte_and_source(self):
        from m6980x_configtool import rom
        klicker_info = bytes.fromhex("4b 6c 69 63 6b 65 72 00 2c 68")    # real entry, confirmed by Tom
        self.assertEqual(rom.macro_icon(klicker_info), 4)
        self.assertEqual(rom.icon_source(4), "eingebaut")
        self.assertEqual(rom.icon_source(41), "eigenes bmp\\1.bmp")
        self.assertIsNone(rom.macro_icon(bytes(9) + b"\x03"))             # new macro: slot + 1


class LayoutTest(unittest.TestCase):
    def test_german_layout_y_z(self):
        self.assertEqual(fn.KEYS["z"], 0x1C)      # German Z sits on the US-Y position
        self.assertEqual(fn.KEYS["y"], 0x1D)
        self.assertEqual(fn.FUNCTIONS_BY_KEY["rueckgaengig"].record, bytes.fromhex("04 83 e0 83 1c 84 1c 84 e0 00"))


class MacroTextTest(unittest.TestCase):
    def test_encode_xy_macro_from_capture(self):
        # Tom's macro "XY": A down, 100 ms, A up, 500 ms, left press&hold, middle press&hold
        ev = fn.encode_events("drueck a, warte 100, los a, warte 500, maus-drueck links, maus-drueck mitte")
        self.assertEqual(bytes([fn.MODE_ONCE]) + ev + b"\x00",
                         bytes.fromhex("04 83 04 88 64 84 04 89 f4 01 21 24 00"))

    def test_tippe_and_klick(self):
        self.assertEqual(fn.encode_events("tippe strg+c"), bytes.fromhex("83 e0 83 06 84 06 84 e0"))
        self.assertEqual(fn.encode_events("klick links"), bytes.fromhex("21 41"))

    def test_decode_round_trip(self):
        mode, events = fn.decode_record(bytes.fromhex("05 83 04 88 64 84 04 89 f4 01 21 41 00 ff"))
        self.assertEqual(mode, "schleife")
        self.assertEqual(events, ["drueck a", "warte 100", "los a", "warte 500", "maus-drueck links", "maus-los links"])

    def test_decode_klicker_macro_on_mouse(self):
        self.assertEqual(fn.decode_record(bytes.fromhex("06 21 41 00")), ("feuer", ["maus-drueck links", "maus-los links"]))

    def test_wheel_events_tested_on_device(self):
        self.assertEqual(fn.encode_events("rad hoch, rad runter"), bytes.fromhex("61 7f"))
        self.assertEqual(fn.decode_record(bytes.fromhex("04 61 7f 00"))[1], ["rad hoch 1", "rad runter 1"])
        self.assertEqual(fn.FUNCTIONS_BY_KEY["scroll-vor"].record, bytes.fromhex("04 61 00"))

    def test_several_notches_like_original_editor(self):
        # the original only writes single notches (0x410ae8); deltas > 1 stay readable
        self.assertEqual(fn.encode_events("rad runter 3, rad hoch 2"), bytes.fromhex("7f 7f 7f 61 61"))
        self.assertEqual(fn.decode_record(bytes.fromhex("04 7e 00"))[1], ["rad runter 2"])
        self.assertEqual(fn.decode_record(bytes.fromhex("04 7f 7f 7f 61 00"))[1], ["rad runter 3", "rad hoch 1"])

    def test_records_from_original_code(self):
        # 0x423630: functions 43-45, list ends at 0x1EDC like [ListMemSize] 0-45
        self.assertEqual(len(fn.FUNCTIONS), 46)
        self.assertEqual(fn.FUNCTIONS_BY_KEY["nur-senkrecht"].address, 0x1ECA)
        self.assertEqual(fn.FUNCTIONS_BY_KEY["gigabyte-ghost"].address, 0x1ECE)
        self.assertEqual(fn.FUNCTIONS_BY_KEY["gigabyte-ghost"].record,
                         bytes.fromhex("04 83 e0 83 e2 83 0e 84 0e 84 e2 84 e0 00"))   # as in the factory ROM

    def test_wheel_buttons_get_click_records(self):
        left = fn.FUNCTIONS_BY_KEY["linksklick"]
        self.assertEqual(fn.placement(left, 0), (left.address, left.record))
        self.assertEqual(fn.placement(left, 8), (0x1EDC, bytes.fromhex("04 21 41 00")))
        self.assertEqual(fn.placement(fn.FUNCTIONS_BY_KEY["browser-zurueck"], 9), (0x1EEC, bytes.fromhex("04 28 48 00")))
        copy = fn.FUNCTIONS_BY_KEY["kopieren"]
        self.assertEqual(fn.placement(copy, 9), (copy.address, copy.record))
        self.assertIs(fn.FUNCTIONS_BY_ADDRESS[0x1EE4], fn.FUNCTIONS_BY_KEY["mittelklick"])

    def test_record_length_skips_zero_arguments(self):
        self.assertEqual(fn.record_length(bytes.fromhex("04 85 e9 00 85 00 00 00 ff ff")), 8)   # Lauter
        self.assertEqual(fn.record_length(bytes.fromhex("04 89 00 01 83 04 00 ff")), 7)          # warte 256
        self.assertEqual(fn.record_length(bytes.fromhex("ff ff ff")), 0)
        self.assertEqual(fn.record_length(bytes.fromhex("00 00 00")), 0)

    def test_rejects_bad_input(self):
        for bad in ("drueck xyz", "warte 0", "warte 70000", "klick oben", "springe", "rad seitwaerts", "rad hoch 16"):
            with self.assertRaises(ValueError):
                fn.encode_events(bad)


if __name__ == "__main__":
    unittest.main()
