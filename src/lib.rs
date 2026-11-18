//! Gemeinsamer Unterbau der Qu-Apps (AniQu, BoxQu, TubeQu). Eine App besteht aus `qu.toml` (Name, Menü,
//! Suche …), optional `style.css` und ihrem eigenen Code (Datenquelle, Seiten); `start` startet sie.

pub mod app;
#[cfg(feature = "gui")]
pub mod cards;
#[cfg(feature = "gui")]
pub mod busy;
pub mod config;
#[cfg(feature = "gui")]
pub mod controls;
#[cfg(feature = "gui")]
pub mod fly;
#[cfg(feature = "gui")]
pub mod icons;
#[cfg(feature = "gui")]
mod install;
#[cfg(feature = "gui")]
pub mod nav;
#[cfg(feature = "gui")]
pub mod nav_shape;
#[cfg(feature = "gui")]
pub mod player;
pub mod progress;
#[cfg(feature = "gui")]
pub mod navbar;
#[cfg(feature = "gui")]
pub mod search;
pub mod segments;
#[cfg(feature = "gui")]
pub mod smooth;
pub mod store;
pub mod storyboard;
#[cfg(feature = "gui")]
pub mod style;
#[cfg(feature = "gui")]
pub mod theme;
pub mod upscale;
#[cfg(feature = "gui")]
pub mod watch;

#[cfg(feature = "gui")]
mod gui;
#[cfg(feature = "gui")]
pub use gui::*;

use std::sync::LazyLock;

pub use config::get as config;

/// Tokio für Netzwerk & Co. – im GTK-Thread wird nur gewartet (`bg`).
pub static RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| tokio::runtime::Runtime::new().expect("tokio"));

/// Führt `fut` im Hintergrund (tokio) aus; das Ergebnis kommt im GTK-Thread an.
pub async fn bg<T: Send + 'static>(fut: impl Future<Output = T> + Send + 'static) -> T {
    RT.spawn(fut).await.expect("Hintergrund-Task abgestürzt")
}
