//! Eigene Bedienleiste über dem Video: Fortschritt (mit Kapiteln), Play/Pause, Lautstärke,
//! Einstellungen (Qualität, Geschwindigkeit), Vollbild.
//! Bubble-Design: Timeline ohne Hintergrund, jede Knopfgruppe in einer eigenen Glas-Pille.
//! Erscheint nur bei Mausbewegung über dem Player und blendet sich nach kurzer Zeit wieder aus.

use crate::segments::Segment;
use gtk::gdk;
use gtk::gdk_pixbuf::Pixbuf;
use std::collections::HashMap;
use gtk4 as gtk;
use gtk::glib;
use gtk::prelude::*;
use libmpv2::Mpv;
use libmpv2::events::PropertyData;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

const HIDE_AFTER: Duration = Duration::from_millis(2000);
/// Beim Ziehen an der Timeline: so lange still halten, bevor gesprungen wird.
const SEEK_DELAY: Duration = Duration::from_millis(150);

/// Wählbare Qualitäten (maximale Bildhöhe, `[player] qualities` in qu.toml) mit Menütext. Gibt es die
/// gewählte nicht, nimmt der Player die nächstkleinere.
fn qualities() -> Vec<(u32, String)> {
    crate::config().player.qualities.iter().map(|h| (*h, if *h == 2160 { "4K (2160p)".into() } else { format!("{h}p") })).collect()
}

pub const SPEEDS: [f64; 8] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];

use crate::app::config_file;

/// Shader-Set für „AI“ im Upscaling-Menü (`[player] upscaler` in qu.toml).
fn upscaler() -> crate::upscale::Shaders {
    crate::config().player.upscaler
}

fn quality_file() -> std::path::PathBuf {
    config_file("quality")
}

