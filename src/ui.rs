//! Minimales Grundgerüst der Oberfläche für Phase 0.
//!
//! Zeigt die Struktur aus SPEC §3 an (Hauptbereich links ~70 %, Sidebar rechts
//! ~30 %) und rendert ausschließlich mit den eingefrorenen Design-Tokens aus
//! [`crate::design`]. Es gibt hier bewusst noch keine Funktion, keinen Zustand
//! und keine Datenhaltung — nur den visuellen Beleg des Design-Freeze.
//!
//! Layout-Regel in dieser Datei: Kinder bekommen feste Breiten oder
//! [`Size::percent`], die Verteilung macht `Alignment::SpaceBetween`.
//! [`Size::fill`] wird für Breiten bewusst NICHT als Lückenfüller benutzt — es
//! belegt den ganzen Restplatz und schiebt spätere Geschwister aus dem Rahmen.

use freya::prelude::*;

use quasardock::design;

/// Wurzelkomponente des Fensters.
pub fn app() -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .background(design::CANVAS)
        .direction(Direction::Horizontal)
        .child(main_area())
        .child(sidebar())
}

// ---------------------------------------------------------------------------
// Hauptbereich (links, ~70 %)
// ---------------------------------------------------------------------------

fn main_area() -> impl IntoElement {
    rect()
        .width(Size::percent(69.0))
        .height(Size::fill())
        .direction(Direction::Vertical)
        .spacing(16.0)
        .padding(Gaps::new_all(20.0))
        .child(toolbar())
        .child(tile_grid())
}

fn toolbar() -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::px(56.0))
        .corner_radius(design::RADIUS_SMALL)
        .background(design::CONTROLS)
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .main_align(Alignment::SpaceBetween)
        .padding(Gaps::new(0.0, 16.0, 0.0, 16.0))
        .child(
            rect()
                .direction(Direction::Vertical)
                .spacing(2.0)
                .child(
                    label()
                        .font_size(design::FONT_BODY)
                        .color(design::TEXT_PRIMARY)
                        .text("Favoriten"),
                )
                .child(
                    label()
                        .font_size(design::FONT_META)
                        .color(design::TEXT_MUTED)
                        .text("8 Apps"),
                ),
        )
        .child(
            rect()
                .direction(Direction::Horizontal)
                .cross_align(Alignment::Center)
                .spacing(12.0)
                .child(search_field())
                .child(view_toggle()),
        )
}

fn search_field() -> impl IntoElement {
    rect()
        .width(Size::px(220.0))
        .height(Size::px(34.0))
        .corner_radius(design::RADIUS_SMALL)
        .background(design::CANVAS)
        .border(Border::new().fill(design::BORDER).width(1.0))
        .cross_align(Alignment::Center)
        .padding(Gaps::new(0.0, 12.0, 0.0, 12.0))
        .child(
            label()
                .font_size(design::FONT_BODY)
                .color(design::TEXT_MUTED)
                .text("Apps durchsuchen …"),
        )
}

fn view_toggle() -> impl IntoElement {
    rect()
        .direction(Direction::Horizontal)
        .spacing(6.0)
        .child(toggle_button("Kacheln", true))
        .child(toggle_button("Liste", false))
}

fn toggle_button(text: &str, active: bool) -> impl IntoElement {
    rect()
        .width(Size::px(84.0))
        .height(Size::px(34.0))
        .corner_radius(design::RADIUS_SMALL)
        .background(if active {
            design::ACCENT
        } else {
            design::CANVAS
        })
        .cross_align(Alignment::Center)
        .main_align(Alignment::Center)
        .child(
            label()
                .font_size(design::FONT_BODY)
                .color(if active {
                    design::CANVAS
                } else {
                    design::TEXT_SECONDARY
                })
                .text(text.to_string()),
        )
}

fn tile_grid() -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .direction(Direction::Vertical)
        .spacing(16.0)
        .child(tile_row(&["Browser", "Editor", "Terminal", "Dateien"]))
        .child(tile_row(&["Musik", "Notizen", "Rechner", "Werkzeuge"]))
}

fn tile_row(names: &[&str; 4]) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::px(112.0))
        .direction(Direction::Horizontal)
        .spacing(16.0)
        .child(tile(names[0]))
        .child(tile(names[1]))
        .child(tile(names[2]))
        .child(tile(names[3]))
}

