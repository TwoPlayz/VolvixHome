//! The screens. Each takes the app state and returns what to draw; clicks come back as Messages.
use std::time::Duration;

use iced::widget::{button, column, container, horizontal_space, mouse_area, row, scrollable, slider, text, text_input, Column, Row, Space};
use iced::{Alignment, Element, Length, Padding};
use volvix_core::{format_duration, search, Album, Repeat, Track};

use crate::ui::{self, icon, icon as ic};
use crate::{App, LibTab, Message, Page, Screen, COVER};

type El<'a> = Element<'a, Message>;

pub fn view(app: &App) -> El<'_> {
    let body: El = match app.screen {
        Screen::Connect => connect(app),
        Screen::SignIn => sign_in(app),
        Screen::Loading => centered(column![ui::title("Volvix Home", 28), ui::dim("Loading your music…", 15)].spacing(8).align_x(Alignment::Center).into()),
        Screen::Main => main_screen(app),
    };
    container(body).width(Length::Fill).height(Length::Fill).style(ui::frame).into()
}

/// Which album covers the current screen shows (fetched in the background, then kept on disk).
pub fn wanted_covers(app: &App) -> Vec<(String, u32)> {
    let lib = &app.lib;
    let mut ids: Vec<String> = vec![];
    if let Some(t) = app.status.track.as_deref().and_then(|id| lib.track(id)) {
        ids.push(t.album_id.clone());
    }
    if app.now_playing {
        ids.extend(app.queue.upcoming().iter().take(30).filter_map(|id| lib.track(id)).map(|t| t.album_id.clone()));
    } else {
        match &app.page {
            Page::Home => {
                ids.extend(home_quick(app).iter().map(|a| a.id.clone()));
                ids.extend(lib.recent_albums(&app.data.recent, 12).iter().map(|a| a.id.clone()));
                ids.extend(lib.recently_added(12).iter().map(|a| a.id.clone()));
                ids.extend(lib.top_artists(12).iter().filter_map(|n| lib.artist_cover_album(n)).map(|a| a.id.clone()));
            }
            Page::Library => match app.lib_tab {
                LibTab::Albums => ids.extend(lib.albums().iter().map(|a| a.id.clone())),
                LibTab::Artists => ids.extend(lib.artists().iter().filter_map(|n| lib.artist_cover_album(n)).map(|a| a.id.clone())),
            },
            Page::Album(id) => ids.push(id.clone()),
            Page::Artist(name) => {
                let p = lib.artist_page(name);
                ids.extend(p.albums.iter().chain(p.appears_on.iter()).map(|a| a.id.clone()));
            }
            Page::Search => {
                if app.search.trim().is_empty() {
                    ids.extend(lib.recently_added(usize::MAX).iter().map(|a| a.id.clone()));
                } else {
                    let r = search::search(lib, &app.data.playlists, &app.search);
                    ids.extend(r.albums.iter().take(12).map(|a| a.id.clone()));
                    ids.extend(r.songs.iter().take(5).map(|t| t.album_id.clone()));
                    ids.extend(r.artists.iter().take(6).filter_map(|n| lib.artist_cover_album(n)).map(|a| a.id.clone()));
                }
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    ids.into_iter().filter(|id| seen.insert(id.clone())).map(|id| (id, COVER)).collect()
}

/* ---------- helpers ---------- */

fn centered(e: El<'_>) -> El<'_> {
    container(e).width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill).into()
}

/// Cut a line so it fits ("A very long album ti…").
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t = t.trim_end().to_string();
        t.push('…');
        t
    }
}

fn cover_of<'a>(app: &'a App, album: Option<&Album>, size: f32) -> El<'a> {
    let handle = album.and_then(|a| app.cover_images.get(&(a.id.clone(), COVER)));
    ui::cover(handle, album.map(|a| a.title.as_str()).unwrap_or(""), size)
}

fn section<'a>(title: &'a str, body: El<'a>) -> El<'a> {
    column![ui::title(title, 22), body].spacing(12).into()
}

