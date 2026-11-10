//! In-library search, matching the website (`web/js/views/search.js`): every typed word must
//! appear somewhere (accents and case ignored), and results that start with what was typed come first.
use unicode_normalization::UnicodeNormalization;

use crate::library::Library;
use crate::model::{Album, Playlist, Track};

/// lower case, accents removed: "Árvore" -> "arvore"
pub fn norm(s: &str) -> String {
    s.nfd().filter(|c| !('\u{300}'..='\u{36f}').contains(c)).flat_map(char::to_lowercase).collect()
}

#[derive(Debug, Default)]
pub struct Results<'a> {
    pub artists: Vec<String>,
    pub albums: Vec<&'a Album>,
    pub songs: Vec<&'a Track>,
    pub playlists: Vec<&'a Playlist>,
}

impl Results<'_> {
    pub fn is_empty(&self) -> bool {
        self.artists.is_empty() && self.albums.is_empty() && self.songs.is_empty() && self.playlists.is_empty()
    }
}

pub fn search<'a>(lib: &'a Library, playlists: &'a [Playlist], query: &str) -> Results<'a> {
    let n = norm(query.trim());
    let words: Vec<&str> = n.split_whitespace().collect();
    if words.is_empty() {
        return Results::default();
    }
    let hit = |text: &str| {
        let t = norm(text);
        words.iter().all(|w| t.contains(w))
    };
    let starts = |text: &str| if norm(text).starts_with(&n) { 0 } else { 1 };

    let mut albums: Vec<&Album> = lib
        .albums()
        .iter()
        .filter(|a| hit(&format!("{} {} {} {}", a.title, a.artist, a.year.map(|y| y.to_string()).unwrap_or_default(), a.genre.clone().unwrap_or_default())))
        .collect();
    let mut artists: Vec<String> = lib.artists().iter().filter(|a| hit(a)).cloned().collect();
    let mut songs: Vec<&Track> = lib.tracks().iter().filter(|t| hit(&format!("{} {}", t.title, t.artist))).collect();
    let playlists: Vec<&Playlist> = playlists.iter().filter(|p| hit(&format!("{} {}", p.name, p.description))).collect();
    // stable sorts: keep library order within each group
    albums.sort_by_key(|a| starts(&a.title));
    artists.sort_by_key(|a| starts(a));
    songs.sort_by_key(|t| starts(&t.title));
    Results { artists, albums, songs, playlists }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::tests::sample;

    #[test]
    fn accents_and_case() {
        assert_eq!(norm("Árvore Café"), "arvore cafe");
        let lib = sample();
        let r = search(&lib, &[], "ARVORE");
        assert_eq!(r.albums.len(), 1);
        let r = search(&lib, &[], "cafe");
        assert_eq!(r.songs[0].id, "t4");
    }

    #[test]
    fn every_word_must_match() {
        let lib = sample();
        let r = search(&lib, &[], "lamps 2022");
        assert_eq!(r.albums.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["a3"]);
        assert!(search(&lib, &[], "lamps zebra").is_empty());
        assert!(search(&lib, &[], "   ").is_empty());
    }

    #[test]
    fn starts_with_first() {
        let lib = sample();
        // "Night Drive" starts with "n", "Noon" too; "Morning"/"Evening"/"Highway" contain n only inside
        let r = search(&lib, &[], "n");
        assert_eq!(r.songs[0].id, "t2");
    }

    #[test]
    fn playlists_found() {
        let lib = sample();
        let pls = vec![Playlist { id: "p1".into(), name: "Road trip".into(), ..Default::default() }];
        assert_eq!(search(&lib, &pls, "road").playlists.len(), 1);
    }
}
