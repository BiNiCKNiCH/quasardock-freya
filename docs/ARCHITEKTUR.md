# Architektur — QuasarDock (Freya)

Stand: Phase 4 (End-Abnahme). Beschreibt den Aufbau des Neuaufbaus mit
**Rust + Freya 0.5** (native Skia-UI, kein WebView) und die Datenflüsse.

## 1. Überblick

```
┌────────────────────────────── quasardock (EXE) ──────────────────────────────┐
│                                                                              │
│  Freya-Laufzeit (Skia-Renderer, winit-Fenster, keine WebView)                │
│      │                                                                       │
│      ├── ui.rs         Eine Oberfläche: Toolbar/Grid/Liste/Sidebar/          │
│      │                 Suche/Einstellungen/DnD — liest Store-Config,         │
│      │                 schreibt über store.update()                          │
│      │                                                                       │
│  Kern (plattformfrei, unit-getestet)                                         │
│      ├── model.rs      Config { folders, apps, settings } + normalize()      │
│      │                 + Migrationen älterer Feldnamen                       │
│      ├── store.rs      Atomarer Store: Temp-Datei + Rename, ~600 ms          │
│      │                 Auto-Save-Debounce, Quarantäne korrupter Dateien      │
│      ├── paths.rs      EXE-relative Pfade: Data/config.json, Data/logs/      │
│      ├── geometry.rs   Andock-Geometrie (dock_rect, taskbar_height,          │
│      │                 maybe_dock) — reine Rechnung, plattformfrei           │
│      ├── icons.rs      Icon-Loader PNG/ICO/PE + Fallback + icons.txt-Log     │
│      ├── launcher.rs   App-Start detached (Args-Parser, cwd, runas,          │
│      │                 Startzustand, Auto-Start-Kette)                       │
│      ├── autostart.rs  Autostart-Format (Registry-Befehl / XDG-Datei)        │
│      │                                                                       │
│  Systemanbindung (plattformgebunden, hinter cfg(windows))                    │
│      ├── windowing.rs  Fenster-Geometrie anwenden, Andocken beim Start,      │
│      │                 Schließen → ins Tray verbergen                        │
│      ├── tray.rs       Tray-Icon + Menü-Kanal (tray-icon 0.20)               │
│      └── (in geometry.rs/autostart.rs) SPI_GETWORKAREA, Registry             │
└──────────────────────────────────────────────────────────────────────────────┘
```

Leitlinie: **so viel Logik wie möglich plattformfrei** (Speicher, Rechnung,
Parsing) und per `cargo test` auf Linux abgedeckt; nur die tatsächliche
Windows-API-Anbindung (Tray, Arbeitsfläche, Registry, PE-Laden) liegt hinter
`#[cfg(windows)]` und ist als Smoketest-Checkliste dokumentiert (README,
`docs/BUILD.md` §3).

## 2. Datenfluss

1. **Start:** `main.rs` erzeugt das Fenster — je nach `settings.start_in_tray`
   von Beginn weg versteckt (off-screen, kein Show-then-Hide, kein Blitz),
   andernfalls mit gespeicherter oder Default-Geometrie (1280×800, min.
   900×600). Danach Aufbau der Freya-UI und des Tray-Icons.
2. **Konfiguration:** `Store::open()` liest `Data/config.json` (Pfad aus dem
   echten EXE-Pfad, `paths.rs`). Korrupte/leere Datei → Quarantäne
   (`config.corrupt-<Zeitstempel>.json`) und Start mit Defaults. Jede
   Änderung läuft über `store.update()` → `normalize()` → atomarer Save
   (Temp + Rename) mit ~600 ms Debounce.
3. **UI-Zustand:** Freya-Signale (`Rc<Signal>`) für Config, aktuellen Ordner,
   Suche, View-Mode, Drag-Zustand, Einstellungen-Overlay. Die UI ist eine
   reine Funktion des Store-Configs plus flüchtiger Interaktionszustände.
