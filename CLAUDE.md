# qu-core – gemeinsamer Unterbau von AniQu, BoxQu und TubeQu (Rust + GTK4 + libmpv)

Bibliothek (Crate `qu-core`, im Code `qu_core`), die alle drei Apps einbinden: `~/Work/aniqu`, `~/Work/boxqu`,
`~/Work/tubequ`. Was hier steht, gilt für alle drei – Änderungen hier, nicht in den Apps.

**Code-Kommentare: Deutsch. Alle Texte in der Oberfläche: Englisch.**

**Diese Datei bei jeder Änderung am Projekt mitpflegen** (gleicher Schritt wie die Code-Änderung).

## Einbinden, bauen

```toml
# Cargo.toml der App
qu-core = { git = "https://github.com/simonroqu/quCore" }
```
Lokal entwickeln ohne Push: in `~/.cargo/config.toml` (nicht im Repo)
```toml
[patch."https://github.com/simonroqu/quCore"]
qu-core = { path = "/home/simon/Work/quCore" }
```
Kein Tag: die Apps nehmen den Hauptzweig. Neuer Stand = hier committen und pushen; die Install-Skripte der Apps holen ihn
per `cargo update -p qu-core` (Cargo.lock hält sonst den alten fest). Achtung: was auf main liegt, landet bei der nächsten
Installation in allen Apps, also nur Funktionierendes pushen. Ein Lockfile, das mit der lokalen Umleitung entstand, enthält keine Quelle für qu-core – Cargo
ergänzt sie beim Bauen ohne Umleitung selbst (geprüft 2026-11-17, auch mit `--release`).

Feature `gui` (Standard): alles mit GTK/libmpv. Ohne (`default-features = false`) bleiben `app` (Ordner; `set_dirs` für
Android), `config`, `progress`, `segments`, `store` (inkl. Suchverlauf), `Shaders` und `RT`/`bg` – für App-Logik ohne Oberfläche (z. B. `aniqu-core`).
Neues ohne GTK dort lassen, GTK-Code hinter `#[cfg(feature = "gui")]`.

`cargo build && cargo test` hier, zusätzlich `cargo build --no-default-features`; danach **alle drei Apps** bauen und testen (`cargo build && cargo test` in jeder).

## Eine Qu-App = `qu.toml` + Icon + `style.css` (optional) + eigener Code

```rust
fn main() -> glib::ExitCode {
    // build: Fenster bauen, gibt Startargument-Handler zurück; Icon für `<app> --install`
    qu_core::start(include_str!("../qu.toml"), include_str!("../data/<id>.svg"), build)
}
```
`install.sh` = `cargo build --release && ./target/release/<app> --install` (Programm nach `~/.local/bin`, Icon,
Startmenü-Eintrag aus `[app]`; geprüft 2026-11-17: Einträge byte-gleich mit den früheren `.desktop`-Dateien).
macOS: `--install` legt `~/Applications/<Titel>.app` (Info.plist, ohne Icon) an und `~/.local/bin/<app>` als Symlink darauf;
GL-Funktionen dort aus `OpenGL.framework` (`player.rs`). Windows: `--install` geht nicht (kein HOME), das macht `install.ps1` der App.
Die Install-Skripte (install.sh/.ps1, windows-bundle.sh) liegen gleich in allen Apps, beschrieben in `~/Work/aniqu/CLAUDE.md`.
App-Code, der die Konfig braucht, testet mit `qu_core::config::load(include_str!("../qu.toml"))` zuerst.
Im `build` der App:
- `qu_core::style::install(&display, include_str!("style.css"))` (Theme, Icons, CSS-Schichten)
- `let shell = qu_core::nav::Shell::new(app)` (Fenster, Seitenstapel `shell.stack`, Menü `shell.navbar`)
- `let player = Rc::new(qu_core::player::Player::<M>::new(hooks))` (M = was die App abspielt, `Hooks` = App-Eigenes)
- Ansehen-Seite der App um `qu_core::watch::WatchFrame::new(&player.widget)`, Infos in `frame.below`
- `let nav = qu_core::nav::Nav::new(&shell, player.clone(), frame)` (Verlauf, Miniplayer, Zoom-Animation)
- Suche `search::SearchModal::from_config(app_suggest, on_search)`
- Seiten mit `shell.stack.add_named(…)`, „watch“ = leerer Platzhalter; Werte aus der Konfig: `qu_core::config()`.

### `qu.toml` (im Repo der App, beim Bauen eingebettet; `Config::parse` prüft, Tippfehler = Fehler)

