//! Windows-Autostart (SPEC §4.5 / §6): Registry `HKCU\...\Run` unter Windows,
//! XDG-Autostart (`~/.config/autostart`) unter Linux. Status frisch lesen,
//! sauber entfernbar; Self-Heal bei stale Pfad.
//!
//! Die reinen Hilfsfunktionen (`format_autostart_command`, `autostart_needs_heal`)
//! sind plattformunabhängig und unit-testbar.

#[cfg(unix)]
use std::path::PathBuf;

/// Registry-Schlüssel unter `HKCU`.
pub const REG_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
/// Wertname im Autostart-Schlüssel.
pub const REG_VALUE: &str = "QuasarDock";

/// Dateiname des XDG-Autostart-Eintrags (Linux).
pub const DESKTOP_FILE: &str = "quasardock.desktop";

/// Klammert den Pfad in Anführungszeichen, wenn er Leerzeichen enthält.
pub fn format_autostart_command(exe_path: &str) -> String {
    if exe_path.contains(' ') {
        format!("\"{}\"", exe_path)
    } else {
        exe_path.to_string()
    }
}

/// Muss der gespeicherte Autostart-Eintrag geheilt werden? (stale Pfad)
pub fn autostart_needs_heal(stored: Option<&str>, current_exe: &str) -> bool {
    match stored {
        None => false,
        Some(s) => s.trim() != format_autostart_command(current_exe).trim(),
    }
}

/// Aktueller EXE-Pfad als Autostart-Befehl (plattformneutral).
pub fn current_exe_command() -> String {
    std::env::current_exe()
        .map(|p| format_autostart_command(&p.to_string_lossy()))
        .unwrap_or_default()
}

/// Liest, ob der Autostart aktiv ist.
pub fn read_autostart() -> bool {
    #[cfg(windows)]
    {
        windows_read()
    }
    #[cfg(not(windows))]
    {
        unix_read()
    }
}

/// Aktiviert (`true`) oder entfernt (`false`) den Autostart-Eintrag.
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_set(enabled)
    }
    #[cfg(not(windows))]
    {
        unix_set(enabled)
    }
}

/// Self-Heal: vorhandenen, aber abweichenden (stale) Eintrag entfernen.
/// `should_enable` gibt an, ob danach ein frischer Eintrag geschrieben wird.
pub fn self_heal_autostart(should_enable: bool) -> Result<(), String> {
    let stored = read_raw();
    let current = current_exe_command();
    if autostart_needs_heal(stored.as_deref(), &current) {
        // Stale Eintrag löschen.
        remove_raw()?;
    }
    if should_enable {
        set_autostart(true)
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Rohe Lese-/Schreib-Hilfen pro Plattform
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn read_raw() -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::*;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu.open_subkey(REG_KEY).ok()?;
    run.get_value::<String, _>(REG_VALUE).ok()
}

#[cfg(windows)]
fn remove_raw() -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::*;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu.open_subkey_with_flags(REG_KEY, KEY_WRITE).ok();
    if let Some(run) = run {
        let _ = run.delete_value(REG_VALUE);
    }
    Ok(())
}

#[cfg(windows)]
fn windows_read() -> bool {
    read_raw()
        .map(|s| s.trim() == current_exe_command().trim())
        .unwrap_or(false)
}

#[cfg(windows)]
fn windows_set(enabled: bool) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::*;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run, _) = hkcu.create_subkey(REG_KEY).map_err(|e| e.to_string())?;
    if enabled {
        run.set_value(REG_VALUE, &current_exe_command())
            .map_err(|e| e.to_string())?;
    } else {
        let _ = run.delete_value(REG_VALUE);
    }
    Ok(())
}

#[cfg(unix)]
fn xdg_autostart_dir() -> Option<PathBuf> {
    if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
        if !x.is_empty() {
            return Some(PathBuf::from(x).join("autostart"));
        }
    }
    if let Ok(h) = std::env::var("HOME") {
        return Some(PathBuf::from(h).join(".config").join("autostart"));
    }
    None
}

#[cfg(unix)]
fn desktop_path() -> Option<PathBuf> {
    xdg_autostart_dir().map(|d| d.join(DESKTOP_FILE))
}

#[cfg(unix)]
fn read_raw() -> Option<String> {
    let p = desktop_path()?;
    let text = std::fs::read_to_string(p).ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Exec=") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

#[cfg(unix)]
fn remove_raw() -> Result<(), String> {
    if let Some(p) = desktop_path() {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

#[cfg(unix)]
fn unix_read() -> bool {
    read_raw()
        .map(|s| s.trim() == current_exe_command().trim())
        .unwrap_or(false)
}

#[cfg(unix)]
fn unix_set(enabled: bool) -> Result<(), String> {
    let dir = xdg_autostart_dir().ok_or_else(|| "Kein XDG_CONFIG_HOME/HOME".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = dir.join(DESKTOP_FILE);
    if enabled {
        let cmd = current_exe_command();
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=QuasarDock\nExec={}\nX-GNOME-Autostart-enabled=true\nHidden=false\n",
            cmd
        );
        std::fs::write(&p, content).map_err(|e| e.to_string())?;
    } else {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_mit_leerzeichen_geklammert() {
        assert_eq!(format_autostart_command("/bin/app"), "/bin/app");
        assert_eq!(
            format_autostart_command("C:\\Prog Files\\a.exe"),
            "\"C:\\Prog Files\\a.exe\""
        );
    }

    #[test]
    fn heal_erkennt_stale_pfad() {
        assert!(!autostart_needs_heal(None, "/x/app"));
        assert!(!autostart_needs_heal(Some("/x/app"), "/x/app"));
        assert!(autostart_needs_heal(Some("/alt/old.exe"), "/x/app"));
        assert!(autostart_needs_heal(
            Some("\"C:\\alt\\old.exe\""),
            "\"C:\\neu\\app.exe\""
        ));
    }

    #[cfg(unix)]
    #[test]
    fn xdg_autostart_an_aus() {
        // Temporäres XDG-Verzeichnis, damit kein echtes Home verändert wird.
        let base = std::env::temp_dir().join(format!("qd-autostart-test-{}", std::process::id()));
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &base) };
        let _ = std::fs::remove_dir_all(&base);

        assert!(!read_autostart(), "anfangs aus");
        set_autostart(true).expect("aktivieren");
        assert!(read_autostart(), "jetzt aktiv");
        set_autostart(false).expect("deaktivieren");
        assert!(!read_autostart(), "wieder aus");

        let _ = std::fs::remove_dir_all(&base);
        unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
    }
}
