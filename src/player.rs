//! Eingebetteter Player für alle Qu-Apps: libmpv zeichnet per OpenGL in eine GtkGLArea, darüber Bedienleiste
//! (`controls.rs`), Meldungen, Spinner, Vorschaubild, Endbildschirm und Abschnitte (Knopf „Skip …“ oder
//! automatisch übersprungen, `[player] segments` in qu.toml).
//!
//! Generisch über `M` = was die App abspielt (Episode, Video …). App-Eigenes über `Hooks`: zusätzliche
//! mpv-Einstellungen (z. B. yt-dlp), Fortschritt speichern, Position an einen Dienst melden, Abschnitte holen,
//! Tempo merken, zweiter Versuch mit anderen Optionen.

use crate::controls::{Controls, MAX_VOLUME, saved_quality, saved_volume, speed_label, sub_tracks};
use crate::segments::{Segment, segment_at, to_skip};
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use libmpv2::events::{Event, PropertyData};
use libmpv2::render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType};
use libmpv2::{Format, Mpv, MpvInitializer};
use std::cell::{Cell, RefCell};
use std::ffi::{CString, c_char, c_void};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

/// Was die App einhängt; alles optional.
pub trait Hooks<M>: 'static {
    /// Eigene mpv-Einstellungen beim Start, nach den gemeinsamen (z. B. `ytdl`, `af`, `user-agent`).
    fn init(&self, _i: &MpvInitializer) -> libmpv2::Result<()> {
        Ok(())
    }
    /// Fortschritt sichern (alle 5 s, bei Pause, Wechsel und Stopp; nicht bei Livestreams).
    fn save(&self, _item: &M, _pos: f64, _dur: f64) {}
    /// Position an einen Dienst melden (Pause, Wechsel, Stopp). `wait`: beim Schließen kurz warten.
    fn report(&self, _item: &M, _pos: f64, _wait: bool) {}
    /// Abschnitte (Opening, Sponsor …) im Hintergrund holen; `None` = keine.
    fn segments(&self, _item: &M) -> Option<Pin<Box<dyn Future<Output = Vec<Segment>> + Send>>> {
        None
    }
    /// Tempo vom Nutzer geändert (zum Merken pro Titel/Kanal).
    fn speed_changed(&self, _item: &M, _speed: f64) {}
    /// Nach einem Ladefehler: Optionen für den einen zweiten Versuch (z. B. mit Login); `None` = dieselben.
    fn retry_opts(&self, _log: &[String], _opts: &str) -> Option<String> {
        None
    }
}

/// Was abgespielt wird.
#[derive(Clone)]
pub struct Media<M> {
    /// Titel/Video der App (für Fortschritt, Abschnitte …); `None` = loser Link.
    pub item: Option<M>,
    /// Qualitätsstufen (Bildhöhe, Adresse), beste zuerst; bei yt-dlp/direkten Links eine mit Höhe 0.
    pub variants: Vec<(u32, String)>,
    /// Weitere `loadfile`-Optionen, schon mit `quoted` geschrieben (Header, Untertitel …).
    pub opts: Vec<String>,
    /// Fortsetzen ab (s).
    pub start: Option<f64>,
    pub live: bool,
    pub speed: f64,
    /// Vorschaubild zeigen, bis das Video lädt (die App füllt `Player::poster`).
    pub poster: bool,
}

impl<M> Media<M> {
    /// Ein Link (yt-dlp oder direkt), sonst Standardwerte.
    pub fn link(item: Option<M>, url: &str) -> Self {
        Media { item, variants: vec![(0, url.to_string())], opts: vec![], start: None, live: false, speed: 1.0, poster: false }
    }
}

pub struct Player<M: Clone + 'static> {
    pub mpv: &'static Mpv,
    /// Video plus Meldungsfläche darüber; das kommt ins Layout.
    pub widget: gtk::Overlay,
    pub controls: Rc<Controls>,
    area: gtk::GLArea,
    msg: gtk::Label,
    /// Reload-Icon unter einer Fehlermeldung (verbindet die App).
    pub retry: gtk::Button,
    /// Endbildschirm: „Nächste Episode“ (sichtbar, wenn die App es setzt) und „Zur Startseite“.
    pub end_next: gtk::Button,
    pub end_home: gtk::Button,
    /// Kurze Einblendung mitten im Bild („+10 s“, „65 %“, „1.5×“ …).
    flash: gtk::Label,
    /// Wird mit `true` aufgerufen, solange ein Video läuft (Bildschirmschoner aus), sonst `false`.
    on_playing: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    spinner: gtk::Image,
    /// Vorschaubild, solange das Video lädt (`Media::poster`; füllt die App).
    pub poster: gtk::Picture,
    /// Wird mit dem Seitenverhältnis (Breite/Höhe) aufgerufen, sobald das Video es kennt.
    on_aspect: Rc<RefCell<Option<Box<dyn Fn(f64)>>>>,
    /// Bekommt das yt-dlp-JSON des Videos (holt mpv beim Laden über yt-dlp ohnehin).
    on_info: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
    track: Rc<RefCell<Track<M>>>,
    hooks: Rc<dyn Hooks<M>>,
    /// Video-Fläche bereit (mpv-Render-Kontext angelegt)? Vorher scheitert `loadfile` mit „No video
    /// output“; das Video wartet dann in `pending`, bis die Fläche da ist.
    gl_ready: Rc<Cell<bool>>,
    pending: Rc<RefCell<Option<(String, String)>>>,
}

/// Was gerade läuft.
struct Track<M> {
    media: Option<Media<M>>,
    /// Optionen ohne Startpunkt (für den Qualitätswechsel).
    base_opts: String,
    last_save: Option<Instant>,
    /// Abschnitte und (automatisch) schon übersprungene (nur einmal springen).
    segments: Vec<Segment>,
    skipped: Vec<usize>,
    /// Was zuletzt geladen wurde (URL, Optionen), für einen zweiten Versuch bei Fehlern.
    load: Option<(String, String)>,
    retried: bool,
    /// Wann `play` lief: kurz danach keine Einblendungen (Tempo/Lautstärke/Untertitel setzt `play` selbst).
    started: Option<Instant>,
}

