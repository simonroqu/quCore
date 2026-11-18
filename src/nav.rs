//! Fenster-Gerüst und Seitenwechsel für alle Qu-Apps.
//!
//! `Shell`: Seitenstapel (`stack`), Ebene mit Menü oben links (`page_layer`), Fenster. Die App legt ihre Seiten
//! mit `stack.add_named` an; „watch“ ist dabei nur ein leerer Platzhalter.
//! `Nav`: Verlauf (Zurück/Vor/Home) und die Ansehen-Seite als eigene Ebene über den Seiten – verlässt man sie
//! während ein Video läuft, spielt es als Miniplayer in einer Ecke weiter (ziehbar, gleitet in die nächste
//! Ecke; Knöpfe: groß machen, schließen, Play, Ton). Der Player wird dabei nie umgehängt (sonst baut mpv
//! seine Video-Fläche neu auf), nur verkleinert gezeichnet (`fly::Zoom`).

use crate::fly;
use crate::navbar::{NavBar, nav_button};
use crate::player::Player;
use crate::watch::WatchFrame;
use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct Shell {
    /// Die Seiten der App („browse“, „show“, „channel“, „watch“ …).
    pub stack: gtk::Stack,
    /// Seiten plus Menü oben links; darüber liegt später die Ansehen-Seite (`Nav`).
    pub page_layer: gtk::Overlay,
    /// Ganz oben: Platz für Suchfenster und Fensterknöpfe.
    pub root: gtk::Overlay,
    pub window: gtk::ApplicationWindow,
    /// Menü oben links aus `[nav]` in qu.toml.
    pub navbar: Rc<NavBar>,
}

impl Shell {
    pub fn new(app: &gtk::Application) -> Self {
        // Keine Kopfzeile: das Menü oben links baut qu-core aus `[nav]` in qu.toml.
        let navbar = NavBar::new(&crate::config().nav);
        // Nicht homogen: sonst bestimmt die breiteste (auch unsichtbare) Seite die Mindestbreite aller Seiten.
        let stack = gtk::Stack::builder().transition_type(gtk::StackTransitionType::Crossfade).hhomogeneous(false).vhomogeneous(false).build();
        // Ebene 1: Seiten plus Navigation. Ebene 2 (darüber): Suchfenster und Fenster-Knöpfe.
        let page_layer = gtk::Overlay::builder().child(&stack).build();
        page_layer.add_overlay(&navbar.widget);
        let root = gtk::Overlay::builder().child(&page_layer).build();
        // Fenster samt Fensterknöpfen (außerhalb von Tiling-WMs), Titel aus qu.toml.
        let window = crate::window(app, &root);
        navbar.close_on_outside_click(&window);
        Shell { stack, page_layer, root, window, navbar }
    }
}

/// Seitenwechsel mit Verlauf für den Zurück-Button; „watch“ = Ansehen-Seite (eigene Ebene, Miniplayer).
pub struct Nav<M: Clone + 'static> {
    pub stack: gtk::Stack,
    back: gtk::Button,
    pub window: gtk::ApplicationWindow,
    pub player: Rc<Player<M>>,
    pub history: RefCell<Vec<String>>,
    /// Seiten, von denen man mit „Zurück“ gekommen ist (Maustaste 5 = vor). Neue Wege leeren sie.
    forward: RefCell<Vec<String>>,
    /// Schwebende Knöpfe oben links. Auf den Seiten immer sichtbar über der Seite; beim Video sitzen sie im
    /// Player und blenden sich mit dessen Leiste aus und ein (offenes Menü hält sie sichtbar).
    pub nav_bar: gtk::Revealer,
    navbar: Rc<NavBar>,
    page_layer: gtk::Overlay,
    pub in_player: Cell<bool>,
    /// Rahmen der Ansehen-Seite (Player samt Infos der App).
    pub watch: Rc<WatchFrame>,
    /// Umgibt die Ansehen-Seite für die Öffnen-/Miniplayer-Animation (fly.rs).
    pub zoom: fly::Zoom,
    /// Knöpfe im Miniplayer (Pause, Ton, Zurück zum Video, Schließen).
    mini_bar: gtk::Box,
    /// Video im Vollbild (Player-Knopf, `f`). Getrennt vom Fenster-Vollbild (Super+F, Fensterknopf):
    /// war das Fenster schon vorher im Vollbild, bleibt es das beim Verlassen (`window_was_full`).
    video_full: Cell<bool>,
    window_was_full: Cell<bool>,
}

