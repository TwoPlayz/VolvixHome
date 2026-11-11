//! The window's frame and the music screens (Home, Library, Album, Artist, Search, Now Playing,
//! the player). Playlists, mixes, Get more music, household, notifications, settings and dialogs
//! are in pages.rs. Each takes the app state and returns what to draw; clicks come back as Messages.
use std::time::Duration;

use iced::widget::{button, column, container, horizontal_space, mouse_area, opaque, row, scrollable, stack, text, text_input, Column, Row, Space};
use iced::{Alignment, Element, Length, Padding};
use volvix_core::playlists::Kind;
use volvix_core::{format_duration, search, Album, Repeat, Track};

use crate::ui::{self, icon, icon as ic};
use crate::{marks, pages};
use crate::{App, LibTab, Message, NpTab, Page, Screen, SearchType, COVER};

pub type El<'a> = Element<'a, Message>;

pub fn view(app: &App) -> El<'_> {
    let body: El = match app.screen {
        Screen::Connect => connect(app),
        Screen::SignIn => sign_in(app),
        Screen::Loading => centered(column![marks::logo(48.0), ui::title("Volvix Home", 28), ui::dim("Loading your music…", 15)].spacing(10).align_x(Alignment::Center).into()),
        Screen::Main => main_screen(app),
    };
    let framed: El = column![title_bar(app), container(body).width(Length::Fill).height(Length::Fill)].into();
    container(framed).width(Length::Fill).height(Length::Fill).style(ui::frame).into()
}

/// The app's own title bar: drag to move, double-click to maximize, and the three window buttons.
fn title_bar(app: &App) -> El<'_> {
    let btn = |c: char, m: Message, close: bool| {
        button(container(ic(c, 10)).center_x(46).center_y(32)).width(46).height(32).padding(0).style(move |_, s| {
            let hover = matches!(s, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: hover.then(|| if close { iced::Color::from_rgb8(0xC4, 0x2B, 0x1C) } else { ui::HOVER }.into()),
                text_color: if hover && close { iced::Color::WHITE } else { ui::TEXT_2 },
                ..Default::default()
            }
        })
        .on_press(m)
    };
    let drag = mouse_area(container(Space::new(Length::Fill, 32)).width(Length::Fill)).on_press(Message::TitlePress);
    row![
        drag,
        btn('\u{E921}', Message::Minimize, false),
        btn(if app.maximized { '\u{E923}' } else { '\u{E922}' }, Message::ToggleMaximize, false),
        btn('\u{E8BB}', Message::CloseWindow, true),
    ]
    .height(32)
    .into()
}

/// Which album covers the current screen shows (fetched in the background, then kept on disk).
pub fn wanted_covers(app: &App) -> Vec<(String, u32)> {
    let lib = &app.lib;
    let mut ids: Vec<String> = vec![];
    let first_album = |tracks: &[String]| tracks.iter().find_map(|id| lib.track(id)).map(|t| t.album_id.clone());
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
                ids.extend(volvix_core::mixes::top_albums(lib, &app.data, 6).iter().map(|a| a.id.clone()));
                ids.extend(volvix_core::mixes::random_albums(lib, &app.me_id(), volvix_core::userdata::now(), 6).iter().map(|a| a.id.clone()));
                ids.extend(lib.top_artists(12).iter().filter_map(|n| lib.artist_cover_album(n)).map(|a| a.id.clone()));
                ids.extend(app.playlists().iter().filter_map(|p| first_album(&p.tracks)));
            }
            Page::Library => match app.lib_tab {
                LibTab::Albums => ids.extend(lib.albums().iter().map(|a| a.id.clone())),
                LibTab::Artists => ids.extend(lib.artists().iter().filter_map(|n| lib.artist_cover_album(n)).map(|a| a.id.clone())),
                LibTab::Playlists => ids.extend(app.playlists().iter().filter_map(|p| first_album(&p.tracks))),
                LibTab::Downloaded => {
                    for s in &app.saved {
                        if s.kind == "album" {
                            ids.push(s.id.clone());
                        } else if let Some(p) = app.playlist(&s.id) {
                            ids.extend(first_album(&p.tracks));
                        }
                    }
                }
            },
            Page::Album(id) => ids.push(id.clone()),
            Page::Artist(name) => {
                let p = lib.artist_page(name);
                ids.extend(p.albums.iter().chain(p.appears_on.iter()).map(|a| a.id.clone()));
            }
            Page::Playlist(id) => {
                if let Some(p) = app.playlist(id) {
                    ids.extend(p.tracks.iter().take(200).filter_map(|t| lib.track(t)).map(|t| t.album_id.clone()));
                }
            }
            Page::Mix(id) => {
                if let Some(m) = app.mixes.iter().find(|m| m.id == *id) {
                    ids.extend(m.ids.iter().filter_map(|t| lib.track(t)).map(|t| t.album_id.clone()));
                }
            }
            Page::Household => {
                for m in &app.household.members {
                    ids.extend(m.playlists.iter().filter_map(|p| p.tracks.first()).filter_map(|e| lib.track(&e.id)).map(|t| t.album_id.clone()));
                    ids.extend(m.now.as_ref().and_then(|n| n["albumId"].as_str()).map(String::from));
                }
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
            Page::GetMusic | Page::Notifications | Page::Settings => {}
        }
    }
    let mut seen = std::collections::HashSet::new();
    ids.into_iter().filter(|id| seen.insert(id.clone())).map(|id| (id, COVER)).collect()
}

