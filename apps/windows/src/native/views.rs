//! The window's frame and the music screens (Home, Library, Album, Artist, Search, Now Playing,
//! the player). Playlists, mixes, Get more music, household, notifications, settings and dialogs
//! are in pages.rs. Each takes the app state and returns what to draw; clicks come back as Messages.
use std::time::Duration;

use iced::widget::{button, column, container, horizontal_space, mouse_area, opaque, row, scrollable, stack, text, text_input, Column, Row, Space};
use iced::{Alignment, Color, Element, Length, Padding};
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
        Screen::Loading => ui::splash(),
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
                background: hover.then(|| if close { iced::Color::from_rgb8(0xC4, 0x2B, 0x1C) } else { ui::HOVER() }.into()),
                text_color: if hover && close { iced::Color::WHITE } else { ui::TEXT_2() },
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
    let first_albums = |tracks: &[String]| playlist_albums(app, tracks, 4).into_iter().map(|a| a.id.clone()).collect::<Vec<String>>();
    if let Some(t) = app.status.track.as_deref().and_then(|id| lib.track(id)) {
        ids.push(t.album_id.clone());
    }
    // the sidebar always shows your playlists' pictures
    ids.extend(app.playlists().iter().flat_map(|p| first_albums(&p.tracks)));
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
                ids.extend(app.playlists().iter().flat_map(|p| first_albums(&p.tracks)));
            }
            Page::Library => match app.lib_tab {
                LibTab::Albums | LibTab::Songs => ids.extend(lib.albums().iter().map(|a| a.id.clone())),
                LibTab::Browse => {}
                LibTab::Artists => ids.extend(lib.artists().iter().filter_map(|n| lib.artist_cover_album(n)).map(|a| a.id.clone())),
                LibTab::Playlists => ids.extend(app.playlists().iter().flat_map(|p| first_albums(&p.tracks))),
                LibTab::Downloaded => {
                    for s in &app.saved {
                        if s.kind == "album" {
                            ids.push(s.id.clone());
                        } else if let Some(p) = app.playlist(&s.id) {
                            ids.extend(first_albums(&p.tracks));
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
            Page::Notifications | Page::Profile | Page::Members | Page::Settings => {}
            Page::GetMusic | Page::Discover(_) => {}
            Page::Browse(..) => ids.extend(lib.albums().iter().map(|a| a.id.clone())),
            Page::Rewind => {
                // the top songs' covers
                if let Some(y) = pages::rewind_year(app) {
                    ids.extend(pages::rewind_songs(app, y).iter().take(5).map(|(t, _)| t.album_id.clone()));
                }
            }
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
    if handle.is_none() {
        if let Some((a, look)) = album.and_then(|a| app.lib.look(&a.id).map(|l| (a, l))) {
            if !a.cover {
                return ui::cover_look(&a.title, size, 6.0, look);
            }
        }
    }
    ui::cover(handle, album.map(|a| a.title.as_str()).unwrap_or(""), size)
}

/// A playlist's cover: its first song's album (Liked Songs gets a heart).
pub fn playlist_cover<'a>(app: &'a App, tracks: &[String], liked: bool, name: &str, size: f32) -> El<'a> {
    if liked {
        return container(ic(icon::HEART_FILL, (size * 0.36) as u16).color(iced::Color::WHITE)).width(size).height(size).center_x(size).center_y(size).style(ui::mix_tile("liked")).into();
    }
    // nothing in it: a tile with the playlist's name; one album: its cover; more: a collage like the website's
    let albums = playlist_albums(app, tracks, 4);
    let part = |a: &Album, w: f32, h: f32| -> El<'a> { ui::cover_box(app.cover_images.get(&(a.id.clone(), COVER)), &a.title, w, h, 0.0) };
    let half = ((size - 1.0) / 2.0).floor();
    match albums.len() {
        0 => ui::cover(None, name, size),
        1 => cover_of(app, Some(albums[0]), size),
        2 => row![part(albums[0], half, size), part(albums[1], half, size)].spacing(1).into(),
        3 => row![part(albums[0], half, size), column![part(albums[1], half, half), part(albums[2], half, half)].spacing(1)].spacing(1).into(),
        _ => column![row![part(albums[0], half, half), part(albums[1], half, half)].spacing(1), row![part(albums[2], half, half), part(albums[3], half, half)].spacing(1)].spacing(1).into(),
    }
}

/// A playlist's picture: the one someone chose for it, or (until it arrives, or if none) the usual one.
pub fn playlist_art<'a>(app: &'a App, p: &volvix_core::playlists::PlaylistView, size: f32) -> El<'a> {
    if let Some(h) = app.playlist_picture(&p.id, p.cover) {
        return ui::cover(Some(h), &p.name, size);
    }
    if let Some(l) = &p.look {
        return ui::cover_look(&p.name, size, 6.0, l);
    }
    playlist_cover(app, &p.tracks, p.kind == Kind::Liked, &p.name, size)
}

/// The albums a playlist's songs come from, in order, up to `max` (its picture).
pub fn playlist_albums<'a>(app: &'a App, tracks: &[String], max: usize) -> Vec<&'a Album> {
    let mut out: Vec<&Album> = vec![];
    for id in tracks {
        if let Some(a) = app.lib.track(id).and_then(|t| app.lib.album_of(t)) {
            if !out.iter().any(|x| x.id == a.id) {
                out.push(a);
                if out.len() == max {
                    break;
                }
            }
        }
    }
    out
}

pub fn section<'a>(title: &'a str, body: El<'a>) -> El<'a> {
    column![ui::title(title, 22), body].spacing(12).into()
}

pub fn card<'a>(art: El<'a>, title: String, sub: String, on: Message) -> El<'a> {
    let size = 164.0;
    button(column![art, text(clip(&title, 22)).size(14).color(ui::TEXT()), ui::dim(clip(&sub, 26), 13)].spacing(4).width(size)).padding(8).style(ui::flat).on_press(on).into()
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
    card(playlist_art(app, &p, 164.0), p.name.clone(), sub, Message::Go(Page::Playlist(p.id.clone())))
}

/// How a song list behaves: plain, or a playlist you can edit (drag to reorder; remove songs
/// unless it's Liked Songs, where the heart does that).
#[derive(Clone, Copy)]
pub enum Rows<'a> {
    Plain,
    Playlist { id: &'a str, liked: bool },
}

/// Song rows are this tall on the website (40 cover with 6 above and below, plus its line spacing); queue rows are 52.
pub const TRACK_ROW_H: f32 = 52.6;
pub const QUEUE_ROW_H: f32 = 52.0;
/// The label between two parts of the queue ("Next from …") is this tall.
pub const LABEL_H: f32 = 38.8;

