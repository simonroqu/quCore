//! Übergangs-Animation wie beim Öffnen einer App auf dem iPhone: die Ansehen-Seite geht aus der
//! angeklickten Karte/Episoden-Kachel auf (bzw. aus dem/in den Miniplayer). `Zoom` umgibt die Seite und zeichnet
//! sie während der Animation verkleinert in ein wachsendes, abgerundetes Rechteck. Nur das Zeichnen
//! wird skaliert, das Layout bleibt – kein Neuanordnen pro Bild.

use gtk4 as gtk;
use gtk::{gdk, glib, graphene, gsk, prelude::*, subclass::prelude::*};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// Dauer der Animation.
const MS: f64 = 380.0;
/// Eckenrundung am Anfang (Vorschaubild), wird zum Ende hin 0.
const RADIUS: f32 = 12.0;

thread_local! {
    /// Startpunkte nach Schlüssel (Episoden-/Video-ID der Kachel bzw. Titel-ID der „Continue watching“-Karte),
    /// damit `play` weiß, wo die Animation startet.
    static THUMBS: RefCell<HashMap<String, glib::WeakRef<gtk::Widget>>> = RefCell::default();
}

/// Startpunkt merken (schwache Referenz, verschwindet mit dem Widget).
pub fn register(id: &str, w: &impl IsA<gtk::Widget>) {
    THUMBS.with_borrow_mut(|m| {
        // ponytail: tote Einträge erst ab 500 aufräumen, eine Liste hat selten mehr Karten.
        if m.len() > 500 {
            m.retain(|_, w| w.upgrade().is_some());
        }
        m.insert(id.to_string(), w.as_ref().downgrade());
    });
}

