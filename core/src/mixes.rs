//! "Made for you": mixes built from your own plays and the files' tags (genre, year, artist), as on
//! the website (`web/js/mixes.js`). Nothing leaves the server. A mix stays the same all day (seeded
//! with your id and the date), so it doesn't reshuffle under your finger.
use std::collections::HashMap;

use crate::library::Library;
use crate::model::{Album, Track, UserData};
use crate::userdata::stats;

const DAY: u64 = 86400;

#[derive(Debug, Clone, PartialEq)]
pub struct Mix {
    pub id: String,
    pub title: String,
    pub sub: String,
    pub ids: Vec<String>,
    /// repeat | heart | history | user | plus | search | music
    pub icon: &'static str,
}

/// Small seeded random (mulberry32, as the website uses): same seed, same mix.
pub struct Rng(u32);
impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng(seed)
    }
    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6D2B79F5);
        let a = self.0;
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        (t ^ (t >> 14)) as f64 / 4294967296.0
    }
    fn shuffle<T: Clone>(&mut self, v: &[T]) -> Vec<T> {
        let mut a = v.to_vec();
        for i in (1..a.len()).rev() {
            let j = (self.next() * (i as f64 + 1.0)) as usize;
            a.swap(i, j);
        }
        a
    }
}

/// FNV-1a over UTF-16 units, as the website hashes.
pub fn hash(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for c in s.encode_utf16() {
        h ^= c as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

/// Insertion-ordered tallies (JavaScript object semantics, so ties sort the same way).
#[derive(Default)]
struct Tally {
    keys: Vec<String>,
    map: HashMap<String, f64>,
}
impl Tally {
    fn add(&mut self, k: &str, v: f64) {
        match self.map.get_mut(k) {
            Some(x) => *x += v,
            None => {
                self.keys.push(k.to_string());
                self.map.insert(k.to_string(), v);
            }
        }
    }
    fn set_if_missing(&mut self, k: &str, v: f64) {
        if !self.map.contains_key(k) {
            self.keys.push(k.to_string());
            self.map.insert(k.to_string(), v);
        }
    }
    fn get(&self, k: &str) -> f64 {
        self.map.get(k).copied().unwrap_or(0.0)
    }
    fn top(&self, n: usize) -> Vec<String> {
        let mut k = self.keys.clone();
        k.sort_by(|a, b| self.get(b).partial_cmp(&self.get(a)).unwrap_or(std::cmp::Ordering::Equal));
        k.truncate(n);
        k
    }
    /// 0..1 relative to the biggest
    fn norm(&self, k: &str) -> f64 {
        let m = self.map.values().cloned().fold(0.0001f64, f64::max);
        self.get(k) / m
    }
}

fn decade(a: Option<&Album>) -> u32 {
    a.and_then(|a| a.year).map(|y| y / 10 * 10).unwrap_or(0)
}

/// What you like: weights per artist / genre / decade, recent plays counting more (30-day half-life).
struct Profile {
    artist: Tally,
    genre: Tally,
    decade: Tally,
    plays: HashMap<String, u64>,
    last: HashMap<String, u64>,
    total: usize,
}

fn profile(lib: &Library, data: &UserData, now: u64) -> Profile {
    let s = stats(data);
    let mut p = Profile { artist: Tally::default(), genre: Tally::default(), decade: Tally::default(), plays: HashMap::new(), last: HashMap::new(), total: 0 };
    for (id, at) in &s.log {
        let Some(t) = lib.track(id) else { continue };
        let a = lib.album_of(t);
        let w = 0.5f64.powf(now.saturating_sub(*at) as f64 / (30 * DAY) as f64);
        p.total += 1;
        *p.plays.entry(id.clone()).or_default() += 1;
        let l = p.last.entry(id.clone()).or_default();
        *l = (*l).max(*at);
        p.artist.add(&t.artist.to_lowercase(), w);
        if let Some(g) = a.and_then(|a| a.genre.as_ref()) {
            p.genre.add(g, w);
        }
        if a.and_then(|a| a.year).is_some() {
            p.decade.add(&decade(a).to_string(), w);
        }
    }
    // plays from before the log existed still count (all-time totals)
    for (id, (n, last)) in &s.plays {
        if !p.plays.contains_key(id) {
            p.plays.insert(id.clone(), *n);
            p.last.insert(id.clone(), *last);
        }
    }
    p
}

/// Songs, no more than `per` from one artist, so a mix isn't one band.
fn pick(lib: &Library, ids: &[String], n: usize, per: usize) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out = vec![];
    for id in ids {
        if out.len() >= n {
            break;
        }
        let Some(t) = lib.track(id) else { continue };
        let k = t.artist.to_lowercase();
        let c = seen.entry(k).or_default();
        if *c >= per {
            continue;
        }
        *c += 1;
        out.push(id.clone());
    }
    out
}

fn mk(id: impl Into<String>, title: impl Into<String>, sub: &str, ids: Vec<String>, icon: &'static str) -> Option<Mix> {
    (ids.len() >= 3).then(|| Mix { id: id.into(), title: title.into(), sub: sub.to_string(), ids, icon })
}

/// Today's mixes for this person. `user_id` is their server id; `now` in seconds.
pub fn mixes(lib: &Library, data: &UserData, user_id: &str, now: u64) -> Vec<Mix> {
    let p = profile(lib, data, now);
    let all: Vec<String> = lib.tracks().iter().map(|t| t.id.clone()).collect();
    let mut rng = Rng::new((hash(user_id) as u64 + now / DAY) as u32);
    let info = |id: &str| -> Option<(&Track, Option<&Album>)> { lib.track(id).map(|t| (t, lib.album_of(t))) };
    let score = |id: &str| -> f64 {
        let Some((t, a)) = info(id) else { return 0.0 };
        3.0 * p.artist.norm(&t.artist.to_lowercase()) + 2.0 * a.and_then(|a| a.genre.as_ref()).map(|g| p.genre.norm(g)).unwrap_or(0.0) + a.map(|a| p.decade.norm(&decade(Some(a)).to_string())).unwrap_or(0.0)
    };
    let mut list: Vec<Option<Mix>> = vec![];
    if p.total >= 3 {
        // your last 30 days, most played first
        let mut recent = Tally::default();
        for (id, at) in &stats(data).log {
            if now.saturating_sub(*at) < 30 * DAY && lib.track(id).is_some() {
                recent.add(id, 1.0);
            }
        }
        let rep: Vec<String> = recent.top(40).into_iter().filter(|id| recent.get(id) >= 2.0).collect();
        list.push(mk("repeat", "On repeat", "What you've played most in the last 30 days", rep, "repeat"));
        // the songs you keep coming back to
        let mut everything = Tally::default();
        let mut by_play: Vec<(&String, &u64)> = p.plays.iter().collect();
        by_play.sort_by(|a, b| a.0.cmp(b.0)); // steady order before ranking
        for (id, n) in by_play {
            if lib.track(id).is_some() {
                everything.set_if_missing(id, *n as f64);
            }
        }
        list.push(mk("top", "Your top songs", "All-time favourites on this server", everything.top(50), "heart"));
        // loved once, forgotten since
        let mut forgotten: Vec<String> = p.plays.iter().filter(|(id, n)| **n >= 3 && now.saturating_sub(p.last.get(*id).copied().unwrap_or(0)) > 60 * DAY && lib.track(id).is_some()).map(|(id, _)| id.clone()).collect();
        forgotten.sort();
        let f: Vec<String> = rng.shuffle(&forgotten).into_iter().take(40).collect();
        list.push(mk("rediscover", "Rediscover", "Songs you used to play a lot and haven't for a while", f, "history"));
        // artist mixes: your top artists, plus music in the same genres from other artists
        for ar in p.artist.top(3) {
            let own: Vec<String> = all.iter().filter(|id| info(id).map(|(t, _)| t.artist.to_lowercase() == ar).unwrap_or(false)).cloned().collect();
            let genres: Vec<String> = own.iter().filter_map(|id| info(id).and_then(|(_, a)| a.and_then(|a| a.genre.clone()))).collect();
            let like: Vec<String> = all
                .iter()
                .filter(|id| info(id).map(|(t, a)| t.artist.to_lowercase() != ar && a.and_then(|a| a.genre.as_ref()).map(|g| genres.contains(g)).unwrap_or(false)).unwrap_or(false))
                .cloned()
                .collect();
            let name = own.first().and_then(|id| info(id)).map(|(t, _)| t.artist.clone()).unwrap_or_else(|| ar.clone());
            let mut seed: Vec<String> = rng.shuffle(&own).into_iter().take(14).collect();
            seed.extend(rng.shuffle(&like));
            let ids = pick(lib, &seed, 30, 4);
            list.push(mk(format!("more-{}", radix36(hash(&ar))), format!("{name} & more"), "Mixed with similar music from your library", ids, "user"));
        }
        // new things that fit your taste
        let cut = now.saturating_sub(90 * DAY);
        let mut fresh: Vec<String> = all.iter().filter(|id| info(id).map(|(_, a)| a.map(|a| a.added > cut).unwrap_or(false)).unwrap_or(false) && !p.plays.contains_key(*id)).cloned().collect();
        fresh.sort_by(|a, b| score(b).partial_cmp(&score(a)).unwrap_or(std::cmp::Ordering::Equal));
        list.push(mk("new", "New for you", "Recently added music, in the genres you play", pick(lib, &fresh, 30, 4), "plus"));
        // never played, ranked by how much you'd probably like it (with a little chance mixed in)
        let mut unplayed: Vec<(String, f64)> = all.iter().filter(|id| !p.plays.contains_key(*id)).map(|id| (id.clone(), score(id) + rng.next() * 0.6)).collect();
        unplayed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let ids: Vec<String> = unplayed.into_iter().map(|x| x.0).collect();
        list.push(mk("discover", "Discover", "Songs you haven't played yet that fit your taste", pick(lib, &ids, 40, 3), "search"));
    } else {
        // nothing played yet: a taste of what's here
        let ids = pick(lib, &rng.shuffle(&all), 30, 3);
        list.push(mk("start", "Start here", "A mix of what's on the server. Play a few songs and this gets personal.", ids, "music"));
    }
    list.into_iter().flatten().collect()
}

/// Albums by how much you've played them (Home's "Your most played").
pub fn top_albums<'a>(lib: &'a Library, data: &UserData, n: usize) -> Vec<&'a Album> {
    let mut plays: Vec<(String, u64)> = vec![];
    for (id, (c, _)) in stats(data).plays {
        let Some(t) = lib.track(&id) else { continue };
        match plays.iter_mut().find(|x| x.0 == t.album_id) {
            Some(x) => x.1 += c,
            None => plays.push((t.album_id.clone(), c)),
        }
    }
    plays.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    plays.into_iter().take(n).filter_map(|(id, _)| lib.album(&id)).collect()
}

