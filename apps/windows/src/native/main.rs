// Volvix Home for Windows, native (no web view). See docs/native-apps-plan.md.
//
// Connect + sign in, Home (with Made For You mixes), Library, Album, Artist, Search, playlists
// (own, collaborative, household; editing, Liked Songs), lyrics, Get more music (Lidarr),
// household and notifications, offline downloads, settings and account; playback with a queue,
// mini player and Now Playing, media keys and the Windows media overlay, tray icon, one copy only,
// updater. Everything that isn't drawing is in volvix-core.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "../settings.rs"]
#[allow(dead_code)]
mod settings;
#[path = "../update.rs"]
#[allow(dead_code)]
mod update;

mod audio;
mod pages;
mod system;
mod ui;
mod views;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::widget::{image, scrollable};
use iced::{window, Size, Subscription, Task};
use serde_json::{json, Value};
use volvix_core::client::SignedIn;
use volvix_core::covers::CoverCache;
use volvix_core::downloads::{Downloads, Saved};
use volvix_core::mixes::{self, Mix};
use volvix_core::playlists::{self, PlaylistView};
use volvix_core::session::SavedSession;
use volvix_core::userdata::{self, Edit, PlayCounter};
use volvix_core::{
    Client, Error, FindResults, Household, Library, LyricsAnswer, Me, Notes, Playlist, Queue, Request, ServerStatus, SharedInvite, SharedList, SharedOpResult, SharedPlaylist, User, UserData,
};

use settings::Settings;

/// The release file the updater would download. Releases don't carry the native app yet, so until
/// it replaces today's app the updater finds nothing to install.
pub const RELEASE_EXE: &str = "volvix-home-native.exe";
/// Windows startup entry (its own, so it never touches today's app's entry).
pub const RUN_NAME: &str = "Volvix Home Native";

/// %LOCALAPPDATA%\Volvix Home Native: settings, sign-in, covers and downloads. Separate from
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
/// Roughly how tall one lyrics line is, to keep the sung line in view.
pub const LYRIC_LINE: f32 = 50.0;

pub fn lyrics_scroller() -> scrollable::Id {
    scrollable::Id::new("lyrics")
}