/// A number that tells one list on the page from another (which row the pointer is over is kept per list).
pub fn list_key(name: &str, kind: u64) -> u64 {
    name.bytes().fold(0xcbf2_9ce4_8422_2325u64 ^ kind, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// A list whose rows can be picked up anywhere and moved, like the website's.
pub struct DragRows<'a> {
    pub list: crate::DragList,
    pub key: u64,
    /// each row's content, and what a plain click on it does
    pub rows: Vec<(El<'a>, Message)>,
    /// the picked-up row's content again, without working buttons (only needed while this list is being dragged)
    pub ghost: Option<El<'a>>,
    /// a label that sits before one of the rows (the queue's "Next from …")
    pub divider: Option<(usize, El<'a>)>,
    pub row_h: f32,
    /// space left of the grip, and between the grip and the row's content
    pub lead: (f32, f32),
    /// the hand cursor over rows (playlists); otherwise the pointing finger (the queue's rows are buttons on the website)
    pub grab_cursor: bool,
}

/// The rows of a list you can reorder. The row under the pointer lights up and shows its grip. Picked up (after the
/// pointer has moved a little), it follows the pointer as a lifted card while the others slide out of its way, and it
/// lands in the gap. Letting go without having moved is a click.
pub fn drag_list<'a>(app: &'a App, r: DragRows<'a>) -> El<'a> {
    let n = r.rows.len();
    let h = r.row_h;
    let (lead, grip_gap) = r.lead;
    let div_at = r.divider.as_ref().map(|d| d.0);
    let drag = app.drag.as_ref().filter(|d| d.list == r.list);
    let lifted = drag.filter(|d| d.active && d.from < n && d.count == n);
    let card = move |content: El<'a>, lit: bool, floating: bool| -> El<'a> {
        let grip: El<'a> = if lit { ui::wi("grip", if h > 52.5 { 18.0 } else { 16.0 }, Color { a: 0.85, ..ui::TEXT_3() }) } else { Space::new(16, 16).into() };
        container(row![container(grip).width(22).center_x(22), content].spacing(grip_gap).align_y(Alignment::Center))
            .padding(Padding { left: lead, right: 12.0, ..Padding::ZERO })
            .width(Length::Fill)
            .height(h)
            .center_y(h)
            .style(move |_| container::Style {
                background: if floating { Some(Color { a: 0.9, ..ui::SURFACE_3() }.into()) } else { lit.then(|| ui::HOVER().into()) },
                border: iced::border::rounded(if floating { 10.0 } else { 8.0 }),
                shadow: if floating { iced::Shadow { color: Color { a: 0.45, ..Color::BLACK }, offset: iced::Vector::new(0.0, 12.0), blur_radius: 32.0 } } else { iced::Shadow::default() },
                ..Default::default()
            })
            .into()
    };
    let mut divider = r.divider;
    let mut col = Column::new();
    match lifted {
        None => {
            for (i, (content, click)) in r.rows.into_iter().enumerate() {
                if div_at == Some(i) {
                    if let Some((_, label)) = divider.take() {
                        col = col.push(label);
                    }
                }
                let lit = app.hover_row == Some((r.key, i));
                col = col.push(
                    mouse_area(card(content, lit, false))
                        .interaction(if r.grab_cursor { iced::mouse::Interaction::Grab } else { iced::mouse::Interaction::Pointer })
                        .on_press(Message::DragStart(r.list.clone(), i, n, div_at.map(|at| (at, LABEL_H)), h, Some(Box::new(click))))
                        .on_enter(Message::RowHover(Some((r.key, i))))
                        .on_exit(Message::RowLeave(r.key, i)),
                );
            }
            let area = mouse_area(col);
            // pressed, not yet moved far: watch the pointer to see whether this becomes a drag
            if drag.is_some() {
                area.on_move(|p| Message::DragAt(p.y)).into()
            } else {
                area.into()
            }
        }
        Some(d) => {
            // the other rows in order, with room opening where the card would land (and closing where it was)
            let label_before = div_at.map(|at| if d.from < at { at - 1 } else { at });
            let gap = |k: usize| -> El<'a> { Space::new(Length::Fill, d.gaps.get(k).copied().unwrap_or(0.0) * h).into() };
            let mut k = 0;
            for (i, (content, _)) in r.rows.into_iter().enumerate() {
                if i == d.from {
                    continue;
                }
                if label_before == Some(k) {
                    // a row that stays one of your picks lands above the label; one that leaves them, below it
                    let label = divider.take().map(|x| x.1);
                    if div_at.map(|at| d.from < at).unwrap_or(false) {
                        col = col.push(gap(k)).push_maybe(label);
                    } else {
                        col = col.push_maybe(label).push(gap(k));
                    }
                } else {
                    col = col.push(gap(k));
                }
                col = col.push(card(content, false, false));
                k += 1;
            }
            if label_before == Some(k) {
                col = col.push_maybe(divider.take().map(|x| x.1));
            }
            col = col.push(gap(k));
            let ghost: El<'a> = r.ghost.unwrap_or_else(|| Space::new(0, 0).into());
            let floating = container(card(ghost, true, true)).padding(Padding { top: d.card_top(), ..Padding::ZERO });
            mouse_area(stack![col, floating]).interaction(iced::mouse::Interaction::Grabbing).on_move(|p| Message::DragAt(p.y)).into()
        }
    }
}

/// What goes in front of each song: its place in the list, its track number (albums), or nothing (search results).
#[derive(Clone, Copy, PartialEq)]
pub enum Numbers {
    Place,
    TrackNo,
    Blank,
}

/// The little marks beside a song: downloaded (a red dot with an arrow) and explicit (E).
fn song_marks<'a>(app: &App, t: &Track) -> Vec<El<'a>> {
    let mut v: Vec<El<'a>> = vec![];
    if app.downloaded.contains(&t.id) {
        v.push(container(ui::wi("download", 9.0, Color::WHITE)).width(14).height(14).center_x(14).center_y(14).style(|_| container::Style { background: Some(ui::ACCENT().into()), border: iced::border::rounded(7.0), ..Default::default() }).into());
    }
    if t.explicit {
        v.push(explicit_badge());
    }
    v
}

pub fn explicit_badge<'a>() -> El<'a> {
    container(text("E").size(10).color(ui::FRAME())).padding([0, 4]).style(|_| container::Style { background: Some(ui::TEXT_2().into()), border: iced::border::rounded(3.0), ..Default::default() }).into()
}

/// A list of songs, like the website's: number (or the playing bars; a play mark under the pointer), cover, title with
/// "artist · album" under it, then like, add to playlist and ⋯, and the length. Clicking a row plays the list from there.
pub fn track_list<'a>(app: &'a App, tracks: &[&'a Track], source: &str, numbers: Numbers, show_album: bool, rows: Rows<'_>) -> El<'a> {
    let ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
    let album_page = numbers == Numbers::TrackNo;
    let key = list_key(source, numbers as u64 + if matches!(rows, Rows::Playlist { .. }) { 16 } else { 0 });
    let discs = tracks.iter().map(|t| t.disc.max(1)).max().unwrap_or(1);
    let current = app.status.track.as_deref();
    let h = TRACK_ROW_H;
    // one row's content; `ghost` = the copy that follows the pointer while it's being moved (its buttons are just pictures)
    let content = |i: usize, t: &'a Track, hovered: bool, ghost: bool| -> El<'a> {
        let playing = current == Some(t.id.as_str());
        let album = app.lib.album_of(t);
        let lead: El<'a> = if hovered {
            // under the pointer: play (or pause, on the song that's playing)
            ui::play_pause(playing && app.status.playing, 14.0, ui::TEXT())
        } else if playing {
            marks::eq(app.status.playing, 12.0)
        } else {
            match numbers {
                Numbers::Place => ui::dim((i + 1).to_string(), 15).into(),
                Numbers::TrackNo => ui::dim(if t.no > 0 { t.no.to_string() } else { String::new() }, 15).into(),
                Numbers::Blank => Space::new(0, 0).into(),
            }
        };
        let title = text(clip(&t.title, 60)).size(15).color(if playing { ui::ACCENT() } else { ui::TEXT() }).wrapping(text::Wrapping::None);
        let marks = song_marks(app, t);
        // on an album page a song by the album's own artist shows just its title (with the marks beside it)
        let own_artist = album_page && album.map(|a| a.artist == t.artist).unwrap_or(false);
        let words: El<'a> = if own_artist {
            let mut line = Row::new().spacing(6).align_y(Alignment::Center).push(title);
            for m in marks {
                line = line.push(m);
            }
            line.into()
        } else {
            let mut sub = t.artist.clone();
            if show_album {
                if let Some(a) = album {
                    sub = format!("{sub} · {}", a.title);
                }
            }
            let mut under = Row::new().spacing(4).align_y(Alignment::Center);
            for m in marks {
                under = under.push(m);
            }
            under = under.push(ui::dim(clip(&sub, 80), 13).wrapping(text::Wrapping::None));
            column![title, under].spacing(1).into()
        };
        let liked = app.is_liked(&t.id);
        let in_playlist = match rows {
            Rows::Playlist { id: pl, liked: false } => Some((pl.to_string(), i)),
            _ => None,
        };
        let menu_open = matches!(&app.pop_menu, Some(crate::PopMenu { kind: crate::MenuKind::Track { id, in_playlist: p }, .. }) if *id == t.id && *p == in_playlist);
        // 32 px round buttons, 2 apart: like, add to a playlist, and the ⋯ menu with everything else
        let small = |name: &'static str, size: f32, on: bool, hint: &'static str, m: Message| -> El<'a> {
            let face = container(ui::wi_btn(name, size, on)).width(32).height(32).center_x(32).center_y(32);
            if ghost {
                container(ui::wi(name, size, if on { ui::ACCENT() } else { ui::TEXT_2() })).width(32).height(32).center_x(32).center_y(32).into()
            } else {
                ui::tip(button(face).padding(0).style(ui::icon_btn(on)).on_press(m), hint)
            }
        };
        let mut line = Row::new().spacing(12).align_y(Alignment::Center).push(container(lead).width(32).center_x(32));
        if !album_page {
            line = line.push(ui::cover_box(album.and_then(|a| app.cover_images.get(&(a.id.clone(), COVER))), album.map(|a| a.title.as_str()).unwrap_or(""), 40.0, 40.0, 4.0));
        }
        line.push(container(words).width(Length::Fill).clip(true))
            .push(
                row![
                    small(if liked { "heartFill" } else { "heart" }, 16.0, liked, if liked { "Remove from Liked Songs" } else { "Add to Liked Songs" }, Message::ToggleLike(t.id.clone())),
                    small("plus", 16.0, false, "Add to playlist", Message::AddToPlaylist(vec![t.id.clone()])),
                    small("more", 18.0, menu_open, "More", Message::PopMenuOpen(crate::MenuKind::Track { id: t.id.clone(), in_playlist })),
                ]
                .spacing(2)
                .align_y(Alignment::Center),
            )
            .push(container(ui::dim(format_duration(t.duration), 13)).width(52).align_x(Alignment::End))
            .into()
    };
    let hovered = |i: usize| app.hover_row == Some((key, i));
    match rows {
        Rows::Playlist { id: pl, .. } => {
            let list = crate::DragList::Playlist(pl.to_string());
            let moving = app.drag.as_ref().filter(|d| d.list == list && d.active).map(|d| d.from);
            let ghost = moving.and_then(|from| tracks.get(from).map(|t| content(from, t, true, true)));
            let rows: Vec<(El<'a>, Message)> = tracks.iter().enumerate().map(|(i, t)| (content(i, t, hovered(i) && moving.is_none(), false), Message::PlayTracks(ids.clone(), i, source.to_string()))).collect();
            drag_list(app, DragRows { list, key, rows, ghost, divider: None, row_h: h, lead: (12.0, 12.0), grab_cursor: true })
        }
        Rows::Plain => {
            let mut col = Column::new();
            let mut last_disc = 0;
            for (i, t) in tracks.iter().enumerate() {
                if album_page && discs > 1 && t.disc.max(1) != last_disc {
                    last_disc = t.disc.max(1);
                    col = col.push(
                        container(row![ui::wi("disc", 16.0, ui::TEXT_2()), text(format!("Disc {last_disc}")).size(13).font(ui::FONT_BOLD).color(ui::TEXT_2())].spacing(8).align_y(Alignment::Center))
                            .padding(Padding { top: if i == 0 { 0.0 } else { 22.0 }, bottom: 6.0, left: 12.0, right: 0.0 }),
                    );
                }
                let lit = hovered(i);
                let card = container(content(i, t, lit, false))
                    .padding([0, 12])
                    .width(Length::Fill)
                    .height(h)
                    .center_y(h)
                    .style(move |_| container::Style { background: lit.then(|| ui::HOVER().into()), border: iced::border::rounded(8.0), ..Default::default() });
                col = col.push(
                    mouse_area(card)
                        .interaction(iced::mouse::Interaction::Pointer)
                        .on_press(Message::PlayTracks(ids.clone(), i, source.to_string()))
                        .on_enter(Message::RowHover(Some((key, i))))
                        .on_exit(Message::RowLeave(key, i)),
                );
            }
            col.into()
        }
    }
}