```toml
[app]
name  = "tubequ"            # Ordner ~/.config/<name> …, Dev-Schalter <NAME>_DEV (eigene Instanz)
title = "TubeQu"            # Fenstertitel
id    = "dev.tubequ.Tubequ" # Application-ID / Fensterklasse / Icon-Name
generic_name = "YouTube client"                     # Startmenü (optional)
comment = "Ad-free YouTube: home feed, search, channels and live streams"
keywords = ["youtube", "video", "stream", "live"]

[nav]                       # Menü oben links
top  = ["menu", "back", "home", "search", "busy"]   # eingebaut: back home search busy menu
menu = ["later", "history", "account"]             # Spalte unter dem Burger (leer = kein Burger)

[[nav.item]]                # eigener Knopf: Icon qu-<icon>-symbolic; Klick hängt die App an navbar.button(id)
id = "later"
icon = "later"
tooltip = "Watch later"

[[nav.item]]                # Slot: leere Box, die App füllt sie (navbar.slot(id)); expands = klappt nach rechts auf
id = "account"
slot = true
expands = true

[search]
placeholder = "Search YouTube"
suggestions = "app"         # "history" = gefilterte letzte Suchen, "app" = Vorschläge aus dem App-Code

[player]                    # alles optional
qualities = [2160, 1440, 1080, 720, 480, 360]       # Menü „Quality“ (Standard [1080, 720, 480, 360])
default_quality = 1080      # muss in qualities stehen
upscaler = "fsrcnnx"        # Shader-Set für „AI“-Upscaling: fsrcnnx | anime4k
segments = "auto"           # Abschnitte (Opening/Sponsor): "button" = Knopf „Skip …“ (Standard), "auto" = überspringen
auto_subtitles = false      # true (Standard): Untertitel beim Laden wie zuletzt (an/aus, Sprache)

[progress]                  # optional
min_watched = 30            # kürzer geschaut (s) wird nicht gespeichert
finished_at = 0.95          # ab diesem Anteil fertig (Standard 0.9)
rewind = 3                  # beim Fortsetzen zurück (s)

[grid]                      # optional (Standard: Poster 2:3 wie AniQu/BoxQu)
card_width = 320            # Spalten richten sich nach dieser Kartenbreite (Karten wachsen mit)
min_card_width = 200        # Strg+Mausrad-Spalten nur, solange Karten so breit bleiben
max_columns = 4
zoom_columns = [3, 6]       # mit Strg+Mausrad wählbar
image = "16:9"              # Bildformat Breite:Höhe
info_lines = 2              # Infozeilen im Lade-Platzhalter
```
Jede App hat einen Test `qu_toml` (in `main.rs`), der die Datei prüft. Neue Felder: in `config.rs` (mit
`deny_unknown_fields`) und hier dokumentieren; nur Felder anlegen, die der Code auch liest.

### Styling in Schichten

`style::install`: Theme (`APPLICATION`) < Kern-CSS (`+1`) < App-`style.css` (`+2`) < `~/.config/<app>/style.css` (`+3`,
ohne Neubauen). Spätere Schicht gewinnt immer, unabhängig von der Spezifität. Gemeinsames gehört in `src/style.css`
hier, nur App-Eigenes ins App-CSS.

## Aufbau (`src/`)

