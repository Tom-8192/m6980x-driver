"""M6980X Configuration Tool GUI - quick Tkinter prototype with every function of the tool (Windows + Linux).

    py -3 -m m6980x_configtool gui          (Linux: python3 -m m6980x_configtool gui)

All device access runs in one worker thread; the window never blocks.
"""

import json
import os
import queue
import threading
import tkinter as tk
from tkinter import filedialog, messagebox, ttk
if __name__ == "__main__" and not __package__:
    import sys
    from pathlib import Path
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
    __package__ = "m6980x_configtool"

from . import backup
from . import functions as fn
from . import rom as romlayout
from .cli import (BUTTON_NAMES, FACTORY_INTERVAL_MS, LED_NAMES, RATES, apply_factory_defaults, describe_target,
                  format_icon, load_rom_file, standard_label)
from .protocol import (BUTTONS, DPI_BUTTON_STAGES, LED_LEVELS, MAX_DPI, PROFILES, ROM_SIZE,
                       M6980X)
from .transport import open_transport

LEVEL_TO_8BIT = {0: 0x00, 1: 0x80, 2: 0xFF}
USABLE_BUTTONS = sorted(BUTTON_NAMES)                 # device button 5 has no physical button
MODE_LABELS = {"einmal": "einmal", "schleife": "Schleife (erneut druecken stoppt)",
               "feuer": "Feuertaste (solange gedrueckt)"}
EVENT_HELP = ("Ein Ereignis pro Zeile oder mit Komma getrennt:\n"
              "  drueck a | los a | tippe strg+c | warte 100\n"
              "  klick links | maus-drueck mitte | maus-los rechts (links/rechts/mitte/zurueck/vor)\n"
              "  rad hoch | rad runter 3 | consumer 0x00E9 | consumer los\n"
              "Tasten: a-z, 0-9, f1-f12, enter, esc, tab, space, entf, pos1, ende, hoch, runter, links, rechts,\n"
              "        strg, shift, alt, win, altgr ...  (deutsches Layout)")


def load_icon_texts():
    path = os.path.join(os.path.dirname(__file__), "..", "assets", "icon_descriptions.json")
    try:
        with open(path, encoding="utf-8") as f:
            items = json.load(f)["macro_pictures"]["items"]
        return {i["index"]: i["de"] for i in items}
    except (OSError, KeyError, ValueError):
        return {}


class Device:
    """Serialises all mouse access in one background thread; results come back via Tk's event loop."""

    def __init__(self, root, log):
        self.root, self.log = root, log
        self.jobs = queue.Queue()
        self.pending = 0                                  # queued + running, only touched in the Tk thread
        self.t = self.g = None
        threading.Thread(target=self._loop, daemon=True).start()

    def submit(self, label, job, done=None, quiet=False):
        self.pending += 1
        self.jobs.put((label, job, done, quiet))

    def idle(self):
        return self.pending == 0

    def _loop(self):
        while True:
            label, job, done, quiet = self.jobs.get()
            try:
                if self.g is None:
                    self.t = open_transport()
                    self.g = M6980X(self.t)
                    self.g.check_model()                 # only the 6980X layout is known
                result, error = job(self.g), None
            except Exception as e:                       # noqa: BLE001 - shown to the user
                result, error = None, e
                self._drop_connection()
            self.root.after(0, self._finish, label, done, result, error, quiet)

    def _drop_connection(self):
        try:
            if self.t:
                self.t.close()
        except Exception:                                # noqa: BLE001
            pass
        self.t = self.g = None

    def _finish(self, label, done, result, error, quiet):
        self.pending -= 1
        if error is not None:
            self.log(f"FEHLER bei '{label}': {error}")
            if not quiet:
                messagebox.showerror("Fehler", f"{label}:\n{error}")
            return
        if not quiet:
            self.log(f"OK: {label}")
        if done:
            done(result)