/// The ⋯ pop-up (a song's or a playlist's), opened where the pointer was, like the website's popover.
pub fn pop_menu<'a>(app: &'a App, m: &'a crate::PopMenu) -> El<'a> {
    let item = |name: &'static str, label: &'static str, msg: Message, danger: bool| -> El<'a> {
        button(row![ui::wi(name, 18.0, if danger { ui::ACCENT() } else { ui::TEXT_2() }), text(label).size(14).font(ui::FONT_BOLD).color(if danger { ui::ACCENT() } else { ui::TEXT() })].spacing(12).align_y(Alignment::Center))
            .width(Length::Fill)
            .padding([9, 12])
            .style(ui::flat)
            .on_press(Message::PopMenuDo(Box::new(msg)))
            .into()
    };
    let line = || -> El<'a> { container(Space::new(Length::Fill, 1)).width(Length::Fill).height(1).style(|_: &iced::Theme| container::Style { background: Some(ui::BORDER().into()), ..Default::default() }).into() };
    let mut items: Vec<El<'a>> = vec![];
    match &m.kind {
        crate::MenuKind::Track { id, in_playlist } => {
            items.push(item("queue", "Play next", Message::PlayNext(id.clone()), false));
            items.push(item("plus", "Add to queue", Message::AddToQueue(id.clone()), false));
            items.push(item("music", "Add to playlist", Message::AddToPlaylist(vec![id.clone()]), false));
            if let Some((pl, index)) = in_playlist {
                items.push(item("x", "Remove from this playlist", Message::RemoveFromPlaylist(pl.clone(), *index, id.clone()), false));
            }
            let got = app.is_saved("track", id);
            items.push(item(if got { "check" } else { "download" }, if got { "Remove download" } else { "Download" }, Message::ToggleSaved("track".into(), id.clone()), false));
            if let Some(t) = app.lib.track(id) {
                items.push(item("disc", "Go to album", Message::Go(Page::Album(t.album_id.clone())), false));
                items.push(item("book", "Credits", Message::ShowCredits(id.clone()), false));
                items.push(item("user", "Go to artist", Message::Go(Page::Artist(t.artist.clone())), false));
            }
        }
        crate::MenuKind::Album(id) => {
            if let Some(a) = app.lib.album(id) {
                let ids: Vec<String> = app.lib.album_tracks(id).iter().map(|t| t.id.clone()).collect();
                items.push(item("queue", "Add to queue", Message::QueueMany(ids.clone()), false));
                items.push(item("queue", "Play next", Message::PlayNextMany(ids.clone()), false));
                items.push(item("book", "Credits & info", Message::ShowAlbumInfo(id.clone()), false));
                if !a.cover {
                    items.push(item("settings", "Customize cover", Message::CoverOpen(id.clone()), false));
                }
                if !a.cover || a.custom_cover {
                    items.push(item("disc", "Change picture", Message::PickAlbumPicture(id.clone()), false));
                }
                if a.custom_cover {
                    items.push(item("x", "Remove picture", Message::RemoveAlbumPicture(id.clone()), false));
                }
                if app.can("share") {
                    items.push(item("send", "Share link…", Message::ShareOpen("album".into(), a.title.clone(), ids), false));
                }
                items.push(item("user", "Go to artist", Message::Go(Page::Artist(a.artist.clone())), false));
            }
        }
        crate::MenuKind::Playlist(id) => {
            if let Some(p) = app.playlist(id) {
                if p.can_manage() {
                    items.push(item("settings", "Edit details", Message::EditPlaylist(p.id.clone()), false));
                    items.push(item("disc", "Change picture", Message::PickPlaylistPicture(p.id.clone()), false));
                    if p.cover > 0 {
                        items.push(item("x", "Remove picture", Message::RemovePlaylistPicture(p.id.clone()), false));
                    }
                    if !p.is_shared() {
                        items.push(item("settings", "Customize cover", Message::CoverOpenPlaylist(p.id.clone()), false));
                    }
                }
                if p.can_manage() {
                    items.push(item("users", "Who can edit", Message::CollabOpen(p.id.clone()), false));
                }
                if !p.tracks.is_empty() {
                    items.push(item("queue", "Add to queue", Message::QueueMany(p.tracks.clone()), false));
                    if app.can("share") {
                        items.push(item("send", "Share link…", Message::ShareOpen("playlist".into(), p.name.clone(), p.tracks.clone()), false));
                    }
                }
                if p.can_manage() {
                    items.push(line());
                    items.push(item("x", "Delete playlist", Message::DeletePlaylist(p.id.clone()), true));
                }
            }
        }
    }
    let (win_w, win_h) = crate::WINDOW_SIZE.lock().map(|w| *w).unwrap_or((1200.0, 800.0));
    let (w, h) = (232.0, items.len() as f32 * 38.0 + 16.0);
    let left = (m.at.0 + 14.0 - w).clamp(8.0, (win_w - w - 8.0).max(8.0));
    let top = if m.at.1 + 14.0 + h > win_h - 8.0 { (m.at.1 - h - 8.0).max(8.0) } else { m.at.1 + 14.0 };
    let card = container(Column::with_children(items).spacing(0)).width(w).padding(6).style(ui::dialog);
    mouse_area(container(opaque(card)).width(Length::Fill).height(Length::Fill).padding(Padding { left, top, ..Padding::ZERO })).on_press(Message::PopMenuClose).into()
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
            st.text_color = ui::ACCENT();
        }
        st
    });
    ui::tip(b.on_press_maybe(m), tip)
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
            background: Some(if on && matches!(s, button::Status::Hovered) { ui::SURFACE_3() } else { ui::FRAME() }.into()),
            text_color: if on { ui::TEXT() } else { ui::TEXT_3() },
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
    } else if matches!(app.page, Page::Profile | Page::Members | Page::Settings) {
        // the Account page keeps its tabs on screen: only the part next to them scrolls
        container(page(app)).padding([8, 28]).width(Length::Fill).height(Length::Fill).into()
    } else {
        scrollable(container(page(app)).padding([8, 28]).width(Length::Fill))
            .id(crate::main_scroller())
            .on_scroll(|v| Message::Scrolled(0, v.absolute_offset().y))
            .style(ui::scroller)
            .height(Length::Fill)
            .into()
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
    if let Some(m) = &app.pop_menu {
        layers = layers.push(pop_menu(app, m));
    }
    if app.account_menu {
        layers = layers.push(opaque(mouse_area(container(account_menu(app)).width(Length::Fill).height(Length::Fill).align_x(Alignment::Start).align_y(Alignment::End).padding(Padding { left: 16.0, bottom: 164.0, ..Padding::ZERO })).on_press(Message::AccountMenu(false))));
    }
    if let Some(d) = &app.dialog {
        if matches!(d, crate::Dialog::PlayOn) {
            // the website's "Play on" is a small pop-up above the player bar, not a centred dialog
            let card = container(pages::dialog(app, d)).width(300).padding(6).style(ui::dialog);
            layers = layers.push(opaque(mouse_area(container(opaque(card)).width(Length::Fill).height(Length::Fill).align_x(Alignment::End).align_y(Alignment::End).padding(Padding { right: 150.0, bottom: 96.0, ..Padding::ZERO })).on_press(Message::DialogCancel)));
            return layers.into();
        }
        let wide = if matches!(d, crate::Dialog::CoverLook { .. }) { 540 } else { 440 };
        let card = container(pages::dialog(app, d)).width(wide).padding(24).style(ui::dialog);
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
    let head = row![ui::dim("Playlists", 13), horizontal_space(), ui::tip(button(ic(icon::ADD, 13)).padding(6).style(ui::icon_btn(false)).on_press(Message::NewPlaylist(vec![])), "New playlist")].align_y(Alignment::Center);
    let mut list = Column::new().spacing(0);
    for p in app.playlists().into_iter().filter(|p| !matches!(p.kind, Kind::Household { .. })) {
        let active = is(&Page::Playlist(p.id.clone()));
        // the playlist's picture (its first album; a heart for Liked Songs), like the website
        let mark = playlist_art(app, &p, 32.0);
        list = list.push(button(row![mark, text(clip(&p.name, 22)).size(14)].spacing(10).align_y(Alignment::Center)).width(Length::Fill).padding([4, 10]).style(ui::nav(active)).on_press(Message::Go(Page::Playlist(p.id.clone()))));
    }
    let playlists = column![container(head).padding(Padding { top: 0.0, bottom: 0.0, left: 14.0, right: 4.0 }), scrollable(list).style(ui::scroller).height(Length::Fill)].spacing(2);

    let who = app.me.as_ref().map(|m| m.user.name.clone()).unwrap_or_default();
    // your picture (or letter), not a gear: the button is your account; 52 tall like the website's
    let (me_id, me_av) = app.me.as_ref().map(|m| (m.user.id.clone(), m.user.avatar)).unwrap_or_default();
    let bottom = button(row![ui::avatar_pic(app.avatar_handle(&me_id, me_av), "u_me", &who, 36.0), column![text(clip(&who, 22)).size(13).font(ui::FONT_BOLD), ui::dim("Settings and account", 11)].spacing(1)].spacing(12).align_y(Alignment::Center))
        .width(Length::Fill)
        .padding([8, 14])
        .style(ui::nav(is(&Page::Settings) || is(&Page::Profile) || is(&Page::Members)))
        .on_press(Message::AccountMenu(!app.account_menu));
    container(column![nav, ui::gap(10), playlists, bottom].spacing(4)).width(240).height(Length::Fill).padding(8).style(ui::panel).into()
}

/// The menu the profile button opens, like the website's: who you are, then Account, Settings and Sign out.
fn account_menu(app: &App) -> El<'_> {
    let user = app.me.as_ref().map(|m| m.user.clone()).unwrap_or_default();
    // the name card is the "Account" button: it opens your profile
    let head = button(
        row![ui::avatar_pic(app.avatar_handle(&user.id, user.avatar), "u_me", &user.name, 40.0), column![text(clip(&user.name, 22)).size(15).font(ui::FONT_BOLD), ui::dim(if user.admin { "Admin" } else { "Member" }, 12)].spacing(1)]
            .spacing(12)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([8, 12])
    .style(ui::flat)
    .on_press(Message::Go(Page::Profile));
    let item = |name: &'static str, label: &'static str, m: Message| -> El<'static> {
        button(row![ui::wi(name, 18.0, ui::TEXT_2()), text(label).size(14).font(ui::FONT_BOLD)].spacing(12).align_y(Alignment::Center)).width(Length::Fill).padding([9, 10]).style(ui::flat).on_press(m).into()
    };
    let line = || container(Space::new(Length::Fill, 1)).width(Length::Fill).height(1).style(|_: &iced::Theme| container::Style { background: Some(ui::BORDER().into()), ..Default::default() });
    let may_members = user.admin || user.perms.iter().any(|p| p == "members");
    let mut card = column![head, container(line()).padding([4, 6])].spacing(2);
    if may_members {
        card = card.push(item("family", "Members", Message::Go(Page::Members)));
    }
    card = card
        .push(item("settings", "Settings", Message::Go(Page::Settings)))
        .push(item("history", "Rewind", Message::Go(Page::Rewind)))
        .push(container(line()).padding([4, 6]))
        .push(item("logout", "Sign out", Message::SignOut));
    // as wide as the button it opens from (the sidebar is 240 wide with 8 of padding each side)
    opaque(container(card).width(224).padding(6).style(ui::dialog)).into()
}

