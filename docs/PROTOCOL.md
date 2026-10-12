# Gigabyte GHOST M6980X – USB-Protokoll

Quellen: Disassembly von `Macro.exe` aus `GHOST(6980X).msi` (Version 1.01, aus `6980X101.exe`),
Lesetests an der echten Maus (2026-10-11) und USBPcap-Mitschnitte der Original-Software
(`GHOST_ReverseEngineering/ghost_init.pcapng`). Alle Code-Aussagen wurden mit Ghidra 12.1.4 (über REA)
gegengeprüft, siehe Abschnitt „Gegenprüfung mit Ghidra“ am Ende.

Belegstufen:
- ✅ **bestätigt** – am Gerät oder im Mitschnitt beobachtet und passend zum Code
- 🔍 **aus dem Code** – im Original-Programm gelesen, noch nicht am Gerät ausprobiert
- ❓ **Vermutung** – plausibel, aber offen

## Gerät

| | |
|---|---|
| USB-ID | `E0FF:0001`, Produktstring `G3` ✅ |
| Konfig-Kanal | Feature-Report **ohne Report-ID** auf Interface 0 (Maus), 8 Datenbytes ✅ |
| Report-Deskriptor | `09 00 15 80 25 7F 75 08 95 08 B1 02` (Ende der Maus-Collection) ✅ |
| Linux | `/dev/hidrawN` des Interface 0, `HIDIOCSFEATURE(9)` / `HIDIOCGFEATURE(9)` 🔍 |

Interface 1 (Vendor-Page `FF03`, Report-ID 1) wird von `Macro.exe` **nicht** benutzt ✅.

## Paketformat und Handshake ✅

Gesendet (ohne Report-ID): `[Gruppe, Befehl, Arg0 … Arg5]`, bei vielen Befehlen ist das letzte Byte `0x12`.

Antwort per GET_FEATURE: `[Status, Daten0 … Daten6]`

| Status-Bit | Bedeutung |
|---|---|
| `0x80` | beschäftigt – noch keinen neuen Befehl senden |
| `0x40` | Antwort fertig |
| `0x3F` | Gruppe der Antwort (`0x41` = Gruppe 1) |

Ablauf wie im Original (`0x44494C`, bei der Geräteerkennung die Kopie `0x444AAC`): GET bis Busy-Bit frei
(max. 20 × 4 ms) → SET → optional warten (Schreibbefehle 10 ms) → GET bis Done-Bit gesetzt (max. 200 × 4 ms).
Bei einer Wartezeit über 4 ms schläft das Original vor dem SET und am Ende zusätzlich je 1 ms. `0x44494C`
öffnet das Gerät für jeden Befehl neu (`CreateFileA`) und schließt es danach.

**Achtung:** Bytes, die ein Befehl nicht füllt, enthalten Reste früherer Antworten. Nur dokumentierte
Bytes auswerten. Das gilt auch beim Senden: Das Original setzt die Bytes zwischen den Argumenten und dem
abschließenden `12` nie. Dort steht Stack-Inhalt oder (bei Gruppe 2) die vorige Antwort, weil Sende- und
Empfangspuffer derselbe sind. Die `00` in den Tabellen unten sind daher Konvention, `m6980x_configtool` sendet dort Nullen.

## Gruppe 0 – System

| Befehl | Senden | Antwort | Stufe |
|---|---|---|---|
| Identifizieren | `00 00` | `0C 12 09 2D 00 03 01` – Original prüft `0C 12` (`0x44407C`). Es sucht die Maus **nicht** per VID/PID, sondern nimmt jedes HID-Gerät mit 9 Byte Feature-Report-Länge, das so antwortet | ✅ |
| Name | `00 01` | ASCII, `G3` | ✅ |
| Sitzung | `00 02 <Zähler>` | Zähler startet bei 3, +1 mod 254. Original sendet es beim Start. Für den Profilwechsel **nicht** nötig (getestet) | 🔍 |

**Modellerkennung des Originals** (`0x42EBD8`): Es sucht nicht nach USB-Kennungen, sondern nimmt jedes HID-Gerät mit
9-Byte-Feature-Report, das auf `00 00` mit `0C 12` antwortet. Das Modell folgt aus Datenbyte 6 und 2 der Antwort:

| Byte 6 | Byte 2 | Modell | Speicher / Profile / Makros / Info-Größe | DPI max (Schritt) |
|---|---|---|---|---|
| 0 | – | 6980 | 8 KB / 3 / 20 / 10 | 3000 (100) |
| 1 | 9 | **6980X** (Toms Maus: `0C 12 09 2D 00 03 01`) | 8 KB / 3 / 20 / 10 | 6030 (90) |
| 1 | ≠ 9 | M8000X | 32 KB / 5 / 70 / 14 | 6030 (90) |
| 4 | – | M8600 | 32 KB / 5 / 70 / 14 | 6030 (90) |
| sonst | | ignoriert | | |

Werte aus `Mappingdefault.ini`. Die USB-Kennungen der anderen Modelle stehen nirgends in der Software.
`m6980x_configtool` sucht fest nach `E0FF:0001`, Interface 0 (schickt also keine Befehle an fremde Geräte), und prüft
seit 2026-10-12 zusätzlich das Modell: Alle Befehle außer `info` brechen ab, wenn es nicht „6980X“ ist, weil die
Speicheraufteilung der anderen Modelle abweicht. `info` zeigt das erkannte Modell an.

## Gruppe 1 – Register (Befehl `N` = lesen, `N | 0x80` = schreiben)