fn album_card<'a>(app: &'a App, a: &'a Album, sub: String) -> El<'a> {
    let size = 164.0;
    button(
        column![
            cover_of(app, Some(a), size),
            text(clip(&a.title, 22)).size(14).color(ui::TEXT),
            ui::dim(clip(&sub, 26), 13),
        ]
        .spacing(4)
        .width(size),
    )
    .padding(8)
    .style(ui::flat)
    .on_press(Message::Go(Page::Album(a.id.clone())))
    .into()
}

fn album_grid<'a>(app: &'a App, albums: impl IntoIterator<Item = &'a Album>, artist_line: bool) -> El<'a> {
    let cards: Vec<El> = albums
        .into_iter()
        .map(|a| {
            let sub = if artist_line { a.artist.clone() } else { a.year.map(|y| y.to_string()).unwrap_or_else(|| "Album".into()) };
            album_card(app, a, sub)
        })
        .collect();
    Row::with_children(cards).spacing(4).wrap().into()
}

fn artist_card<'a>(app: &'a App, name: &'a str) -> El<'a> {
    let size = 164.0;
    button(column![cover_of(app, app.lib.artist_cover_album(name), size), text(clip(name, 22)).size(14).color(ui::TEXT), ui::dim("Artist", 13)].spacing(4).width(size))
        .padding(8)
        .style(ui::flat)
        .on_press(Message::Go(Page::Artist(name.to_string())))
        .into()
}

/// A list of songs. `ids` is the play order the list stands for (clicking a song plays from there).
fn track_list<'a>(app: &'a App, tracks: &[&'a Track], source: &str, numbered: bool, show_album: bool) -> El<'a> {
    let ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
    let discs = tracks.iter().map(|t| t.disc.max(1)).max().unwrap_or(1);
    let current = app.status.track.as_deref();
    let mut col = Column::new().spacing(2);
    let mut last_disc = 0;
    for (i, t) in tracks.iter().enumerate() {
        if numbered && discs > 1 && t.disc.max(1) != last_disc {
            last_disc = t.disc.max(1);
            col = col.push(container(ui::dim(format!("Disc {last_disc}"), 13)).padding(Padding { top: 10.0, bottom: 4.0, left: 12.0, right: 0.0 }));
        }
        let playing = current == Some(t.id.as_str());
        let lead: El = if playing {
            ic(if app.status.playing { icon::VOLUME } else { icon::PAUSE }, 14).color(ui::ACCENT).into()
        } else if numbered {
            ui::dim(if t.no > 0 { t.no.to_string() } else { "–".into() }, 14).into()
        } else {
            cover_of(app, app.lib.album_of(t), 40.0)
        };
        let mut title_line = row![text(clip(&t.title, 60)).size(15).color(if playing { ui::ACCENT } else { ui::TEXT })].spacing(6).align_y(Alignment::Center);
        if t.explicit {
            title_line = title_line.push(container(text("E").size(10).color(ui::FRAME)).padding([0, 4]).style(|_| container::Style { background: Some(ui::TEXT_2.into()), border: iced::border::rounded(3.0), ..Default::default() }));
        }
        let mut sub = t.artist.clone();
        if show_album {
            if let Some(a) = app.lib.album_of(t) {
                sub = format!("{sub} · {}", a.title);
            }
        }
        let line = row![
            container(lead).width(if numbered { 32.0 } else { 48.0 }).center_x(Length::Fixed(if numbered { 32.0 } else { 48.0 })),
            column![title_line, ui::dim(clip(&sub, 80), 13)].spacing(1).width(Length::Fill),
            button(ic(icon::ADD, 14)).style(ui::icon_btn(false)).padding(8).on_press(Message::AddToQueue(t.id.clone())),
            container(ui::dim(format_duration(t.duration), 13)).width(56).align_x(Alignment::End),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        col = col.push(button(line).width(Length::Fill).padding([6, 12]).style(ui::flat).on_press(Message::PlayTracks(ids.clone(), i, source.to_string())));
    }
    col.into()
}

fn header_buttons<'a>(play: Message, shuffle: Message) -> El<'a> {
    row![
        button(row![ic(icon::PLAY, 14), text("Play").size(15)].spacing(8).align_y(Alignment::Center)).padding([10, 22]).style(ui::primary).on_press(play),
        button(row![ic(icon::SHUFFLE, 14), text("Shuffle").size(15)].spacing(8).align_y(Alignment::Center)).padding([10, 22]).style(ui::secondary).on_press(shuffle),
    ]
    .spacing(10)
    .into()
}

/* ---------- connect / sign in ---------- */

fn form_card<'a>(content: Column<'a, Message>) -> El<'a> {
    centered(container(content.spacing(14).width(380)).padding(32).style(ui::panel).into())
}