class App:
    def __init__(self, root):
        self.root = root
        root.title("M6980X Configuration Tool")
        root.geometry("1000x720")
        self.icon_texts = load_icon_texts()
        self.macro_names = [""] * romlayout.MACRO_SLOTS
        self.macro_rom = None

        self.status = tk.StringVar(value="Verbinde ...")
        self.persist = tk.BooleanVar(value=True)
        self.live = tk.BooleanVar(value=True)

        outer = ttk.PanedWindow(root, orient=tk.VERTICAL)
        outer.pack(fill=tk.BOTH, expand=True)
        self.nb = ttk.Notebook(outer)
        outer.add(self.nb, weight=4)
        logframe = ttk.Frame(outer)
        outer.add(logframe, weight=1)
        self.logbox = tk.Text(logframe, height=7, state=tk.DISABLED, font=("Consolas", 9))
        self.logbox.pack(fill=tk.BOTH, expand=True)
        ttk.Label(root, textvariable=self.status, anchor="w").pack(fill=tk.X)

        self.dev = Device(root, self.log)
        self._build_overview()
        self._build_dpi()
        self._build_buttons()
        self._build_macros()
        self._build_backup()
        self.refresh_all()
        self.root.after(1500, self._poll_live)

    # -- helpers -----------------------------------------------------------------------
    def log(self, text):
        self.logbox.configure(state=tk.NORMAL)
        self.logbox.insert(tk.END, text + "\n")
        self.logbox.see(tk.END)
        self.logbox.configure(state=tk.DISABLED)
        self.status.set(text)

    def run(self, label, job, done=None, persist=False):
        """Queue a device job; optionally save settings permanently afterwards (01 02)."""
        if persist and self.persist.get():
            inner = job

            def job(g):                                   # noqa: E306
                result = inner(g)
                g.save_settings()
                return result
        self.status.set(f"... {label}")
        self.dev.submit(label, job, done)

    # -- Übersicht ---------------------------------------------------------------------
    def _build_overview(self):
        f = ttk.Frame(self.nb, padding=10)
        self.nb.add(f, text="Übersicht")
        self.info = tk.StringVar(value="-")
        ttk.Label(f, textvariable=self.info, font=("TkDefaultFont", 10, "bold")).grid(row=0, column=0, columnspan=6, sticky="w")

        box = ttk.LabelFrame(f, text="Aktives Profil", padding=6)
        box.grid(row=1, column=0, sticky="nw", pady=6)
        self.var_profile = tk.IntVar(value=1)
        for p in range(1, PROFILES + 1):
            ttk.Radiobutton(box, text=f"Profil {p}", value=p, variable=self.var_profile,
                            command=self.set_profile).pack(anchor="w")

        box = ttk.LabelFrame(f, text="Aktive DPI-Stufe", padding=6)
        box.grid(row=1, column=1, sticky="nw", pady=6, padx=10)
        self.var_stage = tk.IntVar(value=1)
        for s in range(1, DPI_BUTTON_STAGES + 1):
            ttk.Radiobutton(box, text=f"Stufe {s}", value=s, variable=self.var_stage,
                            command=self.set_stage).pack(anchor="w")

        box = ttk.LabelFrame(f, text="Abtastrate", padding=6)
        box.grid(row=1, column=2, sticky="nw", pady=6)
        self.var_rate = tk.StringVar()
        ttk.Combobox(box, textvariable=self.var_rate, state="readonly", width=10,
                     values=[f"{hz} Hz" for hz in sorted(RATES, reverse=True)]).pack()
        ttk.Button(box, text="Setzen", command=self.set_rate).pack(pady=4)

        box = ttk.LabelFrame(f, text="LED-Farbe je Profil (je Kanal 0 = aus, 1 = mittel, 2 = voll; 27 Farben)", padding=6)
        box.grid(row=2, column=0, columnspan=6, sticky="we", pady=6)
        self.led_vars, self.led_swatches = [], []
        for p in range(PROFILES):
            ttk.Label(box, text=f"Profil {p + 1}").grid(row=p, column=0, padx=4)
            sw = tk.Canvas(box, width=40, height=20, highlightthickness=1, highlightbackground="#888")
            sw.grid(row=p, column=1, padx=4)
            self.led_swatches.append(sw)
            rgb = []
            for c, name in enumerate("RGB"):
                v = tk.StringVar(value="0")
                ttk.Label(box, text=name).grid(row=p, column=2 + 2 * c)
                cb = ttk.Combobox(box, textvariable=v, values=[str(x) for x in LED_LEVELS], width=3, state="readonly")
                cb.grid(row=p, column=3 + 2 * c, padx=2)
                cb.bind("<<ComboboxSelected>>", lambda e, p=p: self._update_swatch(p))
                rgb.append(v)
            preset = ttk.Combobox(box, values=list(LED_NAMES), width=12, state="readonly")
            preset.grid(row=p, column=8, padx=6)
            preset.bind("<<ComboboxSelected>>", lambda e, p=p, cb=preset: self._apply_preset(p, cb.get()))
            ttk.Button(box, text="Setzen", command=lambda p=p: self.set_led(p)).grid(row=p, column=9, padx=4)
            self.led_vars.append(rgb)

        row = ttk.Frame(f)
        row.grid(row=3, column=0, columnspan=6, sticky="w", pady=10)
        ttk.Button(row, text="Alles neu lesen", command=self.refresh_all).pack(side=tk.LEFT)
        ttk.Button(row, text="Jetzt dauerhaft speichern (01 02)", command=self.save).pack(side=tk.LEFT, padx=6)
        ttk.Checkbutton(row, text="Profil/DPI/Abtastrate automatisch dauerhaft speichern",
                        variable=self.persist).pack(side=tk.LEFT, padx=6)
        ttk.Checkbutton(row, text="Live-Anzeige (Tasten an der Maus)", variable=self.live).pack(side=tk.LEFT, padx=6)

    def _update_swatch(self, p):
        r, g, b = (LEVEL_TO_8BIT[int(v.get())] for v in self.led_vars[p])
        self.led_swatches[p].configure(background=f"#{r:02x}{g:02x}{b:02x}")

    def _apply_preset(self, p, name):
        for v, level in zip(self.led_vars[p], LED_NAMES[name]):
            v.set(str(level))
        self._update_swatch(p)

    def set_profile(self):
        p = self.var_profile.get()
        self.run(f"Profil {p} aktivieren", lambda g: g.set_active_profile(p - 1), persist=True)

    def set_stage(self):
        s = self.var_stage.get()
        self.run(f"DPI-Stufe {s} aktivieren", lambda g: g.set_active_dpi_stage(s - 1), persist=True)

    def set_rate(self):
        if not self.var_rate.get():
            return
        hz = int(self.var_rate.get().split()[0])
        self.run(f"Abtastrate {hz} Hz", lambda g: g.set_report_interval(RATES[hz]), persist=True)

    def set_led(self, p):
        rgb = [int(v.get()) for v in self.led_vars[p]]
        self.run(f"LED Profil {p + 1} = {rgb}", lambda g: g.set_led_color(p, *rgb))

    def save(self):
        self.run("Dauerhaft speichern", lambda g: g.save_settings())

    def refresh_all(self):
        def job(g):
            return {
                "name": g.name(), "profile": g.active_profile(), "stage": g.active_dpi_stage(),
                "interval": g.report_interval(),
                "dpi": [g.dpi_stage(i) for i in range(DPI_BUTTON_STAGES)],
                "leds": [g.led_color(p) for p in range(PROFILES)],
                "buttons": [[g.button(p, b) for b in range(BUTTONS)] for p in range(PROFILES)],
                "names": g.read_rom(romlayout.MACRO_INFO_BASE, romlayout.MACRO_SLOTS * romlayout.MACRO_INFO_SIZE),
            }
        self.run("Einstellungen lesen", job, self._show_all)

    def _show_all(self, s):
        self.info.set(f"Geraet: {s['name']}   |   Profil {s['profile'] + 1}   |   DPI-Stufe {s['stage'] + 1}   |   "
                      f"{1000 // s['interval']} Hz")
        self.var_profile.set(s["profile"] + 1)
        self.var_stage.set(s["stage"] + 1)
        self.var_rate.set(f"{1000 // s['interval']} Hz")
        for p, rgb in enumerate(s["leds"]):
            for v, level in zip(self.led_vars[p], rgb):
                v.set(str(level) if level in LED_LEVELS else "0")
            self._update_swatch(p)
        for i, st in enumerate(s["dpi"]):
            self.dpi_x[i].set(st.x_dpi)
            self.dpi_y[i].set(st.y_dpi)
        size = romlayout.MACRO_INFO_SIZE
        self.macro_names = [romlayout.macro_name(s["names"][i * size:(i + 1) * size]) for i in range(romlayout.MACRO_SLOTS)]
        self.button_state = s["buttons"]
        self._fill_targets()
        self._show_buttons()

    def _poll_live(self):
        if self.live.get() and self.dev.idle():
            def done(r):
                p, st = r
                if (p + 1, st + 1) != (self.var_profile.get(), self.var_stage.get()):
                    self.var_profile.set(p + 1)
                    self.var_stage.set(st + 1)
                    self.log(f"Maus: Profil {p + 1}, DPI-Stufe {st + 1}")
                    self._show_buttons()
            self.dev.submit("Live", lambda g: (g.active_profile(), g.active_dpi_stage()), done, quiet=True)
        self.root.after(1500, self._poll_live)

    # -- DPI ---------------------------------------------------------------------------
    def _build_dpi(self):
        f = ttk.Frame(self.nb, padding=10)
        self.nb.add(f, text="DPI")
        ttk.Label(f, text=f"Schritte zu 90 DPI, 90-{MAX_DPI}. Ueber 5040 skaliert die Firmware per Bruch (wie das Original). "
                          "Die Werte gelten fuer alle Profile.").grid(row=0, column=0, columnspan=6, sticky="w", pady=6)
        self.dpi_x, self.dpi_y = [], []
        for i in range(DPI_BUTTON_STAGES):
            ttk.Label(f, text=f"Stufe {i + 1}").grid(
                row=i + 1, column=0, sticky="w", padx=4, pady=3)
            x, y = tk.IntVar(value=0), tk.IntVar(value=0)
            for col, (lbl, var) in enumerate((("X", x), ("Y", y))):
                ttk.Label(f, text=lbl).grid(row=i + 1, column=1 + 2 * col)
                ttk.Spinbox(f, from_=90, to=MAX_DPI, increment=90, textvariable=var, width=7).grid(
                    row=i + 1, column=2 + 2 * col, padx=4)
            ttk.Button(f, text="Setzen", command=lambda i=i: self.set_dpi(i)).grid(row=i + 1, column=5, padx=6)
            self.dpi_x.append(x)
            self.dpi_y.append(y)
        ttk.Button(f, text="Werkswerte 810 / 1620 / 3240 und 500 Hz (Default-Knopf des Originals)",
                   command=self.dpi_default).grid(row=DPI_BUTTON_STAGES + 1, column=0, columnspan=6, sticky="w", pady=10)

    def set_dpi(self, i):
        x, y = self.dpi_x[i].get(), self.dpi_y[i].get()
        self.run(f"DPI-Stufe {i + 1} = {x}/{y}", lambda g: g.set_dpi(i, x, y), self._dpi_done(i), persist=True)

    def _dpi_done(self, i):
        def done(stage):
            self.dpi_x[i].set(stage.x_dpi)
            self.dpi_y[i].set(stage.y_dpi)
        return done

    def dpi_default(self):
        def done(stages):
            for i, st in enumerate(stages):
                self.dpi_x[i].set(st.x_dpi)
                self.dpi_y[i].set(st.y_dpi)
            self.var_rate.set(f"{1000 // FACTORY_INTERVAL_MS} Hz")
        self.run("DPI-Werkswerte und 500 Hz", apply_factory_defaults, done, persist=True)

    # -- Tasten ------------------------------------------------------------------------
    def _build_buttons(self):
        f = ttk.Frame(self.nb, padding=10)
        self.nb.add(f, text="Tasten")
        top = ttk.Frame(f)
        top.grid(row=0, column=0, columnspan=4, sticky="w", pady=4)
        ttk.Label(top, text="Profil").pack(side=tk.LEFT)
        self.btn_profile = tk.IntVar(value=1)
        for p in range(1, PROFILES + 1):
            ttk.Radiobutton(top, text=str(p), value=p, variable=self.btn_profile,
                            command=self._show_buttons).pack(side=tk.LEFT, padx=4)
        ttk.Button(top, text="Alle Tasten dieses Profils auf Standard", command=self.reset_profile).pack(side=tk.LEFT, padx=12)
        self.button_state = [[None] * BUTTONS for _ in range(PROFILES)]
        self.btn_current, self.btn_target, self.targets = {}, {}, []
        for row, b in enumerate(USABLE_BUTTONS, start=1):
            ttk.Label(f, text=f"{b}  {BUTTON_NAMES[b]}", width=14).grid(row=row, column=0, sticky="w", pady=2)
            cur = tk.StringVar(value="-")
            ttk.Label(f, textvariable=cur, width=34).grid(row=row, column=1, sticky="w")
            cb = ttk.Combobox(f, width=42, state="readonly")
            cb.grid(row=row, column=2, padx=6)
            ttk.Button(f, text="Zuweisen", command=lambda b=b: self.assign(b)).grid(row=row, column=3)
            self.btn_current[b], self.btn_target[b] = cur, cb

    def _fill_targets(self):
        self.targets = [("Standard", "std", None)]
        self.targets += [(f.label, "fn", f) for f in fn.FUNCTIONS if f.size and f.record]
        self.targets += [(f"Makro {s + 1}" + (f" '{n}'" if n else ""), "macro", s)
                         for s, n in enumerate(self.macro_names)]
        for b, cb in self.btn_target.items():            # "Standard" says what the button then does
            cb.configure(values=[standard_label(b)] + [t[0] for t in self.targets[1:]])

    def _show_buttons(self):
        p = self.btn_profile.get() - 1
        for b in USABLE_BUTTONS:
            self.btn_current[b].set(describe_target(self.button_state[p][b], self.macro_names, b))

    def assign(self, b):
        p = self.btn_profile.get() - 1
        idx = self.btn_target[b].current()
        if idx < 0:
            return
        label, kind, value = self.targets[idx]
        if kind == "std":
            label = standard_label(b)
            job = lambda g: g.reset_button(p, b)                                    # noqa: E731
        elif kind == "fn":
            job = lambda g: g.assign_function(p, b, value)                          # noqa: E731
        else:
            job = lambda g: g.assign_macro(p, b, value)                             # noqa: E731

        def done(_):
            self.button_state[p][b] = None if kind == "std" else (fn.placement(value, b)[0] if kind == "fn"
                                                                    else romlayout.macro_slot_addr(value))
            self._show_buttons()
        self.run(f"Profil {p + 1}, Taste {BUTTON_NAMES[b]} = {label}", job, done)

    def reset_profile(self):
        p = self.btn_profile.get() - 1
        if not messagebox.askyesno("Standard", f"Alle Tasten von Profil {p + 1} auf Standard setzen?"):
            return

        def job(g):
            for b in range(BUTTONS):
                g.reset_button(p, b)

        def done(_):
            self.button_state[p] = [None] * BUTTONS
            self._show_buttons()
        self.run(f"Profil {p + 1}: alle Tasten Standard", job, done)

    # -- Makros ------------------------------------------------------------------------
    def _build_macros(self):
        f = ttk.Frame(self.nb, padding=10)
        self.nb.add(f, text="Makros")
        cols = ("slot", "name", "bild", "modus", "bytes", "events")
        self.tree = ttk.Treeview(f, columns=cols, show="headings", height=8)
        for c, w, t in zip(cols, (40, 90, 150, 80, 70, 520), ("Nr", "Name", "Bild", "Modus", "Bytes", "Ereignisse")):
            self.tree.heading(c, text=t)
            self.tree.column(c, width=w, anchor="w")
        self.tree.grid(row=0, column=0, columnspan=6, sticky="nsew")
        self.tree.bind("<<TreeviewSelect>>", self._load_selected_macro)
        ttk.Button(f, text="Makros von der Maus lesen (~6 s)", command=self.read_macros).grid(row=1, column=0, sticky="w", pady=4)

        ed = ttk.LabelFrame(f, text="Editor", padding=6)
        ed.grid(row=2, column=0, columnspan=6, sticky="nsew")
        self.m_slot = tk.IntVar(value=1)
        self.m_name = tk.StringVar()
        self.m_mode = tk.StringVar(value=MODE_LABELS["einmal"])
        self.m_icon = tk.StringVar(value="kein Bild")
        ttk.Label(ed, text="Slot").grid(row=0, column=0)
        ttk.Spinbox(ed, from_=1, to=romlayout.MACRO_SLOTS, textvariable=self.m_slot, width=4).grid(row=0, column=1)
        ttk.Label(ed, text="Name (max. 9)").grid(row=0, column=2, padx=(10, 0))
        ttk.Entry(ed, textvariable=self.m_name, width=12).grid(row=0, column=3)
        ttk.Label(ed, text="Modus").grid(row=0, column=4, padx=(10, 0))
        ttk.Combobox(ed, textvariable=self.m_mode, values=list(MODE_LABELS.values()), state="readonly",
                     width=30).grid(row=0, column=5)
        ttk.Label(ed, text="Bild").grid(row=1, column=0)
        self.icon_values = ["kein Bild"] + [f"{n}: {self._icon_text(n)}" for n in range(1, romlayout.ICON_COUNT + 1)]
        ttk.Combobox(ed, textvariable=self.m_icon, values=self.icon_values, state="readonly", width=60).grid(
            row=1, column=1, columnspan=5, sticky="w", pady=4)
        self.m_events = tk.Text(ed, height=7, width=100, font=("Consolas", 9))
        self.m_events.grid(row=2, column=0, columnspan=6, sticky="we")
        self.m_events.bind("<KeyRelease>", lambda e: self._count_bytes())
        self.m_bytes = tk.StringVar(value="")
        ttk.Label(ed, textvariable=self.m_bytes).grid(row=3, column=0, columnspan=3, sticky="w")
        ttk.Label(ed, text=EVENT_HELP, foreground="#555", justify="left").grid(row=4, column=0, columnspan=6, sticky="w")
        bar = ttk.Frame(ed)
        bar.grid(row=5, column=0, columnspan=6, sticky="w", pady=4)
        ttk.Button(bar, text="Auf Maus schreiben", command=self.write_macro).pack(side=tk.LEFT)
        ttk.Button(bar, text="Nur Bild setzen", command=self.set_macro_icon).pack(side=tk.LEFT, padx=6)
        ttk.Button(bar, text="Makro loeschen (wie 'Freigeben')", command=self.clear_macro).pack(side=tk.LEFT, padx=6)
        ttk.Button(bar, text="Aus Datei laden (.ini) ...", command=self.import_macro_file).pack(side=tk.LEFT, padx=(24, 6))
        ttk.Button(bar, text="In Datei speichern (.ini) ...", command=self.export_macro_file).pack(side=tk.LEFT)
        f.columnconfigure(5, weight=1)
        f.rowconfigure(0, weight=1)

    def _editor_record(self):
        mode = next(k for k, v in MODE_LABELS.items() if v == self.m_mode.get())
        text = self.m_events.get("1.0", tk.END)
        return bytes([fn.MODE_NAMES[mode]]) + fn.encode_events(text) + bytes([fn.EV_END])

    def _editor_icon(self):
        v = self.m_icon.get()
        return None if v == "kein Bild" else int(v.split(":")[0])

    def _count_bytes(self):
        try:
            n = len(self._editor_record())
            self.m_bytes.set(f"{n} / {romlayout.MACRO_SLOT_SIZE} Bytes" + ("  - ZU LANG" if n > romlayout.MACRO_SLOT_SIZE else ""))
        except ValueError as e:
            self.m_bytes.set(f"Fehler: {e}")

    def read_macros(self):
        self.run("Makros lesen", lambda g: g.read_rom(0, ROM_SIZE), self._show_macros)

    def _show_macros(self, data):
        self.macro_rom = data
        self.tree.delete(*self.tree.get_children())
        for s in range(romlayout.MACRO_SLOTS):
            start = romlayout.macro_slot_addr(s)
            slot = data[start:start + romlayout.MACRO_SLOT_SIZE]
            info = romlayout.macro_info(data, s)
            name = romlayout.macro_name(info)
            self.macro_names[s] = name
            mode, events = fn.decode_record(slot)
            self.tree.insert("", tk.END, iid=str(s), values=(
                s + 1, name, format_icon(romlayout.macro_icon(info)), mode or "-",
                fn.record_length(slot), ", ".join(events)))
        self._fill_targets()
        self._show_buttons()

    def _load_selected_macro(self, _):
        if not self.tree.selection() or self.macro_rom is None:
            return
        s = int(self.tree.selection()[0])
        start = romlayout.macro_slot_addr(s)
        info = romlayout.macro_info(self.macro_rom, s)
        icon = romlayout.macro_icon(info)
        self.m_slot.set(s + 1)
        self.m_name.set(romlayout.macro_name(info))
        self.m_icon.set(self.icon_values[icon or 0])
        self._set_editor(self.macro_rom[start:start + romlayout.MACRO_SLOT_SIZE])

    def _set_editor(self, record):
        mode, events = fn.decode_record(record)
        self.m_mode.set(MODE_LABELS.get(mode, MODE_LABELS["einmal"]))
        self.m_events.delete("1.0", tk.END)
        self.m_events.insert("1.0", "\n".join(events))
        self._count_bytes()

    def import_macro_file(self):
        """Like the original's editor import: fills mode and events; the file has no name (file name is offered)."""
        path = filedialog.askopenfilename(filetypes=[("Makrodatei der Original-Software", "*.ini")])
        if not path:
            return
        try:
            record = backup.read_macro_ini(path)
        except (OSError, ValueError) as e:
            messagebox.showerror("Makrodatei", str(e))
            return
        self._set_editor(record)
        if not self.m_name.get():
            self.m_name.set(os.path.splitext(os.path.basename(path))[0][:romlayout.MACRO_NAME_LEN])
        self.log(f"Makrodatei geladen: {path} ({len(record)} Bytes) - noch nicht auf der Maus")

    def export_macro_file(self):
        try:
            record = self._editor_record()
        except ValueError as e:
            messagebox.showerror("Makro", str(e))
            return
        path = filedialog.asksaveasfilename(defaultextension=".ini", initialfile=(self.m_name.get() or "default") + ".ini",
                                            filetypes=[("Makrodatei der Original-Software", "*.ini")])
        if not path:
            return
        try:
            backup.write_macro_ini(record, path)
        except (OSError, ValueError) as e:
            messagebox.showerror("Makrodatei", str(e))
            return
        self.log(f"Makro gespeichert: {path} ({len(record)} Bytes)")

    def _icon_text(self, n):
        if n <= romlayout.ICONS_EMBEDDED:
            return self.icon_texts.get(n, "eingebaut")
        return f"eigenes Bild bmp\\{n - romlayout.ICONS_EMBEDDED}.bmp"

    def write_macro(self):
        try:
            record = self._editor_record()
        except ValueError as e:
            messagebox.showerror("Makro", str(e))
            return
        slot, name, icon = self.m_slot.get() - 1, self.m_name.get(), self._editor_icon()
        self.run(f"Makro {slot + 1} '{name}' schreiben ({len(record)} Bytes)",
                 lambda g: (g.write_macro(slot, record, name, icon=icon), g.read_rom(0, ROM_SIZE))[1], self._show_macros)

    def set_macro_icon(self):
        slot, icon = self.m_slot.get() - 1, self._editor_icon()
        self.run(f"Bild von Makro {slot + 1}", lambda g: (g.set_macro_icon(slot, icon), g.read_rom(0, ROM_SIZE))[1],
                 self._show_macros)

    def clear_macro(self):
        slot = self.m_slot.get() - 1
        if messagebox.askyesno("Makro loeschen", f"Makro {slot + 1} loeschen (Name leeren wie das Original)?"):
            self.run(f"Makro {slot + 1} loeschen", lambda g: (g.clear_macro(slot), g.read_rom(0, ROM_SIZE))[1],
                     self._show_macros)

    # -- Sicherung ---------------------------------------------------------------------
    def _build_backup(self):
        f = ttk.Frame(self.nb, padding=10)
        self.nb.add(f, text="Sicherung")
        rows = [
            ("Komplette Sicherung speichern (Speicher + DPI, LED, Abtastrate, Tasten) ...", self.do_backup),
            ("Komplette Sicherung wiederherstellen ...", self.do_restore),
            ("Speicher exportieren (.meb = Format der Original-Software, .bin = roh) ...", self.do_rom_export),
            ("Speicher importieren (.meb / .bin) ...", self.do_rom_import),
            ("Alles freigeben: alle Makros loeschen + alle Tasten Standard (wie das Original) ...", self.do_release),
        ]
        for i, (text, cmd) in enumerate(rows):
            ttk.Button(f, text=text, command=cmd).grid(row=i, column=0, sticky="w", pady=4)

    def do_backup(self):
        path = filedialog.asksaveasfilename(defaultextension=".json", filetypes=[("M6980X-Sicherung", "*.json")])
        if path:
            self.run(f"Sicherung -> {path}", lambda g: backup.save(backup.create(g), path))

    def do_restore(self):
        path = filedialog.askopenfilename(filetypes=[("M6980X-Sicherung", "*.json")])
        if not path:
            return
        data = backup.load(path)
        if messagebox.askyesno("Wiederherstellen", f"Sicherung vom {data['created']} auf die Maus schreiben?"):
            self.run("Sicherung wiederherstellen", lambda g: (backup.restore(g, data, log=lambda *_: None),
                                                              g.save_settings())[0],
                     lambda left: (self.log("Rest-Abweichungen: " + str(left) if left else "ROM bytegleich"),
                                   self.refresh_all()))

    def do_rom_export(self):
        path = filedialog.asksaveasfilename(defaultextension=".meb",
                                            filetypes=[("Original-Sicherung", "*.meb"), ("Rohdaten", "*.bin")])
        if not path:
            return

        def job(g):
            data = g.read_rom(0, ROM_SIZE)
            if path.lower().endswith(".meb"):
                backup.write_meb(data, path)
            else:
                with open(path, "wb") as fh:
                    fh.write(data)
        self.run(f"Speicher -> {path}", job)

    def do_rom_import(self):
        path = filedialog.askopenfilename(filetypes=[("Speicher", "*.meb *.bin")])
        if not path:
            return
        data = load_rom_file(path)
        if messagebox.askyesno("Import", f"{os.path.basename(path)} in den Speicher der Maus schreiben?"):
            self.run("Speicher importieren", lambda g: backup.import_rom(g, data, log=lambda *_: None),
                     lambda left: self.refresh_all())

    def do_release(self):
        if messagebox.askyesno("Freigeben", "Wirklich alle Makros loeschen und alle Tasten auf Standard setzen?\n"
                                            "(Vorher am besten eine Sicherung anlegen.)"):
            self.run("Alles freigeben", lambda g: g.release_all(), lambda _: self.refresh_all())


def main():
    root = tk.Tk()
    App(root)
    root.mainloop()
    return 0


if __name__ == "__main__":
    main()
