//! Playlists, mixes, lyrics, Get more music, household, notifications, downloads, settings and the
//! dialogs.
use iced::widget::{button, checkbox, column, container, horizontal_space, image, row, scrollable, text, text_input, toggler, Column, Row};
use iced::{Alignment, Color, Length};
use volvix_core::mixes::Mix;
use volvix_core::playlists::Kind;
use volvix_core::{FoundItem, Track};

use crate::ui::{self, icon, icon as ic};
use crate::views::{card, clip, cover_of, download_button, header_buttons, playlist_art, playlist_card, playlist_cover, round_icon, section, track_list, El, Numbers, Rows};
use crate::{lyrics_scroller, App, Dialog, Message, Page};

/* ---------- playlists and mixes ---------- */

pub fn playlist_page<'a>(app: &'a App, id: &str) -> El<'a> {
    let Some(p) = app.playlist(id) else { return ui::dim("This playlist isn't here any more.", 15).into() };
    let tracks: Vec<&Track> = p.tracks.iter().filter_map(|t| app.lib.track(t)).collect();
    let total: u32 = tracks.iter().map(|t| t.duration).sum();
    let label = match &p.kind {
        Kind::Liked => "Playlist".to_string(),
        Kind::Own => if p.public { "Playlist · visible to the household" } else { "Playlist" }.to_string(),
        Kind::Shared { mine } => format!("Collaborative playlist{}", if *mine { " · yours" } else { "" }),
        Kind::Household { owner } => format!("{owner}'s playlist"),
    };
    let mut info = column![ui::dim(label, 13), ui::title(p.name.clone(), 40)].spacing(6);
    if !p.description.is_empty() {
        info = info.push(ui::dim(p.description.clone(), 14));
    }
    let mins = (total + 30) / 60;
    info = info.push(ui::dim(format!("{} song{} · {} min", tracks.len(), if tracks.len() == 1 { "" } else { "s" }, mins), 14));
    let ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
    let mut extra = vec![download_button(app, "playlist", id, &ids)];
    // like the website: who can edit, download, and one ⋯ button that has the rest (edit, picture, queue, share, delete)
    if p.can_manage() {
        extra.insert(0, round_icon(icon::PEOPLE, p.is_shared(), "Who can edit", Some(Message::CollabOpen(p.id.clone()))));
        extra.push(round_icon(icon::MORE, false, "More", Some(Message::PopMenuOpen(crate::MenuKind::Playlist(p.id.clone())))));
    }
    if let Kind::Shared { mine: false } = p.kind {
        extra.push(round_icon(icon::CLOSE, false, "Leave this playlist", Some(Message::SharedOp(p.id.clone(), serde_json::json!({ "op": "leave" })))));
    }
    info = info.push(ui::gap(10)).push(header_buttons(Message::PlayTracks(ids.clone(), 0, p.name.clone()), Message::ShuffleTracks(ids, p.name.clone()), extra));
    let head = row![playlist_art(app, &p, 220.0), info].spacing(28).align_y(Alignment::End);
    let body: El = if tracks.is_empty() {
        ui::dim(if p.kind == Kind::Liked { "Songs you like (the heart) show up here." } else { "No songs yet. Use the playlist button on any song to add it here." }, 15).into()
    } else {
        track_list(app, &tracks, &p.name, Numbers::Place, true, if p.can_edit() { Rows::Playlist { id, liked: p.kind == Kind::Liked } } else { Rows::Plain })
    };
    let mut page = column![head, body].spacing(28);
    if p.can_edit() {
        page = page.push(add_songs(app, &p));
    }
    page.into()
}

/// The playlist page's "Add songs": a search box, and songs to add with one click (suggestions while the box is empty).
fn add_songs<'a>(app: &'a App, p: &volvix_core::playlists::PlaylistView) -> El<'a> {
    let q = app.add_q.trim().to_string();
    let found: Vec<&Track> = if q.is_empty() {
        app.lib.tracks().iter().filter(|t| !p.tracks.contains(&t.id)).take(6).collect()
    } else {
        volvix_core::search::search(&app.lib, &app.data.playlists, &q).songs.into_iter().filter(|t| !p.tracks.contains(&t.id)).take(8).collect()
    };
    let mut list = Column::new().spacing(2);
    if q.is_empty() {
        list = list.push(ui::dim("Recommended from your library", 13));
    } else if found.is_empty() {
        list = list.push(ui::dim(format!("Nothing found for {q}"), 13));
    }
    for t in found {
        let album = app.lib.album_of(t);
        list = list.push(
            container(
                row![
                    cover_of(app, album, 40.0),
                    column![text(clip(&t.title, 50)).size(15), ui::dim(clip(&t.artist, 50), 12)].spacing(1).width(Length::Fill),
                    ui::dim(clip(album.map(|a| a.title.as_str()).unwrap_or(""), 30), 13),
                    button(text("Add").size(13)).padding([6, 16]).style(ui::secondary).on_press(Message::AddSongTo(p.id.clone(), t.id.clone())),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding([4, 8]),
        );
    }
    column![
        ui::gap(8),
        ui::title(if p.tracks.is_empty() { "Let's find something for your playlist" } else { "Add songs" }, 22),
        text_input("Search for songs to add", &app.add_q).on_input(Message::AddSongQuery).padding(12).size(15).style(ui::input).width(Length::Fill),
        list,
    ]
    .spacing(12)
    .into()
}

/// A "Made for you" card: red fading to the mix's own colour, its icon and its name in the middle (as the website draws it).
pub fn mix_card(m: &Mix) -> El<'_> {
    let art: El = container(
        column![
            ui::wi(m.icon, 30.0, Color::WHITE),
            text(m.title.clone()).size(16).font(ui::FONT_BOLD).color(Color::WHITE).align_x(iced::alignment::Horizontal::Center).width(Length::Fill).wrapping(text::Wrapping::Word),
        ]
        .spacing(10)
        .align_x(Alignment::Center),
    )
    .width(164)
    .height(164)
    .padding(12)
    .center_x(164)
    .center_y(164)
    .style(ui::mix_tile(&m.id))
    .into();
    card(art, m.title.clone(), m.sub.clone(), Message::Go(Page::Mix(m.id.clone())))
}

pub fn mix_page<'a>(app: &'a App, id: &str) -> El<'a> {
    let Some(m) = app.mixes.iter().find(|m| m.id == id) else { return ui::dim("This mix isn't here today. Mixes change every day.", 15).into() };
    let tracks: Vec<&Track> = m.ids.iter().filter_map(|t| app.lib.track(t)).collect();
    let info = column![
        ui::dim("Made for you", 13),
        ui::title(m.title.clone(), 40),
        ui::dim(m.sub.clone(), 14),
        ui::dim(format!("{} songs · the same all day", tracks.len()), 13),
        ui::gap(10),
        header_buttons(Message::PlayTracks(m.ids.clone(), 0, m.title.clone()), Message::ShuffleTracks(m.ids.clone(), m.title.clone()), vec![round_icon(icon::PLAYLIST, false, "Save as a playlist", Some(Message::NewPlaylist(m.ids.clone())))]),
    ]
    .spacing(6);
    let art = container(ui::wi(m.icon, 64.0, Color::WHITE)).width(220).height(220).center_x(220).center_y(220).style(ui::mix_tile(&m.id));
    column![row![art, info].spacing(28).align_y(Alignment::End), track_list(app, &tracks, &m.title, Numbers::Place, true, Rows::Plain)].spacing(28).into()
}

/// Library → Downloaded: what you chose to keep on this computer.
pub fn downloaded(app: &App) -> El<'_> {
    if app.saved.is_empty() {
        return ui::dim("Nothing downloaded yet. Use the download button on an album or playlist to listen without the server.", 15).into();
    }
    let mut cards: Vec<El> = vec![];
    for s in &app.saved {
        if s.kind == "album" {
            if let Some(a) = app.lib.album(&s.id) {
                cards.push(card(cover_of(app, Some(a), 164.0), a.title.clone(), a.artist.clone(), Message::Go(Page::Album(a.id.clone()))));
            }
        } else if let Some(p) = app.playlist(&s.id) {
            cards.push(playlist_card(app, &p));
        }
    }
    let pending = app.dl_queue_len();
    let status = if pending > 0 || app.dl_active.is_some() {
        format!("Downloading… {} song{} to go", pending + app.dl_active.is_some() as usize, if pending + app.dl_active.is_some() as usize == 1 { "" } else { "s" })
    } else {
        format!("{} songs on this computer ({})", app.downloaded.len(), human_size(app.downloads.size()))
    };
    column![ui::dim(status, 14), Row::with_children(cards).spacing(4).wrap()].spacing(14).into()
}

pub fn human_size(b: u64) -> String {
    let mb = b as f64 / 1_048_576.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else if mb >= 1.0 {
        format!("{mb:.0} MB")
    } else {
        format!("{:.0} KB", b as f64 / 1024.0)
    }
}

/* ---------- lyrics ---------- */

pub fn lyrics<'a>(app: &'a App, track: &str) -> El<'a> {
    let note = |title: &'a str, sub: String| -> El<'a> { column![ui::title(title, 20), ui::dim(sub, 14)].spacing(6).into() };
    let Some(r) = app.lyrics.get(track) else { return note("Looking for lyrics…", "Checking the song's files, then the lyrics service.".into()) };
    let ans = match r {
        Err(e) => return note("Couldn't load lyrics", e.clone()),
        Ok(a) => a,
    };
    let Some(ly) = &ans.lyrics else {
        return note(
            "No lyrics found",
            if ans.online { "Nothing for this song in its files or on LRCLIB. Put a .lrc file with the same name next to the song to add your own." } else { "Online lyrics are off on this server. Put a .lrc file with the same name next to the song, or embed lyrics in its tags." }.into(),
        );
    };
    if ly.instrumental {
        return note("Instrumental", "This song has no lyrics.".into());
    }
    if ly.synced.is_empty() {
        return scrollable(text(ly.plain.clone()).size(20).color(ui::TEXT_2()).line_height(1.5)).style(ui::scroller).height(Length::Fill).into();
    }
    let now = ly.line_at(app.status.position.as_millis() as i64);
    let mut col = Column::new();
    for (i, l) in ly.synced.iter().enumerate() {
        let color = match now {
            Some(n) if n == i => ui::TEXT(),
            Some(n) if i < n => ui::TEXT_3(),
            _ => ui::TEXT_2(),
        };
        let size = if now == Some(i) { 26.0 } else { 22.0 };
        // an instrumental gap: a music note from Windows' icon font (the text font has no ♪)
        let line: El = if l.text.trim().is_empty() {
            ic('\u{E8D6}', size as u16).color(color).into()
        } else {
            text(l.text.clone()).size(size).font(ui::FONT_BOLD).color(color).into()
        };
        // long lines wrap (the scroll position estimate allows for one line each, close enough)
        col = col.push(
            button(line)
                .width(Length::Fill)
                .padding([6, 0])
                .style(ui::flat)
                .on_press(Message::SeekTo(l.ms.max(0) as f32 / 1000.0)),
        );
    }
    // room below the last line, so it can scroll up to the middle
    col = col.push(ui::gap(240));
    scrollable(col).id(lyrics_scroller()).style(ui::scroller).height(Length::Fill).into()
}

/* ---------- get more music (in Search) and requests ---------- */

/// Search's "Get more music": music that isn't on the server, from MusicBrainz, each kind shown as
/// soon as it arrives (as on the website). `None` when there's nothing to show.
pub fn get_more(app: &App) -> Option<El<'_>> {
    let f = &app.find;
    let lidarr = app.me.as_ref().map(|m| m.lidarr.configured).unwrap_or(false);
    let q = app.search.trim();
    if !lidarr || app.offline || q.chars().count() < 2 || f.searched != q {
        return None;
    }
    let kinds = app.search_type.kinds();
    if kinds.is_empty() {
        return None;
    }
    let all = app.search_type == crate::SearchType::All;
    let pending = kinds.iter().any(|k| !f.parts.contains_key(*k));
    let mut body = Column::new().spacing(10);
    let mut shown = false;
    let mut failure = None;
    for kind in kinds {
        let title = match *kind {
            "artists" => "Artists",
            "songs" => "Songs",
            _ => "Albums",
        };
        match f.parts.get(*kind) {
            None => {
                body = body.push(ui::dim(title, 14)).push(ui::dim("Searching…", 13));
            }
            Some(Err(e)) => failure = Some(e.clone()),
            Some(Ok(list)) if list.is_empty() => {}
            Some(Ok(list)) => {
                shown = true;
                let limit = match *kind {
                    "artists" => 4,
                    "songs" => 6,
                    _ => 8,
                };
                let limit = if all && !f.open.contains(*kind) { limit } else { list.len() };
                let rows: Vec<El> = list.iter().take(limit).map(|s| found_row(app, s, *kind)).collect();
                body = body.push(text(title).size(16).font(ui::FONT_BOLD)).push(Column::with_children(rows).spacing(2));
                if list.len() > limit {
                    body = body.push(button(text(format!("Show {} more", list.len() - limit)).size(13)).padding([5, 12]).style(ui::flat).on_press(Message::ShowMoreFound(kind.to_string())));
                }
            }
        }
    }
    if !pending && !shown {
        // nothing to offer (or MusicBrainz didn't answer)
        return failure.map(|e| section("Get more music", container(text(e).size(14)).padding([10, 12]).style(ui::error_box).into()));
    }
    let sub = if pending && !shown {
        "Searching…"
    } else if f.can_get {
        "Not in your library yet? Get it, and Lidarr downloads it and adds it here when it's done."
    } else {
        "Only admins can ask for new music."
    };
    let head = row![
        column![ui::title("Get more music", 22), ui::dim(sub, 13)].spacing(4).width(Length::Fill),
        button(text("Your requests").size(13)).padding([6, 14]).style(ui::secondary).on_press(Message::Go(Page::GetMusic)),
    ]
    .align_y(Alignment::Center);
    let mut c = column![head, body].spacing(12);
    if !pending {
        c = c.push(ui::dim("Results from MusicBrainz.", 12));
    }
    Some(c.into())
}