/* ---------- helpers (also used by pages.rs) ---------- */

pub fn centered(e: El<'_>) -> El<'_> {
    container(e).width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill).into()
}

/// Cut a line so it fits ("A very long album ti…").
pub fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t = t.trim_end().to_string();
        t.push('…');
        t
    }
}

pub fn cover_of<'a>(app: &'a App, album: Option<&Album>, size: f32) -> El<'a> {
    let handle = album.and_then(|a| app.cover_images.get(&(a.id.clone(), COVER)));
    ui::cover(handle, album.map(|a| a.title.as_str()).unwrap_or(""), size)
}

/// A playlist's cover: its first song's album (Liked Songs gets a heart).
pub fn playlist_cover<'a>(app: &'a App, tracks: &[String], liked: bool, name: &str, size: f32) -> El<'a> {
    if liked {
        return container(ic(icon::HEART_FILL, (size * 0.36) as u16).color(ui::TEXT)).width(size).height(size).center_x(size).center_y(size).style(ui::mix_tile("liked")).into();
    }
    match tracks.iter().find_map(|id| app.lib.track(id)).and_then(|t| app.lib.album_of(t)) {
        Some(a) => cover_of(app, Some(a), size),
        None => ui::cover(None, name, size),
    }
}

pub fn section<'a>(title: &'a str, body: El<'a>) -> El<'a> {
    column![ui::title(title, 22), body].spacing(12).into()
}

pub fn card<'a>(art: El<'a>, title: String, sub: String, on: Message) -> El<'a> {
    let size = 164.0;
    button(column![art, text(clip(&title, 22)).size(14).color(ui::TEXT), ui::dim(clip(&sub, 26), 13)].spacing(4).width(size)).padding(8).style(ui::flat).on_press(on).into()
}

fn album_card<'a>(app: &'a App, a: &'a Album, sub: String) -> El<'a> {
    card(cover_of(app, Some(a), 164.0), a.title.clone(), sub, Message::Go(Page::Album(a.id.clone())))
}

pub fn album_grid<'a>(app: &'a App, albums: impl IntoIterator<Item = &'a Album>, artist_line: bool) -> El<'a> {
    let cards: Vec<El> = albums
        .into_iter()
        .map(|a| {
            let sub = if artist_line { a.artist.clone() } else { a.year.map(|y| y.to_string()).unwrap_or_else(|| "Album".into()) };
            album_card(app, a, sub)
        })
        .collect();
    Row::with_children(cards).spacing(4).wrap().into()
}

fn artist_card<'a>(app: &'a App, name: String) -> El<'a> {
    card(cover_of(app, app.lib.artist_cover_album(&name), 164.0), name.clone(), "Artist".into(), Message::Go(Page::Artist(name)))
}

pub fn playlist_card<'a>(app: &'a App, p: &volvix_core::playlists::PlaylistView) -> El<'a> {
    let sub = match &p.kind {
        Kind::Household { owner } => format!("By {owner}"),
        Kind::Shared { .. } => format!("Collaborative · {} songs", p.tracks.len()),
        _ => format!("{} song{}", p.tracks.len(), if p.tracks.len() == 1 { "" } else { "s" }),
    };
    card(playlist_cover(app, &p.tracks, p.kind == Kind::Liked, &p.name, 164.0), p.name.clone(), sub, Message::Go(Page::Playlist(p.id.clone())))
}

/// How a song list behaves: plain, or a playlist you can edit (drag to reorder; remove songs
/// unless it's Liked Songs, where the heart does that).
#[derive(Clone, Copy)]
pub enum Rows<'a> {
    Plain,
    Playlist { id: &'a str, liked: bool },
}

/// The grip you drag a row by.
pub fn drag_handle<'a>(list: crate::DragList, index: usize) -> El<'a> {
    mouse_area(container(ic('\u{E700}', 13).color(ui::TEXT_3)).padding([8, 6]))
        .interaction(iced::mouse::Interaction::Grab)
        .on_press(Message::DragStart(list, index))
        .into()
}

/// While dragging: the row under the pointer reports itself, and a red line shows where it lands.
pub fn drop_target<'a>(app: &App, list: &crate::DragList, index: usize, row: El<'a>) -> El<'a> {
    let Some(d) = app.drag.as_ref().filter(|d| d.list == *list) else { return row };
    let line = |show: bool| -> El<'a> {
        container(Space::new(Length::Fill, 2))
            .style(move |_| container::Style { background: show.then(|| ui::ACCENT.into()), ..Default::default() })
            .into()
    };
    let above = d.over == index && d.over < d.from;
    let below = d.over == index && d.over > d.from;
    let faded: El = if d.from == index { container(row).style(|_| container::Style { background: Some(ui::ACTIVE.into()), border: iced::border::rounded(8.0), ..Default::default() }).into() } else { row };
    mouse_area(column![line(above), faded, line(below)]).on_enter(Message::DragOver(index)).into()
}

