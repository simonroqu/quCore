//! Regeln für den Fortschritt (`[progress]` in qu.toml): ab wann gespeichert wird, ab wann etwas als fertig
//! gilt und wie weit man beim Fortsetzen zurückspringt. Speichern selbst macht die App (eigene Felder).

fn cfg() -> &'static crate::config::Progress {
    &crate::config().progress
}

/// Kürzer geschaut zählt als „nur reingeklickt“ und wird nicht gespeichert.
pub fn worth_saving(pos: f64, dur: f64) -> bool {
    dur > 0.0 && pos >= cfg().min_watched
}

/// Fertig geschaut (Abspann/Ending läuft oft noch): startet beim nächsten Mal von vorne.
pub fn finished(pos: f64, dur: f64) -> bool {
    dur > 0.0 && pos >= dur * cfg().finished_at
}

/// Anteil für den Fortschrittsbalken (0.0 – 1.0; fertig = voll).
pub fn fraction(pos: f64, dur: f64) -> f64 {
    if finished(pos, dur) { 1.0 } else if dur > 0.0 { (pos / dur).clamp(0.0, 1.0) } else { 0.0 }
}

/// Fortsetzen ab: ein paar Sekunden zurück, um wieder reinzukommen; fertig = von vorne (`None`).
pub fn resume_from(pos: f64, dur: f64) -> Option<f64> {
    (!finished(pos, dur)).then(|| (pos - cfg().rewind).max(0.0))
}