fn bell(app: &App) -> El<'_> {
    let n = app.notes.unread;
    let mut r = row![ic(icon::BELL, 16)].spacing(4).align_y(Alignment::Center);
    if n > 0 {
        r = r.push(container(text(if n > 99 { "99+".to_string() } else { n.to_string() }).size(11)).padding([1, 6]).style(ui::badge));
    }
    ui::tip(button(r).padding(8).style(ui::icon_btn(app.page == Page::Notifications && !app.now_playing)).on_press(Message::Go(Page::Notifications)), "Notifications")
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
        Page::Discover(id) => pages::discover_page(app, id),
        Page::Household => pages::household(app),
        Page::Notifications => pages::notifications(app),
        Page::Profile | Page::Members | Page::Settings => pages::account(app),
        Page::Rewind => pages::rewind(app),
        Page::Browse(kind, value) => browse_page(app, kind, value),
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

fn home<'a>(app: &'a App) -> El<'a> {
    let lib = &app.lib;
    if lib.is_empty() {
        return column![ui::title(greeting(), 32), ui::dim("No music yet. Music added to the server shows up here on its own.", 15)].spacing(12).into();
    }
    // quick tiles: jump back in (four albums, then your first two playlists), like the website's
    let quick = |art: El<'a>, title: &str, to: Message| -> El<'a> {
        button(row![art, text(clip(title, 26)).size(15).color(ui::TEXT()), horizontal_space()].spacing(14).align_y(Alignment::Center))
            .width(300)
            .padding(0)
            .style(|t, s| {
                let mut st = ui::flat(t, s);
                st.background = Some(if matches!(s, button::Status::Hovered | button::Status::Pressed) { ui::SURFACE_3() } else { ui::SURFACE_2() }.into());
                st.border = iced::border::rounded(6.0);
                st
            })
            .on_press(to)
            .into()
    };
    let mut tiles: Vec<El> = home_quick(app).into_iter().take(4).map(|a| quick(cover_of(app, Some(a), 56.0), &a.title, Message::Go(Page::Album(a.id.clone())))).collect();
    for p in app.playlists().iter().filter(|p| !matches!(p.kind, Kind::Household { .. })).take(2) {
        tiles.push(quick(playlist_art(app, p, 56.0), &p.name, Message::Go(Page::Playlist(p.id.clone()))));
    }
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

