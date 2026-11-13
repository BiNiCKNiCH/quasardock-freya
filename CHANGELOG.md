# Changelog

Alle nennenswerten Änderungen dieses Projekts werden hier dokumentiert.
Format: [Keep a Changelog](https://keepachangelog.com/de/) — Versionen nach
Abnahmephasen des Neuaufbaus.

## [0.1.0] — 2026-11-13

Erster vollständiger Neuaufbau von QuasarDock mit **Rust + Freya 0.5**
(natives Skia-Rendering, kein WebView). Funktionsumfang entsprechend
SPEC §4 (Stand: Tauri-Referenz v2.8).

### Hinzugefügt

- **Datenmodell + portabler Store** (`model.rs`, `store.rs`, `paths.rs`):
  `Config { folders, apps, settings }` mit `normalize()` (doppelte IDs,
  Ordnerzuordnung, Favoriten-Schutz), Migrationen älterer Feldnamen,
  atomares Schreiben (Temp-Datei + Rename) mit ~600 ms Auto-Save-Debounce,
  Quarantäne korrupter `config.json`. Konfiguration **immer** unter
  `<Programmverzeichnis>/Data/config.json` — abgeleitet aus dem tatsächlichen
  EXE-Pfad, nie aus `%APPDATA%` oder dem Arbeitsverzeichnis.
- **Oberfläche** (`ui.rs`): Kachel-Grid (~96 px) und Listenansicht, umschaltbar;
  Sortierung manuell/alphabetisch; Sidebar mit Ordnern (App-Anzahl, aktiver
  Cyan-Indikator) und Einstellungs-Button; Suche über aktuellen Ordner oder
  alle Ordner; eigenes Titelleisten-Band mit nur einem Schließen-Knopf.
- **Einstellungen-Overlay** mit Tabs Ordner / Apps / Allgemein / Darstellung /
  Über (SPEC §4.5).
- **Drag & Drop:** eigene Pointer-DnD mit Ghost, Platzhalter,
  Einfüge-Indikator und Sidebar-Drop (Cyan-Highlight); sofort persistiert.
- **Systemintegration** (`tray.rs`, `windowing.rs`, `geometry.rs`,
  `autostart.rs`, `launcher.rs`):
  - Tray-Icon mit Menü „Zeigen/Verbergen“ + „Beenden“; Store-Flush beim Beenden.
  - Kein Taskleisten-Eintrag (Windows `WS_EX_TOOLWINDOW`).
  - Versteckter Start (off-screen, kein Show-then-Hide — kein Fenster-Blitz)
    für „Beim Start ins Tray minimieren“.
  - Schließen (X) verbirgt ins Tray; Geometrie wird nur aus sichtbaren
    Zuständen gespeichert.
  - Andocken: bündig rechter Monitorrand + Oberkante Taskleiste;
    Taskleistenhöhe über `SPI_GETWORKAREA` (Windows) mit Linux-Fallback.
  - Windows-Autostart über `HKCU\…\Run` mit Self-Heal bei stale Pfad;
    XDG-Autostart unter Linux.
  - App-Start detached mit Args-Parser (Quotes), Arbeitsverzeichnis,
    `runas`-Elevation und Startzustand (Normal/Minimiert/Maximiert);
    Auto-Start-Kette mit Verzögerung.
- **Icon-Loader** (`icons.rs`): PNG/ICO (inkl. Mehrbild) und eingebettete
  Symbole aus `.exe`/`.dll` (PE-Ressourcen, „MZ“-Kennung); Buchstaben-Fallback
  ohne Toast-Spam; je Anfrage eine Zeile in `Data/logs/icons.txt`.
- **Akzentfarbe** als einzige Design-Abweichung: 8 Voreinstellungen + eigene
  Hex-Farbe, wirkt zentral über die `ACCENT`-Konstante (Design-Freeze bleibt).
- **Windows-Icon:** `icons/icon.ico` unverändert aus der Referenz übernommen.

### Build/CI

- GitHub-Actions-Workflow `.github/workflows/windows.yml` (`windows-latest`):
  `cargo test --all-targets` → `cargo clippy --all-targets -- -D warnings` →
  `cargo build --release --locked` → portables ZIP → Artefakt-Upload.
  Die Windows-EXE entsteht nur auf dem Windows-Läufer (MSVC-Linker); der
  Windows-Typcheck läuft lokal cross (`cargo check --target
  x86_64-pc-windows-msvc`).
- Linux-Quality-Gates: `cargo fmt --check`, `cargo test`, `cargo build`,
  Windows-Check (siehe `docs/BUILD.md` §1.4).

### Getestet

- 43 Unit-Tests (Datenmodell, Store, Pfade, Geometrie, Icons, Autostart,
  Launcher, Store-Flush beim Beenden) — plattformfreie Logik; Windows-API-
  Anbindung als Smoketest-Checkliste (README, `docs/BUILD.md` §3).
