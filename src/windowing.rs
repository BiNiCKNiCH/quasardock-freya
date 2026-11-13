//! Fenster-Steuerung: Sichtbarkeit, Geometrie, Andocken, Skip-Taskbar.
//!
//! Plattformübergreifend über Freya's `Platform::with_window`. Windows-spezifisches
//! (Skip-Taskbar via `WS_EX_TOOLWINDOW`) ist in einem `#[cfg(windows)]`-Block gekapselt
//! und wird nur auf Windows kompiliert.

use freya::prelude::*;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::window::{Window, WindowId};

use crate::geometry::{self, ScreenRect};
use crate::model::WindowGeom;

/// Versteckt das Fenster (App läuft im Tray weiter).
pub fn set_visible(platform: &Platform, window_id: WindowId, visible: bool) {
    platform.with_window(window_id, move |w| {
        w.set_visible(visible);
    });
}

/// Liest die aktuelle Fenstergeometrie (logische Pixel). `None`, falls nicht ermittelbar.
pub fn read_geometry(platform: &Platform, window_id: WindowId) -> Option<WindowGeom> {
    let out = std::rc::Rc::new(std::cell::RefCell::new(None));
    let out_c = out.clone();
    platform.with_window(window_id, move |win| {
        if let Ok(pos) = win.outer_position() {
            let size = win.outer_size();
            *out_c.borrow_mut() = Some(WindowGeom {
                x: pos.x,
                y: pos.y,
                w: size.width as i32,
                h: size.height as i32,
            });
        }
    });
    out.borrow_mut().take()
}

/// Wendet eine Geometrie auf das Fenster an (sichtbarer Zustand).
pub fn apply_geometry(platform: &Platform, window_id: WindowId, geom: &WindowGeom) {
    let (x, y, w, h) = (geom.x, geom.y, geom.w, geom.h);
    platform.with_window(window_id, move |win| {
        let _ = win.set_outer_position(PhysicalPosition::new(x, y));
        let _ = win.request_inner_size(PhysicalSize::new(w as u32, h as u32));
    });
}

/// Dockt das Fenster an, falls es nahe einer Kante ist (rechte Kante + Oberkante der
/// Taskleiste). Reine Geometrie-Rechnung aus `geometry`, hier nur angewandt.
pub fn dock_if_near(platform: &Platform, window_id: WindowId) {
    let (monitor, work) = geometry::primary_screen();
    let out = std::rc::Rc::new(std::cell::RefCell::new(None));
    let out_c = out.clone();
    platform.with_window(window_id, move |win| {
        if let Ok(pos) = win.outer_position() {
            let size = win.outer_size();
            let wrect = ScreenRect {
                x: pos.x,
                y: pos.y,
                width: size.width as i32,
                height: size.height as i32,
            };
            let docked = geometry::maybe_dock(wrect, monitor, work, geometry::DOCK_THRESHOLD);
            if docked != wrect {
                *out_c.borrow_mut() = Some(docked);
            }
        }
    });
    if let Some(d) = out.borrow_mut().take() {
        apply_geometry(platform, window_id, &WindowGeom {
            x: d.x,
            y: d.y,
            w: d.width,
            h: d.height,
        });
    }
}

/// Setzt die Fensterdekoration (native Titelleiste) an/aus.
pub fn set_decorations(platform: &Platform, window_id: WindowId, decorations: bool) {
    platform.with_window(window_id, move |w| {
        w.set_decorations(decorations);
    });
}

/// Windows: Fenster aus der Taskleiste verbannen.
///
/// Winit 0.30 bietet mit `set_skip_taskbar(true)` den unter Windows 10/11
/// verlässlich funktionierenden Weg, den Taskleisten-Eintrag zu unterdrücken
/// (die SPEC nennt das legacy `WS_EX_TOOLWINDOW` — `set_skip_taskbar` ist die
/// moderne, robuste Entsprechung und erfüllt SPEC §6.1: „kein Taskleisten-
/// Eintrag"). Für den `with_window_handle`-Hook (bekommt `&mut Window` direkt).
#[cfg(windows)]
pub fn platform_set_skip_taskbar(window: &mut Window) {
    use winit::platform::windows::WindowExtWindows;
    window.set_skip_taskbar(true);
}

#[cfg(not(windows))]
pub fn platform_set_skip_taskbar(_window: &mut Window) {
    // Kein Windows → keine Aktion (dokumentiert in der Smoketest-Checkliste).
}
