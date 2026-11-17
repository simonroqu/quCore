//! Bausteine fürs Karten-Raster (`[grid]` in qu.toml): Spaltenzahl zur Breite, Strg+Mausrad-Spalten,
//! Bildhöhe zum Format (Poster 2:3, Thumbnail 16:9), Beschriftungen und Lade-Platzhalter (`.skeleton`).

use gtk::prelude::*;
use gtk4 as gtk;

/// Abstand zwischen den Karten.
pub const COLUMN_SPACING: i32 = 16;
/// Abstand oben bis zur ersten Karte (unter der schwebenden Navigation).
pub const TOP_MARGIN: i32 = 56;

fn cfg() -> &'static crate::config::Grid {
    &crate::config().grid
}

/// Spalten für eine Breite: ohne Wahl so viele Karten (`card_width`) wie passen, höchstens `max_columns`;
/// mit Strg+Mausrad gewählt (`want`) diese Zahl, solange Karten ≥ `min_card_width` passen.
pub fn columns(width: i32, want: Option<usize>) -> usize {
    let fit = |card: i32| ((width + COLUMN_SPACING) / (card + COLUMN_SPACING + 8)).max(1) as usize;
    match want {
        Some(n) => n.min(fit(cfg().min_card_width)),
        None => fit(cfg().card_width).min(cfg().max_columns),
    }
}

/// Eine Spalte mehr (`dy` < 0, Mausrad hoch) oder weniger, im Bereich `zoom_columns`.
pub fn step_columns(current: usize, dy: f64) -> usize {
    let n = if dy < 0.0 { current + 1 } else { current.saturating_sub(1) };
    let [lo, hi] = cfg().zoom_columns;
    n.clamp(lo, hi)
}

/// Bildhöhe zur Kartenbreite im Format aus `image` („2:3“, „16:9“ …).
pub fn image_height(width: i32) -> i32 {
    let (w, h) = cfg().ratio();
    width * h / w
}

pub fn label(text: &str, classes: &[&str]) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .lines(2)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(1) // Breite kommt von der Karte, nicht vom Text
        .hexpand(true)
        .css_classes(classes.to_vec())
        .build()
}

/// Grauer Balken über `k` von `n` gleich breiten Teilen der Zeile – ohne feste Pixelbreite,
/// die dem Fenster eine Mindestbreite aufzwänge.
pub fn part(k: i32, n: i32, height: i32) -> gtk::Grid {
    let g = gtk::Grid::builder().column_homogeneous(true).build();
    g.attach(&gtk::Box::builder().height_request(height).css_classes(["skeleton"]).build(), 0, 0, k, 1);
    // Jede freie Spalte braucht ein Kind, sonst zählt sie bei gleich breiten Spalten nicht mit.
    for i in k..n {
        g.attach(&gtk::Box::new(gtk::Orientation::Horizontal, 0), i, 0, 1, 1);
    }
    g
}

/// Lade-Platzhalter in Form einer Karte: Bild, Titel, `info_lines` Infozeilen. Gibt auch den Bild-Platzhalter
/// zurück: seine Höhe folgt wie bei echten Karten der Spaltenbreite.
pub fn ghost_card(width: i32) -> (gtk::Box, gtk::Box) {
    let b = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(8).build();
    let image = gtk::Box::builder().height_request(image_height(width)).css_classes(["skeleton"]).build();
    b.append(&image);
    b.append(&part(4, 5, 16));
    for i in 0..cfg().info_lines {
        b.append(&part(1, if i % 2 == 0 { 2 } else { 3 }, 12));
    }
    (b, image)
}
