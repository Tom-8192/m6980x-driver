"""Button functions, key codes and the macro event format of the GHOST M6980X.

Function records sit at fixed ROM addresses: 0x1D8C + sum of the sizes of all earlier entries
in Mappingdefault.ini [List]/[ListMemSize] (verified for 5 functions, see docs/PROTOCOL.md).
"""

from dataclasses import dataclass

FUNCTION_BASE = 0x1D8C

MODE_ONCE, MODE_LOOP, MODE_HOLD = 0x04, 0x05, 0x06
MODE_NAMES = {"einmal": MODE_ONCE, "schleife": MODE_LOOP, "feuer": MODE_HOLD}

EV_END, EV_KEY_DOWN, EV_KEY_UP, EV_CONSUMER = 0x00, 0x83, 0x84, 0x85
EV_WAIT8, EV_WAIT16 = 0x88, 0x89
EV_MOUSE_DOWN, EV_MOUSE_UP = 0x20, 0x40          # | button mask
EV_WHEEL = 0x60                                    # | signed 5-bit delta: 61 = up 1, 7F = down 1 (tested)
EV_WHEEL_UP, EV_WHEEL_DOWN = 0x61, 0x7F            # the original's editor (0x410ae8) only ever writes these two
MOUSE_BUTTONS = {"links": 0x01, "rechts": 0x02, "mitte": 0x04, "zurueck": 0x08, "vor": 0x10}

# HID keyboard usages (USB HID Usage Tables, page 0x07). Usages are key *positions* of a US keyboard;
# names here follow the German QWERTZ layout, where Y and Z swap places (US-z position = German y).
KEYS = {chr(ord("a") + i): 0x04 + i for i in range(26)}
KEYS["y"], KEYS["z"] = 0x1D, 0x1C
KEYS.update({str(d): 0x1E + (d - 1) % 10 for d in range(10)})
KEYS.update({f"f{n}": 0x3A + n - 1 for n in range(1, 13)})
KEYS.update({
    "enter": 0x28, "esc": 0x29, "backspace": 0x2A, "tab": 0x2B, "space": 0x2C, "leer": 0x2C,
    "minus": 0x2D, "plus": 0x2E, "druck": 0x46, "rollen": 0x47, "pause": 0x48, "einfg": 0x49,
    "pos1": 0x4A, "bildauf": 0x4B, "entf": 0x4C, "ende": 0x4D, "bildab": 0x4E,
    "rechts": 0x4F, "links": 0x50, "runter": 0x51, "hoch": 0x52,
    "strg": 0xE0, "shift": 0xE1, "alt": 0xE2, "win": 0xE3,
    "strg-r": 0xE4, "shift-r": 0xE5, "altgr": 0xE6, "win-r": 0xE7,
})
KEY_NAMES = {v: k for k, v in reversed(list(KEYS.items()))}


@dataclass(frozen=True)
class Function:
    index: int
    key: str            # CLI name
    label: str
    size: int
    record: bytes       # empty = content unknown, cannot be assigned
    confirmed: bool     # record seen in the original's capture or tested on the device (2026-10-11)
    address: int = 0


def _combo(*usages):
    """04 + press all, release in reverse + 00 (pattern of Ctrl+C / Ctrl+V / Win+D)."""
    ev = [b for u in usages for b in (EV_KEY_DOWN, u)] + [b for u in reversed(usages) for b in (EV_KEY_UP, u)]
    return bytes([MODE_ONCE, *ev, EV_END])


def _consumer(usage):
    return bytes([MODE_ONCE, EV_CONSUMER, usage & 0xFF, usage >> 8, EV_CONSUMER, 0, 0, EV_END])


def _hold_key(usage):
    return bytes([MODE_HOLD, EV_KEY_DOWN, usage, 0xFF, 0xFF, EV_END])


def _window_menu(usage):
    """Alt+Space opens the window menu, then the menu letter (German Windows: N minimise, X maximise)."""
    alt, space = KEYS["alt"], KEYS["space"]
    return bytes([MODE_ONCE, EV_KEY_DOWN, alt, EV_KEY_DOWN, space, EV_KEY_UP, space, EV_KEY_UP, alt,
                  EV_KEY_DOWN, usage, EV_KEY_UP, usage, EV_END])