/// Albums picked at random for today (the same picks all day): "Picked for today".
pub fn random_albums<'a>(lib: &'a Library, user_id: &str, now: u64, n: usize) -> Vec<&'a Album> {
    let mut r = Rng::new((hash(&format!("r{user_id}")) as u64 + now / DAY) as u32);
    let a: Vec<&Album> = lib.albums().iter().collect();
    r.shuffle(&a).into_iter().take(n).collect()
}

fn radix36(mut n: u32) -> String {
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = vec![];
    loop {
        out.push(digits[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::tests::sample;
    use crate::userdata::{apply, Edit};

    #[test]
    fn rng_matches_the_website() {
        // values from the website's rng(1) in a browser console
        let mut r = Rng::new(1);
        let first = r.next();
        assert!((first - 0.6270739405881613).abs() < 1e-12, "{first}");
        assert!((r.next() - 0.002735721180215478).abs() < 1e-12);
        assert_eq!(hash("abc"), 440920331);
        assert_eq!(hash("Ärvore"), 3880208345);
    }

    #[test]
    fn start_here_without_plays() {
        let lib = sample();
        let m = mixes(&lib, &UserData::default(), "u1", 1_780_000_000);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].id, "start");
        // the same all day
        assert_eq!(m, mixes(&lib, &UserData::default(), "u1", 1_780_000_000 + 3600));
    }

    #[test]
    fn personal_after_plays() {
        let lib = sample();
        let mut d = UserData::default();
        let now = 1_780_000_000;
        for (i, t) in ["t1", "t1", "t2", "t1", "t2", "t3", "t3"].iter().enumerate() {
            apply(&mut d, &Edit::Play { track: t.to_string(), at: now - 1000 + i as u64 });
        }
        let m = mixes(&lib, &d, "u1", now);
        let ids: Vec<&str> = m.iter().map(|x| x.id.as_str()).collect();
        assert!(ids.contains(&"repeat"), "{ids:?}");
        assert!(ids.contains(&"top"));
        let top = m.iter().find(|x| x.id == "top").unwrap();
        assert_eq!(top.ids[0], "t1", "most played first");
        assert_eq!(top_albums(&lib, &d, 1)[0].id, "a1");
        assert_eq!(random_albums(&lib, "u1", now, 10).len(), 4);
    }
}