/// An artist from MusicBrainz: everything they have released, each with its state (in your library, asked for, or a
/// Get button). Reached from a found artist, or from "See all releases" on a library artist's page.
pub fn discover_page<'a>(app: &'a App, id: &str) -> El<'a> {
    let Some(d) = app.discovered.get(id) else {
        return column![ui::title("Artist", 32), ui::dim("Looking it up on MusicBrainz…", 15)].spacing(10).into();
    };
    let left = d.albums.iter().filter(|a| !a.is_on_server() && !a.requested && !app.find.asked.contains(a.foreign_id.as_deref().unwrap_or(""))).count();
    let mut actions = row![].spacing(10).align_y(Alignment::Center);
    if let Some(n) = &d.on_server {
        actions = actions.push(ui::btn_icon("check", "Open in your library", ui::Btn::Secondary, false).on_press(Message::Go(Page::Artist(n.clone()))));
    }
    if left > 0 && app.find.can_get {
        actions = actions.push(ui::btn_icon("download", format!("Get everything ({left})"), ui::Btn::Primary, false).on_press(Message::RequestMusic("artist".into(), id.to_string())));
    }
    let mut c = column![ui::dim(if d.on_server.is_some() { "Artist" } else { "Not in your library yet" }, 13), ui::title(d.name.clone(), 40), ui::dim(format!("{} releases", d.albums.len()), 15), actions].spacing(8);
    for (title, single) in [("Albums & EPs", false), ("Singles", true)] {
        let rows: Vec<El> = d.albums.iter().filter(|a| (a.kind.as_deref() == Some("Single")) == single).map(|a| found_row(app, a, "albums")).collect();
        if !rows.is_empty() {
            c = c.push(ui::gap(12)).push(ui::title(title, 22)).push(Column::with_children(rows).spacing(2));
        }
    }
    if d.albums.is_empty() {
        c = c.push(ui::dim("MusicBrainz doesn't list any albums for this artist yet.", 14));
    }
    c.push(ui::dim("Lidarr gets whole releases, so a song comes with its album or single.", 12)).into()
}

/// Under a library artist's albums: what they have released that you don't have yet, with a way to get it.
pub fn artist_get_more<'a>(app: &'a App, name: &str) -> Option<El<'a>> {
    if !app.me.as_ref().map(|m| m.lidarr.configured).unwrap_or(false) || app.offline {
        return None;
    }
    let id = app.artist_mbids.get(&name.to_lowercase())?.as_ref()?;
    let d = app.discovered.get(id)?;
    let missing: Vec<&FoundItem> = d.albums.iter().filter(|a| !a.is_on_server()).collect();
    if missing.is_empty() {
        return None;
    }
    let rows: Vec<El> = missing.iter().take(6).map(|a| found_row(app, a, "albums")).collect();
    let head = row![
        column![ui::title("Get more", 22), ui::dim(format!("More by {name} that isn't in your library yet."), 13)].spacing(4).width(Length::Fill),
        button(text("See all releases").size(13)).padding([6, 14]).style(ui::secondary).on_press(Message::Go(Page::Discover(id.clone()))),
    ]
    .align_y(Alignment::Center);
    Some(column![head, Column::with_children(rows).spacing(2)].spacing(10).into())
}

/// Requests: what people asked Lidarr for, and where it is.
pub fn requests(app: &App) -> El<'_> {
    let f = &app.find;
    if !f.configured {
        let (about, action): (&str, Option<El>) = if app.can("settings") {
            ("Connect Lidarr in Settings to search for music you don't have and add it automatically.", Some(ui::btn("Connect Lidarr", ui::Btn::Primary).on_press(Message::Go(Page::Settings)).into()))
        } else {
            ("Ask whoever runs this server to connect Lidarr.", None)
        };
        return column![ui::title("Requests", 32), crate::views::empty_state("download", "Lidarr isn't connected", about, action)].into();
    }
    let mut c = column![ui::title("Requests", 32), ui::dim("Music Lidarr is getting for you. It appears in your library as soon as it's imported.", 15)].spacing(16);
    // what Lidarr is downloading right now
    c = c.push(ui::title("Downloading", 22));
    let items: Vec<serde_json::Value> = f.queue.as_ref().and_then(|q| q["items"].as_array().cloned()).unwrap_or_default();
    if let Some(e) = f.queue.as_ref().and_then(|q| q["error"].as_str()) {
        c = c.push(container(text(e).size(13)).padding([8, 10]).width(Length::Fill).style(ui::error_box));
    } else if items.is_empty() {
        c = c.push(ui::dim("Nothing downloading right now.", 13));
    }
    for d in items {
        let title = d["title"].as_str().unwrap_or("").to_string();
        let artist = d["artist"].as_str().unwrap_or("").to_string();
        let state = d["state"].as_str().unwrap_or("").to_string();
        let pct = (d["progress"].as_f64().unwrap_or(0.0) * 100.0).round();
        let label = match state.as_str() {
            "downloading" => "Downloading", "importPending" => "Ready to import", "importing" => "Importing", "imported" => "Imported", "failedPending" => "Problem",
            "failed" => "Failed", "queued" | "delay" => "Waiting", "paused" => "Paused", "completed" => "Finishing", other => other,
        }
        .to_string();
        let left = d["timeLeft"].as_str().map(|t| {
            let p: Vec<u64> = t.split(|c| c == ':' || c == '.').filter_map(|x| x.parse().ok()).collect();
            let secs = if p.len() >= 3 { p[p.len() - 3] * 3600 + p[p.len() - 2] * 60 + p[p.len() - 1] } else { 0 };
            if secs == 0 { String::new() } else if secs >= 3600 { format!(" · {} h left", (secs as f64 / 3600.0).round()) } else if secs >= 60 { format!(" · {} min left", (secs as f64 / 60.0).round()) } else { " · Almost done".to_string() }
        }).unwrap_or_default();
        let mut meta = label;
        if state == "downloading" {
            meta = format!("{meta} · {pct}%{left}");
        }
        if let Some(m) = d["message"].as_str().filter(|m| !m.is_empty()) {
            meta = format!("{meta} · {m}");
        }
        let art: El = match d["image"].as_str().and_then(|p| app.images.get(p)) {
            Some(h) => image(h.clone()).width(48).height(48).content_fit(iced::ContentFit::Cover).into(),
            None => ui::cover(None, &title, 48.0),
        };
        let bar = iced::widget::progress_bar(0.0..=100.0, pct as f32).height(4).style(|_: &iced::Theme| iced::widget::progress_bar::Style { background: ui::SURFACE_3().into(), bar: ui::ACCENT().into(), border: iced::border::rounded(2.0) });
        c = c.push(container(row![art, column![text(clip(&title, 60)).size(15), ui::dim(artist, 12), bar, ui::dim(meta, 12)].spacing(3).width(Length::Fill)].spacing(14).align_y(Alignment::Center)).padding([6, 12]));
    }
    if f.requests.is_empty() {
        return c
            .push(ui::title("Requested", 22))
            .push(crate::views::empty_state("search", "Nothing requested yet", "Search for an artist or album. Anything not in your library can be requested from there.", Some(ui::btn("Search", ui::Btn::Primary).on_press(Message::Go(Page::Search)).into())))
            .into();
    }
    c = c.push(ui::title("Requested", 22));
    let me = app.me.as_ref().map(|m| m.user.name.clone()).unwrap_or_default();
    for r in &f.requests {
        let here = !(r.on_server.is_null() || r.on_server == serde_json::Value::Bool(false));
        let state = if here {
            "In your library".to_string()
        } else if let Some(p) = &r.problem {
            p.clone()
        } else {
            "Waiting for Lidarr".to_string()
        };
        let art: El = match r.image.as_ref().and_then(|p| app.images.get(p)) {
            Some(h) => image(h.clone()).width(48).height(48).content_fit(iced::ContentFit::Cover).into(),
            None => ui::cover(None, &r.title, 48.0),
        };
        let artist_req = r.kind == "artist";
        let who = if r.by == me { "you".to_string() } else { r.by.clone() };
        let name = if artist_req { format!("Everything by {}", r.artist) } else { r.title.clone() };
        let sub = format!("{}Requested by {who} · {}", if artist_req { String::new() } else { format!("{} · ", r.artist) }, ago(r.at));
        let mut line = row![
            art,
            column![text(clip(&name, 60)).size(15), ui::dim(sub, 12)].spacing(2).width(Length::Fill)
        ]
        .spacing(14)
        .align_y(Alignment::Center);
        if here {
            line = line.push(ui::btn_icon("check", "In your library", ui::Btn::Secondary, false));
        } else {
            line = line.push(ui::dim(state.clone(), 13));
            if r.by == me {
                line = line.push(ui::btn_sm("Cancel", ui::Btn::Ghost).on_press(Message::CancelRequest(r.foreign_id.clone())));
            }
        }
        c = c.push(container(line).padding([6, 12]));
    }
    c.into()
}

/// One found artist / song / album, with what you can do about it.
fn found_row<'a>(app: &'a App, s: &'a FoundItem, kind: &str) -> El<'a> {
    let is_artist = kind == "artists";
    let art: El = match s.image.as_ref().and_then(|p| app.images.get(p)) {
        Some(h) => image(h.clone()).width(48).height(48).content_fit(iced::ContentFit::Cover).into(),
        None => ui::cover(None, &s.label(), 48.0),
    };
    let mut sub = vec![];
    if is_artist {
        sub.push("Artist".to_string());
    }
    if let Some(a) = &s.artist {
        sub.push(a.clone());
    }
    if let Some(k) = s.kind.as_ref().filter(|k| !is_artist && k.as_str() != "Album") {
        sub.push(k.clone());
    }
    if let Some(y) = &s.year {
        sub.push(y.clone());
    }
    let asked = |id: &Option<String>| id.as_ref().map(|i| app.find.asked.contains(i)).unwrap_or(false);
    let get = |label: &'a str, k: &'static str, id: String, primary: bool| -> El<'a> {
        let busy = app.find.asking.contains(&id);
        button(row![ic(icon::DOWNLOAD, 12), text(if busy { "Asking…" } else { label }).size(13)].spacing(6).align_y(Alignment::Center))
            .padding([6, 14])
            .style(if primary { ui::primary } else { ui::secondary })
            .on_press_maybe((!busy).then_some(Message::RequestMusic(k.to_string(), id)))
            .into()
    };
    let can = app.find.can_get;
    let action: El = if s.is_on_server() {
        let go = match (&s.on_server, is_artist) {
            (serde_json::Value::String(v), false) if app.lib.album(v).is_some() => Some(Message::Go(Page::Album(v.clone()))),
            (serde_json::Value::String(v), true) => Some(Message::Go(Page::Artist(v.clone()))),
            _ => None,
        };
        let mut r = row![button(row![ic(icon::DOWNLOADED, 12), text("In your library").size(13)].spacing(6).align_y(Alignment::Center)).padding([6, 14]).style(ui::secondary).on_press_maybe(go)].spacing(6);
        if let (true, Some(id)) = (is_artist, &s.foreign_id) {
            // you have some of their music: this is how to find and get the rest
            r = r.push(button(text("More from them").size(13)).padding([6, 14]).style(ui::secondary).on_press(Message::Go(Page::Discover(id.clone()))));
        }
        r.into()
    } else if s.requested || asked(&s.foreign_id) || asked(&s.album_id) || asked(&s.single_id) {
        row![ic(icon::DOWNLOAD, 12), ui::dim("Requested", 13)].spacing(6).align_y(Alignment::Center).into()
    } else if !can {
        ui::dim("Ask an admin", 13).into()
    } else if is_artist {
        match &s.foreign_id {
            Some(id) => get("Get all", "artist", id.clone(), true),
            None => ui::dim("", 13).into(),
        }
    } else if kind == "songs" {
        // Lidarr gets releases, not songs: offer the single (if any) and the album
        let mut r = row![].spacing(6);
        if let Some(id) = &s.single_id {
            r = r.push(get("Get the single", "album", id.clone(), true));
        }
        if let Some(id) = &s.album_id {
            r = r.push(get("Get the album", "album", id.clone(), s.single_id.is_none()));
        }
        r.into()
    } else {
        match &s.foreign_id {
            Some(id) => get("Get", "album", id.clone(), true),
            None => ui::dim("", 13).into(),
        }
    };
    container(row![art, column![text(clip(&s.label(), 60)).size(15), ui::dim(clip(&sub.join(" · "), 70), 12)].spacing(2).width(Length::Fill), action].spacing(14).align_y(Alignment::Center)).padding([6, 12]).into()
}

/* ---------- credits (Now Playing) ---------- */

