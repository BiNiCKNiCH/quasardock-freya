//! QuasarDock — nativer App-Launcher auf Basis von Rust + Freya (Skia).
//!
//! Phase 0: Grundgerüst, Design-Freeze und Machbarkeitsnachweis.

mod design;
mod ui;

use freya::prelude::*;

/// Eingefrorenes Programmicon (unverändert aus der Referenz übernommen).
const APP_ICON: &[u8] = include_bytes!("../app-icon.png");

fn main() {
    launch(
        LaunchConfig::new().with_window(
            WindowConfig::new(ui::app)
                .with_title("QuasarDock")
                .with_size(design::WINDOW_WIDTH, design::WINDOW_HEIGHT)
                .with_min_size(design::WINDOW_MIN_WIDTH, design::WINDOW_MIN_HEIGHT)
                .with_background(design::CANVAS)
                .with_icon(LaunchConfig::window_icon(APP_ICON)),
        ),
    );
}
