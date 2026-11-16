//! Omarchy-Theme: Farben aus `colors.toml`, Abrundung aus `hyprland.lua` (falls das Theme eine
//! vorgibt) als CSS. Ohne Omarchy gilt ein eingebautes Standard-Theme mit denselben Farben wie
//! „Last Horizon“. Wechselt man das Theme (`omarchy theme set …`), lädt die App automatisch neu.

use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

fn current_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/state/omarchy/current")
}

/// Standard-Theme ohne Omarchy (oder bei kaputter Datei): die Farben von „Last Horizon“.
const DEFAULT_COLORS: &str = r##"
mode = "dark"
accent = "#b59790"
background = "#0c0b0c"
darker_background = "#060606"
foreground = "#FAFCFB"
red = "#c38b7b"
"##;

/// Abrundung aus der `hyprland.lua` des Themes (`decoration = { rounding = 6 }`).
/// Die meisten Themes geben keine vor; dann `None` und die App bleibt bei ihren eigenen Rundungen.
fn rounding(hyprland_lua: &str) -> Option<u32> {
    hyprland_lua.lines().find_map(|l| {
        let rest = l.trim().strip_prefix("rounding")?.trim_start();
        let num: String = rest.strip_prefix('=')?.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
        num.parse().ok()
    })
}

/// Rundungs-Variablen für style.css. Ohne Vorgabe die bisherigen Werte; mit Vorgabe `r` davon
/// abgeleitet (klein = r, mittel = 1,5·r, groß = 2,25·r, Bubbles = 2·r statt Pillenform).
fn radius_vars(r: Option<u32>) -> String {
    let (s, m, l, pill) = match r {
        None => (8, 12, 18, 9999),
        Some(r) => (r, (r * 3).div_ceil(2), (r * 9).div_ceil(4), r * 2),
    };
    format!("window, popover {{ --radius-s: {s}px; --radius-m: {m}px; --radius-l: {l}px; --radius-pill: {pill}px; }}\n")
}

/// Tatsächliche Fenster-Abrundung von Hyprland (eigene Config + Theme), z. B. für den Player,
/// damit seine Ecken zu den Fensterecken passen. Ohne Hyprland `None`.
// ponytail: kurzer hyprctl-Aufruf im GTK-Thread (lokal, wenige ms), nur beim Start und Theme-Wechsel.
fn hyprland_rounding() -> Option<u32> {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    let out = std::process::Command::new("hyprctl").args(["getoption", "decoration:rounding", "-j"]).output().ok()?;
    let j: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    j.get("int")?.as_u64().map(|r| r as u32)
}

/// `--radius-window`: so rund wie die Fensterecken. Reihenfolge: Hyprland, Theme-Vorgabe, 8 px.
fn window_radius_var(hyprland: Option<u32>, theme: Option<u32>) -> String {
    format!("window, popover {{ --radius-window: {}px; }}\n", hyprland.or(theme).unwrap_or(8))
}

/// Liest `key = "wert"`-Zeilen. Mehr braucht colors.toml nicht.
fn parse(s: &str) -> HashMap<String, String> {
    s.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            let v = v.trim().strip_prefix('"')?.split('"').next()?;
            Some((k.trim().to_string(), v.to_string()))
        })
        .collect()
}

/// Nur echte Hex-Farben durchlassen, damit eine kaputte Datei kein kaputtes CSS erzeugt.
fn hex(c: &HashMap<String, String>, key: &str) -> Option<String> {
    let v = c.get(key)?;
    let digits = v.strip_prefix('#')?;
    (matches!(digits.len(), 6 | 8) && digits.chars().all(|ch| ch.is_ascii_hexdigit())).then(|| v.clone())
}

/// CSS aus den Theme-Farben. `None`, wenn Grundfarben fehlen.
fn css(c: &HashMap<String, String>) -> Option<String> {
    let bg = hex(c, "background")?;
    let fg = hex(c, "foreground")?;
    let accent = hex(c, "accent")?;
    let dark = hex(c, "darker_background").unwrap_or_else(|| bg.clone());
    let red = hex(c, "red").unwrap_or_else(|| "#e06c75".into());
    Some(format!(
        r#"
@define-color theme_bg_color {bg};
@define-color theme_fg_color {fg};
@define-color theme_text_color {fg};
@define-color theme_base_color {bg};
@define-color theme_selected_bg_color {accent};
@define-color theme_selected_fg_color {bg};
@define-color accent_bg_color {accent};
@define-color accent_color {accent};
@define-color error_color {red};
@define-color borders alpha({fg}, 0.12);

window, .background {{ background-color: {bg}; color: {fg}; }}
headerbar {{ background: mix({bg}, {fg}, 0.05); color: {fg}; box-shadow: none; border-bottom: 1px solid alpha({fg}, 0.08); }}
entry {{ background: alpha({fg}, 0.07); color: {fg}; }}
entry:focus-within {{ outline-color: alpha({accent}, 0.7); }}
button {{ color: {fg}; }}
button:checked {{ color: {accent}; box-shadow: inset 0 -2px {accent}; }}
scale trough {{ background: alpha({fg}, 0.2); }}
scale highlight {{ background: {accent}; border: none; margin: 0; }}
scale slider {{ background: {fg}; }}
.dim-label {{ color: alpha({fg}, 0.65); }}
.osd {{ background-color: alpha({dark}, 0.85); color: {fg}; }}
.error {{ color: {red}; }}
flowboxchild {{ border-radius: var(--radius-m); }}
flowboxchild:hover {{ background: alpha({fg}, 0.06); }}
scrollbar slider {{ background: alpha({fg}, 0.3); }}
selection {{ background: alpha({accent}, 0.4); }}
progressbar.watched progress {{ background: {red}; }}
label link, label link:visited {{ color: {accent}; }}
popover > contents, popover > arrow {{ background: alpha({dark}, 0.95); color: {fg}; border: 1px solid alpha({fg}, 0.12); }}
popover > contents {{ padding: 8px; border-radius: var(--radius-l); box-shadow: 0 10px 30px rgba(0, 0, 0, 0.45); }}
popover.background {{ background: none; }}
popover label {{ color: {fg}; }}
checkbutton {{ padding: 2px 6px; border-radius: var(--radius-s); }}
checkbutton:hover {{ background: alpha({fg}, 0.08); }}
checkbutton radio {{ background: none; border: 1px solid alpha({fg}, 0.35); box-shadow: none; color: {bg}; }}
checkbutton radio:checked {{ background: {accent}; border-color: {accent}; }}
"#
    ))
}

