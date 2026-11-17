//! Gemeinsamer Unterbau der Qu-Apps (AniQu, BoxQu, Tubequ). Eine App besteht aus `qu.toml` (Name, Menü,
//! Suche …), optional `style.css` und ihrem eigenen Code (Datenquelle, Seiten); `start` startet sie.

pub mod app;
pub mod cards;
pub mod busy;
pub mod config;
pub mod controls;
pub mod fly;
pub mod icons;
mod install;
pub mod nav;
pub mod nav_shape;
pub mod player;
pub mod progress;
pub mod navbar;
pub mod search;
pub mod segments;
pub mod smooth;
pub mod store;
pub mod style;
pub mod theme;
pub mod upscale;
pub mod watch;

use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::OnceCell;
use std::rc::Rc;
use std::sync::LazyLock;

pub use config::get as config;

/// Gemeinsames CSS (lädt `style::install`).
pub const CSS: &str = include_str!("style.css");

/// Tokio für Netzwerk & Co. – im GTK-Thread wird nur gewartet (`bg`).
pub static RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| tokio::runtime::Runtime::new().expect("tokio"));

/// Führt `fut` im Hintergrund (tokio) aus; das Ergebnis kommt im GTK-Thread an.
pub async fn bg<T: Send + 'static>(fut: impl Future<Output = T> + Send + 'static) -> T {
    RT.spawn(fut).await.expect("Hintergrund-Task abgestürzt")
}

thread_local! {
    /// Das Fenster gibt es nur einmal; weitere Starts geben ihre Argumente hierher weiter.
    static OPEN: OnceCell<Rc<dyn Fn(Option<String>)>> = const { OnceCell::new() };
}

/// App starten: `qu.toml` lesen, Ordner/Name setzen, Absturz-Log, Einzelinstanz (`<NAME>_DEV` = eigene
/// Instanz daneben). `build` baut das Fenster einmal und liefert, was mit Startargumenten passiert
/// (`app <link>`/`app <suche>`; auch von weiteren Starts der laufenden App). `icon_svg`: App-Icon für
/// `app --install` (Programm, Icon und Startmenü-Eintrag für den aktuellen Nutzer).
pub fn start(qu_toml: &str, icon_svg: &str, build: fn(&gtk::Application) -> Rc<dyn Fn(Option<String>)>) -> glib::ExitCode {
    let cfg = config::set(config::Config::parse(qu_toml).unwrap_or_else(|e| panic!("{e}")));
    app::init(Box::leak(cfg.app.name.clone().into_boxed_str()));
    if std::env::args().nth(1).as_deref() == Some("--install") {
        return match install::run(icon_svg) {
            Ok(()) => glib::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("Installieren fehlgeschlagen: {e}");
                glib::ExitCode::FAILURE
            }
        };
    }
    // libmpv verlangt LC_NUMERIC=C; ohne setlocale durch GTK bleibt alles auf C.
    gtk::disable_setlocale();
    // Panik-Meldung zusätzlich in ~/.cache/<app>/crash.log: aus dem Startmenü gestartet geht stderr
    // verloren, und Paniken in GTK-Callbacks beenden die App sofort (abort).
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        use std::io::Write;
        let path = app::cache_dir().join("crash.log");
        let _ = std::fs::create_dir_all(app::cache_dir());
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let _ = writeln!(f, "--- {now}\n{info}\n{}", std::backtrace::Backtrace::force_capture());
        }
        default_hook(info);
    }));
    let dev = std::env::var_os(format!("{}_DEV", cfg.app.name.to_uppercase())).is_some();
    let app = gtk::Application::builder()
        .application_id(&cfg.app.id)
        .flags(if dev {
            // Entwicklung: eigene Instanz neben der installierten, statt Aufrufe an sie weiterzugeben.
            gio::ApplicationFlags::HANDLES_COMMAND_LINE | gio::ApplicationFlags::NON_UNIQUE
        } else {
            gio::ApplicationFlags::HANDLES_COMMAND_LINE
        })
        .build();
    // Läuft die App schon, landet ein weiterer Start hier im laufenden Prozess.
    app.connect_command_line(move |app, cmdline| {
        let arg = cmdline.arguments().get(1).map(|a| a.to_string_lossy().into_owned());
        let open = OPEN.with(|o| o.get_or_init(|| build(app)).clone());
        open(arg);
        glib::ExitCode::SUCCESS
    });
    app.run()
}

/// Tiling-Window-Manager: Fenster schließt man per Tastenkürzel, keine Fensterknöpfe/Titelleiste nötig.
pub fn tiling_wm() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
    ["hyprland", "sway", "niri", "river", "i3"].iter().any(|wm| desktop.contains(wm))
}

/// Hauptfenster ohne Kopfzeile mit `root` als Inhalt, Titel aus `qu.toml`. Außerhalb von Tiling-WMs
/// kommen die Fensterknöpfe oben rechts als Bubble dazu, und an freien Flächen zieht man das Fenster.
pub fn window(app: &gtk::Application, root: &gtk::Overlay) -> gtk::ApplicationWindow {
    if !tiling_wm() {
        let controls = gtk::WindowControls::builder().side(gtk::PackType::End).css_classes(["bubble"]).build();
        let wc = gtk::Box::builder().halign(gtk::Align::End).valign(gtk::Align::Start).css_classes(["nav-bubble"]).build();
        wc.append(&controls);
        root.add_overlay(&wc);
    }
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title(&config().app.title)
        .default_width(1280)
        .default_height(860)
        .decorated(false)
        .build();
    if tiling_wm() {
        window.set_child(Some(root));
    } else {
        window.set_child(Some(&gtk::WindowHandle::builder().child(root).build()));
    }
    window
}
