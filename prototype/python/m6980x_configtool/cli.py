"""m6980x-configtool: command line of the M6980X Configuration Tool (Gigabyte GHOST M6980X mouse)."""

import argparse
import hashlib
import json
import os
import sys
from dataclasses import asdict

from . import backup
from . import functions as fn
from . import rom as romlayout
from .protocol import (BUTTONS, DPI_BUTTON_STAGES, LED_LEVELS, PROFILES, ROM_SIZE, SUPPORTED_MODEL, M6980X,
                       M6980XError)
from .transport import TransportError, open_transport


def collect_info(g):
    stages = []
    for i in range(DPI_BUTTON_STAGES):
        s = g.dpi_stage(i)
        stages.append({**asdict(s), "x_dpi": s.x_dpi, "y_dpi": s.y_dpi})
    return {
        "identify": g.identify(),
        "name": g.name(),
        "active_profile": g.active_profile(),
        "active_dpi_stage": g.active_dpi_stage(),
        "register_0a": g.register_0a(),
        "report_interval_ms": g.report_interval(),
        "dpi_supported_steps": g.dpi_supported_steps(),
        "dpi_stages": stages,
        "led_colors": [list(g.led_color(p)) for p in range(PROFILES)],
    }


def cmd_info(g, args):
    info = collect_info(g)
    if args.json:
        print(json.dumps(info, indent=2))
        return
    ident = info["identify"]
    print(f"Geraet:            {info['name']}  (Signatur {'OK' if ident['signature_ok'] else 'FALSCH'}: {ident['raw']})")
    model = ident["model"] or "unbekannt"
    print(f"Modell:            {model}" + ("" if model == SUPPORTED_MODEL else f"   (nicht unterstuetzt, nur {SUPPORTED_MODEL})"))
    print(f"Aktives Profil:    {info['active_profile'] + 1}")
    print(f"Aktive DPI-Stufe:  {info['active_dpi_stage'] + 1}   (Register 07; LEDs zeigen die Stufe)")
    print(f"Register 0A:       0x{info['register_0a']:04X}   (ROM-Adresse des Konfigbereichs)")
    print(f"Abtastrate:        {1000 // info['report_interval_ms']} Hz   (Intervall {info['report_interval_ms']} ms)")
    steps = info["dpi_supported_steps"]
    print(f"DPI-Schritte:      {len(steps)} unterstuetzt ({steps[0]}..{steps[-1]})" if steps else "DPI-Schritte:      keine")
    for i, s in enumerate(info["dpi_stages"]):
        print(f"DPI-Stufe {i + 1}:       X {s['x_dpi']:5d}  Y {s['y_dpi']:5d}   "
              f"(Schritt {s['x_step']}/{s['y_step']} x {s['unit']}, Bruch {s['x_num']}/{s['x_den']} {s['y_num']}/{s['y_den']})")
    for p, rgb in enumerate(info["led_colors"]):
        print(f"LED Profil {p + 1}:      {format_led(rgb)}")


# channel levels: 0 = off, 1 = half, 2 = full
LED_NAMES = {
    # the 8 colours of the original colour picker (Macro.exe 0x451000-0x45115e)
    "aus": (0, 0, 0), "blau": (0, 0, 2), "gruen": (0, 2, 0), "hellblau": (0, 2, 2),
    "hellgruen": (1, 1, 0), "lila": (2, 0, 1), "gelb": (2, 1, 0), "rot": (2, 0, 0),
    # further combinations the mouse accepts
    "weiss": (2, 2, 2), "magenta": (2, 0, 2), "hellgelb": (2, 2, 0), "violett": (1, 0, 2),
    "himmelblau": (0, 1, 2), "cyan": (0, 2, 2), "orange": (2, 1, 0),
}


def format_led(rgb):
    rgb = tuple(rgb)
    if any(c not in LED_LEVELS for c in rgb):
        return f"R {rgb[0]:3d}  G {rgb[1]:3d}  B {rgb[2]:3d}   (ungueltig, erlaubt sind 0-2)"
    name = next((n for n, v in LED_NAMES.items() if v == rgb), "")
    return f"R {rgb[0]}  G {rgb[1]}  B {rgb[2]}   {name}"