/// A list of songs. Clicking one plays the list from there.
pub fn track_list<'a>(app: &'a App, tracks: &[&'a Track], source: &str, numbered: bool, show_album: bool, rows: Rows<'_>) -> El<'a> {
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
            // the playing bars, as on the website (still while paused)
            marks::eq(app.status.playing, 12.0)
        } else if numbered {
            ui::dim(if t.no > 0 { t.no.to_string() } else { "–".into() }, 14).into()
        } else {
            cover_of(app, app.lib.album_of(t), 40.0)
        };
        let mut title_line = row![text(clip(&t.title, 60)).size(15).color(if playing { ui::ACCENT } else { ui::TEXT })].spacing(6).align_y(Alignment::Center);
        if t.explicit {
            title_line = title_line.push(container(text("E").size(10).color(ui::FRAME)).padding([0, 4]).style(|_| container::Style { background: Some(ui::TEXT_2.into()), border: iced::border::rounded(3.0), ..Default::default() }));
        }
        if app.downloaded.contains(&t.id) {
            title_line = title_line.push(ic(icon::DOWNLOADED, 11).color(ui::ACCENT));
        }
        let mut sub = t.artist.clone();
        if show_album {
            if let Some(a) = app.lib.album_of(t) {
                sub = format!("{sub} · {}", a.title);
            }
        }
        let liked = app.is_liked(&t.id);
        let small = |c: char, on: bool, m: Message| button(ic(c, 14)).style(ui::icon_btn(on)).padding(7).on_press(m);
        let mut line = row![
            container(lead).width(if numbered { 32.0 } else { 48.0 }).center_x(Length::Fixed(if numbered { 32.0 } else { 48.0 })),
            column![title_line, ui::dim(clip(&sub, 80), 13)].spacing(1).width(Length::Fill),
            small(if liked { icon::HEART_FILL } else { icon::HEART }, liked, Message::ToggleLike(t.id.clone())),
            small(icon::ADD, false, Message::AddToQueue(t.id.clone())),
            small(icon::PLAYLIST, false, Message::AddToPlaylist(vec![t.id.clone()])),
        ]
        .spacing(4)
        .align_y(Alignment::Center);
        if let Rows::Playlist { id: pl, liked: false } = rows {
            line = line.push(small(icon::DELETE, false, Message::RemoveFromPlaylist(pl.to_string(), i, t.id.clone())));
        }
        line = line.push(container(ui::dim(format_duration(t.duration), 13)).width(52).align_x(Alignment::End));
        let song: El = button(line).width(Length::Fill).padding([6, 12]).style(ui::flat).on_press(Message::PlayTracks(ids.clone(), i, source.to_string())).into();
        match rows {
            Rows::Playlist { id: pl, .. } => {
                let list = crate::DragList::Playlist(pl.to_string());
                let r: El = row![drag_handle(list.clone(), i), song].align_y(Alignment::Center).into();
                col = col.push(drop_target(app, &list, i, r));
            }
            Rows::Plain => col = col.push(song),
        }
    }
    col.into()
}

pub fn header_buttons<'a>(play: Message, shuffle: Message, extra: Vec<El<'a>>) -> El<'a> {
    let mut r = row![
        button(row![ic(icon::PLAY, 14), text("Play").size(15).wrapping(text::Wrapping::None)].spacing(8).align_y(Alignment::Center)).padding([10, 22]).style(ui::primary).on_press(play),
        button(row![ic(icon::SHUFFLE, 14), text("Shuffle").size(15).wrapping(text::Wrapping::None)].spacing(8).align_y(Alignment::Center)).padding([10, 22]).style(ui::secondary).on_press(shuffle),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    for e in extra {
        r = r.push(e);
    }
    // narrow windows: the round buttons go onto the next line instead of squashing the labels
    r.wrap().into()
}

/// A round icon button for page headers (download, edit, delete…). `on` = switched on (red).
pub fn round_icon<'a>(c: char, on: bool, tip: &'a str, m: Option<Message>) -> El<'a> {
    let b = button(container(ic(c, 16)).center_x(20).center_y(20)).width(42).height(42).padding(11).style(move |t, s| {
        let mut st = ui::secondary(t, s);
        if on {
            st.text_color = ui::ACCENT;
        }
        st
    });
    iced::widget::tooltip(b.on_press_maybe(m), container(text(tip).size(12)).padding([4, 8]).style(ui::dialog), iced::widget::tooltip::Position::Bottom).into()
}