/// A link-looking text button (names in Credits).
fn link<'a>(label: String, to: Message) -> El<'a> {
    button(text(label).size(15)).padding(0).on_press(to).style(|_, s| button::Style {
        text_color: if matches!(s, button::Status::Hovered) { ui::ACCENT() } else { ui::TEXT() },
        ..Default::default()
    }).into()
}

/// "Jane Doe, Sam Lee & Kit Moss": each name that's an artist in your library opens their page
/// (a band name with "&" in it, like "Juniper & Ash", stays whole).
fn people_links<'a>(app: &'a App, names: &str) -> El<'a> {
    let mut parts: Vec<El> = vec![];
    let push_name = |parts: &mut Vec<El<'a>>, n: &str| {
        let n = n.trim();
        if app.lib.has_artist(n) {
            parts.push(link(n.to_string(), Message::Go(Page::Artist(n.to_string()))));
        } else {
            parts.push(text(n.to_string()).size(15).into());
        }
    };
    for (i, piece) in names.split(", ").enumerate() {
        if i > 0 {
            parts.push(text(", ").size(15).into());
        }
        if app.lib.has_artist(piece) || !piece.contains(" & ") {
            push_name(&mut parts, piece);
        } else {
            for (j, n) in piece.split(" & ").enumerate() {
                if j > 0 {
                    parts.push(text(" & ").size(15).into());
                }
                push_name(&mut parts, n);
            }
        }
    }
    Row::with_children(parts).wrap().into()
}

pub fn credits<'a>(app: &'a App, t: &'a Track) -> El<'a> {
    let album = app.lib.album_of(t);
    let row_of = |k: String, v: String| -> El<'a> { row![container(ui::dim(k, 14)).width(150), text(v).size(15)].spacing(16).into() };
    let row_el = |k: String, v: El<'a>| -> El<'a> { row![container(ui::dim(k, 14)).width(150), v].spacing(16).into() };
    let own: Vec<&volvix_core::Credit> = t.credits.iter().filter(|c| c.role != "ISRC").collect();
    let mut people = Column::new().spacing(10).push(row_el("Artist".into(), people_links(app, &t.artist)));
    for c in &own {
        people = people.push(row_el(c.role.clone(), people_links(app, &c.names)));
    }
    // MusicBrainz fills in what the file doesn't say
    let note: String = match app.credits.get(&t.id) {
        None => "Looking up more on MusicBrainz…".into(),
        Some(Err(e)) => {
            if own.is_empty() {
                e.clone()
            } else {
                String::new()
            }
        }
        Some(Ok(list)) => {
            let extra: Vec<&volvix_core::Credit> = list.iter().filter(|c| !own.iter().any(|o| o.role == c.role)).collect();
            for c in &extra {
                people = people.push(row_el(c.role.clone(), people_links(app, &c.names)));
            }
            if list.is_empty() {
                if own.is_empty() { "No credits found for this song in the file or on MusicBrainz.".into() } else { String::new() }
            } else if extra.is_empty() {
                "MusicBrainz matches what's in the file.".into()
            } else {
                "MusicBrainz added credits the file doesn't have.".into()
            }
        }
    };
    let mut detail_rows: Vec<El> = vec![];
    if let Some(a) = album {
        detail_rows.push(row_el("Album".into(), link(a.title.clone(), Message::Go(Page::Album(a.id.clone())))));
    }
    let mut add = |k: &str, v: Option<String>| {
        if let Some(v) = v.filter(|v| !v.is_empty()) {
            detail_rows.push(row_of(k.to_string(), v));
        }
    };
    add("Year", album.and_then(|a| a.year).map(|y| y.to_string()));
    add("Label", album.and_then(|a| a.label.clone()));
    add("Genre", album.and_then(|a| a.genre.clone()));
    add(
        "Format",
        album.map(|a| {
            let q = &a.quality;
            match (q.bits, q.khz, q.kbps) {
                (Some(b), Some(k), _) => format!("{} · {b}-bit / {k} kHz", q.codec.to_uppercase()),
                (_, _, Some(k)) => format!("{} · {k} kbps", q.codec.to_uppercase()),
                _ => q.codec.to_uppercase(),
            }
        }),
    );
    add("Length", (t.duration > 0).then(|| volvix_core::format_duration(t.duration)));
    add("Copyright", album.and_then(|a| a.copyright.clone()));
    add("ISRC", t.credits.iter().find(|c| c.role == "ISRC").map(|c| c.names.clone()));
    let details = Column::with_children(detail_rows).spacing(10);
    let mine = volvix_core::userdata::stats(&app.data).plays.get(&t.id).copied();
    let listening: El = match mine {
        Some((n, _)) => row_of("Plays".into(), n.to_string()),
        None => row_of("Plays".into(), "Not played yet".into()),
    };
    let mut c = column![ui::title("Credits", 20), people].spacing(14);
    if !note.is_empty() {
        c = c.push(ui::dim(note, 12));
    }
    c = c.push(ui::title("Details", 20)).push(details).push(ui::title("Your listening", 20)).push(listening);
    scrollable(c).style(ui::scroller).height(Length::Fill).into()
}

/* ---------- household and notifications ---------- */

pub fn household(app: &App) -> El<'_> {
    let mut c = column![ui::title("Household", 32), ui::dim("What everyone is listening to, and the playlists they've made public.", 15)].spacing(16);
    if !app.invites.is_empty() {
        let rows: Vec<El> = app
            .invites
            .iter()
            .map(|i| {
                container(
                    row![
                        column![text(format!("{} invited you to \"{}\"", i.owner_name, i.name)).size(15), ui::dim(format!("{} songs · collaborative", i.count), 12)].spacing(2).width(Length::Fill),
                        button(text("Accept").size(13)).padding([6, 16]).style(ui::primary).on_press(Message::SharedOp(i.id.clone(), serde_json::json!({ "op": "accept" }))),
                        button(text("Decline").size(13)).padding([6, 16]).style(ui::secondary).on_press(Message::SharedOp(i.id.clone(), serde_json::json!({ "op": "decline" }))),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([10, 12])
                .into()
            })
            .collect();
        c = c.push(section("Invites", Column::with_children(rows).spacing(4).into()));
    }
    if app.household.members.is_empty() {
        return c.push(ui::dim("No one else is in your household yet. An admin can add people on the website (Account → People).", 15)).into();
    }
    // listening now first, then the most recently active (like the website)
    let mut members: Vec<&_> = app.household.members.iter().collect();
    members.sort_by_key(|m| (std::cmp::Reverse(m.now.as_ref().and_then(|n| n["listening"].as_bool()).unwrap_or(false)), std::cmp::Reverse(m.now.as_ref().and_then(|n| n["at"].as_u64()).unwrap_or(0)), m.name.clone()));
    let mut cards: Vec<El> = vec![];
    for m in &members {
        let (live, line, play): (bool, El, Option<String>) = match &m.now {
            None => (false, ui::dim("Nothing shared yet", 13).into(), None),
            Some(n) => {
                let title = n["title"].as_str().unwrap_or("").to_string();
                let artist = n["artist"].as_str().unwrap_or("").to_string();
                let live = n["listening"].as_bool().unwrap_or(false);
                let id = n["id"].as_str().or(n["trackId"].as_str()).unwrap_or("").to_string();
                let lead: El = if live { crate::marks::eq(true, 12.0) } else { ui::dim(format!("Last played {}", n["at"].as_u64().map(ago).unwrap_or_default()), 13).into() };
                let song = text(clip(&format!("{title} Â· {artist}"), 40)).size(13).color(ui::TEXT()).wrapping(text::Wrapping::None);
                (live, row![lead, song].spacing(8).align_y(Alignment::Center).into(), (!id.is_empty()).then_some(id))
            }
        };
        let mut r = row![ui::avatar_key::<Message>(&m.id, &m.name, 44.0), column![text(m.name.clone()).size(15).font(ui::FONT_BOLD), line].spacing(2).width(Length::Fill)].spacing(12).align_y(Alignment::Center);
        if let Some(id) = play {
            r = r.push(ui::tip(button(ui::wi("play", 18.0, ui::TEXT())).padding(8).style(ui::secondary).on_press(Message::PlayTracks(vec![id], 0, format!("{}'s pick", m.name))), "Play this song"));
        }
        cards.push(
            container(r)
                .padding(12)
                .width(Length::Fixed(430.0))
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(ui::SURFACE_2().into()),
                    border: iced::Border { color: if live { ui::ACCENT() } else { ui::BORDER() }, width: 1.0, radius: 12.0.into() },
                    ..Default::default()
                })
                .into(),
        );
    }
    c = c.push(column![ui::title("Listening", 22), Row::with_children(cards).spacing(10).wrap()].spacing(12));
    let mut any_lists = false;
    for m in &members {
        if m.playlists.is_empty() {
            continue;
        }
        any_lists = true;
        let first = m.name.split(' ').next().unwrap_or("").to_string();
        let tiles: Vec<El> = m
            .playlists
            .iter()
            .map(|p| {
                let ids: Vec<String> = p.tracks.iter().map(|t| t.id.clone()).collect();
                card(playlist_cover(app, &ids, false, &p.name, 164.0), p.name.clone(), format!("{} songs", ids.len()), Message::Go(Page::Playlist(p.id.clone())))
            })
            .collect();
        c = c.push(column![ui::title(format!("{first}'s playlists"), 22), Row::with_children(tiles).spacing(4).wrap()].spacing(12));
    }
    if !any_lists {
        c = c.push(row![ui::wi("lock", 12.0, ui::TEXT_2()), ui::dim("Nobody has made a playlist public yet. In a playlist's Edit details, turn on \"Visible to the household\".", 13)].spacing(6).align_y(Alignment::Center));
    }
    c.into()
}

fn ago(at: u64) -> String {
    let d = volvix_core::userdata::now().saturating_sub(at);
    if d < 60 {
        "just now".into()
    } else if d < 3600 {
        format!("{} min ago", d / 60)
    } else if d < 86400 {
        format!("{} h ago", d / 3600)
    } else {
        format!("{} d ago", d / 86400)
    }
}

/// Notifications, like the website's: cards with a round icon, the text and how long ago, and what you can do about it.
pub fn notifications(app: &App) -> El<'_> {
    let head = row![ui::title("Notifications", 32), horizontal_space(), ui::btn_sm("Mark all as read", ui::Btn::Secondary).on_press(Message::MarkAllRead)].align_y(Alignment::End);
    if app.notes.items.is_empty() {
        return column![head, crate::views::empty_state("bell", "Nothing new", "Playlist invites, party invites and music you requested show up here.", None)].spacing(20).into();
    }
    let state = |label: &'static str, tick: bool| -> El<'_> {
        let mut r = Row::new().spacing(6).align_y(Alignment::Center);
        if tick {
            r = r.push(ui::wi("check", 14.0, ui::TEXT_2()));
        }
        r.push(text(label).size(13).font(ui::FONT_BOLD).color(ui::TEXT_2())).into()
    };
    let mut list = Column::new().spacing(6);
    for n in &app.notes.items {
        let icon_name = match n.kind.as_str() {
            "playlist_invite" => "users",
            "playlist_joined" => "check",
            "party_invite" => "party",
            "request_ready" => "download",
            _ => "bell",
        };
        let pl = n.data["playlistId"].as_str().map(String::from);
        let mut actions = Row::new().spacing(6).align_y(Alignment::Center);
        match (n.kind.as_str(), n.done.as_deref()) {
            ("playlist_invite", None) => {
                if let Some(pl) = pl {
                    actions = actions
                        .push(ui::btn_sm("Accept", ui::Btn::Primary).on_press(Message::SharedOp(pl.clone(), serde_json::json!({ "op": "accept" }))))
                        .push(ui::btn_sm("Decline", ui::Btn::Secondary).on_press(Message::SharedOp(pl, serde_json::json!({ "op": "decline" }))));
                }
            }
            ("playlist_invite", Some(d)) => actions = actions.push(if d == "accepted" { state("Joined", true) } else { state("Declined", false) }),
            ("playlist_joined", _) => {
                if let Some(pl) = pl {
                    actions = actions.push(ui::btn_sm("Open", ui::Btn::Secondary).on_press(Message::Go(Page::Playlist(pl))));
                }
            }
            ("party_invite", Some(d)) => actions = actions.push(if d == "joined" { state("Joined", true) } else { state("Over", false) }),
            ("request_ready", _) => {
                if let Some(id) = n.data["albumId"].as_str() {
                    let page = if n.data["kind"].as_str() == Some("artist") { Page::Artist(id.to_string()) } else { Page::Album(id.to_string()) };
                    actions = actions.push(ui::btn_sm("Open", ui::Btn::Secondary).on_press(Message::Go(page)));
                }
            }
            _ => {}
        }
        let unread = !n.read;
        let dot: El = container(iced::widget::Space::new(6, 6)).style(move |_| container::Style { background: unread.then(|| ui::ACCENT().into()), border: iced::border::rounded(3.0), ..Default::default() }).into();
        list = list.push(
            container(
                row![
                    dot,
                    container(ui::wi(icon_name, 18.0, ui::TEXT_2())).width(36).height(36).center_x(36).center_y(36).style(|_| container::Style { background: Some(ui::SURFACE_3().into()), border: iced::border::rounded(18.0), ..Default::default() }),
                    column![text(n.text.clone()).size(15), text(ago(n.at)).size(12).color(ui::TEXT_3())].spacing(2).width(Length::Fill),
                    actions,
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding(iced::Padding { top: 12.0, bottom: 12.0, left: 4.0, right: 14.0 })
            .style(|_| container::Style { background: Some(ui::SURFACE_2().into()), border: iced::Border { radius: 12.0.into(), width: 1.0, color: ui::BORDER() }, ..Default::default() }),
        );
    }
    column![head, container(list).max_width(760)].spacing(20).into()
}

/* ---------- settings ---------- */

fn setting_row<'a>(title: impl text::IntoFragment<'a>, sub: impl text::IntoFragment<'a>, control: El<'a>) -> El<'a> {
    container(row![column![text(title).size(15), ui::dim(sub, 13)].spacing(2).width(Length::Fill), control].spacing(16).align_y(Alignment::Center)).padding([10, 4]).into()
}

