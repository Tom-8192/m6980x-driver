# M6980X Configuration Tool

*[English version](README.md)*

Schlankes Konfigurationstool für die **Gigabyte GHOST M6980X** (USB `E0FF:0001`) als Ersatz für die
Gigabyte-Software „GHOST Macro Engine“. Geschrieben in Rust. Es besteht aus zwei Programmen ohne Installation
und ohne Laufzeitumgebung:

* `m6980x_configtool`: Kommandozeile
* `m6980x_configtool_gui`: Oberfläche (egui)

Inoffizieller Nachbau, nicht von Gigabyte. „GHOST“ meint hier nur die Maus bzw. die offizielle Software
(`Macro.exe`).

**Stand 2.0 (Rust-Port des Python-Prototyps 1.1):** Alle Funktionen sind portiert: Profil, LED (27 Farben), DPI
(X/Y, 90–6030), Abtastrate, dauerhaft speichern, Tastenbelegung (46 Funktionen, Makros, Standard), Makros
(Editor, 3 Modi, Mausrad, Consumer-Tasten, Bilder 1–100), Sicherung (JSON), `.meb` und Makro-`.ini` des Originals.
Jeder Schreibvorgang wird zurückgelesen. Die Pakete sind bytegleich mit dem am Gerät getesteten Prototyp
(Paritätstests, siehe unten). Schreiben, Wiederherstellen und die neue Sperre sind unter Windows am Gerät getestet.
Neu gegenüber dem Prototyp ist eine **prozessübergreifende Sperre**: Oberfläche und Kommandozeile dürfen gleichzeitig
laufen. Unter Linux baut das Tool und besteht alle Tests, ist aber noch nicht an der Maus getestet.

**Sprachen:** Englisch und Deutsch. Die Oberfläche hat in den Einstellungen (unten links) eine Sprachauswahl (wird gemerkt), die
Kommandozeile nimmt `--lang en|de`. Ohne Angabe entscheidet `M6980X_LANG` oder die Systemsprache (deutsches System →
Deutsch, sonst Englisch). Eingaben gehen in beiden Sprachen, mit und ohne Umlaute: `button 1 hinten kopieren` wie
`button 1 back copy`, Makrotext `tippe strg+c, warte 100` wie `tap ctrl+c, wait 100`, `drück` wie `drueck`.

**Weitere Sprache:** Alle Texte (Ausgaben, Hilfe, Oberfläche, Tasten-, Farb- und Funktionsnamen) stehen in
`locales/<code>.yml`, eine Datei pro Sprache, beim Bauen eingebettet. `locales/en.yml` z. B. nach `locales/fr.yml`
kopieren, die Werte übersetzen, neu bauen – ohne Codeänderung. Platzhalter wie `%{name}` müssen bleiben; ein Eintrag
`"space|leer"` heißt Anzeigename plus weitere erlaubte Eingabe. `cargo test` prüft, dass jede Sprachdatei alle
Schlüssel und dieselben Platzhalter hat.

**Projektstand, Vorgaben und offene Aufgaben (englisch):** [`docs/HANDOVER.md`](docs/HANDOVER.md) · Protokoll (englisch):
[`docs/PROTOCOL.md`](docs/PROTOCOL.md)

## Bauen

