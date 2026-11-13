//! Portable Pfade (SPEC §5): `<Programmverzeichnis>/Data/config.json` und
//! `<Programmverzeichnis>/Data/logs/`. Basis ist der echte EXE-Pfad
//! (`std::env::current_exe()`), nie Working-Dir, nie `%APPDATA%`.

use std::path::{Path, PathBuf};

/// Liefert das Verzeichnis der ausgeführten EXE (das Programmverzeichnis).
pub fn exe_dir() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| std::io::Error::other("EXE hat kein Elternverzeichnis"))
}

/// Reiner, plattformunabhängiger Hilfswert: zu einem beliebigen EXE-Pfad das
/// zugehörige `Data`-Verzeichnis. (Testbar ohne echte EXE.)
pub fn data_dir_for(exe_path: &Path) -> PathBuf {
    exe_path
        .parent()
        .map(|p| p.join("Data"))
        .unwrap_or_else(|| PathBuf::from("Data"))
}

/// Das `Data`-Verzeichnis dieser Installation (relativ zur EXE).
pub fn data_dir() -> std::io::Result<PathBuf> {
    Ok(data_dir_for(&std::env::current_exe()?))
}

/// Pfad zu `config.json` (relativ zur EXE).
pub fn config_path() -> std::io::Result<PathBuf> {
    Ok(data_dir()?.join("config.json"))
}

/// Verzeichnis für Laufzeit-Logs (`Data/logs/`).
pub fn logs_dir() -> std::io::Result<PathBuf> {
    Ok(data_dir()?.join("logs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_liegt_unter_exe() {
        // Plattformunabhängig: exe_parent/Data.
        let p = data_dir_for(Path::new("/opt/quasardock/quasardock"));
        assert_eq!(p, Path::new("/opt/quasardock/Data"));
    }

    #[test]
    fn config_path_ist_data_config_json() {
        let p = data_dir_for(Path::new("/opt/quasardock/quasardock"));
        assert_eq!(
            p.join("config.json"),
            Path::new("/opt/quasardock/Data/config.json")
        );
    }

    #[test]
    fn logs_dir_ist_data_logs() {
        let p = data_dir_for(Path::new("/opt/quasardock/quasardock"));
        assert_eq!(p.join("logs"), Path::new("/opt/quasardock/Data/logs"));
    }
}