/// The website's description box: a few lines tall.
fn desc_box<'a>(app: &'a App) -> El<'a> {
    iced::widget::text_editor(&app.desc_editor)
        .placeholder("Add an optional description")
        .on_action(Message::DescEdit)
        .height(84)
        .padding(10)
        .size(15)
        .style(|_: &iced::Theme, s| {
            use iced::widget::text_editor::{Status, Style};
            Style {
                background: ui::INPUT_BG().into(),
                border: iced::Border { radius: 8.0.into(), width: 1.0, color: if matches!(s, Status::Focused) { ui::TEXT_2() } else { ui::BORDER() } },
                icon: ui::TEXT_2(),
                placeholder: ui::TEXT_3(),
                value: ui::TEXT(),
                selection: Color { a: 0.35, ..ui::ACCENT() },
            }
        })
        .into()
}

/// A small label above a form field, like the website's.
fn labelled<'a>(label: &'a str, field: El<'a>) -> El<'a> {
    column![text(label).size(13).font(ui::FONT_BOLD), field].spacing(6).into()
}

/// The website's switch row: a bordered card with a title, a line of text and the switch.
fn switch_card<'a>(title: &'a str, about: &'a str, on: bool, f: fn(bool) -> Message) -> El<'a> {
    container(row![column![text(title).size(14).font(ui::FONT_BOLD), ui::dim(about, 13)].spacing(2).width(Length::Fill), switch(on, f)].spacing(16).align_y(Alignment::Center))
        .padding([12, 14])
        .width(Length::Fill)
        .style(|_: &iced::Theme| container::Style { border: iced::Border { color: ui::BORDER(), width: 1.0, radius: 10.0.into() }, ..Default::default() })
        .into()
}

fn switch<'a>(on: bool, f: fn(bool) -> Message) -> El<'a> {
    toggler(on).on_toggle(f).size(24).style(ui::switch_style).into()
}

/// What a member can be allowed to do: key, title, explanation (the website's list).
pub const PERMISSIONS: [(&str, &str, &str); 6] = [
    ("request", "Request music", "Ask Lidarr to download music that isn't on the server yet."),
    ("rescan", "Check for new music", "Look in the music folder for added or removed songs right now."),
    ("parties", "Start listening parties", "Host a party and invite the household."),
    ("members", "Manage members", "Add people, change what they may do, set passwords and remove them."),
    ("settings", "Server settings", "Change the music folder and the Lidarr connection."),
    ("share", "Create share links", "Make a link to an album or playlist that anyone with the link can play for a few days."),
];

/// A row with a button: the button sits right after the text, not pushed to the far edge.
fn action_row<'a>(title: impl text::IntoFragment<'a>, sub: impl text::IntoFragment<'a>, control: El<'a>) -> El<'a> {
    container(row![column![text(title).size(15), ui::dim(sub, 13)].spacing(2).max_width(520), control].spacing(24).align_y(Alignment::Center)).padding([10, 4]).into()
}

/* ---------- Rewind: your year in music ---------- */

fn year_of(secs: u64) -> i32 {
    // days since 1970 → calendar year
    let z = (secs / 86400) as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let m = (5 * doy + 2) / 153;
    let month = if m < 10 { m + 3 } else { m - 9 };
    (yoe + era * 400 + if month <= 2 { 1 } else { 0 }) as i32
}

fn rewind_years(app: &App) -> Vec<i32> {
    let mut years: Vec<i32> = app.data.rest.get("stats").and_then(|s| s.get("years")).and_then(|y| y.as_object()).map(|m| m.keys().filter_map(|k| k.parse().ok()).collect()).unwrap_or_default();
    years.sort();
    years
}

/// The year to show: the one picked, or the latest year that has ended.
pub fn rewind_year(app: &App) -> Option<i32> {
    let this = year_of(volvix_core::userdata::now());
    let done: Vec<i32> = rewind_years(app).into_iter().filter(|y| *y < this).collect();
    app.rewind_year.filter(|y| done.contains(y)).or(done.last().copied())
}

/// The songs played in a year, most played first.
pub fn rewind_songs(app: &App, year: i32) -> Vec<(&volvix_core::Track, u64)> {
    let plays = app.data.rest.get("stats").and_then(|s| s.get("years")).and_then(|y| y.get(year.to_string())).and_then(|d| d.get("plays")).and_then(|p| p.as_object());
    let mut songs: Vec<(&volvix_core::Track, u64)> = plays.map(|m| m.iter().filter_map(|(id, n)| Some((app.lib.track(id)?, n.as_u64()?))).collect()).unwrap_or_default();
    songs.sort_by(|a, b| b.1.cmp(&a.1));
    songs
}

pub fn rewind(app: &App) -> El<'_> {
    let this = year_of(volvix_core::userdata::now());
    let Some(y) = rewind_year(app) else {
        let have = app.data.rest.get("stats").and_then(|s| s.get("years")).and_then(|y| y.get(this.to_string())).and_then(|d| d.get("plays")).and_then(|p| p.as_object()).map(|p| p.len());
        return column![
            ui::title("Rewind", 32),
            crate::views::empty_state(
                "history",
                format!("Your {this} Rewind opens on January 1, {}", this + 1),
                match have {
                    Some(n) => format!("It's building as you listen: {n} songs so far."),
                    None => "Play some music and it starts building.".to_string(),
                },
                None
            ),
        ]
        .into();
    };
    let songs = rewind_songs(app, y);
    let plays: u64 = songs.iter().map(|s| s.1).sum();
    let minutes = app.data.rest.get("stats").and_then(|s| s.get("years")).and_then(|d| d.get(y.to_string())).and_then(|d| d.get("sec")).and_then(|v| v.as_f64()).map(|s| (s / 60.0).round() as u64).unwrap_or(0);
    if plays == 0 {
        return column![ui::title(format!("Rewind {y}"), 32), crate::views::empty_state("history", format!("No plays in {y}"), "Nothing was recorded that year.", None)].into();
    }
    let mut artists: std::collections::HashMap<&str, u64> = Default::default();
    let mut albums: std::collections::HashMap<&str, u64> = Default::default();
    for (t, n) in &songs {
        *artists.entry(t.artist.as_str()).or_default() += n;
        *albums.entry(t.album_id.as_str()).or_default() += n;
    }
    fn rank<'k>(m: std::collections::HashMap<&'k str, u64>) -> Vec<(&'k str, u64)> {
        let mut v: Vec<(&str, u64)> = m.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        v
    }
    let (artists, albums) = (rank(artists), rank(albums));
    let stat = |n: String, label: &'static str| -> El<'static> { column![text(n).size(30).font(ui::FONT_BOLD), ui::dim(label, 13)].spacing(2).into() };
    let hero = column![
        text(y.to_string()).size(56).font(ui::FONT_BOLD).color(ui::ACCENT()),
        ui::title("Your year in music", 28),
        row![stat(minutes.to_string(), "minutes listened"), stat(plays.to_string(), "plays"), stat(songs.len().to_string(), "different songs")].spacing(40),
    ]
    .spacing(8);

    let top_ids: Vec<String> = songs.iter().take(5).map(|(t, _)| t.id.clone()).collect();
    let mut rows = Column::new().spacing(6);
    for (i, (t, n)) in songs.iter().take(5).enumerate() {
        rows = rows.push(
            row![
                container(text((i + 1).to_string()).size(15).color(ui::TEXT_2())).width(24),
                cover_of(app, app.lib.album_of(t), 44.0),
                column![text(clip(&t.title, 40)).size(15), ui::dim(clip(&t.artist, 40), 12)].spacing(2).width(Length::Fill),
                ui::dim(format!("{n} plays"), 13),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        );
    }
    let top_songs = column![
        row![ui::title("Top songs", 22), horizontal_space(), button(row![ic(icon::PLAY, 13), text("Play them").size(13)].spacing(8).align_y(Alignment::Center)).padding([7, 16]).style(ui::secondary).on_press(Message::PlayTracks(top_ids, 0, format!("Your {y} Rewind")))].align_y(Alignment::Center),
        rows,
    ]
    .spacing(12);

    let mut chips = Row::new().spacing(8);
    for (i, (name, n)) in artists.iter().take(5).enumerate() {
        chips = chips.push(button(text(format!("{}  {}  ·  {n} plays", i + 1, clip(name, 22))).size(13)).padding([8, 14]).style(ui::chip(false)).on_press(Message::Go(Page::Artist(name.to_string()))));
    }
    let top_artists = column![ui::title("Top artists", 22), scrollable(chips).direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new().width(4).scroller_width(4))).style(ui::scroller)].spacing(12);

    let mut page = column![hero, top_songs, top_artists].spacing(28);
    if let Some(a) = albums.first().and_then(|(id, _)| app.lib.album(id)) {
        page = page.push(column![ui::title("Top album", 22), card(cover_of(app, Some(a), 164.0), a.title.clone(), a.artist.clone(), Message::Go(Page::Album(a.id.clone())))].spacing(12));
    }
    let done: Vec<i32> = rewind_years(app).into_iter().filter(|v| *v < this && *v != y).collect();
    if !done.is_empty() {
        let mut other = Row::new().spacing(8).align_y(Alignment::Center);
        other = other.push(ui::dim("Other years:", 13));
        for v in done {
            other = other.push(button(text(v.to_string()).size(13)).padding([6, 12]).style(ui::chip(false)).on_press(Message::RewindYear(v)));
        }
        page = page.push(other);
    }
    page.into()
}

/* ---------- Manage a member (admins) ---------- */

fn manage_dialog<'a>(app: &'a App, id: &'a str, password: &'a str, error: &'a Option<String>, confirm_remove: bool) -> El<'a> {
    let Some(m) = app.members.iter().find(|m| m.id == id) else { return ui::dim("That member isn't here any more.", 14).into() };
    let me = app.me.as_ref().map(|u| u.user.clone()).unwrap_or_default();
    let have = |p: &str| me.admin || me.perms.iter().any(|x| x == p);
    let switch_row = |title: &'a str, sub: String, on: bool, key: String, enabled: bool| -> El<'a> {
        let t = toggler(on).size(22).style(ui::switch_style);
        let t = if enabled { t.on_toggle(move |_| Message::ManageToggle(key.clone())) } else { t };
        container(row![column![text(title).size(14), ui::dim(sub, 12)].spacing(2).width(Length::Fill), t].spacing(12).align_y(Alignment::Center)).padding([6, 0]).into()
    };
    let mut c = column![
        ui::title(format!("Manage {}", m.name), 22),
        row![ui::avatar_key::<Message>(&m.id, &m.name, 48.0), column![text(m.name.clone()).size(15).font(ui::FONT_BOLD), ui::dim(m.role(), 12)].spacing(2)].spacing(12).align_y(Alignment::Center),
        switch_row("Turn off explicit music", "Songs marked explicit stay listed but are skipped for them.".into(), m.hide_explicit, "hideExplicit".into(), true),
        text(format!("What {} can do", m.name)).size(13).font(ui::FONT_BOLD),
    ]
    .spacing(8);
    for (key, title, about) in PERMISSIONS {
        let ok = have(key);
        let sub = if ok { about.to_string() } else { format!("{about} (you don't have this one, so you can't give it)") };
        c = c.push(switch_row(title, sub, m.perms.iter().any(|p| p == key), key.to_string(), ok));
    }
    c = c.push(
        row![
            button(text("Everything (admin)").size(13)).padding([6, 14]).style(ui::secondary).on_press_maybe(me.admin.then_some(Message::ManagePreset(true))),
            button(text("Basics only").size(13)).padding([6, 14]).style(ui::secondary).on_press(Message::ManagePreset(false)),
        ]
        .spacing(8),
    );
    c = c.push(field_dialog("New password", password));
    c = c.push(
        row![
            button(text("Set new password").size(13)).padding([8, 16]).style(ui::secondary).on_press_maybe((!password.is_empty()).then_some(Message::ManagePassword)),
            ui::dim("They're signed out everywhere and use this password next time.", 12),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    );
    if let Some(e) = error {
        c = c.push(container(text(e.as_str()).size(13)).padding([8, 10]).width(Length::Fill).style(ui::error_box));
    }
    c = c.push(
        row![
            button(text(if confirm_remove { format!("Really remove {}? Click again", m.name) } else { format!("Remove {}", m.name) }).size(13).color(ui::ACCENT())).padding([8, 16]).style(ui::secondary).on_press(Message::ManageRemove),
            horizontal_space(),
            button(text("Done").size(14)).padding([8, 18]).style(ui::primary).on_press(Message::DialogCancel),
        ]
        .align_y(Alignment::Center),
    );
    scrollable(c.padding(iced::Padding { right: 14.0, ..Default::default() })).style(ui::scroller).height(Length::Fixed(520.0)).into()
}

fn field_dialog<'a>(label: &'a str, value: &'a str) -> El<'a> {
    column![text(label).size(13).font(ui::FONT_BOLD), text_input("At least 8 characters", value).on_input(|s| Message::DialogText(0, s)).secure(true).padding(10).size(15).style(ui::input)].spacing(6).into()
}

/* ---------- Account: Profile, Members, Settings (three tabs, like the website) ---------- */

pub fn account(app: &App) -> El<'_> {
    let tab = match app.page {
        Page::Members => 1,
        Page::Settings => 2,
        _ => 0,
    };
    let may_members = app.me.as_ref().map(|m| m.user.admin || m.user.perms.iter().any(|p| p == "members")).unwrap_or(false);
    let item = |c: char, label: &'static str, page: Page, on: bool| -> El<'static> {
        button(row![ic(c, 16), text(label).size(14)].spacing(12).align_y(Alignment::Center)).width(Length::Fill).padding([10, 14]).style(ui::nav(on)).on_press(Message::Go(page)).into()
    };
    let mut tabs = column![item(icon::PERSON, "Profile", Page::Profile, tab == 0)].spacing(2);
    if may_members {
        tabs = tabs.push(item(icon::PEOPLE, "Members", Page::Members, tab == 1));
    }
    tabs = tabs.push(item(icon::SETTINGS, "Settings", Page::Settings, tab == 2));
    let body: El = match tab {
        0 => profile(app),
        1 => members(app),
        _ => settings(app),
    };
    // the title and the tabs stay; the content next to them scrolls
    column![
        ui::title(if tab == 2 { "Settings" } else { "Account" }, 32),
        row![
            container(tabs).width(210),
            scrollable(container(body).width(Length::Fill).max_width(660).padding(iced::Padding { right: 14.0, ..Default::default() })).style(ui::scroller).width(Length::Fill).height(Length::Fill),
        ]
        .spacing(32)
        .height(Length::Fill),
    ]
    .spacing(16)
    .height(Length::Fill)
    .into()
}