#[derive(Debug, Clone, PartialEq)]
pub enum Page {
    Home,
    Search,
    Library,
    Album(String),
    Artist(String),
    Playlist(String),
    Mix(String),
    GetMusic,
    Household,
    Notifications,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LibTab {
    Playlists,
    Albums,
    Artists,
    Downloaded,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NpTab {
    Queue,
    Lyrics,
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

/// A small window over the app (new playlist, rename, pick a playlist, change password…).
#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    NewPlaylist { name: String, description: String, add: Vec<String>, collaborative: bool },
    EditPlaylist { id: String, name: String, description: String, public: bool },
    DeletePlaylist { id: String, name: String },
    AddTo { tracks: Vec<String> },
    Password { current: String, new: String, error: Option<String>, busy: bool },
    Rename { name: String, error: Option<String> },
}

/// "Get more music": looking through MusicBrainz for music you don't have.
#[derive(Debug, Clone, Default)]
pub struct FindState {
    pub term: String,
    pub searched: String,
    pub busy: bool,
    pub results: Option<Result<FindResults, String>>,
    /// foreign ids asked for in this session (shown as "Requested")
    pub asked: HashSet<String>,
    pub asking: HashSet<String>,
    pub requests: Vec<Request>,
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
    Loaded(Result<Loaded, Error>),
    SwitchServer,
    SignOut,

    Go(Page),
    Back,
    SearchChanged(String),
    LibTab(LibTab),
    CoverLoaded(String, u32, Option<Vec<u8>>),
    ImageLoaded(String, Option<Vec<u8>>),

    PlayTracks(Vec<String>, usize, String),
    ShuffleTracks(Vec<String>, String),
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
    SeekTo(f32),
    Volume(f32),
    ToggleMute,
    JumpQueue(usize),
    RemoveFromQueue(usize),
    ToggleNowPlaying,
    NpTab(NpTab),
    LyricsLoaded(String, Result<LyricsAnswer, Error>),

    /* playlists */
    ToggleLike(String),
    NewPlaylist(Vec<String>),
    EditPlaylist(String),
    DeletePlaylist(String),
    AddToPlaylist(Vec<String>),
    AddToChosen(String),
    RemoveFromPlaylist(String, usize, String),
    MoveInPlaylist(String, usize, usize),
    Saved(Result<UserData, Error>),
    SharedLoaded(Result<SharedList, Error>),
    SharedOp(String, Value),
    SharedDone(Result<SharedOpResult, Error>),
    SharedCreated(Result<SharedOpResult, Error>, String),

    /* dialogs */
    DialogText(u8, String),
    DialogFlag(u8, bool),
    DialogOk,
    DialogCancel,
    PasswordDone(Result<(Client, SignedIn), Error>),
    RenameDone(Result<User, Error>),

    /* household and notifications */
    HouseholdLoaded(Result<Household, Error>),
    NotesLoaded(Result<Notes, Error>),
    MarkAllRead,

    /* get more music */
    FindChanged(String),
    /// from Search: look for these words on the Get more music page
    LookFurther(String),
    FindSubmit,
    Found(String, Result<FindResults, Error>),
    RequestMusic(String, String),
    Requested(String, Result<(), Error>),
    RequestsLoaded(Result<Vec<Request>, Error>),
    CancelRequest(String),

    /* downloads */
    ToggleSaved(String, String),
    DownloadDone(String, Result<(), String>),
    RemoveAllDownloads,

    /* settings */
    SetLeveling(String),
    SetShare(bool),
    SetPrivate(bool),
    SetStartWithWindows(bool),
    SetCloseToTray(bool),
    SetAutoUpdate(bool),
    CheckUpdate,
    ChangePassword,
    RenameMe,
    ServerStatus(Result<ServerStatus, Error>),

    Tick,
    WindowReady(Option<window::Id>),
    Hwnd(Option<isize>),
    CloseRequested(window::Id),
}

/// What signing in (or opening with a saved sign-in) loads.
#[derive(Debug, Clone)]
pub struct Loaded {
    lib: Arc<Library>,
    data: Option<UserData>,
    me: Option<Me>,
    shared: Option<SharedList>,
    /// the server couldn't be reached: opened from this device's copy
    offline: bool,
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
    pub offline: bool,
    pub server_status: Option<ServerStatus>,

    /* app data edits not on the server yet */
    pending: Vec<Edit>,
    in_flight: Vec<Edit>,
    retry_save_at: Option<Instant>,
    counter: PlayCounter,
    last_tick: Instant,

    pub shared: Vec<SharedPlaylist>,
    pub invites: Vec<SharedInvite>,
    pub household: Household,
    pub notes: Notes,
    pub mixes: Vec<Mix>,
    pub lyrics: HashMap<String, Result<LyricsAnswer, String>>,
    lyric_line: Option<usize>,
    pub find: FindState,

    pub downloads: Downloads,
    pub downloaded: HashSet<String>,
    pub saved: Vec<Saved>,
    dl_queue: VecDeque<String>,
    pub dl_active: Option<String>,

    covers: CoverCache,
    pub cover_images: HashMap<(String, u32), image::Handle>,
    cover_pending: HashSet<(String, u32)>,
    pub images: HashMap<String, image::Handle>,
    image_pending: HashSet<String>,

    pub page: Page,
    history: Vec<Page>,
    pub search: String,
    pub lib_tab: LibTab,
    pub now_playing: bool,
    pub np_tab: NpTab,
    pub dialog: Option<Dialog>,
    pub toast: Option<(String, Instant)>,

    pub queue: Queue,
    player: audio::Player,
    pub status: audio::Status,
    last_ended: u64,
    pub volume: f32,
    muted_from: Option<f32>,
    pub seeking: Option<f32>,
    presence: (Option<String>, bool),
    presence_at: Instant,
    next_poll: Instant,

    pub settings: Settings,
    pub update_state: Option<update::State>,
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
        let downloads = Downloads::new(dir.join("offline"));
        let mut app = App {
            covers: CoverCache::new(dir.join("covers")),
            downloaded: downloads.list().into_iter().collect(),
            saved: downloads.saved(),
            downloads,
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
            offline: false,
            server_status: None,
            pending: vec![],
            in_flight: vec![],
            retry_save_at: None,
            counter: PlayCounter::default(),
            last_tick: Instant::now(),
            shared: vec![],
            invites: vec![],
            household: Household::default(),
            notes: Notes::default(),
            mixes: vec![],
            lyrics: HashMap::new(),
            lyric_line: None,
            find: FindState::default(),
            dl_queue: VecDeque::new(),
            dl_active: None,
            cover_images: HashMap::new(),
            cover_pending: HashSet::new(),
            images: HashMap::new(),
            image_pending: HashSet::new(),
            page: Page::Home,
            history: vec![],
            search: String::new(),
            lib_tab: LibTab::Albums,
            now_playing: false,
            np_tab: NpTab::Queue,
            dialog: None,
            toast: None,
            queue: Queue::new(),
            player,
            status: audio::Status::default(),
            last_ended: 0,
            volume: 0.8,
            muted_from: None,
            seeking: None,
            presence: (None, false),
            presence_at: Instant::now(),
            next_poll: Instant::now() + Duration::from_secs(30),
            settings,
            update_state: None,
            window: None,
            instance: Some(instance),
            tray,
            media: None,
            update_tx,
            update_rx,
            next_update_check: Instant::now() + Duration::from_secs(20),
        };
        app.player.set_volume(app.volume * app.volume);
        let mut tasks = vec![window::get_oldest().map(Message::WindowReady)];
        match saved {
            Some(s) if s.token.is_some() => {
                let c = s.client();
                app.client = Some(c.clone());
                app.me = s.user.clone().map(|user| Me { user, ..Default::default() });
                app.screen = Screen::Loading;
                tasks.push(load_everything(c, app.downloads.clone()));
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

    pub fn me_id(&self) -> String {
        self.me.as_ref().map(|m| m.user.id.clone()).unwrap_or_default()
    }

    /// Every playlist you can see (yours, collaborative, the household's).
    pub fn playlists(&self) -> Vec<PlaylistView> {
        playlists::all(&self.data, &self.shared, &self.household, &self.me_id())
    }

    pub fn playlist(&self, id: &str) -> Option<PlaylistView> {
        self.playlists().into_iter().find(|p| p.id == id)
    }

    pub fn is_liked(&self, track: &str) -> bool {
        userdata::is_liked(&self.data, track)
    }

    pub fn setting_str(&self, key: &str, default: &str) -> String {
        userdata::setting(&self.data, key).and_then(|v| v.as_str().map(String::from)).unwrap_or_else(|| default.to_string())
    }
    pub fn setting_bool(&self, key: &str, default: bool) -> bool {
        userdata::setting(&self.data, key).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    fn say(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
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
        self.dialog = None;
        if let Some(m) = &mut self.media {
            m.clear();
        }
    }

    fn go(&mut self, page: Page) -> Task<Message> {
        if self.page != page {
            self.history.push(std::mem::replace(&mut self.page, page.clone()));
            if self.history.len() > 50 {
                self.history.remove(0);
            }
        }
        self.now_playing = false;
        let mut tasks = vec![self.fetch_covers()];
        if let Some(c) = self.client.clone().filter(|_| !self.offline) {
            match page {
                Page::Home => self.refresh_mixes(),
                Page::Household => {
                    let c2 = c.clone();
                    tasks.push(Task::perform(blocking(move || c2.household()), Message::HouseholdLoaded));
                    tasks.push(Task::perform(blocking(move || c.shared()), Message::SharedLoaded));
                }
                Page::Notifications => tasks.push(Task::perform(blocking(move || c.notifications()), Message::NotesLoaded)),
                Page::GetMusic => tasks.push(Task::perform(blocking(move || c.requests()), Message::RequestsLoaded)),
                Page::Settings => tasks.push(Task::perform(blocking(move || c.status()), Message::ServerStatus)),
                _ => {}
            }
        }
        Task::batch(tasks)
    }

    fn refresh_mixes(&mut self) {
        self.mixes = mixes::mixes(&self.lib, &self.data, &self.me_id(), userdata::now());
    }

    /* ---------- app data: apply here at once, save safely in the background ---------- */

    fn edit(&mut self, edits: Vec<Edit>) -> Task<Message> {
        for e in &edits {
            userdata::apply(&mut self.data, e);
        }
        self.pending.extend(edits);
        self.flush_edits()
    }

    fn flush_edits(&mut self) -> Task<Message> {
        if self.offline || !self.in_flight.is_empty() || self.pending.is_empty() {
            return Task::none();
        }
        let Some(c) = self.client.clone() else { return Task::none() };
        self.in_flight = std::mem::take(&mut self.pending);
        let edits = self.in_flight.clone();
        Task::perform(blocking(move || userdata::save_edits(&c, &edits)), Message::Saved)
    }

    /* ---------- covers and pictures ---------- */

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
            if self.offline {
                continue;
            }
            self.cover_pending.insert(key);
            let (c, cache, fetch_id) = (client.clone(), self.covers.clone(), id.clone());
            tasks.push(Task::perform(blocking(move || cache.get(&c, &fetch_id, size, version).ok()), move |b| Message::CoverLoaded(id.clone(), size, b)));
        }
        // pictures of music found through Lidarr
        if let Some(Ok(r)) = &self.find.results {
            let paths: Vec<String> = r.albums.iter().chain(r.artists.iter()).chain(r.songs.iter()).filter_map(|x| x.image.clone()).collect();
            for p in paths {
                if self.images.contains_key(&p) || !self.image_pending.insert(p.clone()) {
                    continue;
                }
                let c = client.clone();
                let path = p.clone();
                tasks.push(Task::perform(blocking(move || c.image_bytes(&path).ok()), move |b| Message::ImageLoaded(p.clone(), b)));
            }
        }
        Task::batch(tasks)
    }

    /* ---------- playing ---------- */

    fn start_current(&mut self, play: bool) -> Task<Message> {
        let (Some(c), Some(id)) = (self.client.clone(), self.queue.current().map(String::from)) else { return Task::none() };
        let local = self.downloads.path(&id).filter(|p| p.exists());
        if self.offline && local.is_none() {
            self.say("That song isn't downloaded, and the server can't be reached.");
            return Task::none();
        }
        let leveling = self.setting_str("leveling", "off");
        self.player.set_gain(audio::replay_gain(self.lib.track(&id).and_then(|t| t.replay_gain.as_ref()), &leveling));
        self.player.load(&c, &id, local, play);
        self.status.track = Some(id.clone());
        self.status.position = Duration::ZERO;
        self.status.duration = None;
        self.status.playing = play;
        self.status.finished = false;
        self.lyric_line = None;
        let mut tasks = vec![self.fetch_covers(), self.load_lyrics()];
        // remember the album as recently played (shared with the website's Home)
        if let Some(album) = self.lib.track(&id).map(|t| t.album_id.clone()) {
            if self.data.recent.first() != Some(&album) && !self.setting_bool("privateSession", false) {
                tasks.push(self.edit(vec![Edit::Recent(album)]));
            }
        }
        Task::batch(tasks)
    }

    fn load_lyrics(&mut self) -> Task<Message> {
        let (Some(c), Some(id)) = (self.client.clone(), self.status.track.clone()) else { return Task::none() };
        if !self.now_playing || self.np_tab != NpTab::Lyrics || self.lyrics.contains_key(&id) || self.offline {
            return Task::none();
        }
        let tid = id.clone();
        Task::perform(blocking(move || c.lyrics(&tid)), move |r| Message::LyricsLoaded(id.clone(), r))
    }

    /* ---------- playlists: your own ones are app data, collaborative ones go to the server ---------- */

    fn add_to_playlist(&mut self, pl_id: &str, tracks: Vec<String>) -> Task<Message> {
        let Some(pl) = self.playlist(pl_id) else { return Task::none() };
        let fresh: Vec<String> = tracks.into_iter().filter(|t| !pl.tracks.contains(t)).fold(vec![], |mut v, t| {
            if !v.contains(&t) {
                v.push(t);
            }
            v
        });
        if fresh.is_empty() {
            self.say(format!("Already in \"{}\"", pl.name));
            return Task::none();
        }
        self.say(format!("Added to \"{}\"", pl.name));
        if pl.is_shared() {
            return self.update(Message::SharedOp(pl.id, json!({ "op": "add", "ids": fresh })));
        }
        self.edit(vec![Edit::AddTracks { id: pl.id, tracks: fresh, at: userdata::now() }])
    }

    /* ---------- downloads ---------- */

    /// Every song a saved album or playlist needs.
    fn needed_downloads(&self) -> Vec<String> {
        let mut out = vec![];
        for s in &self.saved {
            let ids = match s.kind.as_str() {
                "album" => self.lib.album_track_ids(&s.id),
                _ => self.playlist(&s.id).map(|p| p.tracks).unwrap_or_default(),
            };
            for id in ids {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    }

    fn sync_downloads(&mut self) -> Task<Message> {
        let needed = self.needed_downloads();
        let set: HashSet<String> = needed.iter().cloned().collect();
        if !self.offline {
            self.downloads.prune(&set);
            self.downloaded.retain(|id| set.contains(id));
        }
        self.dl_queue = needed.into_iter().filter(|id| !self.downloaded.contains(id) && self.lib.track(id).is_some()).collect();
        self.pump_downloads()
    }

    fn pump_downloads(&mut self) -> Task<Message> {
        if self.dl_active.is_some() || self.offline {
            return Task::none();
        }
        let Some(c) = self.client.clone() else { return Task::none() };
        while let Some(id) = self.dl_queue.pop_front() {
            if self.downloaded.contains(&id) || self.dl_active.as_deref() == Some(&id) {
                continue;
            }
            self.dl_active = Some(id.clone());
            let d = self.downloads.clone();
            let tid = id.clone();
            return Task::perform(blocking(move || d.download(&c, &tid, |_, _| {}).map(|_| ()).map_err(|e| e.to_string())), move |r| Message::DownloadDone(id.clone(), r));
        }
        Task::none()
    }

    pub fn dl_queue_len(&self) -> usize {
        self.dl_queue.len()
    }

    pub fn is_saved(&self, kind: &str, id: &str) -> bool {
        self.saved.iter().any(|s| s.kind == kind && s.id == id)
    }

    /* ---------- the main switchboard ---------- */

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
                Ok((c, who)) => {
                    self.password.clear();
                    self.client = Some(c.clone());
                    self.me = Some(Me { user: who.user, ..Default::default() });
                    self.screen = Screen::Loading;
                    self.save_session();
                    return load_everything(c, self.downloads.clone());
                }
                Err(e) => {
                    self.busy = false;
                    self.form_error = Some(e.to_string());
                }
            },
            Message::Loaded(r) => {
                self.busy = false;
                match r {
                    Ok(l) => {
                        self.lib = l.lib;
                        self.data = l.data.unwrap_or_default();
                        if l.me.is_some() {
                            self.me = l.me;
                        }
                        if let Some(s) = l.shared {
                            self.shared = s.playlists;
                            self.invites = s.invites;
                        }
                        self.offline = l.offline;
                        self.screen = Screen::Main;
                        self.save_session();
                        self.refresh_mixes();
                        if self.offline {
                            self.say("The server can't be reached. Your downloads still play.");
                            self.lib_tab = LibTab::Downloaded;
                        }
                        let mut tasks = vec![self.fetch_covers(), self.sync_downloads()];
                        if let Some(c) = self.client.clone().filter(|_| !self.offline) {
                            tasks.push(Task::perform(blocking(move || c.notifications()), Message::NotesLoaded));
                        }
                        #[cfg(debug_assertions)]
                        if let Some(t) = self.dev_demo() {
                            tasks.push(t);
                        }
                        return Task::batch(tasks);
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
                        let _ = c.set_now_playing(None, false);
                        let _ = c.logout();
                    });
                }
                self.signed_out(None);
            }

            /* ---------- moving around ---------- */
            Message::Go(p) => return self.go(p),
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
            Message::ImageLoaded(path, bytes) => {
                if let Some(b) = bytes.filter(|b| !b.is_empty()) {
                    self.images.insert(path, image::Handle::from_bytes(b));
                }
            }

            /* ---------- playing ---------- */
            Message::PlayTracks(ids, start, source) => {
                self.queue.play(ids, start, &source);
                return self.start_current(true);
            }
            Message::ShuffleTracks(ids, source) => {
                self.queue.play_shuffled(ids, &source);
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
                self.say("Added to the queue");
                if was_empty {
                    return self.start_current(true);
                }
            }
            Message::PlayNext(id) => {
                let was_empty = self.queue.is_empty();
                self.queue.play_next(&id);
                self.say("Plays next");
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
                    return self.update(Message::SeekTo(v));
                }
            }
            Message::SeekTo(v) => {
                let to = Duration::from_secs_f32(v.max(0.0));
                self.player.seek(to);
                self.status.position = to;
                self.lyric_line = None;
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
                self.lyric_line = None;
                return Task::batch([self.fetch_covers(), self.load_lyrics()]);
            }
            Message::NpTab(t) => {
                self.np_tab = t;
                self.now_playing = true;
                self.lyric_line = None;
                return self.load_lyrics();
            }
            Message::LyricsLoaded(id, r) => {
                self.lyrics.insert(id, r.map_err(|e| e.to_string()));
            }

            /* ---------- playlists ---------- */
            Message::ToggleLike(track) => {
                let on = !self.is_liked(&track);
                self.say(if on { "Added to Liked Songs" } else { "Removed from Liked Songs" });
                return self.edit(vec![Edit::Like { track, on, at: userdata::now() }]);
            }
            Message::NewPlaylist(add) => self.dialog = Some(Dialog::NewPlaylist { name: String::new(), description: String::new(), add, collaborative: false }),
            Message::EditPlaylist(id) => {
                if let Some(p) = self.playlist(&id) {
                    self.dialog = Some(Dialog::EditPlaylist { id, name: p.name, description: p.description, public: p.public });
                }
            }
            Message::DeletePlaylist(id) => {
                if let Some(p) = self.playlist(&id) {
                    self.dialog = Some(Dialog::DeletePlaylist { id, name: p.name });
                }
            }
            Message::AddToPlaylist(tracks) => self.dialog = Some(Dialog::AddTo { tracks }),
            Message::AddToChosen(pl) => {
                let Some(Dialog::AddTo { tracks }) = self.dialog.take() else { return Task::none() };
                return self.add_to_playlist(&pl, tracks);
            }
            Message::RemoveFromPlaylist(pl, index, track) => {
                let Some(p) = self.playlist(&pl) else { return Task::none() };
                if p.is_shared() {
                    if let Some(s) = self.shared.iter_mut().find(|s| s.id == pl) {
                        if index < s.tracks.len() {
                            s.tracks.remove(index); // show it gone right away
                        }
                    }
                    return self.update(Message::SharedOp(pl, json!({ "op": "remove", "index": index, "trackId": track })));
                }
                return self.edit(vec![Edit::RemoveAt { id: pl, index, track }]);
            }
            Message::MoveInPlaylist(pl, from, to) => {
                let Some(p) = self.playlist(&pl) else { return Task::none() };
                if to >= p.tracks.len() {
                    return Task::none();
                }
                if p.is_shared() {
                    if let Some(s) = self.shared.iter_mut().find(|s| s.id == pl) {
                        let item = s.tracks.remove(from);
                        s.tracks.insert(to, item);
                    }
                    return self.update(Message::SharedOp(pl, json!({ "op": "move", "from": from, "to": to })));
                }
                return self.edit(vec![Edit::Move { id: pl, from, to }]);
            }
            Message::Saved(r) => {
                let done = std::mem::take(&mut self.in_flight);
                match r {
                    Ok(d) => {
                        // the server's copy, with anything changed since on top
                        self.data = d;
                        for e in &self.pending {
                            userdata::apply(&mut self.data, e);
                        }
                        self.retry_save_at = None;
                    }
                    Err(Error::SignedOut) => {
                        self.signed_out(Some("Your sign-in ran out. Sign in again.".into()));
                        return Task::none();
                    }
                    Err(_) => {
                        // keep them, try again shortly
                        let mut again = done;
                        again.append(&mut self.pending);
                        self.pending = again;
                        self.retry_save_at = Some(Instant::now() + Duration::from_secs(15));
                        return Task::none();
                    }
                }
                return self.flush_edits();
            }
            Message::SharedLoaded(r) => {
                if let Ok(s) = r {
                    self.shared = s.playlists;
                    self.invites = s.invites;
                }
            }
            Message::SharedOp(id, op) => {
                let Some(c) = self.client.clone() else { return Task::none() };
                return Task::perform(blocking(move || c.shared_op(&id, op)), Message::SharedDone);
            }
            Message::SharedDone(r) => match r {
                Ok(res) => {
                    let id = res.playlist.id.clone();
                    self.shared.retain(|p| p.id != id);
                    if !res.gone {
                        self.shared.push(res.playlist);
                    }
                    self.invites.retain(|i| i.id != id);
                }
                Err(e) => {
                    self.say(e.to_string());
                    if let Some(c) = self.client.clone() {
                        return Task::perform(blocking(move || c.shared()), Message::SharedLoaded);
                    }
                }
            },
            Message::SharedCreated(r, old_id) => match r {
                Ok(res) => {
                    let id = res.playlist.id.clone();
                    self.shared.push(res.playlist);
                    self.say("It's collaborative now. Invite people from the website's playlist page.");
                    let t = self.edit(vec![Edit::DeletePlaylist(old_id)]);
                    return Task::batch([t, self.go(Page::Playlist(id))]);
                }
                Err(e) => self.say(e.to_string()),
            },

            /* ---------- dialogs ---------- */
            Message::DialogText(field, s) => match (&mut self.dialog, field) {
                (Some(Dialog::NewPlaylist { name, .. }), 0) | (Some(Dialog::EditPlaylist { name, .. }), 0) | (Some(Dialog::Rename { name, .. }), 0) => *name = s,
                (Some(Dialog::NewPlaylist { description, .. }), 1) | (Some(Dialog::EditPlaylist { description, .. }), 1) => *description = s,
                (Some(Dialog::Password { current, .. }), 0) => *current = s,
                (Some(Dialog::Password { new, .. }), 1) => *new = s,
                _ => {}
            },
            Message::DialogFlag(field, v) => match (&mut self.dialog, field) {
                (Some(Dialog::NewPlaylist { collaborative, .. }), 0) => *collaborative = v,
                (Some(Dialog::EditPlaylist { public, .. }), 0) => *public = v,
                _ => {}
            },
            Message::DialogCancel => self.dialog = None,
            Message::DialogOk => return self.dialog_ok(),
            Message::PasswordDone(r) => match r {
                Ok((c, _)) => {
                    self.client = Some(c);
                    self.save_session();
                    self.dialog = None;
                    self.say("Password changed");
                }
                Err(e) => {
                    if let Some(Dialog::Password { error, busy, .. }) = &mut self.dialog {
                        *error = Some(e.to_string());
                        *busy = false;
                    }
                }
            },
            Message::RenameDone(r) => match r {
                Ok(u) => {
                    if let Some(m) = &mut self.me {
                        m.user = u;
                    }
                    self.save_session();
                    self.dialog = None;
                    self.say("Name changed");
                }
                Err(e) => {
                    if let Some(Dialog::Rename { error, .. }) = &mut self.dialog {
                        *error = Some(e.to_string());
                    }
                }
            },

            /* ---------- household and notifications ---------- */
            Message::HouseholdLoaded(r) => {
                if let Ok(h) = r {
                    self.household = h;
                }
            }
            Message::NotesLoaded(r) => {
                if let Ok(n) = r {
                    // something new: say so
                    let known: HashSet<&String> = self.notes.items.iter().map(|n| &n.id).collect();
                    let fresh: Vec<String> = n.items.iter().filter(|x| !x.read && !known.contains(&x.id)).map(|x| x.text.clone()).collect();
                    let first_load = self.notes.items.is_empty() && self.notes.unread == 0;
                    self.notes = n;
                    if let (false, Some(t)) = (first_load, fresh.first()) {
                        self.say(t.clone());
                    }
                    if self.page == Page::Notifications && self.notes.unread > 0 {
                        return self.update(Message::MarkAllRead);
                    }
                }
            }
            Message::MarkAllRead => {
                for n in &mut self.notes.items {
                    n.read = true;
                }
                self.notes.unread = 0;
                if let Some(c) = self.client.clone() {
                    std::thread::spawn(move || {
                        let _ = c.mark_read(None);
                    });
                }
            }

            /* ---------- get more music ---------- */
            Message::FindChanged(s) => self.find.term = s,
            Message::LookFurther(s) => {
                self.find.term = s;
                let t = self.go(Page::GetMusic);
                return Task::batch([t, self.update(Message::FindSubmit)]);
            }
            Message::FindSubmit => {
                let term = self.find.term.trim().to_string();
                let Some(c) = self.client.clone() else { return Task::none() };
                if term.chars().count() < 2 || self.find.busy {
                    return Task::none();
                }
                self.find.busy = true;
                self.find.searched = term.clone();
                let t2 = term.clone();
                return Task::perform(blocking(move || c.find_music(&t2)), move |r| Message::Found(term.clone(), r));
            }
            Message::Found(term, r) => {
                if term == self.find.searched {
                    self.find.busy = false;
                    self.find.results = Some(r.map_err(|e| e.to_string()));
                    return self.fetch_covers();
                }
            }
            Message::RequestMusic(kind, id) => {
                let Some(c) = self.client.clone() else { return Task::none() };
                self.find.asking.insert(id.clone());
                let fid = id.clone();
                return Task::perform(blocking(move || c.request_music(&kind, &fid)), move |r| Message::Requested(id.clone(), r));
            }
            Message::Requested(id, r) => {
                self.find.asking.remove(&id);
                match r {
                    Ok(()) => {
                        self.find.asked.insert(id);
                        self.say("Requested. It shows up in your library when it's downloaded.");
                        if let Some(c) = self.client.clone() {
                            return Task::perform(blocking(move || c.requests()), Message::RequestsLoaded);
                        }
                    }
                    Err(e) => self.say(e.to_string()),
                }
            }
            Message::RequestsLoaded(r) => {
                if let Ok(list) = r {
                    self.find.requests = list;
                }
            }
            Message::CancelRequest(id) => {
                let Some(c) = self.client.clone() else { return Task::none() };
                self.find.asked.remove(&id);
                return Task::perform(
                    blocking(move || {
                        c.cancel_request(&id)?;
                        c.requests()
                    }),
                    Message::RequestsLoaded,
                );
            }

            /* ---------- downloads ---------- */
            Message::ToggleSaved(kind, id) => {
                if self.is_saved(&kind, &id) {
                    self.saved.retain(|s| !(s.kind == kind && s.id == id));
                    self.say("Removed from this computer");
                } else {
                    self.saved.push(Saved { kind, id });
                    self.say("Downloading for listening offline");
                }
                let _ = self.downloads.set_saved(&self.saved);
                return self.sync_downloads();
            }
            Message::DownloadDone(id, r) => {
                self.dl_active = None;
                match r {
                    Ok(()) => {
                        self.downloaded.insert(id);
                    }
                    Err(e) => self.say(format!("A download didn't finish ({e}). It'll try again later.")),
                }
                return self.pump_downloads();
            }
            Message::RemoveAllDownloads => {
                self.saved.clear();
                let _ = self.downloads.set_saved(&self.saved);
                self.dl_queue.clear();
                self.downloads.prune(&HashSet::new());
                self.downloaded.clear();
                self.say("Downloads removed");
            }

            /* ---------- settings ---------- */
            Message::SetLeveling(v) => {
                let t = self.edit(vec![Edit::Setting("leveling".into(), json!(v.clone()))]);
                let g = audio::replay_gain(self.status.track.as_deref().and_then(|id| self.lib.track(id)).and_then(|t| t.replay_gain.as_ref()), &v);
                self.player.set_gain(g);
                return t;
            }
            Message::SetShare(v) => return self.edit(vec![Edit::Setting("shareActivity".into(), json!(v))]),
            Message::SetPrivate(v) => return self.edit(vec![Edit::Setting("privateSession".into(), json!(v))]),
            Message::SetStartWithWindows(v) => {
                self.settings.set("startWithWindows", v);
                if let Some(t) = &self.tray {
                    t.set_checks(Some(v), None, None);
                }
            }
            Message::SetCloseToTray(v) => {
                self.settings.set("closeToTray", v);
                if let Some(t) = &self.tray {
                    t.set_checks(None, Some(v), None);
                }
            }
            Message::SetAutoUpdate(v) => {
                self.settings.set("autoUpdate", v);
                if let Some(t) = &self.tray {
                    t.set_checks(None, None, Some(v));
                }
            }
            Message::CheckUpdate => self.check_update(),
            Message::ChangePassword => self.dialog = Some(Dialog::Password { current: String::new(), new: String::new(), error: None, busy: false }),
            Message::RenameMe => self.dialog = Some(Dialog::Rename { name: self.me.as_ref().map(|m| m.user.name.clone()).unwrap_or_default(), error: None }),
            Message::ServerStatus(r) => self.server_status = r.ok(),

            /* ---------- the clock: player status, saving, presence, tray, media keys, updates ---------- */
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

    fn dialog_ok(&mut self) -> Task<Message> {
        let Some(d) = self.dialog.clone() else { return Task::none() };
        match d {
            Dialog::NewPlaylist { name, description, add, collaborative } => {
                self.dialog = None;
                let name = if name.trim().is_empty() { format!("My Playlist #{}", self.data.playlists.len() + 1) } else { name.trim().to_string() };
                let id = userdata::new_playlist_id();
                let now = userdata::now();
                let pl = Playlist {
                    id: id.clone(),
                    name: name.clone(),
                    description: description.trim().to_string(),
                    owner: userdata::ME.into(),
                    tracks: add.iter().map(|t| volvix_core::PlaylistEntry { id: t.clone(), added_by: userdata::ME.into(), at: now }).collect(),
                    ..Default::default()
                };
                self.say(format!("Created \"{name}\""));
                let t = self.edit(vec![Edit::CreatePlaylist(pl.clone())]);
                if collaborative {
                    // a collaborative playlist lives on the server so everyone edits the same copy
                    let Some(c) = self.client.clone() else { return t };
                    let ids = pl.track_ids();
                    let old = id.clone();
                    return Task::batch([t, Task::perform(blocking(move || c.create_shared(&pl.name, &pl.description, false, &ids)), move |r| Message::SharedCreated(r, old.clone()))]);
                }
                if add.is_empty() {
                    return Task::batch([t, self.go(Page::Playlist(id))]);
                }
                t
            }
            Dialog::EditPlaylist { id, name, description, public } => {
                self.dialog = None;
                let Some(p) = self.playlist(&id) else { return Task::none() };
                if p.is_shared() {
                    return self.update(Message::SharedOp(id, json!({ "op": "update", "name": name, "description": description, "public": public })));
                }
                self.edit(vec![Edit::UpdatePlaylist { id, name: Some(name), description: Some(description), public: Some(public) }])
            }
            Dialog::DeletePlaylist { id, name } => {
                self.dialog = None;
                self.say(format!("Deleted \"{name}\""));
                let on_page = self.page == Page::Playlist(id.clone());
                if on_page {
                    self.page = Page::Library;
                    self.lib_tab = LibTab::Playlists;
                }
                let shared = self.playlist(&id).map(|p| p.is_shared()).unwrap_or(false);
                self.saved.retain(|s| s.id != id);
                let _ = self.downloads.set_saved(&self.saved);
                if shared {
                    return self.update(Message::SharedOp(id, json!({ "op": "delete" })));
                }
                self.edit(vec![Edit::DeletePlaylist(id)])
            }
            Dialog::AddTo { .. } => Task::none(),
            Dialog::Password { current, new, busy, .. } => {
                let Some(c) = self.client.clone() else { return Task::none() };
                if busy || current.is_empty() || new.is_empty() {
                    return Task::none();
                }
                if let Some(Dialog::Password { busy, error, .. }) = &mut self.dialog {
                    *busy = true;
                    *error = None;
                }
                Task::perform(blocking(move || c.change_password(&current, &new)), Message::PasswordDone)
            }
            Dialog::Rename { name, .. } => {
                let Some(c) = self.client.clone() else { return Task::none() };
                if name.trim().is_empty() {
                    return Task::none();
                }
                Task::perform(blocking(move || c.rename_me(name.trim())), Message::RenameDone)
            }
        }
    }

    /// Debug builds only, for screenshots and smoke tests without clicking:
    /// VH_DEMO=album | nowplaying | lyrics | artist | library | playlists | playlist | home |
    /// getmusic:<words> | search:<words> | settings | household | notifications | dialog.
    #[cfg(debug_assertions)]
    fn dev_demo(&mut self) -> Option<Task<Message>> {
        let demo = std::env::var("VH_DEMO").ok()?;
        let first = self.lib.albums_by_title().first().map(|a| (a.id.clone(), a.artist.clone()))?;
        let mut tasks = vec![];
        match demo.as_str() {
            "album" | "nowplaying" | "lyrics" => {
                tasks.push(self.go(Page::Album(first.0.clone())));
                if demo == "lyrics" {
                    // the dev music has synced lyrics for "Low Tide"
                    let low = self.lib.tracks().iter().find(|t| t.title == "Low Tide").map(|t| t.album_id.clone()).unwrap_or(first.0.clone());
                    tasks.push(self.update(Message::PlayAlbum(low, 0)));
                    self.np_tab = NpTab::Lyrics;
                } else {
                    tasks.push(self.update(Message::PlayAlbum(first.0, 1)));
                }
                if demo != "album" {
                    self.now_playing = true;
                    tasks.push(self.load_lyrics());
                }
            }
            "artist" => tasks.push(self.go(Page::Artist(first.1))),
            "library" => tasks.push(self.go(Page::Library)),
            "playlists" => {
                self.lib_tab = LibTab::Playlists;
                tasks.push(self.go(Page::Library));
            }
            "playlist" => {
                // a demo playlist with a few songs (saved like any other)
                let ids: Vec<String> = self.lib.tracks().iter().take(5).map(|t| t.id.clone()).collect();
                let id = userdata::new_playlist_id();
                let pl = Playlist { id: id.clone(), name: "Road trip".into(), description: "Made by the native app".into(), owner: userdata::ME.into(), ..Default::default() };
                tasks.push(self.edit(vec![Edit::CreatePlaylist(pl), Edit::AddTracks { id: id.clone(), tracks: ids.clone(), at: userdata::now() }, Edit::Like { track: ids[0].clone(), on: true, at: userdata::now() }]));
                tasks.push(self.go(Page::Playlist(id)));
            }
            "home" => tasks.push(self.go(Page::Home)),
            "settings" => tasks.push(self.go(Page::Settings)),
            "household" => tasks.push(self.go(Page::Household)),
            "notifications" => tasks.push(self.go(Page::Notifications)),
            "dialog" => self.dialog = Some(Dialog::NewPlaylist { name: "Sunday morning".into(), description: String::new(), add: vec![], collaborative: false }),
            s if s.starts_with("getmusic:") => {
                self.find.term = s["getmusic:".len()..].to_string();
                tasks.push(self.go(Page::GetMusic));
                tasks.push(self.update(Message::FindSubmit));
            }
            s if s.starts_with("search:") => {
                self.search = s["search:".len()..].to_string();
                tasks.push(self.go(Page::Search));
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
        let dt = self.last_tick.elapsed().as_secs_f64().min(2.0);
        self.last_tick = Instant::now();

        // count plays (not in a private session)
        if self.status.playing && !self.setting_bool("privateSession", false) {
            if let Some(t) = self.status.track.as_deref().and_then(|id| self.lib.track(id)) {
                let (id, dur) = (t.id.clone(), t.duration);
                let edits = self.counter.tick(&id, dur, dt);
                if !edits.is_empty() {
                    tasks.push(self.edit(edits));
                }
            }
        }
        if ended && self.queue.next(true).is_some() {
            tasks.push(self.start_current(true));
        }
        // keep the sung line in view
        if self.now_playing && self.np_tab == NpTab::Lyrics {
            let line = self
                .status
                .track
                .as_ref()
                .and_then(|id| self.lyrics.get(id))
                .and_then(|r| r.as_ref().ok())
                .and_then(|a| a.lyrics.as_ref())
                .and_then(|l| l.line_at(self.status.position.as_millis() as i64));
            if line != self.lyric_line {
                self.lyric_line = line;
                if let Some(i) = line {
                    let y = (i as f32 * LYRIC_LINE - 160.0).max(0.0);
                    tasks.push(scrollable::scroll_to(lyrics_scroller(), scrollable::AbsoluteOffset { x: 0.0, y }));
                }
            }
        }
        // a failed save: try again
        if self.retry_save_at.map(|t| Instant::now() >= t).unwrap_or(false) {
            self.retry_save_at = None;
            tasks.push(self.flush_edits());
        }
        // tell the household what's playing: on change, and every 15 s while playing
        if let Some(c) = self.client.clone().filter(|_| !self.offline && self.screen == Screen::Main) {
            let share = self.setting_bool("shareActivity", true) && !self.setting_bool("privateSession", false);
            let now = if share { (self.status.track.clone(), self.status.playing) } else { (None, false) };
            if now != self.presence || (now.1 && self.presence_at.elapsed() > Duration::from_secs(15)) {
                self.presence = now.clone();
                self.presence_at = Instant::now();
                let c = c.clone();
                std::thread::spawn(move || {
                    let _ = c.set_now_playing(now.0.as_deref(), now.1);
                });
            }
            // notifications and collaborative playlists every 30 s
            if Instant::now() >= self.next_poll {
                self.next_poll = Instant::now() + Duration::from_secs(30);
                let c2 = c.clone();
                tasks.push(Task::perform(blocking(move || c2.notifications()), Message::NotesLoaded));
                tasks.push(Task::perform(blocking(move || c.shared()), Message::SharedLoaded));
            }
        }
        if self.toast.as_ref().map(|(_, at)| at.elapsed() > Duration::from_secs(4)).unwrap_or(false) {
            self.toast = None;
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
            self.update_state = Some(s);
        }
        Task::batch(tasks)
    }

    fn check_update(&self) {
        let tx = self.update_tx.clone();
        let _ = tx.send(update::State::Checking);
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
        let mut subs = vec![iced::time::every(Duration::from_millis(250)).map(|_| Message::Tick), window::close_requests().map(Message::CloseRequested)];
        // Escape closes a dialog or Now Playing; Space plays/pauses when no text box is in use
        subs.push(iced::keyboard::on_key_press(|key, _mods| {
            use iced::keyboard::{key::Named, Key};
            match key {
                Key::Named(Named::Escape) => Some(Message::DialogCancel),
                Key::Named(Named::MediaPlayPause) => Some(Message::TogglePlay),
                _ => None,
            }
        }));
        Subscription::batch(subs)
    }
}

/// After signing in (or at start with a saved sign-in): who you are, the library, your data and
/// collaborative playlists. If the server can't be reached, open from this device's copy.
fn load_everything(c: Client, dl: Downloads) -> Task<Message> {
    Task::perform(
        blocking(move || -> Result<Loaded, Error> {
            match c.me() {
                Ok(me) => {
                    let cat = c.library()?;
                    let _ = dl.save_catalog(&cat);
                    let data = c.user_data().unwrap_or(None);
                    if let Some(d) = &data {
                        let _ = std::fs::write(app_dir().join("offline").join("userdata.json"), serde_json::to_vec(d).unwrap_or_default());
                    }
                    let shared = c.shared().ok();
                    Ok(Loaded { lib: Arc::new(Library::new(cat)), data, me: Some(me), shared, offline: false })
                }
                Err(Error::Unreachable(why)) => {
                    let cat = dl.load_catalog().ok_or(Error::Unreachable(why))?;
                    let data = std::fs::read(app_dir().join("offline").join("userdata.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
                    Ok(Loaded { lib: Arc::new(Library::new(cat)), data, me: None, shared: None, offline: true })
                }
                Err(e) => Err(e),
            }
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
