//! App-Konfiguration `qu.toml` (im Repo der App, beim Bauen eingebettet: `qu_core::start(include_str!("../qu.toml"), …)`).
//! Beschreibt, was die App ausmacht, ohne Code: Name, Menü oben links, Suche.

use serde::Deserialize;
use std::sync::OnceLock;

/// Eingebaute Menüeinträge (alles andere muss als `[[nav.item]]` definiert sein).
pub const BUILTIN: [&str; 5] = ["back", "home", "search", "busy", "menu"];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub app: App,
    pub nav: Nav,
    pub search: Search,
    #[serde(default)]
    pub player: Player,
    #[serde(default)]
    pub progress: Progress,
    #[serde(default)]
    pub grid: Grid,
}

/// Regeln für den Fortschritt (alles optional).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Progress {
    /// Kürzer geschaut (s) wird nicht gespeichert.
    pub min_watched: f64,
    /// Ab diesem Anteil gilt etwas als fertig geschaut.
    pub finished_at: f64,
    /// Beim Fortsetzen so viele Sekunden zurück.
    pub rewind: f64,
}

impl Default for Progress {
    fn default() -> Self {
        Progress { min_watched: 30.0, finished_at: 0.9, rewind: 3.0 }
    }
}

/// Karten-Raster (alles optional; Standard = Poster wie AniQu/BoxQu).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Grid {
    /// Kartenbreite, nach der sich die Spaltenzahl richtet (Karten wachsen bis zur vollen Reihe mit).
    pub card_width: i32,
    /// Mit Strg+Mausrad gewählte Spalten nur, solange Karten mindestens so breit bleiben.
    pub min_card_width: i32,
    /// Höchstens so viele Spalten (sonst riesige Bilder im Vollbild).
    pub max_columns: usize,
    /// Mit Strg+Mausrad wählbare Spaltenzahl (von, bis).
    pub zoom_columns: [usize; 2],
    /// Bildformat Breite:Höhe, z. B. „2:3“ (Poster) oder „16:9“ (Thumbnail).
    pub image: String,
    /// Infozeilen unter dem Titel im Lade-Platzhalter.
    pub info_lines: usize,
}

impl Default for Grid {
    fn default() -> Self {
        Grid { card_width: 170, min_card_width: 120, max_columns: 12, zoom_columns: [4, 12], image: "2:3".into(), info_lines: 1 }
    }
}

impl Grid {
    /// (Breite, Höhe) aus `image`.
    pub fn ratio(&self) -> (i32, i32) {
        self.image.split_once(':').and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?))).filter(|(w, h): &(i32, i32)| *w > 0 && *h > 0).unwrap_or((2, 3))
    }
}

/// Player-Einstellungen (alles optional, sonst Standardwerte).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Player {
    /// Wählbare Qualitäten (maximale Bildhöhe), z. B. `[2160, 1080, 720]`.
    pub qualities: Vec<u32>,
    /// Qualität, bis man eine andere wählt.
    pub default_quality: u32,
    /// Shader-Set für „AI“-Upscaling: `fsrcnnx` oder `anime4k`.
    pub upscaler: crate::upscale::Shaders,
    /// Abschnitte (Opening, Sponsor …): `button` = Knopf „Skip …“ über der Timeline, `auto` = überspringen.
    pub segments: String,
    /// Untertitel beim Laden wie zuletzt (an/aus, Sprache) setzen; `false` = immer aus, bis man sie einschaltet.
    pub auto_subtitles: bool,
}

