"""Command layer for the GHOST M6980X (reads plus individually verified set commands).

Packet on the wire (8 bytes, no report ID): [group, cmd, arg0..arg5].
Handshake taken from the original Macro.exe (0x444aac):
  1. GET_FEATURE until status bit 0x80 (busy) is clear
  2. SET_FEATURE with the command
  3. GET_FEATURE until status bit 0x40 (done) is set; reply = [status, data0..data6]
Reply bytes the command does not fill keep stale values from earlier replies,
so only documented bytes may be interpreted.

See docs/PROTOCOL.md for the evidence behind each command.
"""

import math
import time
from dataclasses import dataclass

from . import functions as fn
from . import rom as romlayout
from .transport import TransportError

STATUS_BUSY = 0x80
STATUS_DONE = 0x40

GROUP_SYSTEM = 0x00
GROUP_REGISTER = 0x01
GROUP_ROM = 0x02

ROM_SIZE = 0x2000          # 8 KB, mirrored above (verified)
ROM_READ_CHUNK = 7
ROM_FILL_CMDS = (0x03, 0x05, 0x06, 0x07, 0x08, 0x09)   # transfer buffer bytes 0-5, 6-11, ..., 30-31
PROFILES = 3
BUTTONS = 10
DPI_STAGES = 4             # entries in registers 03/04
DPI_BUTTON_STAGES = 3      # the DPI button cycles 0 -> 1 -> 2 (observed); LEDs show stage + 1
MAX_DPI = 6030             # Mappingdefault.ini [6980DPIdefault] max
LED_LEVELS = (0, 1, 2)     # per channel: off, half, full
ROM_TRAILER = 0x12         # constant last byte of ROM and set commands in the original
WRITE_DELAY = 0.010        # Macro.exe sleeps 10 ms after sending a set command


class M6980XError(Exception):
    pass


SUPPORTED_MODEL = "6980X"


def model_of(data):
    """Model name like the original (0x42ebd8) from the identify reply data (0C 12 ...):
    byte 6 = 0 -> 6980, 1 and byte 2 = 9 -> 6980X, 1 otherwise -> M8000X, 4 -> M8600, else unknown (None)."""
    if len(data) < 7 or data[0:2] != b"\x0c\x12":
        return None
    kind = data[6]
    if kind == 0:
        return "6980"
    if kind == 1:
        return "6980X" if data[2] == 9 else "M8000X"
    if kind == 4:
        return "M8600"
    return None


@dataclass
class DpiStage:
    x_step: int
    y_step: int
    unit: int
    x_num: int
    x_den: int
    y_num: int
    y_den: int

    @property
    def x_dpi(self):
        return self.x_step * self.unit * self.x_num // self.x_den

    @property
    def y_dpi(self):
        return self.y_step * self.unit * self.y_num // self.y_den


def dpi_to_steps(dpi, unit):
    steps = round(dpi / unit)
    if steps < 1 or steps * unit > MAX_DPI:
        raise ValueError(f"DPI muss zwischen {unit} und {MAX_DPI} liegen")
    return steps


def snap_step(value, supported):
    """0x445900: smallest supported sensor step >= value, otherwise the largest supported one."""
    higher = [s for s in supported if s >= value]
    return min(higher) if higher else max(supported)


