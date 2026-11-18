//! Kleine lokale Dateien in `~/.local/share/<app>/`: lesen, sicher schreiben, Schlüssel-Wert-Tabellen
//! (`key<TAB>wert`, eine Zeile pro Eintrag) und das zuletzt gewählte Tempo pro Titel/Kanal (`speeds.tsv`).

use crate::app::data_file;

pub fn read(name: &str) -> String {
    std::fs::read_to_string(data_file(name)).unwrap_or_default()
}

/// Erst in eine Hilfsdatei schreiben, dann umbenennen: ein Absturz mittendrin zerstört nichts.
pub fn write(name: &str, text: &str) {
    let path = data_file(name);
    let tmp = path.with_extension("tmp");
    let ok = std::fs::create_dir_all(path.parent().unwrap()).and_then(|_| std::fs::write(&tmp, text)).and_then(|_| std::fs::rename(&tmp, &path));
    if let Err(e) = ok {
        eprintln!("{name} nicht gespeichert: {e}");
    }
}

pub fn parse_table(text: &str) -> Vec<(String, String)> {
    text.lines().filter_map(|l| l.split_once('\t')).map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// Wert zu `key` in der Tabelle `file`.
pub fn lookup(file: &str, key: &str) -> Option<String> {
    parse_table(&read(file)).into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Eintrag setzen; `None` (= Standardwert) entfernt ihn.
pub fn put(file: &str, key: &str, value: Option<String>) {
    let mut list = parse_table(&read(file));
    list.retain(|(k, _)| k != key);
    if let Some(v) = value {
        list.push((key.replace(['\t', '\n'], " "), v.replace(['\t', '\n'], " ")));
    }
    write(file, &list.iter().map(|(k, v)| format!("{k}\t{v}")).collect::<Vec<_>>().join("\n"));
}

/// Zuletzt für diesen Titel/Kanal gewählte Geschwindigkeit (sonst normal).
pub fn speed(key: &str) -> f64 {
    lookup("speeds.tsv", key).and_then(|s| s.parse().ok()).unwrap_or(1.0)
}

/// Normal ist Standard und braucht keinen Eintrag.
pub fn set_speed(key: &str, speed: f64) {
    put("speeds.tsv", key, (speed != 1.0).then(|| speed.to_string()));
}

/// So viele letzte Suchen merken (`~/.local/share/<app>/searches.txt`).
const SEARCHES_MAX: usize = 8;

/// Letzte Suchen, neueste zuerst.
pub fn searches() -> Vec<String> {
    std::fs::read_to_string(data_file("searches.txt")).unwrap_or_default().lines().map(String::from).collect()
}

pub fn add_search(q: &str) {
    let mut list = searches();
    list.retain(|s| s != q);
    list.insert(0, q.replace('\n', " "));
    list.truncate(SEARCHES_MAX);
    let path = data_file("searches.txt");
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let _ = std::fs::write(path, list.join("\n"));
}

#[cfg(test)]
mod tests {
    #[test]
    fn table_parse() {
        let l = super::parse_table("a\t1.5\nkaputt\nb\tdub");
        assert_eq!(l, vec![("a".to_string(), "1.5".to_string()), ("b".to_string(), "dub".to_string())]);
    }
}
