//! Vorschaubilder der Timeline (z. B. YouTube-„Storyboard“) als Daten; gezeichnet wird in `controls` (GUI).

/// Vorschaubilder für die Timeline (z. B. YouTube-„Storyboard“): Bögen mit `cols`×`rows` kleinen Bildern.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Storyboard {
    pub width: i32,
    pub height: i32,
    pub cols: u32,
    pub rows: u32,
    /// Bilder pro Sekunde Video (z. B. 0,5 = alle 2 s ein Bild).
    pub fps: f64,
    /// Ein Bild-URL pro Bogen.
    pub sheets: Vec<String>,
}

impl Storyboard {
    /// Bogen und Ausschnitt (x, y) des Bilds zur Zeit `t`.
    pub fn locate(&self, t: f64) -> (usize, i32, i32) {
        let per_sheet = (self.cols * self.rows).max(1) as usize;
        let last = (self.sheets.len() * per_sheet).saturating_sub(1);
        let i = ((t.max(0.0) * self.fps) as usize).min(last);
        let within = (i % per_sheet) as u32;
        (i / per_sheet, (within % self.cols) as i32 * self.width, (within / self.cols) as i32 * self.height)
    }
}