impl<M> Default for Track<M> {
    fn default() -> Self {
        Track { media: None, base_opts: String::new(), last_save: None, segments: vec![], skipped: vec![], load: None, retried: false, started: None }
    }
}

impl<M> Track<M> {
    /// Titel des laufenden Videos (nicht bei Livestreams: da gibt es keinen Fortschritt).
    fn item(&self) -> Option<&M> {
        self.media.as_ref().filter(|m| !m.live).and_then(|m| m.item.as_ref())
    }
}

/// Wert für die Optionsliste von `loadfile` (mpv: `%Länge%Wert`, Länge in Bytes): Kommas im Wert stören so nicht.
pub fn quoted(v: &str) -> String {
    format!("%{}%{v}", v.len())
}

/// HTTP-Header als `loadfile`-Optionen: User-Agent und Referer eigene Optionen, der Rest (Cookie …) als
/// `http-header-fields` (Liste mit „,“ als Trenner, Kommas im Wert mit `\` geschützt).
pub fn header_opts(headers: &[(String, String)]) -> Vec<String> {
    let mut opts = Vec::new();
    let mut fields = Vec::new();
    for (k, v) in headers {
        if k.eq_ignore_ascii_case("user-agent") {
            opts.push(format!("user-agent={}", quoted(v)));
        } else if k.eq_ignore_ascii_case("referer") {
            opts.push(format!("referrer={}", quoted(v)));
        } else {
            fields.push(format!("{k}: {v}").replace(',', "\\,"));
        }
    }
    if !fields.is_empty() {
        opts.push(format!("http-header-fields={}", quoted(&fields.join(","))));
    }
    opts
}

/// Optionen plus Startpunkt (0 = von vorne).
fn with_start(base: &str, start: f64) -> String {
    match (base.is_empty(), start > 0.0) {
        (_, false) => base.to_string(),
        (true, true) => format!("start={start:.1}"),
        (false, true) => format!("{base},start={start:.1}"),
    }
}

/// Beste Stufe bis zur gewählten Höhe (Höhe 0 = unbekannt, passt immer); gibt es keine so kleine, die kleinste.
fn pick_variant(variants: &[(u32, String)], height: u32) -> String {
    variants.iter().find(|(h, _)| *h <= height).or(variants.last()).map(|(_, u)| u.clone()).unwrap_or_default()
}

/// Kurzer Hinweis über dem Video, verschwindet nach ein paar Sekunden von selbst.
pub fn toast(label: &gtk::Label, text: &str) {
    label.set_text(text);
    label.set_visible(true);
    let (label, text) = (label.downgrade(), text.to_string());
    glib::timeout_add_local_once(Duration::from_millis(2500), move || {
        // Nur ausblenden, wenn inzwischen kein neuer Hinweis kam.
        if let Some(l) = label.upgrade().filter(|l| l.text() == text) {
            l.set_visible(false);
        }
    });
}

/// Wie oft während des Abspielens gespeichert wird (zusätzlich bei Pause, Stopp, Schließen).
const SAVE_EVERY: Duration = Duration::from_secs(5);
/// „Skip …“-Knopf: Abstand über der Timeline und Platz, solange die Leiste noch nicht angeordnet ist.
const SKIP_GAP: i32 = 8;
const SKIP_MARGIN: i32 = 80;

fn save<M: 'static>(mpv: &Mpv, track: &RefCell<Track<M>>, hooks: &dyn Hooks<M>) {
    let mut t = track.borrow_mut();
    let Some(item) = t.item() else { return };
    if let (Ok(pos), Ok(dur)) = (mpv.get_property::<f64>("time-pos"), mpv.get_property::<f64>("duration")) {
        hooks.save(item, pos, dur);
        t.last_save = Some(Instant::now());
    }
}

fn report<M: 'static>(mpv: &Mpv, track: &RefCell<Track<M>>, hooks: &dyn Hooks<M>, wait: bool) {
    let t = track.borrow();
    let (Some(item), Ok(pos)) = (t.item(), mpv.get_property::<f64>("time-pos")) else { return };
    hooks.report(item, pos, wait);
}

/// Blockiert, bis `fut` fertig ist, höchstens `limit` (z. B. eine Meldung beim Schließen der App). Der Timer
/// muss *im* `block_on` entstehen: `timeout()` braucht schon beim Anlegen die Tokio-Runtime, im GTK-Thread gibt
/// es keine (Panik „no reactor running“ → Absturz beim Schließen/Zurück).
pub fn wait_at_most(limit: Duration, fut: impl Future<Output = ()>) {
    let _ = crate::RT.block_on(async move { tokio::time::timeout(limit, fut).await });
}

/// Zeigt eine Meldung über dem Video (rot, wenn `error`); `retry` = darunter Reload-Icon anbieten.
fn show(msg: &gtk::Label, retry: &gtk::Button, text: &str, error: bool) {
    msg.set_text(text);
    if error { msg.add_css_class("error") } else { msg.remove_css_class("error") }
    msg.set_visible(true);
    retry.set_visible(error);
}

/// Macht aus den Fehlerzeilen von mpv/yt-dlp eine lesbare Ursache.
/// Aus "ERROR: [youtube] <id>: This video is unavailable" wird "This video is unavailable".
fn reason(log: &[String], fallback: &str) -> String {
    let Some(line) = log.iter().find(|l| l.contains("ERROR:")).or(log.first()) else {
        return fallback.to_string();
    };
    let s = line.split_once("ERROR: ").map_or(line.as_str(), |(_, r)| r);
    if s.starts_with('[')
        && let Some((_, rest)) = s.split_once("] ")
    {
        if let Some((id, msg)) = rest.split_once(": ")
            && !id.contains(' ')
        {
            return msg.to_string();
        }
        return rest.to_string();
    }
    s.to_string()
}

