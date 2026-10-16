# Handover / project status – M6980X Configuration Tool (as of 2026-10-12, version 2.0 = Rust)

This file summarises everything worked out so far, so that a new session can continue without losing knowledge.
Technical details of the protocol are in `docs/PROTOCOL.md`, usage in `README.md`.

**Since 2026-10-12 the tool is a Rust program** in the repository `github.com/Tom-8192/m6980x-driver` (private), locally
at `C:\Users\Tom\dev\m6980x-driver` (outside OneDrive because of `target/`). The Python prototype 1.1 is kept as a
reference in `prototype/python/`. Analysis tools (`re_tools/`), captures (`captures/`), original programs and Tom's
backups (`backups/`) stay in the old project folder `…\OneDrive\Dokumente\OpenRGB Projekt\GHOST_Tool` and do not go
into the repository.

## Goal

Replacement for Gigabyte's "GHOST Macro Engine" software for the **Gigabyte GHOST M6980X** mouse (USB `E0FF:0001`,
OEM **Areson**): lightweight, no installation, **Windows first, always keep Linux in mind**.

## What is done

### Rust 2.0 (2026-10-12)

- Cargo workspace: `crates/m6980x-core` (library), `crates/m6980x-cli` (`m6980x_configtool`), `crates/m6980x-gui`
  (`m6980x_configtool_gui`, egui 0.36). Full parity with prototype 1.1.
- **Parity tests:** `prototype/python/tools/export_fixtures.py` → `tests/fixtures/parity.json`; in 8 scenarios Rust
  sends the same 6423 packets, leaves the same ROM, `.meb`/`.ini` byte-identical. All unit tests of the prototype
  ported as well. CI (GitHub Actions) on Windows and Ubuntu, the prototype workflow
  checks that the fixtures match the prototype (only when `prototype/` or `tests/fixtures/` change).
- **Cross-process lock** around every command (formerly open task 6): Windows named mutex
  `Local\m6980x_configtool`, Linux `flock` on the hidraw device, max. wait 5 s.
- Checked **read-only** on the device (Windows): `info`, `buttons`, `dpi`, `rate`, `led`, `rom-dump` (SHA `55906362…`,
  byte-identical with the Python dump, ~5.9 s like Python), `backup` (Python reads the Rust backup, content equal to
  the evening backup). Rust reads all existing backups of the prototype.
- **Write tests on the device with Rust passed** (2026-10-12, agreed with Tom): backup
  `backups/vor_rusttest_2026-10-12.json` (old project folder), then profile 3, LED profile 3 red, DPI stage 2 X 2700/Y 1800,
  stage 1 active, 1000 Hz, profile 3: back = copy, wheel-up = left-click (click record), forward = speed-2x,
  macro 20 "RustTest" (loop, icon 5) on tilt-left, macro export and import into slot 19, remove icon – everything
  applied and read back. Then `restore`: ROM byte-identical (SHA `55906362…`), `info` and full backup identical to
  before.
- Lock checked on the device: GUI with live display open, 3× `rom-dump` in parallel – all byte-identical, no error.
- License: GPL-2.0-or-later (matching OpenRGB).
- **Multilingual** (2026-10-12, Tom's request: English as the main language, German with real umlauts, further
  languages without code changes): all texts in `locales/<code>.yml` (rust-i18n, embedded at build time; `build.rs` in
  the core rebuilds on changes). The code only uses keys: `tr!("error.no_reply", packet = …)` from
  `crates/m6980x-core/src/i18n.rs` (process-wide language, `name()`/`matches()` for names, `fold()` for input without
  umlauts). Ids in the code: functions = ASCII name of the prototype (`rueckgaengig`), keys/colours/mouse buttons/modes =
  English (`ctrl`, `light-blue`); names per language under `function.*`, `key.*`, `led.*`, `button.*`, `mouse.*`,
  `mode.*`. Input is recognised in all languages. Command-line help under `cli.*` (`help.rs`); a test checks that
  every command/argument has a text. GUI: language picker listing all available files (stored via eframe
  persistence). Without a choice: `--lang`, `M6980X_LANG`, otherwise the system language, otherwise English.
  Unit tests run in German (the parity comparison turns umlauts into ae/oe/ue), `tests/english.rs` checks English,
  `every_language_is_complete` checks all language files. Tried with a test `fr.yml`: works without code changes.
  The keyboard layout stays German (y/z). New English options: `--temporary` (old `--temporaer`), `--icon` (old
  `--bild`), `macro-icon` (old `macro-bild`).
