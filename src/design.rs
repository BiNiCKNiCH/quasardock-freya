//! Design-Freeze (SPEC §2) — die eine Quelle der Wahrheit für alle Farben,
//! Radien und Schriftgrößen der Oberfläche.
//!
//! Diese Werte sind eingefroren. Änderungen hier sind Design-Änderungen und
//! brauchen eine ausdrückliche Freigabe; sie werden nicht "nebenbei" angepasst.

use freya::prelude::Color;

// ---------------------------------------------------------------------------
// Flächen
// ---------------------------------------------------------------------------

/// Hauptfläche des Fensters (`#10131A`).
pub const CANVAS: Color = Color::from_rgb(0x10, 0x13, 0x1A);
/// Erhöhte Flächen (`#1A1E29`).
pub const CONTROLS: Color = Color::from_rgb(0x1A, 0x1E, 0x29);
/// Hover (`#1F2433`).
pub const HOVER: Color = Color::from_rgb(0x1F, 0x24, 0x33);

// ---------------------------------------------------------------------------
// Akzent / Zustände
// ---------------------------------------------------------------------------

/// Akzent (Cyan `#00B4D8`).
pub const ACCENT: Color = Color::from_rgb(0x00, 0xB4, 0xD8);
/// Helle Akzentvariante (`#00E5FF`). Teil des Freeze; erste Verwendung ab Phase 2.
#[allow(dead_code)]
pub const ACCENT_BRIGHT: Color = Color::from_rgb(0x00, 0xE5, 0xFF);
/// Positiv (`#22C55E`). Teil des Freeze; erste Verwendung ab Phase 3.
#[allow(dead_code)]
pub const POSITIVE: Color = Color::from_rgb(0x22, 0xC5, 0x5E);

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

/// Text primär.
pub const TEXT_PRIMARY: Color = Color::from_rgb(0xFF, 0xFF, 0xFF);
/// Text sekundär (Meta-Zeilen, Beschriftungen).
pub const TEXT_SECONDARY: Color = Color::from_rgb(0x94, 0xA3, 0xB8);
/// Text gedimmt (Hinweise, Zähler).
pub const TEXT_MUTED: Color = Color::from_rgb(0x64, 0x74, 0x8B);

// ---------------------------------------------------------------------------
// Borders
// ---------------------------------------------------------------------------

/// Rahmen als ARGB (Weiß, Alpha 13 ≈ 5 %).
pub const BORDER: Color = Color::from_argb(0x0D, 0xFF, 0xFF, 0xFF);
/// Rahmen als ARGB (Weiß, Alpha 20 ≈ 8 %).
pub const BORDER_STRONG: Color = Color::from_argb(0x14, 0xFF, 0xFF, 0xFF);

// ---------------------------------------------------------------------------
// Geometrie
// ---------------------------------------------------------------------------

/// Radius für Kacheln und Karten.
pub const RADIUS_TILE: f32 = 14.0;
/// Radius für kleine Elemente (Knöpfe, Felder).
pub const RADIUS_SMALL: f32 = 9.0;

/// Fensterbreite in logischen Pixeln.
pub const WINDOW_WIDTH: f64 = 1280.0;
/// Fensterhöhe in logischen Pixeln.
pub const WINDOW_HEIGHT: f64 = 800.0;
/// Minimale Fensterbreite.
pub const WINDOW_MIN_WIDTH: f64 = 900.0;
/// Minimale Fensterhöhe.
pub const WINDOW_MIN_HEIGHT: f64 = 600.0;

// ---------------------------------------------------------------------------
// Schrift
// ---------------------------------------------------------------------------

/// Meta-Text (Zähler, Hinweise).
pub const FONT_META: f32 = 11.0;
/// Fließtext / Listenzeilen.
pub const FONT_BODY: f32 = 13.0;
/// Abschnittstitel.
pub const FONT_TITLE: f32 = 20.0;
/// Großer Titel (Sidebar-Kopf).
pub const FONT_TITLE_LARGE: f32 = 24.0;

// ---------------------------------------------------------------------------
// HEX-Schreibweise der Tokens (Doku- und Testreferenz, identisch zu SPEC §2)
// ---------------------------------------------------------------------------

/// Schreibweise der Tokens für Doku, Belege und Konsistenztests.
///
/// Diese Strings werden zur Laufzeit nicht gebraucht — sie sind die
/// nachlesbare Form derselben Werte, die oben als [`Color`]-Konstanten stehen.
#[allow(dead_code)]
pub mod hex {
    pub const CANVAS: &str = "#10131A";
    pub const CONTROLS: &str = "#1A1E29";
    pub const HOVER: &str = "#1F2433";
    pub const ACCENT: &str = "#00B4D8";
    pub const ACCENT_BRIGHT: &str = "#00E5FF";
    pub const POSITIVE: &str = "#22C55E";
    pub const TEXT_PRIMARY: &str = "#FFFFFF";
    pub const TEXT_SECONDARY: &str = "#94A3B8";
    pub const TEXT_MUTED: &str = "#64748B";
    pub const BORDER: &str = "rgba(255,255,255,0.05)";
    pub const BORDER_STRONG: &str = "rgba(255,255,255,0.08)";
}

// ---------------------------------------------------------------------------
// Konsistenz der Tokens (HEX-Schreibweise ⇄ Color-Wert)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn to_hex(color: Color) -> String {
        format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b())
    }

    fn to_rgba(color: Color) -> String {
        format!(
            "rgba({},{},{},{:.2})",
            color.r(),
            color.g(),
            color.b(),
            color.a() as f32 / 255.0
        )
    }

    #[test]
    fn flaechen_tokens_stimmen() {
        assert_eq!(to_hex(CANVAS), hex::CANVAS);
        assert_eq!(to_hex(CONTROLS), hex::CONTROLS);
        assert_eq!(to_hex(HOVER), hex::HOVER);
        assert_eq!(to_hex(CANVAS), "#10131A");
        assert_eq!(to_hex(CONTROLS), "#1A1E29");
    }

    #[test]
    fn akzent_tokens_stimmen() {
        assert_eq!(to_hex(ACCENT), hex::ACCENT);
        assert_eq!(to_hex(ACCENT_BRIGHT), hex::ACCENT_BRIGHT);
        assert_eq!(to_hex(POSITIVE), hex::POSITIVE);
        assert_eq!(to_hex(ACCENT), "#00B4D8");
    }

    #[test]
    fn text_tokens_stimmen() {
        assert_eq!(to_hex(TEXT_PRIMARY), hex::TEXT_PRIMARY);
        assert_eq!(to_hex(TEXT_SECONDARY), hex::TEXT_SECONDARY);
        assert_eq!(to_hex(TEXT_MUTED), hex::TEXT_MUTED);
    }

    #[test]
    fn border_tokens_stimmen() {
        assert_eq!(to_rgba(BORDER), hex::BORDER);
        assert_eq!(to_rgba(BORDER_STRONG), hex::BORDER_STRONG);
    }

    #[test]
    fn fenster_masse_stimmen() {
        assert_eq!((WINDOW_WIDTH, WINDOW_HEIGHT), (1280.0, 800.0));
        assert_eq!((WINDOW_MIN_WIDTH, WINDOW_MIN_HEIGHT), (900.0, 600.0));
        assert!(WINDOW_MIN_WIDTH < WINDOW_WIDTH && WINDOW_MIN_HEIGHT < WINDOW_HEIGHT);
    }
}
