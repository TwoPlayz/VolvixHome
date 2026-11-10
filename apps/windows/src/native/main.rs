// Volvix Home for Windows, native (no web view). In progress: see docs/native-apps-plan.md.
//
// Connect + sign in, Home, Library, Album, Artist, Search, playback with a queue, mini player and
// Now Playing, media keys and the Windows media overlay, tray icon, one copy only, updater.
// Everything that isn't drawing (server API, library, search, queue, cover cache) is in volvix-core.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "../settings.rs"]
#[allow(dead_code)]
mod settings;
#[path = "../update.rs"]
#[allow(dead_code)]
mod update;

mod audio;
mod system;
mod ui;
mod views;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::widget::image;
use iced::{window, Size, Subscription, Task};
use volvix_core::client::SignedIn;
use volvix_core::covers::CoverCache;
use volvix_core::session::SavedSession;
use volvix_core::{Client, Error, Library, Me, Queue, ServerStatus, UserData};

use settings::Settings;

/// The release file the updater would download. Releases don't carry the native app yet, so until
/// it replaces today's app the updater finds nothing to install.
pub const RELEASE_EXE: &str = "volvix-home-native.exe";
/// Windows startup entry (its own, so it never touches today's app's entry).
pub const RUN_NAME: &str = "Volvix Home Native";

/// %LOCALAPPDATA%\Volvix Home Native: settings, sign-in and the cover cache. Separate from
/// today's app while both exist.
pub fn app_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("Volvix Home Native")
}

/// Covers are asked for at this size (the server makes and keeps these copies).
pub const COVER: u32 = 640;