| Reg | Lesen | Bedeutung | Schreiben (Original) | Stufe |
|---|---|---|---|---|
| `02` | `01 02 00` | unbekannt | – | ❓ |
| `03` | `01 03 00 <Stufe>` → `X-Schritt, Y-Schritt (0 = wie X), Einheit` | DPI-Stufe 0–3 (per Taste nur 0–2), Einheit 90 (meldet die Maus 0, rechnet das Original mit 100, `0x4457FC`). **Gilt für alle Profile gemeinsam** (am Gerät getestet) | `01 83 <0=X/1=Y> <Stufe> <Sensor-Schritt> 00 00 12` | ✅ Lesen und Schreiben |
| `03` | `01 03 01` → 7 Byte Bitmaske | unterstützte Schritte 1–56 | – | ✅ |
| `04` | `01 04 00 <Stufe>` → `Xz, Xn, Yz, Yn` | Feinskalierung: DPI = Schritt × 90 × z / n | `01 84 <0=X/1=Y> <Stufe> <z> <n> 00 12` | ✅ Lesen und Schreiben |
| `05` | `01 05 <Profil> <Taste>` → `Flags, Wert lo, Wert hi` | Tastenbelegung. Flags `0x80` = belegt, Wert = ROM-Adresse des Aktions-Datensatzes. GUI-Tasten 3 und 4 sind auf dem Gerät vertauscht (das Original tauscht sie in `0x445398`) | `01 85 <Profil> <Taste> <Typ> <lo> <hi> 12` | ✅ Lesen / 🔍 Schreiben |
| `06` | `01 06 <Profil>` → `R, G, B` | Farbe der RGB-LEDs pro Profil, je Kanal 0 = aus, 1 = mittel, 2 = voll. Die Anzahl leuchtender LEDs (1–3) zeigt die DPI-Stufe. Am Gerät gesetzt und bestätigt (rot = `2 0 0`), ändert den 8-KB-ROM nicht | `01 86 <Profil> <R> <G> <B>` | ✅ |
| `07` | `01 07` | aktive DPI-Stufe ab 0. Die DPI-Taste schaltet 0 → 1 → 2 → 0, es leuchten Stufe + 1 LEDs (am Gerät beobachtet). Stufe 3 (4. Eintrag in Reg 03/04) erreicht die Taste nicht. Beim Profilwechsel bleibt die Stufe erhalten | `01 87 <Stufe> 00 00 00 00 12` | ✅ Lesen und Schreiben |
| `08` | `01 08` | aktives Profil 0–2. Die Profil-Taste schaltet 0 → 1 → 2 → 0 (beobachtet) | `01 88 <Profil> 00 00 00 00 12` – ✅ am Gerät getestet: ohne Init-Sequenz, ~11 ms pro Wechsel, ändert den 8-KB-ROM nicht | ✅ |
| `0A` | `01 0A` → 16 Bit | `0x1F00`, Adresse des Konfigbereichs. Das Original liest beim Start ROM ab dieser Adresse (`0x431535`). Ist das erste Byte nicht `0x12`, schreibt es `02 01 00 1F 00 12` | – | 🔍 Code / ✅ ROM-Dump hat `12` |
| `0B` | `01 0B` | **USB-Abfrageintervall in ms** (1–8, Abtastrate = 1000 / Wert Hz). Gilt für alle Profile | `01 8B <Wert>` | ✅ Mitschnitt: 500 → 333 Hz = `01 8B 02` → `01 8B 03` |

## Gruppe 2 – ROM (8 KB, gespiegelt ab 0x2000) ✅

| Befehl | Senden | Bedeutung | Stufe |
|---|---|---|---|
| Lesen | `02 00 <lo> <hi> 00 00 00 12` | Antwort: 7 Bytes ab Adresse | ✅ |
| 1 Byte schreiben | `02 01 <lo> <hi> 00 <Wert> – 12` | | 🔍 |
| 2 Bytes schreiben | `02 02 <lo> <hi> 00 <W0> <W1> 12` | | 🔍 |
| Puffer füllen | `02 03` / `05` / `06` / `07` / `08` / `09` + je 6 Bytes (09: 2 Bytes) | Puffer-Byte 0–5, 6–11, … 30–31 | 🔍 |
| Puffer schreiben | `02 04 <lo> <hi> 00 <Länge> – 12` | n Bytes (≤ 32) an Adresse | 🔍 |
| Seite schreiben | `02 0A <lo> <hi> 00 – – 12` | 32 Bytes aus dem Puffer, 5 ms warten (`0x444F44`) | 🔍 |

Die Schreibfunktion des Originals (`0x444C30`) wählt nach Länge: 1 Byte → `02 01`, 2 Bytes → `02 02` (+1 ms),
sonst Puffer + `02 04`. Deshalb erscheinen beim Makro-Schreiben einzelne `02 01`-Befehle. Die Puffer-Befehle
warten je 1 ms, `02 09` 0 ms.

## Gruppe 5

`05 00 0C 12` → Antwort-Byte 1 = `03`. Bedeutung unbekannt ❓

## ROM-Layout (aus Dump und Start-Lesezugriffen)

| Bereich | Inhalt | Stufe |
|---|---|---|
| `0x0000–0x1CBF` | 20 Makro-Slots zu je `0x170` Bytes | ✅ Adressen / ❓ Format |
| `0x1CC1–0x1D88` | Makro-Info-Tabelle, 20 × 10 Bytes, Eintrag 0 = „Klicker“ | ✅ Adressen / ❓ Format |
| `0x1D89–0x1D8B` | 3 ungenutzte Bytes (Lücke im Code: Info-Ende + 3) | 🔍 |
| `0x1D8C–0x1EEF` | 51 Funktions-Datensätze, Größen aus `[ListMemSize]`, siehe „Speicheraufteilung aus dem Code“ | 🔍 |
| `0x1F00` | Kennung `0x12` (vom Original geprüft und notfalls geschrieben), siehe Register `0A` | 🔍 |
| `0x1F01–0x1F24` | Tasten-Flags: `0x1F01 + Profil × 12 + Taste` = `80` belegt / `00` Standard (12 Bytes je Profil, 10 Tasten + 2 frei) | ✅ |
| `0x1F3D–0x1F84` | Tastentabelle: `0x1F3D + Profil × 0x18 + Taste × 2` = Adresse des Datensatzes | ✅ |
| übriger Konfigbereich bis `0x1FFF` | unbekannt | ❓ |

Makro-Ereignisse (vorläufig): `83 xx` Taste drücken, `84 xx` Taste loslassen (xx = HID-Usage, z. B.
`E0` = Strg links, `19` = V), `85 lo hi` Consumer-Taste (`00E9` = Lauter, `00EA` = Leiser), `00` = Ende.

## Modellwerte (`Mappingdefault.ini`, Abschnitt 6980)

Speicher 8192 · Profile 3 · Tasten 10 · Makros 20 · MacroInfoSize 10 ·
DPI-Standardwerte 810 / 1620 / 3240 / 6030 · Schrittweite 90 · Maximum 6030

## Korrekturen zur älteren Doku (`GHOST_Mouse_Protocol.md`)

- Der „Flush“ ist der Busy/Done-Handshake. Ohne Prüfung der Statusbits entstehen Timing-Probleme.
- Die „Init-Sequenz“ ist der aufgezeichnete Start der Original-Software (Lesebefehle). Am Ende stehen
  Schreibbefehle: DPI-Stufe 3 = 5850, Register 0B = 2, Profil 1 aktiv. Beim Abspielen werden diese
  Einstellungen überschrieben.
- `02 00 <Wert>` ist ROM-Lesen, kein Schreiben der Abtastrate. `01 08` ist das aktive Profil.
- Die „Commit-Befehle“ sind `01 0B` (lesen) und `01 8B 02` (Register 0B fest auf 2 setzen).

## Verhalten beim Tastendruck ✅

Während die DPI- oder Profil-Taste gedrückt wird, liefert GET_FEATURE gelegentlich einen verkürzten
Report (1 Byte). Das Original zählt einen fehlgeschlagenen GET_FEATURE als „noch nicht bereit“ und fragt
erneut ab. `m6980x_configtool` macht es genauso.

