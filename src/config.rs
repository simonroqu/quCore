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
    }
}
