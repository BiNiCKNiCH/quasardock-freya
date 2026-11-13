//! QuasarDock — Oberfläche (Phase 2).
//!
//! Baut auf dem Datenmodell/Store aus Phase 1 auf (`quasardock::{model,store}`).
//! Alle visuellen Werte kommen aus [`quasardock::design`] (eingefrorene Tokens, SPEC §2).
//! Drag & Drop läuft über native Pointer-Events (kein HTML): eigenes Verschieben im
//! Grid/Liste, auf Sidebar-Ordner ziehen = Move, Cyan-Highlight am Ziel, Ghost,
//! sofort persistiert.

use freya::prelude::*;
use quasardock::design;
use quasardock::model::*;
use quasardock::store::Store;
use quasardock::{autostart, icons, launcher, windowing};

use serde_json::Value;
use std::env;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// Layout-Konstanten (px, SPEC §3)
// ---------------------------------------------------------------------------

const TITLEBAR_H: f32 = 38.0;
const TOOLBAR_H: f32 = 56.0;
const TILE: f32 = 104.0;
const TILE_ICON: f32 = 56.0;
const ROW_H: f32 = 40.0;
const GRID_COLS: usize = 6;

// ---------------------------------------------------------------------------
// Zentraler Kontext: alle reaktiven Zustände + der Store. Wird als Wert kopiert
// an die (inline ausgeführten) Render-Helfer weitergereicht. Alle Felder sind
// billig zu klonen (Rc/State), daher Owned-by-Value statt Referenz — so landet
// kein Zeiger auf einen lokalen Stack in den zurückgegebenen Element-Baum.
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppCtx {
    store: Rc<Store>,
    config: State<Config>,
    current_folder: State<String>,
    search: State<String>,
    search_all: State<bool>,
    settings_open: State<bool>,
    settings_tab: State<usize>,
    dropdown_open: State<bool>,
    drag: State<DragState>,
    focused_id: State<usize>,
    // App-Dialog
    app_edit: State<Option<AppEntry>>,
    app_dlg_folder: State<String>,
    app_dlg_name: State<String>,
    app_dlg_path: State<String>,
    app_dlg_args: State<String>,
    app_dlg_cwd: State<String>,
    app_dlg_icon: State<String>,
    app_dlg_start: State<usize>,
    app_dlg_auto: State<bool>,
    // Ordner-Verwaltung
    folder_renaming: State<Option<String>>,
    folder_rename_name: State<String>,
    folder_add_name: State<String>,
    accent_hex: State<String>,
    // Fenster-Zieh-Status (für Live-Andocken während des Ziehens, SPEC §6).
    window_dragging: State<bool>,
}

/// Laufzeit-Zustand eines Drag-Vorgangs (eigene Pointer-Events).
#[derive(Clone, PartialEq)]
enum DragState {
    Idle,
    Dragging {
        app_id: String,
        grab: CursorPoint,
        pos: CursorPoint,
        target_folder: Option<String>,
        target_index: Option<usize>,
    },
}

/// Snapshot eines `State<T>` (T ist Clone + 'static).
fn snap<T: Clone + 'static>(s: &State<T>) -> T {
    s.read().clone()
}

// ---------------------------------------------------------------------------
// Erweiterungs-Traits: State ist `Copy`, daher kann ein Wert in eine lokale
// `mut`-Kopie gehoben und ueber `write()` (benachrichtigt Subscriber) geschrieben
// werden. So funktionieren `.set_val(...)`/`.with_mut_val(...)` aus jedem
// Kontext (&self, owned, Closure) ohne explizite `mut`-Bindung.
// ---------------------------------------------------------------------------

/// Wert ersetzen, aus `&self`-Kontext.
pub trait SetVal<T: Clone + 'static> {
    fn set_val(&self, v: T);
}

impl<T: Clone + 'static> SetVal<T> for State<T> {
    fn set_val(&self, v: T) {
        let mut c = *self;
        *c.write() = v;
    }
}

/// Wert ueber Closure mutieren, aus `&self`-Kontext.
pub trait WithMutVal<T: Clone + 'static> {
    fn with_mut_val(&self, f: impl FnOnce(&mut T));
}

impl<T: Clone + 'static> WithMutVal<T> for State<T> {
    fn with_mut_val(&self, f: impl FnOnce(&mut T)) {
        let mut c = *self;
        f(&mut *c.write());
    }
}

// ---------------------------------------------------------------------------
// Store-Helfer
// ---------------------------------------------------------------------------

/// Wendet eine Mutation auf den Store an und spiegelt sie in den UI-Snapshot.
fn commit(ctx: &AppCtx, f: impl FnOnce(&mut Config)) {
    ctx.store.update(f);
    ctx.config.set_val(ctx.store.config());
}

/// Schließen (X): Geometrie + offene Änderungen speichern, Fenster verbergen.
/// Die App läuft im Tray weiter; Beenden erfolgt nur über das Tray-Menü
/// (bzw. `platform.exit()` aus dem Tray-Befehl).
fn close_to_tray(ctx: &AppCtx) {
    if ctx.config.read().settings.remember_geometry
        && let Some(g) = windowing::read_geometry(&Platform::get(), Platform::window_id())
    {
        commit(ctx, |c| c.settings.geometry = Some(g));
    }
    let _ = ctx.store.save_now();
    windowing::set_visible(&Platform::get(), Platform::window_id(), false);
}

/// Eindeutige ID (pro Sitzung stabil, hinreichend für Demo-Daten).
fn gen_id(prefix: &str) -> String {
    static C: AtomicU64 = AtomicU64::new(0);
    let n = C.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{}-{}", prefix, nanos, n)
}

/// Erst-Befüllung mit Beispieldaten, damit die Oberfläche Design-Parität zeigt.
fn seed_demo(c: &mut Config) {
    for (name, fid) in [
        ("Office", "office"),
        ("Utilities", "utilities"),
        ("Media", "media"),
        ("Allgemein", "allgemein"),
    ] {
        c.folders.push(Folder {
            id: fid.to_string(),
            name: name.to_string(),
            protected: false,
        });
    }
    let apps = [
        ("Browser", "/usr/bin/firefox", "allgemein"),
        ("Notizen", "/usr/bin/gedit", "allgemein"),
        ("Musik", "/usr/bin/rhythmbox", "media"),
        ("Video", "/usr/bin/totem", "media"),
        ("Rechner", "/usr/bin/gnome-calculator", "utilities"),
        ("Dateimanager", "/usr/bin/nautilus", "utilities"),
        ("Terminal", "/usr/bin/gnome-terminal", "utilities"),
        ("Editor", "/usr/bin/code", "office"),
        ("PDF-Reader", "/usr/bin/evince", "office"),
        ("LibreOffice", "/usr/bin/libreoffice", "office"),
        ("Mail", "/usr/bin/thunderbird", FAVORITES_ID),
        ("Kalender", "/usr/bin/gnome-calendar", FAVORITES_ID),
    ];
    let mut counters: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for (name, path, fid) in apps {
        // Stabile Demo-IDs (Name-basiert), damit QD_INIT/DnD-Nachweis einen
        // bekannten app_id referenzieren kann. Neue Apps zur Laufzeit behalten
        // das Zufalls-gen_id (keine Kollision).
        let sort_order = {
            let n = counters.entry(fid.to_string()).or_insert(0);
            *n += 1;
            *n
        };
        let a = AppEntry {
            id: format!("app_{}", name.to_lowercase().replace(' ', "_")),
            name: name.to_string(),
            path: path.to_string(),
            folder_id: fid.to_string(),
            sort_order,
            ..AppEntry::default()
        };
        c.apps.push(a);
    }
}

fn count_in(cfg: &Config, folder_id: &str) -> usize {
    cfg.apps.iter().filter(|a| a.folder_id == folder_id).count()
}

/// Apps des (oder aller) Ordners, gefiltert und sortiert.
fn compute_apps(cfg: &Config, cf: &str, query: &str, all: bool) -> Vec<AppEntry> {
    let q = query.trim().to_lowercase();
    let mut v: Vec<AppEntry> = cfg
        .apps
        .iter()
        .filter(|a| {
            let in_scope = if all && !q.is_empty() {
                true
            } else {
                a.folder_id == cf
            };
            in_scope && (q.is_empty() || a.name.to_lowercase().contains(&q))
        })
        .cloned()
        .collect();
    match cfg.settings.sort_mode {
        SortMode::Alphabetical => v.sort_by_key(|a| a.name.to_lowercase()),
        SortMode::Manual => v.sort_by_key(|a| a.sort_order),
    }
    v
}

fn move_app_to_folder(c: &mut Config, app_id: &str, fid: &str) {
    let max = c
        .apps
        .iter()
        .filter(|x| x.folder_id == fid)
        .map(|x| x.sort_order)
        .max()
        .unwrap_or(0);
    if let Some(a) = c.apps.iter_mut().find(|a| a.id == app_id) {
        a.folder_id = fid.to_string();
        a.sort_order = max + 1;
    }
}

fn reorder_in_folder(c: &mut Config, cf: &str, app_id: &str, idx: usize) {
    let mut list: Vec<AppEntry> = c
        .apps
        .iter()
        .filter(|a| a.folder_id == cf)
        .cloned()
        .collect();
    list.sort_by_key(|a| a.sort_order);
    let pos = list.iter().position(|a| a.id == app_id);
    if let Some(p) = pos {
        let moved = list.remove(p);
        let mut target = idx.min(list.len());
        if p < idx {
            target = target.saturating_sub(1);
        }
        list.insert(target, moved);
    }
    for (i, a) in list.iter().enumerate() {
        if let Some(slot) = c.apps.iter_mut().find(|x| x.id == a.id) {
            slot.sort_order = (i + 1) as i64;
        }
    }
}

fn upsert_app(c: &mut Config, app: AppEntry) {
    if let Some(slot) = c.apps.iter_mut().find(|a| a.id == app.id) {
        *slot = app;
    } else {
        let max = c
            .apps
            .iter()
            .filter(|x| x.folder_id == app.folder_id)
            .map(|x| x.sort_order)
            .max()
            .unwrap_or(0);
        let mut a = app;
        a.sort_order = max + 1;
        c.apps.push(a);
    }
}

