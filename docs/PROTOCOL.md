# Gigabyte GHOST M6980X – USB protocol

Sources: disassembly of `Macro.exe` from `GHOST(6980X).msi` (version 1.01, from `6980X101.exe`),
read tests on the real mouse (2026-10-11) and USBPcap captures of the original software
(`GHOST_ReverseEngineering/ghost_init.pcapng`). All statements about the code were cross-checked with Ghidra 12.1.4
(via REA), see the section "Cross-check with Ghidra" at the end.

Levels of evidence:
- ✅ **confirmed** – observed on the device or in a capture and consistent with the code
- 🔍 **from the code** – read in the original program, not yet tried on the device
- ❓ **assumption** – plausible but open

## Device

| | |
|---|---|
| USB ID | `E0FF:0001`, product string `G3` ✅ |
| Config channel | feature report **without report ID** on interface 0 (mouse), 8 data bytes ✅ |
| Report descriptor | `09 00 15 80 25 7F 75 08 95 08 B1 02` (end of the mouse collection) ✅ |
| Linux | `/dev/hidrawN` of interface 0, `HIDIOCSFEATURE(9)` / `HIDIOCGFEATURE(9)` 🔍 |

Interface 1 (vendor page `FF03`, report ID 1) is **not** used by `Macro.exe` ✅.

## Packet format and handshake ✅

Sent (without report ID): `[group, command, arg0 … arg5]`; for many commands the last byte is `0x12`.

Reply via GET_FEATURE: `[status, data0 … data6]`

| Status bit | Meaning |
|---|---|
| `0x80` | busy – do not send a new command yet |
| `0x40` | reply ready |
| `0x3F` | group of the reply (`0x41` = group 1) |

Sequence as in the original (`0x44494C`, during device detection the copy `0x444AAC`): GET until the busy bit is
clear (max. 20 × 4 ms) → SET → optional wait (write commands 10 ms) → GET until the done bit is set (max. 200 × 4 ms).
For a wait above 4 ms the original additionally sleeps 1 ms before the SET and at the end. `0x44494C`
opens the device for every command (`CreateFileA`) and closes it afterwards.

**Caution:** bytes a command does not fill contain leftovers of earlier replies. Only evaluate documented
bytes. This also applies when sending: the original never sets the bytes between the arguments and the final
`12`. They contain stack content or (for group 2) the previous reply, because send and receive buffer are the
same. The `00` in the tables below are therefore a convention; `m6980x_configtool` sends zeros there.

## Group 0 – system

| Command | Send | Reply | Level |
|---|---|---|---|
| Identify | `00 00` | `0C 12 09 2D 00 03 01` – the original checks `0C 12` (`0x44407C`). It does **not** look for the mouse by VID/PID but takes every HID device with a 9-byte feature report length that answers like this | ✅ |
| Name | `00 01` | ASCII, `G3` | ✅ |
| Session | `00 02 <counter>` | counter starts at 3, +1 mod 254. The original sends it at startup. **Not** needed for switching profiles (tested) | 🔍 |

**Model detection of the original** (`0x42EBD8`): it does not look for USB IDs but takes every HID device with a
9-byte feature report that answers `00 00` with `0C 12`. The model follows from data bytes 6 and 2 of the reply:

| Byte 6 | Byte 2 | Model | Memory / profiles / macros / info size | DPI max (step) |
|---|---|---|---|---|
| 0 | – | 6980 | 8 KB / 3 / 20 / 10 | 3000 (100) |
| 1 | 9 | **6980X** (Tom's mouse: `0C 12 09 2D 00 03 01`) | 8 KB / 3 / 20 / 10 | 6030 (90) |
| 1 | ≠ 9 | M8000X | 32 KB / 5 / 70 / 14 | 6030 (90) |
| 4 | – | M8600 | 32 KB / 5 / 70 / 14 | 6030 (90) |
| other | | ignored | | |

Values from `Mappingdefault.ini`. The USB IDs of the other models appear nowhere in the software.
`m6980x_configtool` looks for `E0FF:0001`, interface 0 only (so it never sends commands to other devices), and since
2026-10-12 also checks the model: all commands except `info` stop if it is not "6980X", because the memory layout of
the other models differs. `info` shows the detected model.

## Group 1 – registers (command `N` = read, `N | 0x80` = write)

| Reg | Read | Meaning | Write (original) | Level |
|---|---|---|---|---|
| `02` | `01 02 00` | unknown | – | ❓ |
| `03` | `01 03 00 <stage>` → `X step, Y step (0 = like X), unit` | DPI stage 0–3 (by button only 0–2), unit 90 (the mouse reports 0, the original computes with 100, `0x4457FC`). **Shared by all profiles** (tested on the device) | `01 83 <0=X/1=Y> <stage> <sensor step> 00 00 12` | ✅ read and write |
| `03` | `01 03 01` → 7-byte bit mask | supported steps 1–56 | – | ✅ |
| `04` | `01 04 00 <stage>` → `Xn, Xd, Yn, Yd` | fine scaling: DPI = step × 90 × n / d | `01 84 <0=X/1=Y> <stage> <n> <d> 00 12` | ✅ read and write |
| `05` | `01 05 <profile> <button>` → `flags, value lo, value hi` | button assignment. Flags `0x80` = assigned, value = ROM address of the action record. GUI buttons 3 and 4 are swapped on the device (the original swaps them in `0x445398`) | `01 85 <profile> <button> <type> <lo> <hi> 12` | ✅ read / 🔍 write |
| `06` | `01 06 <profile>` → `R, G, B` | colour of the RGB LEDs per profile, per channel 0 = off, 1 = medium, 2 = full. The number of lit LEDs (1–3) shows the DPI stage. Set and confirmed on the device (red = `2 0 0`), does not change the 8 KB ROM | `01 86 <profile> <R> <G> <B>` | ✅ |
| `07` | `01 07` | active DPI stage from 0. The DPI button cycles 0 → 1 → 2 → 0, stage + 1 LEDs are lit (observed on the device). The button never reaches stage 3 (4th entry in reg 03/04). The stage is kept when switching profiles | `01 87 <stage> 00 00 00 00 12` | ✅ read and write |
| `08` | `01 08` | active profile 0–2. The profile button cycles 0 → 1 → 2 → 0 (observed) | `01 88 <profile> 00 00 00 00 12` – ✅ tested on the device: without init sequence, ~11 ms per switch, does not change the 8 KB ROM | ✅ |
| `0A` | `01 0A` → 16 bit | `0x1F00`, address of the config area. At startup the original reads the ROM from this address (`0x431535`). If the first byte is not `0x12`, it writes `02 01 00 1F 00 12` | – | 🔍 code / ✅ ROM dump has `12` |
| `0B` | `01 0B` | **USB polling interval in ms** (1–8, report rate = 1000 / value Hz). Shared by all profiles | `01 8B <value>` | ✅ capture: 500 → 333 Hz = `01 8B 02` → `01 8B 03` |

## Group 2 – ROM (8 KB, mirrored from 0x2000) ✅

| Command | Send | Meaning | Level |
|---|---|---|---|
| Read | `02 00 <lo> <hi> 00 00 00 12` | reply: 7 bytes from the address | ✅ |
| Write 1 byte | `02 01 <lo> <hi> 00 <value> – 12` | | 🔍 |
| Write 2 bytes | `02 02 <lo> <hi> 00 <v0> <v1> 12` | | 🔍 |
| Fill buffer | `02 03` / `05` / `06` / `07` / `08` / `09` + 6 bytes each (09: 2 bytes) | buffer bytes 0–5, 6–11, … 30–31 | 🔍 |
| Write buffer | `02 04 <lo> <hi> 00 <length> – 12` | n bytes (≤ 32) to the address | 🔍 |
| Write page | `02 0A <lo> <hi> 00 – – 12` | 32 bytes from the buffer, wait 5 ms (`0x444F44`) | 🔍 |

The original's write function (`0x444C30`) chooses by length: 1 byte → `02 01`, 2 bytes → `02 02` (+1 ms),
otherwise buffer + `02 04`. That is why single `02 01` commands appear when writing macros. The buffer commands
wait 1 ms each, `02 09` 0 ms.

## Group 5

`05 00 0C 12` → reply byte 1 = `03`. Meaning unknown ❓

## ROM layout (from the dump and the startup reads)

| Range | Content | Level |
|---|---|---|
| `0x0000–0x1CBF` | 20 macro slots of `0x170` bytes each | ✅ addresses / ❓ format |
| `0x1CC1–0x1D88` | macro info table, 20 × 10 bytes, entry 0 = "Klicker" | ✅ addresses / ❓ format |
| `0x1D89–0x1D8B` | 3 unused bytes (gap in the code: end of info + 3) | 🔍 |
| `0x1D8C–0x1EEF` | 51 function records, sizes from `[ListMemSize]`, see "Memory layout from the code" | 🔍 |
| `0x1F00` | marker `0x12` (checked by the original and written if necessary), see register `0A` | 🔍 |
| `0x1F01–0x1F24` | button flags: `0x1F01 + profile × 12 + button` = `80` assigned / `00` default (12 bytes per profile, 10 buttons + 2 free) | ✅ |
| `0x1F3D–0x1F84` | button table: `0x1F3D + profile × 0x18 + button × 2` = address of the record | ✅ |
| rest of the config area up to `0x1FFF` | unknown | ❓ |

Macro events (preliminary): `83 xx` press key, `84 xx` release key (xx = HID usage, e.g.
`E0` = left Ctrl, `19` = V), `85 lo hi` consumer key (`00E9` = volume up, `00EA` = volume down), `00` = end.

## Model values (`Mappingdefault.ini`, section 6980)

Memory 8192 · profiles 3 · buttons 10 · macros 20 · MacroInfoSize 10 ·
DPI defaults 810 / 1620 / 3240 / 6030 · step 90 · maximum 6030

## Corrections to the older documentation (`GHOST_Mouse_Protocol.md`)

- The "flush" is the busy/done handshake. Without checking the status bits, timing problems occur.
- The "init sequence" is the recorded startup of the original software (read commands). At its end there are
  write commands: DPI stage 3 = 5850, register 0B = 2, profile 1 active. Replaying it overwrites these settings.
- `02 00 <value>` is a ROM read, not a write of the report rate. `01 08` is the active profile.
- The "commit commands" are `01 0B` (read) and `01 8B 02` (set register 0B to a fixed 2).

## Behaviour while a button is pressed ✅

While the DPI or profile button is pressed, GET_FEATURE occasionally returns a shortened
report (1 byte). The original counts a failed GET_FEATURE as "not ready yet" and polls again.
`m6980x_configtool` does the same.

## Writing DPI (port of `0x445900`) ✅

Wanted value in steps `w = DPI / 90`. Per axis:
1. Sensor step `s` = smallest supported step ≥ `w`, otherwise the largest one (mask from `01 03 01`, here 1–56).
2. Fraction `w / s`, reduced.
3. Send: `01 83 X`, `01 83 Y` (sensor steps), then `01 84 X`, `01 84 Y` (fractions), 10 ms each.

Special case in the original: if X and Y are not set separately (flag `+0x898` = 0, set when reading if the mouse
reports Y step 0), both axes take the sensor step of max(X, Y). For equal X and Y this makes no difference.

Example 5850 DPI: `w` = 65 → `s` = 56, fraction 65/56. This is byte-identical with the capture of the original
software. Tested on the device: stage 4 to 4500 and back to 6030, raw values identical afterwards. The 8 KB ROM
stays unchanged by profile, LED and DPI changes. These values are therefore stored elsewhere, presumably in the
internal memory of the microcontroller.

## Capture with the original software (2026-10-11, `captures/original_software.pcapng`) ✅

Tom changed the report rate 500 → 333 Hz, put the front side button in profile 1 on "Ctrl" and created
macro 3 "Hallo-Wel" (press and hold T), one after the other.

**Software startup:** as in the old capture, only read commands. After that it rewrites DPI stage 1,
first with a wrong Y step `01` (90 DPI), then corrected. This is a bug of the original software.

**Report rate:** `01 8B 03`.

**Put a button on a keyboard key** (GUI button 3 = device button 4):
```
02 03 06 83 E0 FF FF 00      buffer[0..5] = 06 83 E0 FF FF 00   (record: 06 = key assignment, 83 E0 = Ctrl)
02 04 64 1E 00 06 .. 12      write 6 bytes to ROM 0x1E64
01 85 00 04 80 64 1E 12      profile 0, device button 4, flags 0x80, record 0x1E64
01 88 00 ...                 re-activate profile 0
```
The firmware also enters the address into the button table from ROM 0x1F00 (0x1F45 = `64 1E`).

**Macro** (slot 2 = "Macro 3", address 0x02E0):
```
02 01 E0 02 00 04 ..  12     0x02E0 = 04   (macro record)
02 01 E1 02 00 83 ..  12     0x02E1 = 83   (press key)
02 01 E2 02 00 17 ..  12     0x02E2 = 17   (HID usage "T")
02 01 E3 02 00 00 ..  12     0x02E3 = 00   (end, no release = hold)
02 01 DE 1C 00 03 ..  12     info entry slot 2, byte 9 = 03
02 03 'Hallo-' / 02 05 'Wel' + 3 zero bytes / 02 04 D5 1C 00 09   name (max. 9 characters) to 0x1CD5
```
Info entry = 9 bytes name + 1 byte. For slots 1–4 the last byte equals slot + 1, for slots 0 and 5 it does not
(0x68, 0x79) ❓ (resolved below: it is the icon byte).

**Record types (first byte):** `04` = event sequence (macro), `06` = key assignment with key,
followed by events `83 xx` press, `84 xx` release, `85 lo hi` consumer key, `00` end.
The meaning of `21 41`, `24 89` and `88 0A` was still open at this point ❓ (resolved below).

## Capture 2 and function table ✅

**Function records live at fixed addresses.** `Mappingdefault.ini` (copy in `docs/`) contains
46 functions in `[List]` and their size in `[ListMemSize]`. Function *i* lives at
`0x1D8C + sum(size[0..i-1])`. Confirmed for middle click 0x1D98, Ctrl+C 0x1DD8, Ctrl+V 0x1DE2,
volume up 0x1E46 and Ctrl 0x1E64. When assigning, the original software (re)writes the record to
this address and then sets `01 85 <profile> <device button> 80 <address>`. A macro is assigned the same way,
only with the slot address (`80 E0 02` = slot 2).

**Record/macro format:** byte 0 = mode, then events, `00` = end.

| Byte 0 | Mode | Evidence |
|---|---|---|
| `04` | run once | ✅ macro "XY" |
| `05` | loop | ✅ macro "XY" |
| `06` | fire button / while held | ✅ macro "XY", key assignment Ctrl, middle click |

| Event | Meaning | Evidence |
|---|---|---|
| `83 uu` / `84 uu` | press / release key, HID usage `uu` (`04`=A, `06`=C, `17`=T, `19`=V, `E0`=Ctrl, `E3`=Win) | ✅ |
| `85 lo hi` | consumer key (`00E9` volume up, `00EA` volume down, `0000` = release) | ✅ |
| `88 nn` | wait `nn` ms | ✅ 100 ms = `88 64` |
| `89 lo hi` | wait, 16-bit ms | ✅ 500 ms = `89 F4 01` |
| `2m` / `4m` | press and hold / release mouse button, mask `m` (1 = left, 2 = right ❓, 4 = middle) | ✅ double click `21 41 21 41`; in macro "XY" according to Tom `21` = press and hold left, `24` = press and hold middle |
| `7F` / `61` | mouse wheel (zoom in / zoom out with Ctrl) | ❓ direction |

**Macro "XY"** in slot 18 (address 0x19E0): `04 83 04 88 64 84 04 89 F4 01 21 24 00`, info byte 9 = `13`
(= slot + 1), name "XY". Changing the mode overwrites the whole macro with byte 0 = 05 or 06.

**Device buttons** ✅ (found on the device: in profile 3 every button typed its own number via a macro):

| No. | Button | "Default" sends to the PC ✅ |
|---|---|---|
| 0 | left | mouse button 1 (left click) |
| 1 | right | mouse button 2 (right click) |
| 2 | wheel press | mouse button 3 (middle click) |
| 3 | rear side button | mouse button 4, bit `0x08` ("back") |
| 4 | front side button | mouse button 5, bit `0x10` ("forward") |
| 5 | – (no button triggered, "High" in the INI) | – |
| 6 | tilt wheel left | horizontal scroll −1 (AC Pan, byte 6), one step per press |
| 7 | tilt wheel right | horizontal scroll +1 |
| 8 | wheel up | wheel +1 per notch (byte 5) |
| 9 | wheel down | wheel −1 per notch |

The default column was measured from the input reports (`captures/standard_tasten*.pcapng`, 2026-10-11).
Since then CLI and GUI show "default" with this effect, e.g. "Default (Back, mouse button 4)".

The DPI and profile buttons cannot be reassigned. The GUI of the original counts differently (side buttons swapped).
`m6980x_configtool` uses the device numbers. The `macro` path (write 10 macros, read back, assign) was confirmed on
the device as well.

**Report rate:** at startup the software writes the current value `01 8B 03` back.

## Restoring with `m6980x_configtool` (2026-10-11) ✅

Confirmed on the device with this tool:
- `01 8B 02` → report rate 500 Hz (read back).
- `01 85 <profile> <button> 80 <lo> <hi> 12` → button on a macro or function (read back).
- `01 85 <profile> <button> 00 FF FF 12` → **button back to default** (read back: flags 00). The original software
  does not offer this.
- ROM writes via buffer (`02 03`/`05` + `02 04`) and single byte (`02 01`), several ranges, each read back.
  Afterwards the 8 KB ROM was byte-identical with the backup again (SHA-256 44d6fbae…).

Button table of the firmware: `0x1F3D + profile × 0x18 + button × 2` (2-byte address per button, confirmed for
profiles 0 and 2). When resetting to default, the old address stays there but is no longer active. The flags are at
`0x1F01 + profile × 12 + button` (`80` = assigned, `00` = default), exactly what `01 05` reports as the flags byte.
Confirmed on 2026-10-11 for all 30 buttons after Tom set the rear side button in profiles 2 and 3 to default:
0x1F10 and 0x1F1C changed from `80` to `00`, the addresses in the table stayed.

## Derived functions tested on the device (2026-10-11) ✅

All 14 records derived from the format work (assigned in profile 3, tried by Tom, ROM restored byte-identically
afterwards): left click `06 21 89 FF FF 00`, right click `06 22 …`, Ctrl+Z/X/N/W/O following the pattern
`04 83 E0 83 uu 84 uu 84 E0 00`, mute/play-pause/previous/next track as consumer `04 85 lo hi 85 00 00 00`
(`00E2`, `00CD`, `00B6`, `00B5`), Shift/Alt/Enter as `06 83 uu FF FF 00`.
This also confirms mask 2 = right mouse button.

**Keyboard layout:** HID usages are key positions of a US keyboard. `m6980x_configtool` names keys after the
German QWERTZ layout (Y and Z swapped). Undo therefore sends usage `1C` and worked under German Windows.
At this point 16 functions were still unknown (browser forward/back, scrolling, minimize/maximize,
close program, internet/favourites/e-mail, previous window, 2×/4×/8× speed).

## More functions tested on the device (2026-10-11) ✅

Derived from the record size and tried by Tom: browser back/forward (mouse button mask 8 / 16),
scroll up/down (`04 61 00` / `04 7F 00`), minimize/maximize (Alt+Space, then N or X, window menu),
close program (Alt+F4), previous window (Alt+Tab), internet (consumer `0196`),
favourites (`022A`) and e-mail (`018A`).

**Mouse wheel event:** `0x60 | delta` with a signed 5-bit delta: `61` = 1 notch up, `7F` = 1 notch down
(observed on the device, values above 1 are untested). Consequence: the original records "Zoom In"
(`… 7F …` = Ctrl+wheel down) and "Zoom Out" (`… 61 …`) are swapped; in most programs "Zoom In" zooms out.

**Still unknown at this point:** scroll left/right (size 0 – no record) and 2×/4×/8× speed (size 2).

## Capture 3: the last 5 functions (2026-10-11) ✅

- **2×/4×/8× speed:** 2-byte records `C8 00` (0x1EC4), `CA 00` (0x1EC6), `CE 00` (0x1EC8),
  written with the 2-byte command `02 02 <lo> <hi> 00 <b0> <b1> 12`, then `01 85 02 03 80 <lo> <hi> 12`.
  Effect according to the manual: while the button is held, the mouse moves 2×/4×/8× as fast;
  releasing ends the function. Written with `m6980x_configtool` and confirmed on the device (2×, 4×, 8×).
- **Scroll left/right:** no record (size 0). The software only offers them on the tilt wheel itself
  and then writes `01 85 <profile> <button> 00 FF FF 12`. This is byte-identical with the "default" of
  `m6980x_configtool`.
- Incidentally: "scroll back" was written as `04 7F 00`. This confirms the wheel direction from the manufacturer's view.

With that, 43 functions were known. The remaining three (43–45) are in the code of the original, see "Function
records from the code" at the end.

## Macro preview icons ✅ (code + ROM)

Byte 9 of the macro info entry (`0x1CC1 + slot × 10 + 9`) is the **icon byte**, not "slot + 1":
- New macro (`ListBox2DragDrop`, call of `0x4133A0`): value = slot + 1 → no icon chosen.
- "Change Icon" (`Image111Click` → `0x4133A0`): value = 100 + n for icon n of the 100-icon picker.
- It is drawn with image list index = value − 100 (`ListBox2DrawItem`, 0x40FB61) from the image list of the
  picker (`TForm3.ImageList1`). It contains **41 images embedded in Macro.exe** (index 0 =
  background pattern, 1–40 = symbols such as binoculars, skull, pistol). At runtime 0x43EAF0 appends
  `bmp\1.bmp` … `bmp\60.bmp` as indices 41–100. These 60 files are shipped as **empty placeholders**
  for custom images ("Change Icon" copies custom images there via `temp.bmp`).
- Without a chosen icon (value = slot + 1) the software shows "MACRO n" from `TForm1.ImageList2`.
- ✅ Confirmed by Tom: "Klicker" (0x68 → index 4) had the image `TForm3.ImageList1` index 4 (skull).
  The old entry in the empty slot 6 (0x79) was index 21. Tom removed the icon; afterwards it read `06`
  (= slot + 1), and that was the only change in the ROM.
- Extracted images: `assets/embedded_icons/` (`re_tools/imagelist_extract.py`, overview `uebersicht.html`; not in
  the repository).

`m6980x_configtool macro` keeps a chosen icon when overwriting, `--icon N` sets one, and
`m6980x_configtool macro-icon SLOT N|none` changes only the icon.

## Memory limits for macros

20 slots of a fixed size of 368 bytes each (0x170), no sharing between slots. Per slot, 366 bytes are usable for
events (mode byte + end byte are subtracted). Event sizes: press/release key 2, wait 2 (≤ 255 ms) or 3,
consumer 3, mouse button and wheel 1. A complete key stroke needs 4 bytes.
The name has at most 9 characters. The original editor internally has `Struct_Macro[100]`, so presumably at most
100 events per macro ❓ (refuted below: the editor stops at the slot size).

## Report rate measured and register 02 (2026-10-11) ✅

`re_tools/rate_test.py` counts the mouse's interrupt reports via USBPcap while it is moved:

| State | Median interval | Rate |
|---|---|---|
| register 0B = 2 | 2.00 ms | 500 Hz |
| register 0B = 8, without `01 02` | 8.00 ms | 125 Hz |
| then `01 02 00 … 12` | 8.00 ms | 125 Hz |
| register 0B = 2 + `01 02` | 2.00 ms | 500 Hz |

- **Register 0B takes effect immediately.** The USB descriptor always reports `bInterval = 1` regardless.
- **Group 5** `05 00 0C 12` → 3 is the **number of profiles** (the code uses it as the loop limit
  over the 164-byte profile structures).
- **Register 02** (`01 02 00 … 12`, 4 ms, `0x445C5C`): the original software only sends it when leaving the
  settings page (`0x44B3BC`) if the report rate was changed or the default button was pressed
  (flag `+0x3AC`). Immediate reply, no effect on the rate.
  Assumption ❓: save permanently (settings outside the 8 KB ROM). Confirmed in the next section.

## Register 02 = save permanently ✅ (unplug tests 2026-10-11)

| Changed without `01 02` | after unplugging and replugging | with `01 02` |
|---|---|---|
| LED colours (reg 06) | kept | kept |
| report rate (reg 0B) | lost (back to 500 Hz) | kept |
| DPI values (reg 03/04) | lost | kept |
| active profile (reg 08) | back to profile 1 | kept |
| active DPI stage (reg 07) | back to stage 1 | kept |

It is stored in the internal memory of the microcontroller; the 8 KB ROM stays unchanged.
`m6980x_configtool` saves automatically after `rate`, `dpi set/select`, `profile` and `restore` (can be turned off
with `--temporary`). The init sequence of the old script and of the OpenRGB fork never sends `01 02`. Their DPI and
report rate changes are therefore lost when the mouse is unplugged.

## Capture 4: backup, restore, default, release, editor limit (2026-10-11) ✅

| Action of the original software | Commands |
|---|---|
| Backup to file | 1171 × `02 00` (whole ROM in 7-byte pieces), approx. 75 s |
| Restore | all 256 pages 0x0000–0x1FE0: `02 03`/`05`/`06`/`07`/`08`/`09` (buffer) + `02 0A <lo> <hi> 00 … 12`, approx. 35 s |
| "Default" | `01 85 <profile> <button> 00 00 00 12` for the 10 buttons of a profile |
| "Release" | byte 0 of each of the 20 macro info entries = 00 (empty name) and `01 85 … 00 00 00 12` for all 30 buttons. **No** reset of DPI, LED or report rate; the macro data stays. |
| Macro editor limit | a recorded macro filled 365 of 368 bytes (0x0000–0x016C). The editor stops at the slot size, not at 100 events. |

**Backup file `.meb`:** INI text with `[Mem]`, `count=8192` and one line `<address>=<decimal value>` each, CRLF.
Contains only the 8 KB ROM. `m6980x_configtool rom-dump file.meb` writes the format byte-identically (compared with
a file of the original software), `m6980x_configtool rom-import file.meb` reads it.

Since 2026-10-12 `rom-import` also applies the **button assignment** from the file: flag `0x1F01 + profile × 12 +
button` = `80` → `01 85` to the address from the button table, otherwise default. Before that, only memory and table
were written and the flags were missing, so assignments did not become active. Difference to the original: its
"restore" rewrites all 256 pages (~35 s), `rom-import` only the differing ranges and buttons.
✅ Tested on the device: applied the real original backup `captures/original_sicherung/original.meb` → rear button
in profiles 2/3 middle click again, memory afterwards **byte-identical** with the file (`44d6fbae…`, including the
flags set by the firmware); then Tom's state restored byte-identically.

## Comparison with the manual (peripheral_m6980x_manual.pdf)

- **LED colours:** the colour window offers 7 colours + off. Levels from `Macro.exe` (0x451000–0x45115E):
  off 0 0 0, blue 0 0 2, green 0 2 0, light blue 0 2 2, light green 1 1 0, purple 2 0 1, yellow 2 1 0, red 2 0 0.
  The mouse accepts all 27 combinations; `m6980x_configtool led` knows the 8 original names and more.
- **Factory DPI:** 810 / 1620 / 3240 / 6030. That is what the "Default" button (0x44AD64) writes: values from
  `[6980DPIdefault]`, step 90, using the same DPI function 0x445900. The manual says 400–6000. In the code, 400 is
  only the fallback if the INI is missing (for `step` the fallback is 100); there is no lower limit there, and the
  mouse reports steps 1–56 as supported. `m6980x_configtool` follows the code: 90–6030.
  The default button also writes all **4** stages (including the unused stage 4 = 6030), sets the
  **report rate to 500 Hz** (`01 8B 02`) and marks the page as changed, so that `01 02` follows when leaving it
  (`0x44B2F7–0x44B391`). `m6980x_configtool dpi default` (and the button in the GUI) sets stages 1–3 and 500 Hz
  and saves. Stage 4 stays untouched as Tom decided. ✅ Tested on the device (2026-10-11): 250 Hz →
  `m6980x_configtool dpi default` → 500 Hz and 810/1620/3240, then backup restored byte-identically.
- **LED:** 3 levels per channel give 27 colours (all accepted by the mouse); the original offers only 8 of them.
- **Report rate:** manual 125/250/500/1000 Hz. The software's slider also knows 333 Hz etc. (1–8 ms).
- **9 assignable buttons** (7 + 2) = device buttons 0–4 and 6–9.
- **Scroll speed / Tilt.exe** ("Areson Tilt"; Areson is the OEM, hence manufacturer "A....."): a pure PC helper
  program from the Windows XP era that retrofits horizontal scrolling with the tilt wheel. It reads raw input
  (`RegisterRawInputDevices`/`GetRawInputData`), looks for the scroll bar under the pointer (`WindowFromPoint`,
  `EnumChildWindows`, classes "ScrollBar"/"NUIScrollbar"/"TScrollBar") and sends it scroll messages on a timer.
  The settings are in the registry under `SOFTWARE\Tilt_gaming` (HKCU or HKLM, not checked), controlled by
  `/enable`, `/disable` and `/speed=`.
  It sends nothing to the mouse. Since Vista and on Linux, the operating system does this itself (AC Pan in the
  HID descriptor).
- **Macro export/import** of the original only concerns files on the PC. The mouse has no command for it.
  Format see "Macro file of the original" at the end.

## DPI stage 4 (theoretical only)

Registers 03/04 contain 4 DPI entries, and `01 87 03` also activates stage 4 (seen on the device). But the mouse can
only cycle through 3 stages with its button and shows only 3 LEDs. `Mappingdefault.ini` lists a 4th value (6030) for
the 6980, which the software carries over from the 8000/8600 models. Stage 4 is of no use for this mouse.
`m6980x_configtool` (CLI and GUI) therefore shows and sets only stages 1–3. Backup and restore still include stage 4
so that the state of the mouse is preserved completely.

## Cross-check with Ghidra (2026-10-11)

`Macro.exe` (SHA-256 `9b327ccc…62aa45`) was decompiled with Ghidra 12.1.4 via REA and every statement about the code
in this file was checked against it. Result: **all confirmed, none refuted.** Delphi passes arguments in
registers; where Ghidra's pseudo code loses them, the assembly was read.

| Address | Function |
|---|---|
| `0x44407C` | device detection: all HID devices, feature length 9, `00 00` → `0C 12`, then `00 01`, `00 02 <counter>` |
| `0x444AAC` / `0x44494C` | handshake (detection / all other commands) |
| `0x445280` `0x445470` `0x445398` `0x445BA8` `0x4455AC` | `01 87`, `01 88`, `01 85` (with swap 3↔4), `01 86`, `01 8B`, 10 ms each |
| `0x4455F4` `0x4454C0` `0x445560` `0x44541C` `0x44550C` | read `01 07`, `01 08`, `01 0B`, `01 0A`, `05 00 0C 12` |
| `0x445304` `0x445B24` `0x445700` `0x4457FC` | read `01 05`, `01 06`, `01 03 01` (mask), `01 03`/`01 04` |
| `0x445C5C` | save `01 02`, 4 ms |
| `0x445C08` | sends `00 00…`, returns data byte 5 of the reply (`03`), purpose open ❓ |
| `0x444000` `0x443F80` `0x444BA8` `0x444C30` `0x444F44` | ROM: read, 1 byte, 2 bytes, n bytes, page |
| `0x445900` / `0x445640` | write DPI / reduce fraction |
| `0x44ABBC` | default button (DPI from INI, 500 Hz) |
| `0x44905C` | report rate slider: position p → `01 8B (9 − p)`, display 1000 / (9 − p) Hz |
| `0x4133A0` | write the macro icon byte, callers `0x40F4A4` (slot + 1) and `0x4173F4` (100 + n) |
| `0x40F914` | draw the macro list: value ≤ 100 → "MACRO n", otherwise icon value − 100 |
| `0x43EAF0` | load `Bmp\1.bmp` … `Bmp\60.bmp` |
| `0x451000` | 8 LED colours |

Assumption ❓ (from the code only, not observed): the default button sets the slider to position 7 and relies
on its `OnChange` setting the value `[+0x3A4]` to 7. If the slider is already at 7, `OnChange` presumably does
not fire, and the original sends `01 8B 07` (≈ 142 Hz) instead of `01 8B 02`. A capture could settle this:
set the original to 500 Hz, then press "Default".

Group 5 confirmed: `0x425D44` reads the LED colour for every profile up to the count from `05 00 0C 12` into a
0xA4-byte profile structure. Range 0x1F01–0x1FFF: apart from the marker at 0x1F00, the original reads nothing there
specifically (only for the full backup). It gets the button states via `01 05`. What else is stored there cannot be
derived from the software.

### Memory layout from the code (`0x42EBD8`)

With the INI values of the 6980 (`MemTotal` 8192, `MacroCount` 20, `MacroInfoSize` 10, `[ListMemSize] Total` 360):

| Size | Formula in the code | Value |
|---|---|---|
| macro area | MemTotal − 0x100 − ListMemSize.Total | 7576 |
| slot size | (macro area − 20 × 10) / 20 | **368 = 0x170** |
| slot n | n × slot size | 0x0000, 0x0170, … |
| info entry n | 20 × slot size + 1 + n × 10 | **0x1CC1** + n × 10 |
| function 0 | last info entry + 10 + 3 | **0x1D8C** |
| function i | function i−1 + ListMemSize[i−1] | |
| config area | MemTotal − 0x100 | 0x1F00 |

This also proves the addresses previously derived from the dump and the captures in the code.

### Function records from the code (`0x423630`)

The button assignment builds every record from constants in the code. All 46 functions and 5 extra records:

- Functions 0–42 are byte-identical with `m6980x_configtool`, **except** these cases (all tested on the device):

| No. | Function | Tool | Original |
|---|---|---|---|
| 12 | Undo | usage `1C` (Ctrl+Z on a German keyboard) | `1D` (Ctrl+Z on a US keyboard, Ctrl+Y on a German one) |
| 19/20 | Minimize/maximize | tap Alt+Space, then N/X | hold Alt, Space, N/X, then release in reverse order |
| 33 | Internet | consumer `0196` (AL Internet Browser) | consumer `0223` (AC Home) |
| 35 | Previous window | Alt+Tab | Alt+Esc (`29`) |

  Decision (Tom, 2026-10-11): `m6980x_configtool` keeps all four variants. Reasons and consequences:
  - **Undo:** HID usages are key positions of a US keyboard. On German QWERTZ, `1D` is the Y key, so the original
    triggers Ctrl+Y (*redo*) there, a bug of the original. `m6980x_configtool` only fits the German layout; with a
    US layout it would be the other way round.
  - **Minimize/maximize:** both open the window menu with Alt+Space; `m6980x_configtool` types like a human, the
    original holds all keys. No difference under Windows; under Linux both depend on the desktop environment.
  - **Internet:** `0196` starts the browser (Linux `KEY_WWW`). `0223` is the home page key (Linux `KEY_HOMEPAGE`);
    in an open browser the current tab presumably jumps to the home page ❓ (Windows mapping not officially documented).
  - **Previous window:** Alt+Tab switches to the last used window (twice = back), Alt+Esc sends the window to the
    back and cycles through all windows without restoring minimised ones. Alt+Tab matches the name and works
    everywhere on Linux too.

- **Three functions were missing in `m6980x_configtool`** (in `List2` marked "Only" or without a hint); added since:

| No. | INI name | Record | Address |
|---|---|---|---|
| 43 | Vertical moving | `C6 00`: move vertically only while held ✅ | 0x1ECA |
| 44 | Horizontal moving | `C7 00`: move horizontally only while held ✅ | 0x1ECC |
| 45 | Ghost | `04 83 E0 83 E2 83 0E 84 0E 84 E2 84 E0 00` = Ctrl+Alt+K ✅. This is the hotkey registered by the autostart program `GHOSTOPEN.exe` (`RegisterHotKey(…, MOD_CONTROL\|MOD_ALT, 'K')` at file offset 0xF0A) to open `Macro.exe`. Without the original software installed, nothing happens | 0x1ECE (present like this in the factory ROM) |

- **Wheel buttons 8/9:** if function 0–4 (left, right, middle click, browser forward/back) is put on wheel up/down,
  the original does **not** write the hold record `06 2m 89 FF FF 00` but its own click record
  `04 2m 4m 00` at addresses 46–50 (0x1EDC, 0x1EE0, 0x1EE4, 0x1EE8, 0x1EEC; order mask 1, 2, 4, 0x10, 0x08).
  A wheel notch has no "release". The factory ROM contains `FF` there. Since 2026-10-11 `m6980x_configtool` does it
  like the original ✅ (round 6: wheel up = left click, wheel down = right click, one clean click per notch, nothing
  gets stuck).
- The original only allows function 6/7 (scroll left/right) on button 6/7 itself (= default).
- Function 5 "Perfect": for the M8600, 6980X and a third model `04 85 92 01 …` = consumer `0192` "AL Calculator"
  (Linux `KEY_CALC`), so on this mouse the **calculator** (the original's icon shows one too). For other
  mice `C1 00` (presumably the sensor command "Perfect tracking precision"). In `m6980x_configtool` the function is
  called `calculator` (German `rechner`; `perfect` still works).

### Macro encoder of the editor (`0x410AE8`)

It turns every editor line into bytes. The original produces no other event types:

| Editor line | Bytes |
|---|---|
| mode once / loop / fire | `04` / `05` / `06` at the start |
| press key `SC:<hex>` | `83 uu`, for consumer codes `85 lo hi` |
| release key | `84 uu`, for consumer codes and "Multi-Media" `85 00 00` |
| press left/right/middle/button 4/button 5 | `21` / `22` / `24` / `28` / `30` |
| … release | `41` / `42` / `44` / `48` / `50` |
| wheel down / up | `7F` / `61` (always exactly one notch) |
| wait < 256 ms / otherwise | `88 nn` / `89 lo hi` |
| end | `00` |

The editor recognises `B0 B5 B6 B7 CD E2 E9 EA 183 18A 192 221 223 224 225 22A` as consumer codes. "Alt" is
excluded because its keyboard usage `E2` coincides with "mute" (`E2`).

Consequences for `m6980x_configtool`: the original never produces mouse movement or combined mouse button masks.
**Wheel values above 1** (`m6980x_configtool macro … wheel down 2` = one event `7E`) do not occur in the original and
are untested on the device ❓. Since 2026-10-11 `m6980x_configtool` therefore writes several notches like the
original as several `61`/`7F` (1 byte per notch).

### Input reports of the mouse (capture round 6, `captures/runde6.pcapng`) ✅

| Endpoint | Report | Meaning |
|---|---|---|
| `0x81` (interface 0) | 7 bytes: `buttons, X lo, X hi, Y lo, Y hi, wheel, tilt` | normal mouse |
| `0x82` (interface 1) | `01 12 03 01 <profile>` | **reports every profile switch by itself** (0–2), also by button |
| `0x82` | `02 00 00 <usage> …` (16 bytes) | keyboard from macros/functions. Modifiers like Ctrl/Alt appear as usage `E0`/`E2` in the list, not in the modifier byte |
| `0x82` | `03 lo hi` | consumer key, `03 00 00` = released (also comes after every macro) |

Interesting for Linux: the active profile can be followed via `hidraw` on interface 1 without polling `01 08`.

## Macro file of the original (export/import in the macro editor) ✅ code and device

Dialog: "Ini File(*.ini)", suggestion `default.ini` (form `TForm1`). Export `0x4148A0`, import `0x418408` → `0x415EE4`.
Delphi `TIniFile`, so ANSI text with CRLF:

```ini
[Macro]
count=8
0=4
1=131
2=4
…
7=0
```

- `count` = number of bytes, then one `<index>=<decimal value>` per byte. The bytes are exactly the macro record in
  the ROM (mode `04`/`05`/`06`, events, `00`), produced with the same encoder as when writing to the mouse.
- **The name is not in the file.** The import only fills mode and events into the editor; the name is entered there.
- The export only writes if the macro has more than 2 bytes (not empty).
- The import reads values with `ReadInteger` (missing = 0), decodes up to `00` and skips unknown bytes.
- `[SpecialMacro] index=<n>`: the export never writes this. If `index ≠ 0`, the import only takes the mode and leaves
  the event list empty (`0x415DF8`).

`m6980x_configtool` (since 2026-10-12): `macro-export SLOT file.ini` writes the format byte-identically (test with
Tom's macro 1 "Klicker": `count=4`, `06 21 41 00`). `macro-import SLOT file.ini [--name …] [--icon N]` reads it, cuts
at the real end byte (not at the first `00`, which also occurs in arguments) and takes the file name without `--name`.
In the GUI, the buttons "Load from file" / "Save to file" load/save the editor, as in the original.

**Counter-test with the real original software (2026-10-12)**, unpacked without installation via `msiexec /a`:
- `m6980x_configtool` wrote test macro 20 "TestM20" (mode loop, icon 12, all event types: keys, wait
  8/16 bit, 5 mouse buttons, wheel, consumer). **The original showed it completely correctly** (name, icon, mode,
  all events; confirmed by Tom). ✅
- The original's export of it (`captures/originaltest/TestM20_original.ini`) is **byte-identical** with
  `m6980x_configtool macro-export` (258 bytes, CRLF). ✅
- `macro-import` of this original file into slot 19 produced the same 35 bytes as slot 20. ✅
- Afterwards the backup was restored byte-identically. The original changed no values at startup (it writes DPI
  stage 1 and the report rate back with the existing values).