impl<M: Clone + 'static> Nav<M> {
    pub fn new(shell: &Shell, player: Rc<Player<M>>, watch: Rc<WatchFrame>) -> Rc<Self> {
        // Miniplayer-Knöpfe in den Ecken, je eine Bubble: oben links groß machen, oben rechts schließen,
        // unten links Play und Ton. Die Symbole folgen den Knöpfen der großen Leiste.
        let mini_bar = gtk::Box::builder().orientation(gtk::Orientation::Vertical).margin_start(8).margin_end(8).margin_top(8).margin_bottom(8).visible(false).build();
        let bubble = || gtk::Box::builder().spacing(2).height_request(32).css_classes(["bubble"]).build();
        let (mini_top_left, mini_top_right, mini_bottom) = (bubble(), bubble(), bubble());
        let mini_play = nav_button("pause", "Play/Pause");
        player.controls.play.bind_property("icon-name", &mini_play, "icon-name").sync_create().build();
        let mini_mute = nav_button("volume", "Mute");
        player.controls.mute.bind_property("icon-name", &mini_mute, "icon-name").sync_create().build();
        let mini_open = nav_button("fullscreen", "Back to video");
        let mini_close = nav_button("close", "Close");
        mini_top_left.append(&mini_open);
        mini_top_right.append(&mini_close);
        mini_bottom.append(&mini_play);
        mini_bottom.append(&mini_mute);
        let top = gtk::CenterBox::builder().start_widget(&mini_top_left).end_widget(&mini_top_right).valign(gtk::Align::Start).vexpand(true).build();
        mini_bottom.set_halign(gtk::Align::Start);
        mini_bottom.set_valign(gtk::Align::End);
        mini_bar.append(&top);
        mini_bar.append(&mini_bottom);
        mini_play.connect_clicked({
            let mpv = player.mpv;
            move |_| {
                let _ = mpv.command("cycle", &["pause"]);
            }
        });
        mini_mute.connect_clicked({
            let controls = player.controls.clone();
            move |_| controls.toggle_mute()
        });
        watch.widget.add_overlay(&mini_bar);
        // Über den Seiten, unter Navigation (wird beim Seitenwechsel neu obenauf gelegt) und Suche.
        // In `Zoom` verpackt: geht beim Öffnen aus Karte/Kachel auf, Miniplayer ebenso (fly.rs).
        let zoom = fly::Zoom::new(&watch.widget, &player.widget);
        shell.page_layer.add_overlay(&zoom);
        // Miniplayer ziehen; beim Loslassen gleitet er in die nächste Ecke.
        let drag = gtk::GestureDrag::new();
        drag.set_propagation_phase(gtk::PropagationPhase::Capture);
        let dragging = Rc::new(Cell::new(false));
        drag.connect_drag_update({
            let (watch, zoom, dragging) = (watch.clone(), zoom.clone(), dragging.clone());
            move |g, dx, dy| {
                if !watch.is_mini() {
                    return;
                }
                // Erst ab ein paar Pixeln ist es Ziehen; sonst bleibt es ein Klick (Knöpfe im Miniplayer).
                if !dragging.get() && dx.hypot(dy) < 8.0 {
                    return;
                }
                if !dragging.get() {
                    dragging.set(true);
                    g.set_state(gtk::EventSequenceState::Claimed);
                }
                zoom.set_offset(dx as f32, dy as f32);
            }
        });
        // Loslassen nach Ziehen um (dx, dy): in die nächste Ecke gleiten.
        drag.connect_drag_end({
            let (watch, zoom) = (watch.clone(), zoom.clone());
            move |_, dx, dy| {
                if !dragging.replace(false) {
                    return;
                }
                let (dx, dy) = (dx as f32, dy as f32);
                zoom.set_offset(0.0, 0.0);
                let (Some(player), Some(page)) = (zoom.anchor_bounds(), watch.widget.compute_bounds(&zoom)) else { return };
                // Nächste Ecke = Seite, auf der die Mitte des Miniplayers jetzt liegt.
                let right = page.x() + dx + page.width() / 2.0 > zoom.width() as f32 / 2.0;
                let bottom = page.y() + dy + page.height() / 2.0 > zoom.height() as f32 / 2.0;
                watch.set_corner(right, bottom);
                let from = gtk::graphene::Rect::new(player.x() + dx, player.y() + dy, player.width(), player.height());
                zoom.animate_from(from, None);
            }
        });
        watch.widget.add_controller(drag);

        let nav = Rc::new(Nav {
            stack: shell.stack.clone(),
            back: shell.navbar.button("back").clone(),
            window: shell.window.clone(),
            player: player.clone(),
            history: RefCell::default(),
            forward: RefCell::default(),
            nav_bar: shell.navbar.widget.clone(),
            navbar: shell.navbar.clone(),
            page_layer: shell.page_layer.clone(),
            in_player: Cell::new(false),
            watch,
            zoom,
            mini_bar,
            video_full: Cell::new(false),
            window_was_full: Cell::new(false),
        });
        player.controls.fullscreen.connect_clicked({
            let nav = Rc::downgrade(&nav);
            move |_| {
                if let Some(nav) = nav.upgrade() {
                    nav.set_video_fullscreen(!nav.video_fullscreen());
                }
            }
        });
        // Fenster verlässt das Vollbild von außen (Super+F, Fensterknopf), während das Video es nutzt: Video auch raus.
        shell.window.connect_fullscreened_notify({
            let nav = Rc::downgrade(&nav);
            move |w| {
                if let Some(nav) = nav.upgrade().filter(|n| n.video_full.get() && !w.is_fullscreen()) {
                    nav.window_was_full.set(false);
                    nav.set_video_fullscreen(false);
                }
            }
        });
        mini_open.connect_clicked({
            let nav = nav.clone();
            move |_| nav.go("watch")
        });
        mini_close.connect_clicked({
            let nav = nav.clone();
            move |_| nav.close_mini()
        });
        // Beim Video: Navigation blendet sich zusammen mit der Player-Leiste ein und aus; ein offenes Menü
        // hält sie sichtbar, auch wenn sich die Leiste ausblendet.
        player.controls.widget.connect_reveal_child_notify({
            let nav = Rc::downgrade(&nav);
            move |r| {
                if let Some(nav) = nav.upgrade().filter(|n| n.in_player.get()) {
                    nav.nav_bar.set_reveal_child(r.reveals_child() || nav.navbar.menu_open());
                }
            }
        });
        shell.navbar.connect_menu({
            let nav = Rc::downgrade(&nav);
            move |open| {
                if let Some(nav) = nav.upgrade().filter(|n| n.in_player.get()) {
                    nav.nav_bar.set_reveal_child(open || nav.player.controls.widget.reveals_child());
                }
            }
        });
        nav
    }

    pub fn current(&self) -> String {
        self.stack.visible_child_name().map(|s| s.to_string()).unwrap_or_default()
    }

    pub fn show(&self, page: &str) {
        let watch = page == "watch";
        // Ansehen-Seite erscheint (neu oder aus dem Miniplayer): kurz einblenden.
        let appear = watch && (self.current() != "watch" || self.watch.is_mini());
        // Verlässt man die Ansehen-Seite, läuft ein laufendes Video im Miniplayer weiter, sonst stoppt es.
        if self.current() == "watch" && !watch {
            self.set_video_fullscreen(false);
            if self.player.active() {
                self.set_mini(true);
            } else {
                self.close_mini();
            }
        }
        if watch {
            self.set_mini(false);
        } else if self.watch.is_mini() && !self.player.active() {
            // Im Miniplayer zu Ende gelaufen (z. B. „Home“ im Endbildschirm): weg damit.
            self.close_mini();
        }
        self.watch.widget.set_visible(watch || self.watch.is_mini());
        if appear && !self.zoom.animating() {
            // Die CSS-Animation startet nur neu, wenn sich ihr Name ändert: zwischen a und b wechseln.
            let w = &self.watch.widget;
            let a = w.has_css_class("page-in-a");
            w.remove_css_class(if a { "page-in-a" } else { "page-in-b" });
            w.add_css_class(if a { "page-in-b" } else { "page-in-a" });
        }
        // Von/zur Ansehen-Seite ohne Überblendung: die liegt als eigene Ebene darüber (Zoom-Animation),
        // im Stack steht nur ein leerer Platzhalter. Mit Überblendung wäre es beim Schrumpfen dahinter schwarz.
        let fade = !watch && self.current() != "watch";
        self.stack.set_transition_type(if fade { gtk::StackTransitionType::Crossfade } else { gtk::StackTransitionType::None });
        self.stack.set_visible_child_name(page);
        self.back.set_visible(!self.history.borrow().is_empty());

        if watch != self.in_player.get() {
            let (from, to) = if watch { (&self.page_layer, &self.player.widget) } else { (&self.player.widget, &self.page_layer) };
            from.remove_overlay(&self.nav_bar);
            to.add_overlay(&self.nav_bar);
            self.in_player.set(watch);
        }
        self.nav_bar.set_reveal_child(!watch || self.player.controls.widget.reveals_child() || self.navbar.menu_open());
    }

    pub fn video_fullscreen(&self) -> bool {
        self.video_full.get()
    }

    /// Video-Vollbild an/aus: Player randlos ohne Infos (`WatchFrame::set_fullscreen`), Fenster nur dann ins Vollbild
    /// und wieder heraus, wenn es nicht schon vorher im Vollbild war.
    pub fn set_video_fullscreen(&self, on: bool) {
        if on == self.video_full.get() {
            return;
        }
        self.video_full.set(on);
        if on {
            self.window_was_full.set(self.window.is_fullscreen());
            self.window.fullscreen();
        } else if !self.window_was_full.get() {
            self.window.unfullscreen();
        }
        self.watch.set_fullscreen(on);
        self.player.controls.fullscreen.set_icon_name(if on { "qu-restore-symbolic" } else { "qu-fullscreen-symbolic" });
    }

    pub fn set_mini(&self, on: bool) {
        // Wechsel groß ↔ klein bei laufendem Video: die Seite schrumpft/wächst vom alten Platz aus.
        let from = (self.watch.is_mini() != on && self.player.active()).then(|| self.zoom.anchor_bounds()).flatten();
        // Beim Großwerden bleibt die Seite darunter stehen, bis die Animation fertig ist.
        let backdrop = from.filter(|_| !on).and_then(|_| self.zoom.backdrop(&self.stack));
        self.watch.set_mini(on);
        if let Some(from) = from {
            self.zoom.animate_from(from, backdrop);
        }
        // Mini-Knöpfe erst nach dem Schrumpfen: während der Animation wird die schon kleine Seite groß
        // gezeichnet, die Knöpfe wären riesig. Danach nur, wenn der Player dann noch klein ist.
        self.mini_bar.set_visible(false);
        if on {
            let (bar, watch) = (self.mini_bar.clone(), self.watch.clone());
            self.zoom.after(move || bar.set_visible(watch.is_mini()));
        }
        // Die große Leiste passt nicht in den kleinen Player.
        self.player.controls.widget.set_visible(!on);
    }

    /// Miniplayer schließen bzw. Video beenden (speichert die Position).
    pub fn close_mini(&self) {
        // Das Video hat den Tastatur-Fokus. Verschwindet es damit, gibt GTK den Fokus der ersten Karte,
        // und die Liste springt nach oben. Darum vorher den Fokus abgeben.
        gtk::prelude::GtkWindowExt::set_focus(&self.window, None::<&gtk::Widget>);
        self.player.stop();
        self.set_mini(false);
        self.watch.widget.set_visible(self.current() == "watch");
        self.window.set_title(Some(&crate::config().app.title));
    }

    pub fn go(&self, page: &str) {
        let cur = self.current();
        if cur != page {
            self.history.borrow_mut().push(cur);
            self.forward.borrow_mut().clear();
        }
        self.show(page);
    }

    pub fn back(&self) {
        let prev = self.history.borrow_mut().pop();
        if let Some(p) = prev {
            self.forward.borrow_mut().push(self.current());
            self.show(&p);
        }
    }

    /// Gegenstück zu `back`. ponytail: merkt nur die Seite, nicht ihren Inhalt – die Seiten gibt es je
    /// einmal und sie zeigen noch, was zuletzt drin war.
    pub fn forward(&self) {
        let next = self.forward.borrow_mut().pop();
        if let Some(p) = next {
            self.history.borrow_mut().push(self.current());
            self.show(&p);
        }
    }

    /// Startseite („browse“), Verlauf leer.
    pub fn home(&self) {
        self.history.borrow_mut().clear();
        self.forward.borrow_mut().clear();
        self.show("browse");
    }
}