fn add_folder(c: &mut Config, name: &str) -> String {
    let id = gen_id("folder");
    c.folders.push(Folder {
        id: id.clone(),
        name: name.to_string(),
        protected: false,
    });
    id
}

fn open_app_dialog(ctx: &AppCtx, app: Option<AppEntry>) {
    match app {
        Some(a) => {
            ctx.app_edit.set_val(Some(a.clone()));
            ctx.app_dlg_folder.set_val(a.folder_id.clone());
            ctx.app_dlg_name.set_val(a.name.clone());
            ctx.app_dlg_path.set_val(a.path.clone());
            ctx.app_dlg_args.set_val(a.args.clone());
            ctx.app_dlg_cwd.set_val(a.cwd.clone());
            ctx.app_dlg_icon.set_val(a.icon.clone().unwrap_or_default());
            ctx.app_dlg_start.set_val(match a.start_state {
                StartState::Normal => 0,
                StartState::Minimized => 1,
                StartState::Maximized => 2,
            });
            ctx.app_dlg_auto.set_val(a.auto_start);
        }
        None => {
            let cf = snap(&ctx.current_folder);
            ctx.app_edit.set_val(Some(AppEntry {
                id: gen_id("app"),
                folder_id: cf.clone(),
                ..Default::default()
            }));
            ctx.app_dlg_folder.set_val(cf);
            ctx.app_dlg_name.set_val(String::new());
            ctx.app_dlg_path.set_val(String::new());
            ctx.app_dlg_args.set_val(String::new());
            ctx.app_dlg_cwd.set_val(String::new());
            ctx.app_dlg_icon.set_val(String::new());
            ctx.app_dlg_start.set_val(0);
            ctx.app_dlg_auto.set_val(false);
        }
    }
    ctx.focused_id.set_val(0);
}

fn close_app_dialog(ctx: &AppCtx) {
    ctx.app_edit.set_val(None);
}

fn save_app_dialog(ctx: &AppCtx) {
    let id = snap(&ctx.app_edit)
        .as_ref()
        .map(|a| a.id.clone())
        .unwrap_or_else(|| gen_id("app"));
    let folder_id = snap(&ctx.app_dlg_folder);
    let name = snap(&ctx.app_dlg_name);
    let path = snap(&ctx.app_dlg_path);
    let args = snap(&ctx.app_dlg_args);
    let cwd = snap(&ctx.app_dlg_cwd);
    let icon = snap(&ctx.app_dlg_icon);
    let start = *ctx.app_dlg_start.read();
    let auto = *ctx.app_dlg_auto.read();
    let start_state = match start {
        1 => StartState::Minimized,
        2 => StartState::Maximized,
        _ => StartState::Normal,
    };
    let app = AppEntry {
        id,
        name,
        path,
        args,
        cwd,
        icon: if icon.is_empty() { None } else { Some(icon) },
        folder_id,
        sort_order: 0,
        start_state,
        auto_start: auto,
        auto_start_delay_ms: 0,
    };
    commit(ctx, |c| upsert_app(c, app));
    close_app_dialog(ctx);
}

/// Farbe des Buchstaben-Fallbacks (deterministisch aus dem Namen, SPEC §7).
fn letter_color(s: &str) -> Color {
    let palette: [Color; 6] = [
        design::accent(),
        design::ACCENT_BRIGHT,
        design::POSITIVE,
        Color::from_rgb(0x81, 0x8C, 0xF8),
        Color::from_rgb(0xF4, 0x72, 0xB6),
        Color::from_rgb(0x93, 0xC5, 0xFD),
    ];
    let h: usize = s.bytes().map(|b| b as usize).sum();
    palette[h % palette.len()]
}

// ---------------------------------------------------------------------------
// Wurzel
// ---------------------------------------------------------------------------

pub fn app() -> impl IntoElement {
    let store = use_hook(|| {
        let s = Rc::new(Store::open(None).expect("Store konnte nicht geöffnet werden"));
        let c = s.config();
        if c.apps.is_empty() && c.folders.len() <= 1 {
            s.update(seed_demo);
            let _ = s.save_now();
        }
        s
    });
    let config = use_state(|| store.config());
    let current_folder = use_state(|| FAVORITES_ID.to_string());
    let search = use_state(String::new);
    let search_all = use_state(|| false);
    let settings_open = use_state(|| false);
    let settings_tab = use_state(|| 0usize);
    let dropdown_open = use_state(|| false);
    let drag = use_state(|| DragState::Idle);
    let focused_id = use_state(|| 0usize);
    let app_edit = use_state(|| None::<AppEntry>);
    let app_dlg_folder = use_state(String::new);
    let app_dlg_name = use_state(String::new);
    let app_dlg_path = use_state(String::new);
    let app_dlg_args = use_state(String::new);
    let app_dlg_cwd = use_state(String::new);
    let app_dlg_icon = use_state(String::new);
    let app_dlg_start = use_state(|| 0usize);
    let app_dlg_auto = use_state(|| false);
    let folder_renaming = use_state(|| None::<String>);
    let folder_rename_name = use_state(String::new);
    let folder_add_name = use_state(String::new);
    let accent_hex = use_state(String::new);
    let window_dragging = use_state(|| false);

    let ctx = AppCtx {
        store,
        config,
        current_folder,
        search,
        search_all,
        settings_open,
        settings_tab,
        dropdown_open,
        drag,
        focused_id,
        app_edit,
        app_dlg_folder,
        app_dlg_name,
        app_dlg_path,
        app_dlg_args,
        app_dlg_cwd,
        app_dlg_icon,
        app_dlg_start,
        app_dlg_auto,
        folder_renaming,
        folder_rename_name,
        folder_add_name,
        accent_hex,
        window_dragging,
    };

    // Screenshot/Demo-Initialzustand per Env-Variable QD_INIT (JSON).
    // Wird nur angewandt, wenn der Zielwert vom aktuellen abweicht, damit kein
    // Re-Render/Re-Speichern ausgelöst wird. Ohne die Variable kein Effekt.
    if let Ok(init) = env::var("QD_INIT") {
        let init = init.trim();
        match serde_json::from_str::<Value>(init) {
            Ok(v) => {
                if v.get("settings_open").and_then(|x| x.as_bool()) == Some(true)
                    && !*ctx.settings_open.read()
                {
                    ctx.settings_open.set_val(true);
                }
                if let Some(t) = v.get("settings_tab").and_then(|x| x.as_u64()) {
                    let t = t as usize;
                    if *ctx.settings_tab.read() != t {
                        ctx.settings_tab.set_val(t);
                    }
                }
                if let Some(cf) = v.get("current_folder").and_then(|x| x.as_str())
                    && *ctx.current_folder.read() != cf
                {
                    ctx.current_folder.set_val(cf.to_string());
                }
                if let Some(vm) = v.get("view_mode").and_then(|x| x.as_str()) {
                    let mode = if vm == "list" {
                        ViewMode::List
                    } else {
                        ViewMode::Grid
                    };
                    if ctx.store.config().settings.view_mode != mode {
                        ctx.store.update(|c| c.settings.view_mode = mode);
                        ctx.config.set_val(ctx.store.config());
                    }
                }
                if let Some(q) = v.get("search").and_then(|x| x.as_str())
                    && ctx.search.read().as_str() != q
                {
                    ctx.search.set_val(q.to_string());
                }
                if let Some(mv) = v.get("move_app").and_then(|x| x.as_object())
                    && let (Some(app_id), Some(fid)) = (
                        mv.get("app_id").and_then(|x| x.as_str()),
                        mv.get("to").and_then(|x| x.as_str()),
                    )
                {
                    let aid = app_id.to_string();
                    let fid = fid.to_string();
                    let already = ctx
                        .store
                        .config()
                        .apps
                        .iter()
                        .any(|a| a.id == aid && a.folder_id == fid);
                    if !already {
                        ctx.store.update(|c| move_app_to_folder(c, &aid, &fid));
                        ctx.config.set_val(ctx.store.config());
                    }
                }
                if let Some(a) = v.get("accent").and_then(|x| x.as_str())
                    && let Some(c) = design::parse_hex_color(a)
                {
                    design::set_accent(c);
                }
            }
            Err(e) => eprintln!("[QD_INIT] parse error: {}", e),
        }
    }

    let cfg = snap(&ctx.config);
    let cf = snap(&ctx.current_folder);
    let query = snap(&ctx.search);
    let apps = compute_apps(&cfg, &cf, &query, *ctx.search_all.read());

    let ctx_gpm = ctx.clone();
    let ctx_gpu = ctx.clone();

    let dropdown: Option<_> = if *ctx.dropdown_open.read() {
        Some(dropdown_menu(ctx.clone()))
    } else {
        None
    };
    let settings: Option<_> = if *ctx.settings_open.read() {
        Some(settings_overlay(ctx.clone(), cfg.clone()))
    } else {
        None
    };
    let appdlg: Option<_> = if snap(&ctx.app_edit).is_some() {
        Some(app_dialog(ctx.clone()))
    } else {
        None
    };

    rect()
        .width(Size::fill())
        .height(Size::fill())
        .background(design::CANVAS)
        .direction(Direction::Vertical)
        .on_global_pointer_move(move |e: Event<PointerEventData>| {
            if let DragState::Dragging { .. } = snap(&ctx_gpm.drag) {
                ctx_gpm.drag.with_mut_val(|d| {
                    if let DragState::Dragging { pos, .. } = d {
                        *pos = e.global_location();
                    }
                });
            }
            // Fenster live am Bildschirmrand andocken, während es gezogen wird (SPEC §6).
            if snap(&ctx_gpm.window_dragging) && snap(&ctx_gpm.config).settings.dock_to_edge {
                windowing::dock_if_near(&Platform::get(), Platform::window_id());
            }
        })
        .on_global_pointer_up(move |_: Event<PointerEventData>| {
            if let DragState::Dragging {
                app_id,
                target_folder,
                target_index,
                ..
            } = snap(&ctx_gpu.drag)
            {
                if let Some(fid) = target_folder.clone() {
                    commit(&ctx_gpu, |c| move_app_to_folder(c, &app_id, &fid));
                } else if let Some(idx) = target_index {
                    let cf2 = snap(&ctx_gpu.current_folder);
                    commit(&ctx_gpu, |c| reorder_in_folder(c, &cf2, &app_id, idx));
                } else {
                    // Reinem Klick (kein Drag-Ziel) -> App starten (SPEC §4.9).
                    if let Some(app) = ctx_gpu
                        .store
                        .config()
                        .apps
                        .iter()
                        .find(|a| a.id == app_id)
                        .cloned()
                    {
                        if let Err(e) = launcher::launch(&launcher::request_from_app(&app)) {
                            eprintln!("[launch] {}: {}", app.name, e);
                        }
                        if ctx_gpu.store.config().settings.auto_minimize_after_launch {
                            windowing::set_visible(&Platform::get(), Platform::window_id(), false);
                        }
                    }
                }
                ctx_gpu.drag.set_val(DragState::Idle);
            }
            // Fenster nach einem Drag am Bildschirmrand andocken (SPEC §6).
            if snap(&ctx_gpu.config).settings.dock_to_edge {
                windowing::dock_if_near(&Platform::get(), Platform::window_id());
            }
        })
        .on_global_key_down(move |e: Event<KeyboardEventData>| {
            // F9: Fenster zeigen/verbergen (Linux-Ersatz für das Tray-Menü).
            if let Key::Named(NamedKey::F9) = &e.data().key {
                let p = Platform::get();
                let id = Platform::window_id();
                p.with_window(id, |w| {
                    let vis = w.is_visible().unwrap_or(false);
                    w.set_visible(!vis);
                });
            }
        })
        .child(titlebar(ctx.clone()))
        .child(
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .direction(Direction::Horizontal)
                .child(main_area(ctx.clone(), apps, cf.clone()))
                .child(sidebar(ctx.clone(), cfg, cf.clone())),
        )
        .maybe_child(ghost_tile(ctx.clone()))
        .maybe_child(dropdown)
        .maybe_child(settings)
        .maybe_child(appdlg)
}

