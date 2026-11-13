# QuasarDock (Freya) — Port-Spezifikation

> **Verbindlich für alle Phasen.** Diese Datei ist die eine Quelle der Wahrheit für den
> Neuaufbau von QuasarDock mit **Rust + Freya 0.5** (native Skia-UI, KEIN WebView).

## 0. Ziel und Rahmen

- Neues, eigenständiges Projekt in einem eigenen Projektverzeichnis.
  Das Tauri-Original ist **nur Lesereferenz** und wird **niemals verändert**:
  das Referenz-Repo (README.md, docs/ARCHITEKTUR.md,
  CHANGELOG.md, src/, src-tauri/). Exaktes Verhalten dort ablesen, nicht neu erfinden.
- Motiv: alternative Testversion ohne WebView (der WebView2-Stack zieht >100 MB RAM).
  RAM ist **kein K.o.-Kriterium**, aber ein Mess-/Lernziel (siehe §9).
- Sprache der UI: **Deutsch**. Doku (README/CHANGELOG/ARCHITEKTUR) auf Deutsch.

## 1. Stack

- **Rust + Freya 0.5** (crate `freya = "0.5"`), native Skia-Rendering, kein WebView,
  kein Node/TypeScript.
- Windows 10/11 als Ziel; Entwicklungsmaschine ist Linux. Build-/Testbarkeit auf der
  Linux-Box hat Vorrang (siehe §8).
- Kein Tauri, kein wry/tao-WebView. Tray/Autostart/Andocken über native Crates und
  Win32-Aufrufe (Details in den Phasen).

## 2. Design-Freeze (steht, verpflichtend)

- **Programmicon und Look sind eingefroren** (Nutzerfreigabe beim Tauri-Original) und
  werden unverändert übernommen. Das Programmicon (`app-icon.png`) und das Windows-ICO
  (`src-tauri/icons/icon.ico`) aus dem Referenz-Repo **kopieren, nicht verändern**.
- Keine Eigeninitiative zur „Verbesserung" von Farben/Icon/Layout. Design-Vorschläge des
  Workers nur als ausdrücklich gelabelte Empfehlung, nie als eingezogene Änderung.
- Die Akzentfarbe ist **nur** als optionale Einstellung erlaubt (Default unverändert Cyan).

### Design-Tokens (exakt übernehmen)

| Element | Wert |
|---|---|
| Canvas | `#10131A` |
| Controls / erhöhte Flächen | `#1A1E29` |
| Hover | `#1F2433` |
| Borders | `rgba(255,255,255,0.05–0.08)` |
| Akzent (aktive Zustände) | Cyan `#00B4D8` / `#00E5FF` |
| Positive Zustände | Smaragd `#22C55E` |
| Text primär / sekundär / gedimmt | `#FFFFFF` / `#94A3B8` / `#64748B` |
| Radius | 12–16 px (Kacheln/Karten), 8–10 px (kleine Elemente) |
| Schrift | Inter-artige Sans; Meta 11–13 px, Titel 20–24 px |
| Kontrast | mind. 14:1 auf Canvas (Referenz misst 18,58:1) |

Tokens per Pixel-Sampling eines Screenshots belegen (Skript in `dev/`), nie nur behaupten.

## 3. Layout (aus der Referenz-Skizze)

- Fenster ~1280×800 px, min ~900×600, resizable, dunkel, kein Start-Flash.
- **Links Hauptbereich (~70 %):** Toolbar oben (Ordnername + App-Anzahl, Suchfeld,
  Anzeigeoptionen-Menü „Kacheln/Liste" + „Manuell/Alphabetisch" rechts neben der Suche),
  darunter die Apps.
- **Rechts Sidebar (~30 %):** Kopf „QuasarDock" mit Akzent-Trennlinie, darunter die
  Ordnerliste (mit App-Anzahl, aktiv mit Cyan-Indikator), unten rechts runder
  Einstellungs-Button.
- **Eigene Titelleiste** (keine native Dekoration): Design-Band mit nur einem
  Schließen-Knopf (X). Fensterziehen über das Band. **Kein** Minimieren/Maximieren.
  Schließen verbirgt immer ins Tray.

## 4. Feature-Umfang (vollständig, Stand v2.8 der Referenz)

1. **Kachel- und Listenansicht** mit Umschaltung; Sortierung manuell oder alphabetisch.
2. **Freie Ordnung:** Apps per Drag & Drop im Grid und in der Liste verschieben, auch
   direkt in Sidebar-Ordner. Eigene Pointer-/Maus-DnD (Ghost, Platzhalter, Einfüge-Indikator,
   Sidebar-Drop mit Cyan-Highlight), sofort persistiert.
