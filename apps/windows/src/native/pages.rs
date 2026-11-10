//! Playlists, mixes, lyrics, Get more music, household, notifications, downloads, settings and the
//! dialogs.
use iced::widget::{button, checkbox, column, container, horizontal_space, image, row, scrollable, text, text_input, toggler, Column, Row};
use iced::{Alignment, Length};
use volvix_core::mixes::Mix;
use volvix_core::playlists::Kind;
use volvix_core::{FoundItem, Track};

use crate::ui::{self, icon, icon as ic};
use crate::views::{card, clip, cover_of, download_button, header_buttons, playlist_card, playlist_cover, round_icon, section, track_list, El, Rows};
use crate::{lyrics_scroller, App, Dialog, Message, Page, LYRIC_LINE};

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
    if p.can_manage() {
        extra.push(round_icon(icon::EDIT, false, "Edit details", Some(Message::EditPlaylist(p.id.clone()))));
        extra.push(round_icon(icon::DELETE, false, "Delete playlist", Some(Message::DeletePlaylist(p.id.clone()))));
    }
    if let Kind::Shared { mine: false } = p.kind {
        extra.push(round_icon(icon::CLOSE, false, "Leave this playlist", Some(Message::SharedOp(p.id.clone(), serde_json::json!({ "op": "leave" })))));
    }
    info = info.push(ui::gap(10)).push(header_buttons(Message::PlayTracks(ids.clone(), 0, p.name.clone()), Message::ShuffleTracks(ids, p.name.clone()), extra));
    let head = row![playlist_cover(app, &p.tracks, p.kind == Kind::Liked, &p.name, 220.0), info].spacing(28).align_y(Alignment::End);
    let body: El = if tracks.is_empty() {
        ui::dim(if p.kind == Kind::Liked { "Songs you like (the heart) show up here." } else { "No songs yet. Use the playlist button on any song to add it here." }, 15).into()
    } else {
        track_list(app, &tracks, &p.name, false, true, if p.can_edit() { Rows::Playlist(id) } else { Rows::Plain })
    };
    column![head, body].spacing(28).into()
}

pub fn mix_card(m: &Mix) -> El<'_> {
    let art: El = container(column![ic(mix_icon(m.icon), 30), text(clip(&m.title, 18)).size(16).font(ui::FONT_BOLD)].spacing(10).align_x(Alignment::Center))
        .width(164)
        .height(164)
        .center_x(164)
        .center_y(164)
        .style(ui::mix_tile(&m.id))
        .into();
    card(art, m.title.clone(), m.sub.clone(), Message::Go(Page::Mix(m.id.clone())))
}

fn mix_icon(name: &str) -> char {
    match name {
        "repeat" => icon::REPEAT_ALL,
        "heart" => icon::HEART_FILL,
        "history" => '\u{E81C}',
        "user" => icon::PEOPLE,
        "plus" => icon::ADD,
        "search" => icon::SEARCH,
        _ => '\u{E8D6}',
    }
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
    let art = container(ic(mix_icon(m.icon), 64)).width(220).height(220).center_x(220).center_y(220).style(ui::mix_tile(&m.id));
    column![row![art, info].spacing(28).align_y(Alignment::End), track_list(app, &tracks, &m.title, false, true, Rows::Plain)].spacing(28).into()
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
    } else {
        format!("{mb:.0} MB")
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
        return scrollable(text(ly.plain.clone()).size(20).color(ui::TEXT_2).line_height(1.5)).style(ui::scroller).height(Length::Fill).into();
    }
    let now = ly.line_at(app.status.position.as_millis() as i64);
    let mut col = Column::new();
    for (i, l) in ly.synced.iter().enumerate() {
        let color = match now {
            Some(n) if n == i => ui::TEXT,
            Some(n) if i < n => ui::TEXT_3,
            _ => ui::TEXT_2,
        };
        let words = if l.text.is_empty() { "♪".to_string() } else { l.text.clone() };
        let size = if now == Some(i) { 26.0 } else { 22.0 };
        col = col.push(
            button(text(clip(&words, 70)).size(size).font(ui::FONT_BOLD).color(color))
                .height(LYRIC_LINE)
                .padding([6, 0])
                .style(ui::flat)
                .on_press(Message::SeekTo(l.ms.max(0) as f32 / 1000.0)),
        );
    }
    // room below the last line, so it can scroll up to the middle
    col = col.push(ui::gap(240));
    scrollable(col).id(lyrics_scroller()).style(ui::scroller).height(Length::Fill).into()
}