def parse_led(values):
    if len(values) == 1 and values[0].lower() in LED_NAMES:
        return LED_NAMES[values[0].lower()]
    if len(values) == 3 and all(v in ("0", "1", "2") for v in values):
        return tuple(int(v) for v in values)
    raise ValueError("Farbe als Name (" + ", ".join(LED_NAMES) + ") oder als drei Stufen R G B je 0-2 angeben")


def cmd_led(g, args):
    if args.color:
        g.set_led_color(args.profile - 1, *parse_led(args.color))
    profiles = [args.profile - 1] if args.profile else range(PROFILES)
    for p in profiles:
        print(f"LED Profil {p + 1}: {format_led(g.led_color(p))}")


def persist(g, args):
    """Report rate, DPI, active profile/stage are lost on unplugging unless saved with 01 02 (tested)."""
    if getattr(args, "temporaer", False):
        print("(nicht dauerhaft gespeichert - geht beim Abstecken verloren)")
    else:
        g.save_settings()


def cmd_profile(g, args):
    if args.number is not None:
        g.set_active_profile(args.number - 1)
        persist(g, args)
    print(f"Aktives Profil: {g.active_profile() + 1}")


def cmd_dpi(g, args):
    if args.action == "select":
        g.set_active_dpi_stage(args.stage - 1)
        persist(g, args)
    elif args.action == "set":
        g.set_dpi(args.stage - 1, args.x_dpi, args.y_dpi)
        persist(g, args)
    elif args.action == "default":
        apply_factory_defaults(g)
        persist(g, args)
        print(f"Abtastrate: {1000 // FACTORY_INTERVAL_MS} Hz")
    active = g.active_dpi_stage()
    for i in range(DPI_BUTTON_STAGES):
        s = g.dpi_stage(i)
        marks = "  <- aktiv" if i == active else ""
        axes = f"{s.x_dpi}" if s.x_dpi == s.y_dpi else f"X {s.x_dpi} / Y {s.y_dpi}"
        print(f"Stufe {i + 1}: {axes:>18s} DPI{marks}")


# factory values of the three stages ([6980DPIdefault]; the INI's 4th value belongs to an unused stage)
FACTORY_DPI = (810, 1620, 3240)
FACTORY_INTERVAL_MS = 2   # 500 Hz: the original's Default button (0x44abbc) also writes 01 8B 02


def apply_factory_defaults(g):
    """Like the original "Default" button: DPI from [6980DPIdefault], then report rate 500 Hz.
    Returns the new DPI stages."""
    stages = [g.set_dpi(stage, dpi) for stage, dpi in enumerate(FACTORY_DPI)]
    g.set_report_interval(FACTORY_INTERVAL_MS)
    return stages


def cmd_watch(g, args):
    """Read-only: print every change of active profile / register 07 (e.g. while pressing buttons)."""
    import time
    end = time.monotonic() + args.seconds
    last = None
    print(f"Beobachte {args.seconds} s - Tasten an der Maus druecken ...", flush=True)
    while time.monotonic() < end:
        now = (g.active_profile(), g.active_dpi_stage())
        if now != last:
            print(f"{time.strftime('%H:%M:%S')}  Profil {now[0] + 1}   DPI-Stufe {now[1] + 1} (Register 07 = {now[1]})", flush=True)
            last = now
        time.sleep(0.1)


# device button numbers, determined on the mouse (each button typed its own number); 5 has no button
BUTTON_NAMES = {0: "links", 1: "rechts", 2: "mitte", 3: "hinten", 4: "vorne",
                6: "kipp-links", 7: "kipp-rechts", 8: "rad-hoch", 9: "rad-runter"}
# built-in function ("Standard") of each button, measured from the mouse's input reports (2026-10-11)
STANDARD_ACTIONS = {0: "Linksklick", 1: "Rechtsklick", 2: "Mittelklick", 3: "Zurueck, Maustaste 4",
                    4: "Vor, Maustaste 5", 5: "keine Taste", 6: "Bildlauf links", 7: "Bildlauf rechts",
                    8: "Bildlauf hoch", 9: "Bildlauf runter"}


def standard_label(button=None):
    return f"Standard ({STANDARD_ACTIONS[button]})" if button in STANDARD_ACTIONS else "Standard"