def _hold_mouse(mask):
    return bytes([MODE_HOLD, EV_MOUSE_DOWN | mask, EV_WAIT16, 0xFF, 0xFF, EV_END])


H = bytes.fromhex
S, C, W = KEYS["strg"], KEYS["shift"], KEYS["win"]
# (key, label, ListMemSize, record, confirmed) in Mappingdefault.ini order
_TABLE = [
    ("linksklick", "Linksklick", 6, _hold_mouse(0x01), True),
    ("rechtsklick", "Rechtsklick", 6, _hold_mouse(0x02), True),
    ("mittelklick", "Mittelklick", 6, H("06 24 89 ff ff 00"), True),
    ("browser-vor", "Browser vor", 6, _hold_mouse(0x10), True),
    ("browser-zurueck", "Browser zurueck", 6, _hold_mouse(0x08), True),
    # INI name "Perfect"; on the 6980 the original sends AL Calculator (other mice: sensor command C1 00)
    ("rechner", "Rechner (Consumer 0x0192)", 8, H("04 85 92 01 85 00 00 00"), True),
    ("scroll-links", "Bildlauf links (= Standard von Kipprad links)", 0, b"", True),
    ("scroll-rechts", "Bildlauf rechts (= Standard von Kipprad rechts)", 0, b"", True),
    ("scroll-vor", "Bildlauf hoch", 3, bytes([MODE_ONCE, EV_WHEEL_UP, EV_END]), True),
    ("scroll-zurueck", "Bildlauf runter", 3, bytes([MODE_ONCE, EV_WHEEL_DOWN, EV_END]), True),
    ("doppelklick", "Doppelklick", 6, H("04 21 41 21 41 00"), True),
    ("entf", "Entfernen", 6, H("04 83 4c 84 4c 00"), True),
    ("rueckgaengig", "Rueckgaengig (Strg+Z)", 10, _combo(S, KEYS["z"]), True),
    ("ausschneiden", "Ausschneiden (Strg+X)", 10, _combo(S, KEYS["x"]), True),
    ("kopieren", "Kopieren (Strg+C)", 10, H("04 83 e0 83 06 84 06 84 e0 00"), True),
    ("einfuegen", "Einfuegen (Strg+V)", 10, H("04 83 e0 83 19 84 19 84 e0 00"), True),
    ("neu", "Neu (Strg+N)", 10, _combo(S, KEYS["n"]), True),
    ("schliessen", "Schliessen (Strg+W)", 10, _combo(S, KEYS["w"]), True),
    ("oeffnen", "Oeffnen (Strg+O)", 10, _combo(S, KEYS["o"]), True),
    ("minimieren", "Minimieren", 14, _window_menu(KEYS["n"]), True),
    ("maximieren", "Maximieren", 14, _window_menu(KEYS["x"]), True),
    ("zoom-rein", "Zoom (Original: rein, sendet Strg+Rad runter)", 11, H("04 83 e0 88 0a 7f 88 0a 84 e0 00"), True),
    ("zoom-raus", "Zoom (Original: raus, sendet Strg+Rad hoch)", 11, H("04 83 e0 88 0a 61 88 0a 84 e0 00"), True),
    ("programm-schliessen", "Programm schliessen (Alt+F4)", 10, _combo(KEYS["alt"], KEYS["f4"]), True),
    ("lauter", "Lauter", 8, _consumer(0x00E9), True),
    ("leiser", "Leiser", 8, _consumer(0x00EA), True),
    ("stumm", "Stumm", 8, _consumer(0x00E2), True),
    ("shift", "Shift", 6, _hold_key(C), True),
    ("strg", "Strg", 6, H("06 83 e0 ff ff 00"), True),
    ("alt", "Alt", 6, _hold_key(KEYS["alt"]), True),
    ("enter", "Enter", 6, _hold_key(KEYS["enter"]), True),
    ("lupe", "Lupe (Win+U)", 10, H("04 83 e3 83 18 84 18 84 e3 00"), True),
    ("desktop", "Desktop (Win+D)", 10, _combo(W, KEYS["d"]), True),
    ("internet", "Internet / Browser", 8, _consumer(0x0196), True),
    ("favoriten", "Browser-Favoriten", 8, _consumer(0x022A), True),
    ("vorheriges-fenster", "Vorheriges Fenster (Alt+Tab)", 10, _combo(KEYS["alt"], KEYS["tab"]), True),
    ("email", "E-Mail", 8, _consumer(0x018A), True),
    ("play-pause", "Play/Pause", 8, _consumer(0x00CD), True),
    ("zurueck", "Vorheriger Titel", 8, _consumer(0x00B6), True),
    ("weiter", "Naechster Titel", 8, _consumer(0x00B5), True),
    ("speed-2x", "2x Tempo (solange gehalten)", 2, H("c8 00"), True),
    ("speed-4x", "4x Tempo (solange gehalten)", 2, H("ca 00"), True),
    ("speed-8x", "8x Tempo (solange gehalten)", 2, H("ce 00"), True),
    # records from the original's code (0x423630), tested on the device 2026-10-11 (round 6)
    ("nur-senkrecht", "Nur senkrecht bewegen (solange gehalten)", 2, H("c6 00"), True),
    ("nur-waagerecht", "Nur waagerecht bewegen (solange gehalten)", 2, H("c7 00"), True),
    # Strg+Alt+K = hotkey of the original's autostart GHOSTOPEN.exe, which opens Macro.exe
    ("gigabyte-ghost", "Gigabyte-GHOST-Software oeffnen (Strg+Alt+K)", 14, _combo(S, KEYS["alt"], KEYS["k"]), True),
]


