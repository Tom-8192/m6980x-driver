# Übergabe / Projektstand – M6980X Configuration Tool (Stand 2026-10-12, Version 2.0 = Rust)

Diese Datei fasst alles zusammen, was in der ersten Arbeitssitzung erarbeitet wurde, damit eine neue Sitzung ohne
Wissensverlust weitermachen kann. Technische Details zum Protokoll stehen in `docs/PROTOCOL.md`, die Bedienung in
`README.md`.

**Seit 2026-10-12 ist das Tool ein Rust-Programm** im Repo `github.com/Tom-8192/m6980x-driver` (privat), lokal unter
`C:\Users\Tom\dev\m6980x-driver` (außerhalb von OneDrive wegen `target/`). Der Python-Prototyp 1.1 liegt als Referenz
unter `prototype/python/`. Analysewerkzeuge (`re_tools/`), Mitschnitte (`captures/`), Original-Programme und Toms
Sicherungen (`backups/`) bleiben im alten Projektordner `…\OneDrive\Dokumente\OpenRGB Projekt\GHOST_Tool` und kommen
nicht ins Repo.

## Ziel

Ersatz für die Gigabyte-Software „GHOST Macro Engine“ der Maus **Gigabyte GHOST M6980X** (USB `E0FF:0001`,
Hersteller-OEM **Areson**): schlank, ohne Installation, **erst Windows, Linux immer mitdenken**.

## Was fertig ist

### Rust 2.0 (2026-10-12)

- Cargo-Workspace: `crates/m6980x-core` (Bibliothek), `crates/m6980x-cli` (`m6980x_configtool`), `crates/m6980x-gui`
  (`m6980x_configtool_gui`, egui 0.36). Volle Parität mit dem Prototyp 1.1, alle Befehle und Texte gleich.
- **Paritätstests:** `prototype/python/tools/export_fixtures.py` → `tests/fixtures/parity.json`; Rust sendet in 8
  Szenarien dieselben 6423 Pakete, hinterlässt dasselbe ROM, `.meb`/`.ini` bytegleich. Dazu alle Unit-Tests des
  Prototyps portiert (75 Tests). CI (GitHub Actions) auf Windows und Ubuntu, prüft auch, dass die Fixtures zum Prototyp passen.
- **Prozessübergreifende Sperre** um jeden Befehl umgesetzt (frühere offene Aufgabe 6): Windows benannter Mutex
  `Local\m6980x_configtool`, Linux `flock` auf das hidraw-Gerät, Wartezeit max. 5 s.
- Am Gerät (Windows) **nur lesend** geprüft: `info`, `buttons`, `dpi`, `rate`, `led`, `rom-dump` (SHA `55906362…`,
  bytegleich mit dem Python-Dump, ~5,9 s wie Python), `backup` (Python liest die Rust-Sicherung, Inhalt gleich der
  Abend-Sicherung). Rust liest alle vorhandenen Sicherungen des Prototyps.
- **Schreibtests am Gerät mit Rust bestanden** (2026-10-12, mit Tom abgesprochen): Sicherung
  `backups/vor_rusttest_2026-10-12.json` (alter Projektordner), dann Profil 3, LED Profil 3 rot, DPI Stufe 2 X 2700/Y 1800,
  Stufe 1 aktiv, 1000 Hz, Profil 3: hinten = kopieren, rad-hoch = linksklick (Klick-Datensatz), vorne = speed-2x,
  Makro 20 „RustTest“ (Schleife, Bild 5) auf kipp-links, Makro-Export und Import nach Slot 19, Bild entfernen – alles
  übernommen und zurückgelesen. Danach `restore`: ROM bytegleich (SHA `55906362…`), `info` und Komplettsicherung
  identisch mit vorher.
- Sperre am Gerät geprüft: Oberfläche mit Live-Anzeige offen, parallel 3× `rom-dump` – alle bytegleich, kein Fehler.
- Lizenz: GPL-2.0-or-later (passend zu OpenRGB).
- Oberfläche mit `--fake` durchgespielt (Reiter, Makros lesen/schreiben, Byte-Zähler, Fehlerdialog).
- Linux: kompiliert (Cross-Check `x86_64-unknown-linux-gnu`), noch nicht an der Maus.

### Python-Prototyp 1.1

