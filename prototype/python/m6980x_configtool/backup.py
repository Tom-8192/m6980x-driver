"""Complete backup/restore of the mouse: the 8 KB ROM plus the settings that live outside it
(active profile, DPI stages, active DPI stage, LED colours, report rate, button assignments).

Format: JSON, see docs/PROTOCOL.md. ROM bytes in the firmware area 0x1F00-0x1FFF are not written
directly, except the button address table, which the firmware leaves stale after a reset to default.
"""

import datetime
import json

from . import functions as fn
from . import rom as romlayout
from .protocol import BUTTONS, DPI_STAGES, PROFILES, ROM_SIZE

FORMAT = "m6980x-configtool-backup"
OLD_FORMATS = ("ghost-backup",)      # written before the rename (2026-10-11)
VERSION = 1
FIRMWARE_AREA = 0x1F00
TABLE_START = romlayout.BUTTON_TABLE
TABLE_END = romlayout.BUTTON_TABLE + PROFILES * romlayout.BUTTON_TABLE_STRIDE


def create(g, progress=None):
    stages = [g.dpi_stage(i) for i in range(DPI_STAGES)]
    return {
        "format": FORMAT, "version": VERSION,
        "created": datetime.datetime.now().isoformat(timespec="seconds"),
        "device": g.name(),
        "active_profile": g.active_profile(),
        "active_dpi_stage": g.active_dpi_stage(),
        "report_interval_ms": g.report_interval(),
        "dpi_stages": [{"x_dpi": s.x_dpi, "y_dpi": s.y_dpi} for s in stages],
        "led_colors": [list(g.led_color(p)) for p in range(PROFILES)],
        "buttons": [[g.button(p, b) for b in range(BUTTONS)] for p in range(PROFILES)],
        "rom": g.read_rom(0, ROM_SIZE, progress).hex(),
    }


def save(data, path):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=1)


def load(path):
    with open(path, encoding="utf-8") as f:
        data = json.load(f)
    if data.get("format") not in (FORMAT, *OLD_FORMATS) or data.get("version") != VERSION:
        raise ValueError("keine Sicherung des M6980X Configuration Tool (Version 1)")
    if len(bytes.fromhex(data["rom"])) != ROM_SIZE:
        raise ValueError("ROM-Abbild hat nicht 8192 Bytes")
    return data


def read_meb(path):
    """Backup file of the original software ("Backup all data to file"): INI section [Mem],
    count=8192, then one line '<address>=<decimal value>' per byte. Contains only the 8 KB ROM."""
    values, count = {}, None
    with open(path, encoding="latin-1") as f:
        for line in f:
            key, sep, value = line.strip().partition("=")
            if not sep:
                continue
            if key.lower() == "count":
                count = int(value)
            elif key.isdigit():
                values[int(key)] = int(value)
    if count != ROM_SIZE or sorted(values) != list(range(ROM_SIZE)) or any(not 0 <= v <= 255 for v in values.values()):
        raise ValueError("keine gueltige .meb-Datei (8192 Bytes erwartet)")
    return bytes(values[i] for i in range(ROM_SIZE))


def write_meb(rom, path):
    """Byte-identical to the files the original software writes (CRLF line ends)."""
    lines = ["[Mem]", f"count={len(rom)}"] + [f"{i}={b}" for i, b in enumerate(rom)]
    with open(path, "w", encoding="latin-1", newline="\r\n") as f:
        f.write("\n".join(lines) + "\n")


def write_macro_ini(record, path):
    """Macro export file of the original's editor (0x4148a0): TIniFile, section [Macro], count=<bytes>,
    then '<index>=<byte decimal>' for the raw record (mode byte, events, 00). The name is not stored.
    Like the original, an empty macro (mode + 00 only) is not exported."""
    if len(record) <= 2:
        raise ValueError("leeres Makro - das Original exportiert es nicht")
    lines = ["[Macro]", f"count={len(record)}"] + [f"{i}={b}" for i, b in enumerate(record)]
    with open(path, "w", encoding="latin-1", newline="\r\n") as f:
        f.write("\n".join(lines) + "\n")


def read_macro_ini(path, max_size=romlayout.MACRO_SLOT_SIZE):
    """Read a macro export file of the original (import 0x418408): returns the record bytes, cut at the
    00 end marker. [SpecialMacro] index != 0 makes the original keep only the mode byte (0x415df8)."""
    sections, current = {}, None
    with open(path, encoding="latin-1") as f:
        for line in f:
            line = line.strip().lstrip("﻿\xef\xbb\xbf")
            if line.startswith("[") and line.endswith("]"):
                current = sections.setdefault(line[1:-1].strip().lower(), {})
            elif current is not None and "=" in line:
                key, _, value = line.partition("=")
                current[key.strip().lower()] = value.strip()

    def number(section, key):                       # like TIniFile.ReadInteger with default 0
        try:
            return int(sections.get(section, {}).get(key, "0"))
        except ValueError:
            return 0

    count = number("macro", "count")
    if not 2 <= count <= max_size:
        raise ValueError(f"keine gueltige Makrodatei (count={count}, erlaubt 2-{max_size})")
    data = bytes(number("macro", str(i)) & 0xFF for i in range(count))
    if data[0] not in (0x04, 0x05, 0x06):
        raise ValueError(f"unbekannter Makro-Modus 0x{data[0]:02X}")
    if number("specialmacro", "index"):
        return bytes([data[0], 0x00])
    length = fn.record_length(data)                  # walks the events: 00 also occurs inside arguments
    return data[:length] if data[length - 1] == fn.EV_END else data + bytes([fn.EV_END])


