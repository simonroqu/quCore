# qu-core – gemeinsamer Unterbau von AniQu, BoxQu und Tubequ (Rust + GTK4 + libmpv)

Bibliothek (Crate `qu-core`, im Code `qu_core`), die alle drei Apps einbinden: `~/Work/aniqu`, `~/Work/boxqu`,
`~/Work/tubequ`. Was hier steht, gilt für alle drei – Änderungen hier, nicht in den Apps.

**Code-Kommentare: Deutsch. Alle Texte in der Oberfläche: Englisch.**

**Diese Datei bei jeder Änderung am Projekt mitpflegen** (gleicher Schritt wie die Code-Änderung).

## Einbinden, bauen

```toml
# Cargo.toml der App
qu-core = { git = "https://github.com/<user>/quCore", tag = "v0.1.0" }
```
Lokal entwickeln ohne Tag/Push: in `~/.cargo/config.toml` (nicht im Repo)
```toml
[patch."https://github.com/<user>/quCore"]
qu-core = { path = "/home/simon/Work/quCore" }
```
Bis das Repo existiert, binden die Apps `qu-core = { path = "../quCore" }` ein.

`cargo build && cargo test` hier; danach **alle drei Apps** bauen und testen (`cargo build && cargo test` in jeder).

## Aufbau (`src/`)

| Datei | Aufgabe |
|---|---|
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
