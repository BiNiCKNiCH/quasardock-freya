//! Tray-Icon (immer vorhanden): Menü „Zeigen/Verbergen" + „Beenden".
//!
//! Der Tray-Thread ist von Freya entkoppelt; Befehle werden über einen `mpsc`-Kanal
//! an den Freya-Thread übertragen (die `Platform` ist nicht `Send`).
//!
//! Unter Windows wird das echte Tray via `tray-icon` aufgebaut (Windows-only-Dependency,
//! damit der Linux-Build schlank und linkbar bleibt). Auf Nicht-Windows liefert
//! `spawn_tray` `None` — die App läuft dann ohne System-Tray; das Fenster-Verhalten
//! (versteckt erzeugen, zeigen/verbergen) wird über den F9-Debug-Schalter bzw. die
//! Tray-Logik im UI-Thread getestet. Ein echter Windows-Lauf ist in der
//! Smoketest-Checkliste dokumentiert.

use std::sync::mpsc::Receiver;

// Unter Windows wird das echte Tray via `tray-icon` aufgebaut. Die Crate ist nur
// für das Windows-Target deklariert, damit der Linux-Build schlank und linkbar
// bleibt (Freya's eigene `tray`-Feature würde tray-icon auch auf Linux ziehen).
#[cfg(windows)]
use std::sync::mpsc::channel;
#[cfg(windows)]
use tray_icon::menu::{Menu, MenuItem};
#[cfg(windows)]
use tray_icon::{Icon, TrayIconBuilder};

/// Befehl vom Tray-Menü an den Freya-Thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// Fenster anzeigen/verbergen (Tray bleibt).
    Toggle,
    /// Beenden: vorher Store flushen (im UI-Thread behandelt).
    Quit,
}

/// Startet den Tray. Liefert den `Receiver` für [`TrayCommand`], oder `None`, wenn kein
/// Tray erstellt werden konnte (z. B. headless / kein System-Tray vorhanden).
pub fn spawn_tray(accent: (u8, u8, u8)) -> Option<Receiver<TrayCommand>> {
    spawn_tray_inner(accent)
}

#[cfg(windows)]
fn spawn_tray_inner(accent: (u8, u8, u8)) -> Option<Receiver<TrayCommand>> {
    let (tx, rx) = channel::<TrayCommand>();
    let tx_toggle = tx.clone();
    let tx_quit = tx.clone();

    let (bytes, w, h) = make_icon(accent);
    let icon = Icon::from_rgba(bytes, w, h).ok()?;

    let menu = Menu::new();
    let item_toggle = MenuItem::with_id("toggle", "Zeigen / Verbergen", true, None);
    let item_quit = MenuItem::with_id("quit", "Beenden", true, None);
    menu.append(&item_toggle).ok()?;
    menu.append(&item_quit).ok()?;

    let handle = TrayIconBuilder::new()
        .with_tooltip("QuasarDock")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .ok()?;

    // Menü-Ereignisse abfangen: tray-icon 0.20 hat KEIN `on_menu_event` am
    // Builder, sondern einen globalen `TrayIconEvent`-Kanal. Wir hören in einem
    // eigenen Thread und leiten die Befehle per mpsc an den Freya-Thread weiter.
    std::thread::spawn(move || {
        loop {
            match tray_icon::TrayIconEvent::receiver().recv() {
                Ok(event) => match event.id().as_ref() {
                    "toggle" => {
                        let _ = tx_toggle.send(TrayCommand::Toggle);
                    }
                    "quit" => {
                        let _ = tx_quit.send(TrayCommand::Quit);
                    }
                    _ => {}
                },
                Err(_) => break,
            }
        }
    });

    // `TrayIcon` ist nicht `Send` (enthält `Rc<RefCell<…>>`) und kann daher nicht in
    // einen eigenen Thread verschoben werden. Wir lassen es bewusst „leaken", damit
    // das Icon für die gesamte Laufzeit der App (bis zum Prozess-Ende via „Beenden")
    // erhalten bleibt. Die Fenster-Nachrichten (Menü-Klicks) werden ohnehin vom
    // winit-Message-Loop der App dispatchet, nicht von diesem Thread.
    std::mem::forget(handle);
    Some(rx)
}

#[cfg(not(windows))]
fn spawn_tray_inner(_accent: (u8, u8, u8)) -> Option<Receiver<TrayCommand>> {
    // Kein System-Tray auf dieser Plattform (z. B. headless Linux). Die App läuft
    // weiter; Fenster-Steuerung erfolgt über den F9-Debug-Schalter / Start-Logik.
    None
}

/// Synthetisiert ein einfaches, plattformneutrales Tray-Icon (Akzent-Quadrat mit
/// transparentem Rand), da kein Bild-Asset zur Laufzeit geladen werden soll.
#[cfg(windows)]
fn make_icon(rgb: (u8, u8, u8)) -> (Vec<u8>, u32, u32) {
    let size = 32u32;
    let mut buf = Vec::with_capacity((size * size * 4) as usize);
    let inset = 5u32;
    for y in 0..size {
        for x in 0..size {
            let inside = x >= inset && x < size - inset && y >= inset && y < size - inset;
            if inside {
                buf.extend_from_slice(&[rgb.0, rgb.1, rgb.2, 255]);
            } else {
                buf.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    (buf, size, size)
}
