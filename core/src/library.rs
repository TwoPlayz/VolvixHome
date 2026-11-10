//! The library with the lookups the screens need, built once from `/api/library`.
use std::collections::HashMap;

use crate::model::*;

#[derive(Debug, Clone, Default)]
pub struct Library {
    albums: Vec<Album>,
    tracks: Vec<Track>,
    album_index: HashMap<String, usize>,
    track_index: HashMap<String, usize>,
    /// artist names, A to Z (case-insensitive, "The " ignored for sorting)
    artists: Vec<String>,
    /// album indexes A to Z by title, and newest-added first (worked out once, not on every redraw)
    by_title: Vec<usize>,
    by_added: Vec<usize>,
    /// album indexes per artist name (lower case)
    by_artist: HashMap<String, Vec<usize>>,
    pub scanning: bool,
}

/// One artist's page: their own albums (newest first) and albums by others they're on.
#[derive(Debug, Clone, Default)]
pub struct ArtistPage<'a> {
    pub name: String,
    pub albums: Vec<&'a Album>,
    pub appears_on: Vec<&'a Album>,
    /// their most-present songs, for a "Songs" list (first song of each album, then the rest)
    pub songs: Vec<&'a Track>,
}

fn sort_key(name: &str) -> String {
    let l = name.trim().to_lowercase();
    l.strip_prefix("the ").map(str::to_string).unwrap_or(l)
}