fn error_line<'a>(app: &'a App) -> Option<El<'a>> {
    app.form_error.as_ref().map(|e| container(text(e.as_str()).size(14)).padding([10, 12]).width(Length::Fill).style(ui::error_box).into())
}

fn connect(app: &App) -> El<'_> {
    let mut c = column![
        ui::title("Volvix Home", 28),
        ui::dim("Type the address of your Volvix Home server.", 15),
        text_input("music.example.com or nas.local:4747", &app.address).on_input(Message::AddressChanged).on_submit(Message::Connect).padding(12).size(15).style(ui::input),
    ];
    if let Some(e) = error_line(app) {
        c = c.push(e);
    }
    let label = if app.busy { "Connecting…" } else { "Continue" };
    c = c.push(button(container(text(label).size(15)).center_x(Length::Fill)).width(Length::Fill).padding(12).style(ui::primary).on_press_maybe((!app.busy && !app.address.trim().is_empty()).then_some(Message::Connect)));
    form_card(c)
}

fn sign_in(app: &App) -> El<'_> {
    let server = app.client.as_ref().map(|c| c.base().to_string()).unwrap_or_default();
    let (head, sub, action) = if app.setup_needed {
        ("Create your account", "This server is new. The first account runs it (admin).", "Create account")
    } else {
        ("Sign in", "Use the name and password from your Volvix Home server.", "Sign in")
    };
    let mut c = column![
        ui::title(head, 28),
        ui::dim(sub, 15),
        ui::dim(server, 13),
        text_input("Name", &app.name).on_input(Message::NameChanged).on_submit(Message::SignIn).padding(12).size(15).style(ui::input),
        text_input("Password", &app.password).secure(true).on_input(Message::PasswordChanged).on_submit(Message::SignIn).padding(12).size(15).style(ui::input),
    ];
    if let Some(e) = error_line(app) {
        c = c.push(e);
    }
    let ready = !app.busy && !app.name.trim().is_empty() && !app.password.is_empty();
    c = c.push(button(container(text(if app.busy { "Signing in…" } else { action }).size(15)).center_x(Length::Fill)).width(Length::Fill).padding(12).style(ui::primary).on_press_maybe(ready.then_some(Message::SignIn)));
    c = c.push(container(button(text("Use a different server").size(14)).style(ui::flat).padding([6, 10]).on_press(Message::SwitchServer)).center_x(Length::Fill));
    form_card(c)
}

/* ---------- the main window ---------- */

fn main_screen(app: &App) -> El<'_> {
    let page: El = if app.now_playing { now_playing(app) } else { page(app) };
    let content = container(scrollable(container(page).padding([24, 28]).width(Length::Fill)).style(ui::scroller).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(ui::panel);
    column![row![sidebar(app), content].spacing(8).height(Length::Fill), player_bar(app)].spacing(8).padding(8).into()
}

fn sidebar(app: &App) -> El<'_> {
    let item = |c: char, label: &'static str, page: Page| {
        let active = !app.now_playing
            && match (&app.page, &page) {
                (Page::Album(_), Page::Library) | (Page::Artist(_), Page::Library) => true,
                (a, b) => a == b,
            };
        button(row![ic(c, 18), text(label).size(15)].spacing(14).align_y(Alignment::Center)).width(Length::Fill).padding([10, 14]).style(ui::nav(active)).on_press(Message::Go(page))
    };
    let logo = row![
        container(Space::new(14, 14)).style(|_| container::Style { background: Some(ui::ACCENT.into()), border: iced::border::rounded(7.0), ..Default::default() }),
        ui::title("Volvix Home", 18)
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let who = app.me.as_ref().map(|m| m.user.name.clone()).unwrap_or_default();
    let mut nav = column![
        container(logo).padding([8, 14]),
        ui::gap(8),
        item(icon::HOME, "Home", Page::Home),
        item(icon::SEARCH, "Search", Page::Search),
        item(icon::LIBRARY, "Library", Page::Library),
    ]
    .spacing(2);
    if !app.history.is_empty() || app.now_playing {
        nav = nav.push(ui::gap(6)).push(button(row![ic(icon::BACK, 14), text("Back").size(14)].spacing(14).align_y(Alignment::Center)).width(Length::Fill).padding([8, 14]).style(ui::nav(false)).on_press(Message::Back));
    }
    let bottom = column![
        ui::dim(clip(&who, 26), 13),
        button(row![ic(icon::SIGN_OUT, 14), text("Sign out").size(13)].spacing(10).align_y(Alignment::Center)).padding([6, 0]).style(ui::nav(false)).on_press(Message::SignOut),
    ]
    .spacing(2)
    .padding([8, 14]);
    container(column![nav, Space::with_height(Length::Fill), bottom]).width(240).height(Length::Fill).padding(8).style(ui::panel).into()
}

