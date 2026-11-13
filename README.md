# QuasarDock (Freya)

Nativer App-Launcher für Windows — **Rust + Freya 0.5** mit Skia-Rendering.
Kein WebView, kein Node, keine TypeScript-Schicht: ein einziges natives Programm.

Dies ist der Neuaufbau der Tauri-Fassung als schlanke Skia-Anwendung. Das
Tauri-Original dient nur als Lesereferenz für Verhalten und Design.

## Stand

**Phase 0 (Fundament)** — baubar, startbar, Design eingefroren:

- Fenster 1280 × 800 (min. 900 × 600), dunkel, ohne Start-Flash
- Struktur aus der Referenz: Hauptbereich links (~70 %), Sidebar rechts (~30 %)
- Design-Tokens als zentrale Datei `src/design.rs` (SPEC §2)
- Programmicon und Windows-ICO unverändert aus der Referenz übernommen
- Windows-Buildpfad festgelegt und dokumentiert: `docs/BUILD.md`

Noch nicht enthalten (kommende Phasen): Datenmodell/Persistenz, echte Apps,
Suche, Drag & Drop, Einstellungen, Tray, Andocken, Autostart, Icon-Loader.

## Bauen (Linux)

```bash
cargo build            # Debug-Binary: target/debug/quasardock
cargo run              # startet das Fenster
cargo test             # Design-Token-Konsistenztests
```

System-Voraussetzungen (Debian/Ubuntu):

```bash
sudo apt install build-essential pkg-config libx11-dev libxcb1-dev \
  libxkbcommon-dev libxkbcommon-x11-0 libgl1-mesa-dev libegl1 \
  libgles2 libwayland-dev libfontconfig1-dev libfreetype-dev
```

Headless-Prüfung (CI/Sandbox, ohne Bildschirm):

```bash
xvfb-run -a cargo run
```

Details, Sonderfälle dieser Entwicklungsbox und der **Windows-Buildpfad** stehen
in [`docs/BUILD.md`](docs/BUILD.md).

## Projektstruktur

```
src/main.rs     Start, Fensterkonfiguration, eingebettetes Programmicon
src/design.rs   Design-Freeze: Farben, Radien, Schriftgrößen (SPEC §2)
src/ui.rs       Layout-Grundgerüst (Sidebar + Hauptfläche)
app-icon.png    Programmicon (unverändert aus der Referenz)
icons/icon.ico  Windows-ICO (unverändert aus der Referenz)
docs/BUILD.md   Build-Pfade Linux und Windows
```

## Datenhaltung (geplant, SPEC §5)

100 % portabel: Konfiguration und Laufzeitdateien liegen in
`<Programmverzeichnis>/Data` — nie in `%APPDATA%`, nie im Arbeitsverzeichnis.

## Lizenz / Herkunft

Interne Anwendung. Design und Icon stammen aus der freigegebenen
QuasarDock-Referenz und sind eingefroren.