#[derive(Debug, Clone, PartialEq)]
pub enum Page {
    Home,
    Search,
    Library,
    Album(String),
    Artist(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LibTab {
    Albums,
    Artists,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    /// type the server's address
    Connect,
    /// name + password (or create the first account on a new server)
    SignIn,
    /// signed in, fetching the library
    Loading,
    Main,
}

#[derive(Debug, Clone)]
pub enum Message {
    AddressChanged(String),
    Connect,
    Connected(Result<(Client, ServerStatus), Error>),
    NameChanged(String),
    PasswordChanged(String),
    SignIn,
    SignedIn(Result<(Client, SignedIn), Error>),
    Loaded(Result<(Arc<Library>, Option<UserData>, Me), Error>),
    SwitchServer,
    SignOut,

    Go(Page),
    Back,
    SearchChanged(String),
    LibTab(LibTab),
    CoverLoaded(String, u32, Option<Vec<u8>>),

    PlayTracks(Vec<String>, usize, String),
    PlayAlbum(String, usize),
    ShuffleAlbum(String),
    AddToQueue(String),
    PlayNext(String),
    TogglePlay,
    Next,
    Previous,
    ToggleShuffle,
    CycleRepeat,
    SeekDrag(f32),
    SeekCommit,
    Volume(f32),
    ToggleMute,
    JumpQueue(usize),
    RemoveFromQueue(usize),
    ToggleNowPlaying,

    Tick,
    WindowReady(Option<window::Id>),
    Hwnd(Option<isize>),
    CloseRequested(window::Id),
}

pub struct App {
    dir: PathBuf,
    pub screen: Screen,
    pub address: String,
    pub name: String,
    pub password: String,
    pub form_error: Option<String>,
    pub busy: bool,
    pub setup_needed: bool,

    pub client: Option<Client>,
    pub me: Option<Me>,
    pub lib: Arc<Library>,
    pub data: UserData,

    covers: CoverCache,
    pub cover_images: HashMap<(String, u32), image::Handle>,
    cover_pending: HashSet<(String, u32)>,

    pub page: Page,
    history: Vec<Page>,
    pub search: String,
    pub lib_tab: LibTab,
    pub now_playing: bool,

    pub queue: Queue,
    player: audio::Player,
    pub status: audio::Status,
    last_ended: u64,
    pub volume: f32,
    muted_from: Option<f32>,
    pub seeking: Option<f32>,

    settings: Settings,
    window: Option<window::Id>,
    instance: Option<Receiver<()>>,
    tray: Option<system::Tray>,
    media: Option<system::Media>,
    update_tx: Sender<update::State>,
    update_rx: Receiver<update::State>,
    next_update_check: Instant,
}

fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> impl std::future::Future<Output = T> {
    async move { tokio::task::spawn_blocking(f).await.expect("worker thread") }
}

impl App {
    fn new(instance: Receiver<()>) -> (App, Task<Message>) {
        let dir = app_dir();
        let saved = SavedSession::load(&dir);
        let settings = Settings::load();
        let (update_tx, update_rx) = mpsc::channel();
        let player = audio::Player::start();
        let tray = system::Tray::new(autostart_registered(), settings.close_to_tray, settings.auto_update);
        let mut app = App {
            covers: CoverCache::new(dir.join("covers")),
            dir,
            screen: Screen::Connect,
            address: saved.as_ref().map(|s| s.server.clone()).unwrap_or_default(),
            name: saved.as_ref().and_then(|s| s.user.as_ref()).map(|u| u.name.clone()).unwrap_or_default(),
            password: String::new(),
            form_error: None,
            busy: false,
            setup_needed: false,
            client: None,
            me: None,
            lib: Arc::new(Library::default()),
            data: UserData::default(),
            cover_images: HashMap::new(),
            cover_pending: HashSet::new(),
            page: Page::Home,
            history: vec![],
            search: String::new(),
            lib_tab: LibTab::Albums,
            now_playing: false,
            queue: Queue::new(),
            player,
            status: audio::Status::default(),
            last_ended: 0,
            volume: 0.8,
            muted_from: None,
            seeking: None,
            settings,
            window: None,
            instance: Some(instance),
            tray,
            media: None,
            update_tx,
            update_rx,
            next_update_check: Instant::now() + Duration::from_secs(20),
        };
        app.player.set_volume(app.volume);
        let mut tasks = vec![window::get_oldest().map(Message::WindowReady)];
        match saved {
            Some(s) if s.token.is_some() => {
                let c = s.client();
                app.client = Some(c.clone());
                app.screen = Screen::Loading;
                tasks.push(load_everything(c));
            }
            Some(s) => {
                // address known, signed out: straight to sign-in
                let c = s.client();
                app.client = Some(c.clone());
                app.screen = Screen::SignIn;
                tasks.push(Task::perform(blocking(move || c.status().map(|st| (c, st))), Message::Connected));
            }
            None => {}
        }
        (app, Task::batch(tasks))
    }

    fn save_session(&self) {
        if let Some(c) = &self.client {
            let s = SavedSession { server: c.base().to_string(), token: c.token().map(String::from), user: self.me.as_ref().map(|m| m.user.clone()) };
            let _ = s.save(&self.dir);
        }
    }

    fn signed_out(&mut self, message: Option<String>) {
        self.player.stop();
        self.queue.clear();
        if let Some(c) = &self.client {
            let c = c.with_token(None);
            let _ = SavedSession { server: c.base().to_string(), token: None, user: self.me.as_ref().map(|m| m.user.clone()) }.save(&self.dir);
            self.client = Some(c);
        }
        self.screen = Screen::SignIn;
        self.password.clear();
        self.form_error = message;
        self.busy = false;
        if let Some(m) = &mut self.media {
            m.clear();
        }
    }

    fn go(&mut self, page: Page) {
        if self.page != page {
            self.history.push(std::mem::replace(&mut self.page, page));
            if self.history.len() > 50 {
                self.history.remove(0);
            }
        }
        self.now_playing = false;
    }

    /// Ask for the covers the current screen shows and doesn't have yet.
    fn fetch_covers(&mut self) -> Task<Message> {
        let Some(client) = self.client.clone() else { return Task::none() };
        let mut tasks = vec![];
        for (id, size) in views::wanted_covers(self) {
            let key = (id.clone(), size);
            if self.cover_images.contains_key(&key) || self.cover_pending.contains(&key) {
                continue;
            }
            let Some(album) = self.lib.album(&id) else { continue };
            if !album.cover {
                continue;
            }
            let version = album.cover_v;
            // already on disk: show it now, no round trip
            if let Some(bytes) = self.covers.cached(&id, size, version) {
                self.cover_images.insert(key, image::Handle::from_bytes(bytes));
                continue;
            }
            self.cover_pending.insert(key);
            let (c, cache, fetch_id) = (client.clone(), self.covers.clone(), id.clone());
            tasks.push(Task::perform(blocking(move || cache.get(&c, &fetch_id, size, version).ok()), move |b| Message::CoverLoaded(id.clone(), size, b)));
        }
        Task::batch(tasks)
    }

    fn start_current(&mut self, play: bool) -> Task<Message> {
        let (Some(c), Some(id)) = (self.client.clone(), self.queue.current().map(String::from)) else { return Task::none() };
        self.player.load(&c, &id, play);
        self.status.track = Some(id.clone());
        self.status.position = Duration::ZERO;
        self.status.duration = None;
        self.status.playing = play;
        self.status.finished = false;
        // remember the album as recently played (shared with the website's Home)
        if let Some(album) = self.lib.track(&id).map(|t| t.album_id.clone()) {
            if self.data.recent.first() != Some(&album) {
                self.data.recent.retain(|a| *a != album);
                self.data.recent.insert(0, album.clone());
                self.data.recent.truncate(30);
                let c2 = c.clone();
                std::thread::spawn(move || {
                    // read-modify-write, so playlists saved on another device in the meantime are kept
                    if let Ok(fresh) = c2.user_data() {
                        let mut d = fresh.unwrap_or_default();
                        d.recent.retain(|a| *a != album);
                        d.recent.insert(0, album);
                        d.recent.truncate(30);
                        let _ = c2.save_user_data(&d);
                    }
                });
            }
        }
        self.fetch_covers()
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            /* ---------- connect and sign in ---------- */
            Message::AddressChanged(s) => self.address = s,
            Message::Connect => {
                if self.busy {
                    return Task::none();
                }
                self.busy = true;
                self.form_error = None;
                let typed = self.address.clone();
                return Task::perform(blocking(move || Client::connect(&typed)), Message::Connected);
            }
            Message::Connected(r) => {
                self.busy = false;
                match r {
                    Ok((c, st)) => {
                        self.address = c.base().to_string();
                        self.setup_needed = st.setup_needed;
                        self.client = Some(c);
                        self.screen = Screen::SignIn;
                        self.form_error = None;
                    }
                    Err(e) => {
                        // a saved address that can't be reached right now: still offer sign-in
                        if self.screen != Screen::SignIn {
                            self.screen = Screen::Connect;
                        }
                        self.form_error = Some(e.to_string());
                    }
                }
            }
            Message::NameChanged(s) => self.name = s,
            Message::PasswordChanged(s) => self.password = s,
            Message::SignIn => {
                let Some(c) = self.client.clone() else { return Task::none() };
                if self.busy || self.name.trim().is_empty() || self.password.is_empty() {
                    return Task::none();
                }
                self.busy = true;
                self.form_error = None;
                let (name, pw, setup) = (self.name.trim().to_string(), self.password.clone(), self.setup_needed);
                return Task::perform(blocking(move || if setup { c.setup(&name, &pw) } else { c.login(&name, &pw) }), Message::SignedIn);
            }
            Message::SignedIn(r) => match r {
                Ok((c, _who)) => {
                    self.password.clear();
                    self.client = Some(c.clone());
                    self.screen = Screen::Loading;
                    self.save_session();
                    return load_everything(c);
                }
                Err(e) => {
                    self.busy = false;
                    self.form_error = Some(e.to_string());
                }
            },
            Message::Loaded(r) => {
                self.busy = false;
                match r {
                    Ok((lib, data, me)) => {
                        self.lib = lib;
                        self.data = data.unwrap_or_default();
                        self.me = Some(me);
                        self.screen = Screen::Main;
                        self.save_session();
                        #[cfg(debug_assertions)]
                        if let Some(t) = self.dev_demo() {
                            return t;
                        }
                        return self.fetch_covers();
                    }
                    Err(Error::SignedOut) => self.signed_out(Some("Your sign-in ran out. Sign in again.".into())),
                    Err(e) => {
                        self.screen = Screen::SignIn;
                        self.form_error = Some(e.to_string());
                    }
                }
            }
            Message::SwitchServer => {
                self.signed_out(None);
                self.screen = Screen::Connect;
            }
            Message::SignOut => {
                if let Some(c) = self.client.clone() {
                    std::thread::spawn(move || {
                        let _ = c.logout();
                    });
                }
                self.signed_out(None);
            }

            /* ---------- moving around ---------- */
            Message::Go(p) => {
                self.go(p);
                return self.fetch_covers();
            }
            Message::Back => {
                if self.now_playing {
                    self.now_playing = false;
                } else if let Some(p) = self.history.pop() {
                    self.page = p;
                }
                return self.fetch_covers();
            }
            Message::SearchChanged(s) => {
                self.search = s;
                return self.fetch_covers();
            }
            Message::LibTab(t) => {
                self.lib_tab = t;
                return self.fetch_covers();
            }
            Message::CoverLoaded(id, size, bytes) => {
                self.cover_pending.remove(&(id.clone(), size));
                if let Some(b) = bytes {
                    self.cover_images.insert((id, size), image::Handle::from_bytes(b));
                }
            }

            /* ---------- playing ---------- */
            Message::PlayTracks(ids, start, source) => {
                self.queue.play(ids, start, &source);
                return self.start_current(true);
            }
            Message::PlayAlbum(id, start) => {
                let title = self.lib.album(&id).map(|a| a.title.clone()).unwrap_or_default();
                self.queue.play(self.lib.album_track_ids(&id), start, &title);
                return self.start_current(true);
            }
            Message::ShuffleAlbum(id) => {
                let title = self.lib.album(&id).map(|a| a.title.clone()).unwrap_or_default();
                self.queue.play_shuffled(self.lib.album_track_ids(&id), &title);
                return self.start_current(true);
            }
            Message::AddToQueue(id) => {
                let was_empty = self.queue.is_empty();
                self.queue.add(&id);
                if was_empty {
                    return self.start_current(true);
                }
            }
            Message::PlayNext(id) => {
                let was_empty = self.queue.is_empty();
                self.queue.play_next(&id);
                if was_empty {
                    return self.start_current(true);
                }
            }
            Message::TogglePlay => {
                if self.status.track.is_none() {
                    return self.start_current(true);
                }
                if self.status.playing {
                    self.player.pause();
                    self.status.playing = false;
                } else {
                    // the queue ended on its last song: play it again from the start
                    if self.status.finished {
                        return self.start_current(true);
                    }
                    self.player.play();
                    self.status.playing = true;
                }
            }
            Message::Next => {
                if self.queue.next(false).is_some() {
                    return self.start_current(true);
                }
            }
            Message::Previous => {
                if self.status.position > Duration::from_secs(3) {
                    self.player.seek(Duration::ZERO);
                    self.status.position = Duration::ZERO;
                } else if self.queue.previous().is_some() {
                    return self.start_current(true);
                }
            }
            Message::ToggleShuffle => self.queue.set_shuffle(!self.queue.shuffle()),
            Message::CycleRepeat => self.queue.set_repeat(self.queue.repeat().cycle()),
            Message::SeekDrag(v) => self.seeking = Some(v),
            Message::SeekCommit => {
                if let Some(v) = self.seeking.take() {
                    let to = Duration::from_secs_f32(v.max(0.0));
                    self.player.seek(to);
                    self.status.position = to;
                }
            }
            Message::Volume(v) => {
                self.volume = v;
                self.muted_from = None;
                self.player.set_volume(v * v); // feels even across the slider
            }
            Message::ToggleMute => {
                match self.muted_from.take() {
                    Some(v) => self.volume = v,
                    None => {
                        self.muted_from = Some(self.volume);
                        self.volume = 0.0;
                    }
                }
                self.player.set_volume(self.volume * self.volume);
            }
            Message::JumpQueue(i) => {
                if self.queue.jump(i).is_some() {
                    return self.start_current(true);
                }
            }
            Message::RemoveFromQueue(i) => self.queue.remove(i),
            Message::ToggleNowPlaying => {
                self.now_playing = !self.now_playing;
                return self.fetch_covers();
            }

            /* ---------- the clock: player status, tray, media keys, updates ---------- */
            Message::Tick => return self.tick(),
            Message::WindowReady(id) => {
                self.window = id;
                if let Some(id) = id {
                    return window::run_with_handle(id, |h| {
                        use raw_window_handle::RawWindowHandle;
                        match h.as_raw() {
                            RawWindowHandle::Win32(w) => Some(w.hwnd.get()),
                            _ => None,
                        }
                    })
                    .map(Message::Hwnd);
                }
            }
            Message::Hwnd(h) => {
                if let Some(h) = h {
                    self.media = system::Media::new(h);
                }
            }
            Message::CloseRequested(id) => {
                // the close button hides the window and the music keeps playing; Quit in the tray really exits
                if self.tray.is_some() && self.settings.close_to_tray {
                    return window::change_mode(id, window::Mode::Hidden);
                }
                return iced::exit();
            }
        }
        Task::none()
    }

    /// Debug builds only, for screenshots and smoke tests without clicking:
    /// VH_DEMO=album (open the first album and play its second song), VH_DEMO=nowplaying (also show Now Playing),
    /// VH_DEMO=artist / search:<words> / library.
    #[cfg(debug_assertions)]
    fn dev_demo(&mut self) -> Option<Task<Message>> {
        let demo = std::env::var("VH_DEMO").ok()?;
        let first = self.lib.albums_by_title().first().map(|a| (a.id.clone(), a.artist.clone()))?;
        let mut tasks = vec![];
        match demo.as_str() {
            "album" | "nowplaying" => {
                self.go(Page::Album(first.0.clone()));
                tasks.push(self.update(Message::PlayAlbum(first.0, 1)));
                if demo == "nowplaying" {
                    self.now_playing = true;
                }
            }
            "artist" => self.go(Page::Artist(first.1)),
            "library" => self.go(Page::Library),
            s if s.starts_with("search:") => {
                self.search = s["search:".len()..].to_string();
                self.go(Page::Search);
            }
            _ => {}
        }
        tasks.push(self.fetch_covers());
        Some(Task::batch(tasks))
    }

    fn show_window(&self) -> Task<Message> {
        match self.window {
            Some(id) => Task::batch([window::change_mode(id, window::Mode::Windowed), window::minimize(id, false), window::gain_focus(id)]),
            None => Task::none(),
        }
    }

    fn tick(&mut self) -> Task<Message> {
        let mut tasks = vec![];
        let st = self.player.status();
        let ended = st.ended != self.last_ended;
        self.last_ended = st.ended;
        self.status = st;
        if ended {
            if self.queue.next(true).is_some() {
                tasks.push(self.start_current(true));
            }
        }
        // a second launch, the tray
        if self.instance.as_ref().map(|r| r.try_recv().is_ok()).unwrap_or(false) {
            tasks.push(self.show_window());
        }
        let actions = self.tray.as_ref().map(|t| t.poll()).unwrap_or_default();
        for a in actions {
            use system::TrayAction as T;
            match a {
                T::Show => tasks.push(self.show_window()),
                T::PlayPause => tasks.push(self.update(Message::TogglePlay)),
                T::Next => tasks.push(self.update(Message::Next)),
                T::StartWithWindows(v) => self.settings.set("startWithWindows", v),
                T::CloseToTray(v) => self.settings.set("closeToTray", v),
                T::AutoUpdate(v) => self.settings.set("autoUpdate", v),
                T::CheckUpdate => self.check_update(),
                T::Restart => {
                    if update::restart() {
                        return iced::exit();
                    }
                }
                T::Quit => return iced::exit(),
            }
        }
        // media keys and the Windows overlay
        let events = self.media.as_ref().map(|m| m.poll()).unwrap_or_default();
        for e in events {
            use souvlaki::MediaControlEvent as E;
            let m = match e {
                E::Play | E::Pause | E::Toggle => Some(Message::TogglePlay),
                E::Next => Some(Message::Next),
                E::Previous => Some(Message::Previous),
                E::Stop => {
                    self.player.pause();
                    None
                }
                _ => None,
            };
            if let Some(m) = m {
                tasks.push(self.update(m));
            }
        }
        self.show_in_windows();
        // updates
        if self.settings.auto_update && Instant::now() >= self.next_update_check {
            self.next_update_check = Instant::now() + Duration::from_secs(6 * 3600);
            self.check_update();
        }
        while let Ok(s) = self.update_rx.try_recv() {
            if let (update::State::Ready(v), Some(t)) = (&s, &self.tray) {
                t.restart.set_enabled(true);
                t.set_tooltip(&format!("Volvix Home: version {v} is ready (restart to finish)"));
            }
        }
        Task::batch(tasks)
    }

    fn check_update(&self) {
        let tx = self.update_tx.clone();
        std::thread::spawn(move || {
            let t2 = tx.clone();
            let done = update::run(&move |s| {
                let _ = t2.send(s);
            });
            let _ = tx.send(done);
        });
    }

    /// What's playing, for the media overlay, lock screen and tray tooltip.
    fn show_in_windows(&mut self) {
        let Some(id) = self.status.track.clone() else { return };
        let Some(t) = self.lib.track(&id) else { return };
        let album = self.lib.album_of(t);
        let cover_file = album.filter(|a| a.cover).and_then(|a| self.covers.path_for(&a.id, COVER, a.cover_v)).filter(|p| p.exists());
        if let Some(m) = &mut self.media {
            m.show(&id, &t.title, &t.artist, album.map(|a| a.title.as_str()).unwrap_or(""), cover_file.as_deref(), Some(Duration::from_secs(t.duration as u64)), self.status.playing, self.status.position);
        }
        if let Some(tray) = &self.tray {
            tray.set_tooltip(&format!("{} · {}", t.title, t.artist));
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([iced::time::every(Duration::from_millis(250)).map(|_| Message::Tick), window::close_requests().map(Message::CloseRequested)])
    }
}

/// After signing in (or at start with a saved sign-in): who you are, the library, your data.
fn load_everything(c: Client) -> Task<Message> {
    Task::perform(
        blocking(move || -> Result<(Arc<Library>, Option<UserData>, Me), Error> {
            let me = c.me()?;
            let lib = Library::new(c.library()?);
            let data = c.user_data().unwrap_or(None);
            Ok((Arc::new(lib), data, me))
        }),
        Message::Loaded,
    )
}

/// Is this app in Windows' startup list?
fn autostart_registered() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .and_then(|k| k.get_value::<String, _>(RUN_NAME))
        .is_ok()
}

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().collect();
    let background = args.iter().any(|a| a == "--background");
    let after_update = args.iter().any(|a| a == "--after-update");
    let Some(instance) = system::single_instance(after_update) else { return Ok(()) };
    update::clean_old();

    let start_hidden = background && Settings::load().close_to_tray;
    let icon = window::icon::from_rgba(system::icon_rgba(64), 64, 64).ok();
    let cell = std::cell::RefCell::new(Some(instance));
    iced::application("Volvix Home", App::update, views::view)
        .subscription(App::subscription)
        .theme(|_| ui::theme())
        .default_font(ui::FONT)
        .antialiasing(true)
        .window(window::Settings {
            size: Size::new(1180.0, 800.0),
            min_size: Some(Size::new(760.0, 560.0)),
            icon,
            visible: !start_hidden,
            exit_on_close_request: false,
            ..Default::default()
        })
        .run_with(move || App::new(cell.borrow_mut().take().expect("started once")))
}