## DPI schreiben (Port von `0x445900`) ✅

Gewünschter Wert in Schritten `w = DPI / 90`. Pro Achse:
1. Sensor-Schritt `s` = kleinster unterstützter Schritt ≥ `w`, sonst der größte (Maske aus `01 03 01`, hier 1–56).
2. Bruch `w / s`, gekürzt.
3. Senden: `01 83 X`, `01 83 Y` (Sensor-Schritte), dann `01 84 X`, `01 84 Y` (Brüche), je 10 ms.

Sonderfall im Original: Sind X und Y nicht getrennt eingestellt (Flag `+0x898` = 0, gesetzt beim Lesen,
wenn die Maus Y-Schritt 0 meldet), nehmen beide Achsen den Sensor-Schritt von max(X, Y). Bei gleichem X und Y
macht das keinen Unterschied.

Beispiel 5850 DPI: `w` = 65 → `s` = 56, Bruch 65/56. Das ist bytegleich mit dem Mitschnitt der Original-Software.
Am Gerät getestet: Stufe 4 auf 4500 und zurück auf 6030, Rohwerte danach identisch. Der 8-KB-ROM bleibt
bei Profil-, LED- und DPI-Änderungen unverändert. Diese Werte liegen also woanders, vermutlich im internen
Speicher des Mikrocontrollers.

## Mitschnitt mit der Original-Software (2026-10-11, `captures/original_software.pcapng`) ✅

Tom hat nacheinander die Abtastrate 500 → 333 Hz gestellt, die vordere Seitentaste in Profil 1 auf „Strg“
gelegt und Makro 3 „Hallo-Wel“ angelegt (T drücken und halten).

**Start der Software:** wie im alten Mitschnitt nur Lesebefehle. Danach schreibt sie DPI-Stufe 1 neu,
zuerst mit einem falschen Y-Schritt `01` (90 DPI), dann korrigiert. Das ist ein Fehler der Original-Software.

**Abtastrate:** `01 8B 03`.

**Taste auf Tastaturtaste legen** (GUI-Taste 3 = Gerätetaste 4):
```
02 03 06 83 E0 FF FF 00      Puffer[0..5] = 06 83 E0 FF FF 00   (Datensatz: 06 = Tastenbelegung, 83 E0 = Strg)
02 04 64 1E 00 06 .. 12      6 Bytes nach ROM 0x1E64 schreiben
01 85 00 04 80 64 1E 12      Profil 0, Gerätetaste 4, Flags 0x80, Datensatz 0x1E64
01 88 00 ...                 Profil 0 neu aktivieren
```
Die Firmware trägt die Adresse zusätzlich in die Tastentabelle ab ROM 0x1F00 ein (0x1F45 = `64 1E`).

**Makro** (Slot 2 = „Makro 3“, Adresse 0x02E0):
```
02 01 E0 02 00 04 ..  12     0x02E0 = 04   (Makro-Datensatz)
02 01 E1 02 00 83 ..  12     0x02E1 = 83   (Taste drücken)
02 01 E2 02 00 17 ..  12     0x02E2 = 17   (HID-Usage „T“)
02 01 E3 02 00 00 ..  12     0x02E3 = 00   (Ende, kein Loslassen = gedrückt halten)
02 01 DE 1C 00 03 ..  12     Info-Eintrag Slot 2, Byte 9 = 03
02 03 'Hallo-' / 02 05 'Wel' + 3 Null-Bytes / 02 04 D5 1C 00 09   Name (max. 9 Zeichen) nach 0x1CD5
```
Info-Eintrag = 9 Bytes Name + 1 Byte. Das letzte Byte ist bei Slot 1–4 gleich Slot + 1, bei Slot 0 und 5 nicht
(0x68, 0x79) ❓.

**Datensatz-Typen (erstes Byte):** `04` = Ereignisfolge (Makro), `06` = Tastenbelegung mit Taste,
gefolgt von Ereignissen `83 xx` drücken, `84 xx` loslassen, `85 lo hi` Consumer-Taste, `00` Ende.
Die Bedeutung von `21 41`, `24 89` und `88 0A` ist noch offen ❓.

## Mitschnitt 2 und Funktionstabelle ✅

**Funktions-Datensätze liegen an festen Adressen.** `Mappingdefault.ini` (Kopie in `docs/`) enthält
in `[List]` 46 Funktionen und in `[ListMemSize]` ihre Größe. Funktion *i* liegt bei
`0x1D8C + Summe(Größe[0..i-1])`. Bestätigt für Mittelklick 0x1D98, Strg+C 0x1DD8, Strg+V 0x1DE2,
Lauter 0x1E46 und Strg 0x1E64. Die Original-Software schreibt beim Zuweisen den Datensatz (neu) an
diese Adresse und setzt dann `01 85 <Profil> <Gerätetaste> 80 <Adresse>`. Ein Makro wird genauso zugewiesen,
nur mit der Slot-Adresse (`80 E0 02` = Slot 2).

**Datensatz-/Makro-Format:** Byte 0 = Modus, danach Ereignisse, `00` = Ende.

| Byte 0 | Modus | Beleg |
|---|---|---|
| `04` | einmal ausführen | ✅ Makro „XY“ |
| `05` | Schleife | ✅ Makro „XY“ |
| `06` | Feuertaste / solange gedrückt | ✅ Makro „XY“, Tastenbelegung Strg, Mittelklick |

| Ereignis | Bedeutung | Beleg |
|---|---|---|
| `83 uu` / `84 uu` | Taste drücken / loslassen, HID-Usage `uu` (`04`=A, `06`=C, `17`=T, `19`=V, `E0`=Strg, `E3`=Win) | ✅ |
| `85 lo hi` | Consumer-Taste (`00E9` lauter, `00EA` leiser, `0000` = loslassen) | ✅ |
| `88 nn` | warten `nn` ms | ✅ 100 ms = `88 64` |
| `89 lo hi` | warten 16 Bit ms | ✅ 500 ms = `89 F4 01` |
| `2m` / `4m` | Maustaste drücken und halten / loslassen, Maske `m` (1 = links, 2 = rechts ❓, 4 = mitte) | ✅ Doppelklick `21 41 21 41`; im Makro „XY“ laut Tom `21` = links drücken und halten, `24` = mitte drücken und halten |
| `7F` / `61` | Mausrad (Zoom In / Zoom Out mit Strg) | ❓ Richtung |

**Makro „XY“** in Slot 18 (Adresse 0x19E0): `04 83 04 88 64 84 04 89 F4 01 21 24 00`, Info-Byte 9 = `13`
(= Slot + 1), Name „XY“. Moduswechsel überschreibt das ganze Makro mit Byte 0 = 05 bzw. 06.

**Gerätetasten** ✅ (am Gerät ermittelt: in Profil 3 tippte jede Taste per Makro ihre eigene Nummer):