/// Download button for an album or playlist header.
pub fn download_button<'a>(app: &'a App, kind: &str, id: &str, tracks: &[String]) -> El<'a> {
    let saved = app.is_saved(kind, id);
    let have = tracks.iter().filter(|t| app.downloaded.contains(*t)).count();
    let tip = if !saved {
        "Download to listen offline"
    } else if have < tracks.len() {
        "Downloading… (click to remove)"
    } else {
        "Downloaded (click to remove)"
    };
    round_icon(if saved && have == tracks.len() { icon::DOWNLOADED } else { icon::DOWNLOAD }, saved, tip, Some(Message::ToggleSaved(kind.to_string(), id.to_string())))
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

/// Back and forward, at the top left of the content (as on the website).
fn history_arrows(app: &App) -> El<'_> {
    let arrow = |c: char, m: Option<Message>| {
        let on = m.is_some();
        button(container(ic(c, 12)).center_x(32).center_y(32)).width(32).height(32).padding(0).on_press_maybe(m).style(move |_, s| button::Style {
            background: Some(if on && matches!(s, button::Status::Hovered) { ui::SURFACE_3 } else { ui::FRAME }.into()),
            text_color: if on { ui::TEXT } else { ui::TEXT_3 },
            border: iced::border::rounded(16.0),
            ..Default::default()
        })
    };
    let can_back = app.now_playing || !app.history.is_empty();
    container(row![arrow('\u{E76B}', can_back.then_some(Message::Back)), arrow('\u{E76C}', (!app.forward.is_empty()).then_some(Message::Forward))].spacing(8))
        .padding(iced::Padding { top: 16.0, bottom: 4.0, left: 24.0, right: 24.0 })
        .into()
}

fn main_screen(app: &App) -> El<'_> {
    // Now Playing has its own scrolling panels; every other page scrolls as a whole
    let content: El = if app.now_playing {
        container(now_playing(app)).padding([8, 28]).width(Length::Fill).height(Length::Fill).into()
    } else {
        scrollable(container(page(app)).padding([8, 28]).width(Length::Fill)).style(ui::scroller).height(Length::Fill).into()
    };
    let content = container(column![history_arrows(app), content]).width(Length::Fill).height(Length::Fill).style(ui::panel);
    let mut body = column![];
    if app.offline {
        body = body.push(container(text("Offline: the server can't be reached. Downloaded music still plays.").size(13)).padding([6, 12]).width(Length::Fill).style(ui::error_box));
    }
    let base: El = body.push(row![sidebar(app), content].spacing(8).height(Length::Fill)).push(player_bar(app)).spacing(8).padding(8).into();
    let mut layers = stack![base];
    if let Some((msg, _)) = &app.toast {
        layers = layers.push(
            container(container(text(msg.as_str()).size(14)).padding([10, 16]).style(ui::toast))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::End)
                .padding(Padding { bottom: 110.0, ..Padding::ZERO }),
        );
    }
    if let Some(d) = &app.dialog {
        let card = container(pages::dialog(app, d)).width(440).padding(24).style(ui::dialog);
        layers = layers.push(opaque(mouse_area(container(opaque(card)).width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill).style(ui::scrim)).on_press(Message::DialogCancel)));
    }
    layers.into()
}

fn sidebar(app: &App) -> El<'_> {
    let is = |page: &Page| {
        !app.now_playing
            && match (&app.page, page) {
                (Page::Album(_), Page::Library) | (Page::Artist(_), Page::Library) => true,
                (a, b) => a == b,
            }
    };
    let item = |c: char, label: &'static str, page: Page| {
        let active = is(&page);
        button(row![ic(c, 18), text(label).size(15)].spacing(14).align_y(Alignment::Center)).width(Length::Fill).padding([9, 14]).style(ui::nav(active)).on_press(Message::Go(page))
    };
    let logo = row![
        marks::logo(22.0),
        ui::title("Volvix Home", 18),
        horizontal_space(),
        bell(app),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let mut nav = column![
        container(logo).padding(Padding { top: 8.0, bottom: 8.0, left: 14.0, right: 4.0 }),
        ui::gap(6),
        item(icon::HOME, "Home", Page::Home),
        item(icon::SEARCH, "Search", Page::Search),
        item(icon::LIBRARY, "Library", Page::Library),
    ]
    .spacing(2);
    nav = nav.push(item(icon::PEOPLE, "Household", Page::Household));

    // your playlists
    let head = row![ui::dim("Playlists", 13), horizontal_space(), button(ic(icon::ADD, 13)).padding(6).style(ui::icon_btn(false)).on_press(Message::NewPlaylist(vec![]))].align_y(Alignment::Center);
    let mut list = Column::new().spacing(0);
    for p in app.playlists().into_iter().filter(|p| !matches!(p.kind, Kind::Household { .. })) {
        let active = is(&Page::Playlist(p.id.clone()));
        let mark: El = match p.kind {
            Kind::Liked => ic(icon::HEART_FILL, 12).color(ui::ACCENT).into(),
            Kind::Shared { .. } => ic(icon::PEOPLE, 12).into(),
            _ => ic(icon::PLAYLIST, 12).into(),
        };
        list = list.push(button(row![mark, text(clip(&p.name, 24)).size(14)].spacing(10).align_y(Alignment::Center)).width(Length::Fill).padding([6, 14]).style(ui::nav(active)).on_press(Message::Go(Page::Playlist(p.id.clone()))));
    }
    let playlists = column![container(head).padding(Padding { top: 0.0, bottom: 0.0, left: 14.0, right: 4.0 }), scrollable(list).style(ui::scroller).height(Length::Fill)].spacing(2);

    let who = app.me.as_ref().map(|m| m.user.name.clone()).unwrap_or_default();
    let bottom = button(row![ic(icon::SETTINGS, 16), column![text(clip(&who, 22)).size(13), ui::dim("Settings and account", 11)]].spacing(12).align_y(Alignment::Center))
        .width(Length::Fill)
        .padding([8, 14])
        .style(ui::nav(is(&Page::Settings)))
        .on_press(Message::Go(Page::Settings));
    container(column![nav, ui::gap(10), playlists, bottom].spacing(4)).width(240).height(Length::Fill).padding(8).style(ui::panel).into()
}