- Python-Paket `m6980x_configtool` mit Kommandozeile (`py -3 -m m6980x_configtool …`) und Tkinter-Oberfläche (`py -3 -m m6980x_configtool gui`).
- **Alle Funktionen der Original-Software, am Gerät getestet** (Windows): Profilwechsel, LED-Farbe je Profil
  (27 Kombinationen), DPI Stufe 1–3 (X/Y getrennt, 90–6030), aktive DPI-Stufe, Abtastrate (125–1000 Hz), dauerhaft
  speichern (`01 02`), Tastenbelegung (9 Tasten × 3 Profile, alle 46 Funktionen, Makros, Standard), Makros (Editor,
  3 Modi, Mausrad, Consumer-Tasten, Bilder 1–100), Löschen/Freigeben, komplette Sicherung (JSON), `.meb`-Format
  des Originals (bytegleich), Makrodateien (.ini) des Original-Editors (Export/Import, bytegleich), Werkswerte DPI
  (wie der Default-Knopf inkl. 500 Hz).
- Mehr als das Original: Taste auf „Standard“, alle 27 LED-Farben, X/Y-DPI ohne Rundungsfehler, Rücklesen nach jedem
  Schreiben, deutsches Tastaturlayout im Makrotext.
- 50+ Unit-Tests (`py -3 -m unittest discover -s tests`) mit simulierter Maus (`tests/test_protocol.py: FakeMouse`).
- `re_tools/gui_smoke.py <Projektordner>` (alter Projektordner) spielt die Oberfläche automatisch gegen die simulierte Maus durch.

## Vorgaben von Tom (bitte einhalten)

1. **Code der Original-Software geht vor Handbuch** – Handbücher sind oft ungenau.
2. **DPI-Stufe 4** existiert im Speicher, ist aber nutzlos (Maus schaltet nur 3 Stufen): nur in der Doku erwähnen,
   CLI/GUI zeigen sie nicht. Backup/Restore nehmen sie still mit.
3. **LED: 27 Farben** (3 Stufen je Kanal), die 8 der Original-Software sind nur eine Hersteller-Beschränkung.
4. **Keine Gigabyte-Bilder im finalen Build**: Es gibt nur Texte (`assets/icon_descriptions.json`), aus denen später per
   Bild-KI eigene, urheberrechtsfreie Symbole entstehen. Bild-Ordner, Original-EXEs und Mitschnitte stehen in `.gitignore`.
5. Schreibtests an der Maus nur nach Absprache, vorher immer sichern (`m6980x_configtool backup`) und danach den Speicher
   bytegleich wiederherstellen (bisher jedes Mal gelungen; SHA-256 des Originalspeichers `44d6fbae…`, aktueller
   Stand `55906362…`, siehe „Toms Maus – aktueller Zustand“).
6. Kommunikation auf Deutsch.
7. **Name:** Der Nachbau heißt „M6980X Configuration Tool“ (Paket/Befehl `m6980x_configtool`), nie „ghost“, damit er nicht mit
   der offiziellen „Gigabyte GHOST“-Software verwechselt wird (umbenannt am 2026-10-11).

## Toms Maus – aktueller Zustand (nach allen Tests wiederhergestellt)

- Speicher wie `backups/rom_2026-10-11_original.bin`, mit diesen Ausnahmen (SHA-256 jetzt `55906362…`):
  - Tom hat das übrig gebliebene Bild des leeren Makros 6 entfernt (0x1CFC: `79` → `06`).
  - **Ältere Überbleibsel ohne Wirkung:** 0x1EDC–0x1EDF = `04 21 41 00` (Klick-Datensatz „Linksklick auf Rad“) und
    0x1F4D–0x1F4E = `DC 1E` (Tastentabelle Profil 1, Rad hoch). Stammen aus einem früheren Test in der Oberfläche
    (Linksklick auf Rad hoch, danach wieder Standard); die Firmware lässt die alte Adresse beim Zurücksetzen stehen.
    Rad hoch ist Standard. Nach Toms Entscheidung bewusst nicht bereinigt.
  - Hintere Seitentaste in Profil 2 und 3 auf Standard (Tom, Oberfläche, 2026-10-11 abends): Flags 0x1F10/0x1F1C
    `80` → `00`, die alte Adresse 0x1D98 bleibt in der Tastentabelle stehen.
- LED: Profil 1 weiß (2 2 2), Profil 2 grün (0 2 0), Profil 3 blau (0 0 2).
- DPI: 1620 / 3780 / 5850 (Stufe 4 intern 6030), 500 Hz.
- Tasten: vorne = Makro 1 „Klicker“ (Autoklicker, Modus Feuer, Bild 4 = Totenkopf) in allen drei Profilen, sonst überall
  Standard (Stand 2026-10-11 abends).