/// Laufzeit als „1:02:03“ bzw. „2:03“.
pub fn fmt_duration(s: u32) -> String {
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// Ein-/Ausblenden des Tons und Lautstärke-Rampen: so viele Schritte in diesem Abstand (zusammen 160 ms).
const FADE_STEPS: u32 = 8;
const FADE_STEP: Duration = Duration::from_millis(20);

/// Höchste Lautstärke in % (Regler-Ende). 100 = normal, liegt so in der Mitte des Reglers.
pub const MAX_VOLUME: f64 = 200.0;

/// Zuletzt eingestellte Lautstärke (0–`MAX_VOLUME`), bleibt über Neustarts erhalten.
/// Standard 100 = normale Lautstärke, der Regler steht dann in der Mitte.
pub fn saved_volume() -> f64 {
    std::fs::read_to_string(config_file("volume"))
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map_or(100.0, |v| v.clamp(0.0, MAX_VOLUME))
}

fn save_volume(v: f64) {
    let f = config_file("volume");
    let _ = std::fs::create_dir_all(f.parent().unwrap());
    let _ = std::fs::write(f, format!("{v:.0}"));
}

/// Zuletzt gewählte Qualität (bleibt über Neustarts erhalten).
pub fn saved_quality() -> u32 {
    std::fs::read_to_string(quality_file())
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .filter(|q| crate::config().player.qualities.contains(q))
        .unwrap_or(crate::config().player.default_quality)
}

pub fn speed_label(s: f64) -> String {
    if s == 1.0 { "Normal".into() } else { format!("{s}×") }
}

/// Untertitel-Schriftgrößen (mpv `sub-font-size`, Standard 38).
const SUB_SIZES: [(i64, &str); 4] = [(30, "Small"), (38, "Normal"), (48, "Large"), (60, "Huge")];

/// Untertitel-Einstellungen in `~/.config/<app>/subtitles`, je eine Zeile: Sprache (Sprachcode wie „en“ oder
/// Spur-Name wie „English“), Größe, Hintergrund (`box`/`outline`), an/aus (`on`/`off`, gilt fürs nächste Video).
#[derive(Debug, PartialEq)]
struct SubPrefs {
    lang: String,
    size: i64,
    boxed: bool,
    on: bool,
}

impl SubPrefs {
    fn parse(s: &str) -> Self {
        let mut l = s.lines().map(str::trim);
        let lang = l.next().filter(|x| !x.is_empty()).unwrap_or("en").to_string();
        let size = l.next().and_then(|x| x.parse().ok()).filter(|v| SUB_SIZES.iter().any(|(s, _)| s == v)).unwrap_or(38);
        let boxed = l.next() == Some("box");
        SubPrefs { lang, size, boxed, on: l.next() != Some("off") }
    }

    fn load() -> Self {
        Self::parse(&std::fs::read_to_string(config_file("subtitles")).unwrap_or_default())
    }

    fn save(&self) {
        let f = config_file("subtitles");
        let _ = std::fs::create_dir_all(f.parent().unwrap());
        let style = if self.boxed { "box" } else { "outline" };
        let _ = std::fs::write(f, format!("{}\n{}\n{style}\n{}\n", self.lang, self.size, if self.on { "on" } else { "off" }));
    }

    fn apply(&self, mpv: &Mpv) {
        let _ = mpv.set_property("sub-font-size", self.size);
        let _ = mpv.set_property("sub-border-style", if self.boxed { "background-box" } else { "outline-and-shadow" });
    }
}

/// Untertitelspur: mpv-ID, Schlüssel (Sprachcode; ohne Sprache der Dateiname ohne Endung: „English.srt“ →
/// „English“) und Text fürs Menü. Nur bei Klicks/Laden abgefragt, nicht in Ereignissen.
pub fn sub_tracks(mpv: &Mpv) -> Vec<(i64, String, String)> {
    let n = mpv.get_property::<i64>("track-list/count").unwrap_or(0);
    let prop = |i: i64, k: &str| mpv.get_property::<String>(&format!("track-list/{i}/{k}")).unwrap_or_default();
    (0..n)
        .filter(|i| prop(*i, "type") == "sub")
        .filter_map(|i| {
            let id = mpv.get_property::<i64>(&format!("track-list/{i}/id")).ok()?;
            let (lang, title) = (prop(i, "lang"), prop(i, "title"));
            let stem = title.rsplit_once('.').map_or(title.as_str(), |(n, _)| n).to_string();
            Some(match (lang.is_empty(), stem.is_empty()) {
                (false, _) => (id, lang.clone(), lang_label(&lang)),
                (true, false) => (id, stem.clone(), stem),
                (true, true) => (id, String::new(), "Subtitles".into()),
            })
        })
        .collect()
}

/// Spur für die gewünschte Sprache: genau diese, sonst gleiche Grundsprache (en-US, en-orig), sonst Englisch
/// („en…“, „English“), sonst die erste.
fn pick_sub(tracks: &[(i64, String)], lang: &str) -> Option<i64> {
    let base = |l: &str| l.split('-').next().unwrap_or(l).to_lowercase();
    tracks
        .iter()
        .find(|(_, l)| l.eq_ignore_ascii_case(lang))
        .or_else(|| tracks.iter().find(|(_, l)| base(l) == base(lang)))
        .or_else(|| tracks.iter().find(|(_, l)| l.to_lowercase().starts_with("en")))
        .or(tracks.first())
        .map(|(id, _)| *id)
}

fn keys(tracks: &[(i64, String, String)]) -> Vec<(i64, String)> {
    tracks.iter().map(|(id, k, _)| (*id, k.clone())).collect()
}

/// „en“ → „English“, „en-orig“ → „English (en-orig)“.
fn lang_label(l: &str) -> String {
    let name = match l.split('-').next() {
        Some("en") => "English",
        Some("de") => "German",
        _ => return if l.is_empty() { "Unknown".into() } else { l.to_string() },
    };
    if l.contains('-') { format!("{name} ({l})") } else { name.to_string() }
}

/// Lücke zwischen den Kapitel-Abschnitten der Timeline (px) und wie viel dicker der Abschnitt unter der Maus wird.
const CHAPTER_GAP: f64 = 1.0;
const CHAPTER_HOVER_GROW: f64 = 2.0;
/// Strg+← springt an den Anfang des aktuellen Kapitels; nur in den ersten Sekunden ins vorige (wie YouTube/mpv).
const CHAPTER_BACK_THRESHOLD: f64 = 3.0;

/// So lange (s) gilt das Ziel des letzten Kapitelsprungs als Position, solange mpv noch davor steht
/// (höchstens `CHAPTER_KEYFRAME_GAP` davor: so weit kann das Schlüsselbild vor dem Kapitelanfang liegen).
const CHAPTER_JUMP_HOLD: f64 = 15.0;
const CHAPTER_KEYFRAME_GAP: f64 = 12.0;

/// Position für den nächsten Kapitelsprung. Gesprungen wird zum Schlüsselbild (`hr-seek=no`), das liegt oft
/// ein paar Sekunden vor dem Kapitelanfang, also noch im alten Kapitel. Ohne Ausgleich zielte der nächste
/// Druck wieder auf dasselbe Kapitel. `last` = (Ziel des letzten Sprungs, Sekunden seitdem).
fn chapter_pos(pos: f64, last: Option<(f64, f64)>) -> f64 {
    match last {
        Some((target, age)) if age < CHAPTER_JUMP_HOLD && pos < target && target - pos < CHAPTER_KEYFRAME_GAP => target,
        _ => pos,
    }
}

/// Ziel-Kapitel (Index) für Strg+→ (`dir` > 0) bzw. Strg+← (`dir` < 0) von `pos` aus.
fn chapter_target(chapters: &[(f64, String)], pos: f64, dir: i32) -> Option<usize> {
    let cur = chapters.iter().rposition(|(start, _)| *start <= pos)?;
    if dir > 0 {
        return (cur + 1 < chapters.len()).then_some(cur + 1);
    }
    if pos - chapters[cur].0 > CHAPTER_BACK_THRESHOLD { Some(cur) } else { cur.checked_sub(1).or(Some(0)) }
}

/// Abschnitte der Timeline in Pixeln (`x0`, `x1`) bei Kapiteln: eine Lücke an jeder Kapitelgrenze.
fn chapter_spans(chapters: &[(f64, String)], dur: f64, left: f64, width: f64) -> Vec<(f64, f64)> {
    let x_at = |t: f64| left + width * (t / dur).clamp(0.0, 1.0);
    let starts: Vec<f64> = chapters.iter().map(|(s, _)| *s).filter(|s| *s > 0.0 && *s < dur).collect();
    let mut edges = vec![left];
    edges.extend(starts.iter().map(|s| x_at(*s)));
    edges.push(left + width);
    edges
        .windows(2)
        .enumerate()
        .map(|(i, w)| {
            let x0 = if i == 0 { w[0] } else { w[0] + CHAPTER_GAP / 2.0 };
            let x1 = if i + 2 == edges.len() { w[1] } else { w[1] - CHAPTER_GAP / 2.0 };
            (x0, x1.max(x0))
        })
        .collect()
}

/// Rechteck als Pfad, links mit Rundung `rl`, rechts mit `rr` (0 = eckig).
fn rounded(cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64, rl: f64, rr: f64) {
    let (rl, rr) = (rl.min(w / 2.0).min(h / 2.0), rr.min(w / 2.0).min(h / 2.0));
    let pi = std::f64::consts::PI;
    cr.new_sub_path();
    cr.arc(x + w - rr, y + rr, rr, -pi / 2.0, 0.0);
    cr.arc(x + w - rr, y + h - rr, rr, 0.0, pi / 2.0);
    cr.arc(x + rl, y + h - rl, rl, pi / 2.0, pi);
    cr.arc(x + rl, y + rl, rl, pi, 1.5 * pi);
    cr.close_path();
}

/// Kapitel, in dem `pos` liegt (Kapitel sind nach Startzeit sortiert).
fn chapter_at(chapters: &[(f64, String)], pos: f64) -> Option<&str> {
    chapters.iter().rev().find(|(start, _)| *start <= pos).map(|(_, t)| t.as_str())
}

pub struct Controls {
    pub widget: gtk::Revealer,
    /// Vorschau über der Timeline; liegt als eigene Ebene im Player (player.rs), nicht in der Leiste.
    pub preview: gtk::Box,
    pub fullscreen: gtk::Button,
    mpv: &'static Mpv,
    pub play: gtk::Button,
    seek: gtk::Scale,
    time: gtk::Label,
    vol: gtk::Scale,
    pub mute: gtk::Button,
    settings: gtk::MenuButton,
    /// Linksklick/Taste c: an/aus; Rechtsklick: Menü mit Sprache und Aussehen (`sub_menu`).
    pub subtitles: gtk::Button,
    sub_menu: gtk::Popover,
    sub_prefs: Rc<RefCell<SubPrefs>>,
    speed_buttons: Vec<(f64, gtk::CheckButton)>,
    /// Bekommt beim Ausblenden einen unsichtbaren Mauszeiger.
    cursor_target: gtk::Widget,
    hide_timer: RefCell<Option<glib::SourceId>>,
    chapters: Rc<RefCell<Vec<(f64, String)>>>,
    /// SponsorBlock-Abschnitte, als farbige Bereiche über dem Fortschrittsbalken gezeichnet.
    segments: Rc<RefCell<Vec<Segment>>>,
    /// Bis hierhin (Sekunden) ist das Video vorgeladen; wird hell in der Timeline gezeigt.
    cached_until: Rc<Cell<f64>>,
    /// Vorschau beim Überfahren der Timeline: Bild aus dem Storyboard plus Zeit/Kapitel.
    storyboard: Rc<RefCell<Option<Storyboard>>>,
    /// Geladene Storyboard-Bögen (None = lädt gerade).
    sheets: Rc<RefCell<HashMap<usize, Option<Pixbuf>>>>,
    /// Wird mit der neuen Höhe aufgerufen, wenn die Qualität geändert wird (Player lädt neu).
    on_quality: Rc<RefCell<Option<Box<dyn Fn(u32)>>>>,
    segment_layer: gtk::DrawingArea,
    pos: Cell<f64>,
    /// Ziel eines Timeline-Sprungs und wann er an mpv ging (`None` = wartet noch auf `SEEK_DELAY`).
    seek_target: Rc<Cell<Option<(f64, Option<Instant>)>>>,
    dur: Cell<f64>,
    speed: Cell<f64>,
    /// Ton aus (von mpv gemeldet) und laufende Ein-/Ausblendung bzw. Lautstärke-Rampe (Nummer, damit eine
    /// neuere die ältere abbricht).
    muted: Cell<bool>,
    fade_gen: Cell<u32>,
    vol_gen: Cell<u32>,
    vol_target: Cell<Option<f64>>,
    live: Cell<bool>,
    paused: Cell<bool>,
    /// Ziel und Zeitpunkt des letzten Kapitelsprungs (siehe `chapter_pos`).
    last_chapter_jump: Cell<Option<(f64, std::time::Instant)>>,
}

fn icon_button(icon: &str, tip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tip)
        .css_classes(["flat"])
        // Kein Fokus, sonst würde die Leertaste den zuletzt geklickten Button auslösen.
        .focusable(false)
        .build()
}