/* ---------- get more music ---------- */

pub fn get_music(app: &App) -> El<'_> {
    let f = &app.find;
    let can = app.me.as_ref().map(|m| m.lidarr.can_request).unwrap_or(false);
    let input = text_input("An artist, album or song you don't have", &f.term)
        .on_input(Message::FindChanged)
        .on_submit(Message::FindSubmit)
        .padding(12)
        .size(16)
        .icon(text_input::Icon { font: ui::ICONS, code_point: icon::GLOBE, size: Some(16.into()), spacing: 10.0, side: text_input::Side::Left })
        .style(ui::input);
    let mut c = column![
        ui::title("Get more music", 32),
        ui::dim(if can { "Find music that isn't on the server yet, and Lidarr fetches it." } else { "Find music that isn't on the server yet. Ask an admin for permission to get it." }, 15),
        row![input, button(text(if f.busy { "Looking…" } else { "Look" }).size(15)).padding([11, 22]).style(ui::primary).on_press_maybe((!f.busy).then_some(Message::FindSubmit))].spacing(10),
    ]
    .spacing(16);
    match &f.results {
        None => {}
        Some(Err(e)) => c = c.push(container(text(e.clone()).size(14)).padding([10, 12]).style(ui::error_box)),
        Some(Ok(r)) => {
            if r.albums.is_empty() && r.artists.is_empty() && r.songs.is_empty() {
                c = c.push(ui::dim(format!("Nothing found for \"{}\".", f.searched), 15));
            }
            if !r.songs.is_empty() {
                let rows: Vec<El> = r.songs.iter().map(|s| found_row(app, s, false, can)).collect();
                c = c.push(section("Songs", Column::with_children(rows).spacing(2).into()));
            }
            if !r.albums.is_empty() {
                let rows: Vec<El> = r.albums.iter().map(|s| found_row(app, s, false, can)).collect();
                c = c.push(section("Albums", Column::with_children(rows).spacing(2).into()));
            }
            if !r.artists.is_empty() {
                let rows: Vec<El> = r.artists.iter().map(|s| found_row(app, s, true, can)).collect();
                c = c.push(section("Artists", Column::with_children(rows).spacing(2).into()));
            }
        }
    }
    if !f.requests.is_empty() {
        let me = app.me.as_ref().map(|m| m.user.name.clone()).unwrap_or_default();
        let rows: Vec<El> = f
            .requests
            .iter()
            .map(|r| {
                let here = !(r.on_server.is_null() || r.on_server == serde_json::Value::Bool(false));
                let state = if here {
                    "On the server".to_string()
                } else if let Some(p) = &r.problem {
                    p.clone()
                } else {
                    "Waiting for Lidarr".to_string()
                };
                let mut line = row![
                    column![text(clip(&format!("{} · {}", r.title, r.artist), 60)).size(15), ui::dim(format!("{} · asked by {} · {}", if r.kind == "artist" { "Artist" } else { "Album" }, r.by, state), 12)].spacing(2).width(Length::Fill)
                ]
                .align_y(Alignment::Center);
                if !here && r.by == me {
                    line = line.push(button(text("Cancel").size(13)).padding([6, 14]).style(ui::secondary).on_press(Message::CancelRequest(r.foreign_id.clone())));
                }
                container(line).padding([8, 12]).into()
            })
            .collect();
        c = c.push(section("Requests", Column::with_children(rows).spacing(2).into()));
    }
    c.into()
}