/// A labelled text box, like the website's form fields.
fn field<'a>(label: &'a str, value: &'a str, n: u8, secret: bool, hint: Option<&'a str>, submit: Option<Message>) -> El<'a> {
    let mut input = text_input("", value).on_input(move |s| Message::ProfileField(n, s)).padding(13).size(15).style(ui::input);
    if secret {
        input = input.secure(true);
    }
    if let Some(m) = submit {
        input = input.on_submit(m);
    }
    let mut c = column![text(label).size(13).font(ui::FONT_BOLD), input].spacing(6);
    if let Some(h) = hint {
        c = c.push(ui::dim(h, 12).color(ui::TEXT_3()));
    }
    container(c).max_width(428).into()
}

fn profile(app: &App) -> El<'_> {
    let user = app.me.as_ref().map(|m| m.user.clone()).unwrap_or_default();
    let name: &str = app.profile_name.as_deref().or(app.me.as_ref().map(|m| m.user.name.as_str())).unwrap_or("");
    let you = column![
        field("Your name", name, 0, false, Some("This is also what you sign in with, on every device."), Some(Message::SaveName)),
        button(text("Save name").size(14)).padding([10, 22]).style(ui::primary).on_press(Message::SaveName),
        row![
            ui::avatar_pic::<Message>(app.avatar_handle(&user.id, user.avatar), "u_me", &user.name, 56.0),
            column![text(user.name.clone()).size(15), ui::dim(if user.admin { "Admin" } else { "Member" }, 12)].spacing(2).width(Length::Fill),
            button(row![ic(icon::PERSON, 13), text("Change picture").size(13)].spacing(8).align_y(Alignment::Center)).padding([7, 16]).style(ui::secondary).on_press(Message::PickPicture),
            button(text("Remove").size(13)).padding([7, 14]).style(ui::flat).on_press_maybe((user.avatar > 0).then_some(Message::RemovePicture)),
        ]
        .spacing(10)
        .spacing(14)
        .align_y(Alignment::Center),
    ]
    .spacing(14);
    let password = column![
        field("Current password", app.pw_current.as_str(), 1, true, None, None),
        field("New password", app.pw_new.as_str(), 2, true, Some("At least 8 characters. Your other devices will need to sign in again."), Some(Message::SavePassword)),
        button(text("Change password").size(14)).padding([10, 22]).style(ui::primary).on_press_maybe((!app.pw_current.is_empty() && !app.pw_new.is_empty()).then_some(Message::SavePassword)),
    ]
    .spacing(14);
    let sign_out = action_row("Sign out on this device", "Your playlists and settings stay on the server.", button(text("Sign out").size(13)).padding([8, 16]).style(ui::secondary).on_press(Message::SignOut).into());
    column![section("You", you.into()), section("Password", password.into()), section("Sign out", sign_out)].spacing(28).into()
}

fn members(app: &App) -> El<'_> {
    let mine = app.me.as_ref().map(|m| m.user.clone()).unwrap_or_default();
    let me = mine.id.clone();
    // you can manage someone unless they're an admin or hold more permissions than you (the server checks too)
    let can_manage = |m: &volvix_core::members::Member| mine.admin || (mine.perms.iter().any(|p| p == "members") && !m.admin && m.perms.iter().all(|p| mine.perms.contains(p)));
    let mut list = Column::new().spacing(10);
    for m in &app.members {
        let you = if m.id == me { " (you)" } else { "" };
        let mut line = row![ui::avatar_pic::<Message>(app.avatar_handle(&m.id, m.avatar), &m.id, &m.name, 36.0), column![text(format!("{}{you}", m.name)).size(15), ui::dim(m.role(), 12)].spacing(2).width(Length::Fixed(300.0))].spacing(12).align_y(Alignment::Center);
        if m.id != me && can_manage(m) {
            line = line.push(button(text("Manage").size(13)).padding([6, 14]).style(ui::secondary).on_press(Message::ManageMember(m.id.clone())));
        }
        list = list.push(line);
    }
    let first = column![
        ui::title("Members of this server", 22),
        ui::dim("Everyone shares the same music. Playlists, play history and settings are personal.", 13),
        container(list).padding([6, 4]),
    ]
    .spacing(6);
    let mut add = column![
        field("Name", app.new_member_name.as_str(), 3, false, Some("They sign in with this name."), None),
        field("Password", app.new_member_pw.as_str(), 4, true, Some("At least 8 characters. They can change it under Account."), Some(Message::AddMember)),
        ui::dim("New members can request music and start parties. Change that any time under Manage.", 13),
    ]
    .spacing(14);
    if let Some(e) = &app.member_error {
        add = add.push(container(text(e.as_str()).size(13)).padding([8, 10]).style(ui::error_box));
    }
    add = add.push(button(row![ic(icon::ADD, 14), text("Add to household").size(14)].spacing(8).align_y(Alignment::Center)).padding([10, 22]).style(ui::primary).on_press(Message::AddMember));
    column![first, section("Add a member", add.into())].spacing(28).into()
}

fn settings(app: &App) -> El<'_> {
    let leveling = app.setting_str("leveling", "off");
    let transcoding = app.me.as_ref().map(|m| m.transcoding).unwrap_or(false);
    let server = app.client.as_ref().map(|c| c.base().to_string()).unwrap_or_default();
    let server_line = match &app.server_status {
        Some(s) => format!("{server} · Volvix Home {} · {} albums, {} songs", s.version, s.albums, s.tracks),
        None => server.clone(),
    };
    let update_line = match &app.update_state {
        None => format!("This is version {}.", env!("CARGO_PKG_VERSION")),
        Some(crate::update::State::Checking) => "Checking…".into(),
        Some(crate::update::State::UpToDate) => "You have the latest version.".into(),
        Some(crate::update::State::Downloading(v)) => format!("Downloading version {v}…"),
        Some(crate::update::State::Ready(v)) => format!("Version {v} is installed. The app restarts to use it."),
        Some(crate::update::State::Failed(m)) => m.clone(),
    };
    let btn = |label: &'static str, m: Option<Message>| -> El<'static> { button(text(label).size(13)).padding([6, 14]).style(ui::secondary).on_press_maybe(m).into() };
    let nothing = || -> El<'static> { iced::widget::Space::new(0, 0).into() };

    let privacy = column![
        setting_row("Share what I'm listening to", "Others in your household see your current song on the Household page.", switch(app.setting_bool("shareActivity", true), Message::SetShare)),
        setting_row("Private session", "Stops sharing, and keeps what you play out of Recently played, until you turn it off.", switch(app.setting_bool("privateSession", false), Message::SetPrivate)),
    ];

    let mut playback = Column::new()
        .push(setting_row(
            "Crossfade",
            "The end of one song fades into the start of the next. Not used in listening parties, and the last song just ends.",
            ui::seg(&[("0", "Off"), ("2", "2 s"), ("4", "4 s"), ("6", "6 s"), ("8", "8 s"), ("12", "12 s")], &format!("{}", app.crossfade_secs() as u32), Message::SetCrossfade),
        ))
        .push(setting_row(
            "Crossfade songs on the same album",
            "Off keeps albums flowing the way they were recorded (live albums, mixes), and only crossfades between different albums.",
            switch(app.setting_bool("crossfadeAlbums", false), Message::SetCrossfadeAlbums),
        ));
    if transcoding {
        playback = playback
            .push(setting_row(
                "Streaming quality (this device)",
                "Smaller files use less data. Original sends the file as it is. Converted songs are made once on the server and kept. Downloads use this too.",
                ui::seg(&[("original", "Original"), ("high", "High"), ("normal", "Normal"), ("low", "Data saver")], &app.settings.stream_quality, Message::SetStreamQuality),
            ))
            .push(setting_row("Use less data on a metered connection", "Switches to Data saver whenever Windows says the connection is metered.", switch(app.settings.data_saver, Message::SetDataSaver)));
    } else {
        playback = playback.push(setting_row("Streaming quality", "Smaller files need ffmpeg on the server (the Docker image includes it). Right now songs stream in original quality.", nothing()));
    }
    playback = playback
        .push(setting_row("Volume leveling", "Uses ReplayGain tags in your files so songs play at the same loudness. Files without the tags aren't changed.", ui::seg(&[("off", "Off"), ("track", "Song"), ("album", "Album")], &leveling, Message::SetLeveling)))
        .push(setting_row(
            "Sleep timer",
            match app.sleep_choice.as_str() {
                "song" => "Pauses when this song ends.",
                "0" => "Pause the music after a while, so you can fall asleep to it.",
                _ => "Pauses when the time is up.",
            },
            ui::seg(&[("0", "Off"), ("15", "15 min"), ("30", "30 min"), ("60", "1 hour"), ("song", "End of song")], &app.sleep_choice, Message::SetSleep),
        ))
        .push(setting_row("Resume where I left off", "Reopening Volvix shows the last song and spot, paused.", switch(app.setting_bool("resumeLast", true), Message::SetResume)));
    let eq = app.eq_now();
    playback = playback.push(setting_row(
        "Equalizer",
        "Shape the sound: raise the sliders to boost a part of the sound and lower them to cut it. It works on this computer while it plays (not when casting to a TV).",
        switch(eq.on, Message::EqOn),
    ));
    if eq.on {
        playback = playback.push(eq_panel(&eq));
    }

    let windows = column![
        setting_row("Start with Windows", "Open in the tray when you sign in to Windows.", switch(app.settings.start_with_windows, Message::SetStartWithWindows)),
        setting_row("Close to the system tray", "The close button hides the window and the music keeps playing.", switch(app.settings.close_to_tray, Message::SetCloseToTray)),
        setting_row("Update automatically", "Look for a new version every few hours.", switch(app.settings.auto_update, Message::SetAutoUpdate)),
        action_row("Check for updates", update_line, btn("Check now", Some(Message::CheckUpdate))),
    ];

    let mut downloads = Column::new().push(setting_row(
        "Only play downloads",
        "Use only the songs saved on this computer. Everything else is skipped. Turn it on when you want to save data.",
        switch(app.setting_bool("offlineOnly", false), Message::SetOfflineOnly),
    ));
    downloads = downloads.push(setting_row(
        "Download on a metered connection",
        "Off: downloads wait while Windows says this connection is metered (a mobile hotspot, or one you marked as metered), so a big album can't use up a data plan.",
        switch(app.settings.download_metered, Message::SetDownloadMetered),
    ));
    if transcoding {
        downloads = downloads.push(setting_row(
            "Download quality",
            "Smaller downloads use less storage and data. \"Same as streaming\" follows the streaming quality above. Songs already downloaded stay as they are.",
            ui::seg(&[("stream", "Same as streaming"), ("original", "Original"), ("high", "High"), ("normal", "Normal"), ("low", "Data saver")], &app.settings.download_quality, Message::SetDownloadQuality),
        ));
    }
    downloads = downloads.push(action_row(
        format!("{} songs on this computer", app.downloaded.len()),
        format!("{} used. Albums and playlists you download play without the server.", human_size(app.downloads.size())),
        btn("Remove all", (!app.saved.is_empty() || !app.downloaded.is_empty()).then_some(Message::RemoveAllDownloads)),
    ));

    let cache = Column::new()
        .push(setting_row(
            "Cache songs as they're queued",
            "Keeps the song playing and the next few on this computer, so they play steadily and replaying them uses no data.",
            switch(app.settings.cache_songs, Message::SetCacheSongs),
        ))
        .push(setting_row(
            "Cache limit",
            "The songs you played longest ago are removed first when the cache is over this.",
            ui::seg(&[("1024", "1 GB"), ("2048", "2 GB"), ("5120", "5 GB"), ("10240", "10 GB")], &app.settings.cache_limit_mb.to_string(), Message::SetCacheLimit),
        ))
        .push(action_row(
            format!("{} song{} cached", app.cache_stats.0, if app.cache_stats.0 == 1 { "" } else { "s" }),
            format!("{} used. Clearing it never removes your downloads.", human_size(app.cache_stats.1)),
            btn("Clear cache", (app.cache_stats.1 > 0).then_some(Message::ClearCache)),
        ));

    let mut library = Column::new();
    if app.can("settings") {
        let line = match &app.folder {
            Some(f) => format!("{}{}{}", f.path, if f.exists { "" } else { " · folder not found" }, if f.is_default { "" } else { " · custom" }),
            None => "…".to_string(),
        };
        library = library.push(action_row("Music folder", line, btn("Change", Some(Message::PickFolder))));
    }
    let counts = match &app.server_status {
        Some(s) => format!("{} albums · {} songs{}. You're told when to reload.", s.albums, s.tracks, if s.scanning { " · checking the folder…" } else { "" }),
        None => "…".to_string(),
    };
    let scanning = app.server_status.as_ref().map(|s| s.scanning).unwrap_or(false);
    library = library.push(action_row("Music on this server", counts, btn("Check now", (app.can("rescan") && !scanning).then_some(Message::Rescan))));

    let theme = column![setting_row(
        "Theme",
        match app.theme_choice().as_str() {
            "light" => "Always light.",
            "dark" => "Always dark.",
            _ => "Matches your device's light or dark setting.",
        },
        ui::seg(&[("system", "System"), ("dark", "Dark"), ("light", "Light")], &app.theme_choice(), Message::SetTheme),
    )];
    let mut page = column![section("Appearance", theme.into()), section("Privacy", privacy.into()), section("Playback", playback.into()), section("Windows app", windows.into())].spacing(24);
    if app.can("share") || app.can("members") {
        let now = volvix_core::userdata::now();
        let mut shares = Column::new().spacing(0);
        match &app.shares {
            Some(list) if !list.is_empty() => {
                for s in list {
                    let count = format!("{} · {} song{} · by {} · {}", s.kind, s.count, if s.count == 1 { "" } else { "s" }, s.by, s.time_left(now));
                    shares = shares.push(
                        container(
                            row![
                                column![text(s.name.clone()).size(15), ui::dim(count, 13)].spacing(2).width(Length::Fill),
                                button(row![ic(icon::LINK, 13), text("Copy link").size(13)].spacing(6).align_y(Alignment::Center)).padding([6, 14]).style(ui::secondary).on_press(Message::ShareCopy(s.path.clone())),
                                button(text("Cancel").size(13)).padding([6, 14]).style(ui::flat).on_press(Message::ShareCancel(s.token.clone())),
                            ]
                            .spacing(8)
                            .align_y(Alignment::Center),
                        )
                        .padding([8, 4]),
                    );
                }
            }
            Some(_) => shares = shares.push(ui::dim("No links are open. Share an album or playlist from its menu.", 13)),
            None => shares = shares.push(ui::dim("Loading…", 13)),
        }
        page = page.push(column![ui::title("Share links", 22), ui::dim("Open links, newest first. Cancelling one stops it working straight away.", 13), container(shares).padding([6, 0])].spacing(6));
    }
    page = page.push(section("Downloads", downloads.into())).push(section("Cache", cache.into())).push(section("Library", library.into()));
    if app.can("settings") {
        page = page.push(lidarr_section(app));
    }
    let version = app.server_status.as_ref().map(|s| format!("Version {}", s.version)).unwrap_or_default();
    let host = server.trim_start_matches("https://").trim_start_matches("http://").to_string();
    let _ = server_line;
    page = page.push(section(
        "About",
        column![setting_row("Volvix Home", version, iced::widget::Space::new(0, 0).into()), action_row("Server", host, btn("Switch server", Some(Message::SwitchServer)))].into(),
    ));
    page.max_width(820).into()
}