/// Abstand der Vorschau über der Timeline-Schiene.
const PREVIEW_GAP: f32 = 24.0;

/// Vorschau mittig über der Maus (nicht über die Enden der Timeline hinaus) und immer gleich hoch über
/// der Schiene. Gerechnet in der Ebene, in der sie liegt: so verschieben sie weder Kapitel-Striche
/// unter der Schiene noch eine höhere Vorschau.
fn place_preview(preview: &gtk::Box, seek: &gtk::Scale, trough: &gtk::Widget, x: f64) {
    let Some(layer) = preview.parent() else { return };
    let (Some(tr), Some(sk)) = (trough.compute_bounds(&layer), seek.compute_bounds(&layer)) else { return };
    let w = preview.width().max(preview.first_child().map_or(0, |c| c.width_request())).max(60) as f64;
    let lo = sk.x() as f64;
    let hi = (lo + sk.width() as f64 - w).max(lo);
    preview.set_margin_start((lo + x - w / 2.0).clamp(lo, hi) as i32);
    preview.set_margin_bottom((layer.height() as f32 - tr.y() + PREVIEW_GAP).max(0.0) as i32);
}

fn slider(max: f64) -> gtk::Scale {
    let s = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, max, 1.0);
    s.set_draw_value(false);
    s.set_focusable(false);
    s
}

/// Spalte im Einstellungsmenü: Überschrift und Auswahlknöpfe (nur einer aktiv).
fn radio_column(title: &str, labels: &[String]) -> (gtk::Box, Vec<gtk::CheckButton>) {
    let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
    col.append(&gtk::Label::builder().label(title).xalign(0.0).margin_bottom(4).css_classes(["heading"]).build());
    let mut buttons: Vec<gtk::CheckButton> = Vec::new();
    for l in labels {
        let b = gtk::CheckButton::builder().label(l).focusable(false).build();
        if let Some(first) = buttons.first() {
            b.set_group(Some(first));
        }
        col.append(&b);
        buttons.push(b);
    }
    (col, buttons)
}