- **Repository texts in English** (2026-10-12, Tom's request): code comments, developer messages, docs and new commit
  messages are English. `README.de.md` and `locales/de.yml` are the German translations.
- GUI: **checkbox "Link X and Y"** on the DPI tab (2026-10-12): Y follows X and is locked; turning it on sets Y = X;
  stored. If the mouse reports different X/Y values, the checkbox is cleared so Y stays visible.
- GUI played through with `--fake` (tabs, read/write macros, byte counter, error dialog).

### GUI redesign 2.2 (2026-10-12)

Tom's decisions: side bar + header with profile buttons, changes apply at once (no "Set" buttons), macro editor with
step list, text view and recording, drawn mouse on the buttons page (based on a product photo Tom showed: centre strip
with tilt wheel, three LED bars, DPI and profile button, two side buttons on the left – **drawn with shapes, no product
images**).

- `crates/m6980x-gui/src/`: `main.rs` (frame: side bar, header, status bar, settings dialog), `app.rs` (state and all
  mouse actions as worker jobs), `pages/*.rs` (one page each: overview, buttons, lighting, DPI, performance, macros,
  backup), `theme.rs` (dark/light colours, stored like the language), `widgets.rs` (cards, segmented buttons, swatches,
  two-column layout that stacks when narrow), `mouse_art.rs` (vector mouse with hit regions), `recorder.rs` +
  `hid_keys.rs` (macro recording), `worker.rs` (unchanged apart from the connection state). Icons: `egui-phosphor`.
- Header profile button = activate the profile on the mouse **and** edit its buttons (like the original); the live
  display follows presses of the profile/DPI button. Disconnected mouse: red dot, quiet reconnect every 1.5 s.
- Instant apply: profile, DPI stage, rate, LED colour, button assignment write on click; DPI values when the slider is
  released or a typed value is done (not while dragging). Macros/ROM are still written with an explicit button.
  "Save permanently" (01 02) after profile/DPI/rate as before (checkbox on the performance page).
- Macro steps: `m6980x_core::macro_steps` (`Step`, `steps_from_events`, `events_from_steps`, `fit`) – byte format of
  `encode_events`/`decode_record`, unknown bytes are kept (`Step::Raw`).
- Recording: keys anywhere in the window (egui 0.36 delivers physical keys incl. Ctrl/Shift/Alt/Win/AltGr; Ctrl+C/X/V
  arrive as copy/cut/paste events and are mapped back), mouse buttons and wheel only inside the recording field;
  real pauses or a fixed pause; stops when the slot is full and never leaves a key pressed. While recording, key
  events are taken out of egui's input and no widget keeps the focus.
- Macro jobs show progress (`App::run_tracked`, banner above the editor buttons, percent in the status bar) and a
  success message; the core reports progress via `write_rom_progress` / `write_macro_progress` (same packets).
  After writing, only the slot and the info table are read back (not 8 KB). Empty name → "Macro <slot>".
- Key names extended: `f13`–`f24`, German punctuation positions (`ü ö ä # ^ , . - <`, caps lock, numpad plus) and
  `0xNN` for any usage, so recorded macros and the text view always round-trip.
- Checked with `--fake` via screenshots (all pages, German/English, light/dark, narrow window); recorder covered by unit
  tests. **Not yet clicked through by hand and not yet tried on the real mouse.**
- Linux: compiles (cross-check `x86_64-unknown-linux-gnu`), not yet run with the mouse.

### Scroll settings and tilt mode (2026-10-15, uncommitted until Tom agrees)

- The scroll area on the original's DPI page had been overlooked; reverse engineered with Ghidra/REA and on the
  device, see PROTOCOL.md "Scroll settings". Main finding: **Tilt.exe switches a tilt mode in the mouse** with a
  string descriptor sequence (serial ×2, 120, then 200 = tilt mode / 201 = native); not stored in the mouse.
- New: `pc.rs` (Windows scroll speed via `SystemParametersInfo`, detection of Tilt.exe/Macro.exe), transport
  `string_descriptor`/`serial_number` (Windows; Linux returns "not supported"), `M6980X::set_tilt_mode`,
  CLI `scroll` and `tilt-mode`, GUI card "Scrolling" on the performance page (Windows only) with the button
  "Tilt wheel native", warning banner while Macro.exe runs. `tilt-mode` tested on the device in both directions.
- Fixed: five CLI messages showed `%{…}` placeholders (wrong argument names, e.g. `backup`); new test
  `crates/m6980x-core/tests/placeholders.rs` compares every `tr!` call with the language files.
- GUI card only checked with `--fake` via screenshots (German/English); not clicked by hand.
- Released as **2.3.0**. Afterwards Tom found that the horizontal slider (`SPI_SETWHEELSCROLLCHARS`) has no effect in
  Excel and Firefox → removed again (vertical slider, `scroll <lines>`, tilt mode switch stay). Not released yet
  (Tom: no build for now). Possible next step: own tilt helper replacing Tilt.exe (see PROTOCOL.md).

### Python prototype 1.1

- Python package `m6980x_configtool` with command line (`py -3 -m m6980x_configtool …`) and Tkinter GUI
  (`py -3 -m m6980x_configtool gui`). Its output is German; it is the reference for the parity fixtures.
- **All features of the original software, tested on the device** (Windows): profile switch, LED colour per profile
  (27 combinations), DPI stages 1–3 (separate X/Y, 90–6030), active DPI stage, report rate (125–1000 Hz), persistent
  save (`01 02`), button assignment (9 buttons × 3 profiles, all 46 functions, macros, default), macros (editor,
  3 modes, mouse wheel, consumer keys, icons 1–100), delete/release, full backup (JSON), `.meb` format
  of the original (byte-identical), macro files (.ini) of the original editor (export/import, byte-identical), factory
  DPI (like the default button including 500 Hz).
- More than the original: button back to "default", all 27 LED colours, X/Y DPI without rounding errors, read-back
  after every write, German keyboard layout in the macro text.
- 50+ unit tests (`py -3 -m unittest discover -s tests`) with a simulated mouse (`tests/test_protocol.py: FakeMouse`).
- `re_tools/gui_smoke.py <project folder>` (old project folder) plays through the GUI automatically against the
  simulated mouse.

## Tom's guidelines (please follow)

1. **The original software's code beats the manual** – manuals are often inaccurate.
2. **DPI stage 4** exists in memory but is useless (the mouse only cycles 3 stages): only mention it in the docs,
   CLI/GUI do not show it. Backup/restore silently include it.
3. **LED: 27 colours** (3 levels per channel); the 8 of the original software are only a manufacturer limitation.
4. **No Gigabyte images in the final build**: only own icons, created with an image AI from the texts in
   `assets/icon_descriptions.json` (`assets/icons/`, see below). Image folders, original EXEs and captures are in
   `.gitignore`.
5. Write tests on the mouse only after agreement; always back up first (`m6980x_configtool backup`) and restore the
   memory byte-identically afterwards (succeeded every time so far; SHA-256 of the original memory `44d6fbae…`, current
   state `55906362…`, see "Tom's mouse – current state").
6. Communication with Tom in German; texts in the repository (code comments, docs, commit messages) in English.
   Visible German program texts use real umlauts (ä, ö, ü, ß); input without umlauts keeps working.
7. **Name:** the reimplementation is called "M6980X Configuration Tool" (package/command `m6980x_configtool`), never
   "ghost", so it is not confused with the official "Gigabyte GHOST" software (renamed on 2026-10-11).

## Tom's mouse – current state (restored after all tests)

**Update 2026-10-15:** Tom reinstalled the original software on 2026-10-14 (Tilt.exe and GHOSTOPEN.exe in the
autostart, so the mouse is in tilt mode after every login). The settings changed since the list below (e.g. LED
255/255/0 in all profiles, DPI 810/3600/5850, profile 1 back = "Internet"); current full backup:
`backups/vor_tilttest_2026-10-15.json`. The list below is the state of 2026-10-11.

- Memory like `backups/rom_2026-10-11_original.bin`, with these exceptions (SHA-256 now `55906362…`):
  - Tom removed the leftover icon of the empty macro 6 (0x1CFC: `79` → `06`).
  - **Older leftovers without effect:** 0x1EDC–0x1EDF = `04 21 41 00` (click record "left click on wheel") and
    0x1F4D–0x1F4E = `DC 1E` (button table profile 1, wheel up). They come from an earlier test in the GUI
    (left click on wheel up, then default again); the firmware leaves the old address in place when resetting.
    Wheel up is default. Deliberately not cleaned up, as Tom decided.
  - Rear side button in profiles 2 and 3 set to default (Tom, GUI, 2026-10-11 evening): flags 0x1F10/0x1F1C
    `80` → `00`, the old address 0x1D98 stays in the button table.
- LED: profile 1 white (2 2 2), profile 2 green (0 2 0), profile 3 blue (0 0 2).
- DPI: 1620 / 3780 / 5850 (stage 4 internally 6030), 500 Hz.
- Buttons: forward = macro 1 "Klicker" (auto clicker, mode fire, icon 4 = skull) in all three profiles, default
  everywhere else (as of 2026-10-11 evening).
- Current full backup: `backups/stand_2026-10-11_abend.json` (older ones: `vor_runde6_2026-10-11.json`,
  `vor_default_test_2026-10-11.json`, `ghost_sicherung_vor_werksreset.json`).

## Key protocol findings (summary, details in PROTOCOL.md)

- Feature report without report ID on **interface 0**, 8 bytes `[group, command, data…]`, handshake via status bits
  0x80 (busy) / 0x40 (done). No unlock needed.
- Group 1 registers: `03/04` DPI, `05` button, `06` LED, `07` active stage, `08` profile, `0B` report rate (ms),
  `02` = **save permanently** (without it: rate/DPI/profile/stage lost after unplugging; LED always stays).
- Group 2 ROM (8 KB): read `00`, write `01` (1 B), `02` (2 B), buffer `03/05–09` + `04`, page `0A`.
- ROM: 20 macro slots of 368 B from 0x0000, info table 0x1CC1 (name 9 B + icon byte), function records from 0x1D8C
  (fixed addresses per INI), firmware button table 0x1F3D + profile·0x18 + button·2.
- Device buttons: 0 left, 1 right, 2 wheel click, 3 back, 4 forward, 5 – (no button), 6/7 tilt wheel L/R,
  8/9 wheel up/down.

## Open tasks

1. **Linux test** (main goal!): `crates/m6980x-core/src/transport/linux.rs` uses `/dev/hidraw*` via ioctl (without
   hidapi), udev rule in `linux/`. Never run on Linux yet – start read-only (`m6980x_configtool info`), check device
   detection (interface 0 via sysfs). Short reports (fewer than 8 bytes returned) are treated like "busy" – untested.
2. **Fix the OpenRGB fork** (`…\OpenRGB Projekt\OpenRGB-M6980X`, `Controllers/GigabyteGhostController`): replays the
   wrong 130-packet "init" at every start (overwrites DPI stage 3 and register 0B, activates profile 1),
   ignores the busy/done handshake, sends LED values 0–255 instead of 0–2 and switches the profile when changing the
   colour. Correct would be: only `01 86 <profile> R G B` with handshake, without init.
3. **Own icons**: done on branch `macro-icons` (not merged yet). 41 own AI icons (0 = empty slot, 1–40 = pictures)
   plus an empty frame, 128×128 WebP q90 (135 KB in total, measured against PNG 984 KB / lossless WebP 648 KB with
   no visible difference), embedded via `include_bytes!` (`crates/m6980x-gui/src/icons.rs`). 41–100 are drawn as
   the frame with a silver number. GUI: icon preview + "Choose icon…" dialog with a 10-column grid in the macro
   editor, small icon in the macro list. Rebuild the files with `py -3 assets/icons/make_icons.py <folder> --frame
   <png>` (sources on Tom's desktop, `ghost icons/rebuilt_icons_hires` + `master_button_plate_rounded.png`).
   Checked with `--fake` via screenshots (dark theme, German); not yet on the mouse.
4. GUI 2.2 (see above): Tom to try it by hand and on the mouse (recording, instant apply, reconnect).
5. Keyboard layout of the macro text: key names follow the German layout in every UI language (y/z swapped);
   possibly an option for other layouts.
6. Smaller open questions (❓ in PROTOCOL.md): rest of the config area from 0x1F85, meaning of the bytes after the info
   entries. (Button flags 0x1F01 + profile × 12 + button: resolved 2026-10-11.)
7. **System-wide lock for mouse access** – implemented in Rust 2.0 (see above); the Python prototype does not have it.
   Background (Tom's decision 2026-10-11): the prototype only locks within one process, exactly like the original
   (`EnterCriticalSection` in `0x44494C`). If two instances run at the same time (e.g. open GUI with live display +
   command line, or tool + OpenRGB), the busy/done handshakes get mixed up: one side gets the other's reply or runs
   into "no reply to …". Observed twice on 2026-10-11 (reading button 5, full backup). Harmless so far because every
   write is read back.

## Working environment (Tom's PC)

- Rust 1.99 via rustup (`%USERPROFILE%\.cargo\bin`, in Git Bash possibly `export PATH="$HOME/.cargo/bin:$PATH"`),
  Visual Studio 2022 Build Tools (VC tools) for the MSVC linker; target `x86_64-unknown-linux-gnu` installed for
  `cargo check`. `rustfmt.toml`: line width 120.
- Python 3.13 via `py -3.13` (python/python3 are only Store stubs). `hidapi` installed.
- Wireshark/tshark `C:\Program Files\Wireshark\tshark.exe` and USBPcap installed, not in PATH. Capturing works
  **without admin rights**, best from PowerShell (Git Bash mangles `\\.\USBPcap1`):
  `tshark -i \\.\USBPcap1 -i \\.\USBPcap2 -i \\.\USBPcap3 -w file.pcapng`
- Disassembler: Python package `capstone` (install in its own venv: `py -3.13 -m venv venv` +
  `venv\Scripts\pip install capstone`). Since 2026-10-11 also Ghidra 12.1.4
  (`C:\Users\Tom\Downloads\ghidra_12.1.4_PUBLIC_20260921\ghidra_12.1.4_PUBLIC`), Temurin JDK 21 and the REA MCP server
  in Claude Code (`GHIDRA_INSTALL_DIR` in the MCP registration). REA needs a local copy outside OneDrive:
  `C:\Users\Tom\RE\GHOST\Macro.exe`. All code statements in PROTOCOL.md were cross-checked with it.
- Git Bash: the built-in `expand` shadows Windows' `expand.exe` – use the full path.
- Start the original software without installing it (for counter-tests):
  `msiexec /a "<GHOST(6980X).msi>" TARGETDIR="<folder>" /qn` unpacks all files with their folder structure (no
  autostart, no registry); then start `<folder>\Personal\GIGABYTE\GHOST(6980X)\Macro.exe`. The MSI is created from
  `6980X101.exe` with `re_tools/isx.py`. Used like this on 2026-10-12: the original shows macros written by us
  correctly, macro files byte-identical.
- The original software is **uninstalled** (autostart gone). The folder `Documents\GIGABYTE\GHOST(6980X)` remained.
  Do not run it at the same time as the tool or OpenRGB. At startup it rewrites DPI stage 1 (briefly with a wrong
  Y value) and the report rate.

## Tools in `re_tools/` (old project folder)

| File | Purpose |
|---|---|
| `isx.py`, `cfb_stream.py`, `msiq.vbs` | unpack installer → MSI → CAB, query MSI tables |
| `pe.py`, `peimp.py`, `xref_imports.py`, `disasm_range.py`, `callers.py`, `strxref.py`, `methtab.py` | static analysis of `original_software/Macro.exe` (capstone) |
| `dfm_images.py`, `imagelist_extract.py`, `icon_sheets.py` | read embedded forms and image lists |
| `usbpcap_feat.py` | feature reports from USBPcap captures (with timestamps) |
| `button_map_test.py`, `function_test.py` | tests on the mouse in profile 3 with exact restore |
| `rate_test.py` | measure the real report rate via capture (move the mouse) |
| `gui_smoke.py` | test the GUI automatically against the simulated mouse |

## Further sources

- `docs/Mappingdefault_original.ini` (old project folder) – function list and record sizes of the original software.
- `captures/` – four captures of the original software (`*_feat.txt` = evaluated feature reports), `.meb` example.
- Manual: `C:\Users\Tom\Downloads\peripheral_m6980x_manual.pdf`. Installer ZIP:
  `C:\Users\Tom\Downloads\peripheral_utility_engine_6980x_v1.03.zip`.
- Older work of another developer (LEDs only, with errors): `…\OpenRGB Projekt\GHOST_ReverseEngineering`.