/// Platz des sichtbaren Startpunkts zu `id`, relativ zu `to`.
pub fn thumb_bounds(id: &str, to: &impl IsA<gtk::Widget>) -> Option<graphene::Rect> {
    let pic = THUMBS.with_borrow(|m| m.get(id).and_then(|w| w.upgrade()))?;
    pic.is_mapped().then(|| pic.compute_bounds(to)).flatten()
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Zoom {
        /// Startrechteck (eigene Koordinaten) und Startzeit (Frame-Clock, µs; `None` = noch kein Bild).
        pub from: Cell<Option<(graphene::Rect, Option<i64>)>>,
        pub progress: Cell<f32>,
        /// Standbild der Seite dahinter (vor dem Seitenwechsel aufgenommen) und ihr Platz.
        pub backdrop: RefCell<Option<(gdk::Paintable, graphene::Rect)>>,
        /// Widget in der Seite, das genau auf dem Startrechteck beginnt (der Player).
        pub anchor: glib::WeakRef<gtk::Widget>,
        /// Verschiebung beim Ziehen (Miniplayer), nur gezeichnet.
        pub offset: Cell<(f32, f32)>,
        /// Läuft, sobald die Animation fertig ist (`Zoom::after`).
        pub done: RefCell<Option<Box<dyn FnOnce()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Zoom {
        const NAME: &'static str = "QuZoom";
        type Type = super::Zoom;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            // Kind bekommt den ganzen Platz, eigene Ausrichtung/Ränder (Miniplayer) wendet GTK selbst an.
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for Zoom {
        fn dispose(&self) {
            while let Some(c) = self.obj().first_child() {
                c.unparent();
            }
        }
    }

    impl WidgetImpl for Zoom {
        /// Selbst nie Ziel von Klicks (liegt über dem ganzen Fenster), nur das Kind.
        fn contains(&self, _x: f64, _y: f64) -> bool {
            false
        }

        fn snapshot(&self, s: &gtk::Snapshot) {
            let obj = self.obj();
            let Some(child) = obj.first_child() else { return };
            let to = child.compute_bounds(&*obj).filter(|r| r.width() > 1.0);
            let (Some((from, _)), Some(to)) = (self.from.get(), to) else {
                let (dx, dy) = self.offset.get();
                s.save();
                s.translate(&graphene::Point::new(dx, dy));
                obj.snapshot_child(&child, s);
                s.restore();
                return;
            };
            let e = self.progress.get();
            // Alte Seite bleibt dahinter stehen und dunkelt leicht ab (wie der Homescreen beim iPhone).
            if let Some((img, at)) = self.backdrop.borrow().as_ref() {
                s.save();
                s.translate(&graphene::Point::new(at.x(), at.y()));
                img.snapshot(s, at.width() as f64, at.height() as f64);
                s.restore();
                s.append_color(&gdk::RGBA::new(0.0, 0.0, 0.0, 0.35 * e), at);
            }
            let lerp = |a: f32, b: f32| a + (b - a) * e;
            let lerp_rect = |a: &graphene::Rect, b: &graphene::Rect| {
                graphene::Rect::new(lerp(a.x(), b.x()), lerp(a.y(), b.y()), lerp(a.width(), b.width()), lerp(a.height(), b.height()))
            };
            // Der Player (Anker) beginnt genau auf dem Startrechteck und wächst an seinen Platz; die
            // Seite drumherum wird gleichmäßig mitskaliert. Der sichtbare Ausschnitt wächst vom
            // Startrechteck auf die ganze Seite – so wird das Vorschaubild zum Video, ohne Sprung.
            let anchor = self.anchor.upgrade().and_then(|a| a.compute_bounds(&*obj)).filter(|r| r.width() > 1.0).unwrap_or(to);
            let scale = lerp(from.width() / anchor.width(), 1.0);
            let clip = lerp_rect(&from, &to);
            let r = RADIUS * (1.0 - e);
            let corner = graphene::Size::new(r, r);
            s.push_rounded_clip(&gsk::RoundedRect::new(clip, corner, corner, corner, corner));
            s.save();
            s.translate(&graphene::Point::new(lerp(from.x(), anchor.x()), lerp(from.y(), anchor.y())));
            s.scale(scale, scale);
            s.translate(&graphene::Point::new(-anchor.x(), -anchor.y()));
            obj.snapshot_child(&child, s);
            s.restore();
            s.pop();
        }
    }
}

glib::wrapper! {
    pub struct Zoom(ObjectSubclass<imp::Zoom>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Zoom {
    /// `anchor`: Widget in `child`, das auf dem Startrechteck beginnt (der Player).
    pub fn new(child: &impl IsA<gtk::Widget>, anchor: &impl IsA<gtk::Widget>) -> Self {
        let z: Self = glib::Object::new();
        child.set_parent(&z);
        z.imp().anchor.set(Some(anchor.as_ref()));
        z
    }

    /// Platz des Ankers (Player) jetzt – vor einer Änderung messen, z. B. vor dem Wechsel in den Miniplayer.
    pub fn anchor_bounds(&self) -> Option<graphene::Rect> {
        let a = self.imp().anchor.upgrade()?;
        a.is_mapped().then(|| a.compute_bounds(self)).flatten()
    }

    /// Kind beim Ziehen verschoben zeichnen (Klick-Flächen bleiben am alten Platz, bis losgelassen).
    pub fn set_offset(&self, dx: f32, dy: f32) {
        self.imp().offset.set((dx, dy));
        self.queue_draw();
    }

    pub fn animating(&self) -> bool {
        self.imp().from.get().is_some()
    }

    /// `f` nach der laufenden Animation ausführen (ohne Animation sofort). Ersetzt ein noch wartendes `f`.
    pub fn after(&self, f: impl FnOnce() + 'static) {
        if self.animating() {
            self.imp().done.replace(Some(Box::new(f)));
        } else {
            f();
        }
    }

    /// Standbild von `w` samt Platz, als Hintergrund für `animate_from` (vor dem Seitenwechsel aufnehmen).
    pub fn backdrop(&self, w: &impl IsA<gtk::Widget>) -> Option<(gdk::Paintable, graphene::Rect)> {
        let at = w.is_mapped().then(|| w.compute_bounds(self)).flatten()?;
        Some((gtk::WidgetPaintable::new(Some(w)).current_image(), at))
    }

    /// Anker aus `from` (eigene Koordinaten) an seinen Platz wachsen lassen, Seite mit; `backdrop` dahinter.
    pub fn animate_from(&self, from: graphene::Rect, backdrop: Option<(gdk::Paintable, graphene::Rect)>) {
        if !gtk::Settings::default().is_some_and(|s| s.is_gtk_enable_animations()) {
            return;
        }
        let imp = self.imp();
        let running = imp.from.get().is_some();
        imp.from.set(Some((from, None)));
        imp.backdrop.replace(backdrop);
        imp.progress.set(0.0);
        self.queue_draw();
        if running {
            return; // laufender Tick übernimmt den neuen Start
        }
        self.add_tick_callback(|z, clock| {
            let imp = z.imp();
            let Some((from, t0)) = imp.from.get() else { return glib::ControlFlow::Break };
            let t0 = t0.unwrap_or_else(|| {
                imp.from.set(Some((from, Some(clock.frame_time()))));
                clock.frame_time()
            });
            let t = (((clock.frame_time() - t0) as f64 / 1000.0) / MS).min(1.0);
            // Schnell los, weich ankommen (ease-out, kubisch).
            imp.progress.set((1.0 - (1.0 - t).powi(3)) as f32);
            z.queue_draw();
            if t < 1.0 {
                return glib::ControlFlow::Continue;
            }
            imp.from.set(None);
            imp.backdrop.take();
            if let Some(f) = imp.done.take() {
                f();
            }
            glib::ControlFlow::Break
        });
    }
}
