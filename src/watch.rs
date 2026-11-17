//! Rahmen der Ansehen-Seite für alle Qu-Apps: Player genau so hoch wie das Video (Seitenverhältnis), mittig mit
//! Rand, Breite per Strg+Mausrad (gemerkt), Vollbild, Miniplayer in einer Ecke (ziehbar, main.rs). Darunter
//! `below`: das füllt die App (Titel, Episode, Kanal, Beschreibung …); im Vollbild und als Miniplayer ausgeblendet.

use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::Cell;
use std::rc::Rc;

/// Player-Höhe: volle Breite im Seitenverhältnis, höchstens so hoch wie die Seite.
/// Im Vollbild immer die ganze Höhe.
fn player_height(width: i32, page_height: i32, aspect: f64, fullscreen: bool) -> i32 {
    if fullscreen || aspect <= 0.0 {
        return page_height;
    }
    ((width as f64 / aspect).round() as i32).min(page_height)
}

/// Rand um den Player (Hintergrundfarbe), im Vollbild keiner.
const PLAYER_MARGIN: i32 = 5;
/// Anteil der Fensterbreite, den der Player höchstens einnimmt; links/rechts bleibt Hintergrund.
const PLAYER_WIDTH: f64 = 0.85;
/// Unter dieser Fensterbreite nimmt der Player die volle Breite ein.
const FULL_WIDTH_BELOW: i32 = 1600;
/// Miniplayer unten rechts (Seite verlassen, Video läuft weiter): Breite und Abstand zum Fensterrand.
const MINI_WIDTH: i32 = 400;
const MINI_MARGIN: i32 = 16;

/// Kleinste Player-Breite, die man mit Strg+Mausrad einstellen kann.
const MIN_PLAYER_WIDTH: i32 = 640;
/// So viele Pixel breiter/schmaler pro Mausrad-Raste.
const ZOOM_STEP: f64 = 80.0;

/// Seitenrand des Players. `want` = mit Strg+Mausrad gewählte Player-Breite: Sie bleibt fest, ein
/// schmaleres Fenster nimmt nur den Rand weg; reicht das Fenster nicht, randlos über die ganze Breite.
/// Ohne Wahl: mittig `PLAYER_WIDTH` der Breite, unter `FULL_WIDTH_BELOW` volle Breite. Im Vollbild kein Rand.
fn player_side(width: i32, fullscreen: bool, want: Option<i32>) -> i32 {
    if fullscreen {
        return 0;
    }
    match want {
        Some(p) if p >= width => 0,
        Some(p) => PLAYER_MARGIN.max((width - p) / 2),
        // Schmales Fenster: volle Breite (nur der kleine Rand).
        None if width < FULL_WIDTH_BELOW => PLAYER_MARGIN,
        None => PLAYER_MARGIN.max((width as f64 * (1.0 - PLAYER_WIDTH) / 2.0).round() as i32),
    }
}

/// Neue Wunschbreite nach `dy` Mausrad-Rasten (hoch = negativ = breiter). Ab Fensterbreite = randlos.
fn zoom(current: i32, dy: f64, window: i32) -> i32 {
    ((current as f64 - dy * ZOOM_STEP).round() as i32).clamp(MIN_PLAYER_WIDTH, window.max(MIN_PLAYER_WIDTH))
}

fn saved_width() -> Option<i32> {
    std::fs::read_to_string(crate::app::config_file("player-width")).ok()?.trim().parse().ok()
}

fn save_width(w: i32) {
    let f = crate::app::config_file("player-width");
    let _ = std::fs::create_dir_all(f.parent().unwrap());
    let _ = std::fs::write(f, w.to_string());
}

pub struct WatchFrame {
    pub widget: gtk::Overlay,
    /// Alles unter dem Video (füllt die App), bündig zum Player; im Vollbild und als Miniplayer ausgeblendet.
    pub below: gtk::Box,
    scroll: gtk::ScrolledWindow,
    player: gtk::Widget,
    /// Seitengröße, Seitenverhältnis des Videos und Vollbild: daraus folgt die Player-Höhe.
    size: Cell<(i32, i32)>,
    aspect: Cell<f64>,
    fullscreen: Cell<bool>,
    /// Mit Strg+Mausrad gewählte Player-Breite (bleibt über Neustarts erhalten).
    want: Cell<Option<i32>>,
    /// Miniplayer: nur das Video, klein unten rechts über der aktuellen Seite.
    mini: Cell<bool>,
    /// Ecke des Miniplayers (rechts?, unten?); per Ziehen wählbar, gilt für die Sitzung.
    corner: Cell<(bool, bool)>,
}