fn bell(app: &App) -> El<'_> {
    let n = app.notes.unread;
    let mut r = row![ic(icon::BELL, 16)].spacing(4).align_y(Alignment::Center);
    if n > 0 {
        r = r.push(container(text(if n > 99 { "99+".to_string() } else { n.to_string() }).size(11)).padding([1, 6]).style(ui::badge));
    }
    button(r).padding(8).style(ui::icon_btn(app.page == Page::Notifications && !app.now_playing)).on_press(Message::Go(Page::Notifications)).into()
}

fn page(app: &App) -> El<'_> {
    match &app.page {
        Page::Home => home(app),
        Page::Search => search_page(app),
        Page::Library => library(app),
        Page::Album(id) => album_page(app, id),
        Page::Artist(name) => artist_page(app, name),
        Page::Playlist(id) => pages::playlist_page(app, id),
        Page::Mix(id) => pages::mix_page(app, id),
        Page::GetMusic => pages::requests(app),
        Page::Household => pages::household(app),
        Page::Notifications => pages::notifications(app),
        Page::Settings => pages::settings(app),
    }
}

fn greeting() -> &'static str {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let h = ((secs / 3600) as i64 + local_offset_hours()).rem_euclid(24);
    if h < 12 {
        "Good morning"
    } else if h < 18 {
        "Good afternoon"
    } else {
        "Good evening"
    }
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
    if recent.is_empty() {
        app.lib.recently_added(6)
    } else {
        recent
    }
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
    if !app.mixes.is_empty() {
        let cards: Vec<El> = app.mixes.iter().map(|m| pages::mix_card(m)).collect();
        c = c.push(section("Made for you", Row::with_children(cards).spacing(4).wrap().into()));
    }
    let recent = lib.recent_albums(&app.data.recent, 6);
    if !recent.is_empty() {
        c = c.push(section("Recently played", album_grid(app, recent, true)));
    }
    let most: Vec<&Album> = app.cache.top_albums.iter().filter_map(|id| lib.album(id)).collect();
    if most.len() >= 2 {
        c = c.push(section("Your most played", album_grid(app, most, true)));
    }
    c = c.push(section("Recently added", album_grid(app, lib.recently_added(6), true)));
    let pls: Vec<El> = app.playlists().iter().filter(|p| !matches!(p.kind, Kind::Household { .. })).take(6).map(|p| playlist_card(app, p)).collect();
    if !pls.is_empty() {
        c = c.push(section("Your playlists", Row::with_children(pls).spacing(4).wrap().into()));
    }
    let picks: Vec<&Album> = app.cache.picks.iter().filter_map(|id| lib.album(id)).collect();
    if picks.len() >= 3 {
        c = c.push(section("Picked for today", album_grid(app, picks, true)));
    }
    let artists: Vec<El> = lib.top_artists(6).into_iter().map(|n| artist_card(app, n)).collect();
    c = c.push(section("Artists", Row::with_children(artists).spacing(4).wrap().into()));
    c.into()
}

fn library(app: &App) -> El<'_> {
    let tab = |label: &'static str, t: LibTab| button(text(label).size(14).wrapping(text::Wrapping::None)).padding([7, 16]).style(ui::chip(app.lib_tab == t)).on_press(Message::LibTab(t));
    let head = column![ui::title("Library", 32), row![tab("Playlists", LibTab::Playlists), tab("Albums", LibTab::Albums), tab("Artists", LibTab::Artists), tab("Downloaded", LibTab::Downloaded)].spacing(8)].spacing(14);
    let body: El = match app.lib_tab {
        LibTab::Albums => album_grid(app, app.lib.albums_by_title(), true),
        LibTab::Playlists => {
            let new = card(
                container(ic(icon::ADD, 40)).width(164).height(164).center_x(164).center_y(164).style(|_| container::Style { background: Some(ui::SURFACE_3.into()), border: iced::border::rounded(6.0), ..Default::default() }).into(),
                "New playlist".into(),
                "Make one".into(),
                Message::NewPlaylist(vec![]),
            );
            let mut cards = vec![new];
            cards.extend(app.playlists().iter().map(|p| playlist_card(app, p)));
            Row::with_children(cards).spacing(4).wrap().into()
        }
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
        LibTab::Downloaded => pages::downloaded(app),
    };
    column![head, body].spacing(20).into()
}

