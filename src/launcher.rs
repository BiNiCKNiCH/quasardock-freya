//! App-Start (SPEC §4.9): detached `std::process::Command`, Argumente-Parser
//! (Quotes), Arbeitsverzeichnis, Admin-Elevation (`runas` auf Windows),
//! Startfehler als Klartext. App-Optionen: Startzustand + Auto-Start-Kette.

use crate::model::{AppEntry, StartState};
use std::process::{Command, Stdio};

/// Parst eine Argumentzeile shell-artig mit Double-Quote-Unterstützung.
///
/// - Tokens durch Whitespace getrennt (außerhalb von Anführungszeichen).
/// - `"..."` fasst Leerzeichen zusammen; `\"` ist ein escaped Quote.
/// - Backslash vor Quote escaped ebenfalls ein Quote; sonst literal.
pub fn parse_args(input: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_q {
                    if chars.peek() == Some(&'"') {
                        cur.push('"');
                        chars.next();
                    } else {
                        in_q = false;
                    }
                } else {
                    in_q = true;
                }
            }
            '\\' if in_q => {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    cur.push('\\');
                }
            }
            c if c.is_whitespace() && !in_q => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Fehler beim Starten einer App (Klartext-fähig).
#[derive(Debug)]
pub enum LaunchError {
    EmptyPath,
    Spawn(std::io::Error),
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LaunchError::EmptyPath => write!(f, "Kein Programmpfad angegeben."),
            LaunchError::Spawn(e) => write!(f, "App konnte nicht gestartet werden: {}", e),
        }
    }
}

/// Startanforderung (bereits aufgelöste Felder).
pub struct LaunchRequest {
    pub path: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub start_state: StartState,
    pub elevate: bool,
}

/// Startet eine App detached (entkoppelt vom QuasarDock-Prozess).
///
/// - stdin/stdout/stderr werden auf `null` umgelenkt.
/// - unter Windows mit `CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS`.
/// - bei `elevate` unter Windows via `runas` (UAC).
pub fn launch(req: &LaunchRequest) -> Result<(), LaunchError> {
    if req.path.trim().is_empty() {
        return Err(LaunchError::EmptyPath);
    }

    #[cfg(windows)]
    if req.elevate {
        return launch_elevated(req);
    }

    let mut cmd = Command::new(&req.path);
    cmd.args(&req.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = &req.cwd {
        if !cwd.trim().is_empty() {
            cmd.current_dir(cwd);
        }
    }
    #[cfg(windows)]
    {
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Prozess entkoppeln: neue Sitzung, ignoriere SIGHUP.
        let _ = cmd.process_group(0);
    }
    cmd.spawn().map_err(LaunchError::Spawn)?;
    Ok(())
}

#[cfg(windows)]
fn launch_elevated(req: &LaunchRequest) -> Result<(), LaunchError> {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    // runas über cmd: "runas /user:Administrator \"pfad\" args".
    let quoted = if req.path.contains(' ') {
        format!("\"{}\"", req.path)
    } else {
        req.path.clone()
    };
    let mut argline = quoted;
    for a in &req.args {
        argline.push(' ');
        argline.push_str(a);
    }
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "runas", "/user:Administrator", &argline])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
    cmd.spawn().map_err(LaunchError::Spawn)?;
    Ok(())
}

/// Baut eine `LaunchRequest` aus einem `AppEntry`.
pub fn request_from_app(app: &AppEntry) -> LaunchRequest {
    LaunchRequest {
        path: app.path.clone(),
        args: parse_args(&app.args),
        cwd: if app.cwd.trim().is_empty() {
            None
        } else {
            Some(app.cwd.clone())
        },
        start_state: app.start_state,
        elevate: false,
    }
}

