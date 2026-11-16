//! Aussehen laden: Theme (Omarchy-Farben, `theme.rs`), Icons und CSS in drei Schichten – Kern
//! (`style.css` hier), App (ihr `style.css`), Nutzer (`~/.config/<app>/style.css`, ohne Neubauen).
//! Jede Schicht hat eine eigene Priorität, die spätere gewinnt also immer, egal wie genau die Regel ist
//! (GTK entscheidet bei gleicher Priorität nach Ladereihenfolge, nicht nach Spezifität).

use gtk4 as gtk;

/// Alles einmal beim Fensterbau aufrufen. `app_css`: CSS der App (`include_str!("style.css")` oder "").
pub fn install(display: &gtk::gdk::Display, app_css: &str) {
    let base = gtk::STYLE_PROVIDER_PRIORITY_APPLICATION;
    let user = std::fs::read_to_string(crate::app::config_file("style.css")).unwrap_or_default();
    // Theme liegt auf `APPLICATION`, die CSS-Schichten darüber.
    for (i, css) in [crate::CSS, app_css, user.as_str()].into_iter().enumerate() {
        if css.trim().is_empty() {
            continue;
        }
        let p = gtk::CssProvider::new();
        p.load_from_string(css);
        gtk::style_context_add_provider_for_display(display, &p, base + 1 + i as u32);
    }
    crate::theme::install(display);
    crate::icons::install(display);
}