fn album_page<'a>(app: &'a App, id: &str) -> El<'a> {
    let Some(a) = app.lib.album(id) else { return ui::dim("This album isn't on the server any more.", 15).into() };
    let tracks = app.lib.album_tracks(id);
    let ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
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
    let extra = vec![download_button(app, "album", id, &ids), round_icon(icon::PLAYLIST, false, "Add to a playlist", Some(Message::AddToPlaylist(ids.clone())))];
    info = info.push(ui::gap(10)).push(header_buttons(Message::PlayAlbum(id.to_string(), 0), Message::ShuffleAlbum(id.to_string()), extra));
    let head = row![cover_of(app, Some(a), 220.0), info].spacing(28).align_y(Alignment::End);
    let mut c = column![head, track_list(app, &tracks, &a.title, true, false, Rows::Plain)].spacing(28);
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
    let n_albums = p.albums.len();
    let sub = if n_albums == 0 { "Appears on albums".to_string() } else { format!("{n_albums} album{} · {} songs", if n_albums == 1 { "" } else { "s" }, p.songs.len()) };
    let page_name = p.name.clone();
    let head = column![
        ui::dim("Artist", 13),
        ui::title(page_name.clone(), 48),
        ui::dim(sub, 15),
        ui::gap(6),
        header_buttons(Message::PlayTracks(ids.clone(), 0, page_name.clone()), Message::ShuffleTracks(ids, page_name.clone()), vec![]),
    ]
    .spacing(6);
    let mut c = column![head].spacing(28);
    if !p.songs.is_empty() {
        let top: Vec<&Track> = p.songs.iter().take(5).copied().collect();
        c = c.push(section("Songs", track_list(app, &top, &page_name, false, true, Rows::Plain)));
    }
    if !p.albums.is_empty() {
        c = c.push(section("Albums", album_grid(app, p.albums.clone(), false)));
    }
    if !p.appears_on.is_empty() {
        c = c.push(section("Appears on", album_grid(app, p.appears_on.clone(), true)));
    }
    c.into()
}

fn search_page(app: &App) -> El<'_> {
    let input = text_input("Artists, albums, songs, playlists", &app.search)
        .on_input(Message::SearchChanged)
        .padding(12)
        .size(16)
        .icon(text_input::Icon { font: ui::ICONS, code_point: icon::SEARCH, size: Some(16.into()), spacing: 10.0, side: text_input::Side::Left })
        .style(ui::input);
    let chip = |label: &'static str, t: SearchType| button(text(label).size(14).wrapping(text::Wrapping::None)).padding([7, 16]).style(ui::chip(app.search_type == t)).on_press(Message::SearchType(t));
    let chips = row![
        chip("All", SearchType::All),
        chip("Artists", SearchType::Artists),
        chip("Albums", SearchType::Albums),
        chip("Songs", SearchType::Songs),
        chip("Playlists", SearchType::Playlists)
    ]
    .spacing(8);
    let mut c = column![ui::title("Search", 32), input, chips].spacing(18);
    let q = app.search.trim();
    if q.is_empty() {
        let all = app.lib.recently_added(usize::MAX);
        if all.is_empty() {
            return c.push(ui::dim("No music yet.", 15)).into();
        }
        return c.push(section("Everything on your server", album_grid(app, all, true))).into();
    }
    let ty = app.search_type;
    let all = ty == SearchType::All;
    let show = |t: SearchType| all || ty == t;
    let lists = app.playlists();
    // worked out once per change of the words (see App::refresh_cache), not on every redraw
    let r = match &app.cache.search {
        Some((cq, artists, albums, songs)) if cq == q => search::Results {
            artists: artists.clone(),
            albums: albums.iter().filter_map(|id| app.lib.album(id)).collect(),
            songs: songs.iter().take(200).filter_map(|id| app.lib.track(id)).collect(),
            playlists: vec![],
        },
        _ => search::Results::default(),
    };
    let qn = search::norm(q);
    let found_lists: Vec<&volvix_core::playlists::PlaylistView> = lists.iter().filter(|p| qn.split_whitespace().all(|w| search::norm(&format!("{} {}", p.name, p.description)).contains(w))).collect();
    let mut any = false;
    if show(SearchType::Artists) && !r.artists.is_empty() {
        any = true;
        let n = if all { 6 } else { usize::MAX };
        let cards: Vec<El> = r.artists.iter().take(n).map(|a| artist_card(app, a.clone())).collect();
        c = c.push(titled("Artists", Row::with_children(cards).spacing(4).wrap().into(), (all && r.artists.len() > 6).then_some(SearchType::Artists)));
    }
    if show(SearchType::Songs) && !r.songs.is_empty() {
        any = true;
        let n = if all { 5 } else { 200 };
        let list: Vec<&Track> = r.songs.iter().take(n).copied().collect();
        c = c.push(titled("Songs", track_list(app, &list, "Search", false, true, Rows::Plain), (all && r.songs.len() > 5).then_some(SearchType::Songs)));
    }
    if show(SearchType::Albums) && !r.albums.is_empty() {
        any = true;
        let n = if all { 12 } else { usize::MAX };
        c = c.push(titled("Albums", album_grid(app, r.albums.iter().take(n).copied(), true), (all && r.albums.len() > 12).then_some(SearchType::Albums)));
    }
    if show(SearchType::Playlists) && !found_lists.is_empty() {
        any = true;
        let cards: Vec<El> = found_lists.iter().map(|p| playlist_card(app, p)).collect();
        c = c.push(titled("Playlists", Row::with_children(cards).spacing(4).wrap().into(), None));
    }
    if !any {
        c = c.push(ui::dim(format!("Nothing in your library for \"{q}\"."), 15));
    }
    // music you don't have yet, right below (Lidarr)
    if let Some(more) = pages::get_more(app) {
        c = c.push(more);
    }
    c.into()
}