fn page(app: &App) -> El<'_> {
    match &app.page {
        Page::Home => home(app),
        Page::Search => search_page(app),
        Page::Library => library(app),
        Page::Album(id) => album_page(app, id),
        Page::Artist(name) => artist_page(app, name),
    }
}

fn greeting() -> &'static str {
    // local hour from the system clock (no timezone crate: Windows' local time via the C runtime isn't needed for a greeting)
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let h = ((secs / 3600) as i64 + local_offset_hours()).rem_euclid(24);
    if h < 12 { "Good morning" } else if h < 18 { "Good afternoon" } else { "Good evening" }
}

fn local_offset_hours() -> i64 {
    #[repr(C)]
    struct TimeZoneInformation {
        bias: i32,
        rest: [u8; 168],
    }
    extern "system" {
        fn GetTimeZoneInformation(tz: *mut TimeZoneInformation) -> u32;
    }
    let mut tz = TimeZoneInformation { bias: 0, rest: [0; 168] };
    // SAFETY: plain Win32 call filling a correctly sized struct (TIME_ZONE_INFORMATION is 172 bytes)
    let kind = unsafe { GetTimeZoneInformation(&mut tz) };
    // bias is minutes to add to local time to get UTC; daylight time adds its own bias (-60 nearly everywhere)
    let daylight = if kind == 2 { i32::from_le_bytes([tz.rest[164], tz.rest[165], tz.rest[166], tz.rest[167]]) } else { 0 };
    -((tz.bias + daylight) as i64) / 60
}

fn home_quick(app: &App) -> Vec<&Album> {
    let recent = app.lib.recent_albums(&app.data.recent, 6);
    if recent.is_empty() { app.lib.recently_added(6) } else { recent }
}

fn home(app: &App) -> El<'_> {
    let lib = &app.lib;
    if lib.is_empty() {
        return column![ui::title(greeting(), 32), ui::dim("No music yet. Music added to the server shows up here on its own.", 15)].spacing(12).into();
    }
    // quick tiles: jump back in
    let tiles: Vec<El> = home_quick(app)
        .into_iter()
        .map(|a| {
            button(row![cover_of(app, Some(a), 56.0), text(clip(&a.title, 26)).size(15).color(ui::TEXT), horizontal_space()].spacing(14).align_y(Alignment::Center))
                .width(300)
                .padding(0)
                .style(|t, s| {
                    let mut st = ui::flat(t, s);
                    st.background = Some(if matches!(s, button::Status::Hovered) { ui::SURFACE_3 } else { ui::SURFACE_2 }.into());
                    st.border = iced::border::rounded(6.0);
                    st
                })
                .on_press(Message::Go(Page::Album(a.id.clone())))
                .into()
        })
        .collect();
    let mut c = column![ui::title(greeting(), 32), Row::with_children(tiles).spacing(10).wrap()].spacing(24);
    let recent = lib.recent_albums(&app.data.recent, 6);
    if !recent.is_empty() {
        c = c.push(section("Recently played", album_grid(app, recent, true)));
    }
    c = c.push(section("Recently added", album_grid(app, lib.recently_added(6), true)));
    let artists: Vec<El> = lib.top_artists(6).into_iter().map(|n| artist_card(app, Box::leak(n.into_boxed_str()))).collect();
    c = c.push(section("Artists", Row::with_children(artists).spacing(4).wrap().into()));
    c.into()
}