/// A labelled drop-down (Lidarr's folder and profiles).
fn select<'a>(label: &'a str, options: Vec<String>, selected: Option<String>, which: u8, hint: Option<&'a str>) -> El<'a> {
    // like a drop-down on the website: nothing chosen yet shows the first one
    let selected = selected.filter(|s| options.contains(s)).or_else(|| options.first().cloned());
    let mut c = column![
        text(label).size(13).font(ui::FONT_BOLD),
        iced::widget::pick_list(options, selected, move |n: String| Message::LidarrPick(which, n)).padding([11, 12]).text_size(15).width(Length::Fill).style(ui::pick_style).menu_style(ui::pick_menu),
    ]
    .spacing(6);
    if let Some(h) = hint {
        c = c.push(ui::dim(h, 12).color(ui::TEXT_3()));
    }
    container(c).max_width(428).into()
}

/// Lidarr (admins): the address and key, where music is saved, and the quality and metadata profiles.
fn lidarr_section(app: &App) -> El<'_> {
    let Some(v) = &app.lidarr else { return section("Lidarr", ui::dim("Loading…", 13).into()) };
    let (s, l) = (&v["settings"], &v["lidarr"]);
    let configured = v["configured"].as_bool().unwrap_or(false);
    let url: &str = app.lidarr_url.as_deref().or(s["url"].as_str()).unwrap_or("");
    let status: El = if configured {
        match v["error"].as_str() {
            Some(e) => container(text(e.to_string()).size(13)).padding([8, 10]).width(Length::Fill).style(ui::error_box).into(),
            None => row![ic(icon::DOWNLOADED, 14), text(format!("Connected to Lidarr {}", l["version"].as_str().unwrap_or(""))).size(14)].spacing(8).align_y(Alignment::Center).into(),
        }
    } else {
        ui::dim("Connect Lidarr to search for music you don't have and add it to this server automatically. Lidarr finds and downloads it with the sources you set up there.", 13).into()
    };
    let key_hint = if s["apiKeySet"].as_bool().unwrap_or(false) { "Saved (leave empty to keep it)" } else { "Lidarr → Settings → General → API Key" };
    let mut c = column![status, field("Lidarr address", url, 5, false, Some("For example http://lidarr:8686"), None), field("API key", app.lidarr_key.as_str(), 6, true, Some(key_hint), Some(Message::LidarrSave))].spacing(14);
    if !l.is_null() {
        let names = |k: &str| -> Vec<String> { l[k].as_array().map(|a| a.iter().filter_map(|p| p["name"].as_str().map(String::from)).collect()).unwrap_or_default() };
        let name_of = |k: &str, id: &serde_json::Value| -> Option<String> { l[k].as_array()?.iter().find(|p| p["id"] == *id)?["name"].as_str().map(String::from) };
        let roots: Vec<String> = l["rootFolders"].as_array().map(|a| a.iter().filter_map(|r| r["path"].as_str().map(String::from)).collect()).unwrap_or_default();
        c = c
            .push(select("Add music to", roots, app.lidarr_root.clone().or(s["rootFolder"].as_str().map(String::from)), 0, Some("Lidarr saves downloads here, and Volvix reads your music folder (Settings → Library). They have to be the same place.")))
            .push(select("Quality profile", names("qualityProfiles"), app.lidarr_quality.clone().or(name_of("qualityProfiles", &s["qualityProfileId"])), 1, None))
            .push(select("Metadata profile", names("metadataProfiles"), app.lidarr_meta.clone().or(name_of("metadataProfiles", &s["metadataProfileId"])), 2, None))
            .push(ui::dim("Who may ask for music is set per member (Members → Manage → Request music).", 12));
    }
    if let Some(e) = &app.lidarr_error {
        c = c.push(container(text(e.as_str()).size(13)).padding([8, 10]).width(Length::Fill).style(ui::error_box));
    }
    let mut actions = Row::new().spacing(10).push(button(text(if configured { "Save" } else { "Connect" }).size(14)).padding([10, 22]).style(ui::primary).on_press(Message::LidarrSave));
    if configured {
        actions = actions.push(button(text("Disconnect").size(14)).padding([10, 18]).style(ui::flat).on_press(Message::LidarrOff));
    }
    c = c.push(actions);
    if configured {
        c = c.push(ui::dim("New downloads appear on their own: Volvix Home watches the music folder, so nothing needs to be set up in Lidarr.", 12));
    }
    column![ui::title("Lidarr", 22), ui::dim("Get music you don't have yet.", 13), container(c).padding([6, 4])].spacing(6).into()
}

/// The equalizer's controls: preset, preamp, reset, and ten band sliders (32 Hz on the left, 16 kHz on the right).
fn eq_panel(eq: &crate::eq::EqSettings) -> El<'static> {
    let names: Vec<&'static str> = crate::eq::PRESETS.iter().map(|p| p.0).collect();
    let selected = eq.preset_name();
    let sign = |v: f32| if v > 0.0 { format!("+{v:.0}") } else { format!("{v:.0}") };
    let top = row![
        column![
            ui::dim("Preset", 12),
            iced::widget::pick_list(names, selected, |n: &'static str| Message::EqPreset(n.to_string())).placeholder("Custom").padding([8, 12]).text_size(14).width(240).style(ui::pick_style).menu_style(ui::pick_menu),
        ]
        .spacing(4),
        column![
            ui::dim(format!("Preamp {} dB", sign(eq.pre_db)), 12),
            container(iced::widget::slider(-12.0..=6.0, eq.pre_db, Message::EqPre).step(1.0_f32).style(ui::vslider_style)).width(180),
        ]
        .spacing(10),
        button(text("Reset").size(13)).padding([6, 14]).style(ui::flat).on_press(Message::EqReset),
    ]
    .spacing(24)
    .align_y(Alignment::End);
    let mut bands = Row::new().spacing(4).align_y(Alignment::Center);
    for (i, f) in crate::eq::BANDS.iter().enumerate() {
        let label = if *f >= 1000.0 { format!("{}k", *f / 1000.0) } else { format!("{f}") };
        bands = bands.push(
            column![
                ui::dim(sign(eq.gains[i]), 11),
                iced::widget::vertical_slider(-12.0..=12.0, eq.gains[i], move |v| Message::EqBand(i, v)).step(1.0).height(150).style(ui::vslider_style),
                ui::dim(label, 11),
            ]
            .spacing(6)
            .width(Length::Fill)
            .align_x(Alignment::Center),
        );
    }
    container(column![top, bands].spacing(16)).padding([8, 4]).into()
}

/// Pick the server's music folder: the folders in the one you're in, and "Use this folder".
fn folder_dialog<'a>(browse: &'a Option<volvix_core::admin::Browse>, error: &'a Option<String>) -> El<'a> {
    let mut c = column![ui::title("Music folder", 22), ui::dim("Choose the folder on the server where your music is.", 13)].spacing(8);
    match browse {
        None => c = c.push(ui::dim("Loading…", 13)),
        Some(b) => {
            if b.denied {
                c = c.push(container(text("Volvix isn't allowed to read this folder. The user the server runs as needs read access to it.").size(13)).padding([8, 10]).width(Length::Fill).style(ui::error_box));
            }
            if b.missing {
                c = c.push(ui::dim("That folder doesn't exist on the server, so this is the closest one that does.", 12));
            }
            if b.docker {
                c = c.push(ui::dim("Volvix is running in Docker, so it only sees folders you mounted into the container.", 12));
            }
            c = c.push(row![ic(icon::LIBRARY, 14), text(b.path.clone()).size(13)].spacing(8).align_y(Alignment::Center));
            c = c.push(ui::dim(if b.songs_here > 0 { format!("{} song{} directly in this folder", b.songs_here, if b.songs_here == 1 { "" } else { "s" }) } else { "No songs directly in this folder (subfolders count too)".to_string() }, 12));
            let mut list = Column::new().spacing(2);
            if let Some(p) = &b.parent {
                list = list.push(button(row![ic(icon::BACK, 13), text("Up one level").size(14)].spacing(10).align_y(Alignment::Center)).width(Length::Fill).padding([8, 10]).style(ui::flat).on_press(Message::BrowseTo(p.clone())));
            }
            for d in &b.dirs {
                list = list.push(button(row![ic(icon::LIBRARY, 13), text(d.name.clone()).size(14)].spacing(10).align_y(Alignment::Center)).width(Length::Fill).padding([8, 10]).style(ui::flat).on_press(Message::BrowseTo(d.path.clone())));
            }
            if b.dirs.is_empty() {
                list = list.push(ui::dim("No folders in here.", 12));
            }
            c = c.push(scrollable(list).style(ui::scroller).height(Length::Fixed(260.0)));
        }
    }
    if let Some(e) = error {
        c = c.push(container(text(e.as_str()).size(13)).padding([8, 10]).width(Length::Fill).style(ui::error_box));
    }
    c.push(
        row![
            horizontal_space(),
            button(text("Cancel").size(14)).padding([8, 18]).style(ui::secondary).on_press(Message::DialogCancel),
            button(text("Use this folder").size(14)).padding([8, 18]).style(ui::primary).on_press_maybe(browse.as_ref().map(|_| Message::ChooseFolder)),
        ]
        .spacing(10),
    )
    .into()
}