// ---------------------------------------------------------------------------
// Titelleiste (eigenes Band, nur Schließen-X, SPEC §3)
// ---------------------------------------------------------------------------

fn titlebar(_ctx: AppCtx) -> impl IntoElement {
    let ctx = _ctx.clone();
    rect()
        .width(Size::fill())
        .height(Size::px(TITLEBAR_H))
        .background(design::CANVAS)
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .main_align(Alignment::End)
        .padding(Gaps::new(0.0, 12.0, 0.0, 12.0))
        .on_pointer_down({
            let ctx = ctx.clone();
            move |_| {
                // Fenster-Zieh-Status merken und echtes OS-Drag starten (DPI-/Multi-Monitor-korrekt).
                ctx.window_dragging.set_val(true);
                let p = Platform::get();
                let id = Platform::window_id();
                p.with_window(id, |w| {
                    let _ = w.drag_window();
                });
            }
        })
        .child(
            rect()
                .width(Size::px(30.0))
                .height(Size::px(30.0))
                .corner_radius(8.0_f32)
                .background(design::CONTROLS)
                .border(Border::new().fill(design::BORDER).width(1.0))
                .cross_align(Alignment::Center)
                .main_align(Alignment::Center)
                .on_press(move |_| close_to_tray(&ctx.clone()))
                .child(
                    label()
                        .text("✕")
                        .font_size(16.0)
                        .color(design::TEXT_SECONDARY),
                ),
        )
}

// ---------------------------------------------------------------------------
// Hauptbereich
// ---------------------------------------------------------------------------

fn main_area(ctx: AppCtx, apps: Vec<AppEntry>, cf: String) -> impl IntoElement {
    let cfg = snap(&ctx.config);
    let folder_name = cfg
        .folders
        .iter()
        .find(|f| f.id == cf)
        .map(|f| f.name.clone())
        .unwrap_or_else(|| "Ordner".to_string());
    let count = apps.len();

    rect()
        .width(Size::percent(69.0))
        .height(Size::fill())
        .background(design::CANVAS)
        .direction(Direction::Vertical)
        .child(toolbar(ctx.clone(), folder_name, count))
        .child(content(ctx, apps, cf))
}

fn toolbar(ctx: AppCtx, folder_name: String, count: usize) -> impl IntoElement {
    let ctx = ctx.clone();
    rect()
        .width(Size::fill())
        .height(Size::px(TOOLBAR_H))
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .main_align(Alignment::SpaceBetween)
        .padding(Gaps::new(0.0, 20.0, 0.0, 20.0))
        .spacing(12.0)
        .child(
            rect()
                .direction(Direction::Horizontal)
                .cross_align(Alignment::Center)
                .spacing(12.0)
                .child(
                    label()
                        .text(folder_name)
                        .font_size(design::FONT_TITLE)
                        .color(design::TEXT_PRIMARY),
                )
                .child(count_badge(&format!("{} Apps", count))),
        )
        .child(
            rect()
                .direction(Direction::Horizontal)
                .cross_align(Alignment::Center)
                .spacing(10.0)
                .child(search_field(ctx.clone()))
                .child(view_dropdown_button(ctx.clone()))
                .child(search_scope_button(ctx)),
        )
}

fn count_badge(text: &str) -> impl IntoElement {
    rect()
        .height(Size::px(24.0))
        .corner_radius(design::RADIUS_SMALL)
        .background(design::HOVER)
        .padding(Gaps::new(0.0, 10.0, 0.0, 10.0))
        .cross_align(Alignment::Center)
        .main_align(Alignment::Center)
        .child(
            label()
                .text(text.to_string())
                .font_size(design::FONT_META)
                .color(design::TEXT_MUTED),
        )
}

/// Suchfeld (eigenes Textfeld, SPEC §4.4).
fn search_field(ctx: AppCtx) -> impl IntoElement {
    text_field(ctx.clone(), 1, ctx.search, "Apps durchsuchen …", 240.0)
}

fn search_scope_button(ctx: AppCtx) -> impl IntoElement {
    let ctx = ctx.clone();
    let all = *ctx.search_all.read();
    rect()
        .height(Size::px(34.0))
        .corner_radius(design::RADIUS_SMALL)
        .background(if all {
            design::accent()
        } else {
            design::CANVAS
        })
        .border(Border::new().fill(design::BORDER).width(1.0))
        .padding(Gaps::new(0.0, 10.0, 0.0, 10.0))
        .cross_align(Alignment::Center)
        .main_align(Alignment::Center)
        .on_press(move |_| ctx.search_all.set_val(!*ctx.search_all.read()))
        .child(
            label()
                .text(if all { "Alle Ordner" } else { "Dieser Ordner" })
                .font_size(design::FONT_META)
                .color(if all {
                    design::CANVAS
                } else {
                    design::TEXT_SECONDARY
                }),
        )
}

/// Auslöser für das Anzeigeoptionen-Menü (Kacheln/Liste · Manuell/Alphabetisch).
fn view_dropdown_button(ctx: AppCtx) -> impl IntoElement {
    let ctx = ctx.clone();
    let vm = match snap(&ctx.config).settings.view_mode {
        ViewMode::Grid => "Kacheln",
        ViewMode::List => "Liste",
    };
    rect()
        .height(Size::px(34.0))
        // Explizite Breite (Phasen-2-Mini-Fix): das Label „▤ Kacheln" darf nicht
        // zweizeilig umbrechen („Ka/cheln"). min_width allein reicht in der
        // horizontalen Flex-Zeile nicht, daher feste Breite statt nur Untergrenze.
        .width(Size::px(132.0))
        .corner_radius(design::RADIUS_SMALL)
        .background(design::CANVAS)
        .border(Border::new().fill(design::BORDER).width(1.0))
        .cross_align(Alignment::Center)
        .main_align(Alignment::Center)
        .on_press(move |_| ctx.dropdown_open.set_val(!*ctx.dropdown_open.read()))
        .child(
            label()
                .text(format!("▤ {}", vm))
                .font_size(design::FONT_BODY)
                .color(design::TEXT_SECONDARY),
        )
}

fn content(ctx: AppCtx, apps: Vec<AppEntry>, _cf: String) -> impl IntoElement {
    let vm = snap(&ctx.config).settings.view_mode;
    let body: Element = if apps.is_empty() {
        empty_state(ctx)
    } else if vm == ViewMode::Grid {
        tile_grid(ctx, apps)
    } else {
        list_view(ctx, apps)
    };
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .direction(Direction::Vertical)
        .child(body)
}

fn empty_state(ctx: AppCtx) -> Element {
    let ctx = ctx.clone();
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .direction(Direction::Vertical)
        .cross_align(Alignment::Center)
        .main_align(Alignment::Center)
        .spacing(16.0)
        .child(
            label()
                .text("Noch keine Apps")
                .font_size(design::FONT_TITLE)
                .color(design::TEXT_SECONDARY),
        )
        .child(
            rect()
                .height(Size::px(38.0))
                .corner_radius(design::RADIUS_SMALL)
                .background(design::accent())
                .padding(Gaps::new(0.0, 16.0, 0.0, 16.0))
                .cross_align(Alignment::Center)
                .main_align(Alignment::Center)
                .on_press(move |_| open_app_dialog(&ctx, None))
                .child(
                    label()
                        .text("App hinzufügen")
                        .font_size(design::FONT_BODY)
                        .color(design::CANVAS),
                ),
        )
        .into()
}

fn tile_grid(ctx: AppCtx, apps: Vec<AppEntry>) -> Element {
    let mut idx = 0usize;
    let mut rows: Vec<Element> = Vec::new();
    while idx < apps.len() {
        let end = (idx + GRID_COLS).min(apps.len());
        let mut row = rect()
            .width(Size::fill())
            .height(Size::px(TILE))
            .direction(Direction::Horizontal)
            .spacing(16.0)
            .cross_align(Alignment::Start);
        for (j, app) in apps.iter().enumerate().skip(idx).take(end - idx) {
            row = row.child(tile(ctx.clone(), app.clone(), j));
        }
        rows.push(row.into());
        idx = end;
    }
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .direction(Direction::Vertical)
        .spacing(16.0)
        .padding(Gaps::new_all(20.0))
        .child(rect().direction(Direction::Vertical).children(rows))
        .into()
}

