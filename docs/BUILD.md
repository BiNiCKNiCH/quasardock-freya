# Build-Pfade — QuasarDock (Freya 0.5)

Verbindlich für alle Phasen (SPEC §8). Zwei Pfade: **Linux** (Entwickeln, Testen,
Belege) und **Windows** (das ausgelieferte Ziel).

---

## 1. Linux (Entwicklungs- und Prüfpfad)

### 1.1 Toolchain

| Werkzeug | Version auf der Entwicklungsbox |
|---|---|
| `cargo` | 1.100.0 (3acc79c0c 2026-11-05) |
| `rustc` | 1.100.0 (0ef1bc386 2026-11-09) |
| Edition | 2024 |
| `freya` | 0.5.2 (Crate `freya = "0.5"`, Skia 0.104 über `freya-skia-safe`) |

Nachrüsten, falls Rust fehlt:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o rustup-init.sh
sh rustup-init.sh -y --default-toolchain stable --profile minimal --no-modify-path
```

### 1.2 System-Bibliotheken

Freya rendert über Skia/GL/EGL und öffnet ein X11- bzw. Wayland-Fenster. Nötig
sind die Entwickler-Pakete:

```bash
sudo apt update
sudo apt install -y build-essential pkg-config \
  libx11-dev libxcb1-dev libxcb-xkb1 libxkbcommon-dev libxkbcommon-x11-0 \
  libgl1-mesa-dev libegl1 libgles2 libwayland-dev \
  libfontconfig1-dev libfreetype-dev
sudo apt install -y xvfb x11-utils imagemagick   # nur für Headless-Belege
```

> **Sonderfall dieser Entwicklungsbox (Sandbox, kein `sudo`).**
> Hier fehlten `libEGL`, `libGLESv2`, die `-dev`-Symlinks von `libGL`/
> `libwayland-egl` sowie `libxkbcommon-x11` / `libxcb-xkb`. Es wurde nichts
> systemweit installiert, sondern nur lokal bereitgestellt:
>
> 1. **Linken:** `LIBRARY_PATH` auf das Verzeichnis eines vorhandenen
>    lokalen Sysroot, das die Grafik-Libs enthält
>    (mit `libEGL.so`, `libGL.so`, `libGLESv2.so`, `libwayland-egl.so`).
> 2. **Laden:** derselbe Sysroot plus ein zweites lokales Lib-Verzeichnis in
>    `LD_LIBRARY_PATH`; dort liegen aus den Debian-Paketen
>    `libxkbcommon-x11-0` und `libxcb-xkb1` entpackte `.so`-Dateien
>    (`dpkg-deb -x`, Symlink `libxkbcommon-x11.so -> .so.0.0.0`).
> 3. `TMPDIR` muss auf ein Verzeichnis mit `exec`-Erlaubnis zeigen
>    (`/tmp` ist hier `noexec`, sonst scheitert bereits `rustup-init`).
>
> Diese drei Punkte sind reine Sandbox-Krücken. Auf einem normalen
> Linux-Rechner erledigt das die Paketliste aus 1.2.

### 1.3 Bauen, testen, starten

```bash
cargo build                    # Debug-Binary: target/debug/quasardock
cargo test                     # Quality-Gate: Unit-Tests
cargo run                      # Fenster öffnen
```

Headless (ohne Bildschirm, für Belege):

```bash
xvfb-run -a cargo run
```

Screenshot der laufenden App (Xvfb + ImageMagick):

```bash
Xvfb :99 -screen 0 1280x800x24 -nolisten tcp &
DISPLAY=:99 ./target/debug/quasardock &
sleep 8
DISPLAY=:99 import -window root screenshots/phase0-design-freeze.png
```

**Renderer-Hinweis (Beleg-Lauf).** Auf dieser Box liefert der GL-Pfad über
`llvmpipe`/EGL ein Bild mit vertauschten Rot- und Blaukanälen (gemessen:
Canvas `#1A1310` statt `#10131A`). Der Software-Renderer von Freya rendert
kanalrichtig:

```bash
FREYA_RENDERER=software DISPLAY=:99 ./target/debug/quasardock
```

Der Design-Beleg in `screenshots/` wurde so erzeugt. Die Anwendung selbst bleibt
bei `RendererPreference::Auto` — auf echter Hardware ist der GL-Pfad aktiv.
Verbindlich für alle Token-Belege: **FREYA_RENDERER=software**, sonst ist der
Kanal-Tausch des Sandbox-GL ein Messfehler und kein Designfehler.

### 1.4 Quality-Gates vor jedem Commit

```bash
cargo fmt --check
cargo test
cargo build
cargo check --target x86_64-pc-windows-msvc   # Typen/Features für Windows
```

---

## 2. Windows (Auslieferungspfad)

### 2.1 Entscheidung

**Der Windows-Build läuft auf GitHub Actions mit `windows-latest`.**
Kein Cross-Compile von Linux aus.

### 2.2 Warum (gemessen, nicht vermutet)

1. Skia wird von `freya-skia-bindings` als **natives** Artefakt gebaut bzw. als
   vorkompiliertes Windows-Binary geladen. Der Build-Skript-Pfad ist an die
   Ziel-Toolchain gebunden.
2. `cargo check --target x86_64-pc-windows-msvc` ist von dieser Linux-Box aus
   **grün** — Typprüfung und Feature-Auflösung funktionieren also cross.
   Das ist der Windows-Check, der vor jedem Commit läuft (SPEC §8).
