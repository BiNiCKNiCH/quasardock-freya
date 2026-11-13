//! Fenster-Geometrie und Andocken (SPEC §6).
//!
//! Die eigentliche Rechnung ist plattformfrei und vollständig unit-testbar
//! (`dock_rect`, `maybe_dock`, `taskbar_height`). Die Beschaffung der
//! Monitor-/Arbeitsflächenwerte steckt hinter plattform-spezifischen Hilfen:
//! unter Windows wird die Taskleisten-Höhe über `SystemParametersInfoW`
//! (`SPI_GETWORKAREA`) ermittelt, unter Linux über `monitor.size()` (bzw.
//! im Headless-Fall über einen Umgebungswert).

/// Rechteck in logischen Pixeln (Bildschirmkoordinaten).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl ScreenRect {
    pub fn right(&self) -> i32 {
        self.x + self.width
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }
}

/// Schwellwert (logische Pixel), ab dem ein Fenster zum Rand andockt (SPEC §6).
pub const DOCK_THRESHOLD: i32 = 16;

/// Höhe der Taskleiste: Differenz zwischen Monitor- und Arbeitsfläche.
///
/// Unter Windows ist die Arbeitsfläche (`SPI_GETWORKAREA`) kleiner als der
/// Monitor, weil die Taskleiste Platz wegnimmt; unter Linux (keine Taskleiste
/// im X11-Sinn) ist die Differenz in der Regel 0.
pub fn taskbar_height(work_area: ScreenRect, monitor: ScreenRect) -> i32 {
    (monitor.height - work_area.height).max(0)
}

/// Liegt die rechte Kante des Fensters innerhalb `threshold` Pixel am rechten
/// Monitorrand? Dann soll angedockt werden.
pub fn is_near_right_edge(window_x: i32, window_w: i32, monitor_right: i32, threshold: i32) -> bool {
    (monitor_right - (window_x + window_w)).abs() <= threshold
}

/// Berechnet die angedockte Geometrie: bündig am rechten Monitorrand und
/// bündig an der Oberkante der Taskleiste (Fensterunterkante = Taskleisten-
/// oberkante).
pub fn dock_rect(window_w: i32, window_h: i32, monitor: ScreenRect, work_area: ScreenRect) -> ScreenRect {
    let taskbar = taskbar_height(work_area, monitor);
    let monitor_right = monitor.right();
    let x = monitor_right - window_w;
    // Oberkante der Taskleiste = untere Monitorkante - Taskleistenhöhe.
    let taskbar_top = monitor.bottom() - taskbar;
    // Fensterunterkante soll an der Taskleisten-Oberkante liegen.
    let y = (taskbar_top - window_h).max(monitor.y);
    ScreenRect {
        x,
        y,
        width: window_w,
        height: window_h,
    }
}

/// Hochwertige Andock-Entscheidung: nur wenn das Fenster nahe der rechten Kante
/// ist, wird die angedockte Geometrie zurückgegeben, sonst die übergebene.
pub fn maybe_dock(window: ScreenRect, monitor: ScreenRect, work_area: ScreenRect, threshold: i32) -> ScreenRect {
    if is_near_right_edge(window.x, window.width, monitor.right(), threshold) {
        dock_rect(window.width, window.height, monitor, work_area)
    } else {
        window
    }
}

/// Liefert Monitor- und Arbeitsflächenrechteck der primären Anzeige.
///
/// Unter Windows wird die Arbeitsfläche per `SPI_GETWORKAREA` geholt (Taskleisten-
/// höhe korrekt), unter Linux über `monitor.size()` (Fallback: Umgebung
/// `QD_MONITOR` = `WxH`, sonst 1280×800).
pub fn primary_screen() -> (ScreenRect, ScreenRect) {
    let monitor = fallback_monitor();
    let work_area = work_area_for(monitor);
    (monitor, work_area)
}

#[cfg(windows)]
fn work_area_for(monitor: ScreenRect) -> ScreenRect {
    // Taskleisten-Höhe über SystemParametersInfoW(SPI_GETWORKAREA).
    match spi_work_area() {
        Some(wa) => wa,
        None => monitor,
    }
}

#[cfg(not(windows))]
fn work_area_for(monitor: ScreenRect) -> ScreenRect {
    // Linux-Fallback: Arbeitsfläche == Monitor (keine Taskleiste im X11-Sinn).
    monitor
}

