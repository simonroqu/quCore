# qu-core – gemeinsamer Unterbau von AniQu, BoxQu und Tubequ (Rust + GTK4 + libmpv)

Bibliothek (Crate `qu-core`, im Code `qu_core`), die alle drei Apps einbinden: `~/Work/aniqu`, `~/Work/boxqu`,
`~/Work/tubequ`. Was hier steht, gilt für alle drei – Änderungen hier, nicht in den Apps.

**Code-Kommentare: Deutsch. Alle Texte in der Oberfläche: Englisch.**

**Diese Datei bei jeder Änderung am Projekt mitpflegen** (gleicher Schritt wie die Code-Änderung).

## Einbinden, bauen

```toml
# Cargo.toml der App
qu-core = { git = "https://github.com/simonroqu/quCore", tag = "v0.1.0" }
```
Lokal entwickeln ohne Tag/Push: in `~/.cargo/config.toml` (nicht im Repo)
```toml
[patch."https://github.com/simonroqu/quCore"]
qu-core = { path = "/home/simon/Work/quCore" }
```
Neuer Stand für die Apps: hier committen, neuen Tag setzen und pushen (`v0.1.1` …), dann in jeder App `tag = …` anheben
und `cargo update -p qu-core`. Ein Lockfile, das mit der lokalen Umleitung entstand, enthält keine Quelle für qu-core – Cargo
ergänzt sie beim Bauen ohne Umleitung selbst (geprüft 2026-11-17, auch mit `--release`).

`cargo build && cargo test` hier; danach **alle drei Apps** bauen und testen (`cargo build && cargo test` in jeder).

## Eine Qu-App = `qu.toml` + `style.css` (optional) + eigener Code

```rust
fn main() -> glib::ExitCode {
    qu_core::start(include_str!("../qu.toml"), build)   // build: Fenster bauen, gibt Startargument-Handler zurück
}
```
Im `build` der App: `qu_core::style::install(&display, include_str!("style.css"))`, Menü
`qu_core::navbar::NavBar::new(&qu_core::config().nav)`, Fenster `qu_core::window(app, &root)`, Suche
`search::SearchModal::from_config(app_suggest, on_search)`. Werte aus der Konfig: `qu_core::config()`.

### `qu.toml` (im Repo der App, beim Bauen eingebettet; `Config::parse` prüft, Tippfehler = Fehler)

```toml
[app]
name  = "tubequ"            # Ordner ~/.config/<name> …, Dev-Schalter <NAME>_DEV (eigene Instanz)
title = "Tubequ"            # Fenstertitel
id    = "dev.tubequ.Tubequ" # Application-ID / Fensterklasse

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
| `app.rs` | App-Name (`init("aniqu")` als Erstes in `main`; ohne: Programmname) und Ordner: `config_file` (`~/.config/<app>/`), `data_file` (`~/.local/share/<app>/`), `cache_dir` (`~/.cache/<app>/`) |
| `theme.rs` | Farben aus Omarchy `colors.toml`, Rundung aus Theme-`hyprland.lua`, Live-Reload; Fallback = Farben von „Last Horizon“ |
| `style.css` | Gemeinsames Layout-CSS (`qu_core::CSS`; Bubbles, Timeline, Suche, Tabs, Platzhalter …), nutzt `--radius-*` aus `theme.rs`. Die App lädt `format!("{}{}", qu_core::CSS, <eigenes CSS>)` |
| `icons.rs` | Icon-Satz (`data/icons/qu-*-symbolic.svg`, eingebettet, beim Start nach `~/.cache/<app>/icons`); neue Icons in `ICONS` eintragen (Arraylänge!). `spinner()` statt `gtk::Spinner` (friert in GTK 4.24 ein) |
| `nav_shape.rs` | Selbst gezeichnete Form der Nav-Bubble: Pille → umgedrehtes L (Burger offen) → liegendes U (untere Reihe, `set_parts`, nur Tubequ). Ohne `set_parts` ist die obere Reihe so breit wie die ganze Box (AniQu/BoxQu-Verhalten) |
| `fly.rs` | Öffnen-Animation wie beim iPhone: `Zoom` umgibt die Ansehen-Seite; der Player (Anker) beginnt genau auf dem Startrechteck (Karte/Kachel, `fly::register` bzw. alter Player-Platz) und wächst 380 ms an seinen Platz, die Seite skaliert mit (nur Zeichnen, kein Layout pro Bild). Dahinter beim Großwerden ein abgedunkeltes Standbild der alten Seite (`backdrop`). `after(f)`: nach der Animation ausführen (Miniplayer-Knöpfe erst, wenn er klein ist). Stack-Wechsel von/zur Ansehen-Seite **ohne** Überblendung (sonst beim Schrumpfen dahinter schwarz). `contains` = false, damit der Zoom im Miniplayer-Modus keine Klicks schluckt. Aus bei `gtk-enable-animations=false` |
| `smooth.rs` | Weiches Scrollen mit dem Mausrad (Raststufen gleiten, Touchpad bleibt bei GTK), Tastatur-Fokus im Raster |
| `search.rs` | Schwebendes Suchfenster (wie Spotlight), `SearchModal::new(placeholder, suggest, on_search)`; Vorschläge von der App (`from_history()` = gefilterte letzte Suchen, Tubequ: YouTube). Suchverlauf `searches`/`add_search` in `~/.local/share/<app>/searches.txt` (8 Einträge) |
| `upscale.rs` | Upscaling im Player-Einstellungsmenü: Off (`scale=lanczos`), Sharp (`ewa_lanczossharp`), AI. Shader-Set wählt die App (`Shaders::Fsrcnnx` = FSRCNNX_x2_16-0-4-1, nur Luma von YUV, ab Vergrößerung > 1,3; `Shaders::Anime4k` = Mode A HQ). Shader eingebettet (`data/shaders/`, Lizenzen daneben), beim ersten Gebrauch nach `~/.cache/<app>/shaders/`, per `change-list glsl-shaders`. Gespeichert in `~/.config/<app>/upscaling`. Geprüft 2026-11-17 (RTX 4070 Ti SUPER, `vo-passes`). RTX Video Super Resolution gibt es in mpv unter Linux nicht |

## Noch in den Apps (Kandidaten für später)

`player.rs`, `controls.rs`, `watch.rs`: bei AniQu/BoxQu fast gleich, bei Tubequ stark erweitert (Vorschaubilder der
Timeline, SponsorBlock, Kapitel-Zeichnung, Konten). Erst zusammenführen, wenn die Tubequ-Extras als Erweiterung andocken.

## Regeln (gelten für alle Apps)

- Popover in Theme-Farben, Auswahlpunkte in Akzentfarbe (`theme.rs`).
- Runde Bilder: Klasse `.avatar` + `overflow(Hidden)` + `halign/valign(Center)`.
- CSS-Priorität: App-CSS (inkl. `qu_core::CSS`) = `APPLICATION + 1`, `theme.rs` = `APPLICATION`. GTK entscheidet bei
  gleicher Priorität nach Ladereihenfolge. GTK-Default gibt `scale highlight` einen blauen Rahmen + `margin: -1px` →
  bei eigenen Reglern `border: none; margin: 0;` und eine `min-height`.
- Rundungen über `--radius-s/-m/-l/-pill` (von `theme.rs`).
