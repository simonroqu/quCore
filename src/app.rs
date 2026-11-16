//! Name der App und ihre Ordner (`~/.config/<name>`, `~/.local/share/<name>`, `~/.cache/<name>`).

use std::path::PathBuf;
use std::sync::OnceLock;

static NAME: OnceLock<&'static str> = OnceLock::new();

/// Einmal ganz am Anfang von `main` aufrufen, z. B. `qu_core::app::init("aniqu")`.
pub fn init(name: &'static str) {
    let _ = NAME.set(name);
}

/// Name aus `init`; ohne (z. B. in Tests) der Name des Programms.
pub fn name() -> &'static str {
    NAME.get_or_init(|| {
        let exe = std::env::current_exe().ok().and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()));
        Box::leak(exe.unwrap_or_else(|| "qu".into()).into_boxed_str())
    })
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var(var)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(fallback))
}

/// Datei in `~/.config/<app>/` (Einstellungen).
pub fn config_file(file: &str) -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join(name()).join(file)
}

/// Datei in `~/.local/share/<app>/` (Fortschritt, Listen).
pub fn data_file(file: &str) -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join(name()).join(file)
}

/// `~/.cache/<app>/` (Poster, Icons, Shader).
pub fn cache_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache").join(name())
}