impl Controls {
    pub fn new(mpv: &'static Mpv, cursor_target: &impl IsA<gtk::Widget>) -> Rc<Self> {
        let play = icon_button("qu-pause-symbolic", "Pause (space)");
        play.connect_clicked(move |_| {
            let _ = mpv.command("cycle", &["pause"]);
        });

        let seek = slider(1.0);
        seek.set_hexpand(true);

        let mute = icon_button("qu-volume-symbolic", "Mute (m)");
        let vol = slider(MAX_VOLUME);
        vol.set_width_request(90);
        vol.set_margin_end(8);
        vol.connect_change_value(move |_, _, v| {
            let _ = mpv.set_property("volume", v.clamp(0.0, MAX_VOLUME));
            glib::Propagation::Proceed
        });

        let time = gtk::Label::builder()
            .css_classes(["numeric"])
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        let fullscreen = icon_button("qu-fullscreen-symbolic", "Full screen (f)");

        // Timeline: change-value kommt nur von der Maus, nicht von set_value (keine Rückkopplung).
        // Beim Ziehen nicht bei jeder Bewegung springen (jeder Sprung lädt neu), sondern erst,
        // wenn die Maus kurz still steht oder losgelassen wird. Die Zeit folgt sofort.
        let pending_seek: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
        // Sprungziel, bis mpv dort ankommt: so lange zeigt die Timeline nicht die alte Position (`seek_state`).
        let seek_target: Rc<Cell<Option<(f64, Option<Instant>)>>> = Rc::default();
        seek.connect_change_value({
            let time = time.downgrade();
            let seek_target = seek_target.clone();
            move |s, _, v| {
                seek_target.set(Some((v, None)));
                if let Some(t) = time.upgrade() {
                    t.set_text(&format!("{} / {}", fmt_duration(v.max(0.0) as u32), fmt_duration(s.adjustment().upper() as u32)));
                }
                if let Some(id) = pending_seek.take() {
                    id.remove();
                }
                let (pending, seek_target) = (pending_seek.clone(), seek_target.clone());
                let id = glib::timeout_add_local_once(SEEK_DELAY, move || {
                    pending.take();
                    seek_target.set(Some((v, Some(Instant::now()))));
                    let _ = mpv.command("seek", &[&v.to_string(), "absolute"]);
                });
                pending_seek.replace(Some(id));
                glib::Propagation::Proceed
            }
        });

        // Einstellungen: Qualität und Geschwindigkeit nebeneinander.
        let (quality_col, quality_buttons) =
            radio_column("Quality", &qualities().into_iter().map(|(_, l)| l).collect::<Vec<_>>());
        let (speed_col, speed_buttons) = radio_column("Playback speed", &SPEEDS.map(speed_label));
        let menu = gtk::Box::builder().spacing(24).margin_top(8).margin_bottom(8).margin_start(8).margin_end(8).build();
        menu.append(&quality_col);
        menu.append(&speed_col);
        // Upscaling (upscale.rs): wirkt sofort, auch beim laufenden Video.
        let (upscale_col, upscale_buttons) = radio_column("Upscaling", &upscaler().modes().map(|(_, l)| l.to_string()));
        crate::upscale::apply(mpv, crate::upscale::saved(), upscaler());
        for ((mode, _), b) in upscaler().modes().iter().zip(&upscale_buttons) {
            b.set_active(*mode == crate::upscale::saved());
            let mode = *mode;
            b.connect_toggled(move |b| {
                if b.is_active() {
                    crate::upscale::set(mpv, mode, upscaler());
                }
            });
        }
        menu.append(&upscale_col);
        let settings = gtk::MenuButton::builder()
            .icon_name("qu-settings-symbolic")
            .tooltip_text("Settings")
            .direction(gtk::ArrowType::Up)
            .popover(&gtk::Popover::builder().child(&menu).build())
            .focusable(false)
            .css_classes(["flat"])
            .build();

        let saved = saved_quality();
        let on_quality: Rc<RefCell<Option<Box<dyn Fn(u32)>>>> = Rc::default();
        for ((height, _), b) in qualities().iter().zip(&quality_buttons) {
            b.set_active(*height == saved);
            let (height, on_quality) = (*height, on_quality.clone());
            b.connect_toggled(move |b| {
                if !b.is_active() {
                    return;
                }
                let _ = std::fs::create_dir_all(quality_file().parent().unwrap());
                let _ = std::fs::write(quality_file(), height.to_string());
                // Neue Qualität: der Player lädt an derselben Stelle neu (`connect_quality`).
                if let Some(f) = on_quality.borrow().as_ref() {
                    f(height);
                }
            });
        }
        let speed_buttons: Vec<(f64, gtk::CheckButton)> = SPEEDS.into_iter().zip(speed_buttons).collect();
        for (speed, b) in &speed_buttons {
            b.set_active(*speed == 1.0);
            let speed = *speed;
            b.connect_toggled(move |b| {
                if b.is_active() {
                    let _ = mpv.set_property("speed", speed);
                }
            });
        }

        // Jede Gruppe in einer eigenen Bubble.
        let bubble = |widgets: &[&gtk::Widget]| {
            let b = gtk::Box::builder().spacing(2).valign(gtk::Align::Center).css_classes(["bubble"]).build();
            for w in widgets {
                b.append(*w);
            }
            b
        };
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.append(&bubble(&[play.upcast_ref()]));
        buttons.append(&bubble(&[mute.upcast_ref(), vol.upcast_ref()]));
        buttons.append(&bubble(&[time.upcast_ref()]));
        buttons.append(&gtk::Box::builder().hexpand(true).build());
        // Untertitel: Linksklick an/aus (verbunden in player.rs, wegen der Einblendung), Rechtsklick = Menü.
        let subtitles = icon_button("qu-subtitles-symbolic", "Subtitles (c) · right-click for options");
        let sub_prefs = Rc::new(RefCell::new(SubPrefs::load()));
        sub_prefs.borrow().apply(mpv);
        let sub_tracks_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let (size_col, size_buttons) = radio_column("Size", &SUB_SIZES.map(|(_, l)| l.to_string()));
        let (style_col, style_buttons) = radio_column("Background", &["Outline".to_string(), "Box".to_string()]);
        for ((size, _), b) in SUB_SIZES.iter().zip(&size_buttons) {
            b.set_active(*size == sub_prefs.borrow().size);
            let (size, prefs) = (*size, sub_prefs.clone());
            b.connect_toggled(move |b| {
                if b.is_active() {
                    prefs.borrow_mut().size = size;
                    prefs.borrow().apply(mpv);
                    prefs.borrow().save();
                }
            });
        }
        for (boxed, b) in [false, true].into_iter().zip(&style_buttons) {
            b.set_active(boxed == sub_prefs.borrow().boxed);
            let prefs = sub_prefs.clone();
            b.connect_toggled(move |b| {
                if b.is_active() {
                    prefs.borrow_mut().boxed = boxed;
                    prefs.borrow().apply(mpv);
                    prefs.borrow().save();
                }
            });
        }
        let sub_menu_box = gtk::Box::builder().spacing(24).margin_top(8).margin_bottom(8).margin_start(8).margin_end(8).build();
        sub_menu_box.append(&sub_tracks_box);
        sub_menu_box.append(&size_col);
        sub_menu_box.append(&style_col);
        let sub_menu = gtk::Popover::builder().child(&sub_menu_box).position(gtk::PositionType::Top).build();
        sub_menu.set_parent(&subtitles);
        let right = gtk::GestureClick::builder().button(3).build();
        right.connect_pressed({
            let (menu, holder, prefs) = (sub_menu.clone(), sub_tracks_box.clone(), sub_prefs.clone());
            move |_, _, _, _| {
                // Spuren erst beim Öffnen lesen: jedes Video hat andere.
                while let Some(c) = holder.first_child() {
                    holder.remove(&c);
                }
                let tracks = sub_tracks(mpv);
                if tracks.is_empty() {
                    holder.append(&gtk::Label::builder().label("No subtitles for this video").css_classes(["dim-label"]).build());
                } else {
                    let labels: Vec<String> = tracks.iter().map(|(_, _, l)| l.clone()).collect();
                    let (col, buttons) = radio_column("Language", &labels);
                    let current = mpv.get_property::<i64>("sid").ok();
                    for ((id, lang, _), b) in tracks.into_iter().zip(&buttons) {
                        b.set_active(Some(id) == current);
                        let prefs = prefs.clone();
                        b.connect_toggled(move |b| {
                            if b.is_active() {
                                let _ = mpv.set_property("sid", id);
                                let mut p = prefs.borrow_mut();
                                (p.lang, p.on) = (lang.clone(), true);
                                p.save();
                            }
                        });
                    }
                    holder.append(&col);
                }
                menu.popup();
            }
        });
        subtitles.add_controller(right);
        buttons.append(&bubble(&[subtitles.upcast_ref(), settings.upcast_ref(), fullscreen.upcast_ref()]));

        // Vorschau über der Timeline (Bild + Zeit), folgt der Maus. Liegt in der Leiste über dem Balken;
        // unsichtbar nimmt sie keinen Platz ein, sichtbar wächst die (unten verankerte) Leiste nach oben.
        let preview_pic = gtk::Picture::builder().overflow(gtk::Overflow::Hidden).can_shrink(true).content_fit(gtk::ContentFit::Fill).css_classes(["preview"]).visible(false).build();
        let preview_time = gtk::Label::builder().css_classes(["osd", "pill", "numeric"]).halign(gtk::Align::Center).max_width_chars(40).ellipsize(gtk::pango::EllipsizeMode::End).build();
        let preview = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .halign(gtk::Align::Start)
            .valign(gtk::Align::End)
            .can_target(false)
            .visible(false)
            .build();
        preview.append(&preview_pic);
        preview.append(&preview_time);
        let storyboard: Rc<RefCell<Option<Storyboard>>> = Rc::default();
        let chapters: Rc<RefCell<Vec<(f64, String)>>> = Rc::default();
        let sheets: Rc<RefCell<HashMap<usize, Option<Pixbuf>>>> = Rc::default();

        let bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_start(16)
            .margin_end(16)
            .margin_bottom(14)
            .css_classes(["player-controls"])
            .build();
        seek.add_css_class("timeline");

        // Farbige Sponsor-Bereiche: eigene Zeichenfläche über dem Balken, genau über dessen Schiene.
        let segments: Rc<RefCell<Vec<Segment>>> = Rc::default();
        let cached_until: Rc<Cell<f64>> = Rc::default();
        let segment_layer = gtk::DrawingArea::builder().can_target(false).build();
        // Mausposition über der Timeline (für den dickeren Kapitel-Abschnitt).
        let hover_x: Rc<Cell<Option<f64>>> = Rc::default();
        segment_layer.set_draw_func({
            let (segments, cached_until, seek, chapters, hover_x) = (segments.clone(), cached_until.clone(), seek.downgrade(), chapters.clone(), hover_x.clone());
            move |area, cr, _, _| {
                let Some(seek) = seek.upgrade() else { return };
                let segs = segments.borrow();
                let dur = seek.adjustment().upper();
                // Die Schiene ("trough") ist ein internes Kind des Reglers; ihre Lage bestimmt, wo gezeichnet wird.
                let mut child = seek.first_child();
                while let Some(c) = child.as_ref().filter(|c| c.css_name() != "trough") {
                    child = c.next_sibling();
                }
                let (Some(trough), true) = (child, dur > 1.0) else { return };
                let Some(r) = trough.compute_bounds(area) else { return };
                let (h, y) = (r.height() as f64, r.y() as f64);
                let x_at = |t: f64| r.x() as f64 + r.width() as f64 * (t / dur).clamp(0.0, 1.0);
                let (pos, until) = (seek.value(), cached_until.get());
                // Mit Kapiteln (Klasse `chapters`, Schiene per CSS unsichtbar): Timeline selbst zeichnen, ein
                // Abschnitt pro Kapitel mit Lücke dazwischen wie bei YouTube; alles Weitere nur in diesen Abschnitten.
                if seek.has_css_class("chapters") {
                    let spans = chapter_spans(&chapters.borrow(), dur, r.x() as f64, r.width() as f64);
                    let hovered = hover_x.get();
                    // Rund nur an Anfang und Ende der ganzen Timeline, an Kapitelgrenzen eckig.
                    let last = spans.len().saturating_sub(1);
                    for (i, (x0, x1)) in spans.iter().enumerate() {
                        let grow = if hovered.is_some_and(|x| x >= *x0 - CHAPTER_GAP / 2.0 && x <= *x1 + CHAPTER_GAP / 2.0) { CHAPTER_HOVER_GROW } else { 0.0 };
                        let (hh, r) = (h + 2.0 * grow, (h + 2.0 * grow) / 2.0);
                        rounded(cr, *x0, y - grow, x1 - x0, hh, if i == 0 { r } else { 0.0 }, if i == last { r } else { 0.0 });
                    }
                    cr.clip();
                    let (top, height) = (y - CHAPTER_HOVER_GROW, h + 2.0 * CHAPTER_HOVER_GROW);
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.3);
                    cr.rectangle(r.x() as f64, top, r.width() as f64, height);
                    let _ = cr.fill();
                    // Gespielter Teil weiß (wie `highlight`).
                    cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                    cr.rectangle(r.x() as f64, top, x_at(pos) - r.x() as f64, height);
                    let _ = cr.fill();
                }
                // Vorgeladener Bereich (aktuelle Stelle bis Pufferende) heller, wie bei YouTube:
                // dorthin springen geht ohne Warten.
                if until > pos {
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.3);
                    cr.rectangle(x_at(pos), y, x_at(until) - x_at(pos), h);
                    let _ = cr.fill();
                }
                // Dezent in der Schriftfarbe des Themes, keine Signalfarben.
                let c = seek.color();
                for s in segs.iter() {
                    let x0 = r.x() as f64 + r.width() as f64 * (s.start / dur).clamp(0.0, 1.0);
                    let x1 = r.x() as f64 + r.width() as f64 * (s.end / dur).clamp(0.0, 1.0);
                    cr.set_source_rgba(c.red().into(), c.green().into(), c.blue().into(), 0.45);
                    cr.rectangle(x0, y, (x1 - x0).max(2.0), h);
                    let _ = cr.fill();
                }
            }
        });
        // Gespielter Teil wird mitgezeichnet: bei jeder Änderung (Abspielen, Ziehen) neu.
        seek.connect_value_changed({
            let layer = segment_layer.downgrade();
            move |s| {
                if let Some(l) = layer.upgrade().filter(|_| s.has_css_class("chapters")) {
                    l.queue_draw();
                }
            }
        });
        let seek_row = gtk::Overlay::builder().child(&seek).build();
        seek_row.add_overlay(&segment_layer);
        bar.append(&seek_row);

        let hover = gtk::EventControllerMotion::new();
        hover.connect_motion({
            let (hover_x, layer) = (hover_x.clone(), segment_layer.downgrade());
            move |_, x, _| {
                hover_x.set(Some(x));
                if let Some(l) = layer.upgrade() {
                    l.queue_draw();
                }
            }
        });
        hover.connect_leave({
            let (hover_x, layer) = (hover_x.clone(), segment_layer.downgrade());
            move |_| {
                hover_x.set(None);
                if let Some(l) = layer.upgrade() {
                    l.queue_draw();
                }
            }
        });
        hover.connect_motion({
            let (preview, pic, label, seek, storyboard, sheets, chapters) = (
                preview.clone(),
                preview_pic.clone(),
                preview_time.clone(),
                seek.downgrade(),
                storyboard.clone(),
                sheets.clone(),
                chapters.clone(),
            );
            move |_, x, _| {
                let Some(seek) = seek.upgrade() else { return };
                let dur = seek.adjustment().upper();
                // Zeit unter der Maus: Lage der Schiene ("trough") im Regler.
                let mut child = seek.first_child();
                while let Some(c) = child.as_ref().filter(|c| c.css_name() != "trough") {
                    child = c.next_sibling();
                }
                let (Some(trough), true) = (child, dur > 1.0) else { return };
                let Some(r) = trough.compute_bounds(&seek) else { return };
                let t = ((x - r.x() as f64) / r.width() as f64).clamp(0.0, 1.0) * dur;
                // Zeit und – falls vorhanden – Kapitel an dieser Stelle.
                match chapter_at(&chapters.borrow(), t).filter(|c| !c.is_empty()) {
                    Some(c) => label.set_text(&format!("{} · {c}", fmt_duration(t as u32))),
                    None => label.set_text(&fmt_duration(t as u32)),
                }
                show_frame(&pic, &storyboard, &sheets, t);
                place_preview(&preview, &seek, &trough, x);
                preview.set_visible(true);
            }
        });
        hover.connect_leave({
            let preview = preview.clone();
            move |_| preview.set_visible(false)
        });
        seek.add_controller(hover);
        bar.append(&buttons);

        let widget = gtk::Revealer::builder()
            .child(&bar)
            .transition_type(gtk::RevealerTransitionType::Crossfade)
            .reveal_child(true)
            .valign(gtk::Align::End)
            .build();

        let c = Rc::new(Controls {
            widget,
            preview: preview.clone(),
            fullscreen,
            mpv,
            play,
            seek,
            time,
            vol,
            mute,
            settings,
            subtitles,
            sub_menu,
            sub_prefs,
            speed_buttons,
            cursor_target: cursor_target.clone().upcast(),
            hide_timer: RefCell::default(),
            chapters,
            segments,
            segment_layer,
            cached_until,
            storyboard,
            sheets,
            on_quality,
            pos: Cell::new(0.0),
            seek_target,
            dur: Cell::new(0.0),
            speed: Cell::new(1.0),
            muted: Cell::new(false),
            fade_gen: Cell::new(0),
            vol_gen: Cell::new(0),
            vol_target: Cell::new(None),
            live: Cell::new(false),
            paused: Cell::new(false),
            last_chapter_jump: Cell::new(None),
        });
        c.mute.connect_clicked({
            let c = Rc::downgrade(&c);
            move |_| {
                if let Some(c) = c.upgrade() {
                    c.toggle_mute();
                }
            }
        });
        c
    }

    /// Leiste zeigen und das Ausblenden neu starten (bei Mausbewegung, Taste, Pause …).
    pub fn poke(self: &Rc<Self>) {
        self.widget.set_reveal_child(true);
        self.cursor_target.set_cursor(None);
        self.cursor_target.remove_css_class("idle");
        if let Some(t) = self.hide_timer.take() {
            t.remove();
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(HIDE_AFTER, move || {
            let Some(c) = weak.upgrade() else { return };
            c.hide_timer.take(); // Timer ist abgelaufen, nicht mehr entfernen.
            // Maus ruht: Hover-Hervorhebung aus (style.css `.idle`), auch wenn die Leiste sichtbar bleibt.
            c.cursor_target.add_css_class("idle");
            // Pausiert oder bei offenem Einstellungsmenü sichtbar bleiben.
            if !c.paused.get() && !c.menu_open() {
                c.widget.set_reveal_child(false);
                c.cursor_target.set_cursor_from_name(Some("none"));
            }
        });
        self.hide_timer.replace(Some(id));
    }

    fn menu_open(&self) -> bool {
        self.settings.is_active() || self.sub_menu.is_visible()
    }

    /// Untertitel an/aus (Linksklick, Taste c), wird für die nächsten Videos gemerkt. Beim Einschalten die
    /// gewählte Sprache. `false` = das Video hat keine Untertitel.
    pub fn toggle_subs(&self) -> bool {
        let mut p = self.sub_prefs.borrow_mut();
        if self.mpv.get_property::<String>("sid").is_ok_and(|s| s != "no") {
            let _ = self.mpv.set_property("sid", "no");
            p.on = false;
            p.save();
            return true;
        }
        let Some(id) = pick_sub(&keys(&sub_tracks(self.mpv)), &p.lang) else { return false };
        p.on = true;
        p.save();
        self.mpv.set_property("sid", id).is_ok()
    }

    /// Neues Video geladen: Untertitel wie zuletzt (aus bzw. in der gewählten Sprache).
    pub fn apply_subs(&self) {
        let p = self.sub_prefs.borrow();
        let id = if p.on { pick_sub(&keys(&sub_tracks(self.mpv)), &p.lang) } else { None };
        let _ = match id {
            Some(id) => self.mpv.set_property("sid", id),
            None => self.mpv.set_property("sid", "no"),
        };
    }

    /// Oberkante der Timeline in den Koordinaten von `target` (z. B. für Knöpfe direkt darüber).
    /// `None`, solange die Leiste nicht angeordnet ist.
    pub fn timeline_top(&self, target: &impl IsA<gtk::Widget>) -> Option<f32> {
        self.seek.compute_bounds(target).map(|r| r.y()).filter(|y| *y > 0.0)
    }

    /// Qualität im Menü geändert: `f(höhe)` lädt das Video in der neuen Stufe an derselben Stelle.
    pub fn connect_quality(&self, f: impl Fn(u32) + 'static) {
        self.on_quality.replace(Some(Box::new(f)));
    }

    /// Sofort ausblenden (Maus hat den Player verlassen). Bei offenem Einstellungsmenü nicht.
    pub fn hide(&self) {
        if self.menu_open() {
            return;
        }
        if let Some(t) = self.hide_timer.take() {
            t.remove();
        }
        self.widget.set_reveal_child(false);
    }

    /// Neues Video: Anzeige zurücksetzen. Bei Livestreams gibt es keinen Fortschrittsbalken.
    pub fn reset(self: &Rc<Self>, live: bool) {
        self.live.set(live);
        self.pos.set(0.0);
        self.dur.set(0.0);
        self.chapters.borrow_mut().clear();
        self.cached_until.set(0.0);
        self.storyboard.replace(None);
        self.sheets.borrow_mut().clear();
        self.set_segments(vec![]);
        self.seek.remove_css_class("chapters");
        self.seek.set_visible(!live);
        self.seek.set_value(0.0);
        self.update_time();
        self.poke();
    }

    /// Storyboard des laufenden Videos (aus den yt-dlp-Infos); ohne zeigt die Vorschau nur die Zeit.
    pub fn set_storyboard(&self, sb: Option<Storyboard>) {
        if *self.storyboard.borrow() != sb {
            self.sheets.borrow_mut().clear();
            self.storyboard.replace(sb);
        }
    }

    pub fn set_segments(&self, segments: Vec<Segment>) {
        self.segments.replace(segments);
        self.segment_layer.queue_draw();
    }

    /// Kapitel aus mpv übernehmen (yt-dlp liefert sie beim Laden mit) und im Balken markieren.
    pub fn load_chapters(&self) {
        let count = self.mpv.get_property::<i64>("chapter-list/count").unwrap_or(0);
        let chapters: Vec<(f64, String)> = (0..count)
            .filter_map(|i| {
                let start = self.mpv.get_property::<f64>(&format!("chapter-list/{i}/time")).ok()?;
                let title = self.mpv.get_property::<String>(&format!("chapter-list/{i}/title")).unwrap_or_default();
                Some((start, title))
            })
            .collect();
        // Ab zwei Kapiteln: Timeline in Abschnitte geteilt (siehe Zeichenfläche `segment_layer`).
        if chapters.len() > 1 { self.seek.add_css_class("chapters") } else { self.seek.remove_css_class("chapters") }
        self.chapters.replace(chapters);
        self.segment_layer.queue_draw();
        self.update_time();
    }

    /// Anderer Sprung (Pfeiltasten, Ziffern): gemerktes Kapitelziel gilt nicht mehr.
    pub fn forget_chapter_jump(&self) {
        self.last_chapter_jump.set(None);
    }

    pub fn has_chapters(&self) -> bool {
        self.chapters.borrow().len() > 1
    }

    /// Zum nächsten (`dir` > 0) bzw. vorigen Kapitel springen (Strg+→/←). Gibt den Kapiteltitel zurück
    /// (leer, wenn das Kapitel keinen hat); `None` = keine Kapitel bzw. kein weiteres.
    pub fn chapter_jump(&self, dir: i32) -> Option<String> {
        let chapters = self.chapters.borrow();
        let last = self.last_chapter_jump.get().map(|(t, at)| (t, at.elapsed().as_secs_f64()));
        let i = chapter_target(&chapters, chapter_pos(self.pos.get(), last), dir)?;
        let target = chapters[i].0;
        let _ = self.mpv.command("seek", &[&target.to_string(), "absolute"]);
        self.last_chapter_jump.set(Some((target, std::time::Instant::now())));
        Some(chapters[i].1.clone())
    }

    /// Geschwindigkeit eine Stufe schneller (+1) oder langsamer (-1), wie `>` / `<` bei YouTube.
    pub fn step_speed(&self, dir: i32) {
        let i = SPEEDS.iter().position(|s| *s == self.speed.get()).unwrap_or(3) as i32;
        let next = SPEEDS[(i + dir).clamp(0, SPEEDS.len() as i32 - 1) as usize];
        let _ = self.mpv.set_property("speed", next);
    }

    /// Ton weich aus- bzw. einblenden statt hart umzuschalten (Filter `@fade` in player.rs; die
    /// Lautstärke selbst bleibt dabei unberührt). Knopf, Taste `m` und Miniplayer.
    pub fn toggle_mute(self: &Rc<Self>) {
        let muting = !self.muted.get();
        let generation = self.fade_gen.get() + 1;
        self.fade_gen.set(generation);
        if !muting {
            let _ = self.mpv.command("af-command", &["fade", "volume", "0"]);
            let _ = self.mpv.set_property("mute", false);
        }
        for i in 1..=FADE_STEPS {
            let me = self.clone();
            glib::timeout_add_local_once(FADE_STEP * i, move || {
                if me.fade_gen.get() != generation {
                    return;
                }
                let f = i as f64 / FADE_STEPS as f64;
                let level = if muting { 1.0 - f } else { f };
                let _ = me.mpv.command("af-command", &["fade", "volume", &format!("{level:.3}")]);
                if muting && i == FADE_STEPS {
                    let _ = me.mpv.set_property("mute", true);
                    let _ = me.mpv.command("af-command", &["fade", "volume", "1"]);
                }
            });
        }
    }

    /// Lautstärke um `delta` ändern, in kleinen Schritten statt eines Sprungs (Tasten `+`/`-`).
    pub fn step_volume(self: &Rc<Self>, delta: f64) {
        let from = self.vol_target.get().unwrap_or(self.vol.value());
        let to = (from + delta).clamp(0.0, MAX_VOLUME);
        let start = self.vol.value();
        let generation = self.vol_gen.get() + 1;
        self.vol_gen.set(generation);
        self.vol_target.set(Some(to));
        for i in 1..=FADE_STEPS {
            let me = self.clone();
            glib::timeout_add_local_once(FADE_STEP * i, move || {
                if me.vol_gen.get() != generation {
                    return;
                }
                let v = start + (to - start) * i as f64 / FADE_STEPS as f64;
                let _ = me.mpv.set_property("volume", v.round());
                if i == FADE_STEPS {
                    me.vol_target.set(None);
                }
            });
        }
    }

    /// Wird für jede beobachtete mpv-Eigenschaft aufgerufen, wenn sie sich ändert.
    pub fn update(self: &Rc<Self>, name: &str, data: PropertyData) {
        match (name, data) {
            ("time-pos", PropertyData::Double(t)) => {
                self.pos.set(t);
                // Nach einem Klick in die Timeline: alte Positionen nicht anzeigen, bis mpv am Ziel ist.
                if let Some((target, sent)) = self.seek_target.get() {
                    match seek_state(t, target, sent.map(|s| s.elapsed())) {
                        Seek::Hold => return,
                        Seek::Show => {}
                        Seek::Done => self.seek_target.set(None),
                    }
                }
                // Während der Nutzer den Regler zieht, nicht dazwischenfunken.
                if !self.seek.state_flags().contains(gtk::StateFlags::ACTIVE) {
                    self.seek.set_value(t);
                }
                self.update_time();
            }
            ("demuxer-cache-time", PropertyData::Double(t)) => {
                self.cached_until.set(t);
                self.segment_layer.queue_draw();
            }
            ("duration", PropertyData::Double(d)) => {
                self.dur.set(d);
                self.seek.set_range(0.0, d.max(1.0));
                self.segment_layer.queue_draw();
                self.update_time();
            }
            ("pause", PropertyData::Flag(p)) => {
                self.paused.set(p);
                let (icon, tip) = if p {
                    ("qu-play-symbolic", "Play (space)")
                } else {
                    ("qu-pause-symbolic", "Pause (space)")
                };
                self.play.set_icon_name(icon);
                self.play.set_tooltip_text(Some(tip));
                self.poke();
            }
            ("speed", PropertyData::Double(s)) => {
                self.speed.set(s);
                if let Some((_, b)) = self.speed_buttons.iter().find(|(v, _)| *v == s) {
                    b.set_active(true);
                }
                self.update_time();
                self.poke();
            }
            // Jede Änderung (Regler, Pfeiltasten) merken.
            ("volume", PropertyData::Double(v)) => {
                self.vol.set_value(v);
                save_volume(v);
            }
            ("mute", PropertyData::Flag(m)) => {
                self.muted.set(m);
                self.mute.set_icon_name(if m { "qu-mute-symbolic" } else { "qu-volume-symbolic" });
            }
            _ => {}
        }
    }

    fn update_time(&self) {
        // Während an der Timeline gezogen wird, zeigt die Zeit das Ziel (siehe change-value).
        if self.seek.state_flags().contains(gtk::StateFlags::ACTIVE) {
            return;
        }
        let mut parts = vec![if self.live.get() {
            "● LIVE".to_string()
        } else {
            format!("{} / {}", fmt_duration(self.pos.get() as u32), fmt_duration(self.dur.get() as u32))
        }];
        if let Some(c) = chapter_at(&self.chapters.borrow(), self.pos.get()).filter(|c| !c.is_empty()) {
            parts.push(c.to_string());
        }
        if self.speed.get() != 1.0 {
            parts.push(speed_label(self.speed.get()));
        }
        let t = parts.join(" · ");
        if self.time.text() != t {
            self.time.set_text(&t);
        }
    }
}

