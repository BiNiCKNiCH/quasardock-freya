// Betet das Programmicon (icons/icon.ico) als Windows-Ressource ein, damit die
// EXE im Explorer / in der Taskleiste das echte QuasarDock-Icon zeigt (Defekt 2).
//
// Nur auf Windows (msvc) aktiv — der Linux-Build ist unverändert. `icon.ico` ist
// eingefroren (sha256 c47f1512…, nicht verändern).
fn main() {
    #[cfg(all(windows, target_env = "msvc"))]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/icon.ico")
            .set("ProductName", "QuasarDock")
            .set("FileDescription", "Portabler Programmstarter")
            .set("FileVersion", "0.1.0")
            .set("ProductVersion", "0.1.0");
        // Beim Cross-Check (Linux -> windows-msvc, ohne rc.exe) tolerieren wir das
        // Fehlen der Ressource; der echte Windows-Build (CI) muss gelingen.
        if let Err(e) = res.compile() {
            eprintln!("winresource: Icon nicht eingebettet (Cross-Check?): {e}");
        }
    }
}