fn library(app: &App) -> El<'_> {
    let tab = |label: &'static str, t: LibTab| button(text(label).size(14)).padding([7, 16]).style(ui::chip(app.lib_tab == t)).on_press(Message::LibTab(t));
    let head = column![ui::title("Library", 32), row![tab("Albums", LibTab::Albums), tab("Artists", LibTab::Artists)].spacing(8)].spacing(14);
    let body: El = match app.lib_tab {
        LibTab::Albums => album_grid(app, app.lib.albums_by_title(), true),
        LibTab::Artists => {
            let rows: Vec<El> = app
                .lib
                .artists()
                .iter()
                .map(|n| {
                    let count = app.lib.albums_by(n).len();
                    let sub = if count == 0 { "Appears on albums".to_string() } else { format!("{count} album{}", if count == 1 { "" } else { "s" }) };
                    button(row![cover_of(app, app.lib.artist_cover_album(n), 48.0), column![text(n.as_str()).size(15).color(ui::TEXT), ui::dim(sub, 13)].spacing(2)].spacing(14).align_y(Alignment::Center))
                        .width(Length::Fill)
                        .padding([6, 10])
                        .style(ui::flat)
                        .on_press(Message::Go(Page::Artist(n.clone())))
                        .into()
                })
                .collect();
            Column::with_children(rows).spacing(2).into()
        }
    };
    column![head, body].spacing(20).into()
}

fn album_page<'a>(app: &'a App, id: &str) -> El<'a> {
    let Some(a) = app.lib.album(id) else { return ui::dim("This album isn't on the server any more.", 15).into() };
    let tracks = app.lib.album_tracks(id);
    let mut info = column![
        ui::dim("Album", 13),
        ui::title(a.title.as_str(), 40),
        row![
            button(text(a.artist.as_str()).size(15).color(ui::TEXT)).padding(0).style(ui::flat).on_press(Message::Go(Page::Artist(a.artist.clone()))),
            ui::dim(format!("· {}", app.lib.album_summary(id)), 15),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .spacing(6);
    if let Some(g) = &a.genre {
        info = info.push(ui::dim(g.as_str(), 13));
    }
    info = info.push(ui::gap(10)).push(header_buttons(Message::PlayAlbum(id.to_string(), 0), Message::ShuffleAlbum(id.to_string())));
    let head = row![cover_of(app, Some(a), 220.0), info].spacing(28).align_y(Alignment::End);
    let mut c = column![head, track_list(app, &tracks, &a.title, true, false)].spacing(28);
    let mut foot = vec![];
    if let Some(l) = &a.label {
        foot.push(l.clone());
    }
    if let Some(cp) = &a.copyright {
        foot.push(cp.clone());
    }
    let q = &a.quality;
    if !q.codec.is_empty() {
        let mut s = q.codec.to_uppercase();
        if let (Some(b), Some(k)) = (q.bits, q.khz) {
            s = format!("{s} · {b}-bit / {k} kHz");
        } else if let Some(k) = q.kbps {
            s = format!("{s} · {k} kbps");
        }
        foot.push(s);
    }
    if !foot.is_empty() {
        c = c.push(ui::dim(foot.join("  ·  "), 12));
    }
    c.into()
}

fn artist_page<'a>(app: &'a App, name: &str) -> El<'a> {
    let p = app.lib.artist_page(name);
    let ids: Vec<String> = p.songs.iter().map(|t| t.id.clone()).collect();
    let mut shuffled = ids.clone();
    fastrand_shuffle(&mut shuffled);
    let n_albums = p.albums.len();
    let sub = if n_albums == 0 { "Appears on albums".to_string() } else { format!("{n_albums} album{} · {} songs", if n_albums == 1 { "" } else { "s" }, p.songs.len()) };
    let page_name = p.name.clone();
    let head = column![
        ui::dim("Artist", 13),
        ui::title(page_name.clone(), 48),
        ui::dim(sub, 15),
        ui::gap(6),
        header_buttons(Message::PlayTracks(ids, 0, page_name.clone()), Message::PlayTracks(shuffled, 0, page_name.clone())),
    ]
    .spacing(6);
    let mut c = column![head].spacing(28);
    if !p.songs.is_empty() {
        let top: Vec<&Track> = p.songs.iter().take(5).copied().collect();
        c = c.push(section("Songs", track_list(app, &top, &page_name, false, true)));
    }
    if !p.albums.is_empty() {
        c = c.push(section("Albums", album_grid(app, p.albums.clone(), false)));
    }
    if !p.appears_on.is_empty() {
        c = c.push(section("Appears on", album_grid(app, p.appears_on.clone(), true)));
    }
    c.into()
}