/// A search section, with "Show all" (which switches the filter) when All cuts it short.
fn titled<'a>(title: &'static str, body: El<'a>, more: Option<SearchType>) -> El<'a> {
    let mut head = row![ui::title(title, 22), horizontal_space()].align_y(Alignment::Center);
    if let Some(t) = more {
        head = head.push(button(text("Show all").size(13)).padding([5, 12]).style(ui::flat).on_press(Message::SearchType(t)));
    }
    column![head, body].spacing(12).into()
}

/* ---------- Now Playing and the player bar ---------- */

fn now_playing(app: &App) -> El<'_> {
    let Some(t) = app.status.track.as_deref().and_then(|id| app.lib.track(id)) else {
        return column![ui::title("Now playing", 32), ui::dim("Nothing playing. Pick an album or a song to start.", 15)].spacing(12).into();
    };
    let album = app.lib.album_of(t);
    let liked = app.is_liked(&t.id);
    // the cover shrinks with the window (up to 360 px), so the tabs on the right always fit
    let art = iced::widget::responsive(move |size| cover_of(app, album, size.width.min(360.0)));
    let left = column![
        container(art).width(Length::Fill).height(Length::Shrink).max_height(360),
        ui::gap(8),
        row![
            column![
                ui::title(clip(&t.title, 40), 28),
                button(text(t.artist.as_str()).size(16).color(ui::TEXT_2)).padding(0).style(ui::flat).on_press(Message::Go(Page::Artist(t.artist.clone()))),
                button(text(album.map(|a| a.title.as_str()).unwrap_or("")).size(14).color(ui::TEXT_3)).padding(0).style(ui::flat).on_press_maybe(album.map(|a| Message::Go(Page::Album(a.id.clone())))),
            ]
            .spacing(4)
            .width(Length::Fill),
            button(ic(if liked { icon::HEART_FILL } else { icon::HEART }, 20)).padding(8).style(ui::icon_btn(liked)).on_press(Message::ToggleLike(t.id.clone())),
            button(ic(icon::PLAYLIST, 18)).padding(8).style(ui::icon_btn(false)).on_press(Message::AddToPlaylist(vec![t.id.clone()])),
        ]
        .align_y(Alignment::Start),
    ]
    .spacing(6)
    .width(Length::FillPortion(2))
    .max_width(360);

    let tab = |label: &'static str, tb: NpTab| button(text(label).size(14).wrapping(text::Wrapping::None)).padding([7, 16]).style(ui::chip(app.np_tab == tb)).on_press(Message::NpTab(tb));
    let tabs = row![tab("Up next", NpTab::Queue), tab("Lyrics", NpTab::Lyrics), tab("Credits", NpTab::Credits)].spacing(8);
    let right: El = match app.np_tab {
        NpTab::Queue => {
            let pos = app.queue.position().unwrap_or(0);
            let mut list = Column::new().spacing(2);
            list = list.push(ui::dim(if app.queue.source.is_empty() { String::new() } else { format!("Playing from {}", app.queue.source) }, 13));
            let upcoming = app.queue.upcoming();
            if upcoming.is_empty() {
                list = list.push(ui::gap(8)).push(ui::dim("Nothing after this song.", 14));
            }
            for (k, id) in upcoming.iter().enumerate().take(100) {
                let Some(t) = app.lib.track(id) else { continue };
                let index = pos + 1 + k;
                let line = row![
                    cover_of(app, app.lib.album_of(t), 40.0),
                    column![text(clip(&t.title, 50)).size(14).color(ui::TEXT), ui::dim(clip(&t.artist, 50), 12)].spacing(1).width(Length::Fill),
                    button(ic(icon::CLOSE, 11)).style(ui::icon_btn(false)).padding(8).on_press(Message::RemoveFromQueue(index)),
                    ui::dim(format_duration(t.duration), 12),
                ]
                .spacing(12)
                .align_y(Alignment::Center);
                let song: El = button(line).width(Length::Fill).padding([5, 8]).style(ui::flat).on_press(Message::JumpQueue(index)).into();
                let r: El = row![drag_handle(crate::DragList::Queue, index), song].align_y(Alignment::Center).into();
                list = list.push(drop_target(app, &crate::DragList::Queue, index, r));
            }
            scrollable(list).style(ui::scroller).height(Length::Fill).into()
        }
        NpTab::Lyrics => pages::lyrics(app, &t.id),
        NpTab::Credits => pages::credits(app, t),
    };
    row![left, column![tabs, right].spacing(14).width(Length::FillPortion(3)).height(Length::Fill)].spacing(32).into()
}