- Aktuelle Komplettsicherung: `backups/stand_2026-10-11_abend.json` (ältere: `vor_runde6_2026-10-11.json`,
  `vor_default_test_2026-10-11.json`, `ghost_sicherung_vor_werksreset.json`).

## Wichtigste Protokoll-Erkenntnisse (Kurzfassung, Details in PROTOCOL.md)

- Feature-Report ohne Report-ID auf **Interface 0**, 8 Bytes `[Gruppe, Befehl, Daten…]`, Handshake über Statusbits
  0x80 (busy) / 0x40 (done). Kein Unlock nötig.
- Gruppe 1 Register: `03/04` DPI, `05` Taste, `06` LED, `07` aktive Stufe, `08` Profil, `0B` Abtastrate (ms),
  `02` = **dauerhaft speichern** (ohne: Rate/DPI/Profil/Stufe nach Abstecken weg; LED bleibt immer).
- Gruppe 2 ROM (8 KB): lesen `00`, schreiben `01` (1 B), `02` (2 B), Puffer `03/05–09` + `04`, Seite `0A`.
- ROM: 20 Makro-Slots à 368 B ab 0x0000, Info-Tabelle 0x1CC1 (Name 9 B + Bild-Byte), Funktionsdatensätze ab 0x1D8C
  (feste Adressen laut INI), Tastentabelle der Firmware 0x1F3D + Profil·0x18 + Taste·2.
- Gerätetasten: 0 links, 1 rechts, 2 Rad-Klick, 3 hinten, 4 vorne, 5 – (keine Taste), 6/7 Kipprad L/R, 8/9 Rad hoch/runter.

## Offene Aufgaben

1. **Linux-Test** (Hauptziel!): `crates/m6980x-core/src/transport/linux.rs` nutzt `/dev/hidraw*` per ioctl (ohne hidapi),
   udev-Regel in `linux/`. Noch nie auf Linux gelaufen – zuerst nur lesend (`m6980x_configtool info`), Geräteerkennung
   prüfen (Interface 0 über sysfs). Kurze Reports (Rückgabe < 8 Bytes) werden wie „busy“ behandelt – ungetestet.
2. **OpenRGB-Fork reparieren** (`…\OpenRGB Projekt\OpenRGB-M6980X`, `Controllers/GigabyteGhostController`): spielt
   bei jedem Start die falsche 130-Paket-„Init“ ab (überschreibt DPI-Stufe 3 und Register 0B, aktiviert Profil 1),
   ignoriert den Busy/Done-Handshake, sendet LED-Werte 0–255 statt 0–2 und schaltet beim Farbwechsel das Profil um.
   Richtig wäre: nur `01 86 <Profil> R G B` mit Handshake, ohne Init.
3. **Eigene Symbole** per Bild-KI aus `assets/icon_descriptions.json` erzeugen und in der Oberfläche anzeigen.
4. Oberfläche verfeinern (Tom hat den Prototyp noch nicht beurteilt), evtl. Makro-Aufnahme wie im Original.
5. Kleinere offene Fragen (❓ in PROTOCOL.md): übriger Konfigbereich ab 0x1F85, Bedeutung der Bytes nach den Info-Einträgen.
   (Tasten-Flags 0x1F01 + Profil × 12 + Taste: geklärt 2026-10-11.)
6. **Systemweite Sperre beim Mauszugriff** – in Rust 2.0 umgesetzt (siehe oben), der Python-Prototyp hat sie nicht.
   Hintergrund (Toms Entscheidung 2026-10-11):
   Das Tool sperrt nur innerhalb eines Prozesses, genau wie das Original (`EnterCriticalSection` in `0x44494C`). Laufen zwei
   Instanzen gleichzeitig (z. B. offene Oberfläche mit Live-Anzeige + Kommandozeile, oder Tool + OpenRGB), vermischen sich
   die Busy/Done-Handshakes: Eine Seite bekommt die Antwort der anderen oder läuft in „keine Antwort auf …“. Zweimal am
   2026-10-11 beobachtet (Lesen von Taste 5, Komplettsicherung). Bisher harmlos, weil jeder Schreibvorgang zurückgelesen
   wird. Lösung: im Transport eine prozessübergreifende Sperre um jeden Befehl (Windows: benannter Mutex, Linux: `flock`
   auf eine Lockdatei oder das hidraw-Gerät). Bis dahin: Oberfläche schließen, bevor die Kommandozeile schreibt.

## Arbeitsumgebung (Toms PC)