Windows: [rustup](https://rustup.rs) und die Visual Studio Build Tools (Workload „Desktopentwicklung mit C++“).
```
cargo build --release
```
Die Programme liegen danach in `target\release\`.

Linux (Debian/Ubuntu): Rust per rustup, dazu für die Oberfläche
`sudo apt install libxkbcommon-dev libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev`.
Zugriff auf die Maus (`/dev/hidraw*`, ohne hidapi, der Kernel-Treiber bleibt aktiv):
```
sudo cp linux/70-m6980x-configtool.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger
```

## Oberfläche

```
m6980x_configtool_gui            # oder: m6980x_configtool gui
m6980x_configtool_gui --fake     # Probelauf mit simulierter Maus
m6980x_configtool_gui --lang de  # auf Deutsch starten
```
Fünf Reiter: Übersicht (Profil, DPI-Stufe, Abtastrate, LED mit Farbvorschau, Live-Anzeige der Maustasten), DPI
(mit Haken „X und Y synchron“), Tasten, Makros (Editor mit Bild und Byte-Zähler, .ini-Import/-Export) und Sicherung (JSON, .meb, Freigeben).
Alle Mauszugriffe laufen im Hintergrund. Unten protokolliert ein Log jeden Befehl.

## Kommandozeile

```
m6980x_configtool info                 # Gerät, aktives Profil, DPI-Stufen, LED-Farben (--json)
m6980x_configtool profile 2            # auf Profil 2 wechseln (1-3)
m6980x_configtool led 2 orange         # Farbe für Profil 2 (Name oder R G B je 0-2)
m6980x_configtool dpi                  # DPI-Stufen und aktive Stufe
m6980x_configtool dpi select 2         # Stufe 2 aktivieren
m6980x_configtool dpi set 1 1800 1200  # Stufe 1: X 1800, Y 1200 (90er-Schritte, max. 6030)
m6980x_configtool dpi default          # Werkswerte 810/1620/3240 und 500 Hz
m6980x_configtool watch                # Profil/DPI-Stufe live beobachten
m6980x_configtool rate 500             # 1000/500/333/250/200/166/142/125 Hz
m6980x_configtool buttons              # Tastenbelegung aller 3 Profile
m6980x_configtool functions            # belegbare Funktionen
m6980x_configtool button 1 hinten kopieren    # Profil 1, hintere Seitentaste = Strg+C
m6980x_configtool button 1 vorne makro:3      # Taste auf Makro 3 legen
m6980x_configtool button 1 vorne standard     # Taste wieder auf Grundfunktion
m6980x_configtool macro 5 Test tippe strg+c, warte 100, klick links   # Makro 5 schreiben
m6980x_configtool macro 6 Auto klick links, warte 50 --mode feuer      # Modus einmal/schleife/feuer
m6980x_configtool macro 8 Kopie tippe strg+c --icon 12                # mit Vorschaubild 12 (auch --bild)
m6980x_configtool macro-icon 1 4                 # nur das Bild ändern (1-100 oder keins; auch macro-bild)
m6980x_configtool macro-export 1 klicker.ini     # Makro als Datei der Original-Software
m6980x_configtool macro-import 5 klicker.ini --name Klicker
m6980x_configtool --lang en buttons              # Ausgabe auf Englisch
m6980x_configtool backup sicherung.json          # komplette Sicherung (Speicher + alle Einstellungen)
m6980x_configtool restore sicherung.json         # zurückspielen (nur Abweichungen), dauerhaft speichern
m6980x_configtool rom-dump sicherung.meb         # 8-KB-Speicher (.meb = Format des Originals, sonst roh)
m6980x_configtool rom-import alt.meb             # Sicherung der Original-Software einspielen
m6980x_configtool macros                         # Makro-Slots (oder: --rom datei.bin)
```
Abtastrate, DPI und aktives Profil werden automatisch dauerhaft gespeichert (`01 02`). Mit `--temporary` (auch `--temporaer`) gelten sie
nur bis zum Abstecken. `--device <Pfad>` wählt ein bestimmtes Gerät (hidapi-Pfad bzw. `/dev/hidrawN`).
Sicherungen des Python-Prototyps sind in beide Richtungen lesbar.

Die Original-Software (`Macro.exe`) und OpenRGB sollten nicht gleichzeitig laufen. Sie sprechen die Maus über
denselben Kanal an und kennen die Sperre dieses Tools nicht.

## Tests

```
cargo test --workspace
```
* Port aller Unit-Tests des Prototyps mit simulierter Maus (`crates/m6980x-core/src/fake.rs`)
* **Paritätstests:** `prototype/python/tools/export_fixtures.py` spielt Szenarien (alle Einstellungen, DPI, jede
  Funktion auf normaler und Radtaste, Makros, Freigeben, Wiederherstellen, Import) gegen den Python-Prototyp und
  schreibt `tests/fixtures/parity.json`. Rust muss dieselben 6423 Pakete senden, dasselbe ROM hinterlassen und
  bytegleiche `.meb`-/`.ini`-Dateien schreiben.
* Vorhandene Sicherungen lesen: `M6980X_BACKUP_DIR=<Ordner> cargo test -- --ignored`

## Aufbau

| Pfad | Inhalt |
|---|---|
| `crates/m6980x-core/` | Bibliothek: Transport (Windows hidapi, Linux hidraw, Sperre), Protokoll, ROM-Layout, Funktionen, Sicherung |
| `crates/m6980x-cli/` | Kommandozeile `m6980x_configtool` |
| `crates/m6980x-gui/` | Oberfläche `m6980x_configtool_gui` |
| `docs/PROTOCOL.md` | Protokoll mit Belegstufen (englisch) |
| `docs/HANDOVER.md` | Projektstand, Vorgaben, offene Aufgaben (englisch) |
| `assets/icon_descriptions.json` | Texte zu den Makrobildern (Grundlage für eigene Symbole, keine Gigabyte-Bilder) |
| `locales/` | Übersetzungen, eine YAML-Datei pro Sprache |
| `linux/` | udev-Regel |
| `prototype/python/` | Python-Prototyp 1.1 (Referenz, Quelle der Paritäts-Fixtures) |

## Lizenz

GPL-2.0-or-later (siehe [`LICENSE`](LICENSE)), passend zu OpenRGB. Die Gigabyte-Software und ihre Bilder sind nicht
Teil dieses Repos.