| Nr. | Taste | „Standard“ sendet an den PC ✅ |
|---|---|---|
| 0 | links | Maustaste 1 (Linksklick) |
| 1 | rechts | Maustaste 2 (Rechtsklick) |
| 2 | Mausrad drücken | Maustaste 3 (Mittelklick) |
| 3 | hintere Seitentaste | Maustaste 4, Bit `0x08` („Zurück“) |
| 4 | vordere Seitentaste | Maustaste 5, Bit `0x10` („Vor“) |
| 5 | – (keine Taste ausgelöst, in der INI „High“) | – |
| 6 | Kipprad links | waagerechter Bildlauf −1 (AC Pan, Byte 6), ein Schritt pro Druck |
| 7 | Kipprad rechts | waagerechter Bildlauf +1 |
| 8 | Rad nach oben | Rad +1 pro Raste (Byte 5) |
| 9 | Rad nach unten | Rad −1 pro Raste |

Die Standard-Spalte ist aus den Eingabe-Berichten gemessen (`captures/standard_tasten*.pcapng`, 2026-10-11).
CLI und Oberfläche zeigen „Standard“ seitdem mit dieser Wirkung an, z. B. „Standard (Zurueck, Maustaste 4)“.

DPI- und Profil-Taste sind nicht umbelegbar. Die GUI zählt anders (Seitentasten vertauscht). `m6980x_configtool` verwendet
die Gerätenummern. Dabei wurde auch der `macro`-Weg (10 Makros schreiben, zurücklesen, zuweisen) am Gerät bestätigt.

**Abtastrate:** Beim Start schreibt die Software den aktuellen Wert `01 8B 03` zurück.

## Wiederherstellung mit `m6980x_configtool` (2026-10-11) ✅

Mit diesem Tool am Gerät bestätigt:
- `01 8B 02` → Abtastrate 500 Hz (zurückgelesen).
- `01 85 <Profil> <Taste> 80 <lo> <hi> 12` → Taste auf Makro bzw. Funktion (zurückgelesen).
- `01 85 <Profil> <Taste> 00 FF FF 12` → **Taste auf Standard** (Rücklesen: Flags 00). Die Original-Software
  bietet das nicht an.
- ROM-Schreiben per Puffer (`02 03`/`05` + `02 04`) und Einzelbyte (`02 01`), mehrere Bereiche, jeweils
  zurückgelesen. Danach war der 8-KB-ROM wieder bytegleich mit der Sicherung (SHA-256 44d6fbae…).

Tastentabelle der Firmware: `0x1F3D + Profil × 0x18 + Taste × 2` (2 Bytes Adresse pro Taste, bestätigt für
Profil 0 und 2). Beim Zurücksetzen auf Standard bleibt die alte Adresse dort stehen, aktiv ist
sie dann nicht mehr. Die Flags stehen bei `0x1F01 + Profil × 12 + Taste` (`80` = belegt, `00` = Standard), genau das,
was `01 05` als Flags-Byte meldet. Bestätigt am 2026-10-11 für alle 30 Tasten, nachdem Tom die hintere Seitentaste in
Profil 2 und 3 auf Standard gesetzt hatte: 0x1F10 und 0x1F1C gingen von `80` auf `00`, die Adressen in der Tabelle blieben.

## Abgeleitete Funktionen am Gerät getestet (2026-10-11) ✅

Alle 14 aus dem Format abgeleiteten Datensätze funktionieren (in Profil 3 zugewiesen, von Tom ausprobiert,
danach ROM bytegleich wiederhergestellt): Linksklick `06 21 89 FF FF 00`, Rechtsklick `06 22 …`,
Strg+Z/X/N/W/O nach dem Muster `04 83 E0 83 uu 84 uu 84 E0 00`, Stumm/Play-Pause/Vorheriger/Nächster Titel
als Consumer `04 85 lo hi 85 00 00 00` (`00E2`, `00CD`, `00B6`, `00B5`), Shift/Alt/Enter als `06 83 uu FF FF 00`.
Damit ist auch Maske 2 = rechte Maustaste bestätigt.

**Tastaturlayout:** HID-Usages sind Tastenpositionen einer US-Tastatur. `m6980x_configtool` benennt Tasten nach dem
deutschen QWERTZ-Layout (Y und Z vertauscht). Rückgängig sendet deshalb Usage `1C` und funktionierte unter
deutschem Windows. Unbekannt sind noch 16 Funktionen (Browser vor/zurück, Bildlauf, Minimieren/Maximieren,
Programm schließen, Internet/Favoriten/E-Mail, vorheriges Fenster, 2×/4×/8× Geschwindigkeit).

## Weitere Funktionen am Gerät getestet (2026-10-11) ✅

Aus der Datensatzgröße abgeleitet und von Tom ausprobiert: Browser zurück/vor (Maustasten-Maske 8 / 16),
Bildlauf hoch/runter (`04 61 00` / `04 7F 00`), Minimieren/Maximieren (Alt+Leertaste, dann N bzw. X, deutsches
Fenstermenü), Programm schließen (Alt+F4), vorheriges Fenster (Alt+Tab), Internet (Consumer `0196`),
Favoriten (`022A`) und E-Mail (`018A`).

**Mausrad-Ereignis:** `0x60 | Delta` mit vorzeichenbehaftetem 5-Bit-Delta: `61` = 1 Raste hoch, `7F` = 1 Raste
runter (am Gerät beobachtet, Werte über 1 sind nicht getestet). Folge: Die Original-Datensätze „Zoom In“
(`… 7F …` = Strg+Rad runter) und „Zoom Out“ (`… 61 …`) sind vertauscht, in den meisten Programmen zoomt
„Zoom In“ heraus.

**Noch unbekannt:** Bildlauf links/rechts (Größe 0 – kein Datensatz) und 2×/4×/8× Geschwindigkeit (Größe 2).

## Mitschnitt 3: letzte 5 Funktionen (2026-10-11) ✅

- **2×/4×/8× Geschwindigkeit:** 2-Byte-Datensätze `C8 00` (0x1EC4), `CA 00` (0x1EC6), `CE 00` (0x1EC8),
  geschrieben mit dem 2-Byte-Befehl `02 02 <lo> <hi> 00 <b0> <b1> 12`, dann `01 85 02 03 80 <lo> <hi> 12`.
  Wirkung laut Handbuch: Solange die Taste gehalten wird, bewegt sich die Maus 2×/4×/8× so schnell,
  beim Loslassen endet die Funktion. Mit `m6980x_configtool` geschrieben und am Gerät bestätigt (2×, 4×, 8×).
- **Bildlauf links/rechts:** kein Datensatz (Größe 0). Die Software bietet sie nur auf dem Kipprad selbst an
  und schreibt dann `01 85 <Profil> <Taste> 00 FF FF 12`. Das ist bytegleich mit dem „Standard“ von `m6980x_configtool`.
