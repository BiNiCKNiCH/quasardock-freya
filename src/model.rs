//! Datenmodell (SPEC §5) — `Config`, `Settings`, `AppEntry`, `Folder`.
//!
//! Enthält (De-)Serialisierung, `normalize()` (doppelte IDs entfernen,
//! Ordnerzuordnung reparieren, Favoriten-Schutz) und Migrationen älterer
//! Feldnamen. Unbekannte Felder werden beim Laden akzeptiert und beim nächsten
//! Save nicht mehr geschrieben (sauber rausschreiben).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Name des geschützten Standardordners (umbenennbar, nie löschbar).
pub const FAVORITES_NAME: &str = "Favoriten";
/// Stabile interne ID des Favoriten-Ordners.
pub const FAVORITES_ID: &str = "favorites";

/// Ansicht der App-Liste (SPEC §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    Grid,
    List,
}

/// Sortierung der App-Liste (SPEC §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortMode {
    Manual,
    Alphabetical,
}

/// Startzustand einer einzelnen App (SPEC §4.8 / §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartState {
    #[serde(alias = "Normal", alias = "normal")]
    Normal,
    #[serde(alias = "Minimiert", alias = "Minimized", alias = "minimized")]
    Minimized,
    #[serde(alias = "Maximiert", alias = "Maximized", alias = "maximized")]
    Maximized,
}

/// Fenstergeometrie in logischen Pixeln (Bildschirmkoordinaten).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WindowGeom {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Default for WindowGeom {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            w: 1280,
            h: 800,
        }
    }
}

/// Dauerhafte Einstellungen (SPEC §4.5 / §6).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Beim Start ins Tray minimieren (SPEC §4.5 / §6).
    pub start_in_tray: bool,
    /// Automatisch ins Tray minimieren nach App-Start.
    pub auto_minimize_after_launch: bool,
    /// Am Bildschirmrand andocken.
    pub dock_to_edge: bool,
    /// Fenstergröße und -position merken.
    pub remember_geometry: bool,
    /// Windows-Autostart.
    pub windows_autostart: bool,
    /// Gespeicherte Fenstergeometrie (nur aus sichtbaren Zuständen, SPEC §6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<WindowGeom>,
    /// Eigene Akzentfarbe (Hex); `None` = eingefrorenes Cyan (SPEC §4.5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent_color: Option<String>,
    /// Ansicht (Kacheln/Liste).
    pub view_mode: ViewMode,
    /// Sortierung (Manuell/Alphabetisch).
    pub sort_mode: SortMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            start_in_tray: false,
            auto_minimize_after_launch: false,
            dock_to_edge: false,
            remember_geometry: true,
            windows_autostart: false,
            geometry: None,
            accent_color: None,
            view_mode: ViewMode::Grid,
            sort_mode: SortMode::Manual,
        }
    }
}

/// Ein App-Eintrag (SPEC §5).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub args: String,
    pub cwd: String,
    /// Optionale Icon-Datei; `None` = Buchstaben-Fallback (SPEC §7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// ID des zugeordneten Ordners (`folder_id`).
    pub folder_id: String,
    pub sort_order: i64,
    pub start_state: StartState,
    /// Automatischer Start mit Startkette-Verzögerung (SPEC §4.8).
    pub auto_start: bool,
    /// Verzögerung (ms) innerhalb der Auto-Start-Kette (SPEC §4.8).
    #[serde(default)]
    pub auto_start_delay_ms: u64,
}

impl Default for AppEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            path: String::new(),
            args: String::new(),
            cwd: String::new(),
            icon: None,
            folder_id: FAVORITES_ID.to_string(),
            sort_order: 0,
            start_state: StartState::Normal,
            auto_start: false,
            auto_start_delay_ms: 0,
        }
    }
}

/// Ein Ordner (SPEC §4.3 / §5).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[derive(Default)]
pub struct Folder {
    pub id: String,
    pub name: String,
    /// Geschützte Ordner (z. B. „Favoriten") sind nicht löschbar.
    pub protected: bool,
}

/// Wurzel der Konfiguration (SPEC §5): `Config { folders, apps, settings }`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub folders: Vec<Folder>,
    pub apps: Vec<AppEntry>,
    pub settings: Settings,
}

impl Default for Config {
    fn default() -> Self {
        let mut c = Config {
            folders: Vec::new(),
            apps: Vec::new(),
            settings: Settings::default(),
        };
        c.ensure_favorites();
        c
    }
}