impl Library {
    pub fn new(catalog: Catalog) -> Self {
        let mut lib = Library { albums: catalog.albums, tracks: catalog.tracks, scanning: catalog.scanning, ..Default::default() };
        lib.album_index = lib.albums.iter().enumerate().map(|(i, a)| (a.id.clone(), i)).collect();
        lib.track_index = lib.tracks.iter().enumerate().map(|(i, t)| (t.id.clone(), i)).collect();
        // the website lists artists from the albums' artist names (the catalog's artists list too)
        let mut seen: HashMap<String, String> = HashMap::new();
        for name in lib.albums.iter().map(|a| a.artist.clone()).chain(catalog.artists.into_iter().map(|a| a.name)) {
            if name.trim().is_empty() {
                continue;
            }
            seen.entry(name.to_lowercase()).or_insert(name);
        }
        let mut artists: Vec<String> = seen.into_values().collect();
        artists.sort_by_key(|a| sort_key(a));
        lib.artists = artists;
        let mut by_title: Vec<usize> = (0..lib.albums.len()).collect();
        by_title.sort_by_cached_key(|&i| sort_key(&lib.albums[i].title));
        let mut by_added: Vec<usize> = (0..lib.albums.len()).collect();
        by_added.sort_by(|&a, &b| lib.albums[b].added.cmp(&lib.albums[a].added).then_with(|| lib.albums[a].title.cmp(&lib.albums[b].title)));
        let mut by_artist: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, a) in lib.albums.iter().enumerate() {
            by_artist.entry(a.artist.to_lowercase()).or_default().push(i);
        }
        for list in by_artist.values_mut() {
            list.sort_by(|&a, &b| lib.albums[b].year.unwrap_or(0).cmp(&lib.albums[a].year.unwrap_or(0)).then_with(|| lib.albums[a].title.cmp(&lib.albums[b].title)));
        }
        lib.by_title = by_title;
        lib.by_added = by_added;
        lib.by_artist = by_artist;
        lib
    }

    pub fn is_empty(&self) -> bool {
        self.albums.is_empty()
    }
    pub fn albums(&self) -> &[Album] {
        &self.albums
    }
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }
    pub fn artists(&self) -> &[String] {
        &self.artists
    }
    pub fn album(&self, id: &str) -> Option<&Album> {
        self.album_index.get(id).map(|&i| &self.albums[i])
    }
    pub fn track(&self, id: &str) -> Option<&Track> {
        self.track_index.get(id).map(|&i| &self.tracks[i])
    }
    pub fn album_of(&self, track: &Track) -> Option<&Album> {
        self.album(&track.album_id)
    }

    /// The album's songs in disc, then track-number order.
    pub fn album_tracks(&self, album_id: &str) -> Vec<&Track> {
        let Some(a) = self.album(album_id) else { return vec![] };
        let mut list: Vec<&Track> = a.track_ids.iter().filter_map(|id| self.track(id)).collect();
        list.sort_by_key(|t| (t.disc.max(1), if t.no == 0 { u32::MAX } else { t.no }));
        list
    }

    /// The album's song ids in play order.
    pub fn album_track_ids(&self, album_id: &str) -> Vec<String> {
        self.album_tracks(album_id).into_iter().map(|t| t.id.clone()).collect()
    }

    /// How long the album is, in seconds.
    pub fn album_duration(&self, album_id: &str) -> u32 {
        self.album_tracks(album_id).iter().map(|t| t.duration).sum()
    }

    /// Does the album have more than one disc?
    pub fn album_discs(&self, album_id: &str) -> u32 {
        self.album_tracks(album_id).iter().map(|t| t.disc.max(1)).max().unwrap_or(1)
    }

    /// Albums newest-added first ("Recently added").
    pub fn recently_added(&self, n: usize) -> Vec<&Album> {
        self.by_added.iter().take(n).map(|&i| &self.albums[i]).collect()
    }

    /// Albums A to Z by title.
    pub fn albums_by_title(&self) -> Vec<&Album> {
        self.by_title.iter().map(|&i| &self.albums[i]).collect()
    }

    /// Artists with the most albums first, then alphabetical (Home's "Artists" shelf).
    pub fn top_artists(&self, n: usize) -> Vec<String> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for a in &self.albums {
            *counts.entry(a.artist.as_str()).or_default() += 1;
        }
        let mut list: Vec<(&str, usize)> = counts.into_iter().collect();
        list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        list.into_iter().take(n).map(|(n, _)| n.to_string()).collect()
    }

    /// Albums whose album artist is `name` (case-insensitive), newest first.
    pub fn albums_by(&self, name: &str) -> Vec<&Album> {
        self.by_artist.get(&name.to_lowercase()).map(|l| l.iter().map(|&i| &self.albums[i]).collect()).unwrap_or_default()
    }

    /// Is there an artist by this name in the library (case-insensitive)?
    pub fn has_artist(&self, name: &str) -> bool {
        let n = name.trim().to_lowercase();
        self.by_artist.contains_key(&n) || self.artists.iter().any(|a| a.to_lowercase() == n)
    }

    /// Albums by others with at least one song by `name` on them (features, compilations).
    pub fn appears_on(&self, name: &str) -> Vec<&Album> {
        let n = name.to_lowercase();
        self.albums
            .iter()
            .filter(|a| a.artist.to_lowercase() != n && a.track_ids.iter().filter_map(|id| self.track(id)).any(|t| t.artist.to_lowercase() == n))
            .collect()
    }

    /// The album whose cover stands for an artist (their newest, or one they appear on).
    pub fn artist_cover_album(&self, name: &str) -> Option<&Album> {
        self.albums_by(name).into_iter().next().or_else(|| self.appears_on(name).into_iter().next())
    }

    pub fn artist_page(&self, name: &str) -> ArtistPage<'_> {
        let albums = self.albums_by(name);
        let appears_on = self.appears_on(name);
        let n = name.to_lowercase();
        let mut songs: Vec<&Track> = vec![];
        for a in albums.iter().chain(appears_on.iter()) {
            for t in self.album_tracks(&a.id) {
                if t.artist.to_lowercase().contains(&n) || a.artist.to_lowercase() == n {
                    songs.push(t);
                }
            }
        }
        let shown = albums.first().map(|a| a.artist.clone()).unwrap_or_else(|| name.to_string());
        ArtistPage { name: shown, albums, appears_on, songs }
    }

    /// Recently played albums from the person's data, skipping ones no longer on the server.
    pub fn recent_albums<'a>(&'a self, recent_ids: &[String], n: usize) -> Vec<&'a Album> {
        recent_ids.iter().filter_map(|id| self.album(id)).take(n).collect()
    }

    /// A short line under an album title: "2019 · 11 songs · 43 min"
    pub fn album_summary(&self, album_id: &str) -> String {
        let Some(a) = self.album(album_id) else { return String::new() };
        let songs = a.track_ids.len();
        let mins = (self.album_duration(album_id) + 30) / 60;
        let mut parts = vec![];
        if let Some(y) = a.year {
            parts.push(y.to_string());
        }
        parts.push(format!("{songs} song{}", if songs == 1 { "" } else { "s" }));
        parts.push(if mins >= 60 { format!("{} hr {} min", mins / 60, mins % 60) } else { format!("{mins} min") });
        parts.join(" · ")
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn sample() -> Library {
        let t = |id: &str, title: &str, artist: &str, album: &str, disc: u32, no: u32| Track {
            id: id.into(),
            title: title.into(),
            artist: artist.into(),
            album_id: album.into(),
            disc,
            no,
            duration: 200,
            ..Default::default()
        };
        let a = |id: &str, title: &str, artist: &str, year: u32, added: u64, ids: &[&str]| Album {
            id: id.into(),
            title: title.into(),
            artist: artist.into(),
            year: Some(year),
            added,
            track_ids: ids.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };
        Library::new(Catalog {
            albums: vec![
                a("a1", "Blue Hours", "The Lamps", 2019, 100, &["t3", "t1", "t2"]),
                a("a2", "Árvore", "Mira Sol", 2021, 300, &["t4", "t5"]),
                a("a3", "Night Drive", "The Lamps", 2022, 200, &["t6"]),
                a("a4", "Friends Vol. 1", "Various Artists", 2020, 50, &["t7"]),
            ],
            tracks: vec![
                t("t1", "Morning", "The Lamps", "a1", 1, 1),
                t("t2", "Noon", "The Lamps", "a1", 1, 2),
                t("t3", "Evening", "The Lamps", "a1", 2, 1),
                t("t4", "Café", "Mira Sol", "a2", 1, 1),
                t("t5", "Rio", "Mira Sol", "a2", 1, 2),
                t("t6", "Highway", "The Lamps", "a3", 1, 1),
                t("t7", "Guest Spot", "Mira Sol", "a4", 1, 1),
            ],
            ..Default::default()
        })
    }

    #[test]
    fn album_order_is_disc_then_number() {
        let lib = sample();
        assert_eq!(lib.album_track_ids("a1"), vec!["t1", "t2", "t3"]);
        assert_eq!(lib.album_discs("a1"), 2);
        assert_eq!(lib.album_duration("a1"), 600);
        assert_eq!(lib.album_summary("a1"), "2019 · 3 songs · 10 min");
    }

    #[test]
    fn artists_sorted_ignoring_the() {
        let lib = sample();
        assert_eq!(lib.artists(), &["The Lamps", "Mira Sol", "Various Artists"]);
    }

    #[test]
    fn artist_pages() {
        let lib = sample();
        let p = lib.artist_page("the lamps");
        assert_eq!(p.name, "The Lamps");
        assert_eq!(p.albums.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["a3", "a1"]);
        let m = lib.artist_page("Mira Sol");
        assert_eq!(m.appears_on.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["a4"]);
        assert_eq!(m.songs.len(), 3);
        assert_eq!(lib.artist_cover_album("Mira Sol").unwrap().id, "a2");
    }

    #[test]
    fn shelves() {
        let lib = sample();
        assert_eq!(lib.recently_added(2).iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["a2", "a3"]);
        assert_eq!(lib.top_artists(1), vec!["The Lamps"]);
        assert_eq!(lib.recent_albums(&["gone".into(), "a3".into()], 5).len(), 1);
    }
}