- Nebenbei: „Bildlauf zurück“ wurde als `04 7F 00` geschrieben. Das bestätigt die Rad-Richtung aus Herstellersicht.

Damit waren 43 Funktionen bekannt. Die übrigen drei (43–45) stehen im Code des Originals, siehe „Funktions-Datensätze
aus dem Code“ am Ende.

## Makro-Vorschaubilder ✅ (Code + ROM)

Byte 9 des Makro-Info-Eintrags (`0x1CC1 + Slot × 10 + 9`) ist das **Bild-Byte**, nicht „Slot + 1“:
- Neues Makro (`ListBox2DragDrop`, Aufruf von `0x4133A0`): Wert = Slot + 1 → kein Bild gewählt.
- „Change Icon“ (`Image111Click` → `0x4133A0`): Wert = 100 + n für Bild n der 100er-Bildauswahl.
- Gezeichnet wird mit Bildlisten-Index = Wert − 100 (`ListBox2DrawItem`, 0x40FB61) aus der Bildliste der
  Bildauswahl (`TForm3.ImageList1`). Diese enthält **41 in Macro.exe eingebettete Bilder** (Index 0 =
  Hintergrundmuster, 1–40 = Symbole wie Fernglas, Totenkopf, Pistole). Zur Laufzeit hängt 0x43EAF0
  `bmp\1.bmp` … `bmp\60.bmp` als Index 41–100 an. Diese 60 Dateien sind ausgelieferte **leere Platzhalter**
  für eigene Bilder („Change Icon“ kopiert eigene Bilder über `temp.bmp` dorthin).
- Ohne gewähltes Bild (Wert = Slot + 1) zeigt die Software „MACRO n“ aus `TForm1.ImageList2`.
- ✅ Bestätigt von Tom: „Klicker“ (0x68 → Index 4) hatte das Bild `TForm3.ImageList1` Index 4 (Totenkopf).
  Der alte Eintrag im leeren Slot 6 (0x79) war Index 21. Tom hat das Bild entfernt, danach stand dort `06`
  (= Slot + 1), und das war die einzige Änderung im ROM.
- Extrahierte Bilder: `assets/embedded_icons/` (`re_tools/imagelist_extract.py`, Übersicht `uebersicht.html`).

`m6980x_configtool macro` behält ein gewähltes Bild beim Überschreiben bei, `--bild N` setzt eines, und
`m6980x_configtool macro-bild SLOT N|keins` ändert nur das Bild.

## Speichergrenzen für Makros

20 Slots mit fester Größe von je 368 Bytes (0x170), kein Teilen zwischen Slots. Pro Slot sind 366 Bytes für
Ereignisse nutzbar (Modus-Byte + Ende-Byte gehen ab). Ereignisgrößen: Taste drücken/loslassen 2, warten
2 (≤ 255 ms) oder 3, Consumer 3, Maustaste und Rad 1. Ein vollständiger Tastendruck braucht 4 Bytes.
Der Name hat höchstens 9 Zeichen. Der Original-Editor hat intern `Struct_Macro[100]`, vermutlich also höchstens
100 Ereignisse pro Makro ❓.

## Abtastrate gemessen und Register 02 (2026-10-11) ✅

`re_tools/rate_test.py` zählt die Interrupt-Berichte der Maus per USBPcap, während sie bewegt wird:

| Zustand | Median-Abstand | Rate |
|---|---|---|
| Register 0B = 2 | 2,00 ms | 500 Hz |
| Register 0B = 8, ohne `01 02` | 8,00 ms | 125 Hz |
| danach `01 02 00 … 12` | 8,00 ms | 125 Hz |
| Register 0B = 2 + `01 02` | 2,00 ms | 500 Hz |

- **Register 0B wirkt sofort.** Der USB-Deskriptor meldet unabhängig davon immer `bInterval = 1`.
- **Gruppe 5** `05 00 0C 12` → 3 ist die **Anzahl der Profile** (der Code nutzt sie als Schleifengrenze
  über die 164-Byte-Profilstrukturen).
- **Register 02** (`01 02 00 … 12`, 4 ms, `0x445C5C`): Die Original-Software sendet es nur beim Verlassen der
  Einstellungsseite (`0x44B3BC`), wenn die Abtastrate geändert oder der Default-Knopf gedrückt wurde
  (Flag `+0x3AC`). Antwort sofort, keine Wirkung auf die Rate.
  Vermutung ❓: dauerhaft speichern (Einstellungen außerhalb des 8-KB-ROM).

## Register 02 = dauerhaft speichern ✅ (Abstecktests 2026-10-11)

| Geändert ohne `01 02` | nach Ab-/Anstecken | mit `01 02` |
|---|---|---|
| LED-Farben (Reg 06) | erhalten | erhalten |
| Abtastrate (Reg 0B) | verloren (zurück auf 500 Hz) | erhalten |
| DPI-Werte (Reg 03/04) | verloren | erhalten |
| aktives Profil (Reg 08) | zurück auf Profil 1 | erhalten |
| aktive DPI-Stufe (Reg 07) | zurück auf Stufe 1 | erhalten |

Gespeichert wird im internen Speicher des Mikrocontrollers, der 8-KB-ROM bleibt unverändert.
`m6980x_configtool` speichert nach `rate`, `dpi set/select`, `profile` und `restore` automatisch (abschaltbar mit `--temporaer`).
Die Init-Sequenz des alten Skripts und des OpenRGB-Forks sendet `01 02` nie. Deren DPI- und Abtastraten-
Änderungen gehen deshalb beim Abstecken verloren.

## Mitschnitt 4: Sicherung, Wiederherstellen, Standard, Freigeben, Editor-Grenze (2026-10-11) ✅

| Aktion der Original-Software | Befehle |
|---|---|
| Sicherung in Datei | 1171 × `02 00` (ganzer ROM in 7-Byte-Stücken), ca. 75 s |
| Wiederherstellen | alle 256 Seiten 0x0000–0x1FE0: `02 03`/`05`/`06`/`07`/`08`/`09` (Puffer) + `02 0A <lo> <hi> 00 … 12`, ca. 35 s |
| „Standard“ | `01 85 <Profil> <Taste> 00 00 00 12` für die 10 Tasten eines Profils |
| „Freigeben“ | Byte 0 jedes der 20 Makro-Info-Einträge = 00 (Name leer) und `01 85 … 00 00 00 12` für alle 30 Tasten. **Kein** Reset von DPI, LED oder Abtastrate, Makrodaten bleiben stehen. |
| Makro-Editor-Grenze | ein aufgenommenes Makro füllte 365 von 368 Bytes (0x0000–0x016C). Der Editor hört an der Slotgröße auf, nicht bei 100 Ereignissen. |