def dpi_write_plan(x_steps, y_steps, supported):
    """Port of 0x445900: each axis gets a supported sensor step plus the fraction
    requested/sensor (reduced like 0x445640) that the firmware scales with.
    Returns [(axis, sensor_step, num, den)] for X (0) and Y (1)."""
    plan = []
    for axis, wanted in ((0, x_steps), (1, y_steps)):
        step = snap_step(wanted, supported)
        g = math.gcd(wanted, step)
        plan.append((axis, step, wanted // g, step // g))
    return plan


class M6980X:
    def __init__(self, transport, poll_interval=0.004, busy_tries=20, done_tries=200):
        self.t = transport
        self.poll_interval = poll_interval
        self.busy_tries = busy_tries
        self.done_tries = done_tries

    # -- low level -----------------------------------------------------------------
    def xfer(self, group, cmd, *args, trailer=None, delay=0.0):
        if len(args) > 6:
            raise ValueError("max. 6 Argument-Bytes")
        pkt = bytearray(8)
        pkt[0], pkt[1] = group, cmd
        pkt[2:2 + len(args)] = bytes(args)
        if trailer is not None:
            pkt[7] = trailer

        for _ in range(self.busy_tries):
            status = self._poll()
            if status is not None and not status[0] & STATUS_BUSY:
                break
            time.sleep(self.poll_interval)
        else:
            raise M6980XError("Maus meldet dauerhaft 'beschaeftigt'")

        self.t.set_feature(pkt)
        if delay:
            time.sleep(delay)

        for _ in range(self.done_tries):
            reply = self._poll()
            if reply is not None and reply[0] & STATUS_DONE:
                return reply
            time.sleep(self.poll_interval)
        raise M6980XError(f"keine Antwort auf {bytes(pkt).hex(' ')}")

    def _poll(self):
        # The original treats a failed GET_FEATURE like "not ready" and retries; the mouse
        # does return short reports, e.g. while the user presses its DPI/profile buttons.
        try:
            return self.t.get_feature()
        except (TransportError, OSError):
            return None

    def get_register(self, reg, *args):
        if reg & 0x80:
            raise ValueError("Bit 0x80 waere ein Schreibbefehl")
        return self.xfer(GROUP_REGISTER, reg, *args)

    def _set_register(self, reg, *args, trailer=ROM_TRAILER):
        # original set commands pass a 10 ms delay to the transfer; most end in 0x12
        return self.xfer(GROUP_REGISTER, reg | 0x80, *args, trailer=trailer, delay=WRITE_DELAY)

    # -- system ----------------------------------------------------------------------
    def identify(self):
        """Original only accepts the device if reply bytes 1-2 are 0C 12 (0x44407c)."""
        r = self.xfer(GROUP_SYSTEM, 0x00)
        return {"signature_ok": r[1:3] == b"\x0c\x12", "raw": r[1:].hex(" "), "model": model_of(r[1:])}

    def check_model(self):
        """Refuse anything but the 6980X: the other GHOST models have a different memory layout
        (M8000X/M8600: 32 KB, 5 profiles, 70 macros; 6980: other DPI range), so our addresses would be wrong."""
        ident = self.identify()
        if not ident["signature_ok"]:
            raise M6980XError(f"Geraet antwortet nicht wie eine GHOST-Maus (Identifizieren: {ident['raw']})")
        if ident["model"] != SUPPORTED_MODEL:
            raise M6980XError(f"Modell {ident['model'] or 'unbekannt'} wird nicht unterstuetzt, nur {SUPPORTED_MODEL} "
                              f"(Identifizieren: {ident['raw']})")
        return ident

    def name(self):
        r = self.xfer(GROUP_SYSTEM, 0x01)
        return r[1:7].split(b"\x00")[0].decode("ascii", "replace")

    # -- registers -------------------------------------------------------------------
    def active_profile(self):
        return self.get_register(0x08)[1]

    def set_active_profile(self, profile):
        """Switch the active hardware profile (0-2) and verify by reading it back."""
        if not 0 <= profile < PROFILES:
            raise ValueError(f"Profil muss 0..{PROFILES - 1} sein")
        self._set_register(0x08, profile)
        actual = self.active_profile()
        if actual != profile:
            raise M6980XError(f"Profilwechsel nicht uebernommen: Maus meldet Profil {actual}")

    def active_dpi_stage(self):
        """Register 07: active DPI stage, 0-based (1 = middle, 2 of 3 LEDs lit - seen on device)."""
        return self.get_register(0x07)[1]

    def register_0a(self):
        r = self.get_register(0x0A)
        return r[1] | r[2] << 8

    def save_settings(self):
        """Register 02 (01 02 00 .. 12, 4 ms), sent by the original when leaving its settings page.
        Without it, report rate and DPI changes are lost when the mouse is unplugged (tested);
        LED colours persist without it."""
        self.xfer(GROUP_REGISTER, 0x02, 0, trailer=ROM_TRAILER, delay=0.004)

    def report_interval(self):
        """Register 0B: USB polling interval in ms (1-8); report rate = 1000 / value Hz."""
        return self.get_register(0x0B)[1]

    def set_report_interval(self, ms):
        """Like the original (capture: 500 -> 333 Hz = 01 8B 02 -> 01 8B 03)."""
        if not 1 <= ms <= 8:
            raise ValueError("Abfrageintervall muss 1-8 ms sein")
        self._set_register(0x0B, ms, trailer=None)
        actual = self.report_interval()
        if actual != ms:
            raise M6980XError(f"Abtastrate nicht uebernommen: Maus meldet {actual} ms")

    def led_color(self, profile):
        r = self.get_register(0x06, profile)
        return r[1], r[2], r[3]

    def set_led_color(self, profile, red, green, blue):
        """Set the LED colour of a profile. Each channel has three levels: 0 off, 1 half, 2 full.
        Like the original (0x445ba8) this does not switch the active profile."""
        if not 0 <= profile < PROFILES:
            raise ValueError(f"Profil muss 0..{PROFILES - 1} sein")
        if any(c not in LED_LEVELS for c in (red, green, blue)):
            raise ValueError("LED-Stufen muessen 0 (aus), 1 (mittel) oder 2 (voll) sein")
        self._set_register(0x06, profile, red, green, blue, trailer=None)
        actual = self.led_color(profile)
        if actual != (red, green, blue):
            raise M6980XError(f"LED-Farbe nicht uebernommen: Maus meldet {actual}")

    def dpi_supported_steps(self):
        r = self.get_register(0x03, 1)
        return [byte * 8 + bit + 1 for byte in range(7) for bit in range(8) if r[1 + byte] >> bit & 1]

    def dpi_stage(self, stage):
        a = self.get_register(0x03, 0, stage)
        b = self.get_register(0x04, 0, stage)
        x, y, unit = a[1], a[2] or a[1], a[3] or 100
        # like the original (0x4457fc): only zero denominators are replaced by 1
        return DpiStage(x, y, unit, b[1], b[2] or 1, b[3], b[4] or 1)

    def set_active_dpi_stage(self, stage):
        """Select the active DPI stage like the original (0x445280). Software can also select
        stage 3, which the DPI button never reaches."""
        if not 0 <= stage < DPI_STAGES:
            raise ValueError(f"DPI-Stufe muss 0..{DPI_STAGES - 1} sein")
        self._set_register(0x07, stage)
        actual = self.active_dpi_stage()
        if actual != stage:
            raise M6980XError(f"DPI-Stufe nicht uebernommen: Maus meldet {actual}")

    def set_dpi(self, stage, x_dpi, y_dpi=None):
        """Set the DPI of a stage, rounded to the device unit (90). Returns the DpiStage read back."""
        if not 0 <= stage < DPI_STAGES:
            raise ValueError(f"DPI-Stufe muss 0..{DPI_STAGES - 1} sein")
        unit = self.dpi_stage(stage).unit
        x = dpi_to_steps(x_dpi, unit)
        y = x if y_dpi is None else dpi_to_steps(y_dpi, unit)
        plan = dpi_write_plan(x, y, self.dpi_supported_steps())
        # same order as the original: sensor steps X, Y, then fractions X, Y
        for axis, step, _num, _den in plan:
            self._set_register(0x03, axis, stage, step)
        for axis, _step, num, den in plan:
            self._set_register(0x04, axis, stage, num, den)
        actual = self.dpi_stage(stage)
        if (actual.x_dpi, actual.y_dpi) != (x * unit, y * unit):
            raise M6980XError(f"DPI nicht uebernommen: Maus meldet {actual.x_dpi}/{actual.y_dpi}")
        return actual

    def button(self, profile, button):
        """ROM address of the record assigned to a device button, or None for its default.
        Device numbers (determined on the mouse): 0 left, 1 right, 2 middle, 3 rear thumb, 4 front thumb,
        5 no button, 6 tilt left, 7 tilt right, 8 wheel up, 9 wheel down."""
        r = self.get_register(0x05, profile, button)
        if not r[1] & 0x80:
            return None
        return r[2] | r[3] << 8

    def set_button(self, profile, button, address):
        """Point a button at a function record or macro slot, like the original
        (01 85 <profile> <button> 80 <lo> <hi> 12, then re-activate the active profile)."""
        if not 0 <= profile < PROFILES or not 0 <= button < BUTTONS:
            raise ValueError("Profil oder Taste ausserhalb des Bereichs")
        if not 0 <= address < ROM_SIZE:
            raise ValueError("Adresse ausserhalb des ROM")
        active = self.active_profile()
        self._set_register(0x05, profile, button, 0x80, address & 0xFF, address >> 8)
        self.set_active_profile(active)
        actual = self.button(profile, button)
        if actual != address:
            raise M6980XError(f"Tastenbelegung nicht uebernommen: Maus meldet {actual}")

    # -- ROM -------------------------------------------------------------------------
    def read_rom(self, addr, length, progress=None):
        out = bytearray()
        while len(out) < length:
            a = addr + len(out)
            r = self.xfer(GROUP_ROM, 0x00, a & 0xFF, a >> 8 & 0xFF, 0, 0, 0, trailer=ROM_TRAILER)
            out += r[1:1 + ROM_READ_CHUNK]
            if progress:
                progress(min(len(out), length), length)
        return bytes(out[:length])

    def write_rom_byte(self, addr, value):
        """02 01 <lo> <hi> 00 <value> 00 12 - used by the original's macro editor (1 ms)."""
        self.xfer(GROUP_ROM, 0x01, addr & 0xFF, addr >> 8, 0, value, 0, trailer=ROM_TRAILER, delay=0.001)

    def write_rom_word(self, addr, data):
        """02 02 <lo> <hi> 00 <b0> <b1> 12 - 2-byte write of the original (0x444ba8, 1 ms)."""
        self.xfer(GROUP_ROM, 0x02, addr & 0xFF, addr >> 8, 0, data[0], data[1], trailer=ROM_TRAILER, delay=0.001)

    def write_rom_block(self, addr, data):
        """Up to 32 bytes via the transfer buffer, like the original (0x444c30): fill commands
        03/05/06/07/08/09 with 6 bytes each, then 02 04 <lo> <hi> 00 <len> 00 12."""
        if not 1 <= len(data) <= 32:
            raise ValueError("Block 1-32 Bytes")
        for i, cmd in enumerate(ROM_FILL_CMDS):
            chunk = data[6 * i:6 * i + 6]
            if not chunk:
                break
            self.xfer(GROUP_ROM, cmd, *chunk, delay=0.001)
        # commit delay grows with the length exactly like the original's [ebp-4]
        delay_ms = 1 if len(data) <= 12 else 3 if len(data) <= 18 else 6 if len(data) <= 24 else 9 if len(data) <= 30 else 10
        self.xfer(GROUP_ROM, 0x04, addr & 0xFF, addr >> 8, 0, len(data), 0, trailer=ROM_TRAILER, delay=delay_ms / 1000)

    def write_rom(self, addr, data, bytewise=False):
        """Write and verify. Blocks never cross a 32-byte boundary."""
        if addr < 0 or addr + len(data) > ROM_SIZE:
            raise ValueError("Schreibbereich ausserhalb des ROM")
        if bytewise:
            for i, b in enumerate(data):
                self.write_rom_byte(addr + i, b)
        else:
            pos = 0
            while pos < len(data):
                a = addr + pos
                n = min(32 - a % 32, len(data) - pos)
                if n == 1:
                    self.write_rom_byte(a, data[pos])
                elif n == 2:
                    self.write_rom_word(a, data[pos:pos + 2])
                else:
                    self.write_rom_block(a, data[pos:pos + n])
                pos += n
        actual = self.read_rom(addr, len(data))
        if actual != bytes(data):
            raise M6980XError(f"ROM 0x{addr:04X} nicht korrekt geschrieben: {actual.hex(' ')}")

    # -- high level: functions and macros (formats in docs/PROTOCOL.md) ---------------
    def reset_button(self, profile, button):
        """Back to the button's built-in function. The original GUI has no such option; this
        writes the state the mouse itself reports for untouched buttons (flags 00, address FFFF)."""
        if not 0 <= profile < PROFILES or not 0 <= button < BUTTONS:
            raise ValueError("Profil oder Taste ausserhalb des Bereichs")
        active = self.active_profile()
        self._set_register(0x05, profile, button, 0x00, 0xFF, 0xFF)
        self.set_active_profile(active)
        actual = self.button(profile, button)
        if actual is not None:
            raise M6980XError(f"Taste nicht auf Standard: Maus meldet 0x{actual:04X}")

    def assign_function(self, profile, button, function):
        """Like the original: (re)write the function record at its fixed address, then point the button at it.
        Mouse-button functions on the wheel buttons get the original's click record (functions.placement)."""
        if not function.size:
            raise ValueError(f"'{function.key}' hat keinen Datensatz - es ist die Grundfunktion des Kipprads "
                             "(Taste dort auf 'standard' setzen)")
        if not function.record:
            raise ValueError(f"Inhalt von '{function.key}' ist unbekannt - nicht zuweisbar")
        address, record = fn.placement(function, button)
        self.write_rom(address, record)
        self.set_button(profile, button, address)

    def assign_macro(self, profile, button, slot):
        self.set_button(profile, button, romlayout.macro_slot_addr(slot))

    def write_macro(self, slot, record, name, icon=None):
        """Write a macro (mode byte + events + 00) into a slot, then its info entry
        (picture byte, name max. 9 chars) - same order as the original.
        icon: picture 1-100; None keeps a picture already chosen for this slot."""
        self._check_slot(slot)
        if not 2 <= len(record) <= romlayout.MACRO_SLOT_SIZE or record[-1] != 0x00:
            raise ValueError(f"Makro muss mit 00 enden und hoechstens {romlayout.MACRO_SLOT_SIZE} Bytes lang sein")
        raw_name = name.encode("latin-1")
        if not 1 <= len(raw_name) <= romlayout.MACRO_NAME_LEN:
            raise ValueError(f"Name 1-{romlayout.MACRO_NAME_LEN} Zeichen")
        if icon is not None:
            self._check_icon(icon)
        info = self._info_addr(slot)
        if icon is None:
            icon = romlayout.macro_icon(self.read_rom(info, romlayout.MACRO_INFO_SIZE))
        self.write_rom(romlayout.macro_slot_addr(slot), record, bytewise=True)
        self.set_macro_icon(slot, icon)
        self.write_rom(info, raw_name.ljust(romlayout.MACRO_NAME_LEN, b"\x00"))

    def clear_macro(self, slot):
        """Like the original's "Freigeben": byte 0 of the macro name = 00, the macro data stays."""
        self._check_slot(slot)
        self.write_rom(self._info_addr(slot), b"\x00", bytewise=True)

    def release_all(self):
        """The original's "Freigeben" (capture 4): clear all macro names, all buttons of all profiles to default."""
        for slot in range(romlayout.MACRO_SLOTS):
            self.clear_macro(slot)
        for profile in range(PROFILES):
            for button in range(BUTTONS):
                self.reset_button(profile, button)

    def set_macro_icon(self, slot, icon):
        """Picture byte only, like the original's "Change Icon" (100 + n); None = no picture (slot + 1)."""
        self._check_slot(slot)
        if icon is not None:
            self._check_icon(icon)
        value = slot + 1 if icon is None else romlayout.ICON_OFFSET + icon
        self.write_rom(self._info_addr(slot) + romlayout.MACRO_NAME_LEN, bytes([value]), bytewise=True)

    def macro_icon(self, slot):
        self._check_slot(slot)
        return romlayout.macro_icon(self.read_rom(self._info_addr(slot), romlayout.MACRO_INFO_SIZE))

    @staticmethod
    def _info_addr(slot):
        return romlayout.MACRO_INFO_BASE + slot * romlayout.MACRO_INFO_SIZE

    @staticmethod
    def _check_slot(slot):
        if not 0 <= slot < romlayout.MACRO_SLOTS:
            raise ValueError(f"Makro-Slot muss 0..{romlayout.MACRO_SLOTS - 1} sein")

    @staticmethod
    def _check_icon(icon):
        if not 1 <= icon <= romlayout.ICON_COUNT:
            raise ValueError(f"Bildnummer 1-{romlayout.ICON_COUNT}")