fn tile(ctx: AppCtx, app: AppEntry, index: usize) -> impl IntoElement {
    let app_id = app.id.clone();
    let ctx_d = ctx.clone();
    let ctx_e = ctx.clone();
    let ctx_l = ctx.clone();
    // SPEC §7: konfiguriertes Icon dekodieren + Diagnose (Buchstaben-Fallback visuell).
    if let Some(icon_path) = &app.icon {
        let _ = icons::get_app_icon(icon_path, &ctx.store.logs_dir());
    }
    rect()
        .width(Size::px(TILE))
        .height(Size::px(TILE))
        .corner_radius(design::RADIUS_TILE)
        .background(design::CONTROLS)
        .border(Border::new().fill(design::BORDER).width(1.0))
        .direction(Direction::Vertical)
        .cross_align(Alignment::Center)
        .main_align(Alignment::Center)
        .spacing(8.0)
        .cursor(CursorIcon::Pointer)
        .on_pointer_down(move |e: Event<PointerEventData>| {
            if !e.data().is_primary() {
                return;
            }
            ctx_d.drag.set_val(DragState::Dragging {
                app_id: app_id.clone(),
                grab: e.element_location(),
                pos: e.global_location(),
                target_folder: None,
                target_index: None,
            });
        })
        .on_pointer_enter(move |_| {
            if let DragState::Dragging { .. } = snap(&ctx_e.drag) {
                ctx_e.drag.with_mut_val(|d| {
                    if let DragState::Dragging { target_index, .. } = d {
                        *target_index = Some(index);
                    }
                });
            }
        })
        .on_pointer_leave(move |_| {
            ctx_l.drag.with_mut_val(|d| {
                if let DragState::Dragging { target_index, .. } = d
                    && *target_index == Some(index)
                {
                    *target_index = None;
                }
            });
        })
        .child(tile_visual(Some(app), false))
}

/// Kachel-Inhalt (Icon-Box + Label). `ghost` = halbtransparent für den Zieh-Hover.
fn tile_visual(app: Option<AppEntry>, ghost: bool) -> impl IntoElement {
    let letter = app
        .as_ref()
        .and_then(|a| a.name.chars().next())
        .unwrap_or('?')
        .to_uppercase()
        .next()
        .unwrap_or('?')
        .to_string();
    let name = app.map(|a| a.name).unwrap_or_default();
    let bg = if ghost {
        Color::from_argb(210, 0x1A, 0x1E, 0x29)
    } else {
        design::CONTROLS
    };
    let border = if ghost {
        design::accent()
    } else {
        design::BORDER
    };
    rect()
        .width(Size::px(TILE))
        .height(Size::px(TILE))
        .corner_radius(design::RADIUS_TILE)
        .background(bg)
        .border(Border::new().fill(border).width(1.0))
        .direction(Direction::Vertical)
        .cross_align(Alignment::Center)
        .main_align(Alignment::Center)
        .spacing(8.0)
        .child(
            rect()
                .width(Size::px(TILE_ICON))
                .height(Size::px(TILE_ICON))
                .corner_radius(14.0_f32)
                .background(design::CANVAS)
                .border(Border::new().fill(design::BORDER).width(1.0))
                .cross_align(Alignment::Center)
                .main_align(Alignment::Center)
                .child(
                    label()
                        .text(letter.clone())
                        .font_size(28.0)
                        .color(letter_color(&letter)),
                ),
        )
        .child(
            label()
                .text(name)
                .font_size(design::FONT_BODY)
                .color(design::TEXT_SECONDARY),
        )
}

fn list_view(ctx: AppCtx, apps: Vec<AppEntry>) -> Element {
    let ctx = ctx.clone();
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .direction(Direction::Vertical)
        .spacing(8.0)
        .padding(Gaps::new_all(20.0))
        .children(
            apps.into_iter()
                .enumerate()
                .map(|(i, app)| list_row(ctx.clone(), app, i))
                .collect::<Vec<Element>>(),
        )
        .into()
}

fn list_row(ctx: AppCtx, app: AppEntry, index: usize) -> Element {
    let app_id = app.id.clone();
    let ctx_d = ctx.clone();
    let ctx_e = ctx.clone();
    let ctx_l = ctx.clone();
    let letter = app
        .name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .next()
        .unwrap_or('?')
        .to_string();
    // SPEC §7: konfiguriertes Icon dekodieren + Diagnose (Buchstaben-Fallback visuell).
    if let Some(icon_path) = &app.icon {
        let _ = icons::get_app_icon(icon_path, &ctx.store.logs_dir());
    }
    let name = app.name;
    rect()
        .width(Size::fill())
        .height(Size::px(ROW_H))
        .corner_radius(design::RADIUS_SMALL)
        .background(design::CONTROLS)
        .border(Border::new().fill(design::BORDER).width(1.0))
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .spacing(12.0)
        .padding(Gaps::new(0.0, 10.0, 0.0, 10.0))
        .cursor(CursorIcon::Pointer)
        .on_pointer_down(move |e: Event<PointerEventData>| {
            if !e.data().is_primary() {
                return;
            }
            ctx_d.drag.set_val(DragState::Dragging {
                app_id: app_id.clone(),
                grab: e.element_location(),
                pos: e.global_location(),
                target_folder: None,
                target_index: None,
            });
        })
        .on_pointer_enter(move |_| {
            if let DragState::Dragging { .. } = snap(&ctx_e.drag) {
                ctx_e.drag.with_mut_val(|d| {
                    if let DragState::Dragging { target_index, .. } = d {
                        *target_index = Some(index);
                    }
                });
            }
        })
        .on_pointer_leave(move |_| {
            ctx_l.drag.with_mut_val(|d| {
                if let DragState::Dragging { target_index, .. } = d
                    && *target_index == Some(index)
                {
                    *target_index = None;
                }
            });
        })
        .child(
            rect()
                .width(Size::px(28.0))
                .height(Size::px(28.0))
                .corner_radius(8.0_f32)
                .background(design::CANVAS)
                .border(Border::new().fill(design::BORDER).width(1.0))
                .cross_align(Alignment::Center)
                .main_align(Alignment::Center)
                .child(
                    label()
                        .text(letter.clone())
                        .font_size(15.0)
                        .color(letter_color(&letter)),
                ),
        )
        .child(
            label()
                .text(name)
                .font_size(design::FONT_BODY)
                .color(design::TEXT_PRIMARY),
        )
        .into()
}

