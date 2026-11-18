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

/// Feste Ordner statt XDG/HOME (Android: `filesDir`, `cacheDir`): Einstellungen und Daten unter `files/<app>/`,
/// Cache unter `cache/<app>/`. Vor dem ersten Zugriff aufrufen.
pub fn set_dirs(files: PathBuf, cache: PathBuf) {
    let _ = DIRS.set((files, cache));
}

static DIRS: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();

fn base(var: &str, fallback: &str) -> PathBuf {
    match DIRS.get() {
        Some((files, cache)) => if var == "XDG_CACHE_HOME" { cache.clone() } else { files.clone() },
        None => xdg(var, fallback),
    }
}

// Windows hat weder XDG noch HOME: Einstellungen und Daten nach %APPDATA%\<app>, Cache nach %LOCALAPPDATA%\<app>.
#[cfg(windows)]
fn xdg(var: &str, _: &str) -> PathBuf {
    let win = if var == "XDG_CACHE_HOME" { "LOCALAPPDATA" } else { "APPDATA" };
    PathBuf::from(std::env::var(win).unwrap_or_default())
}

#[cfg(not(windows))]
fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var(var)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(fallback))
}

/// Datei in `~/.config/<app>/` (Einstellungen).
pub fn config_file(file: &str) -> PathBuf {
    base("XDG_CONFIG_HOME", ".config").join(name()).join(file)
}

/// Datei in `~/.local/share/<app>/` (Fortschritt, Listen).
pub fn data_file(file: &str) -> PathBuf {
    base("XDG_DATA_HOME", ".local/share").join(name()).join(file)
}

/// `~/.cache/<app>/` (Poster, Icons, Shader).
pub fn cache_dir() -> PathBuf {
    base("XDG_CACHE_HOME", ".cache").join(name())
}
