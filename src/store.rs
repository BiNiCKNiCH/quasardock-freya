//! Threadsicherer, portabler Store mit atomarem Schreiben, Debounce-Autosave
//! und Quarantäne korrupter Dateien (SPEC §5).
//!
//! - Schreiben erfolgt über Temp-Datei + `rename` (atomar, nie ein korrupter
//!   Zwischenzustand in `config.json`).
//! - Auto-Save ist debounced (~600 ms): schnelle Änderungsfolgen werden zu
//!   einem Schreibvorgang zusammengefasst.
//! - Eine korrupte `config.json` wird in Quarantäne verschoben; die App startet
//!   mit Defaults.
//! - Ein nicht beschreibbarer `Data/`-Pfad führt zu einem erkennbaren Fehler
//!   (`StoreError::NotWritable`), niemals zu einem stillen Fallback.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::model::Config;
use crate::paths;

/// Wartefenster für den debounced Auto-Save (SPEC §5: ~600 ms).
const DEBOUNCE: Duration = Duration::from_millis(600);

/// Fehlerzustand des Stores (kein stiller Fallback).
#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Json(serde_json::Error),
    /// Der `Data/`-Pfad ist nicht beschreibbar.
    NotWritable(PathBuf),
}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Json(e)
    }
}

struct Inner {
    config: Config,
    data_dir: PathBuf,
    config_path: PathBuf,
    logs_dir: PathBuf,
}

/// Threadsicherer Store. Hält die Config hinter einem `Mutex` und schreibt
/// asynchron über einen Hintergrund-Thread (Debounce).
pub struct Store {
    inner: Arc<Mutex<Inner>>,
    save_tx: Sender<()>,
    writes: Arc<AtomicU64>,
    #[allow(dead_code)]
    thread: Option<JoinHandle<()>>,
}

impl Store {
    /// Öffnet (oder erstellt) den Store. Standardmäßig wird der EXE-relativen
    /// `Data/`-Pfad genutzt. `base_dir` überschreibt ihn (für Tests).
    pub fn open(base_dir: Option<PathBuf>) -> Result<Store, StoreError> {
        let data_dir = match base_dir {
            Some(d) => d,
            None => paths::data_dir()?,
        };
        let config_path = data_dir.join("config.json");
        let logs_dir = data_dir.join("logs");

        // Datenverzeichnis anlegen, sofern möglich.
        if let Err(e) = fs::create_dir_all(&data_dir) {
            if e.kind() != io::ErrorKind::AlreadyExists {
                return Err(StoreError::NotWritable(data_dir));
            }
        }

        let config = load_config(&config_path, &data_dir)?;

        let inner = Arc::new(Mutex::new(Inner {
            config,
            data_dir: data_dir.clone(),
            config_path: config_path.clone(),
            logs_dir: logs_dir.clone(),
        }));

        let (save_tx, save_rx) = channel::<()>();
        let writes = Arc::new(AtomicU64::new(0));
        let thread_inner = Arc::clone(&inner);
        let thread_writes = Arc::clone(&writes);
        let thread = thread::spawn(move || {
            save_loop(thread_inner, thread_writes, save_rx);
        });

        Ok(Store {
            inner,
            save_tx,
            writes,
            thread: Some(thread),
        })
    }

    /// Aktuelle Config (Kopie).
    pub fn config(&self) -> Config {
        self.inner.lock().unwrap().config.clone()
    }

    /// Verändert die Config über eine Closure und löst einen debounced Save aus.
    pub fn update<F: FnOnce(&mut Config)>(&self, f: F) {
        {
            let mut g = self.inner.lock().unwrap();
            f(&mut g.config);
            g.config.normalize();
        }
        let _ = self.save_tx.send(());
    }

    /// Debounced Save anfordern (ohne inhaltliche Änderung).
    pub fn request_save(&self) {
        let _ = self.save_tx.send(());
    }