/// Ghost (folgt dem Cursor), nur während eines Drag.
fn ghost_tile(ctx: AppCtx) -> Option<impl IntoElement> {
    if let DragState::Dragging {
        app_id, pos, grab, ..
    } = snap(&ctx.drag)
    {
        let cfg = snap(&ctx.config);
        let app = cfg.apps.iter().find(|a| a.id == app_id).cloned();
        let dx = (pos.x - grab.x) as f32;
        let dy = (pos.y - grab.y) as f32;
        Some(
            rect()
                .position(Position::new_global().left(dx).top(dy))
                .layer(Layer::Overlay)
                .interactive(false)
                .child(tile_visual(app, true)),
        )
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Sidebar
// ---------------------------------------------------------------------------

fn sidebar(ctx: AppCtx, cfg: Config, cf: String) -> impl IntoElement {
    let ctx = ctx.clone();
    rect()
        .width(Size::percent(31.0))
        .height(Size::fill())
        .background(design::CONTROLS)
        .direction(Direction::Vertical)
        .main_align(Alignment::SpaceBetween)
        .padding(Gaps::new_all(24.0))
        .child(sidebar_top(ctx.clone(), cfg, cf))
        .child(sidebar_bottom(ctx))
}

fn sidebar_top(ctx: AppCtx, cfg: Config, cf: String) -> impl IntoElement {
    let ctx = ctx.clone();
    rect()
        .direction(Direction::Vertical)
        .spacing(18.0)
        .child(
            rect()
                .direction(Direction::Vertical)
                .spacing(10.0)
                .child(
                    label()
                        .text("QuasarDock")
                        .font_size(design::FONT_TITLE_LARGE)
                        .color(design::TEXT_PRIMARY),
                )
                .child(
                    label()
                        .text("Programmstarter")
                        .font_size(design::FONT_META)
                        .color(design::TEXT_MUTED),
                )
                .child(
                    rect()
                        .width(Size::px(56.0))
                        .height(Size::px(3.0))
                        .corner_radius(2.0_f32)
                        .background(design::accent()),
                ),
        )
        .child(
            rect().direction(Direction::Vertical).spacing(6.0).children(
                cfg.folders
                    .iter()
                    .map(|f| folder_row(ctx.clone(), f.clone(), count_in(&cfg, &f.id), f.id == cf))
                    .collect::<Vec<Element>>(),
            ),
        )
}

fn folder_row(ctx: AppCtx, folder: Folder, count: usize, active: bool) -> Element {
    let fid = folder.id.clone();
    let ctx_p = ctx.clone();
    let ctx_e = ctx.clone();
    let ctx_l = ctx.clone();
    let name = folder.name.clone();
    let renaming = snap(&ctx.folder_renaming) == Some(fid.clone());

    let row = rect()
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
        .on_press({
            let f = fid.clone();
            move |_| ctx_p.current_folder.set_val(f.clone())
        })
        .on_pointer_enter({
            let f = fid.clone();
            move |_| {
                if let DragState::Dragging { .. } = snap(&ctx_e.drag) {
                    ctx_e.drag.with_mut_val(|d| {
                        if let DragState::Dragging { target_folder, .. } = d {
                            *target_folder = Some(f.clone());
                        }
                    });
                }
            }
        })
        .on_pointer_leave({
            let f = fid.clone();
            move |_| {
                ctx_l.drag.with_mut_val(|d| {
                    if let DragState::Dragging { target_folder, .. } = d
                        && target_folder.as_deref() == Some(f.as_str())
                    {
                        *target_folder = None;
                    }
                });
            }
        });

    let inner: Element = if renaming {
        let ctx_r = ctx.clone();
        let fid2 = fid.clone();
        rect()
            .direction(Direction::Horizontal)
            .cross_align(Alignment::Center)
            .spacing(10.0)
            .child(
                rect()
                    .width(Size::px(3.0))
                    .height(Size::px(18.0))
                    .corner_radius(2.0_f32)
                    .background(design::accent()),
            )
            .child(text_field(
                ctx_r.clone(),
                90,
                ctx_r.folder_rename_name,
                "Name",
                150.0,
            ))
            .child(
                rect()
                    .height(Size::px(28.0))
                    .corner_radius(design::RADIUS_SMALL)
                    .background(design::accent())
                    .cross_align(Alignment::Center)
                    .main_align(Alignment::Center)
                    .padding(Gaps::new(0.0, 8.0, 0.0, 8.0))
                    .on_press(move |_| {
                        let n = snap(&ctx_r.folder_rename_name).trim().to_string();
                        if !n.is_empty() {
                            let id = fid2.clone();
                            commit(&ctx_r, |c| {
                                if let Some(f) = c.folders.iter_mut().find(|f| f.id == id) {
                                    f.name = n;
                                }
                            });
                        }
                        ctx_r.folder_renaming.set_val(None);
                    })
                    .child(
                        label()
                            .text("OK")
                            .font_size(design::FONT_META)
                            .color(design::CANVAS),
                    ),
            )
            .into()
    } else {
        rect()
            .direction(Direction::Horizontal)
            .cross_align(Alignment::Center)
            .spacing(10.0)
            .child(
                rect()
                    .width(Size::px(3.0))
                    .height(Size::px(18.0))
                    .corner_radius(2.0_f32)
                    .background(if active {
                        design::accent()
                    } else {
                        design::CONTROLS
                    }),
            )
            .child(
                label()
                    .text(name)
                    .font_size(design::FONT_BODY)
                    .color(if active {
                        design::TEXT_PRIMARY
                    } else {
                        design::TEXT_SECONDARY
                    }),
            )
            .into()
    };

    row.child(inner)
        .child(count_badge(&count.to_string()))
        .into()
}

fn sidebar_bottom(ctx: AppCtx) -> impl IntoElement {
    let ctx_a = ctx.clone();
    let ctx_s = ctx.clone();
    rect()
        .width(Size::fill())
        .direction(Direction::Horizontal)
        .main_align(Alignment::End)
        .spacing(10.0)
        .child(round_button("＋", move |_| {
            let id = {
                let mut new_id = String::new();
                commit(&ctx_a, |c| {
                    new_id = add_folder(c, "Neuer Ordner");
                });
                new_id
            };
            ctx_a.current_folder.set_val(id);
        }))
        .child(round_button("⚙", move |_| {
            ctx_s.settings_open.set_val(true)
        }))
}

fn round_button(glyph: &str, on_press: impl Fn(()) + 'static) -> impl IntoElement {
    let on_press = Rc::new(on_press);
    rect()
        .width(Size::px(42.0))
        .height(Size::px(42.0))
        .corner_radius(21.0_f32)
        .background(design::HOVER)
        .border(Border::new().fill(design::BORDER_STRONG).width(1.0))
        .cross_align(Alignment::Center)
        .main_align(Alignment::Center)
        .on_press(move |_| on_press(()))
        .child(
            label()
                .text(glyph.to_string())
                .font_size(design::FONT_TITLE)
                .color(design::TEXT_SECONDARY),
        )
}

// ---------------------------------------------------------------------------
// Anzeigeoptionen-Dropdown (Kacheln/Liste · Manuell/Alphabetisch)
// ---------------------------------------------------------------------------

fn dropdown_menu(ctx: AppCtx) -> impl IntoElement {
    let ctx_c = ctx.clone();
    let catcher = rect()
        .position(Position::new_global().left(0.0).top(0.0))
        .width(Size::fill())
        .height(Size::fill())
        .background(Color::from_argb(0, 0, 0, 0))
        .layer(Layer::Overlay)
        .on_press(move |_| ctx_c.dropdown_open.set_val(false));

    let cfg = snap(&ctx.config);
    let vm = cfg.settings.view_mode;
    let sm = cfg.settings.sort_mode;

    let ctx_vm = ctx.clone();
    let ctx_sm = ctx.clone();

    let menu = rect()
        .position(
            Position::new_global()
                .left(1052.0)
                .top(TITLEBAR_H + TOOLBAR_H + 6.0),
        )
        .width(Size::px(220.0))
        .background(design::CONTROLS)
        .corner_radius(design::RADIUS_SMALL)
        .border(Border::new().fill(design::BORDER_STRONG).width(1.0))
        .padding(Gaps::new_all(8.0))
        .direction(Direction::Vertical)
        .spacing(6.0)
        .on_press(move |e: Event<PressEventData>| {
            e.stop_propagation();
        })
        .child(menu_section(
            "ANSICHT",
            vec![
                (
                    "Kacheln",
                    vm == ViewMode::Grid,
                    Rc::new({
                        let ctx_vm = ctx_vm.clone();
                        move || {
                            commit(&ctx_vm, |c| c.settings.view_mode = ViewMode::Grid);
                            ctx_vm.dropdown_open.set_val(false);
                        }
                    }),
                ),
                (
                    "Liste",
                    vm == ViewMode::List,
                    Rc::new({
                        let ctx_vm = ctx_vm.clone();
                        move || {
                            commit(&ctx_vm, |c| c.settings.view_mode = ViewMode::List);
                            ctx_vm.dropdown_open.set_val(false);
                        }
                    }),
                ),
            ],
        ))
        .child(menu_section(
            "SORTIERUNG",
            vec![
                (
                    "Manuell",
                    sm == SortMode::Manual,
                    Rc::new({
                        let ctx_sm = ctx_sm.clone();
                        move || {
                            commit(&ctx_sm, |c| c.settings.sort_mode = SortMode::Manual);
                            ctx_sm.dropdown_open.set_val(false);
                        }
                    }),
                ),
                (
                    "Alphabetisch",
                    sm == SortMode::Alphabetical,
                    Rc::new({
                        let ctx_sm = ctx_sm.clone();
                        move || {
                            commit(&ctx_sm, |c| c.settings.sort_mode = SortMode::Alphabetical);
                            ctx_sm.dropdown_open.set_val(false);
                        }
                    }),
                ),
            ],
        ));

    catcher.child(menu)
}

#[allow(clippy::type_complexity)]
fn menu_section(title: &str, items: Vec<(&str, bool, Rc<dyn Fn()>)>) -> impl IntoElement {
    let title = title.to_string();
    rect()
        .direction(Direction::Vertical)
        .spacing(2.0)
        .child(
            label()
                .text(title)
                .font_size(design::FONT_META)
                .color(design::TEXT_MUTED),
        )
        .child(
            rect().direction(Direction::Vertical).spacing(2.0).children(
                items
                    .into_iter()
                    .map(|(lbl, selected, action)| {
                        let action = action.clone();
                        rect()
                            .height(Size::px(30.0))
                            .corner_radius(design::RADIUS_SMALL)
                            .background(if selected {
                                design::HOVER
                            } else {
                                design::CONTROLS
                            })
                            .padding(Gaps::new(0.0, 8.0, 0.0, 8.0))
                            .cross_align(Alignment::Center)
                            .on_press(move |_| action())
                            .child(
                                label()
                                    .text(if selected {
                                        format!("✓ {}", lbl)
                                    } else {
                                        lbl.to_string()
                                    })
                                    .font_size(design::FONT_BODY)
                                    .color(if selected {
                                        design::accent()
                                    } else {
                                        design::TEXT_SECONDARY
                                    }),
                            )
                            .into()
                    })
                    .collect::<Vec<Element>>(),
            ),
        )
}

// ---------------------------------------------------------------------------
// Einstellungs-Overlay
// ---------------------------------------------------------------------------

fn settings_overlay(ctx: AppCtx, cfg: Config) -> impl IntoElement {
    let ctx_c = ctx.clone();
    let tab = *ctx.settings_tab.read();
    rect()
        .position(Position::new_global().left(0.0).top(0.0))
        .width(Size::fill())
        .height(Size::fill())
        .background(Color::from_argb(140, 0, 0, 0))
        .layer(Layer::Overlay)
        .on_press(move |_| ctx_c.settings_open.set_val(false))
        .child(
            // Panel explizit zentriert: (1280-760)/2=260, (800-540)/2=130.
            // Bei Layer::Overlay löst `height(fill)` auf Inhaltshöhe auf, daher
            // keine main_align-Centrierung, sondern absolute Platzierung.
            rect()
                .position(Position::new_global().left(260.0).top(130.0))
                .width(Size::px(760.0))
                .height(Size::px(540.0))
                .corner_radius(16.0_f32)
                .background(design::CONTROLS)
                .border(Border::new().fill(design::BORDER_STRONG).width(1.0))
                .direction(Direction::Vertical)
                .on_press(move |e: Event<PressEventData>| {
                    e.stop_propagation();
                })
                .child(settings_titlebar(ctx.clone()))
                .child(settings_body(ctx, cfg, tab)),
        )
}

fn settings_titlebar(ctx: AppCtx) -> impl IntoElement {
    let ctx_c = ctx.clone();
    let tab = *ctx.settings_tab.read();
    let tabs = ["Ordner", "Apps", "Allgemein", "Darstellung", "Über"];
    rect()
        .width(Size::fill())
        .height(Size::px(52.0))
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .padding(Gaps::new(0.0, 12.0, 0.0, 12.0))
        .border(Border::new().fill(design::BORDER).width(1.0))
        .child(
            rect()
                .direction(Direction::Horizontal)
                .cross_align(Alignment::Center)
                .spacing(6.0)
                .children(
                    tabs.iter()
                        .enumerate()
                        .map(|(i, t)| {
                            let sel = i == tab;
                            let ctx_t = ctx_c.clone();
                            rect()
                                .height(Size::px(32.0))
                                .corner_radius(design::RADIUS_SMALL)
                                .background(if sel {
                                    design::accent()
                                } else {
                                    design::CANVAS
                                })
                                .padding(Gaps::new(0.0, 12.0, 0.0, 12.0))
                                .cross_align(Alignment::Center)
                                .main_align(Alignment::Center)
                                .on_press(move |_| ctx_t.settings_tab.set_val(i))
                                .child(
                                    label()
                                        .text(t.to_string())
                                        .font_size(design::FONT_BODY)
                                        .color(if sel {
                                            design::CANVAS
                                        } else {
                                            design::TEXT_SECONDARY
                                        }),
                                )
                                .into()
                        })
                        .collect::<Vec<Element>>(),
                ),
        )
        .child(
            rect()
                .width(Size::px(32.0))
                .height(Size::px(32.0))
                .corner_radius(8.0_f32)
                .background(design::CANVAS)
                .cross_align(Alignment::Center)
                .main_align(Alignment::Center)
                .on_press(move |_| ctx_c.settings_open.set_val(false))
                .child(
                    label()
                        .text("✕")
                        .font_size(16.0)
                        .color(design::TEXT_SECONDARY),
                ),
        )
}

fn settings_body(ctx: AppCtx, cfg: Config, tab: usize) -> Element {
    match tab {
        0 => tab_ordner(ctx),
        1 => tab_apps(ctx),
        2 => tab_allgemein(ctx),
        3 => tab_darstellung(ctx, cfg),
        _ => tab_ueber(ctx, cfg),
    }
}

fn settings_scroll(content: impl IntoElement) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .direction(Direction::Vertical)
        .padding(Gaps::new_all(16.0))
        .child(content)
        .into()
}

fn tab_ordner(ctx: AppCtx) -> Element {
    let cfg = snap(&ctx.config);
    let ctx_a = ctx.clone();
    let add_name = snap(&ctx.folder_add_name);
    let ctx2 = ctx.clone();
    settings_scroll(
        rect()
            .direction(Direction::Vertical)
            .spacing(12.0)
            .child(
                label()
                    .text("Ordner verwalten")
                    .font_size(design::FONT_TITLE)
                    .color(design::TEXT_PRIMARY),
            )
            .child(
                rect().direction(Direction::Vertical).spacing(6.0).children(
                    cfg.folders
                        .iter()
                        .cloned()
                        .map(|f| folder_manage_row(ctx.clone(), f))
                        .collect::<Vec<Element>>(),
                ),
            )
            .child(
                rect()
                    .direction(Direction::Horizontal)
                    .cross_align(Alignment::Center)
                    .spacing(8.0)
                    .child(text_field(
                        ctx2.clone(),
                        80,
                        ctx2.folder_add_name,
                        "Neuer Ordner …",
                        220.0,
                    ))
                    .child(
                        rect()
                            .height(Size::px(34.0))
                            .corner_radius(design::RADIUS_SMALL)
                            .background(design::accent())
                            .padding(Gaps::new(0.0, 12.0, 0.0, 12.0))
                            .cross_align(Alignment::Center)
                            .main_align(Alignment::Center)
                            .on_press(move |_| {
                                let n = add_name.trim().to_string();
                                if !n.is_empty() {
                                    let mut new_id = String::new();
                                    commit(&ctx_a, |c| new_id = add_folder(c, &n));
                                    ctx_a.current_folder.set_val(new_id);
                                }
                            })
                            .child(
                                label()
                                    .text("Hinzufügen")
                                    .font_size(design::FONT_BODY)
                                    .color(design::CANVAS),
                            ),
                    ),
            ),
    )
}

fn folder_manage_row(ctx: AppCtx, folder: Folder) -> Element {
    let fid = folder.id.clone();
    let protected = folder.protected;
    let name = folder.name.clone();
    let count = count_in(&snap(&ctx.config), &fid);
    let ctx_r = ctx.clone();
    let ctx_d = ctx.clone();
    rect()
        .width(Size::fill())
        .height(Size::px(44.0))
        .corner_radius(design::RADIUS_SMALL)
        .background(design::CANVAS)
        .border(Border::new().fill(design::BORDER).width(1.0))
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
                        .corner_radius(2.0_f32)
                        .background(design::accent()),
                )
                .child(
                    label()
                        .text(name)
                        .font_size(design::FONT_BODY)
                        .color(design::TEXT_PRIMARY),
                )
                .child(count_badge(&count.to_string())),
        )
        .child(
            rect()
                .direction(Direction::Horizontal)
                .cross_align(Alignment::Center)
                .spacing(6.0)
                .child(
                    rect()
                        .height(Size::px(30.0))
                        .corner_radius(design::RADIUS_SMALL)
                        .background(design::HOVER)
                        .padding(Gaps::new(0.0, 10.0, 0.0, 10.0))
                        .cross_align(Alignment::Center)
                        .main_align(Alignment::Center)
                        .on_press({
                            let f = fid.clone();
                            move |_| ctx_r.folder_renaming.set_val(Some(f.clone()))
                        })
                        .child(
                            label()
                                .text("Umbenennen")
                                .font_size(design::FONT_META)
                                .color(design::TEXT_SECONDARY),
                        ),
                )
                .child(
                    rect()
                        .height(Size::px(30.0))
                        .corner_radius(design::RADIUS_SMALL)
                        .background(design::HOVER)
                        .padding(Gaps::new(0.0, 10.0, 0.0, 10.0))
                        .cross_align(Alignment::Center)
                        .main_align(Alignment::Center)
                        .on_press({
                            let f = fid.clone();
                            move |_| {
                                if !protected {
                                    commit(&ctx_d, |c| {
                                        c.remove_folder(&f);
                                    });
                                }
                            }
                        })
                        .child(
                            label()
                                .text(if protected { "geschützt" } else { "Löschen" })
                                .font_size(design::FONT_META)
                                .color(if protected {
                                    design::TEXT_MUTED
                                } else {
                                    design::TEXT_SECONDARY
                                }),
                        ),
                ),
        )
        .into()
}

