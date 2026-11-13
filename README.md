# QuasarDock (Freya)

Nativer App-Launcher für Windows — **Rust + Freya 0.5** mit Skia-Rendering.
Kein WebView, kein Node, keine TypeScript-Schicht: ein einziges natives Programm,
das Apps startet, in Ordnern organisiert und sich ins System-Tray minimiert.

Dies ist der Neuaufbau der Tauri-Fassung (v2.8) als schlanke Skia-Anwendung.
Das Tauri-Original dient nur als Lesereferenz für Verhalten und Design; das
Programmicon und der Look sind unverändert übernommen (Design-Freeze).

## Funktionen

- **Kachel- und Listenansicht** der Apps im gewählten Ordner, umschaltbar in der
  Toolbar; Sortierung manuell (per Drag & Drop) oder alphabetisch.
- **Freie Ordnung:** Apps per Drag & Drop im Grid, in der Liste und direkt in
  Sidebar-Ordner verschieben (Ghost, Platzhalter, Einfüge-Indikator,
  Sidebar-Drop mit Cyan-Highlight). Jede Änderung wird sofort persistiert.
- **Frei definierbare Ordner** (anlegen/umbenennen/löschen). Der geschützte
  Standardordner heißt „Favoriten“ — umbenennbar, aber nicht löschbar; beim
  Löschen eines Ordners wandern dessen Apps nach „Favoriten“.
- **Suche** über die Apps des aktuellen Ordners oder aller Ordner.
- **Einstellungen** als Overlay mit Tabs:
  - *Ordner* — anlegen/umbenennen/löschen/Reihenfolge
  - *Apps* — Ordner-Auswahl, Hinzufügen/Bearbeiten/Löschen
  - *Allgemein* — Beim Start ins Tray minimieren · Automatisch ins Tray
    minimieren nach App-Start · Am Bildschirmrand andocken · Fenstergröße und
    -position merken · Windows-Autostart
  - *Darstellung* — Akzentfarbe: 8 Voreinstellungen + eigene Hex-Farbe
  - *Über* — Version, Build, Status der Datenablage
- **System-Tray-Icon** immer vorhanden: Menü „Zeigen/Verbergen“ + „Beenden“.
  Schließen (X) verbirgt ins Tray; beendet wird nur über das Tray-Menü.
- **Kein Taskleisten-Eintrag** (Windows `WS_EX_TOOLWINDOW`); „Beim Start ins
  Tray minimieren“ erzeugt das Fenster von Beginn weg versteckt (kein Blitz).
- **Andocken:** Fenster bündig an den rechten Monitorrand und bündig an die
  Oberkante der Taskleiste (Höhe über `SPI_GETWORKAREA` ermittelt).
- **Echte App-Icons:** PNG/ICO-Dateien sowie eingebettete Symbole aus
  `.exe`/`.dll` (PE-Ressourcen). Fehlversuche landen in `Data/logs/icons.txt`.
- **App-Optionen pro Eintrag:** Startzustand (Normal/Minimiert/Maximiert) und
  automatischer Start mit Startketten-Verzögerung.
- **Eigene Titelleiste** mit nur einem Schließen-Knopf; kein Minimieren/Maximieren.

## Installation

Windows: ZIP aus dem CI-Artefakt (`quasardock-windows-x64-portable.zip`)
herunterladen, entpacken, `quasardock.exe` starten. Kein Installer, keine
Registry (außer optionaler Autostart-Eintrag), kein `%APPDATA%` — die EXE legt
ihren `Data`-Ordner neben sich selbst an.

## Datenablage (`Data/`)

100 % portabel, immer relativ zur Programmdatei:

```
<Programmverzeichnis>/Data/config.json          Konfiguration (atomar geschrieben)
<Programmverzeichnis>/Data/logs/quasardock.log  Laufzeit-Log
<Programmverzeichnis>/Data/logs/icons.txt       Icon-Lade-Ergebnisse je App
```

Der Pfad wird aus dem tatsächlichen EXE-/Prozess-Pfad abgeleitet — nie aus dem
Arbeitsverzeichnis, nie aus `%APPDATA%`. Schreiben erfolgt atomar (Temp-Datei
+ Rename) mit ~600 ms Auto-Save-Debounce. Eine korrupte `config.json` wird in
Quarantäne verschoben und die App startet mit Defaults. Einen stillen Fallback
gibt es nicht: ein nicht beschreibbarer Programmordner erzeugt eine klare
Toast-/Statusmeldung.

## Windows-Smoketest-Checkliste

> Die folgenden Punkte sind plattformgebunden und auf der Linux-Entwicklungsbox
> **nicht runtime-verifizierbar** — sie brauchen einen echten Windows-Lauf
> (Windows 10/11). Die plattformfreie Logik dahinter ist per `cargo test`
> abgedeckt (siehe `docs/ARCHITEKTUR.md` und `docs/BUILD.md` §3).

