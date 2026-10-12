"""Generates tests/fixtures/parity.json: scenarios against the simulated mouse of the Python prototype.

The Rust port replays the same scenarios and must send byte-identical packets
(crates/m6980x-core/src/parity_tests.rs). Run from the repository root:

    py -3 prototype/python/tools/export_fixtures.py
"""

import hashlib
import json
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))
sys.path.insert(0, os.path.join(HERE, "..", "tests"))

from m6980x_configtool import backup                      # noqa: E402
from m6980x_configtool import functions as fn             # noqa: E402
from m6980x_configtool.cli import apply_factory_defaults  # noqa: E402
from m6980x_configtool.protocol import M6980X             # noqa: E402
from test_protocol import FakeMouse                       # noqa: E402

OUT = os.path.join(HERE, "..", "..", "..", "tests", "fixtures", "parity.json")

LONG_MACRO = ", ".join(["tippe strg+c", "warte 100", "klick links", "rad runter 3", "consumer 0x00E9",
                        "consumer los", "warte 1000", "drueck shift", "tippe a+b", "los shift"] * 6)


def quiet(*_):
    pass


def scenario_settings(g):
    for p in range(3):
        for rgb in [(r, gr, b) for r in range(3) for gr in range(3) for b in range(3)][p::3]:
            g.set_led_color(p, *rgb)
    for ms in range(1, 9):
        g.set_report_interval(ms)
    for stage in (0, 2, 1, 3):
        g.set_active_dpi_stage(stage)
    for profile in (2, 0, 1):
        g.set_active_profile(profile)
    g.save_settings()
    apply_factory_defaults(g)


def scenario_dpi(g):
    for stage, x, y in [(0, 90, None), (1, 1600, None), (2, 5850, None), (3, 6030, None), (0, 4510, 3000),
                        (1, 4545, 4635), (2, 5040, 5130), (0, 1234, 999)]:
        g.set_dpi(stage, x, y)


def scenario_functions(g):
    for f in fn.FUNCTIONS:
        if f.size and f.record:
            for button in (3, 8):
                g.assign_function(f.index % 3, button, f)


def scenario_macros(g):
    long_record = bytes([fn.MODE_LOOP]) + fn.encode_events(LONG_MACRO) + b"\x00"
    g.write_macro(0, long_record, "Lang", icon=12)
    g.write_macro(19, bytes.fromhex("06 21 41 00"), "Klicker")
    g.write_macro(7, bytes.fromhex("04 83 04 88 64 84 04 00"), "XY", icon=100)
    g.set_macro_icon(3, 40)
    g.set_macro_icon(7, None)
    g.clear_macro(5)
    g.assign_macro(1, 4, 7)
    g.reset_button(0, 4)


def scenario_release(g):
    g.release_all()


def scenario_restore(g):
    snap = backup.create(g)
    g.write_macro(3, bytes.fromhex("04 83 04 84 04 00"), "Test", icon=7)
    g.assign_function(1, 3, fn.FUNCTIONS_BY_KEY["kopieren"])
    g.assign_function(2, 9, fn.FUNCTIONS_BY_KEY["browser-vor"])
    g.reset_button(0, 4)
    g.set_led_color(2, 0, 0, 0)
    g.set_dpi(0, 900)
    g.set_dpi(3, 4500, 2970)
    g.set_report_interval(8)
    g.set_active_dpi_stage(2)
    g.set_active_profile(2)
    g.t.sent.clear()
    return {"left": backup.restore(g, snap, log=quiet)}


def scenario_import(g):
    wanted = bytearray(g.t.rom)
    wanted[0x0100:0x0104] = b"\x04\x83\x05\x00"
    wanted[0x0400:0x0450] = bytes(range(0x50))
    wanted[0x1F01:0x1F25] = bytes(36)
    wanted[0x1F01 + 2 * 12 + 3] = 0x80
    a = 0x1F3D + 2 * 0x18 + 3 * 2
    wanted[a:a + 2] = (0x1D98).to_bytes(2, "little")
    wanted[0x1F01 + 1 * 12 + 9] = 0x80
    a = 0x1F3D + 1 * 0x18 + 9 * 2
    wanted[a:a + 2] = (0x0170).to_bytes(2, "little")
    wanted[0x1F80] = 0x55                                         # firmware area: stays as a difference
    return {"left": backup.import_rom(g, bytes(wanted), log=quiet)}