**Sicherungsdatei `.meb`:** INI-Text mit `[Mem]`, `count=8192` und je einer Zeile `<Adresse>=<Wert dezimal>`, CRLF.
Enthält nur den 8-KB-ROM. `m6980x_configtool rom-dump datei.meb` schreibt das Format bytegleich (verglichen mit einer
Datei der Original-Software), `m6980x_configtool rom-import datei.meb` liest es.

`rom-import` spielt seit 2026-10-12 auch die **Tastenbelegung** aus der Datei ein: Flag `0x1F01 + Profil × 12 + Taste`
= `80` → `01 85` auf die Adresse aus der Tastentabelle, sonst Standard. Vorher wurden nur Speicher und Tabelle
geschrieben, die Flags fehlten, sodass Belegungen nicht aktiv wurden. Unterschied zum Original: Dessen
„Wiederherstellen“ schreibt alle 256 Seiten neu (~35 s), `rom-import` nur die abweichenden Bereiche und Tasten.
✅ Am Gerät getestet: echte Original-Sicherung `captures/original_sicherung/original.meb` eingespielt → hintere Taste
in Profil 2/3 wieder Mittelklick, Speicher danach **bytegleich** mit der Datei (`44d6fbae…`, inkl. der von der Firmware
gesetzten Flags); anschließend Toms Stand bytegleich wiederhergestellt.

## Abgleich mit dem Handbuch (peripheral_m6980x_manual.pdf)

- **LED-Farben:** Das Farbfenster bietet 7 Farben + aus. Stufen aus `Macro.exe` (0x451000–0x45115E):
  aus 0 0 0, blau 0 0 2, grün 0 2 0, hellblau 0 2 2, hellgrün 1 1 0, lila 2 0 1, gelb 2 1 0, rot 2 0 0.
  Die Maus nimmt alle 27 Kombinationen an, `m6980x_configtool led` kennt die 8 Original-Namen und weitere.
- **Werkseinstellung DPI:** 810 / 1620 / 3240 / 6030. So schreibt sie der „Default“-Knopf (0x44AD64): Werte aus
  `[6980DPIdefault]`, Schritt 90, und zwar über dieselbe DPI-Funktion 0x445900. Das Handbuch nennt 400–6000. Im Code ist 400 nur
  der Ersatzwert, falls die INI fehlt (für `step` ist der Ersatzwert 100), eine Untergrenze gibt es dort nicht,
  und die Maus meldet Schritt 1–56 als unterstützt. `m6980x_configtool` folgt dem Code: 90–6030.
  Der Default-Knopf schreibt außerdem alle **4** Stufen (auch die unbenutzte Stufe 4 = 6030), setzt die
  **Abtastrate auf 500 Hz** (`01 8B 02`) und markiert die Seite als geändert, sodass beim Verlassen `01 02`
  folgt (`0x44B2F7–0x44B391`). `m6980x_configtool dpi default` (und der Knopf in der Oberfläche) setzt Stufe 1–3 und 500 Hz
  und speichert. Stufe 4 bleibt nach Toms Vorgabe unberührt. ✅ Am Gerät getestet (2026-10-11): 250 Hz →
  `m6980x_configtool dpi default` → 500 Hz und 810/1620/3240, danach Sicherung bytegleich wiederhergestellt.
- **LED:** 3 Stufen je Kanal ergeben 27 Farben (alle von der Maus angenommen), das Original bietet davon nur 8.
- **Abtastrate:** Handbuch 125/250/500/1000 Hz. Der Regler der Software kennt auch 333 Hz usw. (1–8 ms).
- **9 belegbare Tasten** (7 + 2) = Gerätetasten 0–4 und 6–9.
- **Scrollgeschwindigkeit / Tilt.exe** („Areson Tilt“; Areson ist der OEM, daher Hersteller „A.....“): ein reines
  PC-Hilfsprogramm aus der Windows-XP-Zeit, das dort horizontales Scrollen per Kipprad nachrüstet. Es liest Raw Input
  (`RegisterRawInputDevices`/`GetRawInputData`), sucht die Bildlaufleiste unter dem Zeiger (`WindowFromPoint`,
  `EnumChildWindows`, Klassen „ScrollBar“/„NUIScrollbar“/„TScrollBar“) und schickt ihr per Timer Scroll-Nachrichten.
  Die Einstellungen liegen in der Registry unter `SOFTWARE\Tilt_gaming` (HKCU oder HKLM, nicht geprüft), gesteuert über
  `/enable`, `/disable` und `/speed=`.
  An die Maus sendet es nichts. Seit Vista bzw. unter Linux erledigt das Betriebssystem das selbst (AC Pan im
  HID-Deskriptor).
- **Makro-Export/-Import** des Originals betrifft nur Dateien auf dem PC. Die Maus kennt dafür keinen Befehl.
  Format siehe „Makrodatei des Originals“ am Ende.

## DPI-Stufe 4 (nur theoretisch)

Register 03/04 enthalten 4 DPI-Einträge, und `01 87 03` aktiviert Stufe 4 auch (am Gerät gesehen). Die Maus kann
per Taste aber nur 3 Stufen schalten und zeigt nur 3 LEDs. `Mappingdefault.ini` führt für die 6980 einen 4. Wert (6030),
den die Software von den 8000/8600-Modellen mitbringt. Stufe 4 hat für diese Maus keinen Nutzen. `m6980x_configtool` (CLI und
Oberfläche) zeigt und setzt deshalb nur Stufe 1–3. Sicherung und Wiederherstellung nehmen Stufe 4 trotzdem mit, damit
der Zustand der Maus vollständig erhalten bleibt.

## Gegenprüfung mit Ghidra (2026-10-11)

`Macro.exe` (SHA-256 `9b327ccc…62aa45`) wurde mit Ghidra 12.1.4 über REA dekompiliert und jede Code-Aussage
dieser Datei dagegen geprüft. Ergebnis: **alle bestätigt, keine widerlegt.** Delphi übergibt Argumente in
Registern; wo Ghidras Pseudocode sie verliert, wurde der Assembler gelesen.