fn found_row<'a>(app: &'a App, s: &'a FoundItem, is_artist: bool, can: bool) -> El<'a> {
    let art: El = match s.image.as_ref().and_then(|p| app.images.get(p)) {
        Some(h) => image(h.clone()).width(48).height(48).content_fit(iced::ContentFit::Cover).into(),
        None => ui::cover(None, &s.label(), 48.0),
    };
    let mut sub = vec![];
    if let Some(a) = &s.artist {
        sub.push(a.clone());
    }
    if let Some(k) = &s.kind {
        sub.push(k.clone());
    }
    if let Some(y) = &s.year {
        sub.push(y.clone());
    }
    let target = s.request_target(is_artist);
    let action: El = if s.is_on_server() {
        let go = match (&s.on_server, is_artist) {
            (serde_json::Value::String(v), false) if app.lib.album(v).is_some() => Some(Message::Go(Page::Album(v.clone()))),
            (serde_json::Value::String(v), true) => Some(Message::Go(Page::Artist(v.clone()))),
            _ => None,
        };
        button(text("In your library").size(13)).padding([6, 14]).style(ui::secondary).on_press_maybe(go).into()
    } else if s.requested || target.as_ref().map(|(_, id)| app.find.asked.contains(id)).unwrap_or(false) {
        ui::dim("Requested", 13).into()
    } else if let (true, Some((kind, id))) = (can, target) {
        let busy = app.find.asking.contains(&id);
        button(text(if busy { "Asking…" } else { "Get" }).size(13)).padding([6, 18]).style(ui::primary).on_press_maybe((!busy).then_some(Message::RequestMusic(kind.to_string(), id))).into()
    } else {
        ui::dim("Not on the server", 13).into()
    };
    container(row![art, column![text(clip(&s.label(), 60)).size(15), ui::dim(clip(&sub.join(" · "), 70), 12)].spacing(2).width(Length::Fill), action].spacing(14).align_y(Alignment::Center)).padding([6, 12]).into()
}

/* ---------- household and notifications ---------- */

pub fn household(app: &App) -> El<'_> {
    let mut c = column![ui::title("Household", 32), ui::dim("What everyone's listening to, and the playlists they share.", 15)].spacing(16);
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
    for m in &app.household.members {
        let now: El = match &m.now {
            Some(n) => {
                let title = n["title"].as_str().unwrap_or("").to_string();
                let artist = n["artist"].as_str().unwrap_or("").to_string();
                let live = n["listening"].as_bool().unwrap_or(false);
                let album = n["albumId"].as_str().and_then(|a| app.lib.album(a));
                let id = n["id"].as_str().unwrap_or("").to_string();
                button(
                    row![cover_of(app, album, 48.0), column![ui::dim(if live { "Listening now" } else { "Last played" }, 12), text(clip(&format!("{title} · {artist}"), 60)).size(15)].spacing(2)]
                        .spacing(12)
                        .align_y(Alignment::Center),
                )
                .padding(6)
                .style(ui::flat)
                .on_press_maybe((!id.is_empty()).then(|| Message::PlayTracks(vec![id], 0, format!("{}'s pick", m.name))))
                .into()
            }
            None => ui::dim("Not listening right now", 13).into(),
        };
        let mut block = column![row![ic(icon::PEOPLE, 18), ui::title(m.name.clone(), 22)].spacing(10).align_y(Alignment::Center), now].spacing(10);
        if !m.playlists.is_empty() {
            let cards: Vec<El> = m
                .playlists
                .iter()
                .map(|p| {
                    let ids: Vec<String> = p.tracks.iter().map(|t| t.id.clone()).collect();
                    card(playlist_cover(app, &ids, false, &p.name, 164.0), p.name.clone(), format!("{} songs", ids.len()), Message::Go(Page::Playlist(p.id.clone())))
                })
                .collect();
            block = block.push(Row::with_children(cards).spacing(4).wrap());
        }
        c = c.push(container(block).padding([4, 0]));
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

pub fn notifications(app: &App) -> El<'_> {
    let mut c = column![ui::title("Notifications", 32)].spacing(16);
    if app.notes.items.is_empty() {
        return c.push(ui::dim("Nothing yet. Playlist invites and music you asked for show up here.", 15)).into();
    }
    for n in &app.notes.items {
        let mut line = row![
            container(ic(if n.kind.contains("invite") { icon::PEOPLE } else if n.kind.contains("request") { icon::GLOBE } else { icon::BELL }, 18)).width(28),
            column![text(n.text.clone()).size(15).color(if n.read { ui::TEXT_2 } else { ui::TEXT }), ui::dim(ago(n.at), 12)].spacing(2).width(Length::Fill),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        if n.kind == "playlist_invite" && n.done.is_none() {
            if let Some(pl) = n.data["playlistId"].as_str() {
                line = line
                    .push(button(text("Accept").size(13)).padding([6, 16]).style(ui::primary).on_press(Message::SharedOp(pl.to_string(), serde_json::json!({ "op": "accept" }))))
                    .push(button(text("Decline").size(13)).padding([6, 16]).style(ui::secondary).on_press(Message::SharedOp(pl.to_string(), serde_json::json!({ "op": "decline" }))));
            }
        } else if let Some(d) = &n.done {
            line = line.push(ui::dim(d.clone(), 13));
        }
        c = c.push(container(line).padding([10, 12]));
    }
    c.into()
}

/* ---------- settings ---------- */

fn setting_row<'a>(title: &'a str, sub: &'a str, control: El<'a>) -> El<'a> {
    container(row![column![text(title).size(15), ui::dim(sub, 13)].spacing(2).width(Length::Fill), control].spacing(16).align_y(Alignment::Center)).padding([10, 4]).into()
}