def scenario_reads(g):
    out = {"identify": g.identify(), "name": g.name(), "steps": g.dpi_supported_steps()}
    out["dpi"] = [[s.x_dpi, s.y_dpi] for s in (g.dpi_stage(i) for i in range(4))]
    out["buttons"] = [[g.button(p, b) for b in range(10)] for p in range(3)]
    out["rom"] = g.read_rom(0x1CC1, 200).hex()
    return out


SCENARIOS = {
    "settings": scenario_settings, "dpi": scenario_dpi, "functions": scenario_functions, "macros": scenario_macros,
    "release": scenario_release, "restore": scenario_restore, "import": scenario_import, "reads": scenario_reads,
}

ENCODE = ["drueck a, warte 100, los a", "tippe strg+alt+entf", "klick links\nmaus-drueck vor\nmaus-los zurueck",
          "rad hoch 15, rad runter", "consumer e9, consumer 0x022A, consumer los", "warte 255, warte 256, warte 65535",
          "  Tippe  STRG+Z  ,, ", "tippe y, tippe z, tippe 0, tippe leer, tippe space", LONG_MACRO,
          "drueck", "warte x", "rad", "rad hoch x", "consumer zz", "consumer 0x10000", "klick", "hallo welt",
          "tippe strg+", "warte -5", "rad runter 0"]

DECODE = ["05 83 04 88 64 84 04 89 f4 01 21 41 00 ff", "06 21 41 00", "04 61 7f 00", "04 7f 7f 7f 61 00", "04 7e 61 62 00",
          "04 85 e9 00 85 00 00 00", "04 83 e7 84 99 00", "04 3f 5f 60 80 00", "c8 00", "07 21 00", "ff ff", "00",
          "04 83", "04 85 01", "04 89 10", "04 21 22 41 42 00"]


def main():
    out = {"scenarios": {}, "encode": [], "decode": [], "files": {}}
    for name, run in SCENARIOS.items():
        mouse = FakeMouse()
        g = M6980X(mouse, poll_interval=0)
        extra = run(g) or {}
        out["scenarios"][name] = {"sent": [p.hex(" ") for p in mouse.sent],
                                  "rom_sha256": hashlib.sha256(bytes(mouse.rom)).hexdigest(), **extra}
    for text in ENCODE:
        try:
            out["encode"].append({"text": text, "hex": fn.encode_events(text).hex(" ")})
        except ValueError as e:
            out["encode"].append({"text": text, "error": str(e)})
    for record in DECODE:
        data = bytes.fromhex(record)
        mode, events = fn.decode_record(data)
        out["decode"].append({"hex": record, "mode": mode, "events": events, "length": fn.record_length(data)})
    out["functions"] = [{"key": f.key, "address": f.address, "record": f.record.hex(" ")} for f in fn.FUNCTIONS]
    with tempfile.TemporaryDirectory() as d:
        rom = bytes((i * 13 + 5) & 0xFF for i in range(8192))
        backup.write_meb(rom, os.path.join(d, "x.meb"))
        backup.write_macro_ini(bytes.fromhex("05 85 e9 00 85 00 00 89 00 01 21 41 00"), os.path.join(d, "m.ini"))
        for name in ("x.meb", "m.ini"):
            with open(os.path.join(d, name), "rb") as f:
                out["files"][name] = hashlib.sha256(f.read()).hexdigest()
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        json.dump(out, f, indent=1, ensure_ascii=False)
        f.write("\n")
    print(f"{OUT}: {len(out['scenarios'])} Szenarien, "
          f"{sum(len(s['sent']) for s in out['scenarios'].values())} Pakete")


if __name__ == "__main__":
    main()
