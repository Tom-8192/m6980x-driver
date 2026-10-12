import unittest
from unittest import mock

from m6980x_configtool import transport
from m6980x_configtool.protocol import M6980X, M6980XError, dpi_write_plan, model_of

# Replies recorded from the real mouse (status byte + 7 data bytes)
REPLIES = {
    (0x00, 0x00): "40 0c 12 09 2d 00 03 01",
    (0x00, 0x01): "40 47 33 00 00 00 00 00",
    (0x01, 0x03, 0x01): "41 ff ff ff ff ff ff ff",
}
FILL_OFFSETS = {0x03: 0, 0x05: 6, 0x06: 12, 0x07: 18, 0x08: 24, 0x09: 30}


class FakeMouse:
    """Answers like the device: busy once after each command, then done."""

    def __init__(self, busy_polls=1, accept_profile_switch=True):
        self.sent = []
        self.state = bytes.fromhex("41 00 00 00 00 00 00 00")
        self.busy_polls = busy_polls
        self.pending = 0
        self.profile = 0
        self.accept_profile_switch = accept_profile_switch
        self.leds = [(2, 2, 0), (0, 2, 2), (2, 1, 0)]
        self.accept_led = True
        # per DPI stage: [x_step, y_step] and [x_num, x_den, y_num, y_den] - values of the real mouse
        self.dpi_steps = [[18, 18], [42, 42], [56, 56], [56, 56]]
        self.dpi_ratio = [[1, 1, 1, 1], [1, 1, 1, 1], [65, 56, 65, 56], [67, 56, 67, 56]]
        self.dpi_stage = 1
        self.rom = bytearray(i & 0xFF for i in range(0x2000))
        self.buffer = bytearray(32)
        self.buttons = {(0, 4): (0x80, 0x1D98)}          # (profile, device button) -> (flags, address)
        self.interval = 2

    def set_feature(self, data):
        data = bytes(data)
        assert len(data) == 8
        self.sent.append(data)
        key = next((k for k in sorted(REPLIES, key=len, reverse=True)
                    if data[:len(k)] == bytes(k)), None)
        if data[:2] == b"\x01\x88":
            if self.accept_profile_switch:
                self.profile = data[2]
        elif data[:2] == b"\x01\x86":
            if self.accept_led:
                self.leds[data[2]] = tuple(data[3:6])
        elif data[:2] == b"\x01\x06":
            self.state = bytes([0x41, *self.leds[data[2]]]) + self.state[4:]
        elif data[:2] == b"\x01\x87":
            self.dpi_stage = data[2]
        elif data[:2] == b"\x01\x07":
            self.state = bytes([0x41, self.dpi_stage]) + self.state[2:]
        elif data[:2] == b"\x01\x83":
            self.dpi_steps[data[3]][data[2]] = data[4]
        elif data[:2] == b"\x01\x84":
            self.dpi_ratio[data[3]][2 * data[2]:2 * data[2] + 2] = [data[4], data[5]]
        elif data[:3] == b"\x01\x03\x00":
            self.state = bytes([0x41, *self.dpi_steps[data[3]], 0x5A, 0x01, 0xFF, 0xFF, 0xFF])
        elif data[:3] == b"\x01\x04\x00":
            self.state = bytes([0x41, *self.dpi_ratio[data[3]], 0xFF, 0xFF, 0xFF])
        elif data[:2] == b"\x01\x08":                                 # stale byte 2 like the real mouse
            self.state = bytes([0x41, self.profile, 0x1F]) + self.state[3:]
        elif data[:2] == b"\x01\x02":
            self.saved = True
        elif data[:2] == b"\x01\x85":
            self.buttons[(data[2], data[3])] = (data[4], data[5] | data[6] << 8)
        elif data[:2] == b"\x01\x05":
            flags, addr = self.buttons.get((data[2], data[3]), (0x00, 0xFFFF))
            self.state = bytes([0x41, flags, addr & 0xFF, addr >> 8, 0x00, 0x80, 0x80, 0x00])
        elif data[:2] == b"\x01\x8b":
            self.interval = data[2]
        elif data[:2] == b"\x01\x0b":
            self.state = bytes([0x41, self.interval]) + self.state[2:]
        elif data[0] == 0x02 and data[1] in FILL_OFFSETS:
            self.buffer[FILL_OFFSETS[data[1]]:FILL_OFFSETS[data[1]] + 6] = data[2:8]
        elif data[:2] == b"\x02\x04":
            addr, n = data[2] | data[3] << 8, data[5]
            self.rom[addr:addr + n] = self.buffer[:n]
        elif data[:2] == b"\x02\x02":
            a = data[2] | data[3] << 8
            self.rom[a:a + 2] = data[5:7]
        elif data[:2] == b"\x02\x01":
            self.rom[data[2] | data[3] << 8] = data[5]
        elif key is None and data[0] == 0x02 and data[1] == 0x00:    # ROM read
            addr = data[2] | data[3] << 8
            self.state = bytes([0x42]) + bytes(self.rom[addr:addr + 7])
        elif key is None:
            raise AssertionError(f"unexpected command {data.hex(' ')}")
        else:
            self.state = bytes.fromhex(REPLIES[key])
        self.pending = self.busy_polls

    def get_feature(self):
        if self.pending:
            self.pending -= 1
            return bytes([0x80]) + self.state[1:]
        return self.state

    def close(self):
        pass