def _build():
    out, addr = [], FUNCTION_BASE
    for i, (key, label, size, record, confirmed) in enumerate(_TABLE):
        assert not record or len(record) == size, key
        assert size or not record, key
        out.append(Function(i, key, label, size, record, confirmed, addr))
        addr += size
    return out


FUNCTIONS = _build()
FUNCTIONS_BY_KEY = {f.key: f for f in FUNCTIONS}
FUNCTIONS_BY_KEY["perfect"] = FUNCTIONS_BY_KEY["rechner"]      # old CLI name
FUNCTIONS_BY_ADDRESS = {f.address: f for f in FUNCTIONS if f.size}

# Wheel up/down (device buttons 8/9) have no release, so the original (0x423630) gives the five
# mouse-button functions a plain click instead of the hold record: [ListMemSize] 46-50, after the list.
WHEEL_BUTTONS = (8, 9)
WHEEL_CLICKS = {}
_addr = FUNCTIONS[-1].address + FUNCTIONS[-1].size
for _key, _mask in (("linksklick", 0x01), ("rechtsklick", 0x02), ("mittelklick", 0x04),
                    ("browser-vor", 0x10), ("browser-zurueck", 0x08)):
    WHEEL_CLICKS[_key] = (_addr, bytes([MODE_ONCE, EV_MOUSE_DOWN | _mask, EV_MOUSE_UP | _mask, EV_END]))
    FUNCTIONS_BY_ADDRESS[_addr] = FUNCTIONS_BY_KEY[_key]
    _addr += 4


def placement(function, button):
    """(address, record) the original writes when this function goes on this device button."""
    if button in WHEEL_BUTTONS and function.key in WHEEL_CLICKS:
        return WHEEL_CLICKS[function.key]
    return function.address, function.record