def parse_button(text):
    by_name = {v: k for k, v in BUTTON_NAMES.items()}
    if text in by_name:
        return by_name[text]
    if text.isdigit() and int(text) < BUTTONS:
        return int(text)
    raise ValueError(f"Taste als Nummer 0-{BUTTONS - 1} oder Name ({', '.join(by_name)})")


def macro_names(g):
    info = g.read_rom(romlayout.MACRO_INFO_BASE, romlayout.MACRO_SLOTS * romlayout.MACRO_INFO_SIZE)
    size = romlayout.MACRO_INFO_SIZE
    return [romlayout.macro_name(info[i * size:(i + 1) * size]) for i in range(romlayout.MACRO_SLOTS)]


def describe_target(addr, names, button=None):
    if addr is None:
        return standard_label(button)
    slot = romlayout.macro_slot_of(addr)
    if slot is not None:
        return f"Makro {slot + 1} {names[slot]!r}" if names[slot] else f"Makro {slot + 1}"
    f = fn.FUNCTIONS_BY_ADDRESS.get(addr)
    return f.label if f else f"ROM 0x{addr:04X}"


def cmd_buttons(g, args):
    names = macro_names(g)
    print("Taste          " + "".join(f"  Profil {p + 1:<25d}" for p in range(PROFILES)))
    for b in range(BUTTONS):
        cells = [describe_target(g.button(p, b), names, b) for p in range(PROFILES)]
        label = f"{b} {BUTTON_NAMES.get(b, '')}"
        print(f"{label:14s} " + "".join(f"  {c:<32s}" for c in cells))


def cmd_button(g, args):
    profile, button = args.profile - 1, parse_button(args.button)
    if args.target == "standard":
        g.reset_button(profile, button)
    elif args.target.startswith("makro"):
        slot = int(args.target.split(":", 1)[1]) - 1 if ":" in args.target else -1
        if not 0 <= slot < romlayout.MACRO_SLOTS:
            raise ValueError(f"Makro als makro:1 bis makro:{romlayout.MACRO_SLOTS}")
        g.assign_macro(profile, button, slot)
    else:
        f = fn.FUNCTIONS_BY_KEY.get(args.target)
        if f is None:
            raise ValueError("unbekannte Funktion - Liste mit: m6980x_configtool functions")
        if not f.confirmed:
            print(f"Hinweis: Inhalt von '{f.key}' ist abgeleitet, noch nicht am Geraet bestaetigt.")
        g.assign_function(profile, button, f)
    print(f"Profil {args.profile}, Taste {button}: {describe_target(g.button(profile, button), macro_names(g), button)}")


def cmd_functions(g, args):
    for f in fn.FUNCTIONS:
        state = ("nur als standard" if not f.size else "bestaetigt" if f.confirmed
                 else "abgeleitet" if f.record else "unbekannt")
        print(f"{f.key:20s} {f.label:28s} 0x{f.address:04X}  {state}")


