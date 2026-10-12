# M6980X Configuration Tool – Python prototype 1.1

Lightweight configuration tool for the **Gigabyte GHOST M6980X** (USB `E0FF:0001`) as a replacement for the
Gigabyte software. It runs on Windows and Linux.

This is the **frozen reference** for the Rust version in the repository root: the parity tests compare the Rust
code with this prototype (`tools/export_fixtures.py` → `tests/fixtures/parity.json`). Its command names and output
are German (e.g. `hinten`, `kopieren`, `--temporaer`) and stay that way, because the fixtures are generated from them.

Unofficial reimplementation, not affiliated with Gigabyte. "GHOST" in this documentation only refers to the mouse or
the official Gigabyte software ("GHOST Macro Engine", `Macro.exe`). The tool itself is called `m6980x_configtool`.

**Status 1.1:** everything is tested on the device: profile switch, LED colours, DPI, report rate, button assignment
(all 46 functions, macro, default), creating macros (also mouse wheel) and ROM writes. Key names in macros follow the
German QWERTZ layout. The tool is not yet tested on Linux.
Every write is read back for verification.
Report rate, DPI and active profile are saved permanently (`01 02`) automatically. With `--temporaer` they only last
until unplugged; `py -3 -m m6980x_configtool save` saves later.

## Installation

Windows:
```
py -3 -m pip install hidapi
```

Linux (no hidapi needed, direct access via `/dev/hidraw`):
```
sudo cp linux/70-m6980x-configtool.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger
```

**Project status, guidelines and open tasks:** `docs/HANDOVER.md` · Protocol: `docs/PROTOCOL.md`

## GUI

```
py -3 -m m6980x_configtool gui        # Linux: python3 -m m6980x_configtool gui
```
Quick prototype (Tkinter, no extra packages) with all features in 5 tabs: overview (profile, DPI stage, report rate,
LED with colour preview, live display of the mouse buttons), DPI, buttons, macros (editor with icon and byte counter)
and backup (JSON, .meb, release all). All mouse access runs in the background; a log at the bottom records every
command.

## Usage (command line)

In the project folder:
```
py -3 -m m6980x_configtool info                 # device, active profile, DPI stages, LED colours
py -3 -m m6980x_configtool info --json
py -3 -m m6980x_configtool profile              # show the active profile
py -3 -m m6980x_configtool profile 2            # switch to profile 2 (1-3)
py -3 -m m6980x_configtool led                  # LED colours of all profiles
py -3 -m m6980x_configtool led 2 orange         # colour of profile 2 (name or R G B, each 0-2)
py -3 -m m6980x_configtool dpi                  # DPI stages and active stage
py -3 -m m6980x_configtool dpi select 2         # activate stage 2
py -3 -m m6980x_configtool dpi set 1 1200       # stage 1 to 1200 DPI (steps of 90, max. 6030)
py -3 -m m6980x_configtool dpi set 1 1800 1200  # separate X and Y
py -3 -m m6980x_configtool watch                # watch profile/DPI stage live
py -3 -m m6980x_configtool rate                 # show the report rate
py -3 -m m6980x_configtool rate 500             # 1000/500/333/250/200/166/142/125 Hz
py -3 -m m6980x_configtool buttons              # button assignment of all 3 profiles
py -3 -m m6980x_configtool functions            # assignable functions (confirmed / derived / unknown)
py -3 -m m6980x_configtool button 1 hinten kopieren    # profile 1, rear side button = Ctrl+C
py -3 -m m6980x_configtool button 1 vorne makro:3      # put macro 3 on the front side button
py -3 -m m6980x_configtool button 1 vorne standard     # back to the button's built-in function
py -3 -m m6980x_configtool macro 5 Test tippe strg+c, warte 100, klick links   # write macro 5
py -3 -m m6980x_configtool macro 6 Auto klick links, warte 50 --mode feuer      # mode einmal/schleife/feuer
py -3 -m m6980x_configtool macro 7 Zoom drueck strg, rad hoch, los strg         # mouse wheel in a macro
py -3 -m m6980x_configtool macro 8 Kopie tippe strg+c --bild 12               # with preview icon 12
py -3 -m m6980x_configtool macro-bild 1 4                 # change only the icon (1-100 or keins = none)
py -3 -m m6980x_configtool macro-export 1 klicker.ini      # macro as a file of the original software (.ini)
py -3 -m m6980x_configtool macro-import 5 klicker.ini --name Klicker   # import a macro file of the original
py -3 -m m6980x_configtool backup backup.json       # full backup (memory + all settings)
py -3 -m m6980x_configtool restore backup.json      # write back (differences only) and save permanently
py -3 -m m6980x_configtool rom-dump backup.bin      # back up the whole 8 KB memory (.meb = format of the original)
py -3 -m m6980x_configtool rom-import old.meb       # import a backup of the original software
py -3 -m m6980x_configtool macros               # macro slots (or: --rom backup.bin)
```
On Linux use `python3` instead of `py -3`.

Do not run the original software (`Macro.exe`, "GHOST") or OpenRGB at the same time. Both talk to the mouse over
the same channel.

## Tests

```
py -3 -m unittest discover -s tests
```
The tests use a simulated mouse and need no hardware.

## Layout

| Path | Contents |
|---|---|
| `m6980x_configtool/transport.py` | feature reports: hidapi (Windows) or hidraw ioctl (Linux) |
| `m6980x_configtool/protocol.py` | handshake and commands |
| `m6980x_configtool/rom.py` | ROM layout |
| `m6980x_configtool/functions.py` | function table and macro text codec |
| `m6980x_configtool/backup.py` | backup/restore, `.meb` and macro `.ini` files |
| `m6980x_configtool/cli.py` | command line |
| `m6980x_configtool/gui.py` | Tkinter GUI |
| `tools/export_fixtures.py` | writes the parity fixtures for the Rust tests |