3. **Frei definierbare Ordner** (anlegen/umbenennen/löschen); der geschützte Standardordner
   heißt **„Favoriten"** und ist umbenennbar, aber nicht löschbar. Löschen verschiebt
   enthaltene Apps in „Favoriten" (nie Löschen ohne Ziel).
4. **Suche** über die Apps des aktuellen Ordners bzw. aller Ordner.
5. **Einstellungen** als Overlay im App-Fenster, Tabs:
   - **Ordner** (anlegen/umbenennen/löschen/Reihenfolge)
   - **Apps** (Ordner-Auswahl oben, Liste der Apps des Ordners, Hinzufügen/Bearbeiten/Löschen)
   - **Allgemein** (Beim Start ins Tray minimieren · Automatisch ins Tray minimieren nach
     App-Start · Am Bildschirmrand andocken · Fenstergröße und -position merken ·
     Windows-Autostart)
   - **Darstellung** (Akzentfarbe: Voreinstellungen + eigene Hex-Farbe)
   - **Über** (Version, Build, Status der Datenablage)
6. **System-Tray-Icon** immer vorhanden: Menü „Zeigen/Verbergen" + „Beenden".
7. **Echte App-Icons** in Kachel- und Listenansicht: PNG/ICO-Dateien sowie eingebettete
   Symbole aus `.exe`/`.dll` (PE-Ressourcen). Fehlgeschlagene Läufe → `Data/logs/icons.txt`.
8. **App-Optionen pro Eintrag:** Startzustand (Normal/Minimiert/Maximiert) und
   automatischer Start (mit Startketten-Verzögerung).
9. **App hinzufügen/bearbeiten:** Name, Pfad (Datei-Dialog), Argumente, Arbeitsverzeichnis,
   optional Icon-Datei. Starten detached; Fehler als Klartext-Toast, nie als Panic.

## 5. Datenhaltung (100 % portabel, steht)

- Konfiguration IMMER unter `<Programmverzeichnis>/Data/config.json`; Laufzeitdateien in
  `Data/` (`logs/quasardock.log`, `logs/icons.txt`). Pfad aus dem **tatsächlichen
  EXE-/Prozess-Pfad** ableiten, nie aus dem Working Directory, nie aus `%APPDATA%`.
- **Keine** Speicherort-Option anbieten. Nicht beschreibbarer Programmordner → klare
  Toast-/Statusmeldung, kein stiller Fallback.
- Schreiben **atomar** (Temp-Datei + Rename), Auto-Save mit ~600 ms Debounce. Korrupte
  Datei → Original in Quarantäne, Start mit Defaults.
- Datenmodell: `Config { folders, apps, settings }`, `Settings`, `AppEntry { id, name, path,
  args, cwd, icon, folder_id, sort_order, start_state, auto_start }`, `Folder { id, name,
  protected }` — mit `normalize()` (doppelte IDs, Ordnerzuordnung, Favoriten-Schutz) und
  Migrationen älterer Feldnamen; unbekannte Felder beim Laden akzeptieren, beim nächsten
  Save sauber neu schreiben.

## 6. Fenster-/Tray-Verhalten (exakt wie Referenz)

- **Kein Taskleisten-Eintrag** (`skip_taskbar`; Windows `WS_EX_TOOLWINDOW`).
- **„Beim Start ins Tray minimieren"**: Fenster von Beginn weg **versteckt erzeugen**,
  nie Show-then-Hide (kein Fenster-Blitz). „Fenstergröße/-position merken" speichert nur
  aus sichtbaren Zuständen — ein nie gezeigtes Fenster überschreibt die Geometrie nicht.
- **„Automatisch ins Tray minimieren"** nach App-Start bleibt unabhängig.
- **Andocken** als EINE Option „Am Bildschirmrand andocken": Fenster bündig an den rechten
  Monitorrand UND bündig an die Oberkante der Taskleiste. Taskleistenhöhe NICHT fix
  verdrahten, sondern über die Windows-Arbeitsfläche ermitteln
  (`SystemParametersInfoW(SPI_GETWORKAREA)`), Linux-Fallback `monitor.size()`. Reine
  Geometrie-Rechnung plattformfrei (unit-testbar), Systemaufruf hinter `#[cfg(windows)]`.
- Schließen (X) → Geometrie + offene Änderungen speichern, Fenster verbergen (App läuft im
  Tray weiter). Beenden nur über Tray-Menü „Beenden".