/* ---------- dialogs ---------- */

pub fn dialog<'a>(app: &'a App, d: &'a Dialog) -> El<'a> {
    let buttons = |ok: &'a str, enabled: bool| -> El<'a> {
        row![
            horizontal_space(),
            button(text("Cancel").size(14)).padding([8, 18]).style(ui::secondary).on_press(Message::DialogCancel),
            button(text(ok).size(14)).padding([8, 18]).style(ui::primary).on_press_maybe(enabled.then_some(Message::DialogOk)),
        ]
        .spacing(10)
        .into()
    };
    let field = |placeholder: &'a str, value: &'a str, n: u8| text_input(placeholder, value).on_input(move |s| Message::DialogText(n, s)).on_submit(Message::DialogOk).padding(10).size(15).style(ui::input);
    let err = |e: &'a Option<String>| -> Option<El<'a>> { e.as_ref().map(|e| container(text(e.as_str()).size(13)).padding([8, 10]).width(Length::Fill).style(ui::error_box).into()) };
    match d {
        Dialog::NewPlaylist { name, description: _, add, collaborative, public } => {
            let title = if add.is_empty() { "New playlist".to_string() } else { format!("New playlist with {} song{}", add.len(), if add.len() == 1 { "" } else { "s" }) };
            column![
                ui::title(title, 22),
                labelled("Name", field("My playlist", name, 0).into()),
                labelled("Description", desc_box(app)),
                switch_card("Collaborative", "People you invite can add, remove and reorder songs.", *collaborative, |v| Message::DialogFlag(0, v)),
                switch_card("Visible to the household", "Everyone here can see it on the Household page.", *public, |v| Message::DialogFlag(1, v)),
                buttons("Create", true),
            ]
            .spacing(14)
            .into()
        }
        Dialog::EditPlaylist { name, description: _, public, .. } => column![
            ui::title("Edit details", 22),
            labelled("Name", field("", name, 0).into()),
            labelled("Description", desc_box(app)),
            switch_card("Visible to the household", "Everyone here can see it on the Household page.", *public, |v| Message::DialogFlag(0, v)),
            buttons("Save", true),
        ]
        .spacing(14)
        .into(),
        Dialog::DeletePlaylist { name, .. } => column![ui::title("Delete playlist?", 22), ui::dim(format!("This deletes \"{name}\". Your music stays in your library."), 15), buttons("Delete", true)].spacing(14).into(),
        Dialog::AddTo { tracks } => {
            let mut list = Column::new().spacing(2);
            list = list.push(
                button(row![container(ic(icon::ADD, 18)).width(44).height(44).center_x(44).center_y(44).style(|_| container::Style { background: Some(ui::SURFACE_3().into()), border: iced::border::rounded(6.0), ..Default::default() }), text("New playlist").size(15)].spacing(12).align_y(Alignment::Center))
                    .width(Length::Fill)
                    .padding(6)
                    .style(ui::flat)
                    .on_press(Message::NewPlaylist(tracks.clone())),
            );
            for p in app.playlists().into_iter().filter(|p| p.can_edit()) {
                let all_in = tracks.iter().all(|t| p.tracks.contains(t));
                let sub = format!("{}{} song{}", if p.is_shared() { "Collaborative · " } else { "" }, p.tracks.len(), if p.tracks.len() == 1 { "" } else { "s" });
                let mut r = row![playlist_art(app, &p, 44.0), column![text(clip(&p.name, 40)).size(15), ui::dim(sub, 12)].spacing(2).width(Length::Fill)].spacing(12).align_y(Alignment::Center);
                if all_in {
                    r = r.push(row![ui::wi("check", 14.0, ui::TEXT_2()), ui::dim("Added", 12)].spacing(4).align_y(Alignment::Center));
                }
                list = list.push(button(r).width(Length::Fill).padding(6).style(ui::flat).on_press_maybe((!all_in).then(|| Message::AddToChosen(p.id.clone()))));
            }
            let title = if tracks.len() == 1 { "Add to playlist".to_string() } else { format!("Add {} songs to a playlist", tracks.len()) };
            column![ui::title(title, 22), scrollable(list).style(ui::scroller).height(Length::Shrink).height(380), row![horizontal_space(), button(text("Cancel").size(14)).padding([8, 18]).style(ui::secondary).on_press(Message::DialogCancel)]].spacing(14).into()
        }
        Dialog::Password { current, new, error, busy } => {
            let mut c = column![
                ui::title("Change password", 22),
                text_input("Current password", current).secure(true).on_input(|s| Message::DialogText(0, s)).padding(10).size(15).style(ui::input),
                text_input("New password", new).secure(true).on_input(|s| Message::DialogText(1, s)).on_submit(Message::DialogOk).padding(10).size(15).style(ui::input),
            ]
            .spacing(14);
            if let Some(e) = err(error) {
                c = c.push(e);
            }
            c.push(buttons(if *busy { "Saving…" } else { "Change" }, !busy && !current.is_empty() && !new.is_empty())).into()
        }
        Dialog::Rename { name, error } => {
            let mut c = column![ui::title("Your name", 22), field("Name", name, 0)].spacing(14);
            if let Some(e) = err(error) {
                c = c.push(e);
            }
            c.push(buttons("Save", !name.trim().is_empty())).into()
        }
        Dialog::PlayOn => play_on(app),
        Dialog::Credits { track } => {
            let close = row![horizontal_space(), ui::btn("Close", ui::Btn::Secondary).on_press(Message::DialogCancel)];
            match app.lib.track(track) {
                Some(t) => column![scrollable(credits(app, t)).style(ui::scroller).height(Length::Fixed(420.0)), close].spacing(14).into(),
                None => column![ui::dim("That song isn't here any more.", 14), close].spacing(14).into(),
            }
        }
        Dialog::CoverLook { name, look, .. } => {
            const BGS: [(&str, &str); 16] = [("sunset", "Sunset"), ("ocean", "Ocean"), ("midnight", "Midnight"), ("forest", "Forest"), ("grape", "Grape"), ("rose", "Rose"), ("ember", "Ember"), ("mono", "Graphite"), ("paper", "Paper"), ("stripes", "Stripes"), ("rings", "Rings"), ("grid", "Grid"), ("sunburst", "Sunburst"), ("checker", "Checker"), ("aurora", "Aurora"), ("dusk", "Dusk")];
            const FONTS: [(&str, &str); 7] = [("display", "Modern"), ("serif", "Serif"), ("mono", "Typewriter"), ("rounded", "Rounded"), ("condensed", "Poster"), ("script", "Script"), ("slab", "Slab")];
            const PALETTE: [&str; 12] = ["#ff6a3d", "#ffd23f", "#8fc98a", "#1b998b", "#7fd1e8", "#3282b8", "#3c1053", "#c99bff", "#ff6f91", "#e5484d", "#ffffff", "#111111"];
            let mut swatches = Row::new().spacing(6);
            let mut sw_rows = Column::new().spacing(6);
            for (i, (k, label)) in BGS.iter().enumerate() {
                let on = look.bg == *k;
                // each background is drawn in the colours picked below, so you see what they do to it
                let shown = volvix_core::Look { bg: k.to_string(), font: "display".into(), c1: look.c1.clone().filter(|c| ui::hex(c).is_some()), c2: look.c2.clone().filter(|c| ui::hex(c).is_some()), ink: None };
                let b = button(ui::cover_look::<Message>("", 34.0, 5.0, &shown)).padding(2).style(move |t, st| {
                    let mut x = ui::flat(t, st);
                    x.border = iced::Border { color: if on { ui::ACCENT() } else { Color::TRANSPARENT }, width: 2.0, radius: 8.0.into() };
                    x
                }).on_press(Message::CoverBg(k.to_string()));
                swatches = swatches.push(ui::tip(b, label));
                if i % 6 == 5 {
                    sw_rows = sw_rows.push(std::mem::replace(&mut swatches, Row::new().spacing(6)));
                }
            }
            sw_rows = sw_rows.push(swatches);
            let mut fonts = Row::new().spacing(6);
            for (k, label) in FONTS {
                let on = look.font == k;
                let (f, upper) = ui::look_font(k);
                let text_label = if upper { label.to_uppercase() } else { label.to_string() };
                fonts = fonts.push(button(text(text_label).font(f).size(15).wrapping(text::Wrapping::None)).padding([7, 10]).style(move |t, st| {
                    let mut x = ui::secondary(t, st);
                    x.border = iced::Border { color: if on { ui::ACCENT() } else { Color::TRANSPARENT }, width: 2.0, radius: 10.0.into() };
                    x
                }).on_press(Message::CoverFont(k.to_string())));
            }
            let (d1, d2, d3) = ui::look_defaults(&look.bg);
            let colour_row = |label: &'static str, which: u8, cur: &Option<String>, def: Color| -> El<'a> {
                let mut pal = Row::new().spacing(4).align_y(Alignment::Center);
                for hexc in PALETTE {
                    let col = ui::hex(hexc).unwrap_or(Color::WHITE);
                    let picked = cur.as_deref().map(|c| c.eq_ignore_ascii_case(hexc)).unwrap_or(false);
                    pal = pal.push(
                        button(container(iced::widget::Space::new(18, 18)).style(move |_: &iced::Theme| container::Style { background: Some(col.into()), border: iced::Border { color: ui::BORDER_STRONG(), width: 1.0, radius: 9.0.into() }, ..Default::default() }))
                            .padding(2)
                            .style(move |t, st| {
                                let mut x = ui::flat(t, st);
                                x.border = iced::Border { color: if picked { ui::ACCENT() } else { Color::TRANSPARENT }, width: 2.0, radius: 12.0.into() };
                                x
                            })
                            .on_press(Message::CoverColour(which, hexc.to_string())),
                    );
                }
                let shown = cur.clone().unwrap_or_default();
                row![
                    container(text(label).size(13)).width(76),
                    pal,
                    text_input(&ui::to_hex(def), &shown).on_input(move |v| Message::CoverColour(which, v)).padding(6).size(13).width(84).style(ui::input),
                ]
                .spacing(10)
                .align_y(Alignment::Center)
                .into()
            };
            column![
                ui::title("Customize cover", 22),
                row![
                    ui::cover_look::<Message>(name, 170.0, 6.0, look),
                    column![text("Background").size(13).font(ui::FONT_BOLD), sw_rows, text("Font").size(13).font(ui::FONT_BOLD), Row::with_children(vec![fonts.wrap().into()])].spacing(8).width(Length::Fill),
                ]
                .spacing(18),
                text("Colours").size(13).font(ui::FONT_BOLD),
                colour_row("Primary", 0, &look.c1, d1),
                colour_row("Secondary", 1, &look.c2, d2),
                colour_row("Text", 2, &look.ink, d3),
                row![
                    ui::btn("Use the automatic look", ui::Btn::Secondary).on_press(Message::CoverReset),
                    ui::btn("Reset colours", ui::Btn::Ghost).on_press(Message::CoverResetColours),
                    horizontal_space(),
                    ui::btn("Cancel", ui::Btn::Secondary).on_press(Message::DialogCancel),
                    ui::btn("Save", ui::Btn::Primary).on_press(Message::DialogOk)
                ]
                .spacing(10),
            ]
            .spacing(12)
            .into()
        }
        Dialog::Crop(c) => {
            let view = iced::widget::mouse_area(image(c.preview.clone()).width(crate::CROP_VIEW).height(crate::CROP_VIEW))
                .on_press(Message::CropDown)
                .on_release(Message::CropUp)
                .on_exit(Message::CropUp)
                .on_move(Message::CropMove)
                .interaction(iced::mouse::Interaction::Grab);
            column![
                ui::title("Adjust the picture", 22),
                container(view).width(Length::Fill).center_x(Length::Fill),
                row![ui::wi("minus", 14.0, ui::TEXT_2()), iced::widget::slider(1.0..=4.0, c.zoom, Message::CropZoom).step(0.01_f32), ui::wi("plus", 14.0, ui::TEXT_2())].spacing(10).align_y(Alignment::Center),
                ui::dim("Drag to move it. Use the slider to zoom.", 13),
                row![horizontal_space(), ui::btn("Cancel", ui::Btn::Secondary).on_press(Message::DialogCancel), ui::btn("Use this picture", ui::Btn::Primary).on_press(Message::DialogOk)].spacing(10),
            ]
            .spacing(12)
            .into()
        }
        Dialog::AlbumInfo { album } => {
            let close = row![horizontal_space(), ui::btn("Close", ui::Btn::Secondary).on_press(Message::DialogCancel)];
            let Some(a) = app.lib.album(album) else { return column![ui::dim("That album isn't here any more.", 14), close].spacing(14).into() };
            let tracks = app.lib.album_tracks(album);
            let q = &a.quality;
            let quality = if q.lossless {
                let hi = q.bits.unwrap_or(16) >= 24 || q.khz.unwrap_or(44.1) > 48.0;
                let detail = [q.bits.map(|b| format!("{b}-bit")), q.khz.map(|k| format!("{k} kHz")), Some(q.codec.clone()).filter(|c| !c.is_empty())].into_iter().flatten().collect::<Vec<_>>().join(" / ");
                format!("{}{}", if hi { "Hi-Res Lossless" } else { "Lossless" }, if detail.is_empty() { String::new() } else { format!(" ({detail})") })
            } else {
                let k = q.kbps.unwrap_or(0);
                format!("{} ({} {})", if k >= 256 { "High Quality" } else if k >= 128 { "Normal" } else { "Low" }, if q.codec.is_empty() { "Audio" } else { q.codec.as_str() }, if k > 0 { format!("{k} kbps") } else { String::new() })
            };
            let secs: f64 = tracks.iter().map(|t| t.duration as f64).sum();
            let mut rows: Vec<(&str, String)> = vec![];
            if let Some(y) = a.year { rows.push(("Released", y.to_string())); }
            if let Some(l) = &a.label { rows.push(("Label", l.clone())); }
            if let Some(n) = &a.catalog_number { rows.push(("Catalog number", n.clone())); }
            if let Some(g) = &a.genre { rows.push(("Genre", g.clone())); }
            rows.push(("Songs", format!("{} · {} min", tracks.len(), (secs / 60.0).round() as u64)));
            rows.push(("Quality", quality));
            if let Some(c) = &a.copyright { rows.push(("Copyright", c.clone())); }
            let line = |k: &str, v: String| -> El<'_> { row![container(ui::dim(k.to_string(), 13)).width(130), text(v).size(14).width(Length::Fill)].spacing(8).into() };
            let mut body = column![row![cover_of(app, Some(a), 56.0), column![text(a.title.clone()).size(16).font(ui::FONT_BOLD), ui::dim(a.artist.clone(), 13)].spacing(2)].spacing(12).align_y(Alignment::Center)].spacing(10);
            for (k, v) in rows {
                body = body.push(line(k, v));
            }
            let mut roles: Vec<(String, Vec<String>)> = vec![];
            for t in &tracks {
                for c in &t.credits {
                    if c.role == "ISRC" { continue; }
                    let i = match roles.iter().position(|(r, _)| *r == c.role) { Some(i) => i, None => { roles.push((c.role.clone(), vec![])); roles.len() - 1 } };
                    for n in c.names.split(", ") {
                        if !roles[i].1.iter().any(|x| x.eq_ignore_ascii_case(n)) { roles[i].1.push(n.to_string()); }
                    }
                }
            }
            if roles.is_empty() {
                body = body.push(ui::dim("No credits are tagged in these files. Open a song's menu (the three dots) and choose Credits to look it up on MusicBrainz.", 13));
            } else {
                body = body.push(ui::dim("Credits on this album", 13));
                for (r, n) in roles {
                    body = body.push(line(&r, n.join(", ")));
                }
            }
            column![ui::title("Album info", 22), container(scrollable(body).style(ui::scroller)).max_height(380), close].spacing(14).into()
        }
        Dialog::Share { kind, name, tracks, days, link, error, busy } => {
            let what = if kind == "album" { "this album" } else { "this playlist" };
            let mut c = column![
                ui::title(format!("Share {what}"), 22),
                ui::dim(format!("Anyone with the link can play {name} ({} song{}) in their browser. They can't see anything else on your server, and you can cancel the link any time in Settings.", tracks.len(), if tracks.len() == 1 { "" } else { "s" }), 14),
            ]
            .spacing(12);
            let day = days.to_string();
            match link {
                None => {
                    c = c.push(ui::seg(&[("1", "1 day"), ("7", "7 days"), ("30", "30 days")], &day, |v| Message::ShareDays(v.parse().unwrap_or(7))));
                    if let Some(e) = error {
                        c = c.push(container(text(e.as_str()).size(13)).padding([8, 10]).width(Length::Fill).style(ui::error_box));
                    }
                    c = c.push(row![horizontal_space(), ui::btn("Close", ui::Btn::Secondary).on_press(Message::DialogCancel), ui::btn("Create link", ui::Btn::Primary).on_press_maybe((!*busy).then_some(Message::ShareCreate))].spacing(10));
                }
                Some((url, left)) => {
                    let path = url.find("/s/").or(url.find("/share")).map(|i| url[i..].to_string()).unwrap_or_default();
                    c = c
                        .push(row![text_input("", url).padding(10).size(14).style(ui::input), ui::btn_icon("copy", "Copy", ui::Btn::Secondary, true).on_press(Message::ShareCopy(path))].spacing(8).align_y(Alignment::Center))
                        .push(ui::dim(format!("Works for {}.{}", left.trim_end_matches(" left"), if url.contains("//localhost") || url.contains("//127.") { " This address only works on this computer: open Volvix Home by your server's network or internet address to share with others." } else { "" }), 12))
                        .push(row![horizontal_space(), ui::btn("Close", ui::Btn::Secondary).on_press(Message::DialogCancel)]);
                }
            }
            c.into()
        }
        Dialog::Collab { id, picked } => {
            let name = app.playlist(id).map(|p| p.name).unwrap_or_default();
            let p = app.playlist(id);
            let mut c = column![ui::title(format!("Who can edit \"{name}\"?"), 22)].spacing(12);
            let others = app.others();
            if others.is_empty() {
                c = c.push(ui::dim("No one else is in your household yet. An admin can add people under Account → Members.", 14));
                c.push(row![horizontal_space(), ui::btn("Done", ui::Btn::Primary).on_press(Message::DialogCancel)]).into()
            } else {
                let shared = p.as_ref().map(|p| p.is_shared()).unwrap_or(false);
                c = c.push(ui::dim(if shared { "People you pick get an invite. Once they accept they can add, remove and reorder songs." } else { "Tick someone to make this a collaborative playlist. They get an invite, and once they accept they can add, remove and reorder songs." }, 14));
                for (uid, uname) in others {
                    let state = match &p {
                        Some(p) if p.collaborators.contains(&uid) => "Can edit",
                        Some(p) if p.invited.contains(&uid) => "Invited, waiting to accept",
                        _ => "Not invited",
                    };
                    let on = picked.contains(&uid);
                    let u2 = uid.clone();
                    c = c.push(
                        row![ui::avatar_key::<Message>(&uid, &uname, 40.0), column![text(uname.clone()).size(15), ui::dim(state, 13)].spacing(1).width(Length::Fill), checkbox("", on).size(18).on_toggle(move |v| Message::PartyPick(u2.clone(), v))]
                            .spacing(12)
                            .align_y(Alignment::Center),
                    );
                }
                let shared = p.as_ref().map(|p| p.is_shared()).unwrap_or(false);
                let mut foot = Row::new().spacing(10).align_y(Alignment::Center);
                let anyone = p.as_ref().map(|p| !p.collaborators.is_empty() || !p.invited.is_empty()).unwrap_or(false);
                if shared && anyone {
                    // one place for all of it: taking everyone off again is here too (only when someone is on it)
                    foot = foot.push(ui::tip(ui::btn("Stop sharing", ui::Btn::Danger).on_press(Message::Unshare(id.clone())), "It becomes just yours again. Everyone you invited loses access."));
                }
                foot = foot.push(horizontal_space()).push(ui::btn("Cancel", ui::Btn::Secondary).on_press(Message::DialogCancel)).push(ui::btn("Save", ui::Btn::Primary).on_press(Message::DialogOk));
                c.push(foot).into()
            }
        }
        Dialog::Folder { browse, error } => folder_dialog(browse, error),
        Dialog::Manage { id, password, error, confirm_remove } => manage_dialog(app, id, password, error, *confirm_remove),
        Dialog::AddCast { host, error, busy } => {
            let mut c = column![
                ui::title("Add a TV or speaker", 22),
                ui::dim("Type its address on your network. You can find it in the Google Home app: the device → Settings → Device information.", 13),
                field("192.168.1.40", host, 0),
            ]
            .spacing(14);
            if let Some(e) = err(error) {
                c = c.push(e);
            }
            c.push(buttons(if *busy { "Adding…" } else { "Add" }, !busy && !host.trim().is_empty())).into()
        }
        Dialog::PartyStart { invite } => {
            let has = app.status.track.is_some();
            let intro = if has { "What's playing now becomes the party's queue. " } else { "Nothing is playing yet, so you'll start with an empty queue. " };
            column![
                ui::title("Start a listening party", 22),
                ui::dim(format!("{intro}Everyone you invite can add, remove and skip songs, and you all stay in sync."), 13),
                people_list(app, invite, &[]),
                buttons("Start party", true),
            ]
            .spacing(14)
            .into()
        }
        Dialog::PartyInvite { invite } => {
            let skip: Vec<String> = app.party.as_ref().map(|p| p.members.iter().map(|m| m.id.clone()).chain(p.invited.iter().cloned()).collect()).unwrap_or_default();
            column![ui::title("Invite to the party", 22), people_list(app, invite, &skip), buttons("Send invites", !invite.is_empty())].spacing(14).into()
        }
    }
}