# -- macro events <-> text ------------------------------------------------------------------
def encode_events(text):
    """'drueck a, warte 100, los a, klick links, tippe strg+c' -> event bytes (without mode/end)."""
    out = bytearray()
    for raw in text.replace("\n", ",").split(","):
        words = raw.strip().lower().split()
        if not words:
            continue
        cmd, arg = words[0], " ".join(words[1:])
        if cmd in ("drueck", "los", "tippe"):
            keys = [_key(k) for k in arg.split("+")]
            if cmd != "los":
                for k in keys:
                    out += bytes([EV_KEY_DOWN, k])
            if cmd != "drueck":
                for k in reversed(keys):
                    out += bytes([EV_KEY_UP, k])
        elif cmd == "warte":
            ms = int(arg)
            if not 1 <= ms <= 0xFFFF:
                raise ValueError("Wartezeit 1-65535 ms")
            out += bytes([EV_WAIT8, ms]) if ms <= 0xFF else bytes([EV_WAIT16, ms & 0xFF, ms >> 8])
        elif cmd == "rad":
            parts = arg.split()
            if not parts or parts[0] not in ("hoch", "runter"):
                raise ValueError("rad hoch|runter [1-15]")
            n = int(parts[1]) if len(parts) > 1 else 1
            if not 1 <= n <= 15:
                raise ValueError("Rad-Schritte 1-15")
            out += bytes([EV_WHEEL_UP if parts[0] == "hoch" else EV_WHEEL_DOWN]) * n   # one event per notch
        elif cmd == "consumer":
            usage = 0 if arg == "los" else int(arg, 16)
            if not 0 <= usage <= 0xFFFF:
                raise ValueError("consumer 0x0000-0xFFFF oder 'consumer los'")
            out += bytes([EV_CONSUMER, usage & 0xFF, usage >> 8])
        elif cmd in ("maus-drueck", "maus-los", "klick"):
            if arg not in MOUSE_BUTTONS:
                raise ValueError(f"Maustaste: {', '.join(MOUSE_BUTTONS)}")
            m = MOUSE_BUTTONS[arg]
            if cmd != "maus-los":
                out.append(EV_MOUSE_DOWN | m)
            if cmd != "maus-drueck":
                out.append(EV_MOUSE_UP | m)
        else:
            raise ValueError(f"unbekannter Befehl '{cmd}' (drueck, los, tippe, warte, maus-drueck, maus-los, klick, rad, consumer)")
    return bytes(out)


def _key(name):
    if name not in KEYS:
        raise ValueError(f"unbekannte Taste '{name}'")
    return KEYS[name]


EVENT_SIZES = {EV_KEY_DOWN: 2, EV_KEY_UP: 2, EV_WAIT8: 2, EV_CONSUMER: 3, EV_WAIT16: 3}   # others: 1 byte


def record_length(data):
    """Bytes used by a record/macro including its 00 end marker; 0 for an empty slot.
    Walks the events, because 00 also appears inside event arguments (e.g. 85 E9 00)."""
    if not data or data[0] in (0x00, 0xFF):
        return 0
    i = 1
    while i < len(data) and data[i] != EV_END:
        i += EVENT_SIZES.get(data[i], 1)
    return min(i + 1, len(data))


def decode_record(data):
    """Return (mode_name, [event strings]) for a function record or macro; stops at 00."""
    if not data or data[0] in (0x00, 0xFF):        # erased or zeroed slot
        return None, []
    mode = {v: k for k, v in MODE_NAMES.items()}.get(data[0], f"0x{data[0]:02X}")
    events, i = [], 1
    while i < len(data) and data[i] != EV_END:
        b = data[i]
        if b in (EV_KEY_DOWN, EV_KEY_UP) and i + 1 < len(data):
            k = data[i + 1]
            events.append(f"{'drueck' if b == EV_KEY_DOWN else 'los'} {KEY_NAMES.get(k, f'0x{k:02X}')}")
            i += 2
        elif b == EV_CONSUMER and i + 2 < len(data):
            u = data[i + 1] | data[i + 2] << 8
            events.append(f"consumer 0x{u:04X}" if u else "consumer los")
            i += 3
        elif b == EV_WAIT8 and i + 1 < len(data):
            events.append(f"warte {data[i + 1]}")
            i += 2
        elif b == EV_WAIT16 and i + 2 < len(data):
            events.append(f"warte {data[i + 1] | data[i + 2] << 8}")
            i += 3
        elif b & 0xE0 == EV_WHEEL and b & 0x1F:
            d = b & 0x1F
            d = d - 32 if d & 0x10 else d
            direction = "hoch" if d > 0 else "runter"
            if events and events[-1].startswith(f"rad {direction} "):     # notches in a row -> one line
                d += int(events.pop().split()[-1]) * (1 if d > 0 else -1)
            events.append(f"rad {direction} {abs(d)}")
            i += 1
        elif b & 0xE0 in (EV_MOUSE_DOWN, EV_MOUSE_UP) and b & 0x1F:
            names = [n for n, m in MOUSE_BUTTONS.items() if b & m] or [f"0x{b & 0x1F:02X}"]
            events.append(f"{'maus-drueck' if b & 0xE0 == EV_MOUSE_DOWN else 'maus-los'} {'+'.join(names)}")
            i += 1
        else:
            events.append(f"0x{b:02X}?")
            i += 1
    return mode, events