| Datei | Aufgabe |
|---|---|
| `lib.rs` | `start` (Konfig lesen, Name setzen, Absturz-Log `~/.cache/<app>/crash.log`, Einzelinstanz bzw. `<NAME>_DEV`), `window` (Fenster ohne Kopfzeile, Fensterknöpfe außerhalb von Tiling-WMs), `tiling_wm`, `RT`/`bg` (tokio im Hintergrund), `CSS` |
| `config.rs` | `qu.toml` lesen und prüfen; `qu_core::config()` = Konfig der laufenden App |
| `navbar.rs` | Menü oben links aus `[nav]`: Knöpfe/Slots, Burger-Spalte, zuklappen nach Auswahl und bei Klick daneben (`close_on_outside_click`), `connect_menu`, Tastatur (`key`: Esc auf, Pfeile wählen, Enter drückt; aufklappende Slots lassen das Menü offen), `nav_button` für weitere Icon-Knöpfe |
| `busy.rs` | Lade-Kreis (`busy` im Menü): dreht sich, solange ein `Busy` lebt |
| `style.rs` | Theme, Icons, CSS-Schichten (siehe oben) |
| `app.rs` | App-Name (`init("aniqu")` als Erstes in `main`; ohne: Programmname) und Ordner: `config_file` (`~/.config/<app>/`), `data_file` (`~/.local/share/<app>/`), `cache_dir` (`~/.cache/<app>/`); unter Windows `%APPDATA%\<app>` bzw. Cache `%LOCALAPPDATA%\<app>` |
| `theme.rs` | Farben aus Omarchy `colors.toml`, Rundung aus Theme-`hyprland.lua`, Live-Reload; Fallback = Farben von „Last Horizon“ |
| `style.css` | Gemeinsames Layout-CSS (`qu_core::CSS`; Bubbles, Timeline, Suche, Tabs, Platzhalter …), nutzt `--radius-*` aus `theme.rs`. Die App lädt `format!("{}{}", qu_core::CSS, <eigenes CSS>)` |
| `icons.rs` | Icon-Satz (`data/icons/qu-*-symbolic.svg`, eingebettet, beim Start nach `~/.cache/<app>/icons`); neue Icons in `ICONS` eintragen (Arraylänge!). `spinner()` statt `gtk::Spinner` (friert in GTK 4.24 ein) |
| `nav_shape.rs` | Selbst gezeichnete Form der Nav-Bubble: Pille → umgedrehtes L (Burger offen) → liegendes U (untere Reihe, `set_parts`, nur TubeQu). Ohne `set_parts` ist die obere Reihe so breit wie die ganze Box (AniQu/BoxQu-Verhalten) |
| `fly.rs` | Öffnen-Animation wie beim iPhone: `Zoom` umgibt die Ansehen-Seite; der Player (Anker) beginnt genau auf dem Startrechteck (Karte/Kachel, `fly::register` bzw. alter Player-Platz) und wächst 380 ms an seinen Platz, die Seite skaliert mit (nur Zeichnen, kein Layout pro Bild). Dahinter beim Großwerden ein abgedunkeltes Standbild der alten Seite (`backdrop`). `after(f)`: nach der Animation ausführen (Miniplayer-Knöpfe erst, wenn er klein ist). Stack-Wechsel von/zur Ansehen-Seite **ohne** Überblendung (sonst beim Schrumpfen dahinter schwarz). `contains` = false, damit der Zoom im Miniplayer-Modus keine Klicks schluckt. Aus bei `gtk-enable-animations=false` |
| `smooth.rs` | Weiches Scrollen mit dem Mausrad (Raststufen gleiten, Touchpad bleibt bei GTK), Tastatur-Fokus im Raster |
| `search.rs` | Schwebendes Suchfenster (wie Spotlight), `SearchModal::new(placeholder, suggest, on_search)`; Vorschläge von der App (`from_history()` = gefilterte letzte Suchen, TubeQu: YouTube). Suchverlauf `searches`/`add_search` (liegen in `store`, ohne GTK; hier re-exportiert) in `~/.local/share/<app>/searches.txt` (8 Einträge) |
| `controls.rs` | Bedienleiste über dem Video (Timeline mit Kapiteln/Abschnitten/Storyboard-Vorschau, Play, Lautstärke weich, Zeit, Einstellungen Qualität/Tempo/Upscaling, Untertitel-Knopf: Linksklick/`c` an/aus, Rechtsklick Menü Sprache/Größe/Hintergrund, gespeichert in `subtitles`), Auto-Ausblenden. `connect_quality` (Player lädt Stufe neu; Apps mit yt-dlp ersetzen das), `set_storyboard`, `apply_subs`, `timeline_top`, `fmt_duration`. Einstellungen in `~/.config/<app>/` (`quality`, `volume`, `subtitles`) |
| `player.rs` | `Player<M>`: libmpv in GtkGLArea (GL-Funktionen über `eglGetProcAddress`, unter Windows `wglGetProcAddress` + `opengl32.dll`), Meldungen/Spinner/Vorschaubild (`poster`), Endbildschirm (`end_next` setzt die App sichtbar), Abschnitte (Knopf oder automatisch), Einblendungen (nicht in den ersten 1,5 s), Fortschritt alle 5 s/bei Pause, Untertitel-Name aus `sub_tracks`. `Media` = was gespielt wird (Stufen, `loadfile`-Optionen via `quoted`/`header_opts`, Start, Tempo, live). `Hooks<M>`: `init` (mpv-Optionen, z. B. yt-dlp), `save`, `report` (+ `wait_at_most`), `segments`, `speed_changed`, `retry_opts` |
| `segments.rs` | `Segment` (Start, Ende, Name), `segment_at`, `to_skip` |
| `watch.rs` | `WatchFrame`: Player so hoch wie das Video, mittig mit Rand (85 % ab 1600 px), Strg+Mausrad-Breite (`player-width`), Vollbild, Miniplayer in der Ecke; `below` für die Infos der App; `reset` bei neuem Video |
| `nav.rs` | `Shell` (Fenster, `stack`, `page_layer`, Menü) und `Nav<M>` (Verlauf `go`/`back`/`forward`/`home`, Ansehen-Seite als Ebene, Miniplayer mit Knöpfen, ziehen → nächste Ecke, Zoom-Animation, Menü/Leiste blenden gemeinsam ein) |
| `store.rs` | Dateien in `~/.local/share/<app>/`: `read`, `write` (über Hilfsdatei + Umbenennen), Tabellen `lookup`/`put` (`key<TAB>wert`), Tempo pro Titel/Kanal `speed`/`set_speed` (`speeds.tsv`) |
| `progress.rs` | Fortschrittsregeln aus `[progress]`: `worth_saving`, `finished`, `fraction`, `resume_from`. Speichern (eigene Felder) macht die App |
| `cards.rs` | Raster-Bausteine aus `[grid]`: `columns`, `step_columns`, `image_height`, `label`, `part`, `ghost_card`, `COLUMN_SPACING`, `TOP_MARGIN` |
| `install.rs` | `<app> --install` (siehe oben) |
| `upscale.rs` | Upscaling im Player-Einstellungsmenü: Off (`scale=lanczos`), Sharp (`ewa_lanczossharp`), AI. Shader-Set wählt die App (`Shaders::Fsrcnnx` = FSRCNNX_x2_16-0-4-1, nur Luma von YUV, ab Vergrößerung > 1,3; `Shaders::Anime4k` = Mode A HQ). Shader eingebettet (`data/shaders/`, Lizenzen daneben), beim ersten Gebrauch nach `~/.cache/<app>/shaders/`, per `change-list glsl-shaders`. Gespeichert in `~/.config/<app>/upscaling`. Geprüft 2026-11-17 (RTX 4070 Ti SUPER, `vo-passes`). RTX Video Super Resolution gibt es in mpv unter Linux nicht |

