# M6980X Configuration Tool

Schlankes Konfigurationstool für die **Gigabyte GHOST M6980X** (USB `E0FF:0001`) als Ersatz für die
Gigabyte-Software. Es läuft unter Windows und Linux.

Inoffizieller Nachbau, nicht von Gigabyte. „GHOST“ meint in dieser Doku nur die Maus bzw. die offizielle
Gigabyte-Software („GHOST Macro Engine“, `Macro.exe`). Das Tool selbst heißt `m6980x_configtool`.

**Stand 1.1:** Alles ist am Gerät getestet: Profilwechsel, LED-Farben, DPI, Abtastrate, Tastenbelegung
(alle 46 Funktionen, Makro, Standard), Makros anlegen (auch Mausrad) und ROM-Schreiben. Tastennamen in Makros folgen dem
deutschen QWERTZ-Layout. Unter Linux ist das Tool noch nicht getestet.
Jeder Schreibvorgang wird zur Kontrolle zurückgelesen.
Abtastrate, DPI und aktives Profil werden automatisch dauerhaft gespeichert (`01 02`). Mit `--temporaer`
gelten sie nur bis zum Abstecken, `py -3 -m m6980x_configtool save` speichert später.
Neue Schreibfunktionen kommen erst nach einem Test am Gerät dazu.

## Installation

Windows:
```
py -3 -m pip install hidapi
```

Linux (kein hidapi nötig, Zugriff direkt über `/dev/hidraw`):
```
sudo cp linux/70-m6980x-configtool.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger
```

**Projektstand, Vorgaben und offene Aufgaben:** `docs/UEBERGABE.md` · Protokoll: `docs/PROTOCOL.md`

## Oberfläche

```
py -3 -m m6980x_configtool gui        # Linux: python3 -m m6980x_configtool gui
```
Schneller Prototyp (Tkinter, ohne Zusatzpakete) mit allen Funktionen in 5 Reitern: Übersicht (Profil,
DPI-Stufe, Abtastrate, LED mit Farbvorschau, Live-Anzeige der Maus-Tasten), DPI, Tasten, Makros (Editor mit
Bild und Byte-Zähler) und Sicherung (JSON, .meb, Freigeben). Alle Mauszugriffe laufen im Hintergrund, unten
protokolliert ein Log jeden Befehl.

## Benutzung (Kommandozeile)

Im Projektordner:
```
py -3 -m m6980x_configtool info                 # Gerät, aktives Profil, DPI-Stufen, LED-Farben
py -3 -m m6980x_configtool info --json
py -3 -m m6980x_configtool profile              # aktives Profil anzeigen
py -3 -m m6980x_configtool profile 2            # auf Profil 2 wechseln (1-3)
py -3 -m m6980x_configtool led                  # LED-Farben aller Profile
py -3 -m m6980x_configtool led 2 orange         # Farbe fuer Profil 2 (Name oder R G B je 0-2)
py -3 -m m6980x_configtool dpi                  # DPI-Stufen und aktive Stufe
py -3 -m m6980x_configtool dpi select 2         # Stufe 2 aktivieren
py -3 -m m6980x_configtool dpi set 1 1200       # Stufe 1 auf 1200 DPI (90er-Schritte, max. 6030)
py -3 -m m6980x_configtool dpi set 1 1800 1200  # X und Y getrennt
py -3 -m m6980x_configtool watch                # Profil/DPI-Stufe live beobachten
py -3 -m m6980x_configtool rate                 # Abtastrate anzeigen
py -3 -m m6980x_configtool rate 500             # 1000/500/333/250/200/166/142/125 Hz
py -3 -m m6980x_configtool buttons              # Tastenbelegung aller 3 Profile
py -3 -m m6980x_configtool functions            # belegbare Funktionen (bestaetigt / abgeleitet / unbekannt)
py -3 -m m6980x_configtool button 1 hinten kopieren    # Profil 1, hintere Seitentaste = Strg+C
py -3 -m m6980x_configtool button 1 vorne makro:3      # Taste auf Makro 3 legen
py -3 -m m6980x_configtool button 1 vorne standard     # Taste wieder auf Grundfunktion
py -3 -m m6980x_configtool macro 5 Test tippe strg+c, warte 100, klick links   # Makro 5 schreiben
py -3 -m m6980x_configtool macro 6 Auto klick links, warte 50 --mode feuer      # Modus einmal/schleife/feuer
py -3 -m m6980x_configtool macro 7 Zoom drueck strg, rad hoch, los strg         # Mausrad im Makro
py -3 -m m6980x_configtool macro 8 Kopie tippe strg+c --bild 12               # mit Vorschaubild 12
py -3 -m m6980x_configtool macro-bild 1 4                 # nur das Bild aendern (1-100 oder keins)
py -3 -m m6980x_configtool macro-export 1 klicker.ini      # Makro als Datei der Original-Software (.ini)
py -3 -m m6980x_configtool macro-import 5 klicker.ini --name Klicker   # Makrodatei des Originals einspielen
py -3 -m m6980x_configtool backup sicherung.json    # komplette Sicherung (Speicher + alle Einstellungen)
py -3 -m m6980x_configtool restore sicherung.json   # zurueckspielen (nur Abweichungen) und dauerhaft speichern
py -3 -m m6980x_configtool rom-dump sicherung.bin   # kompletten 8-KB-Speicher sichern (.meb = Format der Original-Software)
py -3 -m m6980x_configtool rom-import alt.meb       # Sicherung der Original-Software einspielen
py -3 -m m6980x_configtool macros               # Makro-Slots (oder: --rom sicherung.bin)
```
Unter Linux `python3` statt `py -3`.

Die Original-Software (`Macro.exe`, „GHOST“) und OpenRGB sollten nicht gleichzeitig laufen. Beide
sprechen die Maus über denselben Kanal an.

## Tests

```
py -3 -m unittest discover -s tests
```
Die Tests benutzen eine simulierte Maus und brauchen keine Hardware.

## Aufbau

| Pfad | Inhalt |
|---|---|
| `m6980x_configtool/transport.py` | Feature-Reports: hidapi (Windows) bzw. hidraw-ioctl (Linux) |
| `m6980x_configtool/protocol.py` | Handshake und Befehle |
| `m6980x_configtool/rom.py` | ROM-Layout (teilweise Hypothese) |
| `m6980x_configtool/cli.py` | Kommandozeile |
| `docs/PROTOCOL.md` | Protokoll mit Belegstufen |
| `assets/icon_descriptions.json` | Text zu jedem Bild (deutsch + englischer KI-Prompt, gemeinsamer Stil), Grundlage für eigene Symbole |
| `assets/embedded_icons/` | aus Macro.exe extrahierte Bilder, Makrobild 1–40 = `TFORM3_ImageList1/<n>.bmp` (nicht veröffentlichen) |
| `assets/original_icons/` | die 60 leeren Platzhalter für eigene Bilder (Makrobild 41–100) |
| `backups/` | ROM-Sicherung vom 2026-10-11, vor jeder Änderung |
| `original_software/` | entpackte Original-Programme zur Analyse (nicht veröffentlichen) |
| `re_tools/` | Analyse-Skripte: Installer entpacken, Disassembly, Mitschnitte auswerten |