impl WatchFrame {
    /// `player`: das Player-Widget (wird nie umgehängt, sonst baut mpv seine Video-Fläche neu auf).
    pub fn new(player: &impl IsA<gtk::Widget>) -> Rc<Self> {
        let below = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(12)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(player);
        content.append(&below);
        let scroll = gtk::ScrolledWindow::builder().child(&content).hscrollbar_policy(gtk::PolicyType::Never).build();
        crate::smooth::attach(&scroll);

        // Unsichtbare Messfläche über der ganzen Seite: liefert Breite und Höhe für die Player-Höhe.
        let sizer = gtk::DrawingArea::builder().can_target(false).build();
        // Liegt als Ebene über den Seiten (main.rs), daher eigener Hintergrund (außer als Miniplayer).
        let widget = gtk::Overlay::builder().child(&scroll).css_classes(["background"]).visible(false).build();
        widget.add_overlay(&sizer);

        let page = Rc::new(WatchFrame {
            widget,
            below,
            scroll,
            player: player.clone().upcast(),
            size: Cell::new((0, 0)),
            aspect: Cell::new(16.0 / 9.0),
            fullscreen: Cell::new(false),
            want: Cell::new(saved_width()),
            mini: Cell::new(false),
            corner: Cell::new((true, true)),
        });

        sizer.connect_resize({
            let page = Rc::downgrade(&page);
            move |_, w, h| {
                if let Some(p) = page.upgrade() {
                    p.size.set((w, h));
                    // Nicht mitten in der Größenberechnung umbauen, sondern direkt danach.
                    glib::idle_add_local_once(move || p.fit_player());
                }
            }
        });
        // Strg+Mausrad: Player-Breite einstellen (vor dem Scrollen der Seite).
        let zoom_ctl = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        zoom_ctl.set_propagation_phase(gtk::PropagationPhase::Capture);
        zoom_ctl.connect_scroll({
            let page = Rc::downgrade(&page);
            move |c, _, dy| {
                let Some(p) = page.upgrade() else { return glib::Propagation::Proceed };
                if !c.current_event_state().contains(gdk::ModifierType::CONTROL_MASK) || p.mini.get() || p.fullscreen.get() {
                    return glib::Propagation::Proceed;
                }
                let w = p.size.get().0;
                // Touchpad liefert Pixel statt Rasten.
                let dy = if c.unit() == gdk::ScrollUnit::Wheel { dy } else { dy / ZOOM_STEP };
                let new = zoom(w - 2 * player_side(w, false, p.want.get()), dy, w);
                p.want.set(Some(new));
                save_width(new);
                p.fit_player();
                glib::Propagation::Stop
            }
        });
        page.widget.add_controller(zoom_ctl);
        page
    }

    fn fit_player(&self) {
        if self.mini.get() {
            let a = if self.aspect.get() > 0.0 { self.aspect.get() } else { 16.0 / 9.0 };
            let h = (MINI_WIDTH as f64 / a).round() as i32;
            self.player.set_margin_top(0);
            self.player.set_margin_start(0);
            self.player.set_margin_bottom(0);
            self.player.set_margin_end(0);
            self.player.add_css_class("rounded");
            self.player.set_size_request(-1, h);
            self.widget.set_size_request(MINI_WIDTH, h);
            return;
        }
        self.widget.set_size_request(-1, -1);
        let (w, h) = self.size.get();
        let fs = self.fullscreen.get();
        let side = player_side(w, fs, self.want.get());
        // Randlos (Vollbild oder ganz breit gezoomt): auch oben/unten kein Rand, eckig.
        let edge = side == 0;
        let m = if edge { 0 } else { PLAYER_MARGIN };
        self.player.set_margin_top(m);
        self.player.set_margin_bottom(m);
        self.player.set_margin_start(side);
        self.player.set_margin_end(side);
        // Text darunter bündig zum Player, 16 px eingerückt (bzw. 11 px mehr als der Player).
        self.below.set_margin_start(side.max(PLAYER_MARGIN) - PLAYER_MARGIN + 16);
        self.below.set_margin_end(side.max(PLAYER_MARGIN) - PLAYER_MARGIN + 16);
        // Abgerundete Ecken wie die Fensterecken (`--radius-window`), im Vollbild eckig.
        if edge { self.player.remove_css_class("rounded") } else { self.player.add_css_class("rounded") }
        if w > 0 && h > 0 {
            self.player.set_size_request(-1, player_height(w - 2 * side, h - 2 * m, self.aspect.get(), fs));
        }
    }