def import_rom(g, wanted, log=print):
    """Write a ROM image (e.g. from a .meb file): everything below the firmware area and the button table,
    then the button assignments the image holds (flags 0x1F01 + table) via 01 85, like restore() does.
    The original's restore rewrites all 256 pages instead; this only touches what differs.
    Returns remaining differences."""
    current = g.read_rom(0, ROM_SIZE)
    for s, e in _diff_runs(current, wanted, 0, FIRMWARE_AREA):
        g.write_rom(s, wanted[s:e + 1])
        log(f"ROM 0x{s:04X}-0x{e:04X} geschrieben")
    for s, e in _diff_runs(current, wanted, TABLE_START, TABLE_END):
        g.write_rom(s, wanted[s:e + 1])
        log(f"Tastentabelle 0x{s:04X}-0x{e:04X} geschrieben")
    for p in range(PROFILES):
        for b in range(BUTTONS):
            target = romlayout.button_assignment(wanted, p, b)
            if target is not None and target >= ROM_SIZE:
                log(f"Profil {p + 1}, Taste {b}: ungueltige Adresse 0x{target:04X} in der Datei - uebersprungen")
                continue
            if g.button(p, b) != target:
                if target is None:
                    g.reset_button(p, b)
                else:
                    g.set_button(p, b, target)
                log(f"Profil {p + 1}, Taste {b} -> {'Standard' if target is None else f'0x{target:04X}'}")
    final = g.read_rom(0, ROM_SIZE)
    return _diff_runs(final, wanted, 0, ROM_SIZE)


def _diff_runs(current, wanted, start, end):
    runs = []
    for i in range(start, end):
        if current[i] != wanted[i]:
            if runs and i <= runs[-1][1] + 1:
                runs[-1][1] = i
            else:
                runs.append([i, i])
    return runs


def restore(g, data, log=print):
    """Write back everything that differs; returns remaining firmware-area differences."""
    wanted = bytes.fromhex(data["rom"])
    current = g.read_rom(0, ROM_SIZE)
    for s, e in _diff_runs(current, wanted, 0, FIRMWARE_AREA):
        g.write_rom(s, wanted[s:e + 1])
        log(f"ROM 0x{s:04X}-0x{e:04X} zurueckgeschrieben")
    for p in range(PROFILES):
        for b in range(BUTTONS):
            target = data["buttons"][p][b]
            if g.button(p, b) != target:
                if target is None:
                    g.reset_button(p, b)
                else:
                    g.set_button(p, b, target)
                log(f"Profil {p + 1}, Taste {b} wiederhergestellt")
    current = g.read_rom(TABLE_START, TABLE_END - TABLE_START)
    for s, e in _diff_runs(current, wanted[TABLE_START:TABLE_END], 0, TABLE_END - TABLE_START):
        g.write_rom(TABLE_START + s, wanted[TABLE_START + s:TABLE_START + e + 1])
        log(f"Tastentabelle 0x{TABLE_START + s:04X} zurueckgeschrieben")
    for p, rgb in enumerate(data["led_colors"]):
        if list(g.led_color(p)) != rgb:
            g.set_led_color(p, *rgb)
            log(f"LED Profil {p + 1} wiederhergestellt")
    for i, st in enumerate(data["dpi_stages"]):
        s = g.dpi_stage(i)
        if (s.x_dpi, s.y_dpi) != (st["x_dpi"], st["y_dpi"]):
            g.set_dpi(i, st["x_dpi"], st["y_dpi"])
            log(f"DPI-Stufe {i + 1} wiederhergestellt")
    if g.report_interval() != data["report_interval_ms"]:
        g.set_report_interval(data["report_interval_ms"])
        log("Abtastrate wiederhergestellt")
    if g.active_dpi_stage() != data["active_dpi_stage"]:
        g.set_active_dpi_stage(data["active_dpi_stage"])
    if g.active_profile() != data["active_profile"]:
        g.set_active_profile(data["active_profile"])
    final = g.read_rom(0, ROM_SIZE)
    return [(s, e) for s, e in _diff_runs(final, wanted, 0, ROM_SIZE)]
