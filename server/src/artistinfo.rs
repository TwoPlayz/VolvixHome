//! Artist pages: a short bio and a photo. MusicBrainz says which Wikipedia article is the artist (matched by exact
//! name, so we never guess), Wikipedia supplies the first paragraph and a picture, and the server keeps both so the
//! lookup happens once a month at most. The picture is saved on the server and shown from there: your devices never
//! contact Wikipedia. Turn it off with VH_ARTIST_INFO=0.

use std::time::Duration;

use serde_json::Value;

use crate::musicbrainz::Client as Mb;

pub struct Info {
    pub bio: String,
    pub image: Option<Vec<u8>>,
}

pub struct Client {
    http: reqwest::Client,
    wiki: String,
    wikidata: String,
    pub enabled: bool,
}

fn base(var: &str, default: &str) -> String {
    std::env::var(var).ok().map(|u| u.trim().trim_end_matches('/').to_string()).filter(|u| !u.is_empty()).unwrap_or_else(|| default.to_string())
}

/// First paragraph, at most ~700 characters, cut at the end of a sentence.
pub fn short_bio(text: &str) -> String {
    let t = text.trim();
    if t.chars().count() <= 700 { return t.to_string(); }
    let cut: String = t.chars().take(700).collect();
    match cut.rfind(". ") {
        Some(i) if i > 200 => cut[..=i].to_string(),
        _ => format!("{}…", cut.trim_end()),
    }
}

impl Client {
    pub fn new() -> Client {
        Client {
            http: reqwest::Client::builder().timeout(Duration::from_secs(12))
                .user_agent(concat!("VolvixHome/", env!("CARGO_PKG_VERSION"), " (https://github.com/TwoPlayz/VolvixHome)")).build().expect("http client"),
            wiki: base("VH_WIKIPEDIA_URL", "https://en.wikipedia.org"),
            wikidata: base("VH_WIKIDATA_URL", "https://www.wikidata.org"),
            enabled: !matches!(std::env::var("VH_ARTIST_INFO").ok().as_deref().map(str::trim), Some("0") | Some("false") | Some("off")),
        }
    }

    async fn json(&self, url: &str) -> Option<Value> {
        let r = self.http.get(url).header("Accept", "application/json").send().await.ok()?;
        if !r.status().is_success() { return None; }
        r.json().await.ok()
    }

    /// An album picture (about 250 px) from the Cover Art Archive, if it has one.
    pub async fn fetch_cover(&self, rg_id: &str) -> Option<Vec<u8>> {
        let base = base("VH_COVERART_URL", crate::musicbrainz::COVER_ART);
        let r = self.http.get(format!("{base}/release-group/{rg_id}/front-250")).send().await.ok()?;
        if !r.status().is_success() { return None; }
        r.bytes().await.ok().filter(|b| b.len() > 100 && b.len() < 2_000_000).map(|b| b.to_vec())
    }

    /// Ok(None) = nothing found. Err = couldn't ask (don't remember that as "none").
    pub async fn find(&self, mb: &Mb, name: &str) -> Result<Option<Info>, String> {
        if !self.enabled { return Ok(None); }
        let Some(links) = mb.artist_links(name).await? else { return Ok(None) };
        let mut title = links.wikipedia;
        if title.is_none() {
            if let Some(q) = links.wikidata {
                let v = self.json(&format!("{}/wiki/Special:EntityData/{q}.json", self.wikidata)).await;
                title = v.and_then(|v| v["entities"][&q]["sitelinks"]["enwiki"]["title"].as_str().map(|t| t.replace(' ', "_")));
            }
        }
        let Some(title) = title else { return Ok(None) };
        let enc: String = title.bytes().map(|b| if b.is_ascii_alphanumeric() || b"_-.~()".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
        let Some(sum) = self.json(&format!("{}/api/rest_v1/page/summary/{enc}", self.wiki)).await else { return Ok(None) };
        if sum["type"].as_str() == Some("disambiguation") { return Ok(None); }
        let bio = short_bio(sum["extract"].as_str().unwrap_or(""));
        if bio.is_empty() { return Ok(None); }
        let image = match sum["thumbnail"]["source"].as_str() {
            Some(u) => match self.http.get(u).send().await {
                Ok(r) if r.status().is_success() => r.bytes().await.ok().filter(|b| b.len() < 1_500_000 && b.len() > 100).map(|b| b.to_vec()),
                _ => None,
            },
            None => None,
        };
        Ok(Some(Info { bio, image }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bios_are_trimmed_at_a_sentence() {
        assert_eq!(short_bio("  Short one.  "), "Short one.");
        let long = format!("{} Last sentence here that goes on and on {}", "A first sentence that is quite long indeed. ".repeat(20), "x".repeat(300));
        let b = short_bio(&long);
        assert!(b.chars().count() <= 701 && b.ends_with('.'), "{b}");
    }
}
