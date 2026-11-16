//! Nav-Bubble als selbst gezeichnete Form: eingeklappt eine Pille, aufgeklappt ein umgedrehtes L
//! (Reihe oben, Spalte unter dem Burger), mit offener Kontoliste ein liegendes U (zusätzlich eine
//! Reihe unten nach rechts). CSS kann nur Rechtecke, darum Pfad + Schatten von Hand.
//! Farben wie `.bubble` in style.css.

use gtk4 as gtk;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

/// Innenabstand der Form (= Padding 2 px + Rand 1 px einer normalen Bubble).
pub const PAD: f32 = 3.0;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct NavShape {
        /// Spalte (bestimmt deren Breite) und untere Reihe (Konto + aufgeklappte Kontoliste).
        pub column: glib::WeakRef<gtk::Widget>,
        pub bottom: glib::WeakRef<gtk::Widget>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NavShape {
        const NAME: &'static str = "QuNavShape";
        type Type = super::NavShape;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for NavShape {}
    impl BoxImpl for NavShape {}

    impl WidgetImpl for NavShape {
        fn snapshot(&self, s: &gtk::Snapshot) {
            let obj = self.obj();
            let (Some(top), Some(menu)) = (obj.first_child(), obj.last_child()) else { return };
            let (Some(t), Some(m)) = (top.compute_bounds(&*obj), menu.compute_bounds(&*obj)) else { return };
            // GTK misst ab der Innenfläche (ohne Padding): Form um PAD nach außen erweitern.
            s.save();
            s.translate(&graphene::Point::new(-PAD, -PAD));
            let h = obj.height() as f32 + 2.0 * PAD;
            let row = t.y() + t.height() + 2.0 * PAD;
            // Spaltenbreite von der Spalte selbst, sonst vom Menü (das mit der unteren Reihe breiter wird).
            let c = self.column.upgrade().and_then(|c| c.compute_bounds(&*obj)).unwrap_or(m);
            let col = c.x() + c.width() + 2.0 * PAD;
            // Rechte Kante der oberen Reihe: mit unterer Reihe (Tubequ) so breit wie ihr Inhalt, sonst die ganze Box.
            let r = if self.bottom.upgrade().is_some() { t.x() + t.width() + 2.0 * PAD } else { obj.width() as f32 + 2.0 * PAD };
            // Untere Reihe nur, solange das Menü offen ist (sonst liegt sie außerhalb).
            let bottom = self
                .bottom
                .upgrade()
                .and_then(|b| b.compute_bounds(&*obj))
                .filter(|b| h > row + 1.0 && b.y() + PAD >= row)
                .map(|b| (b.y(), b.x() + b.width() + 2.0 * PAD));
            if let Ok(path) = gsk::Path::parse(&shape(r, row, col, h, crate::theme::pill(), bottom)) {
                // Schatten wie `box-shadow: 0 6px 20px rgba(0,0,0,.35)`: nur außerhalb der Form.
                s.push_mask(gsk::MaskMode::InvertedAlpha);
                s.append_fill(&path, gsk::FillRule::Winding, &gdk::RGBA::BLACK);
                s.pop();
                s.push_blur(10.0);
                s.save();
                s.translate(&graphene::Point::new(0.0, 6.0));
                s.append_fill(&path, gsk::FillRule::Winding, &gdk::RGBA::new(0.0, 0.0, 0.0, 0.35));
                s.restore();
                s.pop();
                s.pop();
                s.append_fill(&path, gsk::FillRule::Winding, &gdk::RGBA::new(24.0 / 255.0, 24.0 / 255.0, 28.0 / 255.0, 0.55));
                s.append_stroke(&path, &gsk::Stroke::new(1.0), &gdk::RGBA::new(1.0, 1.0, 1.0, 0.12));
            }
            s.restore();
            self.parent_snapshot(s);
        }
    }
}

glib::wrapper! {
    pub struct NavShape(ObjectSubclass<imp::NavShape>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl NavShape {
    pub fn new() -> Self {
        let o: Self = glib::Object::new();
        o.set_orientation(gtk::Orientation::Vertical);
        o
    }

    /// Spalte unter dem Burger und untere Reihe darin, die nach rechts aufgehen kann.
    pub fn set_parts(&self, column: &impl IsA<gtk::Widget>, bottom: &impl IsA<gtk::Widget>) {
        self.imp().column.set(Some(column.upcast_ref()));
        self.imp().bottom.set(Some(bottom.upcast_ref()));
    }
}

/// SVG-Pfad der Form. `w` × `row`: obere Reihe, `col`: Breite der Spalte, `h`: Gesamthöhe
/// (> `row`, solange das Menü aufgeht). `bottom`: obere Kante und rechtes Ende der unteren Reihe;
/// reicht sie rechts über die Spalte hinaus, wird aus dem L ein liegendes U.
/// `pill`: Bubble-Rundung aus dem Theme. Halber Pixel Einzug, damit der 1-px-Rand scharf ist.
fn shape(w: f32, row: f32, col: f32, h: f32, pill: f32, bottom: Option<(f32, f32)>) -> String {
    let (l, t, r, b, cr, cb) = (0.5, 0.5, w - 0.5, row - 0.5, col - 0.5, h - 0.5);
    let rad = pill.min((b - t) / 2.0);
    let top = format!("M{l},{} A{rad},{rad} 0 0 1 {},{t} H{} A{rad},{rad} 0 0 1 {r},{} V{} A{rad},{rad} 0 0 1 {},{b}",
        t + rad, l + rad, r - rad, t + rad, b - rad, r - rad);
    let e = h - row;
    if e < 1.0 {
        return format!("{top} H{} A{rad},{rad} 0 0 1 {l},{} Z", l + rad, b - rad);
    }
    // Innere Ecke (nach innen gewölbt) und Spaltenende wachsen beim Aufklappen mit.
    let f = rad.min(e / 2.0);
    let rb = rad.min((cr - l) / 2.0).min(e - f);
    if let Some((bt, br)) = bottom.map(|(y, x)| (y, x - 0.5)).filter(|&(y, x)| x - cr >= 1.0 && y - b >= 1.0 && cb - y >= 1.0) {
        // Liegendes U: Spalte bis zur unteren Reihe, dann mit innerer Ecke nach rechts.
        let gap = bt - b;
        let f = rad.min(gap / 2.0);
        let re = rad.min((cb - bt) / 2.0);
        let f2 = rad.min(gap / 2.0).min(br - cr - re).max(0.0);
        return format!(
            "{top} H{} A{f},{f} 0 0 0 {cr},{} V{} A{f2},{f2} 0 0 0 {},{bt} H{} A{re},{re} 0 0 1 {br},{} V{} A{re},{re} 0 0 1 {},{cb} H{} A{rb},{rb} 0 0 1 {l},{} Z",
            cr + f, b + f, bt - f2, cr + f2, br - re, bt + re, cb - re, br - re, l + rb, cb - rb
        );
    }
    format!("{top} H{} A{f},{f} 0 0 0 {cr},{} V{} A{rb},{rb} 0 0 1 {},{cb} H{} A{rb},{rb} 0 0 1 {l},{} Z",
        cr + f, b + f, cb - rb, cr - rb, l + rb, cb - rb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_pill_and_l() {
        // Eingeklappt: Pille ohne innere Ecke; aufgeklappt: mit nach innen gewölbter Ecke (Bogen mit sweep 0).
        let closed = shape(130.0, 36.0, 36.0, 36.0, 9999.0, None);
        assert!(!closed.contains(" 0 0 0 "));
        let open = shape(130.0, 36.0, 36.0, 130.0, 9999.0, Some((96.0, 36.0)));
        assert!(open.contains(" 0 0 0 "));
        assert!(open.contains("V129.5") || open.contains(",129.5"), "Spalte reicht bis unten: {open}");
        assert!(gsk::Path::parse(&closed).is_ok() && gsk::Path::parse(&open).is_ok());
        // Kontoliste offen: untere Reihe reicht nach rechts, zwei innere Ecken, rechtes Ende bei 99.5.
        let u = shape(130.0, 36.0, 36.0, 130.0, 9999.0, Some((96.0, 100.0)));
        assert_eq!(u.matches(" 0 0 0 ").count(), 2, "{u}");
        assert!(u.contains("H99.5") || u.contains(" 99.5,"), "{u}");
        assert!(gsk::Path::parse(&u).is_ok());
    }
}
