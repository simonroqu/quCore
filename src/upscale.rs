//! Upscaling beim Zeichnen (mpv auf der GPU): „Sharp“ = besserer Skalierfilter, „AI“ = GLSL-Shader
//! (eingebettet, Lizenzen in `data/shaders/`). Die App wählt das Shader-Set (`Shaders`). Wirkt nur, wenn das
//! Bild größer gezeichnet wird als das Video (z. B. 1080p im Vollbild auf 1440p). NVIDIAs RTX Video Super
//! Resolution kann mpv unter Linux nicht ansprechen.

#![cfg_attr(not(feature = "gui"), allow(dead_code))] // ohne GUI bleibt nur `Shaders` (für qu.toml)

#[cfg(feature = "gui")]
use libmpv2::Mpv;

/// Shader-Set für „AI“ (`[player] upscaler` in qu.toml: `fsrcnnx`, `anime4k`).
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shaders {
    /// FSRCNNX_x2_16-0-4-1 (igv, GPL-3): Filme/Serien/YouTube. Nur Luma von YUV-Videos, ab Vergrößerung > 1,3.
    Fsrcnnx,
    /// Anime4K v4 „Mode A (HQ)“ (bloc97, MIT): für Anime.
    Anime4k,
}

const FSRCNNX: [(&str, &str); 1] = [("FSRCNNX_x2_16-0-4-1.glsl", include_str!("../data/shaders/FSRCNNX_x2_16-0-4-1.glsl"))];
const ANIME4K: [(&str, &str); 6] = [
    ("Anime4K_Clamp_Highlights.glsl", include_str!("../data/shaders/Anime4K_Clamp_Highlights.glsl")),
    ("Anime4K_Restore_CNN_VL.glsl", include_str!("../data/shaders/Anime4K_Restore_CNN_VL.glsl")),
    ("Anime4K_Upscale_CNN_x2_VL.glsl", include_str!("../data/shaders/Anime4K_Upscale_CNN_x2_VL.glsl")),
    ("Anime4K_AutoDownscalePre_x2.glsl", include_str!("../data/shaders/Anime4K_AutoDownscalePre_x2.glsl")),
    ("Anime4K_AutoDownscalePre_x4.glsl", include_str!("../data/shaders/Anime4K_AutoDownscalePre_x4.glsl")),
    ("Anime4K_Upscale_CNN_x2_M.glsl", include_str!("../data/shaders/Anime4K_Upscale_CNN_x2_M.glsl")),
];

impl Shaders {
    fn files(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Shaders::Fsrcnnx => &FSRCNNX,
            Shaders::Anime4k => &ANIME4K,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Shaders::Fsrcnnx => "AI (FSRCNNX)",
            Shaders::Anime4k => "AI (Anime4K)",
        }
    }

    /// Stufen fürs Menü: (Wert in der Datei, Text).
    pub fn modes(self) -> [(&'static str, &'static str); 3] {
        [("off", "Off"), ("sharp", "Sharp"), ("ai", self.label())]
    }
}

/// Gespeicherte Stufe (`~/.config/<app>/upscaling`), Standard „off“.
pub fn saved() -> &'static str {
    let s = std::fs::read_to_string(crate::app::config_file("upscaling")).unwrap_or_default();
    ["off", "sharp", "ai"].into_iter().find(|m| *m == s.trim()).unwrap_or("off")
}

#[cfg(feature = "gui")]
/// Stufe anwenden (geht auch während das Video läuft).
pub fn apply(mpv: &Mpv, mode: &str, shaders: Shaders) {
    let _ = mpv.set_property("scale", if mode == "off" { "lanczos" } else { "ewa_lanczossharp" });
    let _ = mpv.command("change-list", &["glsl-shaders", "clr", ""]);
    if mode != "ai" {
        return;
    }
    // mpv liest Shader nur aus Dateien: einmal in den Cache schreiben (bzw. nach einem Update neu).
    let dir = crate::app::cache_dir().join("shaders");
    let _ = std::fs::create_dir_all(&dir);
    for (name, text) in shaders.files() {
        let path = dir.join(name);
        if std::fs::metadata(&path).map(|m| m.len()).ok() != Some(text.len() as u64) {
            let _ = std::fs::write(&path, text);
        }
        let _ = mpv.command("change-list", &["glsl-shaders", "append", &path.to_string_lossy()]);
    }
}

#[cfg(feature = "gui")]
/// Stufe wählen: anwenden und merken.
pub fn set(mpv: &Mpv, mode: &str, shaders: Shaders) {
    apply(mpv, mode, shaders);
    let f = crate::app::config_file("upscaling");
    let _ = std::fs::create_dir_all(f.parent().unwrap());
    let _ = std::fs::write(f, mode);
}