/// Albums in the order the Library's Sort box says (the website's four ways).
fn sorted_albums(app: &App) -> Vec<&Album> {
    let mut list: Vec<&Album> = app.lib.albums().iter().collect();
    match app.lib_sort.as_str() {
        "title" => list.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
        "artist" => list.sort_by(|a, b| a.artist.to_lowercase().cmp(&b.artist.to_lowercase()).then(a.year.unwrap_or(0).cmp(&b.year.unwrap_or(0)))),
        "year" => list.sort_by(|a, b| b.year.unwrap_or(0).cmp(&a.year.unwrap_or(0))),
        _ => list.sort_by(|a, b| b.added.cmp(&a.added)),
    }
    list
}

/// "Artist · 2024" under an album card.
fn artist_year(a: &Album) -> String {
    match a.year {
        Some(y) => format!("{} · {y}", a.artist),
        None => a.artist.clone(),
    }
}

/// The albums of each genre, decade and label (from the files' tags), biggest first (decades newest first).
fn browse_groups<'a>(app: &'a App, kind: &str) -> Vec<(String, Vec<&'a Album>)> {
    let mut map: std::collections::BTreeMap<String, Vec<&Album>> = Default::default();
    for a in app.lib.albums() {
        if let Some(v) = browse_value(a, kind) {
            map.entry(v).or_default().push(a);
        }
    }
    let mut v: Vec<(String, Vec<&Album>)> = map.into_iter().collect();
    if kind == "decade" {
        v.sort_by(|a, b| b.0.cmp(&a.0));
    } else {
        v.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    }
    v
}

fn browse_value(a: &Album, kind: &str) -> Option<String> {
    match kind {
        "genre" => a.genre.clone().filter(|g| !g.is_empty()),
        "decade" => a.year.map(|y| format!("{}s", y / 10 * 10)),
        _ => a.label.clone().filter(|l| !l.is_empty()),
    }
}

