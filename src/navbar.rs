//! Menü oben links (Nav-Bubble), aufgebaut aus `[nav]` in `qu.toml`: obere Reihe (`top`) und – wenn
//! `menu` Einträge hat – eine Spalte unter dem Burger. Eingebaut: `back` (erst sichtbar, wenn es einen
//! Verlauf gibt), `home`, `search`, `busy` (Lade-Kreis, `busy::Busy`), `menu` (Burger). Eigene Knöpfe
//! holt die App mit `button(id)` und hängt den Klick an; Slots (`slot(id)`) füllt sie selbst.

use crate::config::Nav;
use crate::nav_shape::NavShape;
use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct NavBar {
    /// Das ganze Menü (über die Seiten legen); blendet beim Video mit der Leiste aus.
    pub widget: gtk::Revealer,
    /// Die gezeichnete Form (für „liegt ein Klick im Menü?“).
    pub shape: NavShape,
    buttons: HashMap<String, gtk::Button>,
    slots: HashMap<String, gtk::Box>,
    menu: Option<gtk::Revealer>,
    on_menu: RefCell<Vec<Box<dyn Fn(bool)>>>,
    /// IDs der oberen Reihe und der Spalte (Reihenfolge für die Tastatur), aufklappende Slots.
    top_ids: Vec<String>,
    menu_ids: Vec<String>,
    expanding: Vec<String>,
}

/// Runder Icon-Knopf wie in der Nav-Bubble (`icon` ohne `qu-`/`-symbolic`, z. B. "pause").
pub fn nav_button(icon: &str, tip: &str) -> gtk::Button {
    gtk::Button::builder().icon_name(format!("qu-{icon}-symbolic")).tooltip_text(tip).focusable(false).css_classes(["flat"]).build()
}

impl NavBar {
    pub fn new(cfg: &Nav) -> Rc<Self> {
        let mut buttons = HashMap::new();
        let mut slots = HashMap::new();
        // Ein Eintrag als Widget (Knopf, Lade-Kreis oder Slot).
        let mut make = |id: &str| -> gtk::Widget {
            let builtin = match id {
                "back" => Some(("back", "Back (Esc)")),
                "home" => Some(("home", "Home")),
                "search" => Some(("search", "Search (/)")),
                "menu" => Some(("menu", "More")),
                _ => None,
            };
            if id == "busy" {
                let icon = crate::icons::spinner(16).visible(false).margin_start(6).margin_end(6).build();
                crate::busy::set_icon(&icon);
                return icon.upcast();
            }
            if let Some((icon, tip)) = builtin {
                let b = nav_button(icon, tip);
                b.set_visible(id != "back");
                buttons.insert(id.to_string(), b.clone());
                return b.upcast();
            }
            let item = crate::config::get_item(cfg, id);
            if item.slot {
                let slot = gtk::Box::builder().halign(gtk::Align::Start).build();
                slots.insert(id.to_string(), slot.clone());
                return slot.upcast();
            }
            let b = nav_button(&item.icon, &item.tooltip);
            buttons.insert(id.to_string(), b.clone());
            b.upcast()
        };

        let top = gtk::Box::builder().spacing(2).height_request(32).build();
        for id in &cfg.top {
            top.append(&make(id));
        }
        let shape = NavShape::new();
        shape.set_css_classes(&["bubble", "nav-shape"]);
        shape.append(&top);

        let mut menu = None;
        if !cfg.menu.is_empty() {
            let column = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).halign(gtk::Align::Start).margin_top(2).build();
            let entries: Vec<gtk::Widget> = cfg.menu.iter().map(|id| make(id)).collect();
            for w in &entries {
                // Links bündig: sonst wachsen die Knöpfe mit, wenn ein Slot die Spalte breiter macht.
                w.set_halign(gtk::Align::Start);
                column.append(w);
            }
            // Links ausgerichtet: nav_shape misst an der Revealer-Breite, wie breit die Spalte ist.
            let r = gtk::Revealer::builder().child(&column).halign(gtk::Align::Start).transition_type(gtk::RevealerTransitionType::SlideDown).build();
            shape.append(&r);
            // Aufklappender Slot (unten): Bubble wird zum liegenden U; obere Reihe nur so breit wie ihre Knöpfe.
            let expands = cfg.menu.iter().zip(&entries).find(|(id, _)| crate::config::get_item(cfg, id).expands);
            if let (Some(first), Some((_, bottom))) = (entries.first(), expands) {
                shape.set_parts(first, bottom);
                top.set_halign(gtk::Align::Start);
            }
            menu = Some(r);
        }