| # | Punkt | Erwartung | Status |
|---|---|---|---|
| W1 | Tray-Icon immer vorhanden | Systray zeigt QuasarDock-Icon, Menü „Zeigen/Verbergen“ + „Beenden“ | ☐ braucht echten Windows-Lauf |
| W2 | Kein Taskleisten-Eintrag | Fenster taucht NICHT in der Taskleiste auf | ☐ braucht echten Windows-Lauf |
| W3 | „Beim Start ins Tray minimieren“ | App startet ohne Fenster-Blitz, nur Tray | ☐ braucht echten Windows-Lauf |
| W4 | Schließen (X) → verbergen | Fenster weg, App läuft im Tray weiter; Config geflusht | ☐ braucht echten Windows-Lauf |
| W5 | Beenden nur via Tray | „Beenden“ im Tray-Menü: Store geflusht, Prozess endet sauber | ☐ braucht echten Windows-Lauf |
| W6 | Windows-Autostart (Registry) | `HKCU\…\Run` Eintrag an/aus; Self-Heal bei stale Pfad | ☐ braucht echten Windows-Lauf |
| W7 | Echte Icons aus `.exe`/`.dll` | PE-Ressourcen dekodiert, Icon sichtbar; `Data/logs/icons.txt` | ☐ braucht echten Windows-Lauf |
| W8 | Andocken an Taskleiste | Rechter Rand + Oberkante Taskleiste (`SPI_GETWORKAREA`) | ☐ braucht echten Windows-Lauf |
| W9 | Akzentfarbe wirkt | Preset/Hex-Änderung übernimmt sofort | ☐ braucht echten Windows-Lauf |

## Build-Gates

Vor jedem Commit (Linux, Details in `docs/BUILD.md` §1.4):

```bash
cargo fmt --check
cargo test                                   # Quality-Gate: Unit-Tests
cargo build
cargo check --target x86_64-pc-windows-msvc  # Typen/Features für Windows
```

CI (`.github/workflows/windows.yml`, `windows-latest`): `cargo test
--all-targets` → `cargo clippy --all-targets -- -D warnings` → `cargo build
--release --locked` → portables ZIP → Artefakt-Upload. Die Windows-EXE
entsteht **nur** dort (MSVC-Linker; Cross-Compile von Linux ist für
Skia/Freya nicht praktikabel — Begründung: `docs/BUILD.md` §2.2).

## RAM-Bilanz

Leerlauf-RSS der Linux-Binary gemäß SPEC §9 — Messmethode: `/proc/<pid>/status`
(`VmRSS`, gemittelt über mehrere Abstände nach ruhigem Idle unter Xvfb):

| Build | Leerlauf-RSS |
|---|---|
| Linux, Release | 39,852 kB (≈ 39 MB) |
| Linux, Debug | 55,004 kB (≈ 54 MB) |

Zum Vergleich: die Tauri-/WebView2-Referenz zieht ~100 MB (WebView2-Laufzeit).
Windows-EXE-Wert: noch nicht gemessen (braucht echten Windows-Lauf; nachholen
per Task-Manager „Working Set“ oder `Get-Process quasardock | Select WS`).

## Projektstruktur

```
src/main.rs         Start, Fensterkonfiguration, versteckter Start, Tray-Verkabelung
src/model.rs        Datenmodell (Config/Folder/AppEntry) + normalize() + Migrationen
src/store.rs        Portabler atomarer Store (Temp+Rename, Debounce, Quarantäne)
src/paths.rs        EXE-relative Pfade (Data/, Logs)
src/design.rs       Design-Freeze: Farben, Radien, Schriftgrößen, Akzentfarbe
src/ui.rs           Freya-Oberfläche: Toolbar/Grid/Liste/Sidebar/Suche/Einstellungen/DnD
src/windowing.rs    Fenster-Geometrie, Andocken, Zustand merken
src/geometry.rs     Plattformfreie Andock-Geometrie + SPI_GETWORKAREA (Windows)
src/tray.rs         Tray-Icon + Menü-Kanal
src/icons.rs        Icon-Loader: PNG/ICO/PE-Ressourcen + Fallback + Logging
src/launcher.rs     App-Start detached (Args-Parser, cwd, runas, Startzustand)
src/autostart.rs    Windows-Registry-/XDG-Autostart
app-icon.png        Programmicon (unverändert aus der Referenz)
icons/icon.ico      Windows-ICO (unverändert aus der Referenz)
docs/BUILD.md       Build-Pfade Linux/Windows, Smoketest-Details
docs/ARCHITEKTUR.md Architektur und Datenfluss
```

## Lizenz / Herkunft

Interne Anwendung. Design und Icon stammen aus der freigegebenen
QuasarDock-Referenz und sind eingefroren.
