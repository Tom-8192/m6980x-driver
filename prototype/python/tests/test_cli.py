import unittest

from m6980x_configtool.cli import FACTORY_DPI, LED_NAMES, apply_factory_defaults, format_led, parse_led
from m6980x_configtool.protocol import M6980X
from test_protocol import FakeMouse


class LedCliTest(unittest.TestCase):
    def test_parse_names_and_levels(self):
        self.assertEqual(parse_led(["Rot"]), (2, 0, 0))
        self.assertEqual(parse_led(["0", "1", "2"]), (0, 1, 2))

    def test_parse_rejects_out_of_range(self):
        for bad in (["255", "0", "0"], ["3", "0", "0"], ["rot", "1"], ["regenbogen"], []):
            with self.assertRaises(ValueError):
                parse_led(bad)

    def test_all_names_use_valid_levels(self):
        for rgb in LED_NAMES.values():
            self.assertTrue(all(c in (0, 1, 2) for c in rgb))

    def test_format_flags_invalid_values(self):
        self.assertIn("ungueltig", format_led((255, 255, 0)))
        self.assertIn("gelb", format_led((2, 2, 0)))


class FactoryDefaultsTest(unittest.TestCase):
    def test_dpi_and_500_hz_like_original_default_button(self):
        mouse = FakeMouse()
        mouse.interval = 8
        stages = apply_factory_defaults(M6980X(mouse, poll_interval=0))
        self.assertEqual([s.x_dpi for s in stages], list(FACTORY_DPI))
        self.assertEqual(mouse.interval, 2)
        self.assertIn(bytes.fromhex("01 8B 02"), [s[:3] for s in mouse.sent])


if __name__ == "__main__":
    unittest.main()