/// Your Library, like the website's: Albums, Artists, Songs, Playlists and Browse (plus what's downloaded here).
fn library(app: &App) -> El<'_> {
    let lib = &app.lib;
    let (a, t) = (lib.albums().len(), lib.tracks().len());
    let summary = format!("{a} album{} · {t} song{} · {} artists", if a == 1 { "" } else { "s" }, if t == 1 { "" } else { "s" }, lib.artists().len());
    // tabs: words with a line under the chosen one, over a hairline
    let tab = |label: &'static str, tb: LibTab| -> El<'_> {
        let on = app.lib_tab == tb;
        button(
            column![
                container(text(label).size(15).font(ui::FONT_BOLD).color(if on { ui::TEXT() } else { ui::TEXT_2() })).height(40).center_y(40),
                container(Space::new(label.len() as f32 * 8.4, 2)).style(move |_| container::Style { background: on.then(|| ui::TEXT().into()), border: iced::border::rounded(2.0), ..Default::default() }),
            ]
            .align_x(Alignment::Center),
        )
        .padding(0)
        .style(ui::link)
        .on_press(Message::LibTab(tb))
        .into()
    };
    let tabs = column![
        row![tab("Albums", LibTab::Albums), tab("Artists", LibTab::Artists), tab("Songs", LibTab::Songs), tab("Playlists", LibTab::Playlists), tab("Browse", LibTab::Browse), tab("Downloaded", LibTab::Downloaded)].spacing(24),
        container(Space::new(Length::Fill, 1)).style(|_| container::Style { background: Some(ui::BORDER().into()), ..Default::default() }),
    ];
    let mut page = column![column![ui::title("Your Library", 32), ui::dim(summary, 15)].spacing(8), ui::gap(20), tabs, ui::gap(18)];
    if matches!(app.lib_tab, LibTab::Albums | LibTab::Songs) {
        const SORTS: [(&str, &str); 4] = [("added", "Recently added"), ("title", "Title"), ("artist", "Artist"), ("year", "Release year")];
        let names: Vec<&'static str> = SORTS.iter().map(|s| s.1).collect();
        let chosen = SORTS.iter().find(|s| s.0 == app.lib_sort).map(|s| s.1);
        page = page.push(
            row![
                horizontal_space(),
                ui::dim("Sort", 13),
                iced::widget::pick_list(names, chosen, |n: &'static str| Message::LibSort(SORTS.iter().find(|s| s.1 == n).map(|s| s.0).unwrap_or("added").to_string())).padding([6, 10]).text_size(13).style(ui::pick_style).menu_style(ui::pick_menu),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
        page = page.push(ui::gap(22));
    }
    let nothing = lib.albums().is_empty();
    let body: El = match app.lib_tab {
        LibTab::Playlists => {
            let pls = app.playlists();
            if pls.is_empty() {
                empty_state("music", "No playlists yet", "Make one with the + next to Playlists, or from any song's + button.", Some(ui::btn_icon("plus", "New playlist", ui::Btn::Primary, false).on_press(Message::NewPlaylist(vec![])).into()))
            } else {
                Row::with_children(pls.iter().map(|p| playlist_card(app, p)).collect::<Vec<El>>()).spacing(4).wrap().into()
            }
        }
        LibTab::Downloaded => pages::downloaded(app),
        _ if nothing => empty_state("music", "No music yet", "Music added to the server shows up here on its own.", None),
        LibTab::Albums => Row::with_children(sorted_albums(app).into_iter().map(|a| album_card(app, a, artist_year(a))).collect::<Vec<El>>()).spacing(4).wrap().into(),
        LibTab::Songs => {
            let tracks: Vec<&Track> = sorted_albums(app).into_iter().flat_map(|a| lib.album_tracks(&a.id)).take(2000).collect();
            track_list(app, &tracks, "Your Library", Numbers::Place, true, Rows::Plain)
        }
        LibTab::Browse => {
            const PALETTE: [(u8, u8, u8); 8] = [(0xE5, 0x48, 0x4D), (0x5E, 0x54, 0x8E), (0x33, 0x5C, 0x67), (0xBC, 0x6C, 0x25), (0x3D, 0x5A, 0x80), (0x4A, 0x6C, 0x2F), (0x7A, 0x40, 0x69), (0x9E, 0x2A, 0x2B)];
            let mut c = Column::new().spacing(28);
            let mut any = false;
            for (kind, title, most) in [("genre", "Genres", 40), ("decade", "Decades", 40), ("label", "Labels", 24)] {
                let groups = browse_groups(app, kind);
                if groups.is_empty() {
                    continue;
                }
                let tiles: Vec<El> = groups
                    .into_iter()
                    .take(most)
                    .enumerate()
                    .map(|(i, (value, albums))| {
                        let (r, g, b) = PALETTE[i % PALETTE.len()];
                        let n = albums.len();
                        button(
                            container(column![text(value.clone()).size(18).font(ui::FONT_BOLD).color(Color::WHITE), iced::widget::vertical_space(), text(format!("{n} album{}", if n == 1 { "" } else { "s" })).size(12).color(Color { a: 0.85, ..Color::WHITE })])
                                .width(180)
                                .height(96)
                                .padding(14)
                                .style(move |_| container::Style { background: Some(Color::from_rgb8(r, g, b).into()), border: iced::border::rounded(12.0), ..Default::default() }),
                        )
                        .padding(0)
                        .style(ui::link)
                        .on_press(Message::Go(Page::Browse(kind.to_string(), value)))
                        .into()
                    })
                    .collect();
                c = c.push(section(title, Row::with_children(tiles).spacing(12).wrap().into()));
                any = true;
            }
            if !any {
                empty_state("library", "Nothing to browse yet", "Genres, years and labels come from your files' tags.", None)
            } else {
                c.into()
            }
        }
        LibTab::Artists => {
            // the ones you follow (Follow is on each artist's page), or everyone on the server
            let chip = |label: &'static str, all: bool| button(text(label).size(14)).padding([7, 16]).style(ui::chip(app.lib_all_artists == all)).on_press(Message::LibScope(all));
            let chips = row![chip("Following", false), chip("All artists", true)].spacing(8);
            let names: Vec<&String> = lib.artists().iter().filter(|n| app.lib_all_artists || app.follows(n)).collect();
            if names.is_empty() {
                column![chips, empty_state("user", "You're not following any artists yet", "Open an artist and press Follow. They'll be here.", Some(ui::btn("Browse all artists", ui::Btn::Secondary).on_press(Message::LibScope(true)).into()))].spacing(24).into()
            } else {
                let rows: Vec<El> = names
                    .into_iter()
                    .map(|n| {
                        let count = lib.albums_by(n).len();
                        let sub = if count == 0 { "Appears on albums".to_string() } else { format!("{count} album{}", if count == 1 { "" } else { "s" }) };
                        button(
                            row![cover_of(app, lib.artist_cover_album(n), 56.0), column![text(n.as_str()).size(15).color(ui::TEXT()), ui::dim(sub, 13)].spacing(2).width(Length::Fill), ui::wi("chevronRight", 18.0, ui::TEXT_2())]
                                .spacing(14)
                                .align_y(Alignment::Center),
                        )
                        .width(Length::Fill)
                        .padding([8, 12])
                        .style(ui::flat)
                        .on_press(Message::Go(Page::Artist(n.clone())))
                        .into()
                    })
                    .collect();
                column![chips, Column::with_children(rows).spacing(2)].spacing(24).into()
            }
        }
    };
    page.push(body).into()
}

/// One genre, decade or label: its albums, with Play and Shuffle for all of them.
fn browse_page<'a>(app: &'a App, kind: &'a str, value: &'a str) -> El<'a> {
    let mut albums: Vec<&Album> = app.lib.albums().iter().filter(|a| browse_value(a, kind).as_deref() == Some(value)).collect();
    albums.sort_by(|a, b| b.year.unwrap_or(0).cmp(&a.year.unwrap_or(0)));
    let label = match kind {
        "genre" => "Genre",
        "decade" => "Decade",
        _ => "Label",
    };
    if albums.is_empty() {
        return empty_state("library", "Nothing here", "No albums like that.", Some(ui::btn("Browse", ui::Btn::Secondary).on_press(Message::LibTab(LibTab::Browse)).into()));
    }
    let ids: Vec<String> = albums.iter().flat_map(|a| app.lib.album_track_ids(&a.id)).collect();
    let n = albums.len();
    column![
        column![ui::dim(label, 13), ui::title(value, 32), ui::dim(format!("{n} album{} · {} songs", if n == 1 { "" } else { "s" }, ids.len()), 15)].spacing(6),
        row![
            ui::btn_icon("play", "Play", ui::Btn::Primary, false).on_press(Message::PlayTracks(ids.clone(), 0, value.to_string())),
            ui::btn_icon("shuffle", "Shuffle", ui::Btn::Secondary, false).on_press(Message::ShuffleTracks(ids, value.to_string())),
        ]
        .spacing(8),
        Row::with_children(albums.into_iter().map(|a| album_card(app, a, artist_year(a))).collect::<Vec<El>>()).spacing(4).wrap(),
    ]
    .spacing(16)
    .into()
}