- Rust 1.99 über rustup (`%USERPROFILE%\.cargo\bin`, in Git Bash ggf. `export PATH="$HOME/.cargo/bin:$PATH"`),
  Visual Studio 2022 Build Tools (VC-Tools) für den MSVC-Linker; Ziel `x86_64-unknown-linux-gnu` für `cargo check`
  installiert. `rustfmt.toml`: Zeilenbreite 120.
- Python 3.13 über `py -3.13` (python/python3 sind nur Store-Stubs). `hidapi` installiert.
- Wireshark/tshark `C:\Program Files\Wireshark\tshark.exe` und USBPcap installiert, nicht im PATH. Mitschnitt geht
  **ohne Adminrechte**, am besten aus PowerShell (Git Bash verstümmelt `\\.\USBPcap1`):
  `tshark -i \\.\USBPcap1 -i \\.\USBPcap2 -i \\.\USBPcap3 -w datei.pcapng`
- Disassembler: Python-Paket `capstone` (in einem eigenen venv installieren: `py -3.13 -m venv venv` +
  `venv\Scripts\pip install capstone`). Seit 2026-10-11 außerdem Ghidra 12.1.4
  (`C:\Users\Tom\Downloads\ghidra_12.1.4_PUBLIC_20260921\ghidra_12.1.4_PUBLIC`), Temurin JDK 21 und der REA-MCP-Server
  in Claude Code (`GHIDRA_INSTALL_DIR` in der MCP-Registrierung). REA braucht eine lokale Kopie außerhalb von OneDrive:
  `C:\Users\Tom\RE\GHOST\Macro.exe`. Alle Code-Aussagen in PROTOCOL.md sind damit gegengeprüft.
- Git Bash: das eingebaute `expand` überdeckt Windows' `expand.exe` – vollen Pfad nehmen.
- Original-Software ohne Installation starten (für Gegentests): `msiexec /a "<GHOST(6980X).msi>" TARGETDIR="<Ordner>" /qn`
  packt alle Dateien mit Ordnerstruktur aus (kein Autostart, keine Registry); dann
  `<Ordner>\Personal\GIGABYTE\GHOST(6980X)\Macro.exe` starten. Das MSI entsteht aus `6980X101.exe` mit `re_tools/isx.py`.
  Am 2026-10-12 so genutzt: Original zeigt von uns geschriebene Makros richtig an, Makrodateien bytegleich.
- Original-Software ist **deinstalliert** (Autostart weg). Der Ordner `Documents\GIGABYTE\GHOST(6980X)` blieb übrig.
  Nicht gleichzeitig mit dem Tool oder OpenRGB laufen lassen. Beim Start schreibt sie DPI-Stufe 1 (kurz mit falschem
  Y-Wert) und die Abtastrate neu.

## Werkzeuge in `re_tools/`

| Datei | Zweck |
|---|---|
| `isx.py`, `cfb_stream.py`, `msiq.vbs` | Installer → MSI → CAB entpacken, MSI-Tabellen abfragen |
| `pe.py`, `peimp.py`, `xref_imports.py`, `disasm_range.py`, `callers.py`, `strxref.py`, `methtab.py` | statische Analyse von `original_software/Macro.exe` (capstone) |
| `dfm_images.py`, `imagelist_extract.py`, `icon_sheets.py` | eingebettete Formulare und Bildlisten auslesen |
| `usbpcap_feat.py` | Feature-Reports aus USBPcap-Mitschnitten (mit Zeitstempeln) |
| `button_map_test.py`, `function_test.py` | Tests an der Maus in Profil 3 mit exakter Wiederherstellung |
| `rate_test.py` | echte Abtastrate per Mitschnitt messen (Maus bewegen) |
| `gui_smoke.py` | Oberfläche automatisch gegen die simulierte Maus testen |

## Weitere Quellen

- `docs/Mappingdefault_original.ini` – Funktionsliste und Datensatzgrößen der Original-Software.
- `captures/` – vier Mitschnitte der Original-Software (`*_feat.txt` = ausgewertete Feature-Reports), `.meb`-Beispiel.
- Handbuch: `C:\Users\Tom\Downloads\peripheral_m6980x_manual.pdf`. Installer-ZIP:
  `C:\Users\Tom\Downloads\peripheral_utility_engine_6980x_v1.03.zip`.
- Ältere Arbeit eines anderen Entwicklers (nur LEDs, mit Fehlern): `…\OpenRGB Projekt\GHOST_ReverseEngineering`.