/// Auto-Start-Kette: alle Apps mit `auto_start`, sortiert nach Verzögerung.
/// Liefert `(AppEntry, Verzögerung_ms)` in Ausführungsreihenfolge.
pub fn auto_start_plan(apps: &[AppEntry]) -> Vec<(AppEntry, u64)> {
    let mut v: Vec<(AppEntry, u64)> = apps
        .iter()
        .filter(|a| a.auto_start)
        .map(|a| (a.clone(), a.auto_start_delay_ms))
        .collect();
    v.sort_by_key(|(_, d)| *d);
    v
}

/// Führt die Auto-Start-Kette im Hintergrund aus (jede App mit ihrer Verzögerung).
/// Startfehler werden protokolliert, brechen die Kette nicht ab.
pub fn run_auto_start(apps: &[AppEntry]) {
    let plan = auto_start_plan(apps);
    if plan.is_empty() {
        return;
    }
    let plan: Vec<(AppEntry, u64)> = plan.into_iter().collect();
    std::thread::spawn(move || {
        for (app, delay_ms) in plan {
            if delay_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
            let req = request_from_app(&app);
            if let Err(e) = launch(&req) {
                eprintln!("[autostart] {}: {}", app.name, e);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AppEntry;

    #[test]
    fn parse_args_einfach_und_quotes() {
        assert_eq!(parse_args("firefox"), vec!["firefox"]);
        assert_eq!(
            parse_args(r#"app "arg mit leerzeichen" x"#),
            vec!["app", "arg mit leerzeichen", "x"]
        );
    }

    #[test]
    fn parse_args_windows_pfad_mit_leerzeichen() {
        // Windows-Pfad mit Leerzeichen + Argument in Anführungszeichen.
        assert_eq!(
            parse_args(r#""C:\Program Files\App\app.exe" /x "a b""#),
            vec![r"C:\Program Files\App\app.exe", "/x", "a b"]
        );
    }

    #[test]
    fn parse_args_escaped_quote() {
        assert_eq!(
            parse_args(r#"say "er sagte \"hi\"" ende"#),
            vec!["say", "er sagte \"hi\"", "ende"]
        );
    }

    #[test]
    fn leerer_pfad_fehler() {
        let req = LaunchRequest {
            path: String::new(),
            args: vec![],
            cwd: None,
            start_state: StartState::Normal,
            elevate: false,
        };
        assert!(matches!(launch(&req), Err(LaunchError::EmptyPath)));
    }

    #[cfg(unix)]
    #[test]
    fn detached_start_unix() {
        let req = LaunchRequest {
            path: "/bin/echo".into(),
            args: vec!["hallo".into()],
            cwd: None,
            start_state: StartState::Normal,
            elevate: false,
        };
        assert!(launch(&req).is_ok(), "echo muss detached starten");
    }

    #[cfg(windows)]
    #[test]
    fn detached_start_windows() {
        // %COMSPEC% ist unter Windows immer vorhanden.
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into());
        let req = LaunchRequest {
            path: comspec,
            args: vec!["/c".into(), "exit".into(), "0".into()],
            cwd: None,
            start_state: StartState::Normal,
            elevate: false,
        };
        assert!(launch(&req).is_ok());
    }

    #[test]
    fn auto_start_kette_sortiert_nach_verzoegerung() {
        let mut a = AppEntry::default();
        a.id = "a".into();
        a.auto_start = true;
        a.auto_start_delay_ms = 2000;
        let mut b = AppEntry::default();
        b.id = "b".into();
        b.auto_start = true;
        b.auto_start_delay_ms = 0;
        let mut c = AppEntry::default();
        c.id = "c".into();
        c.auto_start = false; // nicht in der Kette
        let plan = auto_start_plan(&[c, a, b]);
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].0.id, "b");
        assert_eq!(plan[1].0.id, "a");
    }

    #[test]
    fn request_from_app_parst_args() {
        let mut app = AppEntry::default();
        app.path = "/usr/bin/code".into();
        app.args = "--new-window \"file.txt\"".into();
        let req = request_from_app(&app);
        assert_eq!(req.args, vec!["--new-window", "file.txt"]);
        assert!(req.cwd.is_none());
    }
}