fn album_page<'a>(app: &'a App, id: &str) -> El<'a> {
    let Some(a) = app.lib.album(id) else { return ui::dim("This album isn't on the server any more.", 15).into() };
    let tracks = app.lib.album_tracks(id);
    let ids: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
    let mut info = column![
        ui::dim("Album", 13),
        ui::title(a.title.as_str(), 40),
        row![
            button(text(a.artist.as_str()).size(15).color(ui::TEXT())).padding(0).style(ui::flat).on_press(Message::Go(Page::Artist(a.artist.clone()))),
            ui::dim(format!("· {}", app.lib.album_summary(id)), 15),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .spacing(6);
    // quality tags like the website's (Lossless / Hi-Res Lossless / High Quality…, then the genre)
    let q = &a.quality;
    let (label, hires) = if q.lossless {
        let hi = q.bits.unwrap_or(16) >= 24 || q.khz.unwrap_or(44.1) > 48.0;
        (if hi { "HI-RES LOSSLESS" } else { "LOSSLESS" }, hi)
    } else {
        let k = q.kbps.unwrap_or(0);
        (if k >= 256 { "HIGH QUALITY" } else if k >= 128 { "NORMAL" } else { "LOW" }, false)
    };
    let tag = |s: String, strong: bool| -> El<'a> {
        container(text(s).size(11).font(ui::FONT_BOLD).color(if strong { ui::TEXT() } else { ui::TEXT_2() }))
            .padding([3, 8])
            .style(move |_: &iced::Theme| container::Style { border: iced::Border { color: if strong { ui::BORDER_STRONG() } else { ui::BORDER() }, width: 1.0, radius: 4.0.into() }, ..Default::default() })
            .into()
    };
    let mut tags = Row::new().spacing(6);
    tags = tags.push(tag(label.to_string(), hires));
    if let Some(g) = &a.genre {
        tags = tags.push(tag(g.to_uppercase(), false));
    }
    info = info.push(tags);
    let extra = vec![
        round_icon(icon::ADD, false, "Add to playlist", Some(Message::AddToPlaylist(ids.clone()))),
        download_button(app, "album", id, &ids),
        round_icon(icon::MORE, false, "More options", Some(Message::PopMenuOpen(crate::MenuKind::Album(id.to_string())))),
    ];
    info = info.push(ui::gap(10)).push(header_buttons(Message::PlayAlbum(id.to_string(), 0), Message::ShuffleAlbum(id.to_string()), extra));
    let head = row![cover_of(app, Some(a), 220.0), info].spacing(28).align_y(Alignment::End);
    let mut c = column![head, track_list(app, &tracks, &a.title, Numbers::TrackNo, false, Rows::Plain)].spacing(24);
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
        header_buttons(Message::PlayTracks(ids.clone(), 0, page_name.clone()), Message::ShuffleTracks(ids, page_name.clone()), vec![{
            // follow: they show up in Library → Artists
            let f = app.follows(name);
            ui::tip(ui::btn_icon(if f { "check" } else { "plus" }, if f { "Following" } else { "Follow" }, ui::Btn::Secondary, false).on_press(Message::FollowArtist(name.to_string())), if f { "Stop following" } else { "Follow: they show up in Library → Artists" })
        }]),
    ]
    .spacing(6);
    let first = p.albums.first().or(p.appears_on.first());
    let info = app.artist_infos.get(&page_name);
    let hero: El = match info.and_then(|i| i.1.as_ref()) {
        Some(h) => iced::widget::image(h.clone()).width(152).height(152).into(),
        None => ui::artist_hero::<Message>(&page_name, first.map(|a| a.title.as_str()).unwrap_or(&page_name), 152.0),
    };
    let mut c = column![row![hero, head].spacing(28).align_y(Alignment::Center)].spacing(28);
    if let Some(bio) = info.and_then(|i| i.0.as_ref()) {
        c = c.push(column![container(text(bio.clone()).size(14).color(ui::TEXT_2())).max_width(720), ui::dim("From Wikipedia", 11)].spacing(6));
    }
    if !p.songs.is_empty() {
        let top: Vec<&Track> = p.songs.iter().take(5).copied().collect();
        c = c.push(section("Songs", track_list(app, &top, &page_name, Numbers::Place, true, Rows::Plain)));
    }
    if !p.albums.is_empty() {
        c = c.push(section("Albums", album_grid(app, p.albums.clone(), false)));
    }
    if !p.appears_on.is_empty() {
        c = c.push(section("Appears on", album_grid(app, p.appears_on.clone(), true)));
    }
    if let Some(more) = pages::artist_get_more(app, &page_name) {
        c = c.push(more);
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
        c = c.push(titled("Songs", track_list(app, &list, "Search", Numbers::Blank, true, Rows::Plain), (all && r.songs.len() > 5).then_some(SearchType::Songs)));
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

/// The website's "nothing here" block: an icon in a grey circle, a heading, a line of text and maybe a button.
pub fn empty_state<'a>(icon_name: &str, title: impl Into<String>, about: impl Into<String>, action: Option<El<'a>>) -> El<'a> {
    let (title, about): (String, String) = (title.into(), about.into());
    let mut c = column![
        container(ui::wi(icon_name, 28.0, ui::TEXT_2())).width(64).height(64).center_x(64).center_y(64).style(|_| container::Style { background: Some(ui::SURFACE_3().into()), border: iced::border::rounded(32.0), ..Default::default() }),
        ui::gap(16),
        text(title).size(18).font(ui::FONT_BOLD),
        ui::gap(6),
        text(about).size(15).color(ui::TEXT_2()).align_x(iced::alignment::Horizontal::Center),
    ]
    .align_x(Alignment::Center);
    if let Some(a) = action {
        c = c.push(ui::gap(18)).push(a);
    }
    container(c).width(Length::Fill).center_x(Length::Fill).padding([56, 24]).into()
}

/// Now Playing, like the website's: the song big on the left (cover, title, artist, "album · year", like and add),
/// and Up next / Lyrics / Credits on the right.
fn now_playing(app: &App) -> El<'_> {
    let Some(t) = app.status.track.as_deref().and_then(|id| app.lib.track(id)) else {
        return column![
            ui::title("Now playing", 32),
            ui::gap(12),
            empty_state("music", "Nothing playing", "Pick an album, a playlist or a song to start.", Some(ui::btn("Go home", ui::Btn::Primary).on_press(Message::Go(Page::Home)).into())),
        ]
        .into();
    };
    let album = app.lib.album_of(t);
    let liked = app.is_liked(&t.id);
    let in_one = app.cache.current_in_playlist;
    // the cover shrinks with the window (up to 360 px), so the tabs on the right always fit
    let art = iced::widget::responsive(move |size| cover_of(app, album, size.width.min(360.0)));
    let mut title = Row::new().spacing(8).align_y(Alignment::Center).push(text(clip(&t.title, 40)).font(ui::FONT_BOLD).size(28.0));
    if t.explicit {
        title = title.push(explicit_badge());
    }
    let album_line = album.map(|a| match a.year {
        Some(y) => format!("{} · {y}", a.title),
        None => a.title.clone(),
    });
    let round = |name: &'static str, on: bool, hint: &'static str, m: Message| -> El<'_> {
        ui::tip(button(container(ui::wi_btn(name, 20.0, on)).width(40).height(40).center_x(40).center_y(40)).padding(0).style(ui::ring_btn(on)).on_press(m), hint)
    };
    let left = column![
        container(art).width(Length::Fill).height(Length::Shrink).max_height(360),
        ui::gap(20),
        button(title).padding(0).style(ui::link_hover(ui::TEXT(), ui::ACCENT())).on_press_maybe(album.map(|a| Message::Go(Page::Album(a.id.clone())))),
        ui::gap(4),
        button(text(t.artist.as_str()).size(16)).padding(0).style(ui::link_hover(ui::TEXT_2(), ui::TEXT())).on_press(Message::Go(Page::Artist(t.artist.clone()))),
        ui::gap(2),
        button(text(album_line.unwrap_or_default()).size(14)).padding(0).style(ui::link_hover(ui::TEXT_3(), ui::TEXT())).on_press_maybe(album.map(|a| Message::Go(Page::Album(a.id.clone())))),
        ui::gap(14),
        row![
            round(if liked { "heartFill" } else { "heart" }, liked, if liked { "Remove from Liked Songs" } else { "Add to Liked Songs" }, Message::ToggleLike(t.id.clone())),
            round(if in_one { "check" } else { "plus" }, in_one, "Add to playlist", Message::AddToPlaylist(vec![t.id.clone()])),
        ]
        .spacing(6),
    ]
    .width(Length::FillPortion(2))
    .max_width(360);

    let tab = |label: &'static str, tb: NpTab| button(text(label).size(14).wrapping(text::Wrapping::None)).padding([7, 16]).style(ui::chip(app.np_tab == tb)).on_press(Message::NpTab(tb));
    let tabs = row![tab("Up next", NpTab::Queue), tab("Lyrics", NpTab::Lyrics), tab("Credits", NpTab::Credits)].spacing(8);
    let right: El = match app.np_tab {
        NpTab::Queue => scrollable(queue_rows(app, t))
            .id(crate::queue_scroller())
            .on_scroll(|v| Message::Scrolled(1, v.absolute_offset().y))
            .style(ui::scroller)
            .height(Length::Fill)
            .into(),
        NpTab::Lyrics => pages::lyrics(app, &t.id),
        NpTab::Credits => pages::credits(app, t),
    };
    container(row![left, column![tabs, right].spacing(16).width(Length::FillPortion(3)).height(Length::Fill)].spacing(40)).padding(Padding { top: 8.0, ..Padding::ZERO }).into()
}