/// Monitor-Grundlage (ohne Taskleiste).
fn fallback_monitor() -> ScreenRect {
    if let Ok(s) = std::env::var("QD_MONITOR") {
        // Format "WxH" oder "WxH+x+y".
        if let Some((wh, _rest)) = s.split_once('+') {
            if let Some((w, h)) = wh.split_once('x') {
                if let (Ok(w), Ok(h)) = (w.parse::<i32>(), h.parse::<i32>()) {
                    return ScreenRect { x: 0, y: 0, width: w, height: h };
                }
            }
        } else if let Some((w, h)) = s.split_once('x') {
            if let (Ok(w), Ok(h)) = (w.parse::<i32>(), h.parse::<i32>()) {
                return ScreenRect { x: 0, y: 0, width: w, height: h };
            }
        }
    }
    ScreenRect { x: 0, y: 0, width: 1280, height: 800 }
}

#[cfg(windows)]
fn spi_work_area() -> Option<ScreenRect> {
    use std::os::raw::c_void;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, RECT, SPI_GETWORKAREA,
    };
    unsafe {
        let mut rect: RECT = std::mem::zeroed();
        if SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut _ as *mut c_void),
            0,
        ) != 0
        {
            Some(ScreenRect {
                x: rect.left,
                y: rect.top,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
            })
        } else {
            None
        }
    }
}

/// Arbeitsfläche des primären Monitors zur Laufzeit (Taskleiste abgezogen).
///
/// Unter Windows via `SPI_GETWORKAREA`, sonst der Monitor ohne Taskleiste.
/// (Die eigentliche Andock-Rechnung ist plattformfrei und separat getestet.)
#[cfg(windows)]
pub fn primary_work_area() -> ScreenRect {
    spi_work_area().unwrap_or_else(|| primary_screen().1)
}

#[cfg(not(windows))]
pub fn primary_work_area() -> ScreenRect {
    primary_screen().1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taskbar_hoehe_aus_differenz() {
        let monitor = ScreenRect { x: 0, y: 0, width: 1920, height: 1080 };
        let work = ScreenRect { x: 0, y: 0, width: 1920, height: 1040 };
        assert_eq!(taskbar_height(work, monitor), 40);
        // Keine Taskleiste -> 0.
        assert_eq!(taskbar_height(monitor, monitor), 0);
    }

    #[test]
    fn dock_rechts_und_oberkante_taskleiste() {
        let monitor = ScreenRect { x: 0, y: 0, width: 1920, height: 1080 };
        let work = ScreenRect { x: 0, y: 0, width: 1920, height: 1040 }; // Taskleiste 40 px unten
        let r = dock_rect(300, 600, monitor, work);
        assert_eq!(r.x, 1920 - 300, "bündig am rechten Rand");
        // Fensterunterkante = Taskleisten-Oberkante = 1080 - 40 = 1040 -> y = 1040 - 600 = 440.
        assert_eq!(r.y, 440);
        assert_eq!((r.width, r.height), (300, 600));
    }

    #[test]
    fn near_right_edge_schwellwert() {
        let _monitor = ScreenRect { x: 0, y: 0, width: 1920, height: 1080 };
        // Rechte Kante bei 1910 -> 10 px Abstand -> innerhalb Schwelle 16.
        assert!(is_near_right_edge(1910 - 300, 300, 1920, DOCK_THRESHOLD));
        // Rechte Kante bei 1800 -> 120 px Abstand -> außerhalb.
        assert!(!is_near_right_edge(1800 - 300, 300, 1920, DOCK_THRESHOLD));
    }

    #[test]
    fn maybe_dock_nur_wenn_nah() {
        let monitor = ScreenRect { x: 0, y: 0, width: 1920, height: 1080 };
        let work = ScreenRect { x: 0, y: 0, width: 1920, height: 1040 };
        let near = ScreenRect { x: 1620, y: 100, width: 300, height: 600 };
        let docked = maybe_dock(near, monitor, work, DOCK_THRESHOLD);
        assert_eq!(docked.x, 1620);
        let far = ScreenRect { x: 200, y: 100, width: 300, height: 600 };
        assert_eq!(maybe_dock(far, monitor, work, DOCK_THRESHOLD), far);
    }

    #[test]
    fn primary_screen_linux_fallback() {
        // Ohne QD_MONITOR: 1280x800.
        unsafe { std::env::remove_var("QD_MONITOR") };
        let (m, w) = primary_screen();
        assert_eq!((m.width, m.height), (1280, 800));
        assert_eq!(w, m, "Linux: Arbeitsfläche == Monitor");
    }

    #[test]
    fn primary_screen_aus_umgebung() {
        unsafe { std::env::set_var("QD_MONITOR", "2560x1440") };
        let (m, _w) = primary_screen();
        assert_eq!((m.width, m.height), (2560, 1440));
        unsafe { std::env::remove_var("QD_MONITOR") };
    }
}
