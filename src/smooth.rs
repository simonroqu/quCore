//! Weiches Scrollen. GTK springt pro Mausrad-Raste sofort um ein ganzes Stück und beim Weiterschalten
//! mit der Tastatur sofort zur fokussierten Karte, das wirkt ruckelig. Hier gleitet die Liste
//! stattdessen in ein paar Bildern zum Ziel. Touchpads scrollen ohnehin fein (und mit Schwung)
//! und bleiben bei GTK.

use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::Cell;
use std::rc::Rc;

/// Zeitkonstante der Bewegung: nach so vielen Sekunden sind ~63 % des Wegs geschafft.
const TAU: f64 = 0.06;
/// Abstand zum Rand, wenn eine Karte per Tastatur ins Bild geholt wird.
const FOCUS_MARGIN: f64 = 24.0;

/// Neue Position nach `dt` Sekunden: nähert sich dem Ziel exponentiell (schnell los, sanft aus).
fn ease(value: f64, target: f64, dt: f64) -> f64 {
    value + (target - value) * (1.0 - (-dt / TAU).exp())
}

/// Wohin scrollen, damit der Bereich `y..y+h` ganz sichtbar ist (None = ist er schon).
fn reveal(value: f64, page: f64, y: f64, h: f64) -> Option<f64> {
    if y < value + FOCUS_MARGIN {
        Some(y - FOCUS_MARGIN)
    } else if y + h > value + page - FOCUS_MARGIN {
        // Höher als die Seite: lieber den Anfang zeigen.
        Some((y + h - page + FOCUS_MARGIN).min(y - FOCUS_MARGIN))
    } else {
        None
    }
}

/// Laufende Bewegung eines ScrolledWindow.
#[derive(Default)]
struct Glide {
    /// Ziel (None = keine Bewegung).
    target: Cell<Option<f64>>,
    /// Zuletzt selbst gesetzte Position: weicht sie ab, hat jemand anderes gescrollt.
    last: Cell<f64>,
    prev_frame: Cell<Option<i64>>,
}

/// Zu `to` gleiten; läuft schon eine Bewegung, bekommt sie nur ein neues Ziel.
fn glide(scroll: &gtk::ScrolledWindow, state: &Rc<Glide>, to: f64) {
    let adj = scroll.vadjustment();
    let to = to.clamp(adj.lower(), (adj.upper() - adj.page_size()).max(adj.lower()));
    let running = state.target.get().is_some();
    state.target.set(Some(to));
    if running {
        return;
    }
    state.last.set(adj.value());
    state.prev_frame.set(None);
    let state = state.clone();
    scroll.add_tick_callback(move |w, clock| {
        let adj = w.downcast_ref::<gtk::ScrolledWindow>().unwrap().vadjustment();
        let Some(to) = state.target.get() else { return glib::ControlFlow::Break };
        // Jemand anderes hat die Position geändert (Scrollbalken, Wiederherstellen): aufhören.
        if (adj.value() - state.last.get()).abs() > 1.0 {
            state.target.set(None);
            return glib::ControlFlow::Break;
        }
        let now = clock.frame_time();
        let dt = state.prev_frame.get().map_or(1.0 / 60.0, |p| (now - p) as f64 / 1e6);
        state.prev_frame.set(Some(now));
        let v = ease(adj.value(), to, dt);
        if (to - v).abs() < 0.5 {
            adj.set_value(to);
            state.target.set(None);
            return glib::ControlFlow::Break;
        }
        adj.set_value(v);
        state.last.set(adj.value());
        glib::ControlFlow::Continue
    });
}

pub fn attach(scroll: &gtk::ScrolledWindow) {
    let state: Rc<Glide> = Rc::default();

    // Mausrad: vor dem eigenen Scroll-Handler des ScrolledWindow drankommen.
    let ctl = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    ctl.set_propagation_phase(gtk::PropagationPhase::Capture);
    ctl.connect_scroll({
        let (scroll, state) = (scroll.downgrade(), state.clone());
        move |c, _, dy| {
            let Some(scroll) = scroll.upgrade() else { return glib::Propagation::Proceed };
            if c.unit() != gdk::ScrollUnit::Wheel {
                return glib::Propagation::Proceed;
            }
            let adj = scroll.vadjustment();
            // Schrittweite wie bei GTK selbst (Seitenhöhe hoch 2/3).
            let from = state.target.get().unwrap_or(adj.value());
            glide(&scroll, &state, from + dy * adj.page_size().powf(2.0 / 3.0));
            glib::Propagation::Stop
        }
    });
    scroll.add_controller(ctl);

    // Tastatur: GTK scrollt sonst hart zum fokussierten Element. Stattdessen hingleiten, aber nur bei
    // Tastatur-Fokus (sichtbarer Fokusrahmen), nicht wenn ein Mausklick etwas fokussiert.
    if let Some(vp) = scroll.child().and_downcast::<gtk::Viewport>() {
        vp.set_scroll_to_focus(false);
    }
    scroll.connect_realize({
        let state = state.clone();
        move |scroll| {
            let Some(window) = scroll.root().and_downcast::<gtk::Window>() else { return };
            let (scroll, state) = (scroll.downgrade(), state.clone());
            window.connect_focus_widget_notify(move |win| {
                let (Some(scroll), Some(focus)) = (scroll.upgrade(), gtk::prelude::GtkWindowExt::focus(win)) else { return };
                let Some(content) = scroll.child() else { return };
                if !win.gets_focus_visible() || !scroll.is_mapped() || !focus.is_ancestor(&content) {
                    return;
                }
                let Some(b) = focus.compute_bounds(&content) else { return };
                let adj = scroll.vadjustment();
                let now = state.target.get().unwrap_or(adj.value());
                if let Some(to) = reveal(now, adj.page_size(), b.y() as f64, b.height() as f64) {
                    glide(&scroll, &state, to);
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_towards_target() {
        let v = ease(0.0, 100.0, 1.0 / 60.0);
        assert!(v > 0.0 && v < 100.0);
        // Nach 0,5 s praktisch angekommen, ohne übers Ziel hinaus.
        let mut v = 0.0;
        for _ in 0..30 {
            v = ease(v, 100.0, 1.0 / 60.0);
        }
        assert!(v > 99.0 && v <= 100.0);
    }

    #[test]
    fn reveals_focused_card() {
        // Sichtbar: nichts tun.
        assert_eq!(reveal(0.0, 1000.0, 100.0, 300.0), None);
        // Unten abgeschnitten: so weit, dass sie mit Rand ganz drauf ist.
        assert_eq!(reveal(0.0, 1000.0, 900.0, 300.0), Some(224.0));
        // Oben abgeschnitten.
        assert_eq!(reveal(500.0, 1000.0, 400.0, 300.0), Some(376.0));
    }
}