fn fastrand_shuffle(v: &mut [String]) {
    // tiny local shuffle so views don't need another crate (xorshift seeded from the clock)
    let mut x = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) | 1;
    for i in (1..v.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        v.swap(i, (x % (i as u64 + 1)) as usize);
    }
}

fn search_page(app: &App) -> El<'_> {
    let input = text_input("Artists, albums, songs, playlists", &app.search)
        .on_input(Message::SearchChanged)
        .padding(12)
        .size(16)
        .icon(text_input::Icon { font: ui::ICONS, code_point: icon::SEARCH, size: Some(16.into()), spacing: 10.0, side: text_input::Side::Left })
        .style(ui::input);
    let mut c = column![ui::title("Search", 32), input].spacing(20);
    let q = app.search.trim();
    if q.is_empty() {
        let all = app.lib.recently_added(usize::MAX);
        if all.is_empty() {
            return c.push(ui::dim("No music yet.", 15)).into();
        }
        return c.push(section("Everything on your server", album_grid(app, all, true))).into();
    }
    let r = search::search(&app.lib, &app.data.playlists, q);
    if r.is_empty() {
        return c.push(ui::dim(format!("Nothing in your library for \"{q}\"."), 15)).into();
    }
    if !r.artists.is_empty() {
        let cards: Vec<El> = r.artists.iter().take(6).map(|n| artist_card(app, Box::leak(n.clone().into_boxed_str()))).collect();
        c = c.push(section("Artists", Row::with_children(cards).spacing(4).wrap().into()));
    }
    if !r.songs.is_empty() {
        let list: Vec<&Track> = r.songs.iter().take(5).copied().collect();
        c = c.push(section("Songs", track_list(app, &list, "Search", false, true)));
    }
    if !r.albums.is_empty() {
        c = c.push(section("Albums", album_grid(app, r.albums.iter().take(12).copied(), true)));
    }
    c.into()
}

/* ---------- Now Playing and the player bar ---------- */

fn now_playing(app: &App) -> El<'_> {
    let Some(t) = app.status.track.as_deref().and_then(|id| app.lib.track(id)) else {
        return column![ui::title("Now playing", 32), ui::dim("Nothing playing. Pick an album or a song to start.", 15)].spacing(12).into();
    };
    let album = app.lib.album_of(t);
    let left = column![
        cover_of(app, album, 360.0),
        ui::gap(8),
        ui::title(t.title.as_str(), 28),
        button(text(t.artist.as_str()).size(16).color(ui::TEXT_2)).padding(0).style(ui::flat).on_press(Message::Go(Page::Artist(t.artist.clone()))),
        button(text(album.map(|a| a.title.as_str()).unwrap_or("")).size(14).color(ui::TEXT_3)).padding(0).style(ui::flat).on_press_maybe(album.map(|a| Message::Go(Page::Album(a.id.clone())))),
    ]
    .spacing(6)
    .width(360);

    let pos = app.queue.position().unwrap_or(0);
    let mut q = column![ui::title("Up next", 22), ui::dim(if app.queue.source.is_empty() { String::new() } else { format!("Playing from {}", app.queue.source) }, 13)].spacing(4);
    let upcoming = app.queue.upcoming();
    if upcoming.is_empty() {
        q = q.push(ui::gap(8)).push(ui::dim("Nothing after this song.", 14));
    }
    let mut list = Column::new().spacing(2);
    for (k, id) in upcoming.iter().enumerate().take(100) {
        let Some(t) = app.lib.track(id) else { continue };
        let index = pos + 1 + k;
        let line = row![
            cover_of(app, app.lib.album_of(t), 40.0),
            column![text(clip(&t.title, 50)).size(14).color(ui::TEXT), ui::dim(clip(&t.artist, 50), 12)].spacing(1).width(Length::Fill),
            button(ic('\u{E711}', 11)).style(ui::icon_btn(false)).padding(8).on_press(Message::RemoveFromQueue(index)),
            ui::dim(format_duration(t.duration), 12),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        list = list.push(button(line).width(Length::Fill).padding([5, 8]).style(ui::flat).on_press(Message::JumpQueue(index)));
    }
    let right = column![q, list].spacing(10).width(Length::Fill);
    row![left, right].spacing(40).into()
}