fn tab_apps(ctx: AppCtx) -> Element {
    let cfg = snap(&ctx.config);
    let cf = snap(&ctx.current_folder);
    let ctx2 = ctx.clone();
    let ctx3 = ctx.clone();
    let folder_selector =
        rect()
            .direction(Direction::Horizontal)
            .cross_align(Alignment::Center)
            .spacing(6.0)
            .children(
                cfg.folders
                    .iter()
                    .cloned()
                    .map(|f| {
                        let sel = f.id == cf;
                        let ctx3 = ctx3.clone();
                        let fid = f.id.clone();
                        rect()
                            .height(Size::px(30.0))
                            .corner_radius(design::RADIUS_SMALL)
                            .background(if sel {
                                design::accent()
                            } else {
                                design::CANVAS
                            })
                            .border(Border::new().fill(design::BORDER).width(1.0))
                            .padding(Gaps::new(0.0, 12.0, 0.0, 12.0))
                            .cross_align(Alignment::Center)
                            .main_align(Alignment::Center)
                            .on_press(move |_| ctx3.current_folder.set_val(fid.clone()))
                            .child(label().text(f.name).font_size(design::FONT_META).color(
                                if sel {
                                    design::CANVAS
                                } else {
                                    design::TEXT_SECONDARY
                                },
                            ))
                            .into()
                    })
                    .collect::<Vec<Element>>(),
            );

    let apps = compute_apps(&cfg, &cf, "", false);
    let list = rect().direction(Direction::Vertical).spacing(6.0).children(
        apps.into_iter()
            .map(|a| app_manage_row(ctx2.clone(), a))
            .collect::<Vec<Element>>(),
    );

    settings_scroll(
        rect()
            .direction(Direction::Vertical)
            .spacing(12.0)
            .child(
                label()
                    .text("Apps")
                    .font_size(design::FONT_TITLE)
                    .color(design::TEXT_PRIMARY),
            )
            .child(folder_selector)
            .child(list)
            .child(
                rect()
                    .height(Size::px(36.0))
                    .corner_radius(design::RADIUS_SMALL)
                    .background(design::accent())
                    .padding(Gaps::new(0.0, 14.0, 0.0, 14.0))
                    .cross_align(Alignment::Center)
                    .main_align(Alignment::Center)
                    .on_press(move |_| open_app_dialog(&ctx, None))
                    .child(
                        label()
                            .text("App hinzufügen")
                            .font_size(design::FONT_BODY)
                            .color(design::CANVAS),
                    ),
            ),
    )
}

fn app_manage_row(ctx: AppCtx, app: AppEntry) -> Element {
    let ctx_e = ctx.clone();
    let id = app.id.clone();
    let name = app.name;
    let path = app.path;
    rect()
        .width(Size::fill())
        .height(Size::px(40.0))
        .corner_radius(design::RADIUS_SMALL)
        .background(design::CANVAS)
        .border(Border::new().fill(design::BORDER).width(1.0))
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
                    label()
                        .text(name)
                        .font_size(design::FONT_BODY)
                        .color(design::TEXT_PRIMARY),
                )
                .child(
                    label()
                        .text(path)
                        .font_size(design::FONT_META)
                        .color(design::TEXT_MUTED),
                ),
        )
        .child(
            rect()
                .direction(Direction::Horizontal)
                .cross_align(Alignment::Center)
                .spacing(6.0)
                .child(
                    rect()
                        .height(Size::px(28.0))
                        .corner_radius(design::RADIUS_SMALL)
                        .background(design::HOVER)
                        .padding(Gaps::new(0.0, 10.0, 0.0, 10.0))
                        .cross_align(Alignment::Center)
                        .main_align(Alignment::Center)
                        .on_press({
                            let ce = ctx_e.clone();
                            let idb = id.clone();
                            move |_| {
                                let a = snap(&ce.config)
                                    .apps
                                    .into_iter()
                                    .find(|x| x.id == idb)
                                    .clone();
                                if let Some(a) = a {
                                    open_app_dialog(&ce, Some(a));
                                }
                            }
                        })
                        .child(
                            label()
                                .text("Bearbeiten")
                                .font_size(design::FONT_META)
                                .color(design::TEXT_SECONDARY),
                        ),
                )
                .child(
                    rect()
                        .height(Size::px(28.0))
                        .corner_radius(design::RADIUS_SMALL)
                        .background(design::HOVER)
                        .padding(Gaps::new(0.0, 10.0, 0.0, 10.0))
                        .cross_align(Alignment::Center)
                        .main_align(Alignment::Center)
                        .on_press({
                            let ce = ctx_e.clone();
                            let idb = id.clone();
                            move |_| {
                                commit(&ce, |c| c.apps.retain(|x| x.id != idb));
                            }
                        })
                        .child(
                            label()
                                .text("Löschen")
                                .font_size(design::FONT_META)
                                .color(design::TEXT_SECONDARY),
                        ),
                ),
        )
        .into()
}