fn player_bar(app: &App) -> El<'_> {
    let mut bar = column![];
    if let Some(e) = &app.status.error {
        bar = bar.push(container(text(e.as_str()).size(12)).padding([4, 10]).style(ui::error_box));
    }
    // the sides give up room in a narrow window, so the controls never overlap
    let main = iced::widget::responsive(move |size| player_row(app, size.width));
    container(bar.push(container(main).height(68)).spacing(4)).height(84).padding([8, 12]).center_y(84).style(ui::frame).into()
}

fn player_row(app: &App, width: f32) -> El<'_> {
    let narrow = width < 1000.0;
    let st = &app.status;
    let track = st.track.as_deref().and_then(|id| app.lib.track(id));
    let album = track.and_then(|t| app.lib.album_of(t));

    let left: El = match track {
        Some(t) => {
            let liked = app.is_liked(&t.id);
            let in_one = app.cache.current_in_playlist;
            row![
                marks::eq(st.playing, 12.0),
                button(
                    row![
                        cover_of(app, album, 56.0),
                        column![text(clip(&t.title, 28)).size(14).color(ui::TEXT).wrapping(text::Wrapping::None), ui::dim(clip(&t.artist, 32), 12).wrapping(text::Wrapping::None)].spacing(2).width(Length::Fill).clip(true)
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center)
                )
                .width(Length::Fill)
                .padding(4)
                .style(ui::flat)
                .on_press(Message::ToggleNowPlaying),
                button(ic(if liked { icon::HEART_FILL } else { icon::HEART }, 15)).padding(8).style(ui::icon_btn(liked)).on_press(Message::ToggleLike(t.id.clone())),
                // add to a playlist (a tick once it's in one of yours)
                button(ic(if in_one { icon::DOWNLOADED } else { icon::ADD }, 15)).padding(8).style(ui::icon_btn(in_one)).on_press(Message::AddToPlaylist(vec![t.id.clone()])),
            ]
            .spacing(4)
            .align_y(Alignment::Center)
            .into()
        }
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
    let pos = st.position.as_secs_f32().min(total.max(0.0));
    let time = |s: f32| ui::dim(format_duration(s as u32), 11);
    let progress = row![
        container(time(pos)).width(42).align_x(Alignment::End),
        // follows the mouse by itself; seeks when you let go
        El::from(marks::scrub(pos, total.max(1.0), Message::SeekTo)),
        container(if st.loading { ui::dim("…", 11) } else { time(total) }).width(42),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let center = column![controls, progress].spacing(4).align_x(Alignment::Center).width(Length::Fill).max_width(640);

    let vol_icon = if app.volume <= 0.001 { icon::MUTE } else { icon::VOLUME };
    let queue_on = app.now_playing && app.np_tab == NpTab::Queue;
    // where the sound goes: this device, a TV or speaker, or a listening party (lit while either is on)
    let playing_elsewhere = app.cast.is_some() || app.party.is_some();
    let right = row![
        horizontal_space(),
        button(ic(icon::DEVICES, 16)).padding(8).style(ui::icon_btn(playing_elsewhere)).on_press(Message::PlayOn),
        button(ic(icon::QUEUE, 16)).padding(8).style(ui::icon_btn(queue_on)).on_press(if queue_on { Message::ToggleNowPlaying } else { Message::NpTab(NpTab::Queue) }),
        button(ic(vol_icon, 16)).padding(8).style(ui::icon_btn(false)).on_press(Message::ToggleMute),
        container(El::from(marks::scrub(app.volume, 1.0, Message::Volume).on_change(Message::Volume))).width(if narrow { 64 } else { 110 }),
    ]
    .spacing(2)
    .align_y(Alignment::Center)
    .width(if narrow { 196 } else { 260 });

    // fixed sides, the controls take the middle
    row![container(left).width(if narrow { 250 } else { 300 }).clip(true), container(center).width(Length::Fill).center_x(Length::Fill), right].spacing(12).align_y(Alignment::Center).into()
}
