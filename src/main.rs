//! QuasarDock — nativer App-Launcher auf Basis von Rust + Freya (Skia).
//!
//! Phase 0: Grundgerüst, Design-Freeze und Machbarkeitsnachweis.

// Kein Konsolenfenster auf Windows (PE-Subsystem GUI); auf anderen Plattformen
// ohne Wirkung.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod ui; // UI (Phase 2) + Systemintegration-Verdrahtung (Phase 3).

use freya::prelude::*;
use quasardock::design;
use quasardock::store::Store;
use winit::dpi::{LogicalPosition, LogicalSize};

/// Eingefrorenes Programmicon (unverändert aus der Referenz übernommen).
const APP_ICON: &[u8] = include_bytes!("../app-icon.png");

fn main() {
    // Einstellungen laden: Start-Verhalten (Tray/Minimieren), gespeicherte
    // Geometrie und Akzent initialisieren.
    let (start_hidden, remember_geometry, saved_geometry, dock_at_start) = Store::open(None)
        .ok()
        .map(|s| {
            let c = s.config();
            (
                c.settings.start_in_tray,
                c.settings.remember_geometry,
                c.settings.geometry,
                c.settings.dock_to_edge,
            )
        })
        .unwrap_or((false, true, None, false));

    // Zentrale Akzent-Konstante aus gespeicherter Einstellung (oder Default) setzen.
    {
        let accent = Store::open(None)
            .ok()
            .and_then(|s| s.config().settings.accent_color);
        design::init_accent_from(accent.as_deref());
    }

    // Self-Heal des Autostart-Eintrags: wenn die Einstellung aktiv ist, aber der
    // gespeicherte Pfad stale ist (z. B. nach Verschieben der EXE), wird er
    // frisch geschrieben. Keine Wirkung, wenn die Einstellung aus ist.
    if let Ok(s) = Store::open(None) {
        let enabled = s.config().settings.windows_autostart;
        if enabled && let Err(e) = quasardock::autostart::self_heal_autostart(true) {
            eprintln!("[autostart] self-heal: {}", e);
        }
    }

    // Auto-Start-Kette (verzögerte App-Starts) beim Programmstart abfeuern.
    if let Ok(s) = Store::open(None) {
        let apps = s.config().apps.clone();
        quasardock::launcher::run_auto_start(&apps);
    }

    // Tray-Icon (Windows). Unter Linux kein System-Tray; das Fenster wird ggf.
    // versteckt erzeugt (start_in_tray). Der Tray-Thread sendet Befehle
    // (Toggle = zeigen/verbergen, Quit = flushen + beenden).
    if let Some(rx) = quasardock::tray::spawn_tray((0, 180, 216)) {
        std::thread::spawn(move || {
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    quasardock::tray::TrayCommand::Toggle => {
                        let p = Platform::get();
                        let id = Platform::window_id();
                        p.with_window(id, |w| {
                            let vis = w.is_visible().unwrap_or(false);
                            w.set_visible(!vis);
                        });
                    }
                    quasardock::tray::TrayCommand::Quit => {
                        if let Ok(s) = Store::open(None) {
                            let _ = s.save_now();
                        }
                        Platform::get().exit();
                    }
                }
            }
        });
    }

    launch(
        LaunchConfig::new().with_window(
            WindowConfig::new(ui::app)
                .with_title("QuasarDock")
                .with_size(design::WINDOW_WIDTH, design::WINDOW_HEIGHT)
                .with_min_size(design::WINDOW_MIN_WIDTH, design::WINDOW_MIN_HEIGHT)
                .with_background(design::CANVAS)
                .with_icon(LaunchConfig::window_icon(APP_ICON))
                // Keine native Dekoration: die eigene Titelleiste (X aus Phase 2) übernimmt.
                .with_decorations(false)
                // "Beim Start ins Tray minimieren": Fenster von Beginn weg versteckt erzeugen
                // (kein Show-then-Hide, kein Fenster-Blitz).
                .with_window_attributes(move |attrs, _ev| {
                    let mut a = attrs;
                    if dock_at_start {
                        // "Am Bildschirmrand andocken": Fenster bündig an den rechten
                        // Monitorrand + Oberkante der Taskleiste (reine Geometrie-Rechnung,
                        // plattformfrei — siehe geometry::dock_rect).
                        let (monitor, work) = quasardock::geometry::primary_screen();
                        let dock_w = 400i32;
                        let dock_h = work.height;
                        let r = quasardock::geometry::dock_rect(dock_w, dock_h, monitor, work);
                        a = a.with_position(LogicalPosition::new(r.x as f64, r.y as f64));
                        a = a.with_inner_size(LogicalSize::new(r.width as f64, r.height as f64));
                    } else if let Some(g) =
                        saved_geometry.filter(|_| remember_geometry && !start_hidden)
                    {
                        // Gespeicherte Fenstergeometrie wiederherstellen (nur aus sichtbaren
                        // Zuständen: nicht, wenn wir versteckt starten).
                        a = a.with_position(LogicalPosition::new(g.x as f64, g.y as f64));
                        a = a.with_inner_size(LogicalSize::new(g.w as f64, g.h as f64));
                    }
                    if start_hidden {
                        // Freya 0.5.2 zeigt das Fenster nach der Erzeugung unconditionally
                        // an (freya-winit window.rs:367, `window.set_visible(true)`), ein
                        // `with_visible(false)` würde überschrieben. Wir erzeugen das Fenster
                        // daher off-screen, um ohne Blitz versteckt zu starten
                        // (kein Fenster-Blitz, SPEC §6). Das Tray (Windows) bzw. F9 (Linux)
                        // holt es bei Bedarf zurück.
                        a = a.with_position(LogicalPosition::new(-32000.0, -32000.0));
                    }
                    a
                })
                // Schließen (X) versteckt das Fenster nur (Beenden über das Tray-Menü).
                .with_on_close(|_ctx, _id| CloseDecision::KeepOpen)
                // Windows: Taskleisten-Eintrag unterdrücken (WS_EX_TOOLWINDOW).
                .with_window_handle(|_window| {
                    #[cfg(windows)]
                    quasardock::windowing::platform_set_skip_taskbar(_window);
                }),
        ),
    );
}