fn tab_allgemein(ctx: AppCtx) -> Element {
    let cfg = snap(&ctx.config);
    let ctx = ctx.clone();
    let ctx2 = ctx.clone();
    let vm = cfg.settings.view_mode;
    let sm = cfg.settings.sort_mode;
    let remember = cfg.settings.remember_geometry;
    let tray = cfg.settings.start_in_tray;
    let auto_min = cfg.settings.auto_minimize_after_launch;
    let autostart = cfg.settings.windows_autostart;
    let dock = cfg.settings.dock_to_edge;

    settings_scroll(
        rect()
            .direction(Direction::Vertical)
            .spacing(12.0)
            .child(
                label()
                    .text("Allgemein")
                    .font_size(design::FONT_TITLE)
                    .color(design::TEXT_PRIMARY),
            )
            .child(
                label()
                    .text("Gilt für den Start und die ganze Sitzung — die Änderungen sind sofort gespeichert.")
                    .font_size(design::FONT_META)
                    .color(design::TEXT_MUTED),
            )
            .child(setting_card(
                "Standardansicht",
                None,
                segmented(
                    &["Kacheln", "Liste"],
                    if vm == ViewMode::Grid { 0 } else { 1 },
                    Rc::new({
                        let cx = ctx.clone();
                        move |i| {
                            commit(&cx.clone(), |c| {
                                c.settings.view_mode = if i == 0 {
                                    ViewMode::Grid
                                } else {
                                    ViewMode::List
                                };
                            });
                        }
                    }),
                ),
            ))
            .child(setting_card(
                "Standardsortierung",
                None,
                segmented(
                    &["Manuell", "Alphabetisch"],
                    if sm == SortMode::Manual { 0 } else { 1 },
                    Rc::new({
                        let cx = ctx2.clone();
                        move |i| {
                            commit(&cx.clone(), |c| {
                                c.settings.sort_mode = if i == 0 {
                                    SortMode::Manual
                                } else {
                                    SortMode::Alphabetical
                                };
                            });
                        }
                    }),
                ),
            ))
            .child(setting_card(
                "Fenstergröße und -position merken",
                Some("Nur aus sichtbaren Zuständen."),
                switch(remember, {
                    let ctx_r = ctx.clone();
                    Rc::new(move || {
                        commit(&ctx_r, |c| {
                            c.settings.remember_geometry = !c.settings.remember_geometry;
                        });
                    })
                }),
            ))
            .child(setting_card(
                "Beim Start ins Tray minimieren",
                None,
                switch(tray, {
                    let ctx_r = ctx.clone();
                    Rc::new(move || {
                        commit(&ctx_r, |c| c.settings.start_in_tray = !c.settings.start_in_tray);
                    })
                }),
            ))
            .child(setting_card(
                "Automatisch ins Tray minimieren nach App-Start",
                None,
                switch(auto_min, {
                    let ctx_r = ctx.clone();
                    Rc::new(move || {
                        commit(&ctx_r, |c| {
                            c.settings.auto_minimize_after_launch =
                                !c.settings.auto_minimize_after_launch;
                        });
                    })
                }),
            ))
            .child(setting_card(
                "Windows-Autostart",
                Some("XDG-Autostart-Pfad (Linux) bzw. Registry (Windows)."),
                switch(autostart, {
                    let ctx_r = ctx.clone();
                    Rc::new(move || {
                        commit(&ctx_r, |c| {
                            c.settings.windows_autostart = !c.settings.windows_autostart;
                        });
                        let now = ctx_r.config.read().settings.windows_autostart;
                        // Live in Registry (Windows) / XDG (Linux) schreiben; Self-Heal bei stale Pfad.
                        if let Err(e) = autostart::set_autostart(now) {
                            eprintln!("[autostart] {}", e);
                        }
                    })
                }),
            ))
            .child(setting_card(
                "Am Bildschirmrand andocken",
                Some("Fenster bündig an rechten Rand + Oberkante der Taskleiste."),
                switch(dock, {
                    let ctx_r = ctx.clone();
                    Rc::new(move || {
                        commit(&ctx_r, |c| {
                            c.settings.dock_to_edge = !c.settings.dock_to_edge;
                        });
                    })
                }),
            )),
    )
}

fn setting_card(title: &str, hint: Option<&str>, control: impl IntoElement) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .corner_radius(design::RADIUS_SMALL)
        .background(design::CANVAS)
        .border(Border::new().fill(design::BORDER).width(1.0))
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .main_align(Alignment::SpaceBetween)
        .padding(Gaps::new(12.0, 14.0, 12.0, 14.0))
        .child(
            rect()
                .direction(Direction::Vertical)
                .spacing(2.0)
                .child(
                    label()
                        .text(title.to_string())
                        .font_size(design::FONT_BODY)
                        .color(design::TEXT_PRIMARY),
                )
                .child(
                    label()
                        .text(hint.unwrap_or("").to_string())
                        .font_size(design::FONT_META)
                        .color(design::TEXT_MUTED),
                ),
        )
        .child(control)
}

fn tab_darstellung(ctx: AppCtx, cfg: Config) -> Element {
    let current = cfg.settings.accent_color.clone();
    let ctx = ctx.clone();
    let presets = [
        "#00B4D8", "#00E5FF", "#3B82F6", "#8B5CF6", "#EC4899", "#EF4444", "#F59E0B", "#22C55E",
    ];
    settings_scroll(
        rect()
            .direction(Direction::Vertical)
            .spacing(12.0)
            .child(
                label()
                    .text("Darstellung")
                    .font_size(design::FONT_TITLE)
                    .color(design::TEXT_PRIMARY),
            )
            .child(
                label()
                    .text("Akzentfarbe (Voreinstellungen + eigene Hex-Farbe). Wirkt sofort auf die Oberfläche.")
                    .font_size(design::FONT_META)
                    .color(design::TEXT_MUTED),
            )
            .child(
                rect()
                    .direction(Direction::Horizontal)
                    .cross_align(Alignment::Center)
                    .spacing(12.0)
                    .children(
                        presets
                            .iter()
                            .map(|hex| {
                                let hex = hex.to_string();
                                let selected = current.as_deref() == Some(hex.as_str());
                                let ctx = ctx.clone();
                                let rgb = hex_to_rgb(&hex);
                                rect()
                                    .width(Size::px(40.0))
                                    .height(Size::px(40.0))
                                    .corner_radius(20.0_f32)
                                    .background(rgb)
                                    .border(
                                        Border::new()
                                            .fill(if selected {
                                                design::TEXT_PRIMARY
                                            } else {
                                                design::BORDER
                                            })
                                            .width(if selected { 3.0 } else { 1.0 }),
                                    )
                                    .on_press(move |_| {
                                        commit(&ctx.clone(), |c| c.settings.accent_color = Some(hex.clone()));
                                        design::init_accent_from(Some(&hex));
                                    })
                                    .into()
                            })
                            .collect::<Vec<Element>>(),
                    ),
            )
            .child(text_field(
                ctx.clone(),
                70,
                ctx.accent_hex,
                "#00B4D8 (eigene Farbe)",
                220.0,
            ))
            .child(
                rect()
                    .height(Size::px(34.0))
                    .corner_radius(design::RADIUS_SMALL)
                    .background(design::accent())
                    .padding(Gaps::new(0.0, 12.0, 0.0, 12.0))
                    .cross_align(Alignment::Center)
                    .main_align(Alignment::Center)
                    .on_press(move |_| {
                        let h = snap(&ctx.accent_hex).trim().to_string();
                        if h.starts_with('#') && h.len() == 7 {
                            commit(&ctx.clone(), |c| c.settings.accent_color = Some(h));
                        }
                    })
                    .child(
                        label()
                            .text("Anwenden")
                            .font_size(design::FONT_BODY)
                            .color(design::CANVAS),
                    ),
            ),
    )
}

fn tab_ueber(ctx: AppCtx, cfg: Config) -> Element {
    let data_dir = ctx.store.data_dir();
    let path = data_dir.to_string_lossy().to_string();
    let accent = cfg.settings.accent_color.clone();
    settings_scroll(
        rect()
            .direction(Direction::Vertical)
            .spacing(10.0)
            .child(
                label()
                    .text("QuasarDock")
                    .font_size(design::FONT_TITLE)
                    .color(design::TEXT_PRIMARY),
            )
            .child(
                label()
                    .text("Version 0.1.0  ·  Build: Freya 0.5.2  ·  Rust")
                    .font_size(design::FONT_BODY)
                    .color(design::TEXT_SECONDARY),
            )
            .child(
                label()
                    .text(format!(
                        "Ordner: {}   Apps: {}",
                        cfg.folders.len(),
                        cfg.apps.len()
                    ))
                    .font_size(design::FONT_META)
                    .color(design::TEXT_MUTED),
            )
            .child(
                label()
                    .text(format!("Datenablage: {}", path))
                    .font_size(design::FONT_META)
                    .color(design::TEXT_MUTED),
            )
            .child(
                label()
                    .text(if accent.is_some() {
                        "Akzentfarbe: benutzerdefiniert."
                    } else {
                        "Akzentfarbe: eingefrorenes Cyan (#00B4D8)."
                    })
                    .font_size(design::FONT_META)
                    .color(design::TEXT_MUTED),
            )
            .child(
                label()
                    .text("Status der Datenablage: OK (portabel, EXE-relativ).")
                    .font_size(design::FONT_META)
                    .color(design::POSITIVE),
            ),
    )
}

// ---------------------------------------------------------------------------
// App-Dialog (Hinzufügen/Bearbeiten)
// ---------------------------------------------------------------------------