RATES = {1000 // ms: ms for ms in range(1, 9)}   # 1000, 500, 333, 250, 200, 166, 142, 125 Hz


def cmd_rate(g, args):
    if args.hz is not None:
        g.set_report_interval(RATES[args.hz])
        persist(g, args)
    ms = g.report_interval()
    print(f"Abtastrate: {1000 // ms} Hz (Intervall {ms} ms)")


def read_full_rom(g):
    if not sys.stderr.isatty():
        return g.read_rom(0, ROM_SIZE)

    def progress(done, total):
        print(f"\r  ROM lesen: {done * 100 // total:3d} %", end="", file=sys.stderr, flush=True)
    data = g.read_rom(0, ROM_SIZE, progress)
    print(file=sys.stderr)
    return data


def cmd_rom_dump(g, args):
    data = read_full_rom(g)
    if args.file.lower().endswith(".meb"):
        backup.write_meb(data, args.file)              # format of the original software
    else:
        with open(args.file, "wb") as f:
            f.write(data)
    print(f"{len(data)} Bytes -> {args.file}")
    print(f"SHA-256 {hashlib.sha256(data).hexdigest()}")


def load_rom_file(path):
    if path.lower().endswith(".meb"):
        return backup.read_meb(path)
    with open(path, "rb") as f:
        data = f.read()
    if len(data) != ROM_SIZE:
        raise ValueError(f"ROM-Datei muss {ROM_SIZE} Bytes haben")
    return data


def cmd_rom_import(g, args):
    data = load_rom_file(args.file)
    left = backup.import_rom(g, data)
    print(f"Importiert: {args.file} (SHA-256 {hashlib.sha256(data).hexdigest()[:8]})")
    if left:
        print("Abweichungen im Firmware-Bereich (nicht direkt geschrieben):")
        for s, e in left:
            print(f"  0x{s:04X}-0x{e:04X}")
    print("Hinweis: .meb/.bin enthalten nur den Speicher (inkl. Tastenbelegung), nicht DPI, LED und Abtastrate.")


def cmd_save(g, args):
    g.save_settings()
    print("Einstellungen dauerhaft gespeichert (01 02).")


def cmd_backup(g, args):
    data = backup.create(g)
    backup.save(data, args.file)
    print(f"Sicherung -> {args.file}  (Profil {data['active_profile'] + 1}, "
          f"{1000 // data['report_interval_ms']} Hz, ROM {len(data['rom']) // 2} Bytes)")


def cmd_restore(g, args):
    data = backup.load(args.file)
    print(f"Stelle Sicherung vom {data['created']} wieder her ...")
    left = backup.restore(g, data)
    g.save_settings()
    if left:
        print("Abweichungen im Firmware-Bereich (nicht direkt geschrieben):")
        for s, e in left:
            print(f"  0x{s:04X}-0x{e:04X}")
    else:
        print("Fertig - ROM bytegleich mit der Sicherung, Einstellungen wiederhergestellt.")


def cmd_macros(g, args):
    if args.rom:
        data = load_rom_file(args.rom)
    else:
        data = read_full_rom(g)
    for s in range(romlayout.MACRO_SLOTS):
        start = romlayout.macro_slot_addr(s)
        mode, events = fn.decode_record(data[start:start + romlayout.MACRO_SLOT_SIZE])
        info = romlayout.macro_info(data, s)
        name = romlayout.macro_name(info)
        if mode is None and not name:
            continue
        size = fn.record_length(data[start:start + romlayout.MACRO_SLOT_SIZE])
        print(f"Makro {s + 1:2d} {name!r:12s} {format_icon(romlayout.macro_icon(info)):30s} "
              f"{size:3d}/{romlayout.MACRO_SLOT_SIZE} B  Modus {mode or '-':9s} {', '.join(events) or '(keine Ereignisse)'}")


def format_icon(icon):
    return "kein Bild" if icon is None else f"Bild {icon} ({romlayout.icon_source(icon)})"


def parse_icon(text):
    if text in ("keins", "kein"):
        return None
    if text.isdigit() and 1 <= int(text) <= romlayout.ICON_COUNT:
        return int(text)
    raise ValueError(f"Bild als Nummer 1-{romlayout.ICON_COUNT} oder 'keins'")


def cmd_macro(g, args):
    record = bytes([fn.MODE_NAMES[args.mode]]) + fn.encode_events(" ".join(args.events)) + bytes([fn.EV_END])
    g.write_macro(args.slot - 1, record, args.name, icon=args.bild)
    mode, events = fn.decode_record(g.read_rom(romlayout.macro_slot_addr(args.slot - 1), len(record)))
    print(f"Makro {args.slot} {args.name!r} geschrieben ({len(record)}/{romlayout.MACRO_SLOT_SIZE} Bytes, "
          f"{format_icon(g.macro_icon(args.slot - 1))}): Modus {mode}, {', '.join(events)}")


def cmd_macro_export(g, args):
    """Write a macro slot as the original's export file ([Macro] count=, 0=, 1=, ...; the name is not stored)."""
    slot = romlayout.macro_slot_addr(args.slot - 1)
    data = g.read_rom(slot, romlayout.MACRO_SLOT_SIZE)
    record = data[:fn.record_length(data)]
    backup.write_macro_ini(record, args.file)
    mode, events = fn.decode_record(record)
    print(f"Makro {args.slot} -> {args.file} ({len(record)} Bytes, Modus {mode}): {', '.join(events)}")


def cmd_macro_import(g, args):
    """Read the original's export file into a slot; the file has no name, so --name or the file name is used."""
    record = backup.read_macro_ini(args.file)
    name = args.name if args.name is not None else os.path.splitext(os.path.basename(args.file))[0]
    name = name[:romlayout.MACRO_NAME_LEN]
    g.write_macro(args.slot - 1, record, name, icon=args.bild)
    mode, events = fn.decode_record(record)
    print(f"{args.file} -> Makro {args.slot} {name!r} ({len(record)} Bytes, Modus {mode}): {', '.join(events)}")


def cmd_macro_icon(g, args):
    if args.bild is not None:
        g.set_macro_icon(args.slot - 1, parse_icon(args.bild))
    print(f"Makro {args.slot}: {format_icon(g.macro_icon(args.slot - 1))}")


def main(argv=None):
    ap = argparse.ArgumentParser(prog="m6980x_configtool", description="M6980X Configuration Tool (Gigabyte GHOST M6980X)")
    ap.add_argument("--device", help="Geraetepfad (hidapi-Pfad oder /dev/hidrawN)")
    sub = ap.add_subparsers(dest="command", required=True)
    sub.add_parser("gui", help="grafische Oberflaeche starten")
    p = sub.add_parser("info", help="Geraet, Profil, DPI und LED anzeigen")
    p.add_argument("--json", action="store_true")
    p = sub.add_parser("profile", help="aktives Profil anzeigen oder wechseln")
    p.add_argument("number", nargs="?", type=int, choices=range(1, PROFILES + 1), help="Profil 1-3")
    p.add_argument("--temporaer", action="store_true", help="nur bis zum Abstecken, nicht dauerhaft speichern")
    p = sub.add_parser("led", help="LED-Farben anzeigen oder setzen, z. B. 'led 1 rot' oder 'led 1 2 0 1'")
    p.add_argument("profile", nargs="?", type=int, choices=range(1, PROFILES + 1), help="Profil 1-3")
    p.add_argument("color", nargs="*", help="Farbname oder R G B mit Stufen 0 (aus), 1 (mittel), 2 (voll)")
    p = sub.add_parser("dpi", help="DPI-Stufen anzeigen, auswaehlen oder setzen")
    dpi_sub = p.add_subparsers(dest="action")
    q = dpi_sub.add_parser("select", help="aktive DPI-Stufe waehlen")
    q.add_argument("stage", type=int, choices=range(1, DPI_BUTTON_STAGES + 1))
    q.add_argument("--temporaer", action="store_true", help="nur bis zum Abstecken, nicht dauerhaft speichern")
    q = dpi_sub.add_parser("set", help="DPI einer Stufe setzen (Vielfache von 90, max. 6030)")
    q.add_argument("stage", type=int, choices=range(1, DPI_BUTTON_STAGES + 1))
    q.add_argument("x_dpi", type=int, help="DPI (X und Y, wenn kein Y-Wert folgt)")
    q.add_argument("y_dpi", type=int, nargs="?", help="eigener Y-Wert")
    q.add_argument("--temporaer", action="store_true", help="nur bis zum Abstecken, nicht dauerhaft speichern")
    q = dpi_sub.add_parser("default", help="Werkswerte 810/1620/3240 und 500 Hz wie der Default-Knopf des Originals")
    q.add_argument("--temporaer", action="store_true", help="nur bis zum Abstecken, nicht dauerhaft speichern")
    p = sub.add_parser("watch", help="Profil und DPI-Stufe live beobachten (nur lesend)")
    p.add_argument("--seconds", type=float, default=30)
    p = sub.add_parser("rate", help="Abtastrate anzeigen oder setzen")
    p.add_argument("hz", nargs="?", type=int, choices=sorted(RATES, reverse=True), help="Hz")
    p.add_argument("--temporaer", action="store_true", help="nur bis zum Abstecken, nicht dauerhaft speichern")
    sub.add_parser("buttons", help="Tastenbelegung aller Profile anzeigen")
    p = sub.add_parser("button", help="Taste belegen, z. B. 'button 1 hinten kopieren' oder 'button 1 vorne makro:3'")
    p.add_argument("profile", type=int, choices=range(1, PROFILES + 1))
    p.add_argument("button", help="links, rechts, mitte, hinten, vorne, kipp-links, kipp-rechts, rad-hoch, rad-runter oder 0-9")
    p.add_argument("target", help="Funktion (siehe 'functions'), makro:N oder standard")
    sub.add_parser("functions", help="belegbare Funktionen auflisten")
    p = sub.add_parser("macro", help="Makro schreiben, z. B. 'macro 5 Test tippe strg+c, warte 100, klick links'")
    p.add_argument("slot", type=int, choices=range(1, romlayout.MACRO_SLOTS + 1))
    p.add_argument("name", help=f"max. {romlayout.MACRO_NAME_LEN} Zeichen")
    p.add_argument("events", nargs="+", help="drueck/los/tippe TASTE[+TASTE], warte MS, maus-drueck/maus-los/klick links|rechts|mitte")
    p.add_argument("--mode", choices=list(fn.MODE_NAMES), default="einmal")
    p.add_argument("--bild", type=int, choices=range(1, romlayout.ICON_COUNT + 1), metavar="1-100",
                   help="Vorschaubild: 1-40 eingebaut, 41-100 eigene bmp-Dateien; ohne Angabe bleibt das vorhandene")
    p = sub.add_parser("macro-export", help="Makro als Datei im Format der Original-Software (.ini) speichern")
    p.add_argument("slot", type=int, choices=range(1, romlayout.MACRO_SLOTS + 1))
    p.add_argument("file")
    p = sub.add_parser("macro-import", help="Makrodatei der Original-Software (.ini) in einen Slot schreiben")
    p.add_argument("slot", type=int, choices=range(1, romlayout.MACRO_SLOTS + 1))
    p.add_argument("file")
    p.add_argument("--name", help=f"max. {romlayout.MACRO_NAME_LEN} Zeichen; ohne Angabe der Dateiname "
                                  "(die Datei enthaelt keinen Namen)")
    p.add_argument("--bild", type=int, choices=range(1, romlayout.ICON_COUNT + 1), metavar="1-100")
    p = sub.add_parser("macro-bild", help="Vorschaubild eines Makros anzeigen oder setzen ('keins' entfernt es)")
    p.add_argument("slot", type=int, choices=range(1, romlayout.MACRO_SLOTS + 1))
    p.add_argument("bild", nargs="?", help="1-100 oder keins")
    sub.add_parser("save", help="Abtastrate/DPI dauerhaft speichern (sonst beim Abstecken verloren)")
    p = sub.add_parser("backup", help="komplette Sicherung (ROM + DPI, LED, Abtastrate, Tasten) als JSON")
    p.add_argument("file")
    p = sub.add_parser("restore", help="Sicherung zurueckschreiben (nur Abweichungen)")
    p.add_argument("file")
    p = sub.add_parser("rom-dump", help="8-KB-ROM in Datei sichern (.meb = Format der Original-Software, sonst roh)")
    p.add_argument("file")
    p = sub.add_parser("rom-import", help="8-KB-ROM aus .meb (Original-Software) oder .bin zurueckschreiben")
    p.add_argument("file")
    p = sub.add_parser("macros", help="Makro-Slots und Info-Tabelle anzeigen")
    p.add_argument("--rom", help="ROM-Datei statt Maus lesen")
    args = ap.parse_args(argv)

    handlers = {"info": cmd_info, "profile": cmd_profile, "led": cmd_led, "dpi": cmd_dpi, "watch": cmd_watch,
                "rate": cmd_rate, "buttons": cmd_buttons, "button": cmd_button, "functions": cmd_functions,
                "macro": cmd_macro, "macro-bild": cmd_macro_icon,
                "macro-export": cmd_macro_export, "macro-import": cmd_macro_import, "save": cmd_save, "backup": cmd_backup, "restore": cmd_restore, "rom-dump": cmd_rom_dump, "rom-import": cmd_rom_import, "macros": cmd_macros}
    if args.command == "led" and args.color:
        try:
            parse_led(args.color)
        except ValueError as e:
            ap.error(str(e))
    if args.command == "gui":
        from .gui import main as gui_main
        return gui_main()
    if args.command == "macros" and args.rom or args.command == "functions":
        handlers[args.command](None, args)
        return 0
    try:
        t = open_transport(args.device)
    except TransportError as e:
        print(f"Fehler: {e}", file=sys.stderr)
        return 1
    try:
        g = M6980X(t)
        if args.command != "info":          # info stays usable to see which model is attached
            g.check_model()
        handlers[args.command](g, args)
    except (M6980XError, TransportError, ValueError) as e:
        print(f"Fehler: {e}", file=sys.stderr)
        return 1
    finally:
        t.close()
    return 0