fn tile(name: &str) -> impl IntoElement {
    rect()
        .width(Size::percent(23.0))
        .height(Size::fill())
        .corner_radius(design::RADIUS_TILE)
        .background(design::CONTROLS)
        .border(Border::new().fill(design::BORDER).width(1.0))
        .direction(Direction::Vertical)
        .cross_align(Alignment::Center)
        .main_align(Alignment::Center)
        .spacing(10.0)
        .child(
            rect()
                .width(Size::px(44.0))
                .height(Size::px(44.0))
                .corner_radius(design::RADIUS_SMALL)
                .background(design::CANVAS)
                .border(Border::new().fill(design::BORDER).width(1.0))
                .cross_align(Alignment::Center)
                .main_align(Alignment::Center)
                .child(
                    label()
                        .font_size(design::FONT_TITLE)
                        .color(design::ACCENT)
                        .text(name.chars().next().unwrap_or('?').to_string()),
                ),
        )
        .child(
            label()
                .font_size(design::FONT_BODY)
                .color(design::TEXT_SECONDARY)
                .text(name.to_string()),
        )
}

// ---------------------------------------------------------------------------
// Sidebar (rechts, ~30 %)
// ---------------------------------------------------------------------------

fn sidebar() -> impl IntoElement {
    rect()
        .width(Size::percent(31.0))
        .height(Size::fill())
        .background(design::CONTROLS)
        .direction(Direction::Vertical)
        .main_align(Alignment::SpaceBetween)
        .padding(Gaps::new_all(24.0))
        .child(
            rect()
                .width(Size::fill())
                .direction(Direction::Vertical)
                .spacing(18.0)
                .child(
                    rect()
                        .direction(Direction::Vertical)
                        .spacing(10.0)
                        .child(
                            label()
                                .font_size(design::FONT_TITLE_LARGE)
                                .color(design::TEXT_PRIMARY)
                                .text("QuasarDock"),
                        )
                        .child(
                            rect()
                                .width(Size::px(56.0))
                                .height(Size::px(3.0))
                                .corner_radius(2.0)
                                .background(design::ACCENT),
                        ),
                )
                .child(
                    rect()
                        .width(Size::fill())
                        .direction(Direction::Vertical)
                        .spacing(4.0)
                        .child(folder_row("Favoriten", "8", true))
                        .child(folder_row("Entwicklung", "5", false))
                        .child(folder_row("Spiele", "3", false)),
                ),
        )
        .child(
            rect()
                .width(Size::fill())
                .direction(Direction::Horizontal)
                .main_align(Alignment::End)
                .child(
                    rect()
                        .width(Size::px(42.0))
                        .height(Size::px(42.0))
                        .corner_radius(21.0)
                        .background(design::HOVER)
                        .border(Border::new().fill(design::BORDER_STRONG).width(1.0))
                        .cross_align(Alignment::Center)
                        .main_align(Alignment::Center)
                        .child(
                            label()
                                .font_size(design::FONT_TITLE)
                                .color(design::TEXT_SECONDARY)
                                .text("⚙"),
                        ),
                ),
        )
}

fn folder_row(name: &str, count: &str, active: bool) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::px(40.0))
        .corner_radius(design::RADIUS_SMALL)
        .background(if active {
            design::HOVER
        } else {
            design::CONTROLS
        })
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .main_align(Alignment::SpaceBetween)
        .padding(Gaps::new(0.0, 12.0, 0.0, 12.0))
        .child(
            rect()
                .direction(Direction::Horizontal)
                .cross_align(Alignment::Center)
                .spacing(10.0)
                .child(
                    rect()
                        .width(Size::px(3.0))
                        .height(Size::px(18.0))
                        .corner_radius(2.0)
                        .background(if active {
                            design::ACCENT
                        } else {
                            design::CONTROLS
                        }),
                )
                .child(
                    label()
                        .font_size(design::FONT_BODY)
                        .color(if active {
                            design::TEXT_PRIMARY
                        } else {
                            design::TEXT_SECONDARY
                        })
                        .text(name.to_string()),
                ),
        )
        .child(
            label()
                .font_size(design::FONT_META)
                .color(design::TEXT_MUTED)
                .text(count.to_string()),
        )
}