/// Bubble-Rundung in px (wie `--radius-pill`), für die selbst gezeichnete Nav-Bubble.
static PILL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(9999);

pub fn pill() -> f32 {
    PILL.load(std::sync::atomic::Ordering::Relaxed) as f32
}

fn apply(provider: &gtk::CssProvider) {
    let dir = current_dir().join("theme");
    let text = std::fs::read_to_string(dir.join("colors.toml")).unwrap_or_default();
    // Ohne Omarchy (oder bei kaputter Datei): eingebautes Standard-Theme.
    let (colors, theme_css) = match css(&parse(&text)) {
        Some(c) => (parse(&text), c),
        None => (parse(DEFAULT_COLORS), css(&parse(DEFAULT_COLORS)).expect("Standard-Theme ist gültig")),
    };
    let radius = rounding(&std::fs::read_to_string(dir.join("hyprland.lua")).unwrap_or_default());
    PILL.store(radius.map_or(9999, |r| r * 2), std::sync::atomic::Ordering::Relaxed);
    provider.load_from_string(&(radius_vars(radius) + &window_radius_var(hyprland_rounding(), radius) + &theme_css));
    if let Some(s) = gtk::Settings::default() {
        let dark = colors.get("mode").is_none_or(|m| m == "dark");
        s.set_gtk_interface_color_scheme(if dark { gtk::InterfaceColorScheme::Dark } else { gtk::InterfaceColorScheme::Light });
    }
}

/// Theme anwenden und bei Theme-Wechsel neu laden.
pub fn install(display: &gtk::gdk::Display) {
    let provider = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    apply(&provider);

    // omarchy theme set ersetzt das Verzeichnis current/theme und schreibt current/theme.name.
    let Ok(monitor) = gio::File::for_path(current_dir()).monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
    else {
        return;
    };
    // Ein Theme-Wechsel erzeugt viele Ereignisse kurz hintereinander: einmal danach neu laden.
    let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
    monitor.connect_changed(move |_, _, _, _| {
        if let Some(id) = pending.take() {
            id.remove();
        }
        let (provider, pending2) = (provider.clone(), pending.clone());
        let id = glib::timeout_add_local_once(Duration::from_millis(300), move || {
            pending2.take();
            apply(&provider);
        });
        pending.replace(Some(id));
    });
    // ponytail: Monitor lebt so lange wie die App, daher bewusst geleakt.
    Box::leak(Box::new(monitor));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_validates() {
        let c = parse("mode = \"dark\"\naccent = \"#b59790\"\nbackground = \"#0c0b0c\"\nforeground = \"#FAFCFB\"\nbad = \"red; }\"\n# kommentar");
        assert_eq!(c["accent"], "#b59790");
        assert_eq!(hex(&c, "bad"), None);
        assert!(css(&c).unwrap().contains("scale highlight { background: #b59790;"));
        assert!(css(&parse("accent = \"#fff\"")).is_none(), "ohne Grundfarben kein CSS");
        assert!(css(&parse(DEFAULT_COLORS)).is_some(), "Standard-Theme muss gültig sein");
    }

    #[test]
    fn rounding_from_theme() {
        let lua = "hl.config({\n  decoration = {\n    rounding = 6,\n    rounding_power = 3,\n  },\n})";
        assert_eq!(rounding(lua), Some(6));
        assert_eq!(rounding("general = { gaps_in = 5 }"), None);
        assert!(radius_vars(None).contains("--radius-pill: 9999px"));
        assert!(radius_vars(Some(6)).contains("--radius-s: 6px; --radius-m: 9px; --radius-l: 14px; --radius-pill: 12px"));
        assert!(radius_vars(Some(0)).contains("--radius-pill: 0px"));
        assert!(window_radius_var(Some(4), Some(6)).contains("--radius-window: 4px"));
        assert!(window_radius_var(None, Some(6)).contains("--radius-window: 6px"));
        assert!(window_radius_var(None, None).contains("--radius-window: 8px"));
    }
}