fn app_dialog(ctx: AppCtx) -> impl IntoElement {
    let ctx_c = ctx.clone();
    let ctx_s = ctx.clone();
    let ctx_d = ctx.clone();
    let existing = snap(&ctx.app_edit);
    rect()
        .position(Position::new_global().left(0.0).top(0.0))
        .width(Size::fill())
        .height(Size::fill())
        .direction(Direction::Vertical)
        .main_align(Alignment::Center)
        .cross_align(Alignment::Center)
        .background(Color::from_argb(140, 0, 0, 0))
        .layer(Layer::Overlay)
        .on_press({
            let c = ctx_c.clone();
            move |_| close_app_dialog(&c.clone())
        })
        .child(
            rect()
                .position(Position::new_global().left(360.0).top(120.0))
                .width(Size::px(560.0))
                .corner_radius(16.0_f32)
                .background(design::CONTROLS)
                .border(Border::new().fill(design::BORDER_STRONG).width(1.0))
                .direction(Direction::Vertical)
                .padding(Gaps::new_all(18.0))
                .spacing(12.0)
                .on_press(move |e: Event<PressEventData>| {
                    e.stop_propagation();
                })
                .child(
                    label()
                        .text(if existing.is_some() {
                            "App bearbeiten"
                        } else {
                            "App hinzufügen"
                        })
                        .font_size(design::FONT_TITLE)
                        .color(design::TEXT_PRIMARY),
                )
                .child(field_row(
                    "Name",
                    text_field(ctx_d.clone(), 2, ctx_d.app_dlg_name, "Anzeigename", 360.0),
                ))
                .child(field_row(
                    "Pfad",
                    text_field(
                        ctx_d.clone(),
                        3,
                        ctx_d.app_dlg_path,
                        "/Pfad/zur/Datei",
                        360.0,
                    ),
                ))
                .child(field_row(
                    "Argumente",
                    text_field(ctx_d.clone(), 4, ctx_d.app_dlg_args, "optional", 360.0),
                ))
                .child(field_row(
                    "Arbeitsverzeichnis",
                    text_field(ctx_d.clone(), 5, ctx_d.app_dlg_cwd, "optional", 360.0),
                ))
                .child(field_row(
                    "Icon",
                    text_field(
                        ctx_d.clone(),
                        6,
                        ctx_d.app_dlg_icon,
                        "optional (.png/.ico)",
                        360.0,
                    ),
                ))
                .child(folder_select(ctx_d.clone()))
                .child(startstate_select(ctx_d.clone()))
                .child(auto_start_row(ctx_d.clone()))
                .child(
                    rect()
                        .direction(Direction::Horizontal)
                        .main_align(Alignment::End)
                        .spacing(10.0)
                        .child(
                            rect()
                                .height(Size::px(36.0))
                                .corner_radius(design::RADIUS_SMALL)
                                .background(design::HOVER)
                                .padding(Gaps::new(0.0, 14.0, 0.0, 14.0))
                                .cross_align(Alignment::Center)
                                .main_align(Alignment::Center)
                                .on_press(move |_| close_app_dialog(&ctx_s))
                                .child(
                                    label()
                                        .text("Abbrechen")
                                        .font_size(design::FONT_BODY)
                                        .color(design::TEXT_SECONDARY),
                                ),
                        )
                        .child(
                            rect()
                                .height(Size::px(36.0))
                                .corner_radius(design::RADIUS_SMALL)
                                .background(design::accent())
                                .padding(Gaps::new(0.0, 14.0, 0.0, 14.0))
                                .cross_align(Alignment::Center)
                                .main_align(Alignment::Center)
                                .on_press({
                                    let c = ctx_c.clone();
                                    move |_| save_app_dialog(&c.clone())
                                })
                                .child(
                                    label()
                                        .text("Speichern")
                                        .font_size(design::FONT_BODY)
                                        .color(design::CANVAS),
                                ),
                        ),
                ),
        )
}

fn field_row(label_text: &str, input: impl IntoElement) -> impl IntoElement {
    rect()
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .spacing(12.0)
        .child(
            rect().width(Size::px(140.0)).child(
                label()
                    .text(label_text.to_string())
                    .font_size(design::FONT_BODY)
                    .color(design::TEXT_SECONDARY),
            ),
        )
        .child(input)
}

fn folder_select(ctx: AppCtx) -> impl IntoElement {
    let cfg = snap(&ctx.config);
    let cf = snap(&ctx.app_dlg_folder);
    rect()
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .spacing(12.0)
        .child(
            rect().width(Size::px(140.0)).child(
                label()
                    .text("Ordner")
                    .font_size(design::FONT_BODY)
                    .color(design::TEXT_SECONDARY),
            ),
        )
        .child(
            rect()
                .direction(Direction::Horizontal)
                .cross_align(Alignment::Center)
                .spacing(6.0)
                .children(
                    cfg.folders
                        .iter()
                        .cloned()
                        .map(|f| {
                            let sel = f.id == cf;
                            let ctx = ctx.clone();
                            let fid = f.id.clone();
                            rect()
                                .height(Size::px(30.0))
                                .corner_radius(design::RADIUS_SMALL)
                                .background(if sel {
                                    design::accent()
                                } else {
                                    design::CANVAS
                                })
                                .border(Border::new().fill(design::BORDER).width(1.0))
                                .padding(Gaps::new(0.0, 10.0, 0.0, 10.0))
                                .cross_align(Alignment::Center)
                                .main_align(Alignment::Center)
                                .on_press(move |_| ctx.app_dlg_folder.set_val(fid.clone()))
                                .child(label().text(f.name).font_size(design::FONT_META).color(
                                    if sel {
                                        design::CANVAS
                                    } else {
                                        design::TEXT_SECONDARY
                                    },
                                ))
                                .into()
                        })
                        .collect::<Vec<Element>>(),
                ),
        )
}

fn startstate_select(ctx: AppCtx) -> impl IntoElement {
    let sel = *ctx.app_dlg_start.read();
    rect()
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .spacing(12.0)
        .child(
            rect().width(Size::px(140.0)).child(
                label()
                    .text("Startzustand")
                    .font_size(design::FONT_BODY)
                    .color(design::TEXT_SECONDARY),
            ),
        )
        .child(segmented(
            &["Normal", "Minimiert", "Maximiert"],
            sel,
            Rc::new(move |i| ctx.app_dlg_start.set_val(i)),
        ))
}

fn auto_start_row(ctx: AppCtx) -> impl IntoElement {
    let on = *ctx.app_dlg_auto.read();
    rect()
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .spacing(12.0)
        .child(
            rect().width(Size::px(140.0)).child(
                label()
                    .text("Auto-Start")
                    .font_size(design::FONT_BODY)
                    .color(design::TEXT_SECONDARY),
            ),
        )
        .child(switch(on, Rc::new(move || ctx.app_dlg_auto.set_val(!on))))
}

// ---------------------------------------------------------------------------
// Wiederverwendbare Bedienelemente
// ---------------------------------------------------------------------------

/// Eigenes Textfeld (kein HTML): tippt in den übergebenen State, Fokus über Klick.
fn text_field(
    ctx: AppCtx,
    id: usize,
    value: State<String>,
    placeholder: &str,
    width: f32,
) -> impl IntoElement {
    let ctx_press = ctx.clone();
    let ctx_kb = ctx.clone();
    let focused = *ctx_press.focused_id.read() == id;
    let text = value.read().clone();
    let is_empty = text.is_empty();
    let show = if is_empty {
        placeholder.to_string()
    } else {
        text
    };
    // Platzhalter-Dimmstufe (lesbarer als TEXT_MUTED), SPEC-Abnahme Mini-Fix.
    let color = if is_empty && !focused {
        Color::from_argb(255, 0x64, 0x74, 0x8B)
    } else {
        design::TEXT_PRIMARY
    };
    rect()
        .width(Size::px(width))
        .height(Size::px(34.0))
        .corner_radius(design::RADIUS_SMALL)
        .background(if focused {
            design::HOVER
        } else {
            design::CANVAS
        })
        .border(
            Border::new()
                .fill(if focused {
                    design::accent()
                } else {
                    design::BORDER
                })
                .width(1.0),
        )
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .padding(Gaps::new(0.0, 10.0, 0.0, 10.0))
        .on_press(move |_| ctx_press.focused_id.set_val(id))
        .on_global_key_down(move |e: Event<KeyboardEventData>| {
            if *ctx_kb.focused_id.read() != id {
                return;
            }
            match &e.data().key {
                Key::Character(s) => {
                    if s == " " {
                        value.with_mut_val(|v| v.push(' '));
                    } else {
                        value.with_mut_val(|v| v.push_str(s));
                    }
                }
                Key::Named(NamedKey::Backspace) => {
                    value.with_mut_val(|v| {
                        v.pop();
                    });
                }
                _ => {}
            }
        })
        .child(label().text(show).font_size(design::FONT_BODY).color(color))
}

/// Umschalter (Schalter), SPEC §4.5.
fn switch(current: bool, on_toggle: Rc<dyn Fn()>) -> impl IntoElement {
    let w = 40.0_f32;
    let h = 22.0_f32;
    let thumb = 18.0_f32;
    rect()
        .width(Size::px(w))
        .height(Size::px(h))
        .corner_radius(h / 2.0_f32)
        .background(if current {
            design::accent()
        } else {
            design::HOVER
        })
        .on_press(move |_| on_toggle())
        .padding(Gaps::new_all(2.0))
        .direction(Direction::Horizontal)
        .main_align(if current {
            Alignment::End
        } else {
            Alignment::Start
        })
        .child(
            rect()
                .width(Size::px(thumb))
                .height(Size::px(thumb))
                .corner_radius(thumb / 2.0_f32)
                .background(design::TEXT_PRIMARY),
        )
}

/// Segmente (z. B. Kacheln/Liste), SPEC §3.
fn segmented(
    options: &'static [&'static str],
    selected: usize,
    on_select: Rc<dyn Fn(usize)>,
) -> impl IntoElement {
    let mut row = rect()
        .direction(Direction::Horizontal)
        .spacing(6.0)
        .cross_align(Alignment::Center);
    for (i, &lbl) in options.iter().enumerate() {
        let sel = i == selected;
        let on_select = on_select.clone();
        let pill = rect()
            .height(Size::px(30.0))
            .min_width(Size::px(72.0))
            .corner_radius(design::RADIUS_SMALL)
            .background(if sel {
                design::accent()
            } else {
                design::CANVAS
            })
            .border(
                Border::new()
                    .fill(if sel {
                        design::accent()
                    } else {
                        design::BORDER
                    })
                    .width(1.0),
            )
            .cross_align(Alignment::Center)
            .main_align(Alignment::Center)
            .on_press(move |_| on_select(i))
            .child(
                label()
                    .text(lbl)
                    .font_size(design::FONT_BODY)
                    .color(if sel {
                        design::CANVAS
                    } else {
                        design::TEXT_SECONDARY
                    }),
            );
        row = row.child(pill);
    }
    row
}

fn hex_to_rgb(hex: &str) -> Color {
    if hex.starts_with('#') && hex.len() == 7 {
        let r = u8::from_str_radix(&hex[1..3], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[3..5], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[5..7], 16).unwrap_or(0);
        Color::from_rgb(r, g, b)
    } else {
        design::accent()
    }
}