fn switch<'a>(on: bool, f: fn(bool) -> Message) -> El<'a> {
    toggler(on).on_toggle(f).size(22).into()
}

pub fn settings(app: &App) -> El<'_> {
    let user = app.me.as_ref().map(|m| m.user.clone()).unwrap_or_default();
    let leveling = app.setting_str("leveling", "off");
    let chip = |label: &'static str, v: &'static str| button(text(label).size(13)).padding([6, 14]).style(ui::chip(leveling == v)).on_press(Message::SetLeveling(v.to_string()));
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
        Some(crate::update::State::Ready(v)) => format!("Version {v} is ready. Restart from the tray menu to finish."),
        Some(crate::update::State::Failed(m)) => m.clone(),
    };
    let role = if user.admin { "Admin" } else { "Member" };
    let col = column![
        ui::title("Settings", 32),
        section(
            "Account",
            column![
                setting_row(Box::leak(user.name.clone().into_boxed_str()), Box::leak(format!("{role} · signed in as {}", user.username).into_boxed_str()), button(text("Change name").size(13)).padding([6, 14]).style(ui::secondary).on_press(Message::RenameMe).into()),
                setting_row("Password", "Change the password you sign in with.", button(text("Change password").size(13)).padding([6, 14]).style(ui::secondary).on_press(Message::ChangePassword).into()),
                setting_row("Sign out", "Sign out on this computer. Downloads stay.", button(text("Sign out").size(13)).padding([6, 14]).style(ui::secondary).on_press(Message::SignOut).into()),
            ]
            .into()
        ),
        section(
            "Playback",
            column![
                setting_row("Volume leveling", "Play songs (or whole albums) at an even loudness, from their ReplayGain tags.", row![chip("Off", "off"), chip("Songs", "track"), chip("Albums", "album")].spacing(6).into()),
                setting_row("Share what I'm listening to", "The household sees what you're playing.", switch(app.setting_bool("shareActivity", true), Message::SetShare)),
                setting_row("Private session", "Nothing you play counts toward your stats, mixes or Recently played.", switch(app.setting_bool("privateSession", false), Message::SetPrivate)),
            ]
            .into()
        ),
        section(
            "Downloads",
            column![setting_row(
                Box::leak(format!("{} songs on this computer", app.downloaded.len()).into_boxed_str()),
                Box::leak(format!("{} used. Albums and playlists you download play without the server.", human_size(app.downloads.size())).into_boxed_str()),
                button(text("Remove all").size(13)).padding([6, 14]).style(ui::secondary).on_press_maybe((!app.saved.is_empty() || !app.downloaded.is_empty()).then_some(Message::RemoveAllDownloads)).into()
            )]
            .into()
        ),
        section(
            "Windows",
            column![
                setting_row("Start with Windows", "Open in the tray when you sign in to Windows.", switch(app.settings.start_with_windows, Message::SetStartWithWindows)),
                setting_row("Close to the system tray", "The close button hides the window and the music keeps playing.", switch(app.settings.close_to_tray, Message::SetCloseToTray)),
                setting_row("Update automatically", "Look for a new version every few hours.", switch(app.settings.auto_update, Message::SetAutoUpdate)),
                setting_row("Updates", Box::leak(update_line.into_boxed_str()), button(text("Check now").size(13)).padding([6, 14]).style(ui::secondary).on_press(Message::CheckUpdate).into()),
            ]
            .into()
        ),
        section(
            "Server",
            column![setting_row("Connected to", Box::leak(server_line.into_boxed_str()), button(text("Switch server").size(13)).padding([6, 14]).style(ui::secondary).on_press(Message::SwitchServer).into())].into()
        ),
        ui::dim("Volvix Home for Windows, native preview. Built with iced.", 12),
    ]
    .spacing(24)
    .max_width(820);
    col.into()
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
        Dialog::NewPlaylist { name, description, add, collaborative } => {
            let title = if add.is_empty() { "New playlist".to_string() } else { format!("New playlist with {} song{}", add.len(), if add.len() == 1 { "" } else { "s" }) };
            column![
                ui::title(title, 22),
                field("Name", name, 0),
                field("Description (optional)", description, 1),
                checkbox("Collaborative: people you invite can add and reorder songs", *collaborative).on_toggle(|v| Message::DialogFlag(0, v)).size(18).text_size(14),
                buttons("Create", true),
            ]
            .spacing(14)
            .into()
        }
        Dialog::EditPlaylist { name, description, public, .. } => column![
            ui::title("Edit details", 22),
            field("Name", name, 0),
            field("Description (optional)", description, 1),
            checkbox("Visible to the household", *public).on_toggle(|v| Message::DialogFlag(0, v)).size(18).text_size(14),
            buttons("Save", true),
        ]
        .spacing(14)
        .into(),
        Dialog::DeletePlaylist { name, .. } => column![ui::title("Delete playlist?", 22), ui::dim(format!("This deletes \"{name}\". Your music stays in your library."), 15), buttons("Delete", true)].spacing(14).into(),
        Dialog::AddTo { tracks } => {
            let mut list = Column::new().spacing(2);
            list = list.push(
                button(row![container(ic(icon::ADD, 18)).width(44).height(44).center_x(44).center_y(44).style(|_| container::Style { background: Some(ui::SURFACE_3.into()), border: iced::border::rounded(6.0), ..Default::default() }), text("New playlist").size(15)].spacing(12).align_y(Alignment::Center))
                    .width(Length::Fill)
                    .padding(6)
                    .style(ui::flat)
                    .on_press(Message::NewPlaylist(tracks.clone())),
            );
            for p in app.playlists().into_iter().filter(|p| p.can_edit()) {
                let all_in = tracks.iter().all(|t| p.tracks.contains(t));
                let sub = format!("{}{} songs", if p.is_shared() { "Collaborative · " } else { "" }, p.tracks.len());
                let mut r = row![playlist_cover(app, &p.tracks, p.kind == Kind::Liked, &p.name, 44.0), column![text(clip(&p.name, 40)).size(15), ui::dim(sub, 12)].spacing(2).width(Length::Fill)].spacing(12).align_y(Alignment::Center);
                if all_in {
                    r = r.push(ui::dim("Added", 12));
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
    }
}