| Adresse | Funktion |
|---|---|
| `0x44407C` | Geräteerkennung: alle HID-Geräte, Feature-Länge 9, `00 00` → `0C 12`, dann `00 01`, `00 02 <Zähler>` |
| `0x444AAC` / `0x44494C` | Handshake (Erkennung / alle übrigen Befehle) |
| `0x445280` `0x445470` `0x445398` `0x445BA8` `0x4455AC` | `01 87`, `01 88`, `01 85` (mit Tausch 3↔4), `01 86`, `01 8B`, je 10 ms |
| `0x4455F4` `0x4454C0` `0x445560` `0x44541C` `0x44550C` | `01 07`, `01 08`, `01 0B`, `01 0A`, `05 00 0C 12` lesen |
| `0x445304` `0x445B24` `0x445700` `0x4457FC` | `01 05`, `01 06`, `01 03 01` (Maske), `01 03`/`01 04` lesen |
| `0x445C5C` | `01 02` speichern, 4 ms |
| `0x445C08` | `00 00…` senden, liefert Datenbyte 5 der Antwort (`03`), Zweck offen ❓ |
| `0x444000` `0x443F80` `0x444BA8` `0x444C30` `0x444F44` | ROM: lesen, 1 Byte, 2 Bytes, n Bytes, Seite |
| `0x445900` / `0x445640` | DPI schreiben / Bruch kürzen |
| `0x44ABBC` | Default-Knopf (DPI aus INI, 500 Hz) |
| `0x44905C` | Abtastraten-Regler: Position p → `01 8B (9 − p)`, Anzeige 1000 / (9 − p) Hz |
| `0x4133A0` | Makro-Bild-Byte schreiben, Aufrufer `0x40F4A4` (Slot + 1) und `0x4173F4` (100 + n) |
| `0x40F914` | Makroliste zeichnen: Wert ≤ 100 → „MACRO n“, sonst Bild Wert − 100 |
| `0x43EAF0` | `Bmp\1.bmp` … `Bmp\60.bmp` laden |
| `0x451000` | 8 LED-Farben |

Vermutung ❓ (nur aus dem Code, nicht beobachtet): Der Default-Knopf setzt den Regler auf Position 7 und verlässt
sich darauf, dass dessen `OnChange` den Wert `[+0x3A4]` auf 7 setzt. Steht der Regler schon auf 7, feuert
`OnChange` vermutlich nicht, und das Original sendet `01 8B 07` (≈ 142 Hz) statt `01 8B 02`. Klären ließe
sich das mit einem Mitschnitt: Original auf 500 Hz stellen, dann „Default“ drücken.

Gruppe 5 bestätigt: `0x425D44` liest für jedes Profil bis zur Anzahl aus `05 00 0C 12` die LED-Farbe in eine
0xA4-Byte-Profilstruktur. Bereich 0x1F01–0x1FFF: Das Original liest dort außer der Kennung bei 0x1F00 nichts gezielt
(nur beim Komplett-Backup). Tastenzustände holt es über `01 05`. Was dort steht, lässt sich aus der Software nicht ableiten.

### Speicheraufteilung aus dem Code (`0x42EBD8`)

Mit den INI-Werten der 6980 (`MemTotal` 8192, `MacroCount` 20, `MacroInfoSize` 10, `[ListMemSize] Total` 360):

| Größe | Formel im Code | Wert |
|---|---|---|
| Makrobereich | MemTotal − 0x100 − ListMemSize.Total | 7576 |
| Slotgröße | (Makrobereich − 20 × 10) / 20 | **368 = 0x170** |
| Slot n | n × Slotgröße | 0x0000, 0x0170, … |
| Info-Eintrag n | 20 × Slotgröße + 1 + n × 10 | **0x1CC1** + n × 10 |
| Funktion 0 | letzter Info-Eintrag + 10 + 3 | **0x1D8C** |
| Funktion i | Funktion i−1 + ListMemSize[i−1] | |
| Konfigbereich | MemTotal − 0x100 | 0x1F00 |

Damit sind die bisher aus Dump und Mitschnitten abgeleiteten Adressen auch im Code belegt.

### Funktions-Datensätze aus dem Code (`0x423630`)

Die Tastenzuweisung baut jeden Datensatz aus Konstanten im Code. Alle 46 Funktionen und 5 Zusatz-Datensätze:

- Die Funktionen 0–42 sind bytegleich mit `m6980x_configtool`, **bis auf** diese Fälle (alle am Gerät getestet):

| Nr. | Funktion | Tool | Original |
|---|---|---|---|
| 12 | Rückgängig | Usage `1C` (Strg+Z auf deutscher Tastatur) | `1D` (Strg+Z auf US-Tastatur, auf deutscher Strg+Y) |
| 19/20 | Minimieren/Maximieren | Alt+Leer tippen, dann N/X | Alt, Leer, N/X gedrückt halten, dann umgekehrt loslassen |
| 33 | Internet | Consumer `0196` (AL Internet Browser) | Consumer `0223` (AC Home) |
| 35 | Vorheriges Fenster | Alt+Tab | Alt+Esc (`29`) |

  Entscheidung (Tom, 2026-10-11): `m6980x_configtool` behält alle vier Varianten. Gründe und Folgen:
  - **Rückgängig:** HID-Usages sind Tastenpositionen einer US-Tastatur. `1D` liegt auf deutschem QWERTZ auf dem Y,
    das Original löst dort also Strg+Y (*Wiederholen*) aus, ein Fehler des Originals. `m6980x_configtool` passt nur für deutsches
    Layout, bei US-Layout wäre es umgekehrt.
  - **Minimieren/Maximieren:** Beide öffnen das Fenstermenü mit Alt+Leer, `m6980x_configtool` tippt wie ein Mensch, das Original
    hält alle Tasten. Unter Windows kein Unterschied, unter Linux hängt beides von der Desktop-Oberfläche ab.
  - **Internet:** `0196` startet den Browser (Linux `KEY_WWW`). `0223` ist die Startseite-Taste (Linux `KEY_HOMEPAGE`);
    in einem offenen Browser springt der aktuelle Tab vermutlich auf die Startseite ❓ (Windows-Zuordnung nicht offiziell belegt).
  - **Vorheriges Fenster:** Alt+Tab wechselt zum zuletzt benutzten Fenster (zweimal = zurück), Alt+Esc schiebt das
    Fenster nach hinten und geht reihum durch alle Fenster, ohne minimierte wiederherzustellen. Alt+Tab passt zum
    Namen und funktioniert auch unter Linux überall.

- **Drei Funktionen fehlen in `m6980x_configtool`** (in `List2` als „Only“ bzw. ohne Hinweis):

| Nr. | INI-Name | Datensatz | Adresse |
|---|---|---|---|
| 43 | Vertical moving | `C6 00`: solange gehalten nur senkrecht bewegen ✅ | 0x1ECA |
| 44 | Horizontal moving | `C7 00`: solange gehalten nur waagerecht bewegen ✅ | 0x1ECC |
| 45 | Ghost | `04 83 E0 83 E2 83 0E 84 0E 84 E2 84 E0 00` = Strg+Alt+K ✅. Das ist der Hotkey, den der Autostart `GHOSTOPEN.exe` anmeldet (`RegisterHotKey(…, MOD_CONTROL\|MOD_ALT, 'K')` bei Datei-Offset 0xF0A), um `Macro.exe` zu öffnen. Ohne installierte Original-Software passiert nichts | 0x1ECE (steht so im Werks-ROM) |