class ProtocolTest(unittest.TestCase):
    def setUp(self):
        self.mouse = FakeMouse()
        self.g = M6980X(self.mouse, poll_interval=0)

    def test_identify_and_name(self):
        self.assertTrue(self.g.identify()["signature_ok"])
        self.assertEqual(self.g.identify()["model"], "6980X")
        self.assertEqual(self.g.name(), "G3")

    def test_model_like_original(self):
        # 0x42ebd8: identify byte 6 = kind, byte 2 separates 6980X from M8000X
        self.assertEqual(model_of(bytes.fromhex("0c 12 09 2d 00 03 01")), "6980X")   # Tom's mouse
        self.assertEqual(model_of(bytes.fromhex("0c 12 07 2d 00 05 01")), "M8000X")
        self.assertEqual(model_of(bytes.fromhex("0c 12 09 2d 00 03 00")), "6980")
        self.assertEqual(model_of(bytes.fromhex("0c 12 09 2d 00 05 04")), "M8600")
        self.assertIsNone(model_of(bytes.fromhex("0c 12 09 2d 00 03 02")))
        self.assertIsNone(model_of(bytes.fromhex("00 00 00 00 00 00 01")))

    def test_check_model_accepts_only_6980x(self):
        self.assertEqual(self.g.check_model()["model"], "6980X")
        for reply in ("40 0c 12 07 2d 00 05 01", "40 00 00 00 00 00 00 00"):
            with mock.patch.dict(REPLIES, {(0x00, 0x00): reply}):
                with self.assertRaises(M6980XError):
                    M6980X(FakeMouse(), poll_interval=0).check_model()

    def test_dpi_stage_uses_unit_and_ratio(self):
        s = self.g.dpi_stage(2)
        self.assertEqual((s.x_step, s.unit, s.x_num, s.x_den), (0x38, 90, 0x41, 0x38))
        self.assertEqual(s.x_dpi, 5850)

    def test_supported_steps(self):
        self.assertEqual(self.g.dpi_supported_steps(), list(range(1, 57)))

    def test_led_and_profile(self):
        self.assertEqual(self.g.led_color(1), (0, 2, 2))
        self.assertEqual(self.g.active_profile(), 0)

    def test_button_uses_device_numbers_and_default(self):
        self.assertEqual(self.g.button(0, 4), 0x1D98)
        self.assertEqual(self.mouse.sent[-1][:4], bytes([0x01, 0x05, 0x00, 0x04]))
        self.assertIsNone(self.g.button(0, 0))

    def test_left_click_on_wheel_writes_click_record(self):
        from m6980x_configtool import functions as fn
        self.g.assign_function(2, 8, fn.FUNCTIONS_BY_KEY["linksklick"])
        self.assertEqual(self.g.button(2, 8), 0x1EDC)
        self.assertEqual(bytes(self.mouse.rom[0x1EDC:0x1EE0]), bytes.fromhex("04 21 41 00"))

    def test_report_interval(self):
        self.g.set_report_interval(3)
        self.assertEqual(self.mouse.sent[0], bytes.fromhex("01 8b 03 00 00 00 00 00"))
        self.assertEqual(self.g.report_interval(), 3)
        with self.assertRaises(ValueError):
            self.g.set_report_interval(9)

    def test_write_rom_block_matches_original_capture(self):
        self.g.write_rom(0x1E64, bytes.fromhex("06 83 e0 ff ff 00"))
        writes = [p.hex(" ") for p in self.mouse.sent if p[0] == 0x02 and p[1] != 0x00]
        self.assertEqual(writes, ["02 03 06 83 e0 ff ff 00", "02 04 64 1e 00 06 00 12"])
        self.assertEqual(bytes(self.mouse.rom[0x1E64:0x1E6A]), bytes.fromhex("06 83 e0 ff ff 00"))

    def test_write_rom_splits_at_32_byte_boundaries(self):
        data = bytes(range(40))
        self.g.write_rom(0x1D90, data)                  # 0x1D90-0x1D9F, then 0x1DA0-0x1DB7
        commits = [p for p in self.mouse.sent if p[:2] == b"\x02\x04"]
        self.assertEqual([(c[2] | c[3] << 8, c[5]) for c in commits], [(0x1D90, 16), (0x1DA0, 24)])
        self.assertEqual(bytes(self.mouse.rom[0x1D90:0x1D90 + 40]), data)

    def test_assign_function_writes_record_then_button(self):
        from m6980x_configtool.functions import FUNCTIONS_BY_KEY
        f = FUNCTIONS_BY_KEY["kopieren"]
        self.g.assign_function(0, 3, f)
        self.assertEqual(bytes(self.mouse.rom[0x1DD8:0x1DE2]), bytes.fromhex("04 83 e0 83 06 84 06 84 e0 00"))
        self.assertIn(bytes.fromhex("01 85 00 03 80 d8 1d 12"), self.mouse.sent)
        self.assertEqual(self.g.button(0, 3), 0x1DD8)

    def test_speed_function_uses_original_2_byte_write(self):
        from m6980x_configtool.functions import FUNCTIONS_BY_KEY
        self.g.assign_function(2, 3, FUNCTIONS_BY_KEY["speed-2x"])
        sent = [p.hex(" ") for p in self.mouse.sent if p[1] & 0x80 or p[0] == 0x02 and p[1]]
        # identical to the original software capture (profile 3, rear thumb button)
        self.assertEqual(sent[:2], ["02 02 c4 1e 00 c8 00 12", "01 85 02 03 80 c4 1e 12"])

    def test_save_settings_packet(self):
        self.g.save_settings()
        # same as the original software (capture 1: 01 02 00 .. 12)
        self.assertEqual(self.mouse.sent[0], bytes.fromhex("01 02 00 00 00 00 00 12"))
        self.assertTrue(self.mouse.saved)

    def test_reset_button(self):
        self.g.reset_button(0, 4)
        self.assertIn(bytes.fromhex("01 85 00 04 00 ff ff 12"), self.mouse.sent)
        self.assertIsNone(self.g.button(0, 4))

    def test_assign_unknown_function_is_refused(self):
        from m6980x_configtool.functions import FUNCTIONS_BY_KEY
        with self.assertRaises(ValueError):
            self.g.assign_function(0, 3, FUNCTIONS_BY_KEY["scroll-links"])
        self.assertFalse([p for p in self.mouse.sent if p[1] & 0x80 or p[0] == 0x02 and p[1]])

    def test_write_macro_like_original(self):
        record = bytes.fromhex("04 83 17 00")                       # "Hallo-Wel" from the capture
        self.g.write_macro(2, record, "Hallo-Wel")
        self.assertEqual(bytes(self.mouse.rom[0x02E0:0x02E4]), record)
        self.assertEqual(bytes(self.mouse.rom[0x1CD5:0x1CDF]), b"Hallo-Wel\x03")
        byte_writes = [p.hex(" ") for p in self.mouse.sent if p[:2] == b"\x02\x01"]
        self.assertEqual(byte_writes[:5], ["02 01 e0 02 00 04 00 12", "02 01 e1 02 00 83 00 12",
                                           "02 01 e2 02 00 17 00 12", "02 01 e3 02 00 00 00 12",
                                           "02 01 de 1c 00 03 00 12"])

    def test_write_macro_keeps_chosen_picture(self):
        info9 = 0x1CC1 + 9                                         # slot 0 picture byte
        self.mouse.rom[info9] = 0x68                               # "Klicker" has picture 4 on the real mouse
        self.g.write_macro(0, bytes.fromhex("06 21 41 00"), "Klicker")
        self.assertEqual(self.mouse.rom[info9], 0x68)
        self.assertEqual(self.g.macro_icon(0), 4)

    def test_write_macro_with_picture_and_reset(self):
        self.g.write_macro(5, bytes.fromhex("04 83 04 84 04 00"), "Test", icon=21)
        self.assertEqual(self.mouse.rom[0x1CC1 + 5 * 10 + 9], 100 + 21)
        self.g.set_macro_icon(5, None)                              # like a new macro: slot + 1
        self.assertEqual(self.mouse.rom[0x1CC1 + 5 * 10 + 9], 6)
        self.assertIsNone(self.g.macro_icon(5))
        with self.assertRaises(ValueError):
            self.g.set_macro_icon(5, 101)

    def test_clear_macro_like_original_release(self):
        self.g.write_macro(2, bytes.fromhex("04 83 17 00"), "Hallo-Wel")
        self.mouse.sent.clear()
        self.g.clear_macro(2)
        # capture 4 ("Freigeben"): 02 01 d5 1c 00 00 00 12
        self.assertIn(bytes.fromhex("02 01 d5 1c 00 00 00 12"), self.mouse.sent)
        self.assertEqual(self.mouse.rom[0x1CD5], 0)
        self.assertEqual(bytes(self.mouse.rom[0x02E0:0x02E4]), bytes.fromhex("04 83 17 00"))  # data stays

    def test_write_macro_rejects_bad_input(self):
        with self.assertRaises(ValueError):
            self.g.write_macro(0, bytes.fromhex("04 83 17"), "x")      # no end marker
        with self.assertRaises(ValueError):
            self.g.write_macro(0, bytes.fromhex("04 00"), "zehnzeichn")  # name too long
        with self.assertRaises(ValueError):
            self.g.write_macro(20, bytes.fromhex("04 00"), "x")
        self.assertEqual(self.mouse.sent, [])

    def test_rom_read_chunks(self):
        data = self.g.read_rom(0x0100, 10)
        self.assertEqual(data, bytes(range(0, 7)) + bytes(range(7, 10)))
        self.assertEqual(self.mouse.sent[0], bytes.fromhex("02 00 00 01 00 00 00 12"))

    def test_never_sends_write_commands(self):
        self.g.identify(); self.g.name(); self.g.dpi_stage(2); self.g.led_color(1)
        self.g.button(0, 4); self.g.read_rom(0, 14); self.g.report_interval()
        for pkt in self.mouse.sent:
            group, cmd = pkt[0], pkt[1]
            self.assertFalse(group == 0x01 and cmd & 0x80, pkt.hex(" "))
            self.assertFalse(group == 0x02 and cmd != 0x00, pkt.hex(" "))
            self.assertFalse(group == 0x00 and cmd not in (0x00, 0x01), pkt.hex(" "))

    def test_set_active_profile_sends_original_packet_and_verifies(self):
        self.g.set_active_profile(2)
        self.assertEqual(self.mouse.sent[0], bytes.fromhex("01 88 02 00 00 00 00 12"))
        self.assertEqual(self.mouse.sent[1][:2], b"\x01\x08")
        self.assertEqual(self.g.active_profile(), 2)

    def test_set_active_profile_rejects_invalid(self):
        with self.assertRaises(ValueError):
            self.g.set_active_profile(3)
        self.assertEqual(self.mouse.sent, [])

    def test_set_active_profile_detects_ignored_switch(self):
        g = M6980X(FakeMouse(accept_profile_switch=False), poll_interval=0)
        with self.assertRaises(M6980XError):
            g.set_active_profile(1)

    def test_set_led_color_sends_original_packet_and_verifies(self):
        self.g.set_led_color(1, 2, 0, 1)
        self.assertEqual(self.mouse.sent[0], bytes.fromhex("01 86 01 02 00 01 00 00"))
        self.assertEqual(self.g.led_color(1), (2, 0, 1))
        self.assertNotIn(b"\x01\x88", [pkt[:2] for pkt in self.mouse.sent])  # no profile switch

    def test_set_led_color_rejects_levels_above_2(self):
        for bad in [(3, 0, 0), (0, 255, 0), (0, 0, -1)]:
            with self.assertRaises(ValueError):
                self.g.set_led_color(0, *bad)
        with self.assertRaises(ValueError):
            self.g.set_led_color(3, 0, 0, 0)
        self.assertEqual(self.mouse.sent, [])

    def test_set_led_color_detects_ignored_write(self):
        self.mouse.accept_led = False
        with self.assertRaises(M6980XError):
            self.g.set_led_color(0, 0, 0, 2)

    def test_dpi_plan_matches_original_capture(self):
        steps = list(range(1, 57))
        self.assertEqual(dpi_write_plan(65, 65, steps), [(0, 56, 65, 56), (1, 56, 65, 56)])
        self.assertEqual(dpi_write_plan(67, 67, steps), [(0, 56, 67, 56), (1, 56, 67, 56)])
        self.assertEqual(dpi_write_plan(18, 42, steps), [(0, 18, 1, 1), (1, 42, 1, 1)])
        # unsupported step: next higher supported one, fraction reduced (30/32 -> 15/16)
        self.assertEqual(dpi_write_plan(30, 30, [16, 32])[0], (0, 32, 15, 16))

    def test_set_dpi_sends_original_packets_and_verifies(self):
        stage = self.g.set_dpi(2, 5850)
        self.assertEqual([p.hex(" ") for p in self.mouse.sent if p[1] & 0x80], [
            "01 83 00 02 38 00 00 12", "01 83 01 02 38 00 00 12",
            "01 84 00 02 41 38 00 12", "01 84 01 02 41 38 00 12"])
        self.assertEqual((stage.x_dpi, stage.y_dpi), (5850, 5850))

    def test_set_dpi_separate_axes_and_rounding(self):
        stage = self.g.set_dpi(3, 4510, 3000)          # rounded to 50 / 33 steps of 90
        self.assertEqual((stage.x_dpi, stage.y_dpi), (4500, 2970))
        self.assertEqual(self.g.dpi_stage(0).x_dpi, 1620)  # other stages untouched

    def test_set_dpi_rejects_out_of_range(self):
        for bad in (0, 30, 6030 + 90, 10000):
            with self.assertRaises(ValueError):
                self.g.set_dpi(0, bad)
        with self.assertRaises(ValueError):
            self.g.set_dpi(4, 1600)
        self.assertFalse([p for p in self.mouse.sent if p[1] & 0x80])

    def test_set_active_dpi_stage(self):
        self.g.set_active_dpi_stage(2)
        self.assertEqual(self.mouse.sent[0], bytes.fromhex("01 87 02 00 00 00 00 12"))
        self.assertEqual(self.g.active_dpi_stage(), 2)
        with self.assertRaises(ValueError):
            self.g.set_active_dpi_stage(4)

    def test_get_register_refuses_set_bit(self):
        with self.assertRaises(ValueError):
            self.g.get_register(0x86, 0)

    def test_failed_reads_are_retried_like_busy(self):
        class FlakyMouse(FakeMouse):
            reads = 0

            def get_feature(self):
                self.reads += 1
                if self.reads % 2:      # every other read fails like the real short report
                    raise transport.TransportError("unexpected feature report length 1: 00")
                return super().get_feature()

        g = M6980X(FlakyMouse(), poll_interval=0)
        self.assertEqual(g.name(), "G3")
        g.set_active_profile(2)
        self.assertEqual(g.active_profile(), 2)

    def test_timeout_when_never_done(self):
        self.mouse.busy_polls = 10_000
        g = M6980X(self.mouse, poll_interval=0, busy_tries=3, done_tries=3)
        with self.assertRaises(M6980XError):
            g.name()


class TransportTest(unittest.TestCase):
    def test_hidraw_ioctl_numbers(self):
        self.assertEqual(transport.HIDIOCSFEATURE, 0xC0094806)
        self.assertEqual(transport.HIDIOCGFEATURE, 0xC0094807)

    def test_strip_report_id(self):
        self.assertEqual(transport._strip_report_id(b"\x00" + bytes(range(8))), bytes(range(8)))
        self.assertEqual(transport._strip_report_id(bytes(range(8))), bytes(range(8)))


if __name__ == "__main__":
    unittest.main()