4. **App-Start:** Klick → `launcher.rs` baut den Befehl (Args mit Quotes,
   `cwd`, optional `runas`-Elevation, Startzustand), startet detached; nach
   Erfolg optional „Automatisch ins Tray minimieren“.
5. **Icons:** beim Rendern einer Kachel/Zeile fragt die UI `icons.rs` nach
   dem Icon: PNG → Dekodierung; ICO → Mehrbild-Extraktion (größtes Bild);
   `.exe`/`.dll` → PE-Ressourcen (RT_GROUP_ICON/RT_ICON). Ergebnis je
   Anfrage eine Zeile in `Data/logs/icons.txt`; Fehlschlag → Buchstaben-
   Fallback (kein Toast-Spam).
6. **Beenden:** nur über Tray-Menü „Beenden“ → Store-Flush → Prozessende.
   Schließen (X) versteckt das Fenster ins Tray und speichert Geometrie
   (nur aus sichtbaren Zuständen).

## 3. Windows-Besonderheiten

- **Kein Taskleisten-Eintrag:** `WS_EX_TOOLWINDOW` (skip_taskbar-Äquivalent).
- **Taskleistenhöhe:** `SystemParametersInfoW(SPI_GETWORKAREA)` — die
  Arbeitsfläche minus Monitor-Rect ergibt die Taskleistenhöhe; der reine
  Rechnungsteil (`taskbar_height`, `dock_rect`, `maybe_dock`) ist
  plattformfrei und getestet. Linux-Fallback: `monitor.size()`.
- **Autostart:** `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` —
  Eintrag mit exakt aktuellem EXE-Befehl; Self-Heal ersetzt einen stale
  Pfad beim Setzen. Linux: XDG-Autostart-Datei.
- **PE-Icons:** Sektionstabelle + RVA→Offset-Auflistung, RT_GROUP_ICON und
  RT_ICON-Blobs; defekte/fehlende Quellen enden immer im Fallback-Pfad.

## 4. Design-Freeze

Farben, Radien, Schriftgrößen und das Programmicon sind eingefroren
(SPEC §2) und zentral in `src/design.rs` als Konstanten hinterlegt; die
Akzentfarbe ist die **einzige** Laufzeit-Abweichung (Option in den
Einstellungen, Default Cyan `#00B4D8`) und wirkt über eine zentrale
`accent()`-Funktion auf alle aktiven Zustände.

## 5. Teststrategie

- **43 Unit-Tests** auf Linux (plattformfrei): Datenmodell/normalize,
  Migrationen, Store-Atomarität/Debounce/Quarantäne/Portabilität,
  Favoriten-Schutz, Andock-Geometrie, Icon-Pfad-Klassifizierung und
  PE-Parsing, Autostart-Formatierung, Args-Parser, Store-Flush beim Beenden.
- **Windows-Check:** `cargo check --target x86_64-pc-windows-msvc
  --all-targets` — Typprüfung inkl. aller `#[cfg(windows)]`-Pfade.
- **CI (`windows-latest`):** dieselben Tests + Clippy (`-D warnings`) +
  Release-Build auf echter MSVC-Toolchain.
- **Smoketest W1–W9** (echter Windows-Lauf): Tray, Taskleisten-Abwesenheit,
  versteckter Start, X→Tray, Beenden, Registry-Autostart, PE-Icons,
  Andocken, Akzentfarbe.

## 6. Bekannte Grenzen

- Windows-only-Runtime-Verhalten (Tray-Icon am echten System, Registry,
  Taskleisten-Ignore, echtes PE-Laden) ist nicht auf Linux laufzeit-verifizierbar
  und nur als Checkliste abgedeckt.
- Installer (NSIS/MSI) sind optional und nicht Teil des Umfangs; portable
  Einzel-EXE ist der Demo-Pfad.
