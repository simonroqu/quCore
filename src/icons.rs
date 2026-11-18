//! Eigener minimalistischer Icon-Satz für alle Qu-Apps (data/icons, Namen `qu-…-symbolic`). Die SVGs
//! stecken im Programm und werden beim Start in den Cache geschrieben und GTK bekannt gemacht. Endung
//! `-symbolic`: GTK färbt sie in der aktuellen Schriftfarbe ein, sie passen sich also dem Theme an.

use gtk4 as gtk;

const ICONS: [(&str, &str); 24] = [
    ("qu-account-symbolic", include_str!("../data/icons/qu-account-symbolic.svg")),
    ("qu-back-symbolic", include_str!("../data/icons/qu-back-symbolic.svg")),
    ("qu-close-symbolic", include_str!("../data/icons/qu-close-symbolic.svg")),
    ("qu-fullscreen-symbolic", include_str!("../data/icons/qu-fullscreen-symbolic.svg")),
    ("qu-history-symbolic", include_str!("../data/icons/qu-history-symbolic.svg")),
    ("qu-home-symbolic", include_str!("../data/icons/qu-home-symbolic.svg")),
    ("qu-later-symbolic", include_str!("../data/icons/qu-later-symbolic.svg")),
    ("qu-menu-symbolic", include_str!("../data/icons/qu-menu-symbolic.svg")),
    ("qu-mute-symbolic", include_str!("../data/icons/qu-mute-symbolic.svg")),
    ("qu-next-symbolic", include_str!("../data/icons/qu-next-symbolic.svg")),
    ("qu-pause-symbolic", include_str!("../data/icons/qu-pause-symbolic.svg")),
    ("qu-play-symbolic", include_str!("../data/icons/qu-play-symbolic.svg")),
    ("qu-reload-symbolic", include_str!("../data/icons/qu-reload-symbolic.svg")),
    ("qu-restore-symbolic", include_str!("../data/icons/qu-restore-symbolic.svg")),
    ("qu-search-symbolic", include_str!("../data/icons/qu-search-symbolic.svg")),
    ("qu-settings-symbolic", include_str!("../data/icons/qu-settings-symbolic.svg")),
    ("qu-spinner-symbolic", include_str!("../data/icons/qu-spinner-symbolic.svg")),
    ("qu-subscribed-symbolic", include_str!("../data/icons/qu-subscribed-symbolic.svg")),
    ("qu-subscribe-symbolic", include_str!("../data/icons/qu-subscribe-symbolic.svg")),
    ("qu-subtitles-symbolic", include_str!("../data/icons/qu-subtitles-symbolic.svg")),
    ("qu-tv-symbolic", include_str!("../data/icons/qu-tv-symbolic.svg")),
    ("qu-volume-symbolic", include_str!("../data/icons/qu-volume-symbolic.svg")),
    ("qu-watchlisted-symbolic", include_str!("../data/icons/qu-watchlisted-symbolic.svg")),
    ("qu-watchlist-symbolic", include_str!("../data/icons/qu-watchlist-symbolic.svg")),
];

/// Lade-Kreis: eigenes Icon, per CSS gedreht (`.qu-spinner` im gemeinsamen CSS), sichtbar = läuft.
/// Statt `gtk::Spinner`: der friert in GTK 4.24 ein, sobald er einmal ausgeblendet war
/// (auch durch Seitenwechsel), und dreht sich nach dem Wiedereinblenden nicht mehr.
pub fn spinner(size: i32) -> gtk::builders::ImageBuilder {
    gtk::Image::builder().icon_name("qu-spinner-symbolic").pixel_size(size).css_classes(["qu-spinner"])
}

pub fn install(display: &gtk::gdk::Display) {
    // Lose Dateien direkt im Suchpfad findet GTK ohne Theme-Struktur.
    let dir = crate::app::cache_dir().join("icons");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    for (name, svg) in ICONS {
        let _ = std::fs::write(dir.join(format!("{name}.svg")), svg);
    }
    gtk::IconTheme::for_display(display).add_search_path(dir);
}