fn player_bar(app: &App) -> El<'_> {
    let st = &app.status;
    let track = st.track.as_deref().and_then(|id| app.lib.track(id));
    let album = track.and_then(|t| app.lib.album_of(t));

    let left: El = match track {
        Some(t) => button(
            row![
                cover_of(app, album, 56.0),
                column![text(clip(&t.title, 34)).size(14).color(ui::TEXT), ui::dim(clip(&t.artist, 38), 12)].spacing(2),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding(4)
        .style(ui::flat)
        .on_press(Message::ToggleNowPlaying)
        .into(),
        None => row![ui::cover::<Message>(None, "", 56.0), ui::dim("Nothing playing", 13)].spacing(12).align_y(Alignment::Center).into(),
    };

    let has = track.is_some() || !app.queue.is_empty();
    let small = |c: char, on: bool, m: Message| button(ic(c, 16)).padding(8).style(ui::icon_btn(on)).on_press_maybe(has.then_some(m));
    let play_icon = if st.playing { icon::PAUSE } else { icon::PLAY };
    let play = button(container(ic(play_icon, 14).color(ui::FRAME)).center_x(16).center_y(16)).width(36).height(36).padding(10).style(ui::round_play).on_press_maybe(has.then_some(Message::TogglePlay));
    let repeat_icon = if app.queue.repeat() == Repeat::One { icon::REPEAT_ONE } else { icon::REPEAT_ALL };
    let controls = row![
        small(icon::SHUFFLE, app.queue.shuffle(), Message::ToggleShuffle),
        small(icon::PREVIOUS, false, Message::Previous),
        play,
        small(icon::NEXT, false, Message::Next),
        small(repeat_icon, app.queue.repeat() != Repeat::Off, Message::CycleRepeat),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    // the library's length (from the file's tags) is right; decoders only estimate for some formats
    let total = track.filter(|t| t.duration > 0).map(|t| Duration::from_secs(t.duration as u64)).or(st.duration).unwrap_or_default().as_secs_f32();
    let pos = app.seeking.unwrap_or(st.position.as_secs_f32()).min(total.max(0.0));
    let time = |s: f32| ui::dim(format_duration(s as u32), 11);
    let progress = row![
        container(time(pos)).width(42).align_x(Alignment::End),
        slider(0.0..=total.max(1.0), pos, Message::SeekDrag).on_release(Message::SeekCommit).step(0.25_f32).style(ui::slider),
        container(if st.loading { ui::dim("…", 11) } else { time(total) }).width(42),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let center = column![controls, progress].spacing(4).align_x(Alignment::Center).width(Length::FillPortion(2)).max_width(640);

    let vol_icon = if app.volume <= 0.001 { icon::MUTE } else { icon::VOLUME };
    let right = row![
        horizontal_space(),
        button(ic(icon::QUEUE, 16)).padding(8).style(ui::icon_btn(app.now_playing)).on_press(Message::ToggleNowPlaying),
        button(ic(vol_icon, 16)).padding(8).style(ui::icon_btn(false)).on_press(Message::ToggleMute),
        container(slider(0.0..=1.0, app.volume, Message::Volume).step(0.01_f32).style(ui::slider)).width(110),
    ]
    .spacing(4)
    .align_y(Alignment::Center)
    .width(Length::FillPortion(1));

    let mut bar = column![];
    if let Some(e) = &st.error {
        bar = bar.push(container(text(e.as_str()).size(12)).padding([4, 10]).style(ui::error_box));
    }
    let main = row![container(left).width(Length::FillPortion(1)), center, right].spacing(16).align_y(Alignment::Center);
    container(bar.push(mouse_area(main)).spacing(4)).height(84).padding([8, 12]).center_y(84).style(ui::frame).into()
}
