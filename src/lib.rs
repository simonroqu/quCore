//! Gemeinsamer Unterbau der Qu-Apps (AniQu, BoxQu, Tubequ): Pfade, Theme, CSS, Icons, Animationen,
//! Nav-Bubble, Suchfenster, Upscaling. Jede App ruft am Anfang `app::init("<name>")`.

pub mod app;
pub mod fly;
pub mod icons;
pub mod nav_shape;
pub mod search;
pub mod smooth;
pub mod theme;
pub mod upscale;

/// Gemeinsames CSS; die App lädt es zusammen mit ihrem eigenen (`format!("{}{}", qu_core::CSS, …)`).
pub const CSS: &str = include_str!("style.css");
