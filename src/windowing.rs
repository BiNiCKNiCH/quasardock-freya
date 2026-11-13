//! Fenster-Steuerung: Sichtbarkeit, Geometrie, Andocken, Skip-Taskbar.
//!
//! Plattformübergreifend über Freya's `Platform::with_window`. Windows-spezifisches
//! (Skip-Taskbar via `WS_EX_TOOLWINDOW`) ist in einem `#[cfg(windows)]`-Block gekapselt
//! und wird nur auf Windows kompiliert.

use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;
use std::rc::Rc;

use freya::prelude::*;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::window::{Window, WindowId};

use crate::geometry::{self, ScreenRect};
use crate::model::WindowGeom;
use crate::paths;

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
        win.set_outer_position(PhysicalPosition::new(x, y));
        let _ = win.request_inner_size(PhysicalSize::new(w as u32, h as u32));
    });
}

/// Monitor- und Arbeitsflächenrechteck des Monitors, auf dem das Fenster gerade
/// steht — in PHYSISCHEN (Device-)Pixeln, konsistent zu `outer_position`.
///
/// Unter Windows via `GetMonitorInfoW` des Fenster-Monitors (monitor-spezifisch,
/// DPI-korrekt; `rcWork` schließt die Taskleiste aus). Unter Linux via
/// `current_monitor()` (Arbeitsfläche == Monitor, da keine Taskleiste im X11-Sinn).
/// Die reine Andock-Rechnung in `geometry` arbeitet anschließend plattformfrei mit
/// diesen Rechtecken weiter.
pub fn monitor_rects_for_window(window: &Window) -> (ScreenRect, ScreenRect) {
    #[cfg(windows)]
    {
        monitor_rects_windows(window)
    }
    #[cfg(not(windows))]
    {
        monitor_rects_linux(window)
    }
}

#[cfg(windows)]
fn monitor_rects_windows(window: &Window) -> (ScreenRect, ScreenRect) {
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO};
    use winit::platform::windows::MonitorHandleExtWindows;

    // Monitor des Fensters (nicht nur primär) via HMONITOR + GetMonitorInfoW.
    // rcMonitor/rcWork sind physische Device-Pixel, konsistent zu outer_position.
    if let Some(hmon) = window.current_monitor().map(|m| m.hmonitor()) {
        let mut mi: MONITORINFO = unsafe { std::mem::zeroed() };
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if unsafe { GetMonitorInfoW(hmon, &mut mi) } != 0 {
            let monitor = ScreenRect {
                x: mi.rcMonitor.left,
                y: mi.rcMonitor.top,
                width: mi.rcMonitor.right - mi.rcMonitor.left,
                height: mi.rcMonitor.bottom - mi.rcMonitor.top,
            };
            let work = ScreenRect {
                x: mi.rcWork.left,
                y: mi.rcWork.top,
                width: mi.rcWork.right - mi.rcWork.left,
                height: mi.rcWork.bottom - mi.rcWork.top,
            };
            return (monitor, work);
        }
    }
    // Fallback: primäre Arbeitsfläche via SPI_GETWORKAREA (Taskleiste abgezogen).
    let work = geometry::spi_work_area().unwrap_or(ScreenRect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    });
    (work, work)
}

#[cfg(not(windows))]
fn monitor_rects_linux(window: &Window) -> (ScreenRect, ScreenRect) {
    if let Some(m) = window.current_monitor() {
        let size = m.size();
        let pos = m.position();
        let monitor = ScreenRect {
            x: pos.x,
            y: pos.y,
            width: size.width as i32,
            height: size.height as i32,
        };
        return (monitor, monitor);
    }
    // Umgebungs-Fallback (Headless/CI ohne Monitor).
    let (m, w) = geometry::primary_screen();
    (m, w)
}

/// Dockt das Fenster an, falls es nahe der rechten Kante ist (rechte Kante + Oberkante
/// der Taskleiste). Reine Geometrie-Rechnung aus `geometry`, hier nur angewandt.
/// Läuft bei jedem Fenster-Zug (Ziehen) und beim Start, solange die Option an ist.
pub fn dock_if_near(platform: &Platform, window_id: WindowId) {
    let out = Rc::new(RefCell::new(None));
    let out_c = out.clone();
    platform.with_window(window_id, move |win| {
        let (monitor, work) = monitor_rects_for_window(win);
        if let Ok(pos) = win.outer_position() {
            let size = win.outer_size();
            let wrect = ScreenRect {
                x: pos.x,
                y: pos.y,
                width: size.width as i32,
                height: size.height as i32,
            };
            let docked = geometry::maybe_dock(wrect, monitor, work, geometry::DOCK_THRESHOLD);
            let changed = docked != wrect;
            log_dock_attempt(&wrect, &monitor, &work, changed, &docked);
            if changed {
                *out_c.borrow_mut() = Some(docked);
            }
        }
    });
    if let Some(d) = out.borrow_mut().take() {
        apply_geometry(
            platform,
            window_id,
            &WindowGeom {
                x: d.x,
                y: d.y,
                w: d.width,
                h: d.height,
            },
        );
    }
}

/// Erzwingt das Andocken (unabhängig von der Schwelle) — für den Start, wenn die
/// Option „Am Bildschirmrand andocken" aktiv ist (SPEC §6).
pub fn dock_now(platform: &Platform, window_id: WindowId) {
    let out = Rc::new(RefCell::new(None));
    let out_c = out.clone();
    platform.with_window(window_id, move |win| {
        let (monitor, work) = monitor_rects_for_window(win);
        let size = win.outer_size();
        let w = size.width as i32;
        let h = size.height as i32;
        let r = geometry::dock_rect(w, h, monitor, work);
        let input = ScreenRect {
            x: 0,
            y: 0,
            width: w,
            height: h,
        };
        log_dock_attempt(&input, &monitor, &work, true, &r);
        *out_c.borrow_mut() = Some(r);
    });
    if let Some(d) = out.borrow_mut().take() {
        apply_geometry(
            platform,
            window_id,
            &WindowGeom {
                x: d.x,
                y: d.y,
                w: d.width,
                h: d.height,
            },
        );
    }
}

/// Eine Trace-Zeile je Dock-Versuch nach `Data/logs/quasardock.log`, damit der
/// User am echten Windows die Ursache nachliefern kann (Defekt 1: Debugierbarkeit).
fn log_dock_attempt(
    win: &ScreenRect,
    monitor: &ScreenRect,
    work: &ScreenRect,
    changed: bool,
    result: &ScreenRect,
) {
    if let Ok(dir) = paths::logs_dir() {
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("quasardock.log"))
        {
            let _ = writeln!(
                f,
                "dock_attempt window={{x:{},y:{},w:{},h:{}}} monitor={{x:{},y:{},w:{},h:{}}} work={{w:{},h:{}}} -> changed={} result={{x:{},y:{},w:{},h:{}}}",
                win.x,
                win.y,
                win.width,
                win.height,
                monitor.x,
                monitor.y,
                monitor.width,
                monitor.height,
                work.width,
                work.height,
                changed,
                result.x,
                result.y,
                result.width,
                result.height
            );
        }
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