impl Config {
    /// Leere, aber gültige Config mit garantiert vorhandenem Favoriten-Ordner.
    pub fn new() -> Self {
        Config::default()
    }

    /// Stellt sicher, dass der Favoriten-Ordner existiert und geschützt ist.
    fn ensure_favorites(&mut self) {
        let idx = self.folders.iter().position(|f| f.name == FAVORITES_NAME);
        match idx {
            Some(i) => {
                // ID auf die stabile Konstante zwingen, Schutz einschalten.
                self.folders[i].id = FAVORITES_ID.to_string();
                self.folders[i].protected = true;
            }
            None => {
                self.folders.insert(
                    0,
                    Folder {
                        id: FAVORITES_ID.to_string(),
                        name: FAVORITES_NAME.to_string(),
                        protected: true,
                    },
                );
            }
        }
    }

    /// Normalisiert die Config (SPEC §5):
    /// 1. doppelte Ordner-IDs entfernen (erste Behalte),
    /// 2. Favoriten-Schutz sicherstellen,
    /// 3. doppelte App-IDs entfernen (erste Behalte),
    /// 4. ungültige `folder_id` einer App → Favoriten.
    pub fn normalize(&mut self) {
        let mut seen = HashSet::new();
        self.folders.retain(|f| seen.insert(f.id.clone()));

        self.ensure_favorites();

        let mut seen_app = HashSet::new();
        self.apps.retain(|a| seen_app.insert(a.id.clone()));

        let folder_ids: HashSet<String> = self.folders.iter().map(|f| f.id.clone()).collect();
        for app in self.apps.iter_mut() {
            if !folder_ids.contains(&app.folder_id) {
                app.folder_id = FAVORITES_ID.to_string();
            }
        }
    }

    /// Entfernt einen Ordner. Geschützte Ordner (Favoriten) werden nicht
    /// entfernt (Rückgabe `false`). Enthaltene Apps wandern nach Favoriten.
    pub fn remove_folder(&mut self, id: &str) -> bool {
        let pos = match self.folders.iter().position(|f| f.id == id) {
            Some(p) => p,
            None => return false,
        };
        if self.folders[pos].protected {
            return false;
        }
        self.folders.remove(pos);
        for app in self.apps.iter_mut() {
            if app.folder_id == id {
                app.folder_id = FAVORITES_ID.to_string();
            }
        }
        self.normalize();
        true
    }

    /// Parst JSON mit Migration älterer Feldnamen und normalisiert anschließend.
    pub fn from_json_str(s: &str) -> Result<Config, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_str(s)?;
        let migrated = migrate_value(value);
        let mut cfg: Config = serde_json::from_value(migrated)?;
        cfg.normalize();
        Ok(cfg)
    }

    /// Serialisiert als pretty JSON (nur bekannte Felder, keine unbekannten).
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Wendet bekannte Migrationen auf den rohen JSON-Wert an, bevor er in
/// `Config` deserialisiert wird (SPEC §5: „Migrationen älterer Feldnamen").
fn migrate_value(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut()
        && let Some(settings) = obj.get_mut("settings").and_then(|s| s.as_object_mut())
    {
        migrate_settings(settings);
    }
    value
}

