//! QuasarDock — Rust-Kern (Datenmodell, portabler Store, Pfade).
//!
//! Phase 1: Datenmodell + portabler atomarer Store + Migrationen. Die
//! UI-Module (`design`, `ui`) bleiben erhalten, damit der Binärstart aus
//! Phase 0 weiterhin funktioniert; sie sind hier nur eingebunden.

pub mod autostart;
pub mod design;
pub mod geometry;
pub mod icons;
pub mod launcher;
pub mod model;
pub mod paths;
pub mod store;
pub mod tray;
pub mod windowing;