/// Was die Timeline nach einem Sprung zu `target` mit der gemeldeten Position `pos` macht.
#[derive(Debug, PartialEq)]
enum Seek {
    /// Ziel weiter zeigen (alte Position oder Schlüsselbild vor dem Ziel).
    Hold,
    /// `pos` zeigen, Ziel aber gemerkt lassen: mpv meldet nach dem Sprung erst das Ziel, dann das Schlüsselbild davor.
    Show,
    /// Sprung erledigt, wieder normal.
    Done,
}

/// Noch nicht an mpv gesendet (`since_sent` = `None`): Ziel zeigen. mpv springt zum Schlüsselbild vor dem Ziel
/// (`hr-seek=no`) und spielt von dort hin: bis 1 s hinter dem Ziel nie etwas davor zeigen (spätestens nach 15 s
/// aufgeben). Weit weg (alte Position vor dem Sprung): nur 2 s ignorieren – danach ein anderer Sprung (Tasten).
fn seek_state(pos: f64, target: f64, since_sent: Option<Duration>) -> Seek {
    let Some(d) = since_sent else { return Seek::Hold };
    if pos >= target + 1.0 {
        Seek::Done
    } else if pos >= target {
        Seek::Show
    } else if pos >= target - 10.0 {
        if d > Duration::from_secs(15) { Seek::Done } else { Seek::Hold }
    } else if d > Duration::from_secs(2) {
        Seek::Done
    } else {
        Seek::Hold
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn seek_holds_target() {
        use super::{seek_state, Seek};
        use std::time::Duration;
        let ms = |n| Some(Duration::from_millis(n));
        assert_eq!(seek_state(30.0, 90.0, None), Seek::Hold); // noch nicht gesendet
        assert_eq!(seek_state(30.0, 90.0, ms(100)), Seek::Hold); // alte Position
        assert_eq!(seek_state(90.0, 90.0, ms(0)), Seek::Show); // mpv meldet erst das Ziel …
        assert_eq!(seek_state(83.4, 90.0, ms(50)), Seek::Hold); // … dann das Schlüsselbild davor: Ziel halten
        assert_eq!(seek_state(89.8, 90.0, ms(3500)), Seek::Hold); // knapp davor: würde als 1:29 erscheinen
        assert_eq!(seek_state(90.3, 90.0, ms(3500)), Seek::Show); // am Ziel angekommen
        assert_eq!(seek_state(91.2, 90.0, ms(4500)), Seek::Done); // darüber hinaus: fertig
        assert_eq!(seek_state(30.0, 90.0, ms(2500)), Seek::Done); // weit weg nach 2 s: anderer Sprung
        assert_eq!(seek_state(86.5, 90.0, ms(16000)), Seek::Done); // hängt: aufgeben
    }
    use super::*;

    #[test]
    fn chapters_speed_quality() {
        let ch = vec![(0.0, "Intro".to_string()), (60.0, "Teil 1".to_string()), (300.0, "Ende".to_string())];
        assert_eq!(chapter_at(&ch, 10.0), Some("Intro"));
        assert_eq!(chapter_at(&ch, 60.0), Some("Teil 1"));
        assert_eq!(chapter_at(&ch, 999.0), Some("Ende"));
        assert_eq!(chapter_at(&[], 5.0), None);
        // Strg+→: nächstes Kapitel; Strg+←: Anfang des aktuellen, in den ersten 3 s das vorige.
        assert_eq!(chapter_target(&ch, 10.0, 1), Some(1));
        assert_eq!(chapter_target(&ch, 400.0, 1), None);
        assert_eq!(chapter_target(&ch, 100.0, -1), Some(1));
        assert_eq!(chapter_target(&ch, 61.0, -1), Some(0));
        assert_eq!(chapter_target(&ch, 1.0, -1), Some(0));
        assert_eq!(chapter_target(&[], 5.0, 1), None);
        // Nach einem Sprung auf 60 s landet mpv beim Schlüsselbild 57,5 s: zählt als 60 s, damit der nächste
        // Druck weiterspringt (→ Kapitel 2) bzw. zurück ins vorige (← Kapitel 0).
        let p = chapter_pos(57.5, Some((60.0, 0.3)));
        assert_eq!(chapter_target(&ch, p, 1), Some(2));
        assert_eq!(chapter_target(&ch, p, -1), Some(0));
        // Weitergespielt, zu alt oder weit weg gesprungen: echte Position.
        assert_eq!(chapter_pos(62.0, Some((60.0, 0.3))), 62.0);
        assert_eq!(chapter_pos(57.5, Some((60.0, 20.0))), 57.5);
        assert_eq!(chapter_pos(10.0, Some((60.0, 0.3))), 10.0);
        // Abschnitte: 3 Kapitel → 3 Stücke, Lücken an den Grenzen, Enden bündig.
        let sp = chapter_spans(&ch, 600.0, 0.0, 600.0);
        assert_eq!(sp.len(), 3);
        assert_eq!((sp[0].0, sp[2].1), (0.0, 600.0));
        assert_eq!(sp[0].1, 60.0 - CHAPTER_GAP / 2.0);
        assert_eq!(sp[1].0, 60.0 + CHAPTER_GAP / 2.0);
        assert_eq!(speed_label(1.5), "1.5×");
        assert_eq!(speed_label(1.0), "Normal");
        assert_eq!(fmt_duration(3723), "1:02:03");
        assert_eq!(fmt_duration(83), "1:23");
        let sb = Storyboard { width: 10, height: 5, cols: 2, rows: 2, fps: 1.0, sheets: vec!["a".into(), "b".into()] };
        assert_eq!(sb.locate(0.0), (0, 0, 0));
        assert_eq!(sb.locate(3.0), (0, 10, 5));
        assert_eq!(sb.locate(4.0), (1, 0, 0));
        assert_eq!(sb.locate(99.0), (1, 10, 5), "nach dem Ende: letztes Bild");
    }

    #[test]
    fn subtitles() {
        let t = |v: &[(i64, &str)]| v.iter().map(|(i, l)| (*i, l.to_string())).collect::<Vec<_>>();
        let tracks = t(&[(1, "de"), (2, "en-orig"), (3, "en")]);
        assert_eq!(pick_sub(&tracks, "en"), Some(3));
        assert_eq!(pick_sub(&tracks, "en-orig"), Some(2));
        assert_eq!(pick_sub(&tracks, "de"), Some(1));
        assert_eq!(pick_sub(&t(&[(1, "de"), (2, "en-orig")]), "en"), Some(2));
        assert_eq!(pick_sub(&t(&[(1, "de")]), "fr"), Some(1));
        assert_eq!(pick_sub(&[], "en"), None);
        assert_eq!(lang_label("en-orig"), "English (en-orig)");
        // Spur-Namen statt Sprachcodes (Untertitel-Dateien in AniQu/BoxQu).
        let named = t(&[(1, "বাংলা"), (2, "English"), (3, "Français")]);
        assert_eq!(pick_sub(&named, "Français"), Some(3));
        assert_eq!(pick_sub(&named, "Deutsch"), Some(2), "sonst Englisch");
        assert_eq!(pick_sub(&named, "en"), Some(2), "Standard „en“ findet „English“");
        assert_eq!(pick_sub(&t(&[(4, "Español")]), "English"), Some(4), "sonst die erste");
        assert_eq!(SubPrefs::parse(""), SubPrefs { lang: "en".into(), size: 38, boxed: false, on: true });
        assert_eq!(SubPrefs::parse("de\n48\nbox\n"), SubPrefs { lang: "de".into(), size: 48, boxed: true, on: true });
        assert!(!SubPrefs::parse("Français\n48\nbox\noff\n").on);
        assert_eq!(SubPrefs::parse("de\n999\n").size, 38);
    }
}

// Storyboard liegt in `storyboard` (ohne GTK, auch für App-Logik ohne Oberfläche); hier weiter erreichbar.
pub use crate::storyboard::Storyboard;

static HTTP: std::sync::LazyLock<reqwest::Client> = std::sync::LazyLock::new(reqwest::Client::new);
/// Gleichzeitige Downloads von Storyboard-Bögen.
static SHEET_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

/// Bild laden ohne Festplatten-Cache (Bögen braucht nur das laufende Video).
async fn fetch_image(url: String) -> Result<Vec<u8>, String> {
    let _slot = SHEET_SLOTS.acquire().await.map_err(|e| e.to_string())?;
    let res = HTTP.get(url).send().await.and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?;
    Ok(res.bytes().await.map_err(|e| e.to_string())?.to_vec())
}

/// Vorschaubild zur Zeit `t` zeigen. Fehlt der Bogen noch, wird er im Hintergrund geladen und
/// danach gezeigt (solange die Maus noch dort ist); bis dahin nur die Zeit.
fn show_frame(pic: &gtk::Picture, sb: &Rc<RefCell<Option<Storyboard>>>, sheets: &Rc<RefCell<HashMap<usize, Option<Pixbuf>>>>, t: f64) {
    let Some(board) = sb.borrow().clone() else {
        pic.set_visible(false);
        return;
    };
    // Anzeigegröße: mindestens 192 px breit, Seitenverhältnis wie im Storyboard.
    let w = board.width.max(192);
    pic.set_size_request(w, w * board.height / board.width.max(1));
    let (sheet, x, y) = board.locate(t);
    let loaded = sheets.borrow().get(&sheet).cloned();
    match loaded {
        Some(Some(pixbuf)) => {
            if x + board.width <= pixbuf.width() && y + board.height <= pixbuf.height() {
                let frame = pixbuf.new_subpixbuf(x, y, board.width, board.height);
                #[allow(deprecated)] // ponytail: Texture::for_pixbuf reicht für kleine Vorschaubilder
                pic.set_paintable(Some(&gdk::Texture::for_pixbuf(&frame)));
                pic.set_visible(true);
            }
        }
        Some(None) => {} // lädt gerade
        None => {
            sheets.borrow_mut().insert(sheet, None);
            let url = board.sheets[sheet].clone();
            let (pic, sb, sheets) = (pic.downgrade(), sb.clone(), sheets.clone());
            glib::spawn_future_local(async move {
                let bytes = crate::bg(fetch_image(url)).await;
                let pixbuf = bytes.ok().and_then(|b| {
                    let loader = gtk::gdk_pixbuf::PixbufLoader::new();
                    loader.write(&b).ok()?;
                    loader.close().ok()?;
                    loader.pixbuf()
                });
                // Inzwischen anderes Video: verwerfen (die Tabelle wurde dann geleert).
                if !sheets.borrow().contains_key(&sheet) {
                    return;
                }
                match pixbuf {
                    Some(p) => {
                        sheets.borrow_mut().insert(sheet, Some(p));
                        if let Some(pic) = pic.upgrade().filter(|p| p.parent().is_some_and(|b| b.is_visible())) {
                            show_frame(&pic, &sb, &sheets, t);
                        }
                    }
                    None => {
                        sheets.borrow_mut().remove(&sheet);
                    }
                }
            });
        }
    }
}