/// Migriert Legacy-Einstellungsfelder in die aktuelle Form.
fn migrate_settings(settings: &mut serde_json::Map<String, serde_json::Value>) {
    // v2.7: Das Dropdown `start_state` (Normal/Minimiert) wurde durch den
    // Schalter `start_in_tray` ersetzt; alte Werte migrieren.
    if let Some(v) = settings.remove("start_state") {
        let minimized = match &v {
            serde_json::Value::String(s) => s == "Minimiert" || s.eq_ignore_ascii_case("minimized"),
            serde_json::Value::Bool(b) => *b,
            _ => false,
        };
        if minimized {
            settings.insert("start_in_tray".to_string(), serde_json::Value::Bool(true));
        }
    }
    // Noch älter: boolesches `minimized`.
    if let Some(v) = settings.remove("minimized")
        && v.as_bool() == Some(true)
    {
        settings.insert("start_in_tray".to_string(), serde_json::Value::Bool(true));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_enthalten_favoriten() {
        let c = Config::new();
        let fav = c.folders.iter().find(|f| f.id == FAVORITES_ID).unwrap();
        assert_eq!(fav.name, FAVORITES_NAME);
        assert!(fav.protected);
    }

    #[test]
    fn normalize_entfernt_doppelte_ids_und_reparatur_ordner() {
        let mut c = Config::new();
        c.folders.push(Folder {
            id: "dev".into(),
            name: "Dev".into(),
            protected: false,
        });
        c.folders.push(Folder {
            id: "dev".into(),
            name: "Dev-Duplikat".into(),
            protected: false,
        });
        c.apps.push(AppEntry {
            id: "a1".into(),
            name: "X".into(),
            folder_id: "dev".into(),
            ..Default::default()
        });
        c.apps.push(AppEntry {
            id: "a1".into(),
            name: "X-Duplikat".into(),
            folder_id: "dev".into(),
            ..Default::default()
        });
        c.apps.push(AppEntry {
            id: "a2".into(),
            name: "Y".into(),
            folder_id: "existiert-nicht".into(),
            ..Default::default()
        });
        c.normalize();
        assert_eq!(c.folders.iter().filter(|f| f.id == "dev").count(), 1);
        assert_eq!(c.apps.len(), 2, "doppelte App-ID entfernt");
        assert_eq!(
            c.apps[1].folder_id, FAVORITES_ID,
            "ungültige Ordnerzuordnung repariert"
        );
    }

    #[test]
    fn favoriten_nicht_loeschbar() {
        let mut c = Config::new();
        assert!(
            !c.remove_folder(FAVORITES_ID),
            "Favoriten darf nicht gelöscht werden"
        );
        assert!(c.folders.iter().any(|f| f.id == FAVORITES_ID));

        c.folders.push(Folder {
            id: "dev".into(),
            name: "Dev".into(),
            protected: false,
        });
        c.apps.push(AppEntry {
            id: "a1".into(),
            name: "Editor".into(),
            folder_id: "dev".into(),
            ..Default::default()
        });
        assert!(c.remove_folder("dev"));
        assert!(!c.folders.iter().any(|f| f.id == "dev"));
        assert_eq!(
            c.apps[0].folder_id, FAVORITES_ID,
            "Apps wandern nach Favoriten"
        );
    }

    #[test]
    fn migration_alte_feldnamen() {
        let legacy = r#"{
            "folders": [
                {"id":"fav","name":"Favoriten","protected":true},
                {"id":"dev","name":"Dev"}
            ],
            "apps": [
                {"id":"a1","name":"X","path":"/x","folder_id":"dev","legacy_unknown":123}
            ],
            "settings": {"start_state":"Minimiert","auto_minimize_after_launch":true}
        }"#;
        let c = Config::from_json_str(legacy).unwrap();
        assert!(
            c.settings.start_in_tray,
            "start_state=Minimiert -> start_in_tray"
        );
        assert!(c.settings.auto_minimize_after_launch);
        // Unbekanntes Feld wurde akzeptiert (kein Fehler) und beim Save weggelassen.
        let out = c.to_json_string().unwrap();
        assert!(
            !out.contains("legacy_unknown"),
            "unbekanntes Feld darf nicht erneut geschrieben werden"
        );
        // Favoriten-ID wurde auf die Konstante normalisiert.
        assert!(
            c.folders
                .iter()
                .any(|f| f.id == FAVORITES_ID && f.protected)
        );
        // App blieb im existierenden Ordner „dev".
        assert_eq!(c.apps[0].folder_id, "dev");
    }

    #[test]
    fn unbekannte_felder_werden_akzeptiert() {
        let json = r#"{"folders":[],"apps":[],"settings":{"foo":42,"start_in_tray":true}}"#;
        let c = Config::from_json_str(json).unwrap();
        assert!(c.settings.start_in_tray);
        let out = c.to_json_string().unwrap();
        assert!(
            !out.contains("\"foo\""),
            "unbekanntes Feld wird nicht zurückgeschrieben"
        );
    }

    #[test]
    fn roundtrip_ist_stabil() {
        let mut c = Config::new();
        c.settings.accent_color = Some("#00E5FF".into());
        c.folders.push(Folder {
            id: "dev".into(),
            name: "Dev".into(),
            protected: false,
        });
        c.apps.push(AppEntry {
            id: "a1".into(),
            name: "Editor".into(),
            path: "/usr/bin/code".into(),
            folder_id: "dev".into(),
            start_state: StartState::Maximized,
            auto_start: true,
            ..Default::default()
        });
        let s = c.to_json_string().unwrap();
        let c2 = Config::from_json_str(&s).unwrap();
        assert_eq!(c, c2);
    }
}
