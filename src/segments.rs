//! Abschnitte im Video (Opening/Ending von AniSkip, Sponsor-Teile von SponsorBlock …): als Bereiche über der
//! Timeline gezeichnet; je nach App mit „Skip …“-Knopf oder automatisch übersprungen.

#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    /// Anzeigename („intro“, „sponsor“ …) für Knopf bzw. Einblendung.
    pub label: &'static str,
}

/// Liegt `pos` in einem Abschnitt? Dann dessen Index. Kurz vor dem Ende nicht mehr (Sprung um Bruchteile
/// einer Sekunde lohnt nicht).
pub fn segment_at(segments: &[Segment], pos: f64) -> Option<usize> {
    segments.iter().position(|s| pos >= s.start && pos < s.end - 0.5)
}

/// Wie `segment_at`, aber nur Abschnitte, die noch nicht übersprungen wurden (automatisches Überspringen).
pub fn to_skip(segments: &[Segment], skipped: &[usize], pos: f64) -> Option<usize> {
    segments.iter().enumerate().position(|(i, s)| !skipped.contains(&i) && pos >= s.start && pos < s.end - 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_segments() {
        let s = vec![Segment { start: 10.0, end: 20.0, label: "intro" }, Segment { start: 50.0, end: 60.0, label: "ending" }];
        assert_eq!(segment_at(&s, 15.0), Some(0));
        assert_eq!(segment_at(&s, 19.8), None, "kurz vor dem Ende nicht mehr");
        assert_eq!(segment_at(&s, 30.0), None);
        assert_eq!(to_skip(&s, &[0], 15.0), None);
        assert_eq!(to_skip(&s, &[0], 55.0), Some(1));
    }
}