/// MPRIS-Plugin aus dem Paket mpv-mpris (Medientasten, Anzeige in Waybar & Co.).
const MPRIS_PLUGIN: &str = "/usr/lib/mpv-mpris/mpris.so";

// GTK4 nutzt unter Wayland (und standardmäßig auch unter X11) EGL.
// ponytail: nur eglGetProcAddress; Mesa und NVIDIA liefern darüber auch Core-GL-Funktionen.
#[cfg(not(windows))]
static EGL: LazyLock<libloading::Library> =
    LazyLock::new(|| unsafe { libloading::Library::new("libEGL.so.1").expect("libEGL.so.1 fehlt") });

// Unter Windows nutzt GTK4 WGL. wglGetProcAddress kennt nur Funktionen ab GL 1.2 (für ältere liefert es
// 0, 1, 2, 3 oder -1), die alten wie glGetIntegerv stehen direkt in opengl32.dll.
#[cfg(windows)]
static GL: LazyLock<libloading::Library> =
    LazyLock::new(|| unsafe { libloading::Library::new("opengl32.dll").expect("opengl32.dll fehlt") });

#[cfg(windows)]
fn get_proc_address(_: &(), name: &str) -> *mut c_void {
    let name = CString::new(name).unwrap();
    unsafe {
        let wgl: libloading::Symbol<unsafe extern "system" fn(*const c_char) -> *mut c_void> =
            GL.get(b"wglGetProcAddress\0").expect("wglGetProcAddress");
        let p = wgl(name.as_ptr());
        if !matches!(p as isize, -1..=3) {
            return p;
        }
        GL.get::<*mut c_void>(name.as_bytes_with_nul()).map_or(std::ptr::null_mut(), |s| *s)
    }
}

#[cfg(not(windows))]
fn get_proc_address(_: &(), name: &str) -> *mut c_void {
    let name = CString::new(name).unwrap();
    unsafe {
        let f: libloading::Symbol<unsafe extern "C" fn(*const c_char) -> *mut c_void> =
            EGL.get(b"eglGetProcAddress\0").expect("eglGetProcAddress");
        f(name.as_ptr())
    }
}

fn current_fbo() -> i32 {
    const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
    let ptr = get_proc_address(&(), "glGetIntegerv");
    let get: unsafe extern "C" fn(u32, *mut i32) = unsafe { std::mem::transmute(ptr) };
    let mut fbo = 0;
    unsafe { get(GL_FRAMEBUFFER_BINDING, &mut fbo) };
    fbo
}