## 7. Icons (Windows)

- Icon-Loader: PNG direkt; ICO-Mehrbild dekomprimieren; `.exe`/`.dll` per PE-Ressourcen
  („MZ"-Kennung). Fehlende/defekte Quelle → Buchstaben-Fallback, nie Toast-Spam.
- Jede `get_app_icon`-Anfrage → eine Zeile `Data/logs/icons.txt` (App, Pfad, Ergebnis,
  Fehlerort). Windows-Pfadformen (Backslash, Laufwerksbuchstabe) testen.

## 8. Build-Pfad (in Phase 0 verbindlich klären und in `docs/BUILD.md` festhalten)

- **Schritt 1 in Phase 0:** Freya 0.5 muss auf der Linux-Box bauen UND starten
  (headless via `xvfb-run`). `cargo --version` prüfen; fehlende Rust-Toolchain per rustup
  nachrüsten, fehlende System-Libs (Skia-/GL-/X11-dev) dokumentieren.
- **Schritt 2:** Windows-EXE-Pfad bestimmen. Realistisch ist Cross-Compile NUR schwer
  (Skia); wahrscheinlich nötig: **GitHub Actions `windows-latest`** (wie bei der
  Flutter-Referenz). Was immer gewählt wird: in `docs/BUILD.md` mit exakten Befehlen
  festhalten, später Phasen folgen dieser Datei. Portable-Einzel-EXE ist der Demo-Pfad;
  Installer (NSIS/MSI) optional.
- Qualitätstore vor jedem Commit: `cargo test` (Linux) und `cargo check --target
  x86_64-pc-windows-msvc --all-targets` (falls Cross-Target möglich; sonst der in
  `docs/BUILD.md` dokumentierte Windows-Check).

## 9. RAM-Messung (Lernziel, kein K.o.-Kriterium)

- In der End-Abnahme (Phase 4) die tatsächliche Speichernutzung messen und dokumentieren:
  Linux-Binary (RSS/Working-Set im Leerlauf, z. B. via `/proc/<pid>/status` VmRSS) und,
  falls verfügbar, der Windows-EXE-Wert. Vergleichswert: Tauri/WebView2 ~100 MB.
- Ziel ist eine **belastbare Zahl**, nicht ein Mindestwert. Messmethode im Beleg dokumentieren.

## 10. Tests (Regeln aus der Referenz, immer anwenden)

- Keine fest verdrahteten `/tmp`-Pfade: `std::env::temp_dir()` und compile-time Basis über
  `env!("CARGO_MANIFEST_DIR")`; Testordner prozess-eindeutig (`std::process::id()`).
- Programmstart-/Unix-Tests hinter `#[cfg(unix)]` kapseln, je mit `#[cfg(windows)]`-Zwilling
  (`%COMSPEC% /c exit 0` vs. `/bin/ls`), damit Windows-CI dieselben Tests ausführen kann.
- Persistenz-Tests: Atomarität, Debounce, Migration, Quarantäne, Portabilität (nichts in
  AppData), Favoriten-Schutz.

## 11. Repo-Hygiene (steht, von Anfang an)

- Repo zeigt **nur die App**: Quellcode, Doku, CI-Workflow, aktuelle Screenshots. README
  schlank (~150–200 Zeilen). Versionshistorie in CHANGELOG.md, Technik in
  docs/ARCHITEKTUR.md.
- QA-/Beleg-Werkzeug (`dev/`-Skripte, alte Screenshot-Sätze, delivery-Binaries) gehört in
  eine lokale Archivarbeitskopie `<projekt>-dev/` **neben** dem Repo — nicht ins Repo.
- **Buildprodukte/Binaries nicht committen** (User-Vorgabe).
- **Agentenname nirgends**: nicht in Commit-Metadaten (Autoren-Identität = GitHub-Konto des
  Users, noreply-E-Mail), nicht in Dateiinhalten, nicht in Belegen.

## 12. Abnahme-Phasen (gated)

- **Phase 0** — Machbarkeit + Scaffold + Design-Freeze + Build-Pfad.
- **Phase 1** — Datenmodell + portabler Store + Migrationen (Rust-Kern).
- **Phase 2** — UI: Grid/Liste/Sidebar/Suche/Einstellungen/DnD (Design-Parität).
- **Phase 3** — Systemintegration: Tray/Andocken/Autostart/Icons/App-Start.
- **Phase 4** — End-Abnahme + Windows-EXE + RAM-Messung + Doku.

Jede Phase committet einmal; ein Commit je Phase.