3. Das **Linken** einer `.exe` braucht den MSVC-Linker (`link.exe`) und die
   Windows-SDK-Importbibliotheken. Beides gibt es unter Linux nicht; ein
   MinGW-Pfad (`x86_64-pc-windows-gnu`) wird von Skia/Freya nicht unterstützt.
   Der Versuch auf dieser Box endet eindeutig:

   ```
   $ cargo build --target x86_64-pc-windows-msvc
   error: linker `link.exe` not found
   note: the msvc targets depend on the msvc linker but `link.exe` was not found
   note: please ensure that Visual Studio 2017 or later, or Build Tools for
         Visual Studio were installed with the Visual C++ option
   ```

   Ergebnis: Objektcode und Typen sind für Windows korrekt, aber die EXE
   entsteht **nur auf einem Windows-Läufer**.

Damit ist der Pfad derselbe wie bei der Flutter-/Tauri-Referenz: CI auf
`windows-latest`, Artefakt als Download.

### 2.3 CI-Workflow

Datei: [`.github/workflows/windows.yml`](../.github/workflows/windows.yml) —
läuft bei Push auf `main`, bei Pull Requests und manuell.

Schritte: `checkout` → `dtolnay/rust-toolchain@stable` → `Swatinem/rust-cache`
→ `cargo test --all-targets` → `cargo clippy --all-targets -- -D warnings`
→ `cargo build --release --locked` → portables ZIP → `actions/upload-artifact`.

Artefakte je Lauf:

```
quasardock-windows-x64-portable.zip     (quasardock.exe + app-icon.png + README.md)
target/release/quasardock.exe
```

### 2.4 Dasselbe lokal auf einem Windows-Rechner

PowerShell, Rust-Toolchain mit MSVC-Target vorausgesetzt
([rustup.rs](https://rustup.rs), „Desktop development with C++" in den
Visual-Studio-Build-Tools installiert):

```powershell
rustup default stable-msvc
cargo test --all-targets
cargo build --release --locked
# Ergebnis: target\release\quasardock.exe
```

Portables Paket von Hand:

```powershell
New-Item -ItemType Directory -Force -Path staging | Out-Null
Copy-Item target\release\quasardock.exe staging\
Copy-Item app-icon.png staging\
Copy-Item README.md staging\
Compress-Archive -Path staging\* -DestinationPath quasardock-windows-x64-portable.zip
```

**Portabel** heißt: die EXE legt ihren `Data`-Ordner neben sich selbst an
(SPEC §5). Kein Installer nötig, keine Registry, kein `%APPDATA%`.
Installer (NSIS/MSI) sind optional und nicht Teil von Phase 0.

---

## 3. Smoketest-Checkliste — braucht echten Windows-Lauf

Die folgenden Punkte sind **plattformgebunden und auf der Linux-Box nicht
runtime-verifizierbar**. Sie sind hier als Smoketest gelistet und dürfen NICHT
als „verifiziert" behauptet werden — sie brauchen einen echten Windows-Lauf
(Windows 10/11, MSVC-Toolchain, wie in §2.3/§2.4).

| # | Punkt (SPEC) | Was prüfen | Erwartung |
|---|---|---|---|
| W1 | Tray-Icon immer vorhanden (§6.1) | App starten → Systray zeigt QuasarDock-Icon | Icon sichtbar, Menü „Zeigen/Verbergen" + „Beenden" |
| W2 | Kein Taskleisten-Eintrag (§6.1) | `skip_taskbar` / `WS_EX_TOOLWINDOW` | Fenster taucht NICHT in der Taskleiste auf |
| W3 | „Beim Start ins Tray minimieren" (§6.2) | Setting an → App startet ohne Fenster-Blitz | Fenster nicht sichtbar beim Start, nur Tray |
| W4 | Schließen (X) → verbergen (§6.3) | X klicken | Fenster weg, App läuft im Tray weiter; Config geflusht |
| W5 | Beenden nur via Tray (§6.3) | „Beenden" im Tray-Menü | Store wird geflusht, Prozess endet sauber |
| W6 | Windows-Autostart Registry (§6.6) | Setting an/aus → `HKCU\…\Run` Eintrag | Eintrag vorhanden/entfernt; Self-Heal bei stale Pfad |
| W7 | Echte App-Icons aus `.exe`/`.dll` (§7) | App mit `.exe`-Icon hinzufügen | PE-Ressourcen („MZ") decodiert, Icon sichtbar; `Data/logs/icons.txt` |
| W8 | Andocken an Taskleiste (§6.4) | „Am Bildschirmrand andocken" an → Fenster rechter Rand + Oberkante Taskleiste | `compute_dock_rect` nutzt `SPI_GETWORKAREA`-Höhe |
| W9 | Akzentfarbe wirkt (§6.7) | Accent-Preset/Hex ändern | Oberfläche übernimmt sofort (zentral `ACCENT`) |

Die reinen **Geometrie- und Logik-Rechnungen** (Andock-Threshold, `dock_rect`,
`taskbar_height`, Icon-Pfad-Klassifizierung, PE-Parsing, Registry-Befehls-
Formatierung, Store-Flush) sind plattformfrei und werden auf Linux per
`cargo test` abgedeckt (siehe `geometry.rs`, `icons.rs`, `autostart.rs`,
`store.rs`). Nur die *Anbindung an die Windows-API* (Tray, `SPI_GETWORKAREA`,
Registry, echtes PE-Laden) braucht W1–W8.

### 2.5 Windows-Icon

`icons/icon.ico` liegt unverändert im Repo (identisch zur Referenz, sha256
siehe Commit). Das Einbetten als EXE-Ressource erfolgt in Phase 3 zusammen mit
dem Icon-Loader; das Fenstericon ist bereits jetzt gesetzt
(`LaunchConfig::window_icon` mit `app-icon.png`).