    /// Seitenverhältnis des laufenden Videos (vom Player gemeldet).
    pub fn set_aspect(&self, aspect: f64) {
        self.aspect.set(aspect);
        self.fit_player();
    }

    pub fn is_mini(&self) -> bool {
        self.mini.get()
    }

    /// Miniplayer an/aus: dieselbe Seite samt Player, nur klein unten rechts und ohne Infos
    /// (der Player wird nicht umgehängt, sonst müsste mpv seine Video-Fläche neu aufbauen).
    pub fn set_mini(&self, on: bool) {
        self.mini.set(on);
        self.place_mini();
        if on {
            self.widget.remove_css_class("background");
            self.widget.add_css_class("mini");
            self.scroll.vadjustment().set_value(0.0);
        } else {
            self.widget.add_css_class("background");
            self.widget.remove_css_class("mini");
        }
        let v = if on { gtk::PolicyType::Never } else { gtk::PolicyType::Automatic };
        self.scroll.set_policy(gtk::PolicyType::Never, v);
        self.below.set_visible(!on && !self.fullscreen.get());
        self.fit_player();
    }

    /// Miniplayer in die Ecke `right`/`bottom` legen (nach dem Ziehen).
    pub fn set_corner(&self, right: bool, bottom: bool) {
        self.corner.set((right, bottom));
        self.place_mini();
    }

    /// Ausrichtung/Ränder: im Miniplayer in die gewählte Ecke, sonst ganze Fläche.
    fn place_mini(&self) {
        let on = self.mini.get();
        let (right, bottom) = self.corner.get();
        let side = |end: bool| if !on { gtk::Align::Fill } else if end { gtk::Align::End } else { gtk::Align::Start };
        self.widget.set_halign(side(right));
        self.widget.set_valign(side(bottom));
        let m = if on { MINI_MARGIN } else { 0 };
        self.widget.set_margin_start(m);
        self.widget.set_margin_end(m);
        self.widget.set_margin_top(m);
        self.widget.set_margin_bottom(m);
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.fullscreen.set(fullscreen);
        self.below.set_visible(!fullscreen && !self.mini.get());
        self.scroll.vadjustment().set_value(0.0);
        self.fit_player();
    }

    /// Neues Video: bis es sein Format meldet 16:9 annehmen, nach oben scrollen.
    pub fn reset(&self) {
        self.aspect.set(16.0 / 9.0);
        self.fit_player();
        self.scroll.vadjustment().set_value(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_fits_video() {
        // 16:9 bei 1280 Breite: 720 hoch, Seite ist höher -> passt.
        assert_eq!(player_height(1280, 1400, 16.0 / 9.0, false), 720);
        // Breites Fenster, niedrige Seite: durch die Seitenhöhe begrenzt.
        assert_eq!(player_height(1920, 900, 16.0 / 9.0, false), 900);
        // Hochkant-Video (9:16): wäre riesig, also Seitenhöhe.
        assert_eq!(player_height(1280, 1400, 9.0 / 16.0, false), 1400);
        assert_eq!(player_height(1280, 1400, 16.0 / 9.0, true), 1400);
        // Seitenrand: unter 1600 px volle Breite (5 px), sonst 85 % mittig, Vollbild = 0.
        assert_eq!(player_side(1343, false, None), 5);
        assert_eq!(player_side(2560, false, None), 192);
        assert_eq!(player_side(2560, true, None), 0);
        // Gewählte Breite bleibt, schmaleres Fenster nimmt den Rand weg, zu schmal = randlos.
        assert_eq!(player_side(2560, false, Some(1600)), 480);
        assert_eq!(player_side(1800, false, Some(1600)), 100);
        assert_eq!(player_side(1500, false, Some(1600)), 0);
    }

    #[test]
    fn zoom_steps_and_limits() {
        assert_eq!(zoom(1600, -1.0, 2560), 1680);
        assert_eq!(zoom(1600, 1.0, 2560), 1520);
        assert_eq!(zoom(2500, -3.0, 2560), 2560);
        assert_eq!(zoom(700, 5.0, 2560), MIN_PLAYER_WIDTH);
    }
}