/// Household members to tick (`skip` = already in or invited).
fn people_list<'a>(app: &'a App, picked: &'a [String], skip: &[String]) -> El<'a> {
    let others = app.others();
    if others.is_empty() {
        return ui::dim("No one else is in your household yet. You can still start a party and invite people later.", 13).into();
    }
    let mut list = Column::new().spacing(6);
    for (id, name) in others {
        let here = skip.contains(&id);
        let on = here || picked.contains(&id);
        let label = if here { format!("{name} · already invited or in the party") } else { name };
        let id2 = id.clone();
        let mut cb = checkbox(label, on).size(18).text_size(14);
        if !here {
            cb = cb.on_toggle(move |v| Message::PartyPick(id2.clone(), v));
        }
        list = list.push(cb);
    }
    scrollable(list).style(ui::scroller).height(Length::Shrink).into()
}

/// The cast button's window: where the sound plays, and the listening party.
fn play_on(app: &App) -> El<'_> {
    let line = || -> El<'_> { container(iced::widget::Space::new(Length::Fill, 1)).width(Length::Fill).height(1).style(|_: &iced::Theme| container::Style { background: Some(ui::BORDER().into()), ..Default::default() }).into() };
    let item = |icon_name: &'static str, label: String, sub: Option<String>, on: bool, msg: Option<Message>, danger: bool| -> El<'_> {
        let mut t = column![text(label).size(14).font(ui::FONT_BOLD).color(if danger { ui::ACCENT() } else { ui::TEXT() })].spacing(2);
        if let Some(s) = sub {
            t = t.push(ui::dim(s, 12));
        }
        button(row![ui::wi(if on { "check" } else { icon_name }, 18.0, if danger { ui::ACCENT() } else { ui::TEXT_2() }), t.width(Length::Fill)].spacing(12).align_y(Alignment::Center))
            .width(Length::Fill)
            .padding([9, 12])
            .style(ui::flat)
            .on_press_maybe(msg)
            .into()
    };
    let mut c = column![container(column![text("Play on").size(14).font(ui::FONT_BOLD), ui::dim("One device plays at a time", 12)].spacing(2)).padding([8, 12]), line()].spacing(4);
    let mut list = Column::new().spacing(2);
    let casting = app.cast.as_ref();
    let here_active = app.dev_active.as_deref().map(|a| a == app.dev_id).unwrap_or(true);
    let this = if casting.is_some() { Some(Message::CastStop) } else if !here_active { Some(Message::DeviceHere) } else { None };
    list = list.push(item("devices", "This computer".into(), None, casting.is_none() && here_active, this, false));
    // your other devices where Volvix Home is open (the website, a phone): move what plays over to one
    for (id, name, active) in app.connect_devices.iter().filter(|d| d.0 != app.dev_id) {
        list = list.push(item("devices", format!("{name}{}", if *active { " · playing" } else { "" }), None, *active, Some(Message::DeviceTransfer(id.clone())), false));
    }
    list = list.push(line());
    match casting {
        Some(l) => {
            list = list.push(item("devices", format!("Casting to {}", l.name), None, true, None, false));
            list = list.push(item("x", "Stop casting".into(), None, false, Some(Message::CastStop), false));
        }
        None => {
            for (i, d) in app.cast_devices.iter().enumerate() {
                list = list.push(item("devices", d.name.clone(), (!d.model.is_empty()).then(|| d.model.clone()), false, Some(Message::CastStart(i)), false));
            }
            if app.cast_devices.is_empty() {
                list = list.push(ui::dim("No TVs or speakers found yet", 13));
            }
            list = list.push(item("plus", "Add a TV or speaker by address…".into(), None, false, Some(Message::CastAddOpen), false));
        }
    }
    // the listening party lives here too: start it, invite people, leave or end it
    list = list.push(line());
    match &app.party {
        None => list = list.push(item("users", "Start a listening party…".into(), None, false, Some(Message::PartyStartOpen), false)),
        Some(p) => {
            let me = app.my_id();
            let host = p.host == me;
            let names: Vec<String> = p.members.iter().map(|m| if m.id == me { "You".to_string() } else { app.person_name(&m.id) }).collect();
            let who = if host { "You're the host".to_string() } else { format!("{} is the host", app.person_name(&p.host)) };
            list = list.push(item("users", format!("Listening party ({} here)", p.members.len()), Some(format!("{} · {who}", names.join(", "))), true, None, false));
            list = list.push(item("plus", "Invite people".into(), None, false, Some(Message::PartyInviteOpen), false));
            list = list.push(item("x", if host { "End party for everyone".into() } else { "Leave party".into() }, None, false, Some(Message::PartyLeave), true));
        }
    }
    c = c.push(scrollable(list).style(ui::scroller).height(Length::Shrink));
    c.into()
}
