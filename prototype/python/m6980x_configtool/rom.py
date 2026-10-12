"""Layout of the 8 KB onboard ROM (see docs/PROTOCOL.md; computed the same way by the original, 0x42ebd8).

  0x0000-0x1CBF  20 macro slots of 0x170 bytes (slot n at n * 0x170)
  0x1CC1-0x1D88  macro info table, 20 entries of 10 bytes (MacroInfoSize=10)
  0x1D8C-0x1EEF  function records at fixed addresses, incl. the wheel-button clicks (see functions.py)
  0x1F00-0x1FFF  config maintained by the firmware: 0x1F00 = marker 0x12; button flags (80 assigned / 00 default)
                 at BUTTON_FLAGS + profile * 12 + button; button addresses at
                 BUTTON_TABLE + profile * 0x18 + button * 2 (stale after a reset to default)
"""

MACRO_SLOTS = 20
MACRO_SLOT_SIZE = 0x170
MACRO_INFO_BASE = 0x1CC1
MACRO_INFO_SIZE = 10
BUTTON_FLAGS = 0x1F01
BUTTON_FLAGS_STRIDE = 12
BUTTON_TABLE = 0x1F3D
BUTTON_TABLE_STRIDE = 0x18
MACRO_NAME_LEN = 9          # name bytes; byte 9 of the entry is the picture byte
# Picture byte (Macro.exe 0x4133a0): new macros get slot + 1 (no picture, the GUI shows "MACRO n"),
# the "Change Icon" dialog writes 100 + n for picture n of its 100-entry grid. The grid's image list
# (Form3.ImageList1) holds 40 pictures embedded in Macro.exe at index 1-40 (index 0 = background),
# followed at runtime by bmp\1.bmp ... bmp\60.bmp (empty placeholders for own pictures) at index 41-100.
# Confirmed: Tom's "Klicker" (0x68) shows Form3.ImageList1 index 4.
ICON_OFFSET = 100
ICON_COUNT = 100
ICONS_EMBEDDED = 40


def macro_icon(info):
    """Picture number 1-100 chosen for a macro, or None (no picture / default)."""
    value = info[MACRO_NAME_LEN] if len(info) > MACRO_NAME_LEN else 0xFF
    n = value - ICON_OFFSET
    return n if 1 <= n <= ICON_COUNT else None


def icon_source(n):
    """Where picture n comes from: embedded picture (assets/embedded_icons/TFORM3_ImageList1/<n>.bmp)
    or the user's own file bmp\\<n-40>.bmp in the original program folder."""
    if 1 <= n <= ICONS_EMBEDDED:
        return "eingebaut"
    return f"eigenes bmp\\{n - ICONS_EMBEDDED}.bmp"


def button_assignment(rom, profile, button):
    """Record address a button points to in a ROM image, or None if its flag says default (00).
    The table entry alone is not enough: the firmware leaves it stale after a reset to default."""
    if not rom[BUTTON_FLAGS + profile * BUTTON_FLAGS_STRIDE + button] & 0x80:
        return None
    a = BUTTON_TABLE + profile * BUTTON_TABLE_STRIDE + button * 2
    return rom[a] | rom[a + 1] << 8


def macro_slot_addr(slot):
    return slot * MACRO_SLOT_SIZE


def macro_info(rom, slot):
    start = MACRO_INFO_BASE + slot * MACRO_INFO_SIZE
    return rom[start:start + MACRO_INFO_SIZE]


def printable(data):
    return "".join(chr(c) if 32 <= c < 127 else "." for c in data)


def is_erased(data):
    return all(c == 0xFF for c in data)


def macro_slot_of(addr):
    if addr < MACRO_SLOTS * MACRO_SLOT_SIZE and addr % MACRO_SLOT_SIZE == 0:
        return addr // MACRO_SLOT_SIZE
    return None


def macro_name(info):
    """Name part of an info entry; erased (FF) entries have no name."""
    raw = info[:MACRO_NAME_LEN]
    if is_erased(raw):
        return ""
    return raw.split(bytes([0]))[0].decode("latin-1")