    /// Sofort synchron schreiben (z. B. vor dem Beenden).
    pub fn save_now(&self) -> Result<(), StoreError> {
        let guard = self.inner.lock().unwrap();
        let res = write_atomic(&guard.config_path, &guard.data_dir, &guard.config);
        if res.is_ok() {
            self.writes.fetch_add(1, Ordering::Relaxed);
        }
        res
    }

    /// Anzahl tatsächlicher Schreibvorgänge (für Tests / Telemetrie).
    pub fn write_count(&self) -> u64 {
        self.writes.load(Ordering::Relaxed)
    }

    /// Pfad der aktuellen `config.json`.
    pub fn config_path(&self) -> PathBuf {
        self.inner.lock().unwrap().config_path.clone()
    }

    /// Das genutzte Datenverzeichnis.
    pub fn data_dir(&self) -> PathBuf {
        self.inner.lock().unwrap().data_dir.clone()
    }

    /// Das Laufzeit-Log-Verzeichnis.
    pub fn logs_dir(&self) -> PathBuf {
        self.inner.lock().unwrap().logs_dir.clone()
    }
}

/// Hintergrund-Thread: sammelt Save-Anforderungen und schreibt debounced.
fn save_loop(inner: Arc<Mutex<Inner>>, writes: Arc<AtomicU64>, rx: Receiver<()>) {
    loop {
        // Auf die erste Anforderung warten.
        if rx.recv().is_err() {
            return; // Sender dropped -> beenden.
        }
        // Debounce-Fenster: jede weitere Anforderung verlängert es auf 600 ms.
        let mut deadline = Instant::now() + DEBOUNCE;
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            match rx.recv_timeout(deadline - now) {
                Ok(()) => deadline = Instant::now() + DEBOUNCE,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        if let Ok(guard) = inner.lock() {
            if write_atomic(&guard.config_path, &guard.data_dir, &guard.config).is_ok() {
                writes.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// Lädt die Config: valide Datei → geparst; korrupt/leer → Quarantäne + Defaults.
fn load_config(config_path: &Path, data_dir: &Path) -> Result<Config, StoreError> {
    if config_path.exists() {
        let text = fs::read_to_string(config_path)?;
        if text.trim().is_empty() {
            let _ = quarantine(config_path, data_dir);
            return Ok(Config::new());
        }
        match Config::from_json_str(&text) {
            Ok(cfg) => Ok(cfg),
            Err(_) => {
                // Korrupt → Quarantäne + Defaults.
                let _ = quarantine(config_path, data_dir);
                Ok(Config::new())
            }
        }
    } else {
        Ok(Config::new())
    }
}

/// Verschiebt eine korrupte `config.json` nach `config.json.corrupt-<ns>` und
/// bewahrt so den Originalinhalt zur Wiederherstellung.
fn quarantine(config_path: &Path, _data_dir: &Path) -> io::Result<()> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos().to_string())
        .unwrap_or_else(|_| "0".to_string());
    let mut target = config_path.to_path_buf();
    target.set_extension(format!("json.corrupt-{}", stamp));
    let _ = fs::rename(config_path, &target);
    Ok(())
}

/// Schreibt die Config atomar: zuerst in eine eindeutige Temp-Datei, dann
/// `rename` auf das Ziel. Ein Leser sieht immer entweder die alte oder die
/// neue, niemals eine halbgeschriebene Datei.
pub fn write_atomic(
    config_path: &Path,
    data_dir: &Path,
    config: &Config,
) -> Result<(), StoreError> {
    fs::create_dir_all(data_dir)?;
    let json = config.to_json_string()?;

    // Eindeutiger Temp-Name (PID + ns + Zähler), um Parallelität zu entkoppeln.
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_name = format!("config.json.tmp-{}-{}-{}", pid, nanos, n);
    let tmp_path = data_dir.join(tmp_name);

    {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(json.as_bytes())?;
        f.sync_all()?;
    }
    // Atomares Verschieben (Temp → Ziel). Auf demselben Dateisystem ist rename
    // atomar; ein evtl. verbliebener Temp-Rest wird aufgeräumt.
    fs::rename(&tmp_path, config_path)?;
    let _ = fs::remove_file(&tmp_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AppEntry, Config, FAVORITES_ID, Folder};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Frisches, prozess-eindeutiges Temp-Verzeichnis (SPEC §10: kein festes
    /// `/tmp`, sondern `temp_dir()` + PID + Zähler).
    fn tmp_base(name: &str) -> PathBuf {
        let pid = std::process::id();
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("quasardock-test-{}-{}-{}", pid, name, n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn config_landet_in_data_relativ_zur_exe() {
        // Nutzt den echten EXE-Pfad (portabel, nie Working-Dir/AppData).
        let store = Store::open(None).expect("Store öffnen");
        store.update(|c| {
            c.apps.push(AppEntry {
                id: "beleg".into(),
                name: "Beleg".into(),
                ..Default::default()
            });
        });
        // Debounce abwarten.
        thread::sleep(DEBOUNCE + Duration::from_millis(500));
        let cfg_path = store.config_path();
        assert!(
            cfg_path.exists(),
            "config.json muss existieren: {:?}",
            cfg_path
        );

        // Relativ zur EXE:
        let exe = std::env::current_exe().unwrap();
        let exe_dir = exe.parent().unwrap();
        assert!(
            cfg_path.starts_with(exe_dir),
            "Pfad {:?} muss unter EXE-Dir {:?} liegen",
            cfg_path,
            exe_dir
        );
        // Portabilität: nichts in AppData.
        let p = cfg_path.to_string_lossy().to_lowercase();
        assert!(
            !p.contains("appdata"),
            "Darf nicht in AppData liegen: {}",
            p
        );

        // Aufräumen, damit kein Schrott im Build-Output bleibt.
        let _ = fs::remove_dir_all(exe_dir.join("Data"));
    }

    #[test]
    fn atomares_schreiben_kein_korrupter_zwischenzustand() {
        let base = tmp_base("atomic");
        fs::create_dir_all(&base).unwrap();
        let cfg_path = base.join("config.json");
        let base_a = Arc::new(base.clone());
        let cfg_a = Arc::new(cfg_path.clone());

        let mut handles = Vec::new();
        for i in 0..40u32 {
            let base = Arc::clone(&base_a);
            let cfg = Arc::clone(&cfg_a);
            handles.push(thread::spawn(move || {
                for _ in 0..5 {
                    let mut c = Config::new();
                    c.apps.push(AppEntry {
                        id: format!("app{}", i),
                        name: format!("A{}", i),
                        ..Default::default()
                    });
                    write_atomic(&cfg, &base, &c).unwrap();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        // Final eine bekannte Config schreiben und einlesen.
        let mut final_cfg = Config::new();
        final_cfg.apps.push(AppEntry {
            id: "final".into(),
            name: "Final".into(),
            ..Default::default()
        });
        write_atomic(&cfg_path, &base, &final_cfg).unwrap();

        // Datei muss valides, vollständiges JSON sein (nie ein halber Zustand).
        let text = fs::read_to_string(&cfg_path).unwrap();
        let parsed = Config::from_json_str(&text).expect("config.json muss valides JSON sein");
        assert_eq!(parsed.apps.len(), 1);
        assert_eq!(parsed.apps[0].id, "final");

        // Keine Temp-Reste übrig.
        let tmp_left = fs::read_dir(&base)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().contains(".tmp-"));
        assert!(!tmp_left, "es dürfen keine Temp-Dateien übrig bleiben");
    }

    #[test]
    fn debounce_coalesced_save() {
        let base = tmp_base("debounce");
        let store = Store::open(Some(base.clone())).unwrap();
        for i in 0..10u32 {
            store.update(|c| {
                c.settings.windows_autostart = i % 2 == 0;
            });
        }
        // Nach dem Burst und dem Debounce-Fenster genau EIN Schreibvorgang.
        thread::sleep(DEBOUNCE + Duration::from_millis(400));
        let count = store.write_count();
        assert_eq!(count, 1, "10 schnelle Updates -> 1 coalesced Save");
        // Letzter Wert erhalten.
        assert_eq!(store.config().settings.windows_autostart, 9 % 2 == 0);
    }

    #[test]
    fn korrupte_datei_wird_quarantaeniert() {
        let base = tmp_base("quarantine");
        fs::create_dir_all(&base).unwrap();
        let cfg_path = base.join("config.json");
        let corrupt = "{ this is not valid json ";
        fs::write(&cfg_path, corrupt).unwrap();

        let store = Store::open(Some(base.clone())).unwrap();
        // Config startet mit Defaults (Favoriten vorhanden).
        let c = store.config();
        assert!(
            c.folders.iter().any(|f| f.id == FAVORITES_ID),
            "startet mit Defaults"
        );
        // Quarantäne: Original bewahrt.
        let quarantined: Vec<PathBuf> = fs::read_dir(&base)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains(".corrupt-")
            })
            .collect();
        assert!(
            !quarantined.is_empty(),
            "korrupte Datei muss quarantäniert sein"
        );
        let qtext = fs::read_to_string(&quarantined[0]).unwrap();
        assert!(
            qtext.contains("this is not valid"),
            "Originaltext muss erhalten bleiben"
        );
        // Nach einem Save sind die Defaults valide auf Disk.
        store.save_now().unwrap();
        let reloaded = Config::from_json_str(&fs::read_to_string(&cfg_path).unwrap()).unwrap();
        assert!(reloaded.folders.iter().any(|f| f.id == FAVORITES_ID));
    }

    #[test]
    fn leere_datei_wird_wie_korrupt_behandelt() {
        let base = tmp_base("empty");
        fs::create_dir_all(&base).unwrap();
        let cfg_path = base.join("config.json");
        fs::write(&cfg_path, "").unwrap();
        let store = Store::open(Some(base.clone())).unwrap();
        assert!(store.config().folders.iter().any(|f| f.id == FAVORITES_ID));
        let quarantined = fs::read_dir(&base)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().contains(".corrupt-"));
        assert!(quarantined);
    }

    #[test]
    fn ordner_loeschen_verschiebt_apps_nach_favoriten() {
        let base = tmp_base("remove");
        let store = Store::open(Some(base.clone())).unwrap();
        store.update(|c| {
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
        });
        // Favoriten darf nicht gelöscht werden.
        store.update(|c| {
            assert!(!c.remove_folder(FAVORITES_ID));
        });
        store.update(|c| {
            assert!(c.remove_folder("dev"));
        });
        let cfg = store.config();
        assert!(!cfg.folders.iter().any(|f| f.id == "dev"));
        assert_eq!(cfg.apps[0].folder_id, FAVORITES_ID);
    }

    // Nicht beschreibbarer `Data/`-Pfad → erkennbarer Fehler (kein stiller Fallback).
    #[cfg(unix)]
    #[test]
    fn nicht_beschreibbarer_pfad_fehler_unix() {
        use std::os::unix::fs::PermissionsExt;
        let base = tmp_base("readonly");
        let data = base.join("Data");
        fs::create_dir_all(&data).unwrap();
        let mut perms = fs::metadata(&data).unwrap().permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&data, perms).unwrap();

        let store = Store::open(Some(data.clone())).unwrap();
        let res = store.save_now();
        assert!(
            res.is_err(),
            "Schreiben in Read-only-Verzeichnis muss fehlschlagen"
        );

        // Berechtigung zurücksetzen, damit Aufräumen klappt.
        let mut p2 = fs::metadata(&data).unwrap().permissions();
        p2.set_mode(0o755);
        fs::set_permissions(&data, p2).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn nicht_beschreibbarer_pfad_fehler_windows() {
        let base = tmp_base("readonly");
        let data = base.join("Data");
        fs::create_dir_all(&data).unwrap();
        let mut perms = fs::metadata(&data).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&data, perms).unwrap();

        let store = Store::open(Some(data.clone())).unwrap();
        let res = store.save_now();
        assert!(res.is_err(), "Read-only Data/-Pfad muss Fehler erzeugen");

        let mut p2 = fs::metadata(&data).unwrap().permissions();
        p2.set_readonly(false);
        fs::set_permissions(&data, p2).unwrap();
    }
}
