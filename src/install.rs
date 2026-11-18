//! `<app> --install`: das laufende Programm nach `~/.local/bin/<name>`, Icon und Startmenü-Eintrag
//! (`~/.local/share/applications/<id>.desktop`) für den aktuellen Nutzer, ohne root. Texte aus `[app]` in
//! qu.toml. Entfernen: diese drei Dateien löschen. macOS: `~/Applications/<Titel>.app` und `~/.local/bin/<name>`
//! als Verweis darauf. Windows: install.ps1 der App (braucht die DLLs daneben).

use std::path::PathBuf;

/// Inhalt des Startmenü-Eintrags.
fn desktop_entry(bin: &str) -> String {
    let a = &crate::config().app;
    let keywords: String = a.keywords.iter().map(|k| format!("{k};")).collect();
    let mut s = format!("[Desktop Entry]\nType=Application\nName={}\n", a.title);
    if !a.generic_name.is_empty() {
        s += &format!("GenericName={}\n", a.generic_name);
    }
    if !a.comment.is_empty() {
        s += &format!("Comment={}\n", a.comment);
    }
    s += &format!("Exec={bin} %u\nIcon={}\nTerminal=false\nCategories=AudioVideo;Video;Player;\n", a.id);
    if !keywords.is_empty() {
        s += &format!("Keywords={keywords}\n");
    }
    s + &format!("StartupWMClass={}\nStartupNotify=true\n", a.id)
}

#[cfg(not(target_os = "macos"))]
pub fn run(icon_svg: &str) -> Result<(), String> {
    let a = &crate::config().app;
    let home = PathBuf::from(std::env::var("HOME").map_err(|e| e.to_string())?);
    let bin = home.join(".local/bin").join(&a.name);
    let icon = home.join(".local/share/icons/hicolor/scalable/apps").join(format!("{}.svg", a.id));
    let apps = home.join(".local/share/applications");
    let desktop = apps.join(format!("{}.desktop", a.id));
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    for dir in [bin.parent().unwrap(), icon.parent().unwrap(), &apps] {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    // Erst löschen, dann kopieren: eine laufende alte Version behält ihre Datei (sonst „Text file busy“).
    if me != bin {
        let _ = std::fs::remove_file(&bin);
        std::fs::copy(&me, &bin).map_err(|e| format!("{}: {e}", bin.display()))?;
    }
    std::fs::write(&icon, icon_svg).map_err(|e| format!("{}: {e}", icon.display()))?;
    std::fs::write(&desktop, desktop_entry(&bin.to_string_lossy())).map_err(|e| format!("{}: {e}", desktop.display()))?;
    // Startmenü neu einlesen (falls das Werkzeug da ist).
    let _ = std::process::Command::new("update-desktop-database").arg("-q").arg(&apps).status();
    println!("Installiert: {}", bin.display());
    println!("Startmenü:   {}", desktop.display());
    Ok(())
}

/// macOS: App-Bundle in `~/Applications` (Launchpad/Spotlight), Programm darin, `~/.local/bin/<name>` zeigt darauf.
// ponytail: ohne Icon (bräuchte .icns aus dem SVG); GTK/mpv bleiben in Homebrew, das Bundle läuft nur auf diesem Mac.
#[cfg(target_os = "macos")]
pub fn run(_icon_svg: &str) -> Result<(), String> {
    let a = &crate::config().app;
    let home = PathBuf::from(std::env::var("HOME").map_err(|e| e.to_string())?);
    let app = home.join("Applications").join(format!("{}.app", a.title));
    let exe = app.join("Contents/MacOS").join(&a.name);
    let link = home.join(".local/bin").join(&a.name);
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    for dir in [exe.parent().unwrap(), link.parent().unwrap()] {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    if me != exe {
        let _ = std::fs::remove_file(&exe);
        std::fs::copy(&me, &exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    }
    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\n\
         <key>CFBundleName</key><string>{}</string>\n<key>CFBundleIdentifier</key><string>{}</string>\n\
         <key>CFBundleExecutable</key><string>{}</string>\n<key>CFBundlePackageType</key><string>APPL</string>\n\
         <key>NSHighResolutionCapable</key><true/>\n</dict></plist>\n",
        a.title, a.id, a.name
    );
    let info = app.join("Contents/Info.plist");
    std::fs::write(&info, plist).map_err(|e| format!("{}: {e}", info.display()))?;
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&exe, &link).map_err(|e| format!("{}: {e}", link.display()))?;
    println!("Installiert: {}", app.display());
    println!("Terminal:    {}", link.display());
    Ok(())
}