- **Rad-Tasten 8/9:** Legt man Funktion 0–4 (Links-, Rechts-, Mittelklick, Browser vor/zurück) auf Rad hoch/runter,
  schreibt das Original **nicht** den Halte-Datensatz `06 2m 89 FF FF 00`, sondern einen eigenen Klick-Datensatz
  `04 2m 4m 00` an Adresse 46–50 (0x1EDC, 0x1EE0, 0x1EE4, 0x1EE8, 0x1EEC; Reihenfolge Maske 1, 2, 4, 0x10, 0x08).
  Eine Rad-Raste hat kein „Loslassen“. Im Werks-ROM stehen dort `FF`. `m6980x_configtool` macht es seit 2026-10-11 wie das
  Original ✅ (Runde 6: Rad hoch = Linksklick, Rad runter = Rechtsklick, je Raste ein sauberer Klick, nichts hängt).
- Funktion 6/7 (Bildlauf links/rechts) lässt das Original nur auf Taste 6/7 selbst zu (= Standard).
- Funktion 5 „Perfect“: Für M8600, 6980X und ein drittes Modell `04 85 92 01 …` = Consumer `0192` „AL Calculator“
  (Linux `KEY_CALC`), auf dieser Maus also der **Taschenrechner** (auch das Original-Symbol zeigt einen). Für andere
  Mäuse `C1 00` (vermutlich der Sensor-Befehl „Perfect tracking precision“). In `m6980x_configtool` heißt die Funktion `rechner`
  (`perfect` gilt weiter).

### Makro-Kodierer des Editors (`0x410AE8`)

Er setzt jede Editorzeile in Bytes um. Mehr Ereignis-Typen erzeugt das Original nicht:

| Editorzeile | Bytes |
|---|---|
| Modus einmal / Schleife / Feuer | `04` / `05` / `06` am Anfang |
| Taste drücken `SC:<hex>` | `83 uu`, bei Consumer-Codes `85 lo hi` |
| Taste loslassen | `84 uu`, bei Consumer-Codes und „Multi-Media“ `85 00 00` |
| Links/Rechts/Mitte/Taste 4/Taste 5 drücken | `21` / `22` / `24` / `28` / `30` |
| … loslassen | `41` / `42` / `44` / `48` / `50` |
| Rad runter / hoch | `7F` / `61` (immer genau eine Raste) |
| Warten < 256 ms / sonst | `88 nn` / `89 lo hi` |
| Ende | `00` |

Als Consumer-Codes erkennt der Editor `B0 B5 B6 B7 CD E2 E9 EA 183 18A 192 221 223 224 225 22A`. „Alt“ ist
ausgenommen, weil seine Tastatur-Usage `E2` mit „Stumm“ (`E2`) zusammenfällt.

Folgen für `m6980x_configtool`: Mausbewegung oder kombinierte Maustasten-Masken erzeugt das Original nie. **Rad-Werte über 1**
(`m6980x_configtool macro … rad runter 2` = ein Ereignis `7E`) kommen im Original nicht vor und sind am Gerät nicht getestet ❓.
`m6980x_configtool` schreibt deshalb seit 2026-10-11 mehrere Rasten wie das Original als mehrere `61`/`7F` (1 Byte pro Raste).

### Eingabe-Berichte der Maus (Mitschnitt Runde 6, `captures/runde6.pcapng`) ✅

| Endpunkt | Bericht | Bedeutung |
|---|---|---|
| `0x81` (Interface 0) | 7 Byte: `Tasten, X lo, X hi, Y lo, Y hi, Rad, Kipp` | normale Maus |
| `0x82` (Interface 1) | `01 12 03 01 <Profil>` | **meldet jeden Profilwechsel von selbst** (0–2), auch per Taste |
| `0x82` | `02 00 00 <Usage> …` (16 Byte) | Tastatur aus Makros/Funktionen. Modifier wie Strg/Alt stehen als Usage `E0`/`E2` in der Liste, nicht im Modifier-Byte |
| `0x82` | `03 lo hi` | Consumer-Taste, `03 00 00` = losgelassen (kommt auch nach jedem Makro) |

Für Linux interessant: Das aktive Profil lässt sich über `hidraw` von Interface 1 mitlesen, ohne `01 08` abzufragen.

## Makrodatei des Originals (Export/Import im Makro-Editor) ✅ Code und Gerät

Dialog: „Ini File(*.ini)“, Vorschlag `default.ini` (Formular `TForm1`). Export `0x4148A0`, Import `0x418408` → `0x415EE4`.
Delphi-`TIniFile`, also ANSI-Text mit CRLF:

```ini
[Macro]
count=8
0=4
1=131
2=4
…
7=0
```

- `count` = Anzahl Bytes, danach je Byte `<Index>=<Wert dezimal>`. Die Bytes sind genau der Makro-Datensatz im ROM
  (Modus `04`/`05`/`06`, Ereignisse, `00`), erzeugt mit demselben Kodierer wie beim Schreiben in die Maus.
- **Der Name steht nicht in der Datei.** Der Import füllt nur Modus und Ereignisse in den Editor, den Namen gibt man dort ein.
- Der Export schreibt nur, wenn das Makro mehr als 2 Bytes hat (nicht leer).
- Der Import liest Werte mit `ReadInteger` (fehlend = 0), dekodiert bis `00` und überspringt unbekannte Bytes.
- `[SpecialMacro] index=<n>`: Der Export schreibt das nie. Ist `index ≠ 0`, übernimmt der Import nur den Modus und lässt
  die Ereignisliste leer (`0x415DF8`).

`m6980x_configtool` (seit 2026-10-12): `macro-export SLOT datei.ini` schreibt das Format bytegleich (Test mit Toms
Makro 1 „Klicker“: `count=4`, `06 21 41 00`). `macro-import SLOT datei.ini [--name …] [--bild N]` liest es, schneidet
am echten Ende-Byte ab (nicht am ersten `00`, das auch in Argumenten vorkommt) und nimmt ohne `--name` den Dateinamen.
In der Oberfläche laden/speichern die Knöpfe „Aus Datei laden“ / „In Datei speichern“ den Editor, wie im Original.

**Gegentest mit der echten Original-Software (2026-10-12)**, ohne Installation per `msiexec /a` ausgepackt:
- `m6980x_configtool` schrieb Testmakro 20 „TestM20“ (Modus Schleife, Bild 12, alle Ereignisarten: Tasten, Wartezeit
  8/16 Bit, 5 Maustasten, Rad, Consumer). **Das Original zeigte es vollständig richtig an** (Name, Bild, Modus,
  alle Ereignisse; Tom bestätigt). ✅
- Der Export des Originals davon (`captures/originaltest/TestM20_original.ini`) ist **bytegleich** mit
  `m6980x_configtool macro-export` (258 Bytes, CRLF). ✅
- `macro-import` dieser Originaldatei nach Slot 19 ergab dieselben 35 Bytes wie Slot 20. ✅
- Danach Sicherung bytegleich wiederhergestellt. Das Original änderte beim Start keine Werte (schreibt DPI-Stufe 1
  und Abtastrate mit den vorhandenen Werten zurück).