/// The queue, like the website's: the song that's playing, then the ones you queued yourself ("Next in queue"), then
/// what follows from the album or playlist ("Next from …"). The upcoming rows can be dragged anywhere, or removed.
fn queue_rows<'a>(app: &'a App, now: &'a Track) -> El<'a> {
    let key = list_key("queue", 99);
    let pos = app.queue.position().unwrap_or(0);
    let label = |s: String| -> El<'a> { container(text(s).size(13).font(ui::FONT_BOLD).color(ui::TEXT_2())).padding(Padding { top: 14.0, bottom: 0.0, left: 12.0, right: 12.0 }).height(LABEL_H).into() };
    // one row: cover, name over artist, length, and (for upcoming songs, under the pointer) a remove button
    let content = |t: &'a Track, index: Option<usize>, current: bool, hovered: bool, ghost: bool| -> El<'a> {
        let mut name = Row::new().spacing(6).align_y(Alignment::Center).push(text(clip(&t.title, 50)).size(14).font(ui::FONT_BOLD).color(if current { ui::ACCENT() } else { ui::TEXT() }).wrapping(text::Wrapping::None));
        if t.explicit {
            name = name.push(explicit_badge());
        }
        let mut line = row![
            ui::cover_box(app.lib.album_of(t).and_then(|a| app.cover_images.get(&(a.id.clone(), COVER))), app.lib.album_of(t).map(|a| a.title.as_str()).unwrap_or(""), 40.0, 40.0, 4.0),
            container(column![name, ui::dim(clip(&t.artist, 50), 12).wrapping(text::Wrapping::None)].spacing(1)).width(Length::Fill).clip(true),
            ui::dim(format_duration(t.duration), 12),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        if let Some(i) = index {
            let x = container(ui::wi_btn("x", 15.0, false)).width(28).height(28).center_x(28).center_y(28);
            line = line.push(if !hovered {
                El::from(Space::new(28, 28))
            } else if ghost {
                x.into()
            } else {
                ui::tip(button(x).padding(0).style(ui::icon_btn(false)).on_press(Message::RemoveFromQueue(i)), "Remove from queue")
            });
        }
        line.into()
    };
    let mut col = Column::new().push(label("Now playing".into()));
    // the song that's playing: lights up under the pointer, a click starts it again
    let lit = app.hover_row == Some((key, usize::MAX));
    col = col.push(
        mouse_area(
            container(content(now, None, true, false, false))
                .padding([0, 12])
                .width(Length::Fill)
                .height(QUEUE_ROW_H)
                .center_y(QUEUE_ROW_H)
                .style(move |_| container::Style { background: lit.then(|| ui::HOVER().into()), border: iced::border::rounded(8.0), ..Default::default() }),
        )
        .interaction(iced::mouse::Interaction::Pointer)
        .on_press(Message::JumpQueue(pos))
        .on_enter(Message::RowHover(Some((key, usize::MAX))))
        .on_exit(Message::RowLeave(key, usize::MAX)),
    );
    let upcoming: Vec<&Track> = app.queue.upcoming().iter().take(300).filter_map(|id| app.lib.track(id)).collect();
    if upcoming.is_empty() {
        let none = if app.queue.repeat() == Repeat::All { "Repeat is on: the queue starts over." } else { "Nothing else queued." };
        return col.push(container(ui::dim(none, 15)).padding([8, 12])).into();
    }
    let mine = app.queue.user_queued().min(upcoming.len());
    let from = if app.queue.source.is_empty() { "Next".to_string() } else { format!("Next from {}", app.queue.source) };
    // the first part's label sits above the list; the second part's (if there are both) between its rows
    col = col.push(label(if mine > 0 { "Next in queue".into() } else { from.clone() }));
    let list = crate::DragList::Queue;
    let moving = app.drag.as_ref().filter(|d| d.list == list && d.active).map(|d| d.from);
    let hovered = |k: usize| app.hover_row == Some((key, k)) && moving.is_none();
    let ghost = moving.and_then(|k| upcoming.get(k).map(|t| content(t, Some(pos + 1 + k), false, true, true)));
    let rows: Vec<(El<'a>, Message)> = upcoming.iter().enumerate().map(|(k, t)| (content(t, Some(pos + 1 + k), false, hovered(k), false), Message::JumpQueue(pos + 1 + k))).collect();
    let divider = (mine > 0 && mine < upcoming.len()).then(|| (mine, label(from)));
    col.push(drag_list(app, DragRows { list, key, rows, ghost, divider, row_h: QUEUE_ROW_H, lead: (4.0, 6.0), grab_cursor: false })).into()
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
                        column![text(clip(&t.title, 28)).size(14).color(ui::TEXT()).wrapping(text::Wrapping::None), ui::dim(clip(&t.artist, 32), 12).wrapping(text::Wrapping::None)].spacing(2).width(Length::Fill).clip(true)
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center)
                )
                .width(Length::Fill)
                .padding(4)
                .style(ui::flat)
                .on_press(Message::ToggleNowPlaying),
                ui::tip(button(ic(if liked { icon::HEART_FILL } else { icon::HEART }, 15)).padding(8).style(ui::icon_btn(liked)).on_press(Message::ToggleLike(t.id.clone())), if liked { "Remove from Liked Songs" } else { "Add to Liked Songs" }),
                // add to a playlist (a tick once it's in one of yours)
                ui::tip(button(ic(if in_one { icon::DOWNLOADED } else { icon::ADD }, 15)).padding(8).style(ui::icon_btn(in_one)).on_press(Message::AddToPlaylist(vec![t.id.clone()])), "Add to playlist"),
            ]
            .spacing(4)
            .align_y(Alignment::Center)
            .into()
        }
        None => row![ui::cover::<Message>(None, "", 56.0), text("Nothing playing").size(15).color(ui::TEXT())].spacing(12).align_y(Alignment::Center).into(),
    };

    let has = track.is_some() || !app.queue.is_empty();
    let small = |c: char, on: bool, hint: &str, m: Message| -> El<'_> { ui::tip(button(ic(c, 16)).padding(8).style(ui::icon_btn(on)).on_press_maybe(has.then_some(m)), hint) };
    let play =ui::tip(button(container(ui::play_pause(st.playing, 18.0, ui::FRAME())).center_x(18).center_y(18)).width(36).height(36).padding(9).style(ui::round_play).on_press_maybe(has.then_some(Message::TogglePlay)), if st.playing { "Pause" } else { "Play" });
    let repeat_icon = if app.queue.repeat() == Repeat::One { icon::REPEAT_ONE } else { icon::REPEAT_ALL };
    let controls = row![
        small(icon::SHUFFLE, app.queue.shuffle(), "Shuffle", Message::ToggleShuffle),
        small(icon::PREVIOUS, false, "Previous", Message::Previous),
        play,
        small(icon::NEXT, false, "Next", Message::Next),
        small(repeat_icon, app.queue.repeat() != Repeat::Off, "Repeat", Message::CycleRepeat),
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
        ui::tip(button(container(ui::cast_icon(17.0, if playing_elsewhere { ui::ACCENT() } else { ui::TEXT_2() })).center_x(16).center_y(16)).padding(8).style(ui::icon_btn(playing_elsewhere)).on_press(Message::PlayOn), "Play on"),
        ui::tip(button(ic(icon::QUEUE, 16)).padding(8).style(ui::icon_btn(queue_on)).on_press(if queue_on { Message::ToggleNowPlaying } else { Message::NpTab(NpTab::Queue) }), "Queue"),
        ui::tip(button(ic(vol_icon, 16)).padding(8).style(ui::icon_btn(false)).on_press(Message::ToggleMute), if app.volume <= 0.001 { "Unmute" } else { "Mute" }),
        container(El::from(marks::scrub(app.volume, 1.0, Message::Volume).on_change(Message::Volume))).width(if narrow { 64 } else { 110 }),
    ]
    .spacing(2)
    .align_y(Alignment::Center)
    .width(if narrow { 196 } else { 260 });

    // fixed sides, the controls take the middle
    row![container(left).width(if narrow { 250 } else { 300 }).clip(true), container(center).width(Length::Fill).center_x(Length::Fill), right].spacing(12).align_y(Alignment::Center).into()
}
