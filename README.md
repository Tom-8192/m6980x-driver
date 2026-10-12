# M6980X Configuration Tool

*[Deutsche Fassung](README.de.md)*

A lightweight configuration tool for the **Gigabyte GHOST M6980X** mouse (USB `E0FF:0001`), replacing Gigabyte's
"GHOST Macro Engine" software. Written in Rust. It consists of two programs that need no installation and no
runtime:

* `m6980x_configtool`: command line
* `m6980x_configtool_gui`: graphical user interface (egui)

Unofficial reimplementation, not affiliated with Gigabyte. "GHOST" refers only to the mouse or the official software
(`Macro.exe`).

**Status 2.0 (Rust port of the Python prototype 1.1):** All features are ported: profile, LED (27 colours), DPI
(separate X/Y, 90–6030), report rate, persistent save, button assignment (46 functions, macros, default), macros
(editor, 3 modes, mouse wheel, consumer keys, icons 1–100), backup (JSON), and the original software's `.meb` and
macro `.ini` files. Every write is read back for verification. The packets sent are byte-identical to those of the
prototype that was tested on the device (parity tests, see below). Writing, restoring and the new lock have been
tested on the device under Windows.
New compared to the prototype is a **cross-process lock**: the GUI and the command line can run at the same time.
On Linux the tool builds and passes all tests but has not yet been tested with the mouse.

**Languages:** English and German. The GUI has a language switch (top right, remembered); the command line takes
`--lang en|de`. Without a choice, `M6980X_LANG` or the system language decides (German system → German, otherwise
English). Input is understood in both languages, e.g. `button 1 back copy` and `button 1 hinten kopieren`, or the
macro text `tap ctrl+c, wait 100` and `tippe strg+c, warte 100`. Key names follow the German QWERTZ layout (y and z
are swapped compared to US).

**Adding a language:** all texts (output, help, GUI, key/colour/function names) are in `locales/<code>.yml`, one file
per language, embedded at build time. Copy `locales/en.yml` to e.g. `locales/fr.yml`, translate the values, rebuild —
no code changes. Placeholders like `%{name}` must be kept; an entry `"space|leer"` means display name plus further
accepted input. `cargo test` checks that every language file has all keys and the same placeholders.

**Project status, guidelines and open tasks (German):** [`docs/UEBERGABE.md`](docs/UEBERGABE.md) · Protocol:
[`docs/PROTOCOL.md`](docs/PROTOCOL.md)

## Building

Windows: [rustup](https://rustup.rs) and the Visual Studio Build Tools (workload "Desktop development with C++").
```
cargo build --release
```
The programs are then in `target\release\`.

Linux (Debian/Ubuntu): Rust via rustup, plus for the GUI
`sudo apt install libxkbcommon-dev libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev`.
Access to the mouse (`/dev/hidraw*`, no hidapi; the kernel driver stays attached):
```
sudo cp linux/70-m6980x-configtool.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger
```

## Graphical user interface

```
m6980x_configtool_gui            # or: m6980x_configtool gui
m6980x_configtool_gui --fake     # dry run against a simulated mouse
m6980x_configtool_gui --lang de  # start in German
```
Five tabs: overview (profile, DPI stage, report rate, LED with colour preview, live display of the mouse buttons),
DPI, buttons, macros (editor with icon and byte counter, `.ini` import/export) and backup (JSON, `.meb`, release
all). All mouse access runs in the background; a log at the bottom records every command.

## Command line

```
m6980x_configtool info                 # device, active profile, DPI stages, LED colours (--json)
m6980x_configtool profile 2            # switch to profile 2 (1-3)
m6980x_configtool led 2 orange         # colour of profile 2 (name or R G B, each 0-2)
m6980x_configtool dpi                  # DPI stages and active stage
m6980x_configtool dpi select 2         # activate stage 2
m6980x_configtool dpi set 1 1800 1200  # stage 1: X 1800, Y 1200 (steps of 90, max. 6030)
m6980x_configtool dpi default          # factory values 810/1620/3240 and 500 Hz
m6980x_configtool watch                # watch profile/DPI stage live
m6980x_configtool rate 500             # 1000/500/333/250/200/166/142/125 Hz
m6980x_configtool buttons              # button assignment of all 3 profiles
m6980x_configtool functions            # assignable functions
m6980x_configtool button 1 back copy          # profile 1, rear side button = Ctrl+C
m6980x_configtool button 1 forward macro:3    # put macro 3 on the front side button
m6980x_configtool button 1 forward default    # back to the button's built-in function
m6980x_configtool macro 5 Test tap ctrl+c, wait 100, click left    # write macro 5
m6980x_configtool macro 6 Auto click left, wait 50 --mode fire      # mode once/loop/fire
m6980x_configtool macro 8 Copy tap ctrl+c --icon 12                 # with preview icon 12
m6980x_configtool macro-icon 1 4                 # change only the icon (1-100 or none)
m6980x_configtool macro-export 1 klicker.ini     # macro as a file of the original software
m6980x_configtool macro-import 5 klicker.ini --name Klicker
m6980x_configtool --lang de buttons              # output in German
m6980x_configtool backup backup.json             # full backup (memory + all settings)
m6980x_configtool restore backup.json            # write back (differences only) and save permanently
m6980x_configtool rom-dump backup.meb            # 8 KB memory (.meb = format of the original, otherwise raw)
m6980x_configtool rom-import old.meb             # import a backup of the original software
m6980x_configtool macros                         # macro slots (or: --rom file.bin)
```
Report rate, DPI and active profile are saved permanently (`01 02`) automatically. With `--temporary` they only last
until the mouse is unplugged. `--device <path>` selects a specific device (hidapi path or `/dev/hidrawN`).
Backups of the Python prototype can be read in both directions.

Do not run the original software (`Macro.exe`) or OpenRGB at the same time: they talk to the mouse over the same
channel and do not know about this tool's lock.

## Tests

```
cargo test --workspace
```
* Port of all unit tests of the prototype, using a simulated mouse (`crates/m6980x-core/src/fake.rs`)
* **Parity tests:** `prototype/python/tools/export_fixtures.py` runs scenarios (all settings, DPI, every function on
  a normal and a wheel button, macros, release all, restore, import) against the Python prototype and writes
  `tests/fixtures/parity.json`. Rust must send the same 6423 packets, leave the same ROM and write byte-identical
  `.meb`/`.ini` files.
* Read existing backups: `M6980X_BACKUP_DIR=<folder> cargo test -- --ignored`

## Layout

| Path | Contents |
|---|---|
| `crates/m6980x-core/` | Library: transport (Windows hidapi, Linux hidraw, lock), protocol, ROM layout, functions, backup |
| `crates/m6980x-cli/` | Command line `m6980x_configtool` |
| `crates/m6980x-gui/` | GUI `m6980x_configtool_gui` |
| `docs/PROTOCOL.md` | Protocol with levels of evidence (German) |
| `assets/icon_descriptions.json` | Descriptions of the macro pictures (basis for own icons, no Gigabyte images) |
| `locales/` | Translations, one YAML file per language |
| `linux/` | udev rule |
| `prototype/python/` | Python prototype 1.1 (reference, source of the parity fixtures) |

## License

GPL-2.0-or-later (see [`LICENSE`](LICENSE)), matching OpenRGB. Gigabyte's software and its images are not part of this
repository.