## Android (`android/`, Stand 2026-11-18)

Gemeinsame Android-Bausteine aller Qu-Apps (Kotlin/Compose, Paket `dev.roqu.qu`), wie `src/` am Desktop:
| Datei | Aufgabe |
|---|---|
| `src/dev/roqu/qu/Ui.kt` | `QuTheme` (Farben = `DEFAULT_COLORS`), `Nav<P>` (Seitenstapel), `QuApp` (Seite + Nav-Bubble Zurück/Home/Suche + App-Knöpfe, Spotlight-Suche mit Verlauf, Zurück-Taste, Fokus Bubble → Seite), `tvFocus`, `glass`, `IconButton`, `Grid`, `PosterCard`, `ghosts`, `header`, `note`, `wide`, `TabButton`, `Tile`, `ghostTiles`, `DetailHeader` |
| `src/dev/roqu/qu/Player.kt` | `VideoPlayer(load, message, save, skips, next)` mit `Video` (HLS/DASH/Datei, getrennte Tonspur, Header, Untertitel, Start, Tempo, live), `Skip`, `Subtitle`, `Fullscreen` |
| `src/dev/roqu/qu/Icons.kt` | `QuIcons.*` aus `data/icons`, erzeugt von `python3 android/icons.py <namen…>` (immer die ganze Liste) |
| `build-rust.sh` | baut den FFI-Crate der App (`sh <qu-core>/android/build-rust.sh <app>-ffi`, aus dem `android/`-Ordner der App) |

Die Apps finden den Ordner über `cargo metadata` (Gradle, `app/build.gradle.kts`: `sourceSets["main"].kotlin.srcDir`) –
gleicher Stand wie der Rust-Teil, lokal der Patch. Werkzeuge, Waydroid-Test und Fernbedienungs-Regeln: `~/Work/aniqu/CLAUDE.md`
(Abschnitt Android). Seiten bleiben in den Apps (wie am Desktop). Abhängigkeiten (Compose, Coil, Media3 inkl. HLS/DASH)
trägt jede App in ihrer `build.gradle.kts` ein.

## Was bewusst in den Apps bleibt (so entschieden – nicht ohne Rückfrage ändern)

**Alle Seiten bleiben in den Apps** (Startseite/Raster-Logik, Titel- bzw. Kanalseite, Infos unter dem Video), auch
wenn sie sich bei AniQu/BoxQu ähneln. Der Kern liefert nur Bausteine dafür (`cards`, `WatchFrame::below`, `store` …).
Außerdem in den Apps: Datenquelle (hianime, MovieBox, YouTube), was gespeichert wird (Fortschritts-Einträge mit eigenen Feldern, Watchlist/„Watch later“) und
App-Eigenheiten (TubeQus Link-Weiche in `install.sh`, Kontozeile im Menü).

## Regeln (gelten für alle Apps)

- Popover in Theme-Farben, Auswahlpunkte in Akzentfarbe (`theme.rs`).
- Runde Bilder: Klasse `.avatar` + `overflow(Hidden)` + `halign/valign(Center)`.
- CSS-Priorität: App-CSS (inkl. `qu_core::CSS`) = `APPLICATION + 1`, `theme.rs` = `APPLICATION`. GTK entscheidet bei
  gleicher Priorität nach Ladereihenfolge. GTK-Default gibt `scale highlight` einen blauen Rahmen + `margin: -1px` →
  bei eigenen Reglern `border: none; margin: 0;` und eine `min-height`.
- Rundungen über `--radius-s/-m/-l/-pill` (von `theme.rs`).