impl<M: Clone + 'static> Player<M> {
    pub fn new(hooks: impl Hooks<M>) -> Self {
        let hooks: Rc<dyn Hooks<M>> = Rc::new(hooks);
        // ponytail: ein Mpv für die ganze Laufzeit, bewusst geleakt, damit der RenderContext ihn 'static leihen kann.
        let mut mpv = Mpv::with_initializer(|i| {
            i.set_property("vo", "libmpv")?;
            // Streams kommen fertig aus der App; yt-dlp schaltet die App über `Hooks::init` ein.
            i.set_property("ytdl", false)?;
            i.set_property("hwdec", "auto-safe")?;
            // Lauter als normal erlaubt (bis 200 %, mpv-Standard 130 %).
            i.set_property("volume-max", MAX_VOLUME)?;
            i.set_property("volume", saved_volume())?;
            i.set_property("keep-open", true)?;
            // Sprünge zum nächsten Schlüsselbild statt bildgenau: schneller (YouTube drosselt Downloads, bildgenau
            // müsste erst Sekunden davor laden). Ziel liegt dadurch meist 1–2 s neben der Stelle.
            i.set_property("hr-seek", "no")?;
            // Puffer: so weit vorladen wie möglich, damit Sprünge nach vorn sofort laufen; Zurückspringen bleibt im
            // schon Geladenen. ponytail: im RAM, 1080p ≈ 2 GB/h – bei langen Videos `cache-on-disk` erwägen.
            i.set_property("cache", "yes")?;
            i.set_property("cache-secs", 36000)?;
            i.set_property("demuxer-max-bytes", "1GiB")?;
            i.set_property("demuxer-max-back-bytes", "512MiB")?;
            // Nur der Fade-Filter für weiches Aus-/Einblenden (controls.rs); Apps können `af` ergänzen.
            i.set_property("af", "@fade:lavfi=[volume=1]")?;
            // Medientasten und Anzeige in der Leiste (MPRIS), falls das mpv-mpris-Plugin installiert ist.
            if std::path::Path::new(MPRIS_PLUGIN).exists() {
                i.set_property("scripts", MPRIS_PLUGIN)?;
            }
            hooks.init(&i)
        })
        .expect("mpv failed to start");
        // mpv meldet neue Ereignisse aus einem eigenen Thread; ausgewertet wird im GTK-Thread.
        let (ev_tx, ev_rx) = async_channel::unbounded::<()>();
        mpv.set_wakeup_callback(move || {
            let _ = ev_tx.try_send(());
        });
        // Fehlerzeilen (z. B. von yt-dlp) als Ereignisse bekommen, um die Ursache anzeigen zu können.
        unsafe { libmpv2_sys::mpv_request_log_messages(mpv.ctx.as_ptr(), c"error".as_ptr()) };
        let mpv: &'static Mpv = Box::leak(Box::new(mpv));
        for (name, format) in [
            ("time-pos", Format::Double),
            ("duration", Format::Double),
            ("pause", Format::Flag),
            ("volume", Format::Double),
            ("mute", Format::Flag),
            ("speed", Format::Double),
            ("eof-reached", Format::Flag),
            // Gewählte Untertitelspur (Name fürs Einblenden liefert `sub_tracks`).
            ("current-tracks/sub/id", Format::Int64),
            ("dwidth", Format::Int64),
            ("dheight", Format::Int64),
            ("demuxer-cache-time", Format::Double),
            ("paused-for-cache", Format::Flag),
            ("seeking", Format::Flag),
            // Vom ytdl_hook gesetzt, solange das Video läuft (mpv ≥ 0.37; nur mit yt-dlp).
            ("user-data/mpv/ytdl/json-subprocess-result/stdout", Format::String),
        ] {
            let _ = mpv.observe_property(name, format, 0);
        }

        let area = gtk::GLArea::new();
        area.set_hexpand(true);
        // Höhe bestimmt die Ansehen-Seite passend zum Video; nicht von selbst wachsen.
        area.set_focusable(true);

        let ctx: Rc<RefCell<Option<RenderContext<'static>>>> = Rc::default();
        let gl_ready: Rc<Cell<bool>> = Rc::default();
        let pending: Rc<RefCell<Option<(String, String)>>> = Rc::default();

        area.connect_realize({
            let (ctx, gl_ready, pending) = (ctx.clone(), gl_ready.clone(), pending.clone());
            move |a| {
                a.make_current();
                if let Some(e) = a.error() {
                    eprintln!("OpenGL nicht verfügbar: {e}");
                    return;
                }
                let mut rc = mpv
                    .create_render_context(vec![
                        RenderParam::ApiType(RenderParamApiType::OpenGl),
                        RenderParam::InitParams(OpenGLInitParams { get_proc_address, ctx: () }),
                    ])
                    .expect("mpv render context");
                // Der Callback kommt aus einem mpv-Thread; neu gezeichnet wird im GTK-Thread.
                let (tx, rx) = async_channel::unbounded::<()>();
                rc.set_update_callback(move || {
                    let _ = tx.try_send(());
                });
                let weak = a.downgrade();
                glib::spawn_future_local(async move {
                    while rx.recv().await.is_ok() {
                        match weak.upgrade() {
                            Some(a) => a.queue_render(),
                            None => break,
                        }
                    }
                });
                *ctx.borrow_mut() = Some(rc);
                gl_ready.set(true);
                if let Some((url, opts)) = pending.take() {
                    let _ = mpv.command("loadfile", &[&url, "replace", "-1", &opts]);
                }
            }
        });

        area.connect_unrealize({
            let (ctx, gl_ready) = (ctx.clone(), gl_ready.clone());
            move |a| {
                a.make_current();
                gl_ready.set(false);
                ctx.borrow_mut().take();
            }
        });

        area.connect_render(move |a, _| {
            if let Some(rc) = ctx.borrow().as_ref() {
                let s = a.scale_factor();
                let _ = rc.render::<()>(current_fbo(), a.width() * s, a.height() * s, true);
            }
            glib::Propagation::Stop
        });

        let widget = gtk::Overlay::new();
        // Abrundung (CSS-Klasse `rounded`) schneidet auch das Video ab.
        widget.set_overflow(gtk::Overflow::Hidden);
        let controls = Controls::new(mpv, &widget);
        let track: Rc<RefCell<Track<M>>> = Rc::default();

        // Kurze Einblendung mitten im Bild.
        let flash = gtk::Label::builder()
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .visible(false)
            .can_target(false)
            .css_classes(["osd", "pill", "flash"])
            .build();

        // Klick aufs Video: Pause umschalten. Doppelklick: im linken/rechten Drittel 10 s zurück/vor (wie in der
        // YouTube-App), in der Mitte Vollbild (der erste Klick pausiert, der zweite hebt das wieder auf).
        let click = gtk::GestureClick::new();
        click.connect_pressed({
            let (controls, flash) = (controls.clone(), flash.clone());
            move |g, n, x, _| {
                // Im Miniplayer: Ziehen statt Pause (Pause über den Knopf im Miniplayer).
                if g.widget().is_some_and(|w| in_mini(&w)) {
                    return;
                }
                let _ = mpv.command("cycle", &["pause"]);
                if n == 2 {
                    let w = g.widget().map_or(1, |w| w.width()) as f64;
                    let third = if x < w / 3.0 { -1 } else if x > w * 2.0 / 3.0 { 1 } else { 0 };
                    if third == 0 {
                        controls.fullscreen.emit_clicked();
                    } else {
                        let _ = mpv.command("seek", &[&(10 * third).to_string()]);
                        toast(&flash, if third < 0 { "−10 s" } else { "+10 s" });
                    }
                }
            }
        });
        area.add_controller(click);
        controls.subtitles.connect_clicked({
            let (controls, flash) = (controls.clone(), flash.clone());
            move |_| {
                if !controls.toggle_subs() {
                    toast(&flash, "No subtitles");
                }
            }
        });

        // Mausbewegung über dem Video blendet die Leiste ein. Nur echte Bewegung zählt: GTK schickt nach
        // Änderungen im Layout Bewegungs-Ereignisse an derselben Stelle nach, die Leiste bliebe sonst sichtbar.
        let motion = gtk::EventControllerMotion::new();
        let last = Cell::new((f64::NAN, f64::NAN));
        motion.connect_motion({
            let controls = controls.clone();
            move |_, x, y| {
                let (lx, ly) = last.replace((x, y));
                if (x - lx).abs() >= 1.0 || (y - ly).abs() >= 1.0 || lx.is_nan() {
                    controls.poke();
                }
            }
        });
        // Maus verlässt den Player: Leiste sofort weg.
        motion.connect_leave({
            let controls = controls.clone();
            move |_| controls.hide()
        });
        widget.add_controller(motion);

        let msg = gtk::Label::builder()
            .wrap(true)
            .justify(gtk::Justification::Center)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .margin_start(24)
            .margin_end(24)
            .visible(false)
            .css_classes(["osd"])
            .build();
        let retry = gtk::Button::builder()
            .icon_name("qu-reload-symbolic")
            .tooltip_text("Reload")
            .halign(gtk::Align::Center)
            .focusable(false)
            .visible(false)
            .css_classes(["flat", "osd"])
            .cursor(&gdk::Cursor::from_name("pointer", None).unwrap())
            .build();
        let msg_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        msg_box.append(&msg);
        msg_box.append(&retry);
        widget.set_child(Some(&area));
        // Vorschaubild unter Meldung/Spinner, bis das Video geladen ist (`play` zeigt es, `FileLoaded` blendet es aus).
        let poster = gtk::Picture::builder().content_fit(gtk::ContentFit::Cover).can_target(false).visible(false).build();
        widget.add_overlay(&poster);
        widget.add_overlay(&msg_box);
        // Laden: nur ein Spinner, kein Text.
        let spinner = crate::icons::spinner(32).halign(gtk::Align::Center).valign(gtk::Align::Center).visible(false).build();
        widget.add_overlay(&spinner);
        let on_aspect: Rc<RefCell<Option<Box<dyn Fn(f64)>>>> = Rc::default();
        let on_info: Rc<RefCell<Option<Box<dyn Fn(&str)>>>> = Rc::default();
        // Hinweis oben links („Skipped sponsor“).
        let note = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Start)
            .margin_start(16)
            .margin_top(16)
            .visible(false)
            .css_classes(["osd", "pill"])
            .build();
        widget.add_overlay(&note);
        widget.add_overlay(&flash);

        // Endbildschirm: Nochmal / Nächste Episode / Zur Startseite (nur Icons).
        let end_again = gtk::Button::builder().icon_name("qu-reload-symbolic").tooltip_text("Watch again").focusable(false).build();
        let end_next = gtk::Button::builder().icon_name("qu-next-symbolic").tooltip_text("Next episode").focusable(false).visible(false).build();
        let end_home = gtk::Button::builder().icon_name("qu-home-symbolic").tooltip_text("Home").focusable(false).build();
        let end_box = gtk::Box::builder()
            .spacing(2)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .visible(false)
            .css_classes(["bubble", "end-screen"])
            .build();
        end_box.append(&end_again);
        end_box.append(&end_next);
        end_box.append(&end_home);
        end_again.connect_clicked(move |_| {
            let _ = mpv.command("seek", &["0", "absolute"]);
            let _ = mpv.set_property("pause", false);
        });
        widget.add_overlay(&end_box);

        // „Skip intro“/„Skip ending“ (`[player] segments = "button"`): rechts direkt über der Timeline, sichtbar
        // solange man im Abschnitt ist, auch bei ausgeblendeter Leiste.
        let skip_label = gtk::Label::new(None);
        let skip_button = gtk::Button::builder()
            .child(&skip_label)
            .focusable(false)
            .cursor(&gdk::Cursor::from_name("pointer", None).unwrap())
            .build();
        let skip = gtk::Box::builder()
            .halign(gtk::Align::End)
            .valign(gtk::Align::End)
            .margin_end(16)
            .margin_bottom(SKIP_MARGIN)
            .visible(false)
            .css_classes(["bubble"])
            .build();
        skip.append(&skip_button);
        widget.add_overlay(&skip);
        let skip_to: Rc<Cell<Option<f64>>> = Rc::default();
        skip_button.connect_clicked({
            let (skip, skip_to) = (skip.downgrade(), skip_to.clone());
            move |_| {
                if let Some(end) = skip_to.take() {
                    let _ = mpv.command("seek", &[&end.to_string(), "absolute"]);
                }
                if let Some(s) = skip.upgrade() {
                    s.set_visible(false);
                }
            }
        });
        let auto_skip = crate::config().player.segments == "auto";

        widget.add_overlay(&controls.widget);
        widget.add_overlay(&controls.preview);
        let on_playing: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> = Rc::default();

        glib::spawn_future_local({
            let (msg, retry, spinner, poster) = (msg.clone(), retry.clone(), spinner.clone(), poster.clone());
            let (on_aspect, on_info, on_playing) = (on_aspect.clone(), on_info.clone(), on_playing.clone());
            let (flash, end_box, note) = (flash.clone(), end_box.clone(), note.clone());
            let (skip, skip_label, skip_to, player_widget) = (skip.clone(), skip_label.clone(), skip_to.clone(), widget.clone());
            let (controls, track, hooks) = (controls.clone(), track.clone(), hooks.clone());
            async move {
                let mut log: Vec<String> = Vec::new();
                // Erst nach dem ersten Laden zeigt der Spinner Puffern/Springen; davor regeln Start/Geladen ihn.
                let mut loaded = false;
                // Zustände aus den Ereignissen mitschreiben statt mpv zu fragen: `get_property` wartet, solange
                // mpv beschäftigt ist (gemessen bis 200 ms), und hält so die Oberfläche samt Spinner an.
                let waiting: Rc<Cell<(bool, bool)>> = Rc::default(); // (Puffer leer, springt)
                let (mut dw, mut dh) = (0i64, 0i64);
                let mut sub_on = false;
                while ev_rx.recv().await.is_ok() {
                    while let Some(ev) = mpv.wait_event(0.0) {
                        match ev {
                            Ok(Event::StartFile) => {
                                log.clear();
                                loaded = false;
                                end_box.set_visible(false);
                                skip.set_visible(false);
                                skip_to.set(None);
                                msg.set_visible(false);
                                retry.set_visible(false);
                                spinner.set_visible(true);
                            }
                            Ok(Event::FileLoaded) => {
                                loaded = true;
                                spinner.set_visible(false);
                                poster.set_visible(false);
                                controls.load_chapters();
                                if crate::config().player.auto_subtitles {
                                    controls.apply_subs();
                                }
                            }
                            Ok(Event::LogMessage { text, .. }) => log.push(text.trim().to_string()),
                            Ok(Event::PropertyChange { name, change, .. }) => {
                                // Puffer leer oder Sprung: Spinner zeigen, aber erst nach kurzer Zeit, damit
                                // Sprünge innerhalb des Vorgeladenen nicht flackern.
                                if let ("paused-for-cache" | "seeking", PropertyData::Flag(on)) = (name, &change) {
                                    let (mut buffering, mut seeking) = waiting.get();
                                    if name == "seeking" { seeking = *on } else { buffering = *on }
                                    waiting.set((buffering, seeking));
                                }
                                if loaded && (name == "paused-for-cache" || name == "seeking") {
                                    let is_waiting = |w: &Cell<(bool, bool)>| w.get().0 || w.get().1;
                                    if is_waiting(&waiting) {
                                        let (sp, w) = (spinner.downgrade(), waiting.clone());
                                        glib::timeout_add_local_once(Duration::from_millis(300), move || {
                                            if let Some(sp) = sp.upgrade().filter(|_| is_waiting(&w)) {
                                                sp.set_visible(true);
                                            }
                                        });
                                    } else {
                                        spinner.set_visible(false);
                                    }
                                }
                                match (name, &change) {
                                    (n, PropertyData::Str(json)) if n.ends_with("json-subprocess-result/stdout") => {
                                        if let Some(f) = on_info.borrow().as_ref() {
                                            f(json);
                                        }
                                    }
                                    ("dwidth", PropertyData::Int64(v)) => dw = *v,
                                    ("dheight", PropertyData::Int64(v)) => dh = *v,
                                    _ => {}
                                }
                                if (name == "dwidth" || name == "dheight")
                                    && dw > 0
                                    && dh > 0
                                    && let Some(f) = on_aspect.borrow().as_ref()
                                {
                                    f(dw as f64 / dh as f64);
                                }
                                let due = track.borrow().last_save.is_none_or(|t| t.elapsed() >= SAVE_EVERY);
                                let paused = name == "pause" && matches!(change, PropertyData::Flag(true));
                                if (name == "time-pos" && due) || paused {
                                    save(mpv, &track, &*hooks);
                                }
                                if paused {
                                    report(mpv, &track, &*hooks, false);
                                }
                                // Abschnitte: automatisch ans Ende springen oder „Skip …“-Knopf anbieten.
                                if let (true, PropertyData::Double(pos)) = (name == "time-pos", &change) {
                                    if auto_skip {
                                        let mut t = track.borrow_mut();
                                        if let Some(i) = to_skip(&t.segments, &t.skipped, *pos) {
                                            t.skipped.push(i);
                                            let s = t.segments[i].clone();
                                            let _ = mpv.command("seek", &[&s.end.to_string(), "absolute"]);
                                            toast(&note, &format!("Skipped {}", s.label));
                                        }
                                    } else {
                                        let seg = {
                                            let t = track.borrow();
                                            segment_at(&t.segments, *pos).map(|i| t.segments[i].clone())
                                        };
                                        match seg {
                                            Some(s) => {
                                                skip_label.set_text(&format!("Skip {}", s.label));
                                                skip_to.set(Some(s.end));
                                                // Direkt über der Timeline (ihre Höhe hängt von Theme/Schrift ab).
                                                if let Some(y) = controls.timeline_top(&player_widget) {
                                                    skip.set_margin_bottom((player_widget.height() as f32 - y) as i32 + SKIP_GAP);
                                                }
                                                skip.set_visible(true);
                                            }
                                            None if skip.is_visible() => {
                                                skip.set_visible(false);
                                                skip_to.set(None);
                                            }
                                            None => {}
                                        }
                                    }
                                }
                                // Einblendungen bei Änderungen durch den Nutzer (nicht direkt nach dem Start).
                                let quiet = track.borrow().started.is_none_or(|t| t.elapsed() < Duration::from_millis(1500));
                                match (name, &change) {
                                    ("volume", PropertyData::Double(v)) if loaded && !quiet => toast(&flash, &format!("{v:.0} %")),
                                    ("mute", PropertyData::Flag(m)) if loaded && !quiet => toast(&flash, if *m { "Muted" } else { "Sound on" }),
                                    ("speed", PropertyData::Double(s)) => {
                                        if loaded && !quiet {
                                            toast(&flash, &speed_label(*s));
                                        }
                                        // Tempo merken (die App setzt es beim nächsten Titel/Kanal wieder).
                                        if let Some(item) = track.borrow().item().filter(|_| !quiet) {
                                            hooks.speed_changed(item, *s);
                                        }
                                    }
                                    ("pause", PropertyData::Flag(p)) => {
                                        if let Some(f) = on_playing.borrow().as_ref() {
                                            f(!p);
                                        }
                                    }
                                    ("eof-reached", PropertyData::Flag(e)) => end_box.set_visible(*e && loaded),
                                    ("current-tracks/sub/id", PropertyData::Int64(id)) => {
                                        if loaded && !quiet {
                                            let name = sub_tracks(mpv).into_iter().find(|(i, _, _)| i == id).map(|(_, _, l)| l);
                                            toast(&flash, &format!("Subtitles: {}", name.unwrap_or_default()));
                                        }
                                        sub_on = true;
                                    }
                                    ("current-tracks/sub/id", PropertyData::None) => {
                                        if loaded && !quiet && sub_on {
                                            toast(&flash, "Subtitles off");
                                        }
                                        sub_on = false;
                                    }
                                    _ => {}
                                }
                                controls.update(name, change);
                            }
                            // Ein Fehler-Ereignis ist hier das Ende einer Datei mit Fehler.
                            Err(e) => {
                                eprintln!("Video-Fehler: {log:?}");
                                // Stream-Server verweigern gelegentlich kurzzeitig: einmal neu laden (ggf. mit
                                // anderen Optionen von der App, z. B. Login), erst dann den Fehler zeigen.
                                let again = {
                                    let mut t = track.borrow_mut();
                                    let again = (!t.retried)
                                        .then(|| t.load.clone())
                                        .flatten()
                                        .map(|(url, opts)| (url, hooks.retry_opts(&log, &opts).unwrap_or(opts)));
                                    t.retried = true;
                                    again
                                };
                                if let Some((url, opts)) = again {
                                    let _ = mpv.command("loadfile", &[&url, "replace", "-1", &opts]);
                                } else {
                                    let why = reason(&log, &e.to_string());
                                    spinner.set_visible(false);
                                    show(&msg, &retry, &format!("Couldn't load video:\n{why}"), true);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        // Qualität im Einstellungsmenü geändert: passende Stufe an derselben Stelle neu laden (Apps mit yt-dlp
        // ersetzen das über `controls.connect_quality`).
        controls.connect_quality({
            let track = track.clone();
            move |height| {
                let t = track.borrow();
                let Some(media) = t.media.as_ref() else { return };
                let pos = mpv.get_property::<f64>("time-pos").unwrap_or(0.0);
                let opts = with_start(&t.base_opts, if media.live { 0.0 } else { pos });
                let _ = mpv.command("loadfile", &[&pick_variant(&media.variants, height), "replace", "-1", &opts]);
            }
        });

        Player { mpv, widget, controls, area, msg, retry, end_next, end_home, flash, on_playing, spinner, poster, on_aspect, on_info, track, hooks, gl_ready, pending }
    }

    /// Video-Fläche schon beim App-Start einrichten (OpenGL + mpv-Render-Kontext, gemessen ~120 ms im
    /// GTK-Thread), nicht erst beim ersten Video-Klick. Geht erst, wenn das Fenster da ist.
    pub fn prepare(&self) {
        if !self.area.is_realized() {
            self.area.realize();
        }
    }

    pub fn connect_playing(&self, f: impl Fn(bool) + 'static) {
        self.on_playing.replace(Some(Box::new(f)));
    }

    /// yt-dlp-JSON des laufenden Videos (z. B. Storyboard, Ersatz-Details).
    pub fn connect_info(&self, f: impl Fn(&str) + 'static) {
        self.on_info.replace(Some(Box::new(f)));
    }

    pub fn connect_aspect(&self, f: impl Fn(f64) + 'static) {
        self.on_aspect.replace(Some(Box::new(f)));
    }

    /// Fehler ohne geladenes Video (z. B. Anfragen-Limit): ohne Reload-Icon, das ginge ja auch nicht.
    pub fn show_error(&self, text: &str) {
        self.spinner.set_visible(false);
        show(&self.msg, &self.retry, text, true);
        self.retry.set_visible(false);
    }

    /// Kurze Einblendung mitten im Bild.
    pub fn flash(&self, text: &str) {
        toast(&self.flash, text);
    }

    /// Spinner zeigen, solange die App die Stream-Adresse noch holt (voriges Video wird gesichert und beendet).
    pub fn loading(&self) {
        save(self.mpv, &self.track, &*self.hooks);
        report(self.mpv, &self.track, &*self.hooks, false);
        self.track.replace(Track::default());
        let _ = self.mpv.command("stop", &[]);
        self.msg.set_visible(false);
        self.retry.set_visible(false);
        self.spinner.set_visible(true);
    }

    /// Was zuletzt abgespielt wurde (z. B. zum Neuladen mit frischer Adresse).
    pub fn current(&self) -> Option<Media<M>> {
        self.track.borrow().media.clone()
    }

    /// Abspielen; `media.start` setzt fort.
    pub fn play(&self, media: Media<M>) {
        save(self.mpv, &self.track, &*self.hooks); // vorheriges sichern
        report(self.mpv, &self.track, &*self.hooks, false);
        // Abschnitte im Hintergrund holen.
        if let Some(fut) = media.item.as_ref().filter(|_| !media.live).and_then(|i| self.hooks.segments(i)) {
            let (track, controls) = (self.track.clone(), self.controls.clone());
            let url = media.variants.first().map(|v| v.1.clone());
            glib::spawn_future_local(async move {
                let segs = crate::bg(fut).await;
                let mut t = track.borrow_mut();
                // Inzwischen etwas anderes gestartet: verwerfen.
                if t.media.as_ref().and_then(|m| m.variants.first()).map(|v| v.1.clone()) == url {
                    controls.set_segments(segs.clone());
                    t.segments = segs;
                }
            });
        }
        let _ = self.mpv.set_property("speed", media.speed);
        // Vorschaubild, bis das Video lädt; das alte erst entfernen, sonst blitzt es kurz auf.
        self.poster.set_paintable(None::<&gdk::Paintable>);
        self.poster.remove_css_class("loaded");
        self.poster.set_visible(media.poster);
        let url = pick_variant(&media.variants, saved_quality());
        let base_opts = media.opts.join(",");
        let opts = with_start(&base_opts, if media.live { 0.0 } else { media.start.unwrap_or(0.0) });
        let live = media.live;
        self.track.replace(Track { media: Some(media), base_opts, started: Some(Instant::now()), load: Some((url.clone(), opts.clone())), ..Default::default() });
        self.controls.reset(live);
        self.area.grab_focus();
        if !self.gl_ready.get() {
            // Video-Fläche noch nicht bereit (erstes Video der Sitzung): laden, sobald sie da ist.
            self.pending.replace(Some((url, opts)));
            return;
        }
        // loadfile <url> <flags> <index> <optionen>; index -1 = ohne Playlist-Position.
        if let Err(e) = self.mpv.command("loadfile", &[&url, "replace", "-1", &opts]) {
            eprintln!("mpv loadfile: {e}");
        }
    }

    /// Läuft ein Video (geladen, kein Fehler, nicht am Ende)? Dann geht es beim Verlassen in den Miniplayer.
    pub fn active(&self) -> bool {
        self.track.borrow().load.is_some() && !self.msg.is_visible() && !self.end_home.parent().is_some_and(|b| b.is_visible())
    }

    /// Wiedergabe beenden (Miniplayer schließen, App schließen). Speichert vorher die Position.
    pub fn stop(&self) {
        save(self.mpv, &self.track, &*self.hooks);
        // ponytail: wartet auch beim Schließen des Miniplayers (kurz); nur beim App-Ende wäre es nötig.
        report(self.mpv, &self.track, &*self.hooks, true);
        self.track.replace(Track::default());
        self.pending.take();
        let _ = self.mpv.command("stop", &[]);
        self.poster.set_visible(false);
        self.msg.set_visible(false);
        self.retry.set_visible(false);
        self.spinner.set_visible(false);
        if let Some(f) = self.on_playing.borrow().as_ref() {
            f(false);
        }
    }

    fn speed(&self, dir: i32) -> bool {
        self.controls.step_speed(dir);
        true
    }

    /// Strg+→/←: nächstes/voriges Kapitel, mit Einblendung des Titels.
    pub fn chapter(&self, dir: i32) {
        match self.controls.chapter_jump(dir) {
            Some(t) if !t.is_empty() => toast(&self.flash, &t),
            Some(_) => {}
            None if self.controls.has_chapters() => {}
            None => toast(&self.flash, "No chapters"),
        }
        self.controls.poke();
    }

    /// Tastenkürzel wie bei YouTube. Gibt `true` zurück, wenn die Taste verwendet wurde.
    pub fn key(&self, key: gdk::Key) -> bool {
        // Sprünge mit Einblendung; Lautstärke/Ton/Tempo blenden über ihre Ereignisse ein.
        let jump = |s: i32| {
            self.controls.forget_chapter_jump();
            let _ = self.mpv.command("seek", &[&s.to_string()]);
            toast(&self.flash, &if s < 0 { format!("−{} s", -s) } else { format!("+{s} s") });
            self.controls.poke();
            true
        };
        // 0–9: an 0–90 % springen (wie bei YouTube).
        if let Some(d) = key.to_unicode().and_then(|c| c.to_digit(10)) {
            self.controls.forget_chapter_jump();
            let _ = self.mpv.command("seek", &[&(d * 10).to_string(), "absolute-percent"]);
            self.controls.poke();
            return true;
        }
        // Lautstärke und Ton weich (controls.rs), nicht als harter Sprung.
        match key {
            gdk::Key::Up | gdk::Key::plus | gdk::Key::KP_Add => {
                self.controls.step_volume(5.0);
                return true;
            }
            gdk::Key::Down | gdk::Key::minus | gdk::Key::KP_Subtract => {
                self.controls.step_volume(-5.0);
                return true;
            }
            gdk::Key::m => {
                self.controls.toggle_mute();
                return true;
            }
            _ => {}
        }
        let cmd: &[&str] = match key {
            gdk::Key::space | gdk::Key::k => &["cycle", "pause"],
            gdk::Key::Left => return jump(-5),
            gdk::Key::Right => return jump(5),
            gdk::Key::j => return jump(-10),
            gdk::Key::l => return jump(10),
            // Bild für Bild (bei Pause), wie bei YouTube.
            gdk::Key::comma => &["frame-back-step"],
            gdk::Key::period => &["frame-step"],
            gdk::Key::c => {
                if !self.controls.toggle_subs() {
                    toast(&self.flash, "No subtitles");
                }
                self.controls.poke();
                return true;
            }
            gdk::Key::greater => return self.speed(1),
            gdk::Key::less => return self.speed(-1),
            _ => return false,
        };
        let _ = self.mpv.command(cmd[0], &cmd[1..]);
        self.controls.poke();
        true
    }
}

/// Liegt `w` im Miniplayer (Ansehen-Seite mit Klasse `mini`)?
fn in_mini(w: &gtk::Widget) -> bool {
    std::iter::successors(Some(w.clone()), |w| w.parent()).any(|w| w.has_css_class("mini"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_outside_runtime() {
        // Normaler Test-Thread ohne Tokio-Kontext, wie der GTK-Thread.
        let t = std::time::Instant::now();
        wait_at_most(Duration::from_millis(50), std::future::pending());
        assert!(t.elapsed() >= Duration::from_millis(50));
        wait_at_most(Duration::from_secs(5), async {});
    }

    fn l(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn reason_extracts_cause() {
        let log = l(&[
            "ERROR: [youtube] jNQXAC9eVRc: This video is unavailable",
            "youtube-dl failed: unexpected error occurred",
            "Failed to recognize file format.",
        ]);
        assert_eq!(reason(&log, "x"), "This video is unavailable");
        let bot = l(&["ERROR: [youtube] abc: Sign in to confirm you're not a bot. Use --cookies"]);
        assert_eq!(reason(&bot, "x"), "Sign in to confirm you're not a bot. Use --cookies");
        assert_eq!(reason(&l(&["Failed to recognize file format."]), "x"), "Failed to recognize file format.");
        assert_eq!(reason(&[], "fallback"), "fallback");
    }

    #[test]
    fn options_and_quality() {
        assert_eq!(quoted("https://a,b"), "%11%https://a,b");
        assert_eq!(with_start("referrer=%2%x/", 12.34), "referrer=%2%x/,start=12.3");
        assert_eq!(with_start("", 5.0), "start=5.0");
        assert_eq!(with_start("a=1", 0.0), "a=1");
        let v: Vec<(u32, String)> = vec![(1080, "a".into()), (720, "b".into()), (360, "c".into())];
        assert_eq!(pick_variant(&v, 1080), "a");
        assert_eq!(pick_variant(&v, 2160), "a", "4K gewählt, nur 1080p da: beste vorhandene");
        assert_eq!(pick_variant(&v, 480), "c");
        assert_eq!(pick_variant(&v, 240), "c");
        assert_eq!(pick_variant(&[(0, "yt".into())], 720), "yt", "Höhe unbekannt passt immer");
        let h = |k: &str, v: &str| (k.to_string(), v.to_string());
        let o = header_opts(&[h("User-Agent", "UA"), h("Referer", "https://r/"), h("Cookie", "a=1, b=2")]);
        assert_eq!(o, vec!["user-agent=%2%UA".to_string(), "referrer=%10%https://r/".into(), "http-header-fields=%17%Cookie: a=1\\, b=2".into()]);
    }
}