impl Default for Player {
    fn default() -> Self {
        Player {
            qualities: vec![1080, 720, 480, 360],
            default_quality: 1080,
            upscaler: crate::upscale::Shaders::Fsrcnnx,
            segments: "button".into(),
            auto_subtitles: true,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    /// Kurzname: Ordner (`~/.config/<name>` …), Dev-Schalter `<NAME>_DEV`.
    pub name: String,
    /// Fenstertitel.
    pub title: String,
    /// Application-ID (Fensterklasse, Einzelinstanz), z. B. `dev.roqu.AniQu`.
    pub id: String,
    /// Startmenü-Eintrag (`<app> --install`): Untertitel, Beschreibung, Suchwörter.
    #[serde(default)]
    pub generic_name: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub keywords: Vec<String>,
}

/// Menü oben links: `top` = obere Reihe, `menu` = Spalte unter dem Burger (leer = kein Burger).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nav {
    pub top: Vec<String>,
    #[serde(default)]
    pub menu: Vec<String>,
    #[serde(default, rename = "item")]
    pub items: Vec<NavItem>,
}

/// Eigener Menüeintrag: ein Knopf (Icon + Tooltip, die App hängt den Klick an) oder ein `slot`
/// (leere Box, die App füllt sie selbst, z. B. Tubequs Kontozeile).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavItem {
    pub id: String,
    /// Icon-Name ohne `qu-`/`-symbolic`, z. B. `later` → `qu-later-symbolic`.
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub tooltip: String,
    #[serde(default)]
    pub slot: bool,
    /// Slot klappt nach rechts auf (Nav-Bubble wird zum liegenden U, nur unten in `menu`).
    #[serde(default)]
    pub expands: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Search {
    /// Text im leeren Suchfeld.
    pub placeholder: String,
    /// `history` (gefilterte letzte Suchen) oder `app` (Vorschläge aus dem App-Code).
    #[serde(default = "history")]
    pub suggestions: String,
}

fn history() -> String {
    "history".into()
}

impl Config {
    /// Lesen und prüfen; Fehler sind Fehler der App (eingebettete Datei), daher als Text.
    pub fn parse(text: &str) -> Result<Config, String> {
        let c: Config = toml::from_str(text).map_err(|e| format!("qu.toml: {e}"))?;
        for id in c.nav.top.iter().chain(&c.nav.menu) {
            if !BUILTIN.contains(&id.as_str()) && !c.nav.items.iter().any(|i| &i.id == id) {
                return Err(format!("qu.toml: Menüeintrag „{id}“ ist weder eingebaut ({}) noch als [[nav.item]] definiert", BUILTIN.join(", ")));
            }
        }
        if c.nav.top.contains(&"menu".to_string()) && c.nav.menu.is_empty() {
            return Err("qu.toml: „menu“ (Burger) in nav.top, aber nav.menu ist leer".into());
        }
        if !c.player.qualities.contains(&c.player.default_quality) {
            return Err(format!("qu.toml: player.default_quality {} steht nicht in player.qualities", c.player.default_quality));
        }
        let (lo, hi) = (c.grid.zoom_columns[0], c.grid.zoom_columns[1]);
        if lo == 0 || lo > hi {
            return Err(format!("qu.toml: grid.zoom_columns = [{lo}, {hi}] ist kein gültiger Bereich"));
        }
        if c.grid.image.split_once(':').is_none_or(|(w, h)| w.trim().parse::<u32>().is_err() || h.trim().parse::<u32>().is_err()) {
            return Err(format!("qu.toml: grid.image = „{}“, erwartet z. B. „2:3“ oder „16:9“", c.grid.image));
        }
        if !["button", "auto"].contains(&c.player.segments.as_str()) {
            return Err(format!("qu.toml: player.segments = „{}“, erlaubt: button, auto", c.player.segments));
        }
        if !["history", "app"].contains(&c.search.suggestions.as_str()) {
            return Err(format!("qu.toml: search.suggestions = „{}“, erlaubt: history, app", c.search.suggestions));
        }
        Ok(c)
    }

    pub fn item(&self, id: &str) -> Option<&NavItem> {
        self.nav.items.iter().find(|i| i.id == id)
    }
}

/// Eigener Eintrag zu einer ID (nach `parse` gibt es ihn sicher).
pub(crate) fn get_item<'a>(nav: &'a Nav, id: &str) -> &'a NavItem {
    nav.items.iter().find(|i| i.id == id).unwrap_or_else(|| panic!("qu.toml: [[nav.item]] „{id}“ fehlt"))
}

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Die Konfiguration der laufenden App (gesetzt von `qu_core::start`).
pub fn get() -> &'static Config {
    CONFIG.get().expect("qu_core::start wurde nicht aufgerufen")
}

/// Konfig ohne `start` laden (für Tests der App: `qu_core::config::load(include_str!("../qu.toml"))`).
/// Ist schon eine gesetzt, bleibt sie.
pub fn load(text: &str) {
    if CONFIG.get().is_none() {
        set(Config::parse(text).unwrap_or_else(|e| panic!("{e}")));
    }
}

pub(crate) fn set(c: Config) -> &'static Config {
    let _ = CONFIG.set(c);
    get()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: &str = r#"
        [app]
        name = "demo"
        title = "Demo"
        id = "dev.example.Demo"
        [nav]
        top = ["menu", "back", "home", "search", "busy"]
        menu = ["later", "account"]
        [[nav.item]]
        id = "later"
        icon = "later"
        tooltip = "Watch later"
        [[nav.item]]
        id = "account"
        slot = true
        expands = true
        [search]
        placeholder = "Search demo"
    "#;

    #[test]
    fn parses_and_checks() {
        let c = Config::parse(OK).unwrap();
        assert_eq!(c.search.suggestions, "history");
        assert!(c.item("account").unwrap().expands);
        assert!(Config::parse(&OK.replace(r#"menu = ["later", "account"]"#, r#"menu = ["later", "nope"]"#)).unwrap_err().contains("nope"));
        assert!(Config::parse(&OK.replace(r#"menu = ["later", "account"]"#, "")).unwrap_err().contains("Burger"));
        assert!(Config::parse(&OK.replace("title", "titel")).is_err(), "Tippfehler fallen auf");
        assert_eq!(c.player.default_quality, 1080, "ohne [player]: Standardwerte");
        let p = Config::parse(&format!("{OK}\n[player]\nqualities = [720, 480]\ndefault_quality = 720\nupscaler = \"anime4k\"\n")).unwrap();
        assert_eq!(p.player.upscaler, crate::upscale::Shaders::Anime4k);
        assert!(Config::parse(&format!("{OK}\n[player]\nqualities = [720]\ndefault_quality = 1080\n")).is_err());
        let g = Config::parse(&format!("{OK}\n[grid]\ncard_width = 320\nimage = \"16:9\"\nzoom_columns = [3, 6]\n")).unwrap();
        assert_eq!((g.grid.ratio(), g.grid.card_width, g.grid.max_columns), ((16, 9), 320, 12));
        assert!(Config::parse(&format!("{OK}\n[grid]\nimage = \"breit\"\n")).is_err());
        assert!(Config::parse(&format!("{OK}\n[grid]\nzoom_columns = [6, 3]\n")).is_err());
        assert_eq!(c.progress.finished_at, 0.9);
    }
}