        let widget = gtk::Revealer::builder()
            .child(&shape)
            .transition_type(gtk::RevealerTransitionType::Crossfade)
            .reveal_child(true)
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Start)
            .css_classes(["nav-bubble"])
            .build();
        let expanding = cfg.items.iter().filter(|i| i.expands).map(|i| i.id.clone()).collect();
        let bar = Rc::new(NavBar {
            widget,
            shape,
            buttons,
            slots,
            menu,
            on_menu: RefCell::default(),
            top_ids: cfg.top.clone(),
            menu_ids: cfg.menu.clone(),
            expanding,
        });

        // Burger klappt auf/zu; jeder andere Knopf klappt nach dem Klick zu.
        for (id, b) in &bar.buttons {
            let weak = Rc::downgrade(&bar);
            let toggle = id == "menu";
            b.connect_clicked(move |_| {
                if let Some(bar) = weak.upgrade() {
                    bar.set_menu(toggle && !bar.menu_open());
                }
            });
        }
        bar
    }

    /// Knopf zu einer ID (eingebaut oder `[[nav.item]]`). Fehlt er in `qu.toml`, ist das ein Fehler der App.
    pub fn button(&self, id: &str) -> &gtk::Button {
        self.buttons.get(id).unwrap_or_else(|| panic!("qu.toml: Menüeintrag „{id}“ fehlt in nav.top/nav.menu"))
    }

    /// Gibt es den Knopf (z. B. `back`, wenn die App ihn optional nutzt)?
    pub fn has(&self, id: &str) -> bool {
        self.buttons.contains_key(id) || self.slots.contains_key(id)
    }

    /// Slot zum Selbstfüllen.
    pub fn slot(&self, id: &str) -> &gtk::Box {
        self.slots.get(id).unwrap_or_else(|| panic!("qu.toml: Slot „{id}“ fehlt in nav.top/nav.menu"))
    }

    pub fn menu_open(&self) -> bool {
        self.menu.as_ref().is_some_and(|m| m.reveals_child())
    }

    /// Burger-Spalte auf-/zuklappen (ohne Burger: nichts).
    pub fn set_menu(&self, open: bool) {
        let Some(m) = &self.menu else { return };
        m.set_reveal_child(open);
        for f in self.on_menu.borrow().iter() {
            f(open);
        }
    }

    /// Wird bei jedem Auf-/Zuklappen aufgerufen (z. B. um einen aufgeklappten Slot mit zu schließen).
    pub fn connect_menu(&self, f: impl Fn(bool) + 'static) {
        self.on_menu.borrow_mut().push(Box::new(f));
    }

    /// Klick neben das offene Menü klappt es zu; der Klick geht trotzdem weiter an sein Ziel.
    pub fn close_on_outside_click(self: &Rc<Self>, window: &gtk::ApplicationWindow) {
        if self.menu.is_none() {
            return;
        }
        let outside = gtk::GestureClick::builder().button(0).propagation_phase(gtk::PropagationPhase::Capture).build();
        let (bar, win) = (Rc::downgrade(self), window.downgrade());
        outside.connect_pressed(move |_, _, x, y| {
            let (Some(bar), Some(win)) = (bar.upgrade(), win.upgrade()) else { return };
            let inside = win.pick(x, y, gtk::PickFlags::DEFAULT).is_some_and(|w| w.is_ancestor(&bar.shape));
            if bar.menu_open() && !inside {
                bar.set_menu(false);
            }
        });
        window.add_controller(outside);
    }

    /// Knopf eines Eintrags: der Knopf selbst oder bei einem Slot der erste Knopf darin.
    fn key_button(&self, id: &str) -> Option<gtk::Button> {
        if let Some(b) = self.buttons.get(id) {
            return Some(b.clone());
        }
        fn first(w: &gtk::Widget) -> Option<gtk::Button> {
            if let Ok(b) = w.clone().downcast::<gtk::Button>() {
                return Some(b);
            }
            let mut c = w.first_child();
            while let Some(child) = c {
                if let Some(b) = first(&child) {
                    return Some(b);
                }
                c = child.next_sibling();
            }
            None
        }
        first(self.slots.get(id)?.upcast_ref())
    }

    /// Menü per Tastatur (die App ruft das aus ihrem Tasten-Handler): Esc klappt auf (erster Eintrag gewählt),
    /// Pfeile wählen (hoch/runter in der Spalte mit Umlauf, rechts/links in die obere Reihe), Enter/Leertaste
    /// drückt, Esc klappt zu. Die Knöpfe sind sonst nicht fokussierbar (Maus-Bedienung), nur solange so geöffnet.
    /// `true` = Taste verbraucht.
    pub fn key(&self, window: &gtk::ApplicationWindow, key: gtk::gdk::Key) -> bool {
        use gtk::gdk::Key;
        if self.menu.is_none() {
            return false;
        }
        let col_all: Vec<(gtk::Button, bool)> =
            self.menu_ids.iter().filter_map(|id| Some((self.key_button(id)?, self.expanding.contains(id)))).collect();
        let top_all: Vec<gtk::Button> =
            self.top_ids.iter().filter(|id| *id != "menu").filter_map(|id| self.buttons.get(id).cloned()).collect();
        let shown = |b: &gtk::Button| b.is_visible() && b.parent().is_some_and(|p| p.is_visible());
        let col: Vec<&gtk::Button> = col_all.iter().map(|(b, _)| b).filter(|b| shown(b)).collect();
        let top: Vec<&gtk::Button> = top_all.iter().filter(|b| shown(b)).collect();
        let all = || col_all.iter().map(|(b, _)| b).chain(&top_all);
        let close = || {
            self.set_menu(false);
            for b in all() {
                b.set_focusable(false);
            }
            gtk::prelude::GtkWindowExt::set_focus(window, None::<&gtk::Widget>);
        };
        let go = |b: Option<&&gtk::Button>| {
            if let Some(b) = b {
                b.grab_focus();
            }
        };
        if !self.menu_open() {
            if key != Key::Escape {
                return false;
            }
            self.set_menu(true);
            for b in all() {
                b.set_focusable(true);
            }
            // Markierung des gewählten Knopfs zeigen (`:focus-visible`).
            window.set_focus_visible(true);
            go(col.first());
            return true;
        }
        let focus = gtk::prelude::GtkWindowExt::focus(window);
        let is = |b: &&gtk::Button| focus.as_ref().is_some_and(|f| f == b.upcast_ref::<gtk::Widget>());
        let (in_col, in_top) = (col.iter().position(is), top.iter().position(is));
        let down = key == Key::Down;
        match key {
            Key::Escape => close(),
            Key::Up | Key::Down => match (in_col, in_top) {
                (Some(i), _) => {
                    let n = col.len();
                    go(col.get(if down { (i + 1) % n } else { (i + n - 1) % n }));
                }
                (None, Some(_)) if !down => {}
                _ => go(col.first()),
            },
            Key::Right => match (in_col, in_top) {
                (_, Some(i)) => go(top.get(i + 1)),
                (Some(_), _) => go(top.first()),
                _ => go(col.first()),
            },
            Key::Left => match in_top {
                Some(0) => go(col.first()),
                Some(i) => go(top.get(i - 1)),
                None => {}
            },
            Key::Return | Key::KP_Enter | Key::space => {
                let Some(b) = col.iter().chain(&top).find(|b| is(b)).map(|b| (*b).clone()) else { return false };
                // Aufklappender Slot (Konto) lässt das Menü offen, sonst schließt der Klick es.
                let keep_open = col_all.iter().any(|(c, e)| *e && *c == b);
                if !keep_open {
                    close();
                }
                b.emit_clicked();
            }
            _ => return false,
        }
        true
    }
}
