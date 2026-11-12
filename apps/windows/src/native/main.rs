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
mod eq;
mod marks;
mod pages;
mod pictures;
mod system;
mod ui;
mod views;
mod webicons;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::widget::{image, scrollable};
use iced::{window, Size, Subscription, Task};
use serde_json::{json, Value};
use volvix_core::cast::{CastDevice, CastStatus, Party};
use volvix_core::client::SignedIn;
use volvix_core::covers::CoverCache;
use volvix_core::downloads::{Downloads, Saved};
use volvix_core::mixes::{self, Mix};
use volvix_core::playlists::{self, PlaylistView};
use volvix_core::session::SavedSession;
use volvix_core::userdata::{self, Edit, PlayCounter};
use volvix_core::{
    Client, Credit, Error, FoundItem, Household, Library, LyricsAnswer, Me, Notes, Playlist, Queue, Request, ServerStatus, SharedInvite, SharedList, SharedOpResult, SharedPlaylist, User, UserData,
};

use settings::Settings;

/// The release file the updater would download. Releases don't carry the native app yet, so until
/// it replaces today's app the updater finds nothing to install.
pub const RELEASE_EXE: &str = "volvix-home-native.exe";
/// Windows startup entry (its own, so it never touches today's app's entry).
pub const RUN_NAME: &str = "Volvix Home Native";

/// %LOCALAPPDATA%\Volvix Home Native: settings, sign-in, covers and downloads. Separate from
/// today's app while both exist.
/// This computer's id among your devices (kept between runs).
fn device_id(dir: &std::path::Path) -> String {
    let file = dir.join("device-id");
    if let Ok(t) = std::fs::read_to_string(&file) {
        let t = t.trim();
        if !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric()) {
            return t.to_string();
        }
    }
    let id = format!("w{:x}{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() as u64 ^ (std::process::id() as u64) << 20).unwrap_or(7), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(&file, &id);
    id
}

/// Keep a line open to the server's device events, reconnecting when it drops, until `gen` moves on.
fn spawn_device_events(client: Client, id: String, out: Arc<std::sync::Mutex<Vec<serde_json::Value>>>, gen: Arc<std::sync::atomic::AtomicU64>, my: u64) {
    use std::io::BufRead;
    use std::sync::atomic::Ordering;
    std::thread::Builder::new()
        .name("devices".into())
        .spawn(move || {
            while gen.load(Ordering::SeqCst) == my {
                let attempt = client.device_events(&id, "Windows PC · app");
                if let Ok(r) = attempt {
                    let mut br = std::io::BufReader::new(r);
                    let mut line = String::new();
                    loop {
                        line.clear();
                        let got = br.read_line(&mut line);
                        match got {
                            Ok(0) | Err(_) => break,
                            Ok(_) => {
                                if gen.load(Ordering::SeqCst) != my {
                                    return;
                                }
                                if let Some(d) = line.trim().strip_prefix("data:") {
                                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(d.trim()) {
                                        out.lock().unwrap().push(v);
                                    }
                                }
                            }
                        }
                    }
                }
                std::thread::sleep(Duration::from_secs(3));
            }
        })
        .ok();
}

pub fn app_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("Volvix Home Native")
}

/// Covers are asked for at this size (the server makes and keeps these copies).
pub const COVER: u32 = 640;
/// Small covers (song rows, the player bar) are their own little bitmaps with a rounder corner for their size.
pub const THUMB: u32 = 96;
/// The app's own title bar: its height, and how wide the three window buttons are (logical px).
pub const TITLE_BAR: f32 = 32.0;
pub const TITLE_BUTTONS: f32 = 46.0 * 3.0;

/// Roughly how tall one lyrics line is, to keep the sung line in view.
pub const LYRIC_LINE: f32 = 50.0;

pub fn lyrics_scroller() -> scrollable::Id {
    scrollable::Id::new("lyrics")
}

/// What a drawn cover look is for.
#[derive(Debug, Clone, PartialEq)]
pub enum LookTarget {
    Album(String),
    Playlist(String),
}

/// What a picture being adjusted is for.
#[derive(Debug, Clone, PartialEq)]
pub enum CropTarget {
    Playlist(String),
    Album(String),
    Avatar,
}

/// The adjust-a-picture dialog: the picture, how far it is zoomed and which spot of it is in the square.
#[derive(Debug, Clone)]
pub struct CropState {
    pub target: CropTarget,
    pub img: std::sync::Arc<::image::DynamicImage>,
    pub zoom: f32,
    pub cx: f32,
    pub cy: f32,
    pub preview: image::Handle,
}

/// The preview is this many px square.
pub const CROP_VIEW: f32 = 300.0;

impl PartialEq for CropState {
    fn eq(&self, o: &CropState) -> bool {
        std::sync::Arc::ptr_eq(&self.img, &o.img) && self.target == o.target && self.zoom == o.zoom && self.cx == o.cx && self.cy == o.cy
    }
}

impl CropState {
    fn shape(&self) -> pictures::Shape {
        if self.target == CropTarget::Avatar { pictures::Shape::Circle } else { pictures::Shape::Rounded }
    }
    /// Side of the square, in the picture's own pixels.
    pub fn side(&self) -> f32 {
        self.img.width().min(self.img.height()) as f32 / self.zoom
    }
    fn clamp(&mut self) {
        let (w, h, s) = (self.img.width() as f32, self.img.height() as f32, self.side());
        self.cx = self.cx.clamp(s / 2.0, (w - s / 2.0).max(s / 2.0));
        self.cy = self.cy.clamp(s / 2.0, (h - s / 2.0).max(s / 2.0));
    }
    fn refresh(&mut self) {
        self.clamp();
        self.preview = pictures::preview(&self.img, self.cx, self.cy, self.side(), 300, self.shape());
    }
    fn new(target: CropTarget, img: std::sync::Arc<::image::DynamicImage>) -> CropState {
        let mut c = CropState { target, cx: img.width() as f32 / 2.0, cy: img.height() as f32 / 2.0, zoom: 1.0, preview: image::Handle::from_rgba(1, 1, vec![0, 0, 0, 0]), img };
        c.refresh();
        c
    }
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
    /// Account → Profile, Members and Settings: three tabs of one page, like the website
    Profile,
    Members,
    Settings,
    /// your year in music (opens once the year has ended)
    Rewind,
    /// the albums of one genre, decade or label (from Library → Browse): the kind, then the value
    Browse(String, String),
    /// an artist from MusicBrainz (by id): their releases, with a way to get what is missing
    Discover(String),
    /// one release from MusicBrainz (by id): its songs, with a way to get it
    DiscoverAlbum(String),
}

/// An artist looked up on MusicBrainz, and what they have released.
#[derive(Debug, Clone, Default)]
pub struct Discovered {
    pub name: String,
    pub albums: Vec<FoundItem>,
    /// the same artist in your library, when you have them
    pub on_server: Option<String>,
    /// their picture (a path on the server)
    pub image: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LibTab {
    Albums,
    Artists,
    Songs,
    Playlists,
    Browse,
    /// only in the app: what's kept on this computer
    Downloaded,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NpTab {
    Queue,
    Lyrics,
    Credits,
}

/// What a row is being dragged within.
#[derive(Debug, Clone, PartialEq)]
pub enum DragList {
    Queue,
    Playlist(String),
}

/// A row that has been pressed (and, once the pointer has moved a little, picked up) in a list you can reorder.
#[derive(Debug, Clone)]
pub struct Drag {
    pub list: DragList,
    pub from: usize,
    /// where it would land if let go now
    pub over: usize,
    /// what a plain click on the row does (letting go without having moved)
    pub click: Option<Box<Message>>,
    /// where the pointer was pressed (window pixels); 5 px from there the row is picked up
    pub start: (f32, f32),
    pub active: bool,
    /// how many rows the list has, how tall each is, and the label between two of them (before which row, how tall)
    pub count: usize,
    pub row_h: f32,
    pub divider: Option<(usize, f32)>,
    /// the pointer's height within the list, and how far into the row it took hold
    pub y: f32,
    pub grab: Option<f32>,
    /// the room opened before each of the other rows (and after the last), in rows; it eases toward `over`
    pub gaps: Vec<f32>,
    pub last_frame: Instant,
}

impl Drag {
    /// Where row `i` starts when nothing is being moved.
    pub fn row_top(&self, i: usize) -> f32 {
        i as f32 * self.row_h + self.divider.filter(|(at, _)| i >= *at).map(|(_, h)| h).unwrap_or(0.0)
    }

    /// Where the picked-up card's top is (it stays inside the list).
    pub fn card_top(&self) -> f32 {
        (self.y - self.grab.unwrap_or(0.0)).clamp(0.0, self.row_top(self.count.saturating_sub(1)))
    }

    /// Work out where it would land: after as many rows as have their middle above the card's middle (the website's rule).
    fn place(&mut self) {
        let mid = self.card_top() + self.row_h / 2.0;
        let above = (0..self.count).filter(|j| *j != self.from && self.row_top(*j) + self.row_h / 2.0 < mid).count();
        self.over = above.min(self.count.saturating_sub(1));
    }
}

/// The page's scrolling area and the queue's (a picked-up row near their top or bottom edge scrolls them).
pub fn main_scroller() -> scrollable::Id {
    scrollable::Id::new("main")
}
pub fn queue_scroller() -> scrollable::Id {
    scrollable::Id::new("queue")
}

/// What a pop-up menu (the more button) is for.
#[derive(Debug, Clone)]
pub enum MenuKind {
    /// a song: `in_playlist` = (the playlist, its place in it) when it can be taken out
    Track { id: String, in_playlist: Option<(String, usize)> },
    Playlist(String),
    /// an album's ⋯ (its id)
    Album(String),
    /// the sound-quality mark of the player bar: what the song that plays is (a note, nothing to choose)
    Quality { title: String, detail: String },
}

#[derive(Debug, Clone)]
pub struct PopMenu {
    pub kind: MenuKind,
    /// where the pointer was when it opened (window pixels)
    pub at: (f32, f32),
}

/// The pointer and the window size, kept by a listener that costs nothing per move (it never sends a message).
static POINTER: std::sync::Mutex<(f32, f32)> = std::sync::Mutex::new((0.0, 0.0));
static WINDOW_SIZE: std::sync::Mutex<(f32, f32)> = std::sync::Mutex::new((1200.0, 800.0));

/// Things the screens show that are worth working out once per change, not on every redraw
/// (a redraw happens for every tick of the clock and every mouse move).
#[derive(Default)]
pub struct Cache {
    pub playlists: Vec<PlaylistView>,
    /// the search words these results are for, and the results (ids)
    pub search: Option<(String, Vec<String>, Vec<String>, Vec<String>)>,
    pub top_albums: Vec<String>,
    pub picks: Vec<String>,
    /// the playing song is in one of your playlists (the mini player's tick)
    pub current_in_playlist: bool,
}

/// Search's filter chips, as on the website.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchType {
    All,
    Artists,
    Albums,
    Songs,
    Playlists,
}

impl SearchType {
    /// Which "Get more music" kinds this filter shows.
    pub fn kinds(self) -> &'static [&'static str] {
        match self {
            SearchType::All => &["artists", "songs", "albums"],
            SearchType::Artists => &["artists"],
            SearchType::Albums => &["albums"],
            SearchType::Songs => &["songs"],
            SearchType::Playlists => &[],
        }
    }
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
    NewPlaylist { name: String, description: String, add: Vec<String>, collaborative: bool, public: bool },
    EditPlaylist { id: String, name: String, description: String, public: bool },
    DeletePlaylist { id: String, name: String },
    AddTo { tracks: Vec<String> },
    Password { current: String, new: String, error: Option<String>, busy: bool },
    Rename { name: String, error: Option<String> },
    /// the cast button: this device, TVs and speakers, and the listening party
    PlayOn,
    AddCast { host: String, error: Option<String>, busy: bool },
    PartyStart { invite: Vec<String> },
    PartyInvite { invite: Vec<String> },
    /// an admin's controls for one member: explicit songs, what they may do, a new password, remove
    Manage { id: String, password: String, error: Option<String>, confirm_remove: bool },
    /// pick the server's music folder
    Folder { browse: Option<volvix_core::admin::Browse>, error: Option<String> },
    /// a song's credits (from its ⋯ menu)
    Credits { track: String },
    /// an album's info and who is credited on it
    AlbumInfo { album: String },
    /// choosing how an album without art is drawn
    CoverLook { target: LookTarget, name: String, look: volvix_core::Look },
    /// adjusting a picture (move it, zoom it) before it is used
    Crop(Box<CropState>),
    /// make a share link for an album or playlist: how long, then the link to copy
    Share { kind: String, name: String, tracks: Vec<String>, days: u32, link: Option<(String, String)>, error: Option<String>, busy: bool },
    /// who can edit a collaborative playlist (ticked = can edit, or invited)
    Collab { id: String, picked: Vec<String> },
}

/// Playing on a TV or speaker: the app keeps the queue and the clock but stays silent, and tells the TV
/// what to do (like the website).
#[derive(Debug, Clone)]
pub struct CastLink {
    pub device: String,
    pub name: String,
    /// the song the TV was last asked to play
    loaded: Option<String>,
    sent_playing: bool,
    sent_volume: f32,
}

/// "Get more music": looking through MusicBrainz for music you don't have.
#[derive(Debug, Clone, Default)]
pub struct FindState {
    /// the words these results are for
    pub searched: String,
    /// results per kind ("artists", "songs", "albums") as they arrive
    pub parts: HashMap<String, Result<Vec<FoundItem>, String>>,
    /// kinds showing everything ("Show N more")
    pub open: HashSet<String>,
    pub can_get: bool,
    pub configured: bool,
    seq: u64,
    /// foreign ids asked for in this session (shown as "Requested")
    pub asked: HashSet<String>,
    pub asking: HashSet<String>,
    pub requests: Vec<Request>,
    /// what Lidarr is downloading right now (`/api/lidarr/queue`)
    pub queue: Option<serde_json::Value>,
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
    CoverLoaded(String, u32, Option<(u32, u32, Vec<u8>)>),
    ImageLoaded(String, Option<Vec<u8>>),

    PlayTracks(Vec<String>, usize, String),
    ShuffleTracks(Vec<String>, String),
    PlayAlbum(String, usize),
    ShuffleAlbum(String),
    AddToQueue(String),
    Forward,
    /// drag to reorder: picked up a row, moved over another, let go anywhere
    /// a row was pressed: the list, the row, how many rows, the label between rows, the row height, what a click does
    DragStart(DragList, usize, usize, Option<(usize, f32)>, f32, Option<Box<Message>>),
    /// the pointer's height within that list
    DragAt(f32),
    /// a new frame while a row is picked up (the others ease out of its way; the list scrolls near its edges)
    DragFrame,
    /// a scrolling area moved (0 the page, 1 the queue): how far down it is now
    Scrolled(u8, f32),
    /// the pop-up menu: open for a song or playlist, pick something from it (closes it), close it
    PopMenuOpen(MenuKind),
    PopMenuDo(Box<Message>),
    PopMenuClose,
    /// the row under the pointer (lit up, and shows its grip)
    RowHover(Option<(u64, usize)>),
    PlayNext(String),
    /// a song's credits in a window; a share link for an album ("album") or playlist: open, pick days, make it
    ShowCredits(String),
    ShowAlbumInfo(String),
    CoverOpen(String),
    CoverOpenPlaylist(String),
    CoverBg(String),
    CoverFont(String),
    /// 0 primary, 1 secondary, 2 text: a colour (hex) picked or typed
    CoverColour(u8, String),
    CoverResetColours,
    CoverReset,
    CoverSaved(String, Option<volvix_core::Look>, Result<(), Error>),
    PickAlbumPicture(String),
    AlbumPictureChosen(String, Option<std::path::PathBuf>),
    AlbumPictureSaved(String, Result<u64, String>),
    RemoveAlbumPicture(String),
    AlbumPictureRemoved(String, Result<(), Error>),
    CropLoaded(CropTarget, Result<std::sync::Arc<::image::DynamicImage>, String>),
    CropZoom(f32),
    CropDown,
    CropMove(iced::Point),
    CropUp,
    ShareOpen(String, String, Vec<String>),
    ShareDays(u32),
    ShareCreate,
    ShareCreated(Result<volvix_core::admin::Share, Error>),
    /// who can edit a playlist (your own one becomes collaborative first)
    CollabOpen(String),
    /// make a collaborative playlist just yours again (everyone else loses access)
    Unshare(String),
    Unshared(Result<SharedOpResult, Error>),
    LibSort(String),
    LibScope(bool),
    FollowArtist(String),
    RowLeave(u64, usize),
    /// add every song of a playlist to the queue
    QueueMany(Vec<String>),
    PlayNextMany(Vec<String>),
    /// a playlist's own picture: pick a file, it was saved on the server, remove it; and one arriving from the server
    PickPlaylistPicture(String),
    PlaylistPictureChosen(String, Option<std::path::PathBuf>),
    PlaylistPictureSaved(String, Result<u64, String>),
    RemovePlaylistPicture(String),
    PlaylistPictureRemoved(String, Result<(), Error>),
    PlaylistCoverLoaded(String, u64, Option<Vec<u8>>),
    /// the playlist page's Add songs box
    AddSongQuery(String),
    AddSongTo(String, String),
    DragEnd,
    TogglePlay,
    /* the cast button: TVs, speakers and listening parties */
    PlayOn,
    /// the pointer rests on / leaves a button that has a hover hint
    HintOn(String),
    HintOff,
    CastDevices(Result<Vec<CastDevice>, Error>),
    CastStart(usize),
    CastStop,
    CastAddOpen,
    CastAddDone(Result<CastDevice, Error>),
    CastPlayed(Result<(), Error>),
    CastStatus(Result<CastStatus, Error>),
    PartyLoaded(Result<Option<Party>, Error>),
    PartyStartOpen,
    PartyInviteOpen,
    PartyPick(String, bool),
    PartyLeave,
    PartyDone(Result<Option<Party>, Error>, String),
    Next,
    Previous,
    ToggleShuffle,
    CycleRepeat,
    SeekTo(f32),
    Volume(f32),
    ToggleMute,
    JumpQueue(usize),
    RemoveFromQueue(usize),
    ClearQueue,
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
    /// your data as the server has it now (asked for every few seconds, to pick up changes made on the website or another device)
    DataPulled(Result<Option<UserData>, Error>),
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
    SearchType(SearchType),
    /// typing stopped for a moment: look further (Lidarr) for these words
    SearchSettled(String),
    FoundPart(u64, String, Result<(Vec<FoundItem>, bool, bool), Error>),
    ShowMoreFound(String),
    RequestMusic(String, String),
    Requested(String, Result<(), Error>),
    RequestsLoaded(Result<Vec<Request>, Error>),
    QueueLoaded(Result<serde_json::Value, Error>),
    ArtistInfoLoaded(String, Option<String>, Option<Vec<u8>>),
    ArtistMbid(String, Option<String>),
    DiscoverLoaded(String, Result<serde_json::Value, Error>),
    /// the server was asked whether it is back (true = it answered)
    ServerAnswered(bool),
    DiscoverAlbumLoaded(String, Result<serde_json::Value, Error>),
    DescEdit(iced::widget::text_editor::Action),
    CancelRequest(String),

    /* downloads */
    ToggleSaved(String, String),
    DownloadDone(String, Result<(), String>),
    RemoveAllDownloads,

    /* settings */
    SetLeveling(String),
    CreditsLoaded(String, Result<Vec<Credit>, Error>),

    /* the window's own title bar */
    TitlePress,
    Minimize,
    ToggleMaximize,
    CloseWindow,
    Maximized(bool),
    CheckMaximized,
    SetShare(bool),
    SetPrivate(bool),
    SetStartWithWindows(bool),
    SetCloseToTray(bool),
    SetAutoUpdate(bool),
    SetDownloadMetered(bool),
    /// the account menu under the profile button (Settings, Sign out)
    AccountMenu(bool),
    SetResume(bool),
    SetSleep(String),
    /// crossfade length in seconds ("0" = off), and whether songs on the same album fade into each other too
    SetCrossfade(String),
    SetCrossfadeAlbums(bool),
    SetTheme(String),
    /// Profile → Change picture / Remove: pick a file, send it, remove it; and a picture arriving from the server
    PickPicture,
    PictureChosen(Option<std::path::PathBuf>),
    PictureSaved(Result<u64, String>),
    RemovePicture,
    PictureRemoved(Result<(), Error>),
    AvatarLoaded(String, u64, Option<Vec<u8>>),
    /// the equalizer: on/off, a preset by name, the preamp (dB), one band (index, dB), back to flat
    EqOn(bool),
    EqPreset(String),
    EqPre(f32),
    EqBand(usize, f32),
    EqReset,
    SetStreamQuality(String),
    SetDownloadQuality(String),
    SetOfflineOnly(bool),
    SetCacheSongs(bool),
    SetCacheLimit(String),
    ClearCache,
    DeviceTransfer(String),
    DeviceDone(Result<(), Error>),
    DeviceHere,
    CacheFetched(String, Result<(), Error>),
    SetDataSaver(bool),
    /// a text box on the Profile or Members tab: 0 your name, 1 current password, 2 new password, 3 member name, 4 member password
    ProfileField(u8, String),
    SaveName,
    SavePassword,
    AddMember,
    MembersLoaded(Result<Vec<volvix_core::members::Member>, Error>),
    RewindYear(i32),
    LidarrLoaded(Result<serde_json::Value, Error>),
    /// the Lidarr form: 0 = root folder, 1 = quality profile, 2 = metadata profile (a name picked from the list)
    LidarrPick(u8, String),
    LidarrSave,
    LidarrOff,
    LidarrSaved(Result<serde_json::Value, Error>),
    FolderLoaded(Result<volvix_core::admin::LibraryFolder, Error>),
    SharesLoaded(Result<Vec<volvix_core::admin::Share>, Error>),
    Rescan,
    RescanDone(Result<(), Error>),
    /// the music folder picker: open it, go into a folder, use this folder
    PickFolder,
    BrowseTo(String),
    Browsed(Result<volvix_core::admin::Browse, Error>),
    ChooseFolder,
    FolderSaved(Result<(), Error>),
    ShareCopy(String),
    ShareCancel(String),
    ShareCancelled(Result<(), Error>),
    /// the Manage window: open it for a member, switch one of its options, set a password, remove the member
    ManageMember(String),
    /// "hideExplicit" or one permission (request, rescan, parties, members, settings, share)
    ManageToggle(String),
    /// all permissions (true) or the basics only (false)
    ManagePreset(bool),
    MemberEdited(Result<volvix_core::members::Member, Error>),
    ManagePassword,
    ManageRemove,
    ManageDone(Result<(), Error>, &'static str),
    MemberAdded(Result<(), Error>),
    CheckUpdate,
    ChangePassword,
    RenameMe,
    ServerStatus(Result<ServerStatus, Error>),

    Tick,
    /// a new frame while the launch screen animates
    Frame,
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
    pub credits: HashMap<String, Result<Vec<Credit>, String>>,
    lyric_line: Option<usize>,
    pub search_type: SearchType,
    last_title_press: Option<Instant>,
    pub maximized: bool,
    hwnd: Option<isize>,
    pub find: FindState,

    pub downloads: Downloads,
    /// songs kept as they are queued, and which are being fetched now
    pub song_cache: volvix_core::cache::SongCache,
    caching: HashSet<String>,
    next_cache_at: Instant,
    /// (songs, bytes) in the cache, for Settings
    pub cache_stats: (usize, u64),
    /// dragging the picture in the adjust dialog: whether the pointer is down, and where it was last
    crop_drag: bool,
    crop_last: Option<iced::Point>,
    /// your other devices (this computer, the website, a phone…): who is online and which one plays
    pub dev_id: String,
    pub connect_devices: Vec<(String, String, bool)>,
    pub dev_active: Option<String>,
    dev_events: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    dev_gen: Arc<std::sync::atomic::AtomicU64>,
    dev_started: bool,
    /// what this computer last told the server it is playing (and when), so changes are sent and the rest is a heartbeat
    dev_now_key: String,
    dev_now_at: Instant,
    /// another device is playing: this one shows it (no sound) and sends it what you press
    remote: Option<RemoteNow>,
    dev_now: Option<serde_json::Value>,
    /// right after starting something here, the old player's last report must not pull this one back
    remote_block: Instant,
    /// where the song that was just handed over starts
    pending_start: Duration,
    pub downloaded: HashSet<String>,
    pub saved: Vec<Saved>,
    dl_queue: VecDeque<String>,
    pub dl_active: Option<String>,

    covers: CoverCache,
    pub cover_images: HashMap<(String, u32), image::Handle>,
    cover_pending: HashSet<(String, u32)>,
    /// the order covers were loaded in, so the oldest can be let go when there are a great many
    cover_order: std::collections::VecDeque<(String, u32)>,
    pub images: HashMap<String, image::Handle>,
    image_pending: HashSet<String>,
    /// artist photos being fetched for the grids
    artist_pending: HashSet<String>,

    pub page: Page,
    pub history: Vec<Page>,
    pub forward: Vec<Page>,
    pub drag: Option<Drag>,
    /// pictures chosen for playlists, by (playlist, version); and the ones being fetched
    pl_covers: HashMap<(String, u64), image::Handle>,
    pl_cover_pending: HashSet<(String, u64)>,
    pub pop_menu: Option<PopMenu>,
    /// the row under the pointer: which list on the page (see `views::list_key`) and which row
    pub hover_row: Option<(u64, usize)>,
    /// people to invite once a playlist of yours has become collaborative
    collab_invite: Vec<String>,
    /// Library: how albums and songs are sorted (added, title, artist, year), and whether Artists shows everyone
    pub lib_sort: String,
    pub lib_all_artists: bool,
    scroll_y: [f32; 2],
    pub add_q: String,
    pub cache: Cache,
    start_hidden: bool,
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
    pub cast: Option<CastLink>,
    pub cast_devices: Vec<CastDevice>,
    pub party: Option<Party>,
    cast_poll: Instant,
    party_poll: Instant,
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
    /// the sleep timer: pause at this time, or when the song ends
    pub sleep_choice: String,
    sleep_until: Option<Instant>,
    sleep_song: bool,
    /// where the last session stopped has been put back (once per start)
    resumed: bool,
    resume_saved_at: Instant,
    last_advanced: u64,
    next_pull: Instant,
    /// when the server could not be reached: when to look again
    next_reconnect: Instant,
    /// the song whose successor has been opened ahead of time (crossfade)
    preload_for: Option<String>,
    pub account_menu: bool,
    /// Account → Profile: the name being typed (None: as it is now), and the two password fields
    pub profile_name: Option<String>,
    pub pw_current: String,
    pub pw_new: String,
    /// Account → Members
    pub members: Vec<volvix_core::members::Member>,
    pub new_member_name: String,
    pub new_member_pw: String,
    pub member_error: Option<String>,
    pub rewind_year: Option<i32>,
    /// Settings → Library and Share links (loaded when Settings opens)
    /// profile pictures by (person, version), cut round; and the ones being fetched
    avatars: HashMap<(String, u64), image::Handle>,
    /// artist name -> (bio, round photo) as the server knows them
    pub artist_infos: HashMap<String, (Option<String>, Option<image::Handle>)>,
    /// MusicBrainz artists looked up (by id), and which id belongs to each library artist (lower-case name)
    pub discovered: HashMap<String, Discovered>,
    /// releases looked up on MusicBrainz, by id
    pub album_pages: HashMap<String, serde_json::Value>,
    pub artist_mbids: HashMap<String, Option<String>>,
    /// the multi-line description box of the New / Edit playlist dialogs
    pub desc_editor: iced::widget::text_editor::Content,
    avatar_pending: HashSet<(String, u64)>,
    /// Settings → Lidarr: what the server says, and what is being typed / picked
    pub lidarr: Option<serde_json::Value>,
    pub lidarr_url: Option<String>,
    pub lidarr_key: String,
    pub lidarr_root: Option<String>,
    pub lidarr_quality: Option<String>,
    pub lidarr_meta: Option<String>,
    pub lidarr_error: Option<String>,
    pub folder: Option<volvix_core::admin::LibraryFolder>,
    pub shares: Option<Vec<volvix_core::admin::Share>>,
}

fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> impl std::future::Future<Output = T> {
    async move { tokio::task::spawn_blocking(f).await.expect("worker thread") }
}

impl App {
    fn new(instance: Receiver<()>, start_hidden: bool) -> (App, Task<Message>) {
        let dir = app_dir();
        ui::since_start(); // the launch screen's clock starts now
        let saved = SavedSession::load(&dir);
        let settings = Settings::load();
        let (update_tx, update_rx) = mpsc::channel();
        let player = audio::Player::start();
        let tray = system::Tray::new(autostart_registered(), settings.close_to_tray, settings.auto_update);
        let downloads = Downloads::new(dir.join("offline"));
        let song_cache = volvix_core::cache::SongCache::new(dir.join("cache"));
        let mut app = App {
            cache_stats: song_cache.stats(),
            crop_drag: false,
            crop_last: None,
            dev_id: device_id(&dir),
            connect_devices: vec![],
            dev_active: None,
            dev_events: Arc::new(std::sync::Mutex::new(vec![])),
            dev_gen: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            dev_started: false,
            dev_now_key: String::new(),
            dev_now_at: Instant::now(),
            remote: None,
            dev_now: None,
            remote_block: Instant::now(),
            pending_start: Duration::ZERO,
            song_cache,
            caching: HashSet::new(),
            next_cache_at: Instant::now() + Duration::from_secs(2),
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
            credits: HashMap::new(),
            lyric_line: None,
            search_type: SearchType::All,
            last_title_press: None,
            maximized: false,
            hwnd: None,
            find: FindState::default(),
            dl_queue: VecDeque::new(),
            dl_active: None,
            cover_images: HashMap::new(),
            cover_pending: HashSet::new(),
            cover_order: std::collections::VecDeque::new(),
            images: HashMap::new(),
            image_pending: HashSet::new(),
            artist_pending: HashSet::new(),
            page: Page::Home,
            history: vec![],
            forward: vec![],
            drag: None,
            pl_covers: HashMap::new(),
            pl_cover_pending: HashSet::new(),
            pop_menu: None,
            hover_row: None,
            collab_invite: vec![],
            lib_sort: "added".into(),
            lib_all_artists: false,
            scroll_y: [0.0; 2],
            add_q: String::new(),
            cache: Cache::default(),
            start_hidden,
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
            cast: None,
            cast_devices: vec![],
            party: None,
            cast_poll: Instant::now(),
            party_poll: Instant::now(),
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
            sleep_choice: "0".into(),
            sleep_until: None,
            sleep_song: false,
            resumed: false,
            resume_saved_at: Instant::now(),
            last_advanced: 0,
            next_pull: Instant::now() + Duration::from_secs(6),
            next_reconnect: Instant::now() + Duration::from_secs(8),
            preload_for: None,
            account_menu: false,
            profile_name: None,
            pw_current: String::new(),
            pw_new: String::new(),
            members: vec![],
            new_member_name: String::new(),
            new_member_pw: String::new(),
            member_error: None,
            rewind_year: None,
            avatars: HashMap::new(),
            artist_infos: HashMap::new(),
            discovered: HashMap::new(),
            album_pages: HashMap::new(),
            artist_mbids: HashMap::new(),
            desc_editor: iced::widget::text_editor::Content::new(),
            avatar_pending: HashSet::new(),
            lidarr: None,
            lidarr_url: None,
            lidarr_key: String::new(),
            lidarr_root: None,
            lidarr_quality: None,
            lidarr_meta: None,
            lidarr_error: None,
            folder: None,
            shares: None,
        };
        app.apply_volume();
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

    /// Every playlist you can see (yours, collaborative, the household's), as of the last change.
    pub fn playlists(&self) -> &[PlaylistView] {
        &self.cache.playlists
    }

    pub fn playlist(&self, id: &str) -> Option<PlaylistView> {
        self.cache.playlists.iter().find(|p| p.id == id).cloned()
    }

    /// Work out again what the screens show (after anything that may have changed it).
    fn refresh_cache(&mut self) {
        self.cache.playlists = playlists::all(&self.data, &self.shared, &self.household, &self.me_id());
        let q = self.search.trim().to_string();
        if !q.is_empty() && self.cache.search.as_ref().map(|s| s.0 != q).unwrap_or(true) {
            let r = volvix_core::search::search(&self.lib, &[], &q);
            let ids = |v: Vec<&volvix_core::Album>| v.into_iter().map(|a| a.id.clone()).collect::<Vec<_>>();
            self.cache.search = Some((q, r.artists, ids(r.albums), r.songs.into_iter().map(|t| t.id.clone()).collect()));
        }
        self.cache.top_albums = mixes::top_albums(&self.lib, &self.data, 6).into_iter().map(|a| a.id.clone()).collect();
        self.cache.picks = mixes::random_albums(&self.lib, &self.me_id(), userdata::now(), 6).into_iter().map(|a| a.id.clone()).collect();
        let cur = self.status.track.clone();
        self.cache.current_in_playlist = cur.map(|id| self.cache.playlists.iter().any(|p| p.can_edit() && p.kind != playlists::Kind::Liked && p.tracks.contains(&id))).unwrap_or(false);
    }

    /// Every message lands here; the cache is refreshed afterwards unless it's one of the frequent
    /// ones (the clock, a slider, hovering while dragging) that can't change what it holds.
    /// The theme in your settings (shared with the website): "system", "dark" or "light".
    pub fn theme_choice(&self) -> String {
        self.data.rest.get("theme").and_then(|v| v.as_str()).unwrap_or("system").to_string()
    }

    fn apply_theme(&self) {
        // debug builds only: VH_THEME=light | dark previews a theme without touching your saved setting
        #[cfg(debug_assertions)]
        if let Ok(t) = std::env::var("VH_THEME") {
            ui::set_light(t == "light");
            return;
        }
        ui::set_light(match self.theme_choice().as_str() {
            "light" => true,
            "dark" => false,
            _ => system::system_uses_light(),
        });
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        self.apply_theme();
        if self.account_menu && matches!(msg, Message::Go(_) | Message::SignOut) {
            self.account_menu = false; // a choice in the account menu closes it
        }
        let hot = matches!(msg, Message::Tick | Message::Volume(_) | Message::DragAt(_) | Message::DragFrame | Message::Scrolled(..) | Message::RowHover(_) | Message::RowLeave(..) | Message::CoverLoaded(..) | Message::ImageLoaded(..));
        let track_before = self.status.track.clone();
        let t = self.handle(msg);
        if !hot || self.status.track != track_before {
            self.refresh_cache();
        }
        t
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
            self.forward.clear();
            self.history.push(std::mem::replace(&mut self.page, page.clone()));
            if self.history.len() > 50 {
                self.history.remove(0);
            }
        }
        self.now_playing = false;
        self.hover_row = None;
        let mut tasks = vec![self.fetch_covers()];
        if let Some(c) = self.client.clone().filter(|_| !self.offline) {
            match page {
                Page::Home => self.refresh_mixes(),
                Page::Household => {
                    let c2 = c.clone();
                    tasks.push(Task::perform(blocking(move || c2.household()), Message::HouseholdLoaded));
                    tasks.push(Task::perform(blocking(move || c.shared()), Message::SharedLoaded));
                }
                Page::Artist(name) => {
                    // the MusicBrainz side: which artist this is, so "Get more" can list what is missing
                    if self.me.as_ref().map(|m| m.lidarr.configured).unwrap_or(false) && !self.artist_mbids.contains_key(&name.to_lowercase()) {
                        let (c2, n2) = (c.clone(), name.clone());
                        tasks.push(Task::perform(
                            blocking(move || {
                                let (list, _, _) = c2.find_part(&n2, "artists")?;
                                Ok::<_, Error>(list.into_iter().find(|x| x.label().eq_ignore_ascii_case(&n2) && x.foreign_id.is_some()).and_then(|x| x.foreign_id))
                            }),
                            {
                                let n3 = name.clone();
                                move |r| Message::ArtistMbid(n3.clone(), r.ok().flatten())
                            },
                        ));
                    }
                    if self.me.as_ref().map(|m| m.artist_info).unwrap_or(false) && !self.artist_infos.contains_key(&name) {
                    let n = name.clone();
                    tasks.push(Task::perform(
                        blocking(move || {
                            let v = c.artist_info(&n).map_err(|e| e.to_string())?;
                            let bio = v["bio"].as_str().map(String::from);
                            let img = match v["image"].as_str() {
                                Some(p) => c.image_bytes(p).ok(),
                                None => None,
                            };
                            Ok::<_, String>((bio, img))
                        }),
                        move |r| match r {
                            Ok((bio, img)) => Message::ArtistInfoLoaded(name.clone(), bio, img),
                            Err(_) => Message::ArtistInfoLoaded(name.clone(), None, None),
                        },
                    ));
                    }
                }
                Page::Discover(id) if !self.discovered.contains_key(&id) => {
                    let i = id.clone();
                    tasks.push(Task::perform(blocking(move || c.discover_artist(&i)), move |r| Message::DiscoverLoaded(id.clone(), r)));
                }
                Page::DiscoverAlbum(id) if !self.album_pages.contains_key(&id) => {
                    let i = id.clone();
                    tasks.push(Task::perform(blocking(move || c.discover_album(&i)), move |r| Message::DiscoverAlbumLoaded(id.clone(), r)));
                }
                Page::Notifications => tasks.push(Task::perform(blocking(move || c.notifications()), Message::NotesLoaded)),
                Page::GetMusic => {
                    let c2 = c.clone();
                    tasks.push(Task::perform(blocking(move || c2.lidarr_queue()), Message::QueueLoaded));
                    tasks.push(Task::perform(blocking(move || c.requests()), Message::RequestsLoaded));
                }
                Page::Settings => {
                    let (c2, c3) = (c.clone(), c.clone());
                    tasks.push(Task::perform(blocking(move || c3.shares()), Message::SharesLoaded));
                    if self.can("settings") {
                        let c4 = c2.clone();
                        tasks.push(Task::perform(blocking(move || c4.lidarr_settings()), Message::LidarrLoaded));
                        tasks.push(Task::perform(blocking(move || c2.library_folder()), Message::FolderLoaded));
                    }
                    tasks.push(Task::perform(blocking(move || c.status()), Message::ServerStatus));
                }
                Page::Members => tasks.push(Task::perform(blocking(move || c.members()), Message::MembersLoaded)),
                Page::Profile => self.profile_name = None,
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
        let wanted: Vec<(String, u32)> = views::wanted_covers(self).into_iter().flat_map(|(id, s)| [(id.clone(), s), (id, THUMB)]).collect();
        for (id, size) in wanted {
            let key = (id.clone(), size);
            if self.cover_images.contains_key(&key) || self.cover_pending.contains(&key) {
                continue;
            }
            let Some(album) = self.lib.album(&id) else { continue };
            if !album.cover {
                continue;
            }
            let version = album.cover_v;
            // from disk when it is there, else from the server; then shrunk and given rounded corners (off the main thread)
            self.cover_pending.insert(key);
            let offline = self.offline;
            let (c, cache, fetch_id) = (client.clone(), self.covers.clone(), id.clone());
            tasks.push(Task::perform(
                blocking(move || {
                    // a small cover is cut from the normal copy
                    let (from, max, frac) = if size == THUMB { (COVER, 96, 0.11) } else { (size, 288, 0.045) };
                    let bytes = if offline { cache.cached(&fetch_id, from, version) } else { cache.get(&c, &fetch_id, from, version).ok() }?;
                    pictures::cover_rgba(&bytes, max, frac)
                }),
                move |b| Message::CoverLoaded(id.clone(), size, b),
            ));
        }
        // photos of the artists on screen (a few at a time)
        if self.me.as_ref().map(|m| m.artist_info).unwrap_or(false) && !self.offline && self.artist_pending.len() < 4 {
            let names: Vec<String> = match (&self.page, self.lib_tab) {
                (Page::Home, _) => self.lib.top_artists(12),
                (Page::Library, LibTab::Artists) => self.lib.artists().iter().filter(|n| self.lib_all_artists || self.follows(n)).take(60).cloned().collect(),
                _ => vec![],
            };
            for name in names {
                if self.artist_pending.len() >= 4 {
                    break;
                }
                if self.artist_infos.contains_key(&name) || !self.artist_pending.insert(name.clone()) {
                    continue;
                }
                let c = client.clone();
                let n = name.clone();
                tasks.push(Task::perform(
                    blocking(move || {
                        let v = c.artist_info(&n).map_err(|e| e.to_string())?;
                        let bio = v["bio"].as_str().map(String::from);
                        let img = v["image"].as_str().and_then(|p| c.image_bytes(p).ok());
                        Ok::<_, String>((bio, img))
                    }),
                    move |r| match r {
                        Ok((bio, img)) => Message::ArtistInfoLoaded(name.clone(), bio, img),
                        Err(_) => Message::ArtistInfoLoaded(name.clone(), None, None),
                    },
                ));
            }
        }
        // pictures of music found through Lidarr
        if self.page == Page::Search || matches!(self.page, Page::Artist(_) | Page::Discover(_) | Page::DiscoverAlbum(_)) {
            let mut paths: Vec<String> = self.find.parts.values().filter_map(|r| r.as_ref().ok()).flatten().filter_map(|x| x.image.clone()).collect();
            paths.extend(self.discovered.values().flat_map(|d| d.albums.iter()).filter_map(|x| x.image.clone()));
            paths.extend(self.discovered.values().filter_map(|d| d.image.clone()));
            paths.extend(self.album_pages.values().filter_map(|v| v["image"].as_str().map(String::from)));
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

    /// The sound this device makes: none while a TV or speaker is playing (it keeps the clock, the TV makes the sound).
    fn apply_volume(&mut self) {
        let v = if self.cast.is_some() { 0.0 } else { self.volume * self.volume };
        self.player.set_volume(v);
    }

    pub fn my_id(&self) -> String {
        self.me.as_ref().map(|m| m.user.id.clone()).unwrap_or_default()
    }

    /// A household member's first name.
    pub fn person_name(&self, id: &str) -> String {
        let me = self.me.as_ref().filter(|m| m.user.id == id).map(|m| m.user.name.clone());
        let name = me.or_else(|| self.household.members.iter().find(|m| m.id == id).map(|m| m.name.clone())).unwrap_or_else(|| "Someone".into());
        name.split_whitespace().next().unwrap_or("Someone").to_string()
    }

    /// Whether this account may start listening parties (admins always).
    /// Do you follow this artist (Library → Artists → Following)?
    pub fn follows(&self, name: &str) -> bool {
        self.data.rest.get("followedArtists").and_then(|l| l.as_array()).map(|l| l.iter().any(|n| n.as_str().map(|n| n.eq_ignore_ascii_case(name)).unwrap_or(false))).unwrap_or(false)
    }

    /// Is this person allowed to do `what` (admins can do everything)?
    pub fn can(&self, what: &str) -> bool {
        self.me.as_ref().map(|m| m.user.admin || m.user.perms.iter().any(|p| p == what)).unwrap_or(false)
    }

    fn may_party(&self) -> bool {
        self.me.as_ref().map(|m| m.user.admin || m.user.perms.iter().any(|p| p == "parties")).unwrap_or(false)
    }

    /// Everyone else in the household, for inviting.
    pub fn others(&self) -> Vec<(String, String)> {
        let me = self.my_id();
        self.household.members.iter().filter(|m| m.id != me).map(|m| (m.id.clone(), m.name.clone())).collect()
    }

    fn end_cast(&mut self, message: &str) {
        if self.cast.take().is_some() {
            self.apply_volume();
            self.say(message.to_string());
        }
    }

    /// Keep the TV doing what this app says: the song, play / pause, volume.
    fn cast_sync(&mut self) -> Task<Message> {
        let Some(c) = self.client.clone() else { return Task::none() };
        let Some(link) = self.cast.as_mut() else { return Task::none() };
        let dev = link.device.clone();
        let mut tasks = vec![];
        if let Some(t) = self.status.track.clone() {
            if link.loaded.as_deref() != Some(t.as_str()) {
                link.loaded = Some(t.clone());
                link.sent_playing = self.status.playing;
                let (at, playing, c, dev) = (self.status.position.as_secs_f64(), self.status.playing, c.clone(), dev.clone());
                tasks.push(Task::perform(blocking(move || c.cast_play(&dev, &t, at, playing)), Message::CastPlayed));
            } else if link.sent_playing != self.status.playing {
                link.sent_playing = self.status.playing;
                let (c, dev, act) = (c.clone(), dev.clone(), if self.status.playing { "play" } else { "pause" });
                std::thread::spawn(move || {
                    let _ = c.cast_control(&dev, act, None);
                });
            }
        }
        let v = if self.muted_from.is_some() { 0.0 } else { self.volume };
        if (v - link.sent_volume).abs() > 0.02 {
            link.sent_volume = v;
            std::thread::spawn(move || {
                let _ = c.cast_control(&dev, "volume", Some(v as f64));
            });
        }
        Task::batch(tasks)
    }

    fn start_current(&mut self, play: bool) -> Task<Message> {
        self.remote = None;
        self.remote_block = Instant::now() + Duration::from_millis(2500);
        let (Some(c), Some(mut id)) = (self.client.clone(), self.queue.current().map(String::from)) else { return Task::none() };
        // "Only play downloads": skip ahead to a song that is saved on this computer
        if self.setting_bool("offlineOnly", false) && !self.downloads.has(&id) {
            let mut found = false;
            for _ in 0..self.queue.len() {
                match self.queue.next(true).map(String::from) {
                    Some(n) if self.downloads.has(&n) => {
                        id = n;
                        found = true;
                        break;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            if !found {
                self.say("Only downloaded songs play. Turn that off in Settings → Downloads.");
                return Task::none();
            }
        }
        let local = self.local_for(&id);
        if self.offline && local.is_none() {
            self.say("That song isn't downloaded, and the server can't be reached.");
            return Task::none();
        }
        self.preload_for = None;
        self.player.set_gain(self.gain_for(&id));
        let start = std::mem::take(&mut self.pending_start);
        self.player.load_at(&self.stream_client(&c), &id, local, play, start);
        self.status.track = Some(id.clone());
        self.status.position = start;
        self.status.duration = None;
        self.status.playing = play;
        self.status.finished = false;
        self.lyric_line = None;
        let mut tasks = vec![self.fetch_covers(), self.load_lyrics()];
        if play {
            tasks.push(self.announce_playing());
        }
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
        if !self.now_playing || self.offline {
            return Task::none();
        }
        let tid = id.clone();
        match self.np_tab {
            NpTab::Lyrics if !self.lyrics.contains_key(&id) => Task::perform(blocking(move || c.lyrics(&tid)), move |r| Message::LyricsLoaded(id.clone(), r)),
            NpTab::Credits if !self.credits.contains_key(&id) => Task::perform(blocking(move || c.credits(&tid)), move |r| Message::CreditsLoaded(id.clone(), r)),
            _ => Task::none(),
        }
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
                // one song, downloaded from its ⋯ menu
                "track" => vec![s.id.clone()],
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

    /// A person's profile picture, once it has been fetched.
    pub fn avatar_handle(&self, id: &str, version: u64) -> Option<&image::Handle> {
        if version == 0 {
            return None;
        }
        self.avatars.get(&(id.to_string(), version))
    }

    /// The picture someone chose for a playlist, once it has been fetched.
    pub fn playlist_picture(&self, id: &str, version: u64) -> Option<&image::Handle> {
        if version == 0 {
            return None;
        }
        self.pl_covers.get(&(id.to_string(), version))
    }

    /// Fetch the chosen pictures of your playlists that aren't here yet.
    fn fetch_playlist_covers(&mut self) -> Vec<Task<Message>> {
        let Some(c) = self.client.clone().filter(|_| !self.offline) else { return vec![] };
        let owner = self.me.as_ref().map(|m| m.user.id.clone()).unwrap_or_default();
        let mut tasks = vec![];
        let list: Vec<(String, u64)> = self.playlists().iter().map(|p| (p.id.clone(), p.cover)).collect();
        for key in list {
            if key.1 == 0 || self.pl_covers.contains_key(&key) || !self.pl_cover_pending.insert(key.clone()) {
                continue;
            }
            let (c, owner) = (c.clone(), owner.clone());
            tasks.push(Task::perform(blocking(move || c.playlist_cover_bytes(&owner, &key.0, key.1).ok().map(|b| (key, b))), |r| match r { Some(((id, v), b)) => Message::PlaylistCoverLoaded(id, v, Some(b)), None => Message::PlaylistCoverLoaded(String::new(), 0, None) }));
        }
        tasks
    }

    /// Fetch the pictures of you and the household members that aren't here yet.
    fn fetch_avatars(&mut self) -> Vec<Task<Message>> {
        let Some(c) = self.client.clone().filter(|_| !self.offline) else { return vec![] };
        let mut wanted: Vec<(String, u64)> = self.members.iter().map(|m| (m.id.clone(), m.avatar)).collect();
        if let Some(m) = &self.me {
            wanted.push((m.user.id.clone(), m.user.avatar));
        }
        let mut tasks = vec![];
        for (id, v) in wanted {
            if v == 0 || self.avatars.contains_key(&(id.clone(), v)) || !self.avatar_pending.insert((id.clone(), v)) {
                continue;
            }
            let c = c.clone();
            let (who, ver) = (id.clone(), v);
            tasks.push(Task::perform(blocking(move || c.avatar_bytes(&who, ver).ok()), move |b| Message::AvatarLoaded(id.clone(), v, b)));
        }
        tasks
    }

    /// Seconds of crossfade between songs, from your settings (shared with the website; 0 = off).
    pub fn crossfade_secs(&self) -> f32 {
        userdata::setting(&self.data, "crossfade").and_then(|v| v.as_f64()).unwrap_or(0.0).clamp(0.0, 12.0) as f32
    }

    /// The queue played out (or was skipped past its last song): back to "Nothing playing", like the website.
    fn end_queue(&mut self) {
        self.player.stop();
        self.queue.clear();
        self.status.track = None;
        self.status.playing = false;
        self.status.finished = false;
        self.status.position = Duration::ZERO;
        self.status.duration = None;
        let _ = std::fs::remove_file(self.dir.join("resume.json"));
    }

    /// The song after this one took over by itself (it faded in as the last one faded out): move the queue on.
    fn on_crossfaded(&mut self) -> Task<Message> {
        self.queue.next(true);
        self.preload_for = None;
        self.lyric_line = None;
        let Some(id) = self.status.track.clone() else { return Task::none() };
        let mut tasks = vec![self.fetch_covers(), self.load_lyrics()];
        if let Some(album) = self.lib.track(&id).map(|t| t.album_id.clone()) {
            if self.data.recent.first() != Some(&album) && !self.setting_bool("privateSession", false) {
                tasks.push(self.edit(vec![Edit::Recent(album)]));
            }
        }
        Task::batch(tasks)
    }

    /// What another device needs to carry on: the queue, the song, the spot (the website's `snapshot`).
    fn device_state(&self) -> serde_json::Value {
        let items = self.queue.items();
        let index = self.queue.position().unwrap_or(0);
        let from = index.saturating_sub(50);
        let to = (index + 450).min(items.len());
        json!({
            "queue": items[from..to],
            "index": index - from,
            "pos": self.status.position.as_secs_f64(),
            "playing": self.status.playing,
            "context": serde_json::Value::Null,
            "shuffle": self.queue.shuffle(),
            "repeat": match self.queue.repeat() { volvix_core::Repeat::Off => "off", volvix_core::Repeat::All => "all", volvix_core::Repeat::One => "one" },
        })
    }

    /// This computer is the one playing now: the website and your other devices pause.
    fn announce_playing(&self) -> Task<Message> {
        let Some(c) = self.client.clone().filter(|_| !self.offline) else { return Task::none() };
        let me = self.dev_id.clone();
        Task::perform(blocking(move || c.device_active(&me)), |_| Message::Tick)
    }

    /// Show what the device that is playing plays (its queue, song, spot), or stop showing it.
    fn sync_remote(&mut self) {
        if Instant::now() < self.remote_block {
            return;
        }
        if self.party.is_some() || self.cast.is_some() {
            self.remote = None;
            return;
        }
        let active = self.dev_active.clone();
        let now = self.dev_now.clone();
        match (active, now) {
            (Some(a), Some(n)) if a != self.dev_id && n["queue"].as_array().map(|q| !q.is_empty()).unwrap_or(false) => {
                let ids: Vec<String> = n["queue"].as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                let want = ids.get(n["index"].as_u64().unwrap_or(0) as usize).cloned();
                let known: Vec<String> = ids.into_iter().filter(|id| self.lib.track(id).is_some()).collect();
                let at = want.and_then(|w| known.iter().position(|x| *x == w));
                let name = self.connect_devices.iter().find(|d| d.0 == a).map(|d| d.1.clone()).unwrap_or_else(|| "another device".into());
                let server = n["serverNow"].as_f64().unwrap_or(0.0);
                let r = RemoteNow {
                    device: a,
                    name: name.clone(),
                    playing: n["playing"].as_bool().unwrap_or(false),
                    pos_ms: n["posMs"].as_f64().unwrap_or(0.0),
                    updated_at: n["updatedAt"].as_f64().unwrap_or(0.0),
                    offset_ms: if server > 0.0 { server - now_ms() } else { 0.0 },
                };
                if self.remote.is_none() {
                    self.player.pause(); // no sound here: the other device has it
                }
                if let Some(i) = at {
                    if self.queue.items() != known.as_slice() || self.queue.position() != Some(i) {
                        self.queue.mirror(known, i, &format!("Playing on {name}"));
                    }
                }
                self.remote = Some(r);
            }
            _ => {
                if self.remote.take().is_some() {
                    self.status.playing = false;
                }
            }
        }
    }

    /// The song, whether it plays and where it is, as the other device says (its clock, kept up to date here).
    fn mirror_status(&mut self, r: &RemoteNow) {
        let mut pos = (if r.playing { r.pos_ms + (now_ms() + r.offset_ms - r.updated_at) } else { r.pos_ms }).max(0.0);
        let track = self.queue.current().map(String::from);
        let dur = track.as_deref().and_then(|id| self.lib.track(id)).map(|t| t.duration as f64).unwrap_or(0.0);
        if dur > 0.0 {
            pos = pos.min(dur * 1000.0);
        }
        self.status.track = track;
        self.status.playing = r.playing;
        self.status.loading = false;
        self.status.finished = false;
        self.status.position = Duration::from_millis(pos as u64);
        self.status.duration = (dur > 0.0).then(|| Duration::from_secs_f64(dur));
    }

    /// Open the live line to the server that lists your devices, and act on what it says.
    fn devices_tick(&mut self) -> Vec<Task<Message>> {
        use std::sync::atomic::Ordering;
        let mut tasks = vec![];
        // the connection lives while signed in; a passing "offline" blip must not drop it (it reconnects by itself)
        let signed_in = self.client.is_some() && self.screen == Screen::Main;
        if signed_in && !self.dev_started {
            self.dev_started = true;
            let my = self.dev_gen.fetch_add(1, Ordering::SeqCst) + 1;
            spawn_device_events(self.client.clone().unwrap(), self.dev_id.clone(), self.dev_events.clone(), self.dev_gen.clone(), my);
        } else if !signed_in && self.dev_started {
            self.dev_started = false;
            self.dev_gen.fetch_add(1, Ordering::SeqCst);
            self.connect_devices.clear();
            self.dev_active = None;
        }
        let events: Vec<serde_json::Value> = std::mem::take(&mut *self.dev_events.lock().unwrap());
        let had_events = !events.is_empty();
        for e in events {
            match e["type"].as_str() {
                Some("devices") => {
                    let before = self.dev_active.clone();
                    self.connect_devices = e["devices"].as_array().map(|l| l.iter().map(|d| (d["id"].as_str().unwrap_or("").to_string(), d["name"].as_str().unwrap_or("").to_string(), d["active"].as_bool().unwrap_or(false))).collect()).unwrap_or_default();
                    self.dev_active = e["active"].as_str().map(String::from);
                    self.dev_now = e["now"].as_object().map(|_| e["now"].clone());
                    // another device took over: this one goes quiet
                    if let Some(a) = self.dev_active.clone().filter(|a| *a != self.dev_id && Some(a) != before.as_ref()) {
                        if self.status.playing {
                            self.player.pause();
                            self.status.playing = false;
                            let name = self.connect_devices.iter().find(|d| d.0 == a).map(|d| d.1.clone()).unwrap_or_else(|| "another device".into());
                            self.say(format!("Playing on {name}"));
                        }
                    }
                }
                Some("transfer") if e["to"].as_str() == Some(self.dev_id.as_str()) && self.cast.is_none() && self.party.is_none() => {
                    let s = &e["state"];
                    let ids: Vec<String> = s["queue"].as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                    let want = s["index"].as_u64().unwrap_or(0) as usize;
                    let cur = ids.get(want).cloned();
                    let ids: Vec<String> = ids.into_iter().filter(|id| self.lib.track(id).is_some()).collect();
                    if let Some(i) = cur.and_then(|c| ids.iter().position(|x| *x == c)) {
                        self.queue.play(ids, i, "Moved from another device");
                        self.queue.set_repeat(match s["repeat"].as_str() { Some("all") => volvix_core::Repeat::All, Some("one") => volvix_core::Repeat::One, _ => volvix_core::Repeat::Off });
                        self.pending_start = Duration::from_secs_f64(s["pos"].as_f64().unwrap_or(0.0).max(0.0));
                        let play = s["playing"].as_bool().unwrap_or(true);
                        tasks.push(self.start_current(play));
                        self.say(if play { "Playing here" } else { "Press play to continue here" });
                    }
                }
                // the device that is playing says what it plays
                Some("now") if e["device"].as_str() == self.dev_active.as_deref() && self.dev_active.as_deref() != Some(self.dev_id.as_str()) => {
                    self.dev_now = Some(e["state"].clone());
                }
                // another of your devices (the website, a phone) asked this one to play, pause, skip, seek…
                Some("command") if e["to"].as_str() == Some(self.dev_id.as_str()) && self.party.is_none() => {
                    let cmd = e["cmd"].clone();
                    match cmd["op"].as_str() {
                        Some("play") if !self.status.playing => tasks.push(self.update(Message::TogglePlay)),
                        Some("pause") if self.status.playing => tasks.push(self.update(Message::TogglePlay)),
                        Some("toggle") => tasks.push(self.update(Message::TogglePlay)),
                        Some("next") => tasks.push(self.update(Message::Next)),
                        Some("prev") => tasks.push(self.update(Message::Previous)),
                        Some("seek") => tasks.push(self.update(Message::SeekTo(cmd["pos"].as_f64().unwrap_or(0.0).max(0.0) as f32))),
                        Some("setQueue") => {
                            let ids: Vec<String> = cmd["queue"].as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).filter(|id| self.lib.track(id).is_some()).collect()).unwrap_or_default();
                            let i = cmd["index"].as_u64().unwrap_or(0) as usize;
                            if !ids.is_empty() {
                                let i = i.min(ids.len() - 1);
                                tasks.push(self.update(Message::PlayTracks(ids, i, "Remote control".to_string())));
                            }
                        }
                        Some("add") | Some("addNext") => {
                            let ids: Vec<String> = cmd["ids"].as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).filter(|id| self.lib.track(id).is_some()).collect()).unwrap_or_default();
                            if !ids.is_empty() {
                                let m = if cmd["op"].as_str() == Some("add") { Message::QueueMany(ids) } else { Message::PlayNextMany(ids) };
                                tasks.push(self.update(m));
                            }
                        }
                        Some("playAt") => {
                            let items: Vec<String> = self.queue.items().to_vec();
                            let i = cmd["index"].as_u64().unwrap_or(0) as usize;
                            if i < items.len() {
                                tasks.push(self.update(Message::PlayTracks(items, i, "Remote control".to_string())));
                            }
                        }
                        _ => {}
                    }
                    self.dev_now_key.clear(); // tell the others what changed, soon
                }
                _ => {}
            }
        }
        if had_events {
            self.sync_remote();
        }
        // this computer is the one playing: tell the others what it plays (when it changes, and every few seconds)
        if signed_in && !self.offline && self.cast.is_none() && self.party.is_none() && self.remote.is_none() && (self.dev_active.as_deref() == Some(self.dev_id.as_str()) || self.status.playing) {
            if let (Some(track), Some(c)) = (self.status.track.clone(), self.client.clone()) {
                let items = self.queue.items();
                let index = self.queue.position().unwrap_or(0);
                let key = format!("{track}|{index}|{}|{}", self.status.playing, items.len());
                if key != self.dev_now_key || self.dev_now_at.elapsed() >= Duration::from_secs(5) {
                    self.dev_now_key = key;
                    self.dev_now_at = Instant::now();
                    let from = index.saturating_sub(50);
                    let to = (index + 450).min(items.len());
                    let state = json!({
                        "queue": items[from..to],
                        "index": index - from,
                        "posMs": self.status.position.as_millis() as u64,
                        "playing": self.status.playing,
                    });
                    let me = self.dev_id.clone();
                    std::thread::spawn(move || {
                        let _ = c.device_report(&me, state);
                    });
                }
            }
        }
        tasks
    }

    /// A copy of the song on this computer: a download, or one cached as it was queued.
    fn local_for(&self, id: &str) -> Option<PathBuf> {
        if let Some(p) = self.downloads.path(id).filter(|p| p.exists()) {
            return Some(p);
        }
        let p = self.song_cache.path(id).filter(|p| p.exists())?;
        self.song_cache.touch(id);
        Some(p)
    }

    /// What is playing and what plays soon (kept in the cache, and fetched into it).
    fn cache_keep(&self) -> Vec<String> {
        let mut v: Vec<String> = self.queue.current().map(String::from).into_iter().collect();
        v.extend(self.queue.upcoming().iter().take(4).cloned());
        v
    }

    /// Fetch the song playing and the next few into the cache, a couple at a time, in the background.
    fn prefetch(&mut self) -> Task<Message> {
        if !self.settings.cache_songs || self.offline || self.cast.is_some() || self.party.is_some() || self.screen != Screen::Main {
            return Task::none();
        }
        if !self.settings.download_metered && system::on_metered_connection() {
            return Task::none();
        }
        let Some(c) = self.client.clone() else { return Task::none() };
        let sc = self.stream_client(&c);
        let mut tasks = vec![];
        for id in self.cache_keep().into_iter().take(4) {
            if self.caching.len() >= 2 {
                break;
            }
            if self.caching.contains(&id) || self.local_for(&id).is_some() {
                continue;
            }
            self.caching.insert(id.clone());
            let (cache, client, tid) = (self.song_cache.clone(), sc.clone(), id.clone());
            tasks.push(Task::perform(blocking(move || cache.fetch(&client, &tid)), move |r| Message::CacheFetched(id.clone(), r)));
        }
        Task::batch(tasks)
    }

    /// Crossfade: a few seconds before the end, open the next song so it can fade in.
    fn crossfade_prepare(&mut self) {
        let fade = self.crossfade_secs();
        self.player.set_crossfade(fade);
        if fade <= 0.0 || !self.status.playing || self.cast.is_some() || self.party.is_some() || self.queue.repeat() == volvix_core::Repeat::One {
            return;
        }
        let (Some(cur), Some(dur)) = (self.status.track.clone(), self.status.duration) else { return };
        let left = dur.as_secs_f32() - self.status.position.as_secs_f32();
        if left > fade + 6.0 || self.preload_for.as_deref() == Some(cur.as_str()) {
            return;
        }
        self.preload_for = Some(cur.clone());
        let Some(next) = self.queue.upcoming().first().cloned() else { return };
        let same_album = match (self.lib.track(&cur), self.lib.track(&next)) {
            (Some(a), Some(b)) => a.album_id == b.album_id,
            _ => false,
        };
        if same_album && !self.setting_bool("crossfadeAlbums", false) {
            return; // albums flow straight through unless asked
        }
        let Some(c) = self.client.clone() else { return };
        let local = self.local_for(&next);
        let gain = self.gain_for(&next);
        self.player.preload(&self.stream_client(&c), &next, local, gain);
    }

    /// The equalizer as saved in your settings (shared with the website).
    pub fn eq_now(&self) -> eq::EqSettings {
        eq::EqSettings::from_json(userdata::setting(&self.data, "eq").as_ref())
    }

    fn save_eq(&mut self, e: eq::EqSettings) -> Task<Message> {
        self.player.set_eq(e.clone());
        self.edit(vec![Edit::Setting("eq".into(), e.to_json())])
    }

    /// The client with the streaming quality chosen in Settings (the data saver drops to "low" on a metered connection).
    fn stream_client(&self, c: &Client) -> Client {
        if !self.me.as_ref().map(|m| m.transcoding).unwrap_or(false) {
            return c.clone();
        }
        if self.settings.data_saver && system::on_metered_connection() {
            return c.with_quality("low");
        }
        c.with_quality(&self.settings.stream_quality)
    }

    /// The client for downloads: their own quality, or the streaming one.
    fn download_client(&self, c: &Client) -> Client {
        if !self.me.as_ref().map(|m| m.transcoding).unwrap_or(false) || self.settings.download_quality == "stream" {
            return self.stream_client(c);
        }
        c.with_quality(&self.settings.download_quality)
    }

    fn pump_downloads(&mut self) -> Task<Message> {
        if self.dl_active.is_some() || self.offline {
            return Task::none();
        }
        // on a metered connection the queue waits (Settings → Downloads); it carries on by itself afterwards
        if !self.settings.download_metered && !self.dl_queue.is_empty() && system::on_metered_connection() {
            return Task::none();
        }
        let Some(c) = self.client.clone().map(|c| self.download_client(&c)) else { return Task::none() };
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

    /// Search's "Get more music": ask the server for each kind the filter shows, separately, so
    /// each appears as soon as it's ready. Older answers are ignored.
    fn look_further(&mut self, q: String) -> Task<Message> {
        let q = q.trim().to_string();
        if q != self.search.trim() || q == self.find.searched {
            return Task::none(); // still typing, or already looked
        }
        self.find.seq += 1;
        self.find.parts.clear();
        self.find.open.clear();
        self.find.searched = q.clone();
        let lidarr = self.me.as_ref().map(|m| m.lidarr.configured).unwrap_or(false);
        let Some(c) = self.client.clone().filter(|_| lidarr && !self.offline && q.chars().count() >= 2) else { return Task::none() };
        let seq = self.find.seq;
        Task::batch(self.search_type.kinds().iter().map(|kind| {
            let (c, q, k) = (c.clone(), q.clone(), kind.to_string());
            let k2 = k.clone();
            Task::perform(blocking(move || c.find_part(&q, &k)), move |r| Message::FoundPart(seq, k2.clone(), r))
        }))
    }

    /// The volume-leveling gain for a song: album gain while an album plays in order (keeps its
    /// loud and quiet songs as made), song gain otherwise. 1.0 with leveling off.
    fn gain_for(&self, track: &str) -> f32 {
        let mode = self.setting_str("leveling", "off");
        if mode == "off" {
            return 1.0;
        }
        let Some(t) = self.lib.track(track) else { return 1.0 };
        // "track" = by song, "album" = by album, as in the website
        audio::replay_gain(t.replay_gain.as_ref(), &mode)
    }

    pub fn dl_queue_len(&self) -> usize {
        self.dl_queue.len()
    }

    pub fn is_saved(&self, kind: &str, id: &str) -> bool {
        self.saved.iter().any(|s| s.kind == kind && s.id == id)
    }

    /* ---------- the main switchboard ---------- */

    fn handle(&mut self, msg: Message) -> Task<Message> {
        // while another device is playing, the controls drive that device
        if let Some(r) = self.remote.clone() {
            let cmd = match &msg {
                Message::TogglePlay => Some(json!({ "op": if r.playing { "pause" } else { "play" } })),
                Message::Next => Some(json!({ "op": "next" })),
                Message::Previous => Some(json!({ "op": "prev" })),
                Message::SeekTo(v) => Some(json!({ "op": "seek", "pos": *v as f64 })),
                Message::JumpQueue(i) => Some(json!({ "op": "playAt", "index": *i })),
                // the device that is playing keeps the music: what you pick plays there (a paused one is taken over below)
                Message::PlayTracks(ids, i, _) if r.playing => Some(json!({ "op": "setQueue", "queue": ids.iter().take(500).collect::<Vec<_>>(), "index": (*i).min(499) })),
                Message::PlayAlbum(album, i) if r.playing => {
                    let ids: Vec<String> = self.lib.album_tracks(album).iter().map(|t| t.id.clone()).collect();
                    Some(json!({ "op": "setQueue", "queue": ids.iter().take(500).collect::<Vec<_>>(), "index": (*i).min(499) }))
                }
                Message::QueueMany(ids) if r.playing => Some(json!({ "op": "add", "ids": ids })),
                Message::AddToQueue(id) if r.playing => Some(json!({ "op": "add", "ids": [id] })),
                Message::PlayNextMany(ids) if r.playing => Some(json!({ "op": "addNext", "ids": ids })),
                Message::PlayNext(id) if r.playing => Some(json!({ "op": "addNext", "ids": [id] })),
                _ => None,
            };
            if let Some(cmd) = cmd {
                if let Some(c) = self.client.clone() {
                    let (me, to) = (self.dev_id.clone(), r.device.clone());
                    std::thread::spawn(move || {
                        let _ = c.device_command(&me, &to, cmd);
                    });
                }
                return Task::none();
            }
            if matches!(msg, Message::RemoveFromQueue(_) | Message::ClearQueue | Message::ToggleShuffle) {
                self.say("Not available while you control another device");
                return Task::none();
            }
        }
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
            Message::ServerAnswered(up) => {
                if up && self.offline {
                    if let Some(c) = self.client.clone() {
                        return load_everything(c, self.downloads.clone());
                    }
                }
            }
            Message::Loaded(r) => {
                self.busy = false;
                let was_offline = self.offline;
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
                            if !was_offline {
                                self.say("The server can't be reached. Your downloads still play.");
                                self.lib_tab = LibTab::Downloaded;
                            }
                        } else if was_offline {
                            self.say("Back online");
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
                    self.forward.push(std::mem::replace(&mut self.page, p));
                }
                return self.fetch_covers();
            }
            Message::Forward => {
                if let Some(p) = self.forward.pop() {
                    self.history.push(std::mem::replace(&mut self.page, p));
                    self.now_playing = false;
                }
                return self.fetch_covers();
            }
            Message::DragStart(list, from, count, divider, row_h, click) => {
                let start = POINTER.lock().map(|p| *p).unwrap_or((0.0, 0.0));
                self.drag = Some(Drag { list, from, over: from, click, start, active: false, count, row_h, divider, y: 0.0, grab: None, gaps: vec![], last_frame: Instant::now() });
            }
            Message::DragAt(y) => {
                if let Some(d) = &mut self.drag {
                    // how far into the row the pointer took hold (from the first report after the press)
                    if d.grab.is_none() {
                        d.grab = Some((y - d.row_top(d.from)).clamp(0.0, d.row_h));
                    }
                    d.y = y;
                    if !d.active {
                        let p = POINTER.lock().map(|p| *p).unwrap_or(d.start);
                        if (p.0 - d.start.0).hypot(p.1 - d.start.1) < 5.0 {
                            return Task::none();
                        }
                        // moved far enough: it's picked up, and the room it leaves is where it was
                        d.active = true;
                        d.gaps = vec![0.0; d.count.max(1)];
                        if let Some(g) = d.gaps.get_mut(d.from) {
                            *g = 1.0;
                        }
                        d.last_frame = Instant::now();
                        self.hover_row = None;
                    }
                    d.place();
                }
            }
            Message::DragFrame => {
                let Some(d) = &mut self.drag else { return Task::none() };
                if !d.active {
                    return Task::none();
                }
                let dt = d.last_frame.elapsed().as_secs_f32().min(0.1);
                d.last_frame = Instant::now();
                // the other rows slide to where they belong (the website takes about 0.16 s)
                let k = 1.0 - (-dt / 0.05).exp();
                let over = d.over;
                for (i, g) in d.gaps.iter_mut().enumerate() {
                    *g += (if i == over { 1.0 } else { 0.0 } - *g) * k;
                }
                // near the top or bottom edge of the list's view: scroll, as the website does (12 px every 30 ms)
                let (_, py) = POINTER.lock().map(|p| *p).unwrap_or((0.0, 0.0));
                let (_, wh) = WINDOW_SIZE.lock().map(|w| *w).unwrap_or((1200.0, 800.0));
                let (top, which) = match d.list {
                    DragList::Queue => (150.0, queue_scroller()),
                    DragList::Playlist(_) => (96.0, main_scroller()),
                };
                let step = 400.0 * dt;
                let dy = if py < top + 60.0 {
                    -step
                } else if py > wh - 160.0 {
                    step
                } else {
                    0.0
                };
                if dy != 0.0 {
                    return scrollable::scroll_by(which, scrollable::AbsoluteOffset { x: 0.0, y: dy });
                }
            }
            Message::Scrolled(which, y) => {
                // (the list reports the pointer's new place in it by itself after it scrolls)
                self.scroll_y[(which as usize).min(1)] = y;
            }
            Message::PopMenuOpen(kind) => {
                let at = POINTER.lock().map(|p| *p).unwrap_or((0.0, 0.0));
                self.pop_menu = Some(PopMenu { kind, at });
            }
            Message::PopMenuClose => self.pop_menu = None,
            Message::PopMenuDo(m) => {
                self.pop_menu = None;
                return self.update(*m);
            }
            Message::RowHover(h) => {
                // not while a menu or dialog is open over the rows, and not while a row is being carried around
                if self.pop_menu.is_some() || self.dialog.is_some() {
                    self.hover_row = None;
                } else if !self.drag.as_ref().map(|d| d.active).unwrap_or(false) {
                    self.hover_row = h;
                }
            }
            Message::RowLeave(list, i) => {
                if self.hover_row == Some((list, i)) {
                    self.hover_row = None;
                }
            }
            Message::QueueMany(ids) => {
                let n = ids.len();
                for id in ids {
                    self.queue.add(&id);
                }
                self.say(format!("Added {n} song{} to the queue", if n == 1 { "" } else { "s" }));
            }
            Message::PlayNextMany(ids) => {
                for id in ids.iter().rev() {
                    self.queue.play_next(id);
                }
                self.say("Playing next");
            }
            Message::PickPlaylistPicture(id) => {
                return Task::perform(
                    blocking(|| rfd::FileDialog::new().set_title("Choose a picture").add_filter("Pictures", &["jpg", "jpeg", "png", "webp"]).pick_file()),
                    move |p| Message::PlaylistPictureChosen(id.clone(), p),
                );
            }
            Message::PlaylistPictureChosen(id, Some(path)) => {
                return Task::perform(blocking(move || pictures::load(&path).map(std::sync::Arc::new)), move |r| Message::CropLoaded(CropTarget::Playlist(id.clone()), r));
            }
            Message::PlaylistPictureChosen(_, None) => {}
            Message::PlaylistPictureSaved(id, r) => match r {
                Ok(v) => {
                    self.say("Picture updated");
                    return self.edit(vec![Edit::PlaylistCover { id, version: v }]);
                }
                Err(e) => self.say(e),
            },
            Message::RemovePlaylistPicture(id) => {
                let Some(c) = self.client.clone() else { return Task::none() };
                return Task::perform(
                    blocking(move || {
                        let r = c.remove_playlist_cover(&id);
                        (id, r)
                    }),
                    |(id, r)| Message::PlaylistPictureRemoved(id, r),
                );
            }
            Message::PlaylistPictureRemoved(id, r) => match r {
                Ok(()) => {
                    self.say("Picture removed");
                    return self.edit(vec![Edit::PlaylistCover { id, version: 0 }]);
                }
                Err(e) => self.say(e.to_string()),
            },
            Message::PlaylistCoverLoaded(id, v, bytes) => {
                self.pl_cover_pending.remove(&(id.clone(), v));
                if let Some(b) = bytes {
                    self.pl_covers.insert((id, v), pictures::rounded_handle(&b, 512, 0.045).unwrap_or_else(|| image::Handle::from_bytes(b)));
                }
            }
            Message::CoverOpen(id) => {
                let look = self.lib.look(&id).cloned().unwrap_or(volvix_core::Look { bg: "sunset".into(), font: "display".into(), ..Default::default() });
                let name = self.lib.album(&id).map(|a| a.title.clone()).unwrap_or_default();
                self.dialog = Some(Dialog::CoverLook { target: LookTarget::Album(id), name, look });
            }
            Message::CoverOpenPlaylist(id) => {
                let Some(p) = self.playlist(&id) else { return Task::none() };
                let look = p.look.clone().unwrap_or(volvix_core::Look { bg: "sunset".into(), font: "display".into(), ..Default::default() });
                self.dialog = Some(Dialog::CoverLook { target: LookTarget::Playlist(id), name: p.name, look });
            }
            Message::CoverBg(v) => {
                if let Some(Dialog::CoverLook { look, .. }) = &mut self.dialog {
                    look.bg = v;
                }
            }
            Message::CoverFont(v) => {
                if let Some(Dialog::CoverLook { look, .. }) = &mut self.dialog {
                    look.font = v;
                }
            }
            Message::CoverColour(which, v) => {
                if let Some(Dialog::CoverLook { look, .. }) = &mut self.dialog {
                    let v = if v.trim().is_empty() { None } else { Some(v) };
                    match which {
                        0 => look.c1 = v,
                        1 => look.c2 = v,
                        _ => look.ink = v,
                    }
                }
            }
            Message::CoverResetColours => {
                if let Some(Dialog::CoverLook { look, .. }) = &mut self.dialog {
                    look.c1 = None;
                    look.c2 = None;
                    look.ink = None;
                }
            }
            Message::CoverReset => {
                let Some(Dialog::CoverLook { target, .. }) = self.dialog.take() else { return Task::none() };
                match target {
                    LookTarget::Playlist(id) => {
                        self.say("Back to the automatic cover");
                        return self.edit(vec![Edit::PlaylistLook { id, look: None }]);
                    }
                    LookTarget::Album(album) => {
                        let Some(c) = self.client.clone() else { return Task::none() };
                        let id = album.clone();
                        return Task::perform(blocking(move || c.clear_album_look(&id)), move |r| Message::CoverSaved(album.clone(), None, r));
                    }
                }
            }
            Message::CoverSaved(album, look, r) => match r {
                Ok(()) => {
                    std::sync::Arc::make_mut(&mut self.lib).set_look(&album, look);
                    self.say("Cover saved for everyone");
                }
                Err(e) => self.say(e.to_string()),
            },
            Message::PickAlbumPicture(id) => {
                return Task::perform(
                    blocking(|| rfd::FileDialog::new().set_title("Choose a picture").add_filter("Pictures", &["jpg", "jpeg", "png", "webp"]).pick_file()),
                    move |p| Message::AlbumPictureChosen(id.clone(), p),
                );
            }
            Message::AlbumPictureChosen(id, Some(path)) => {
                return Task::perform(blocking(move || pictures::load(&path).map(std::sync::Arc::new)), move |r| Message::CropLoaded(CropTarget::Album(id.clone()), r));
            }
            Message::AlbumPictureChosen(_, None) => {}
            Message::AlbumPictureSaved(id, r) => match r {
                Ok(v) => {
                    std::sync::Arc::make_mut(&mut self.lib).set_album_picture_state(&id, true, v);
                    self.cover_images.retain(|k, _| k.0 != id);
                    self.say("Picture updated for everyone");
                    return self.fetch_covers();
                }
                Err(e) => self.say(e),
            },
            Message::RemoveAlbumPicture(id) => {
                let Some(c) = self.client.clone() else { return Task::none() };
                return Task::perform(blocking(move || (id.clone(), c.clear_album_picture(&id))), |(id, r)| Message::AlbumPictureRemoved(id, r));
            }
            Message::AlbumPictureRemoved(id, r) => match r {
                Ok(()) => {
                    std::sync::Arc::make_mut(&mut self.lib).set_album_picture_state(&id, false, 0);
                    self.cover_images.retain(|k, _| k.0 != id);
                    self.say("Picture removed");
                }
                Err(e) => self.say(e.to_string()),
            },
            Message::CropLoaded(target, r) => match r {
                Ok(img) => self.dialog = Some(Dialog::Crop(Box::new(CropState::new(target, img)))),
                Err(e) => self.say(e),
            },
            Message::CropZoom(z) => {
                if let Some(Dialog::Crop(c)) = &mut self.dialog {
                    c.zoom = z.clamp(1.0, 4.0);
                    c.refresh();
                }
            }
            Message::CropDown => self.crop_drag = true,
            Message::CropUp => {
                self.crop_drag = false;
                self.crop_last = None;
            }
            Message::CropMove(p) => {
                if self.crop_drag {
                    if let (Some(last), Some(Dialog::Crop(c))) = (self.crop_last, &mut self.dialog) {
                        let k = c.side() / CROP_VIEW;
                        c.cx -= (p.x - last.x) * k;
                        c.cy -= (p.y - last.y) * k;
                        c.refresh();
                    }
                }
                self.crop_last = Some(p);
            }
            Message::ShowAlbumInfo(id) => self.dialog = Some(Dialog::AlbumInfo { album: id }),
            Message::ShowCredits(id) => {
                self.dialog = Some(Dialog::Credits { track: id.clone() });
                if !self.credits.contains_key(&id) {
                    if let Some(c) = self.client.clone().filter(|_| !self.offline) {
                        let tid = id.clone();
                        return Task::perform(blocking(move || c.credits(&tid)), move |r| Message::CreditsLoaded(id.clone(), r));
                    }
                }
            }
            Message::ShareOpen(kind, name, tracks) => {
                if !self.can("share") {
                    self.say("You don't have permission to make share links. Ask an admin.");
                } else if tracks.is_empty() {
                    self.say("There's nothing in it to share yet");
                } else {
                    self.dialog = Some(Dialog::Share { kind, name, tracks, days: 7, link: None, error: None, busy: false });
                }
            }
            Message::ShareDays(d) => {
                if let Some(Dialog::Share { days, .. }) = &mut self.dialog {
                    *days = d;
                }
            }
            Message::ShareCreate => {
                let (Some(c), Some(Dialog::Share { kind, name, tracks, days, busy, .. })) = (self.client.clone(), &mut self.dialog) else { return Task::none() };
                *busy = true;
                let (kind, name, tracks, days) = (kind.clone(), name.clone(), tracks.clone(), *days);
                return Task::perform(blocking(move || c.create_share(&kind, &name, &tracks, days)), Message::ShareCreated);
            }
            Message::ShareCreated(r) => {
                let base = self.client.as_ref().map(|c| c.base().to_string()).unwrap_or_default();
                if let Some(Dialog::Share { link, error, busy, .. }) = &mut self.dialog {
                    *busy = false;
                    match r {
                        Ok(s) => *link = Some((format!("{base}{}", s.path), s.time_left(userdata::now()))),
                        Err(e) => *error = Some(e.to_string()),
                    }
                }
            }
            Message::CollabOpen(id) => {
                let Some(p) = self.playlist(&id) else { return Task::none() };
                if p.is_shared() {
                    let mut picked = p.collaborators.clone();
                    picked.extend(p.invited.iter().cloned());
                    self.dialog = Some(Dialog::Collab { id, picked });
                    if let Some(c) = self.client.clone() {
                        return Task::perform(blocking(move || c.household()), Message::HouseholdLoaded);
                    }
                } else {
                    // still just yours: it becomes collaborative when you save this with someone ticked
                    self.dialog = Some(Dialog::Collab { id, picked: vec![] });
                    if let Some(c) = self.client.clone() {
                        return Task::perform(blocking(move || c.household()), Message::HouseholdLoaded);
                    }
                }
            }
            Message::Unshare(id) => {
                self.dialog = None;
                let Some(c) = self.client.clone() else { return Task::none() };
                return Task::perform(blocking(move || c.shared_op(&id, json!({ "op": "unshare" }))), Message::Unshared);
            }
            Message::Unshared(r) => match r {
                Ok(res) => {
                    // it comes back as one of your own playlists, under the same address
                    let p = res.playlist;
                    self.shared.retain(|x| x.id != p.id);
                    let ids: Vec<String> = p.tracks.iter().map(|t| t.id.clone()).collect();
                    let pl = Playlist { id: p.id.clone(), name: p.name.clone(), description: p.description.clone(), public: p.public, owner: userdata::ME.into(), ..Default::default() };
                    self.say("Playlist is no longer collaborative");
                    return self.edit(vec![Edit::CreatePlaylist(pl), Edit::AddTracks { id: p.id, tracks: ids, at: userdata::now() }]);
                }
                Err(e) => self.say(e.to_string()),
            },
            Message::LibSort(s) => self.lib_sort = s,
            Message::LibScope(all) => self.lib_all_artists = all,
            Message::FollowArtist(name) => {
                let on = !self.follows(&name);
                self.say(if on { format!("Following {name}. They're in Library → Artists.") } else { format!("Unfollowed {name}") });
                return self.edit(vec![Edit::FollowArtist { name, on }]);
            }
            Message::PlayNext(id) => {
                self.queue.play_next(&id);
                self.say("Playing next");
            }
            Message::AddSongQuery(s) => self.add_q = s,
            Message::AddSongTo(pl, id) => return self.add_to_playlist(&pl, vec![id]),
            Message::DragEnd => {
                if let Some(d) = self.drag.take() {
                    let p = POINTER.lock().map(|p| *p).unwrap_or(d.start);
                    let moved = (p.0 - d.start.0).hypot(p.1 - d.start.1) >= 5.0;
                    if !d.active && !moved {
                        // let go where it was pressed: a click
                        if let Some(m) = d.click {
                            return self.update(*m);
                        }
                    } else if d.active && d.over != d.from {
                        // dropped somewhere else: it moves there (and nothing starts playing)
                        return match d.list {
                            DragList::Queue => {
                                // the queue's rows are counted from the song after the current one
                                let base = self.queue.position().map(|p| p + 1).unwrap_or(0);
                                self.queue.move_item(base + d.from, base + d.over);
                                Task::none()
                            }
                            DragList::Playlist(pl) => self.update(Message::MoveInPlaylist(pl, d.from, d.over)),
                        };
                    }
                }
            }
            Message::SearchChanged(s) => {
                self.search = s.clone();
                // look further (Lidarr) once typing stops for a moment, as the website does
                let settle = Task::perform(
                    async move {
                        tokio::time::sleep(Duration::from_millis(350)).await;
                        s
                    },
                    Message::SearchSettled,
                );
                return Task::batch([self.fetch_covers(), settle]);
            }
            Message::LibTab(t) => {
                self.lib_tab = t;
                return self.fetch_covers();
            }
            Message::CoverLoaded(id, size, bytes) => {
                self.cover_pending.remove(&(id.clone(), size));
                if let Some((w, h, px)) = bytes {
                    self.cover_order.push_back((id.clone(), size));
                    self.cover_images.insert((id, size), image::Handle::from_rgba(w, h, px));
                    // a very big library: let the oldest go (they are fetched again from disk when needed)
                    while self.cover_order.len() > 700 {
                        if let Some(k) = self.cover_order.pop_front() {
                            self.cover_images.remove(&k);
                        }
                    }
                }
            }
            Message::ImageLoaded(path, bytes) => {
                if let Some(b) = bytes.filter(|b| !b.is_empty()) {
                    // pictures of found music are rounded squares too
                    let h = pictures::rounded_handle(&b, 256, 0.12).unwrap_or_else(|| image::Handle::from_bytes(b));
                    self.images.insert(path, h);
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
                    return self.announce_playing();
                }
            }
            /* ---------- the cast button: TVs, speakers and listening parties ---------- */
            Message::PlayOn => {
                self.dialog = Some(Dialog::PlayOn);
                if let Some(c) = self.client.clone().filter(|_| !self.offline) {
                    let c2 = c.clone();
                    return Task::batch([Task::perform(blocking(move || c.cast_devices()), Message::CastDevices), Task::perform(blocking(move || c2.party_current()), Message::PartyLoaded)]);
                }
            }
            Message::HintOn(l) => ui::hint_on(l),
            Message::HintOff => ui::hint_off(),
            Message::CastDevices(r) => {
                if let Ok(list) = r {
                    self.cast_devices = list;
                }
            }
            Message::CastStart(i) => {
                let Some(dev) = self.cast_devices.get(i).cloned() else { return Task::none() };
                if self.party.is_some() {
                    self.say("Casting isn't available in a party");
                    return Task::none();
                }
                if self.status.track.is_none() {
                    self.say("Start a song first, then cast it");
                    return Task::none();
                }
                self.cast = Some(CastLink { device: dev.id, name: dev.name.clone(), loaded: None, sent_playing: self.status.playing, sent_volume: -1.0 });
                self.cast_poll = Instant::now() + Duration::from_secs(4);
                self.dialog = None;
                self.apply_volume();
                self.say(format!("Connecting to {}…", dev.name));
                return self.cast_sync();
            }
            Message::CastStop => {
                self.dialog = None;
                if let (Some(link), Some(c)) = (self.cast.clone(), self.client.clone()) {
                    std::thread::spawn(move || {
                        let _ = c.cast_control(&link.device, "stop", None);
                    });
                    self.end_cast("Stopped casting");
                }
            }
            Message::CastAddOpen => self.dialog = Some(Dialog::AddCast { host: String::new(), error: None, busy: false }),
            Message::CastAddDone(r) => match r {
                Ok(d) => {
                    self.say(format!("{} added", if d.name.is_empty() { "That device" } else { d.name.as_str() }));
                    self.dialog = Some(Dialog::PlayOn);
                    if let Some(c) = self.client.clone() {
                        return Task::perform(blocking(move || c.cast_devices()), Message::CastDevices);
                    }
                }
                Err(e) => {
                    if let Some(Dialog::AddCast { error, busy, .. }) = &mut self.dialog {
                        *error = Some(e.to_string());
                        *busy = false;
                    }
                }
            },
            Message::CastPlayed(r) => {
                if let Err(e) = r {
                    let name = self.cast.as_ref().map(|c| c.name.clone()).unwrap_or_default();
                    self.end_cast(&format!("Couldn't cast to {name}: {e}"));
                } else if let Some(l) = &self.cast {
                    self.say(format!("Casting to {}", l.name));
                }
            }
            Message::CastStatus(r) => {
                if let (Ok(s), Some(link)) = (r, self.cast.clone()) {
                    if !s.connected {
                        self.end_cast(&format!("Lost the connection to {}", link.name));
                    } else if s.player_state == "IDLE" && s.idle_reason == "CANCELLED" {
                        self.end_cast(&format!("{} is playing something else now", link.name));
                    }
                }
            }
            Message::PartyLoaded(r) => {
                if let Ok(p) = r {
                    if p.is_some() && self.cast.is_some() {
                        // a party and a TV can't both have the sound
                        if let (Some(link), Some(c)) = (self.cast.clone(), self.client.clone()) {
                            std::thread::spawn(move || {
                                let _ = c.cast_control(&link.device, "stop", None);
                            });
                        }
                        self.end_cast("Stopped casting for the party");
                    }
                    self.party = p;
                }
            }
            Message::PartyStartOpen => {
                if !self.may_party() {
                    self.say("You don't have permission to start parties. Ask an admin.");
                } else {
                    self.dialog = Some(Dialog::PartyStart { invite: vec![] });
                }
            }
            Message::PartyInviteOpen => self.dialog = Some(Dialog::PartyInvite { invite: vec![] }),
            Message::PartyPick(id, on) => {
                if let Some(Dialog::PartyStart { invite } | Dialog::PartyInvite { invite } | Dialog::Collab { picked: invite, .. }) = &mut self.dialog {
                    invite.retain(|i| *i != id);
                    if on {
                        invite.push(id);
                    }
                }
            }
            Message::PartyLeave => {
                self.dialog = None;
                let (Some(c), Some(p)) = (self.client.clone(), self.party.clone()) else { return Task::none() };
                let text = if p.host == self.my_id() { "Party ended" } else { "You left the party" };
                return Task::perform(
                    blocking(move || c.party_leave(&p.id).map(|_| None)),
                    move |r| Message::PartyDone(r, text.to_string()),
                );
            }
            Message::PartyDone(r, text) => match r {
                Ok(p) => {
                    self.party = p;
                    self.say(text);
                }
                Err(e) => self.say(e.to_string()),
            },
            Message::Next => {
                if self.queue.next(false).is_some() {
                    return self.start_current(true);
                }
                // skipping past the last song ends the queue, like the song finishing
                if self.queue.current().is_some() {
                    self.end_queue();
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
            Message::SeekTo(v) => {
                let to = Duration::from_secs_f32(v.max(0.0));
                self.player.seek(to);
                self.status.position = to;
                self.lyric_line = None;
                if let (Some(link), Some(c)) = (self.cast.clone(), self.client.clone()) {
                    std::thread::spawn(move || {
                        let _ = c.cast_control(&link.device, "seek", Some(to.as_secs_f64()));
                    });
                }
            }
            Message::Volume(v) => {
                self.volume = v;
                self.muted_from = None;
                self.apply_volume(); // feels even across the slider
            }
            Message::ToggleMute => {
                match self.muted_from.take() {
                    Some(v) => self.volume = v,
                    None => {
                        self.muted_from = Some(self.volume);
                        self.volume = 0.0;
                    }
                }
                self.apply_volume();
            }
            Message::JumpQueue(i) => {
                if self.queue.jump(i).is_some() {
                    return self.start_current(true);
                }
            }
            Message::RemoveFromQueue(i) => self.queue.remove(i),
            Message::ClearQueue => {
                if self.party.is_some() {
                    self.say("Not available in a party");
                } else {
                    self.queue.clear_upcoming();
                }
            }
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
            Message::CreditsLoaded(id, r) => {
                self.credits.insert(id, r.map_err(|e| e.to_string()));
            }

            /* ---------- playlists ---------- */
            Message::ToggleLike(track) => {
                let on = !self.is_liked(&track);
                self.say(if on { "Added to Liked Songs" } else { "Removed from Liked Songs" });
                return self.edit(vec![Edit::Like { track, on, at: userdata::now() }]);
            }
            Message::NewPlaylist(add) => {self.desc_editor = iced::widget::text_editor::Content::new(); self.dialog = Some(Dialog::NewPlaylist { name: String::new(), description: String::new(), add, collaborative: false, public: false }); }
            Message::EditPlaylist(id) => {
                if let Some(p) = self.playlist(&id) {
                    self.desc_editor = iced::widget::text_editor::Content::with_text(&p.description);
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
            Message::DataPulled(r) => {
                if let Ok(Some(remote)) = r {
                    // not while something of ours is waiting to be saved: ours goes first, then this comes back around
                    if self.pending.is_empty() && self.in_flight.is_empty() {
                        let sig = |d: &UserData| serde_json::to_string(&(&d.playlists, &d.recent, d.rest.get("settings"), d.rest.get("theme"), d.rest.get("followed"))).unwrap_or_default();
                        if sig(&remote) != sig(&self.data) {
                            self.data = remote;
                            self.refresh_mixes();
                            return self.fetch_covers();
                        }
                    }
                }
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
                    let t = self.edit(vec![Edit::DeletePlaylist(old_id)]);
                    let page = self.go(Page::Playlist(id.clone()));
                    // then invite the people who were ticked
                    let invite = std::mem::take(&mut self.collab_invite);
                    if invite.is_empty() {
                        return Task::batch([t, page]);
                    }
                    self.say("Invites sent");
                    return Task::batch([t, page, self.update(Message::SharedOp(id, json!({ "op": "invite", "userIds": invite })))]);
                }
                Err(e) => self.say(e.to_string()),
            },

            /* ---------- dialogs ---------- */
            Message::DialogText(field, s) => match (&mut self.dialog, field) {
                (Some(Dialog::NewPlaylist { name, .. }), 0) | (Some(Dialog::EditPlaylist { name, .. }), 0) | (Some(Dialog::Rename { name, .. }), 0) => *name = s,
                (Some(Dialog::NewPlaylist { description, .. }), 1) | (Some(Dialog::EditPlaylist { description, .. }), 1) => *description = s,
                (Some(Dialog::AddCast { host, .. }), 0) => *host = s,
                (Some(Dialog::Password { current, .. }), 0) => *current = s,
                (Some(Dialog::Password { new, .. }), 1) => *new = s,
                (Some(Dialog::Manage { password, .. }), 0) => *password = s,
                _ => {}
            },
            Message::DialogFlag(field, v) => match (&mut self.dialog, field) {
                (Some(Dialog::NewPlaylist { collaborative, .. }), 0) => *collaborative = v,
                (Some(Dialog::NewPlaylist { public, .. }), 1) => *public = v,
                (Some(Dialog::EditPlaylist { public, .. }), 0) => *public = v,
                _ => {}
            },
            Message::DialogCancel => {
                self.pop_menu = None;
                self.dialog = None;
                self.account_menu = false;
            }
            Message::AccountMenu(open) => self.account_menu = open,
            Message::DialogOk => return self.dialog_ok(),
            Message::PasswordDone(r) => match r {
                Ok((c, _)) => {
                    self.client = Some(c);
                    self.save_session();
                    self.dialog = None;
                    self.pw_current.clear();
                    self.pw_new.clear();
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
                    self.profile_name = None;
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
                if std::env::var("VH_DEMO").as_deref() == Ok("badge") {
                    return Task::none(); // the demo shows a made-up count
                }
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
            Message::SearchType(t) => {
                self.search_type = t;
                let q = self.search.trim().to_string();
                self.find.searched.clear(); // the kinds shown changed: look again
                return Task::batch([self.fetch_covers(), self.update(Message::SearchSettled(q))]);
            }
            Message::SearchSettled(q) => return self.look_further(q),
            Message::FoundPart(seq, kind, r) => {
                if seq == self.find.seq {
                    match r {
                        Ok((list, can, configured)) => {
                            self.find.can_get = can;
                            self.find.configured = configured;
                            self.find.parts.insert(kind, Ok(list));
                        }
                        Err(e) => {
                            self.find.parts.insert(kind, Err(e.to_string()));
                        }
                    }
                    return self.fetch_covers();
                }
            }
            Message::ShowMoreFound(kind) => {
                self.find.open.insert(kind);
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
            Message::DescEdit(a) => {
                self.desc_editor.perform(a);
                let t = self.desc_editor.text().trim_end_matches('\n').to_string();
                match &mut self.dialog {
                    Some(Dialog::NewPlaylist { description, .. }) | Some(Dialog::EditPlaylist { description, .. }) => *description = t,
                    _ => {}
                }
            }
            Message::ArtistMbid(name, id) => {
                if std::env::var("VH_DEMO").as_deref() == Ok("discover") {
                    return Task::none(); // the demo has made-up results
                }
                self.artist_mbids.insert(name.to_lowercase(), id.clone());
                if let (Some(id), Some(c)) = (id, self.client.clone()) {
                    if !self.discovered.contains_key(&id) {
                        let i = id.clone();
                        return Task::perform(blocking(move || c.discover_artist(&i)), move |r| Message::DiscoverLoaded(id.clone(), r));
                    }
                }
            }
            Message::DiscoverLoaded(id, r) => {
                if let Ok(v) = r {
                    let d = Discovered {
                        name: v["name"].as_str().unwrap_or("").to_string(),
                        albums: serde_json::from_value(v["albums"].clone()).unwrap_or_default(),
                        on_server: v["onServer"].as_str().map(String::from),
                        image: v["image"].as_str().map(String::from),
                    };
                    self.find.can_get = v["canRequest"].as_bool().unwrap_or(false);
                    self.discovered.insert(id, d);
                    return self.fetch_covers();
                }
            }
            Message::DiscoverAlbumLoaded(id, r) => {
                if let Ok(v) = r {
                    self.find.can_get = v["canRequest"].as_bool().unwrap_or(false);
                    self.album_pages.insert(id, v);
                    return self.fetch_covers();
                }
            }
            Message::ArtistInfoLoaded(name, bio, img) => {
                let handle = img.and_then(|b| pictures::rounded_handle(&b, 420, 0.1));
                self.artist_pending.remove(&name);
                self.artist_infos.insert(name, (bio, handle));
                return self.fetch_covers(); // the next few artists
            }
            Message::QueueLoaded(r) => {
                if std::env::var("VH_DEMO").as_deref() == Ok("requests") {
                    return Task::none(); // the demo shows made-up downloads
                }
                if let Ok(q) = r {
                    self.find.queue = Some(q);
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
            Message::SetLeveling(mode) => {
                // the website keeps off | track | album
                let t = self.edit(vec![Edit::Setting("leveling".into(), json!(mode))]);
                let g = self.status.track.clone().map(|id| self.gain_for(&id)).unwrap_or(1.0);
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
            Message::ProfileField(n, s) => match n {
                0 => self.profile_name = Some(s),
                1 => self.pw_current = s,
                2 => self.pw_new = s,
                5 => self.lidarr_url = Some(s),
                6 => self.lidarr_key = s,
                3 => self.new_member_name = s,
                _ => self.new_member_pw = s,
            },
            // the same calls the old dialogs made; a problem is shown in the dialog that opens
            Message::SaveName => {
                let name = self.profile_name.clone().or_else(|| self.me.as_ref().map(|m| m.user.name.clone())).unwrap_or_default();
                self.dialog = Some(Dialog::Rename { name, error: None });
                return self.dialog_ok();
            }
            Message::SavePassword => {
                self.dialog = Some(Dialog::Password { current: self.pw_current.clone(), new: self.pw_new.clone(), error: None, busy: false });
                return self.dialog_ok();
            }
            Message::AddMember => {
                let Some(c) = self.client.clone() else { return Task::none() };
                let (name, pw) = (self.new_member_name.trim().to_string(), self.new_member_pw.clone());
                if name.is_empty() || pw.is_empty() {
                    self.member_error = Some("Enter a name and a password.".into());
                    return Task::none();
                }
                self.member_error = None;
                return Task::perform(blocking(move || c.add_member(&name, &pw)), Message::MemberAdded);
            }
            Message::MembersLoaded(r) => {
                if let Ok(list) = r {
                    self.members = list;
                }
            }
            Message::RewindYear(y) => self.rewind_year = Some(y),
            Message::LidarrLoaded(r) => {
                if let Ok(v) = r {
                    self.lidarr = Some(v);
                }
            }
            Message::LidarrPick(which, name) => match which {
                0 => self.lidarr_root = Some(name),
                1 => self.lidarr_quality = Some(name),
                _ => self.lidarr_meta = Some(name),
            },
            Message::LidarrSave => {
                let Some(c) = self.client.clone() else { return Task::none() };
                let s = self.lidarr.as_ref().map(|l| l["settings"].clone()).unwrap_or_default();
                let l = self.lidarr.as_ref().map(|l| l["lidarr"].clone()).unwrap_or_default();
                // a name picked from a list → its id (profiles) or path (folder); not touched → what the server has
                let id_of = |list: &serde_json::Value, name: &Option<String>, current: &serde_json::Value| -> i64 {
                    let entries = list.as_array().cloned().unwrap_or_default();
                    name.as_ref()
                        .and_then(|n| entries.iter().find(|p| p["name"].as_str() == Some(n.as_str()))?["id"].as_i64())
                        .or_else(|| current.as_i64().filter(|id| entries.iter().any(|p| p["id"].as_i64() == Some(*id))))
                        .or_else(|| entries.first()?["id"].as_i64()) // nothing chosen: the first one, as the drop-down shows
                        .unwrap_or(0)
                };
                let first_root = l["rootFolders"].as_array().and_then(|a| a.first()).and_then(|r| r["path"].as_str()).map(String::from);
                let body = json!({
                    "url": self.lidarr_url.clone().or_else(|| s["url"].as_str().map(String::from)).unwrap_or_default(),
                    "apiKey": self.lidarr_key,
                    "rootFolder": self.lidarr_root.clone().or_else(|| s["rootFolder"].as_str().filter(|p| !p.is_empty()).map(String::from)).or(first_root).unwrap_or_default(),
                    "qualityProfileId": id_of(&l["qualityProfiles"], &self.lidarr_quality, &s["qualityProfileId"]),
                    "metadataProfileId": id_of(&l["metadataProfiles"], &self.lidarr_meta, &s["metadataProfileId"]),
                });
                self.lidarr_error = None;
                return Task::perform(blocking(move || c.save_lidarr(body)), Message::LidarrSaved);
            }
            Message::LidarrOff => {
                let Some(c) = self.client.clone() else { return Task::none() };
                return Task::perform(blocking(move || c.save_lidarr(json!({ "url": "" }))), |r| Message::LidarrSaved(r));
            }
            Message::LidarrSaved(r) => match r {
                Ok(v) => {
                    let first = !self.lidarr.as_ref().map(|l| l["configured"].as_bool().unwrap_or(false)).unwrap_or(false);
                    let on = v["configured"].as_bool().unwrap_or(false);
                    self.say(if !on { "Lidarr disconnected" } else if first { "Lidarr connected" } else { "Saved" });
                    self.lidarr = Some(v);
                    self.lidarr_url = None;
                    self.lidarr_key.clear();
                    self.lidarr_root = None;
                    self.lidarr_quality = None;
                    self.lidarr_meta = None;
                }
                Err(e) => self.lidarr_error = Some(e.to_string()),
            },
            Message::FolderLoaded(r) => self.folder = r.ok(),
            Message::SharesLoaded(r) => self.shares = Some(r.unwrap_or_default()),
            Message::Rescan => {
                let Some(c) = self.client.clone() else { return Task::none() };
                self.say("Checking the music folder…");
                return Task::perform(blocking(move || c.rescan()), Message::RescanDone);
            }
            Message::RescanDone(r) => {
                match r {
                    Ok(()) => self.say("Checking the music folder"),
                    Err(e) => self.say(e.to_string()),
                }
                if let Some(c) = self.client.clone() {
                    return Task::perform(blocking(move || c.status()), Message::ServerStatus);
                }
            }
            Message::PickFolder => {
                self.dialog = Some(Dialog::Folder { browse: None, error: None });
                let Some(c) = self.client.clone() else { return Task::none() };
                return Task::perform(blocking(move || c.browse_folders("")), Message::Browsed);
            }
            Message::BrowseTo(path) => {
                let Some(c) = self.client.clone() else { return Task::none() };
                return Task::perform(blocking(move || c.browse_folders(&path)), Message::Browsed);
            }
            Message::Browsed(r) => {
                if let Some(Dialog::Folder { browse, error }) = &mut self.dialog {
                    match r {
                        Ok(b) => {
                            *browse = Some(b);
                            *error = None;
                        }
                        Err(e) => *error = Some(e.to_string()),
                    }
                }
            }
            Message::ChooseFolder => {
                let (Some(c), Some(Dialog::Folder { browse: Some(b), .. })) = (self.client.clone(), self.dialog.clone()) else { return Task::none() };
                return Task::perform(blocking(move || c.set_library_folder(&b.path)), Message::FolderSaved);
            }
            Message::FolderSaved(r) => match r {
                Ok(()) => {
                    self.dialog = None;
                    self.say("Music folder changed");
                    if let Some(c) = self.client.clone() {
                        return Task::perform(blocking(move || c.library_folder()), Message::FolderLoaded);
                    }
                }
                Err(e) => {
                    if let Some(Dialog::Folder { error, .. }) = &mut self.dialog {
                        *error = Some(e.to_string());
                    }
                }
            },
            Message::ShareCopy(path) => {
                let base = self.client.as_ref().map(|c| c.base().to_string()).unwrap_or_default();
                self.say("Link copied");
                return iced::clipboard::write(format!("{base}{path}"));
            }
            Message::ShareCancel(token) => {
                let Some(c) = self.client.clone() else { return Task::none() };
                return Task::perform(blocking(move || c.cancel_share(&token)), Message::ShareCancelled);
            }
            Message::ShareCancelled(r) => {
                match r {
                    Ok(()) => self.say("Link cancelled"),
                    Err(e) => self.say(e.to_string()),
                }
                if let Some(c) = self.client.clone() {
                    return Task::perform(blocking(move || c.shares()), Message::SharesLoaded);
                }
            }
            Message::ManageMember(id) => self.dialog = Some(Dialog::Manage { id, password: String::new(), error: None, confirm_remove: false }),
            Message::ManageToggle(key) => {
                let (Some(c), Some(Dialog::Manage { id, .. })) = (self.client.clone(), self.dialog.clone()) else { return Task::none() };
                let Some(m) = self.members.iter().find(|m| m.id == id).cloned() else { return Task::none() };
                let body = if key == "hideExplicit" {
                    json!({ "hideExplicit": !m.hide_explicit })
                } else {
                    let mut perms = m.perms.clone();
                    if perms.contains(&key) {
                        perms.retain(|p| *p != key);
                    } else {
                        perms.push(key);
                    }
                    json!({ "perms": perms })
                };
                return Task::perform(blocking(move || c.edit_member(&id, body)), Message::MemberEdited);
            }
            Message::ManagePreset(all) => {
                let (Some(c), Some(Dialog::Manage { id, .. })) = (self.client.clone(), self.dialog.clone()) else { return Task::none() };
                let perms: Vec<&str> = if all { pages::PERMISSIONS.iter().map(|p| p.0).collect() } else { vec!["request", "parties"] };
                return Task::perform(blocking(move || c.edit_member(&id, json!({ "perms": perms }))), Message::MemberEdited);
            }
            Message::MemberEdited(r) => match r {
                Ok(u) => {
                    if let Some(slot) = self.members.iter_mut().find(|m| m.id == u.id) {
                        *slot = u;
                    }
                    if let Some(Dialog::Manage { error, .. }) = &mut self.dialog {
                        *error = None;
                    }
                }
                Err(e) => {
                    if let Some(Dialog::Manage { error, .. }) = &mut self.dialog {
                        *error = Some(e.to_string());
                    }
                }
            },
            Message::ManagePassword => {
                let (Some(c), Some(Dialog::Manage { id, password, .. })) = (self.client.clone(), self.dialog.clone()) else { return Task::none() };
                if password.is_empty() {
                    return Task::none();
                }
                return Task::perform(blocking(move || c.set_member_password(&id, &password)), |r| Message::ManageDone(r, "password"));
            }
            Message::ManageRemove => {
                let (Some(c), Some(Dialog::Manage { id, confirm_remove, .. })) = (self.client.clone(), self.dialog.clone()) else { return Task::none() };
                if !confirm_remove {
                    if let Some(Dialog::Manage { confirm_remove, .. }) = &mut self.dialog {
                        *confirm_remove = true; // asks once more
                    }
                    return Task::none();
                }
                return Task::perform(blocking(move || c.remove_member(&id)), |r| Message::ManageDone(r, "removed"));
            }
            Message::ManageDone(r, what) => match r {
                Ok(()) => {
                    if what == "removed" {
                        self.dialog = None;
                        self.say("Member removed");
                        if let Some(c) = self.client.clone() {
                            return Task::perform(blocking(move || c.members()), Message::MembersLoaded);
                        }
                    } else {
                        if let Some(Dialog::Manage { password, error, .. }) = &mut self.dialog {
                            password.clear();
                            *error = None;
                        }
                        self.say("New password set");
                    }
                }
                Err(e) => {
                    if let Some(Dialog::Manage { error, .. }) = &mut self.dialog {
                        *error = Some(e.to_string());
                    }
                }
            },
            Message::MemberAdded(r) => match r {
                Ok(()) => {
                    self.new_member_name.clear();
                    self.new_member_pw.clear();
                    self.say("Added to the household");
                    if let Some(c) = self.client.clone() {
                        return Task::perform(blocking(move || c.members()), Message::MembersLoaded);
                    }
                }
                Err(e) => self.member_error = Some(e.to_string()),
            },
            Message::SetCrossfade(s) => {
                let secs: f64 = s.parse().unwrap_or(0.0);
                return self.edit(vec![Edit::Setting("crossfade".into(), json!(secs))]);
            }
            Message::PickPicture => {
                return Task::perform(
                    blocking(|| rfd::FileDialog::new().set_title("Choose a picture").add_filter("Pictures", &["jpg", "jpeg", "png", "webp"]).pick_file()),
                    Message::PictureChosen,
                );
            }
            Message::PictureChosen(Some(path)) => {
                return Task::perform(blocking(move || pictures::load(&path).map(std::sync::Arc::new)), |r| Message::CropLoaded(CropTarget::Avatar, r));
            }
            Message::PictureChosen(None) => {}
            Message::PictureSaved(r) => match r {
                Ok(v) => {
                    if let Some(m) = &mut self.me {
                        m.user.avatar = v;
                    }
                    self.save_session();
                    self.say("Picture updated");
                }
                Err(e) => self.say(e),
            },
            Message::RemovePicture => {
                let Some(c) = self.client.clone() else { return Task::none() };
                return Task::perform(blocking(move || c.remove_avatar()), Message::PictureRemoved);
            }
            Message::PictureRemoved(r) => match r {
                Ok(()) => {
                    if let Some(m) = &mut self.me {
                        m.user.avatar = 0;
                    }
                    self.save_session();
                    self.say("Picture removed");
                }
                Err(e) => self.say(e.to_string()),
            },
            Message::AvatarLoaded(id, v, bytes) => {
                self.avatar_pending.remove(&(id.clone(), v));
                if let Some(h) = bytes.and_then(|b| pictures::round_handle(&b)) {
                    self.avatars.insert((id, v), h);
                }
            }
            Message::SetTheme(t) => {
                let task = self.edit(vec![Edit::Theme(t)]);
                self.apply_theme();
                return task;
            }
            Message::SetCrossfadeAlbums(v) => return self.edit(vec![Edit::Setting("crossfadeAlbums".into(), json!(v))]),
            Message::EqOn(v) => {
                let mut e = self.eq_now();
                e.on = v;
                return self.save_eq(e);
            }
            Message::EqPreset(name) => {
                let e = self.eq_now().with_preset(&name);
                return self.save_eq(e);
            }
            Message::EqPre(v) => {
                let mut e = self.eq_now();
                e.pre_db = v.round();
                e.preset = "custom".into();
                return self.save_eq(e);
            }
            Message::EqBand(i, v) => {
                let mut e = self.eq_now();
                if i < 10 {
                    e.gains[i] = v.round();
                    e.preset = "custom".into();
                }
                return self.save_eq(e);
            }
            Message::EqReset => {
                let e = self.eq_now().with_preset("Flat");
                return self.save_eq(e);
            }
            Message::SetStreamQuality(q) => {
                self.settings.set_text("streamQuality", &q);
                self.say(if q == "original" { "Streaming in original quality" } else { "Streaming smaller files from the next song" });
            }
            Message::SetDownloadQuality(q) => {
                self.settings.set_text("downloadQuality", &q);
                self.say("New downloads use this quality");
            }
            Message::DeviceHere => {
                if let Some(r) = self.remote.take() {
                    // take the music over from the device that is playing: same queue, song and spot
                    let now_ms = now_ms() + r.offset_ms;
                    let pos = (if r.playing { r.pos_ms + (now_ms - r.updated_at) } else { r.pos_ms }).max(0.0);
                    self.pending_start = Duration::from_millis(pos as u64);
                    self.dialog = None;
                    let mut tasks = vec![self.start_current(r.playing)];
                    if !r.playing {
                        tasks.push(self.announce_playing());
                    }
                    self.say("Playing here");
                    return Task::batch(tasks);
                }
                self.say("Press play here to move it back")
            }
            Message::DeviceDone(r) => {
                if let Err(e) = r {
                    self.say(e.to_string());
                }
            }
            Message::DeviceTransfer(to) => {
                let Some(c) = self.client.clone() else { return Task::none() };
                if self.queue.current().is_none() {
                    self.say("Start a song first, then move it");
                    return Task::none();
                }
                let name = self.connect_devices.iter().find(|d| d.0 == to).map(|d| d.1.clone()).unwrap_or_else(|| "that device".into());
                let state = self.device_state();
                let me = self.dev_id.clone();
                self.player.pause();
                self.status.playing = false;
                self.dialog = None;
                self.say(format!("Playing on {name}"));
                return Task::perform(blocking(move || c.device_transfer(&me, &to, state)), Message::DeviceDone);
            }
            Message::SetCacheSongs(v) => {
                self.settings.set("cacheSongs", v);
                if v {
                    return self.prefetch();
                }
            }
            Message::SetCacheLimit(v) => {
                self.settings.set_text("cacheLimitMb", &v);
                let keep = self.cache_keep();
                self.song_cache.trim(self.settings.cache_limit_mb * 1024 * 1024, &keep);
                self.cache_stats = self.song_cache.stats();
            }
            Message::ClearCache => {
                let freed = self.song_cache.clear();
                self.cache_stats = self.song_cache.stats();
                self.say(format!("Cleared {}", crate::pages::human_size(freed)));
            }
            Message::CacheFetched(id, r) => {
                self.caching.remove(&id);
                if r.is_ok() {
                    let keep = self.cache_keep();
                    self.song_cache.trim(self.settings.cache_limit_mb * 1024 * 1024, &keep);
                    self.cache_stats = self.song_cache.stats();
                }
                return self.prefetch();
            }
            Message::SetOfflineOnly(v) => return self.edit(vec![Edit::Setting("offlineOnly".into(), json!(v))]),
            Message::SetDataSaver(v) => self.settings.set("dataSaver", v),
            Message::SetResume(v) => return self.edit(vec![Edit::Setting("resumeLast".into(), json!(v))]),
            Message::SetSleep(choice) => {
                self.sleep_song = choice == "song";
                self.sleep_until = choice.parse::<u64>().ok().filter(|m| *m > 0).map(|m| Instant::now() + Duration::from_secs(m * 60));
                self.say(match choice.as_str() {
                    "0" => "Sleep timer off".to_string(),
                    "song" => "Will pause when this song ends".to_string(),
                    m => format!("Will pause in {m} minutes"),
                });
                self.sleep_choice = choice;
            }
            Message::SetDownloadMetered(v) => {
                self.settings.set("downloadMetered", v);
                return self.pump_downloads(); // allowing it starts anything that was waiting
            }
            Message::CheckUpdate => self.check_update(),
            Message::ChangePassword => self.dialog = Some(Dialog::Password { current: String::new(), new: String::new(), error: None, busy: false }),
            Message::RenameMe => self.dialog = Some(Dialog::Rename { name: self.me.as_ref().map(|m| m.user.name.clone()).unwrap_or_default(), error: None }),
            Message::ServerStatus(r) => self.server_status = r.ok(),

            /* ---------- the clock: player status, saving, presence, tray, media keys, updates ---------- */
            Message::Tick => return self.tick(),
            Message::Frame => {} // only here to redraw the launch screen while it shows
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
                    self.hwnd = Some(h);
                    self.media = system::Media::new(h);
                    // keep Windows' own frame (so minimize, maximize, restore and close animate, and
                    // snapping works) but draw our title bar in its place
                    system::install_frame(h, TITLE_BAR, TITLE_BUTTONS);
                }
                // shown only now, so the standard title bar never flashes
                if let (Some(id), false) = (self.window, self.start_hidden) {
                    return Task::batch([window::change_mode(id, window::Mode::Windowed), window::gain_focus(id)]);
                }
            }
            Message::TitlePress => {
                let Some(id) = self.window else { return Task::none() };
                // a double click maximizes / restores, as on any Windows title bar
                if self.last_title_press.map(|t| t.elapsed() < Duration::from_millis(400)).unwrap_or(false) {
                    self.last_title_press = None;
                    return Task::batch([window::toggle_maximize(id), window::get_maximized(id).map(Message::Maximized)]);
                }
                self.last_title_press = Some(Instant::now());
                return window::drag(id);
            }
            Message::Minimize => {
                if let Some(id) = self.window {
                    return window::minimize(id, true);
                }
            }
            Message::ToggleMaximize => {
                if let Some(id) = self.window {
                    self.maximized = !self.maximized;
                    return Task::batch([window::toggle_maximize(id), window::get_maximized(id).map(Message::Maximized)]);
                }
            }
            Message::Maximized(m) => self.maximized = m,
            Message::CheckMaximized => {
                if let Some(id) = self.window {
                    return window::get_maximized(id).map(Message::Maximized);
                }
            }
            Message::CloseWindow => {
                if let Some(id) = self.window {
                    return self.update(Message::CloseRequested(id));
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
            Dialog::NewPlaylist { name, description, add, collaborative, public } => {
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
                    public,
                    ..Default::default()
                };
                self.say(format!("Created \"{name}\""));
                let t = self.edit(vec![Edit::CreatePlaylist(pl.clone())]);
                if collaborative {
                    // a collaborative playlist lives on the server so everyone edits the same copy
                    let Some(c) = self.client.clone() else { return t };
                    let ids = pl.track_ids();
                    let old = id.clone();
                    return Task::batch([t, Task::perform(blocking(move || c.create_shared(&pl.name, &pl.description, pl.public, &ids)), move |r| Message::SharedCreated(r, old.clone()))]);
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
            Dialog::AddTo { .. } | Dialog::PlayOn | Dialog::Manage { .. } | Dialog::Folder { .. } | Dialog::Credits { .. } | Dialog::AlbumInfo { .. } | Dialog::Share { .. } => Task::none(),
            Dialog::CoverLook { target, look, .. } => {
                self.dialog = None;
                // only colours that really are colours are kept
                let ok = |v: Option<String>| v.filter(|c| ui::hex(c).is_some());
                let look = volvix_core::Look { bg: look.bg, font: look.font, c1: ok(look.c1), c2: ok(look.c2), ink: ok(look.ink) };
                match target {
                    LookTarget::Playlist(id) => {
                        self.say("Cover saved");
                        self.edit(vec![Edit::PlaylistLook { id, look: Some(look.to_value()) }])
                    }
                    LookTarget::Album(album) => {
                        let Some(c) = self.client.clone() else { return Task::none() };
                        let (id, l2) = (album.clone(), look.clone());
                        Task::perform(blocking(move || c.set_album_look(&id, &l2)), move |r| Message::CoverSaved(album.clone(), Some(look.clone()), r))
                    }
                }
            }
            Dialog::Crop(c) => {
                self.dialog = None;
                let Some(client) = self.client.clone() else { return Task::none() };
                self.say("Saving the picture…");
                let (cx, cy, side) = (c.cx, c.cy, c.side());
                let img = c.img.clone();
                match c.target {
                    CropTarget::Playlist(id) => Task::perform(
                        blocking(move || (id.clone(), pictures::cut_jpeg(&img, cx, cy, side, 512).and_then(|j| client.set_playlist_cover(&id, &j).map_err(|e| e.to_string())))),
                        |(id, r)| Message::PlaylistPictureSaved(id, r),
                    ),
                    CropTarget::Album(id) => Task::perform(
                        blocking(move || (id.clone(), pictures::cut_jpeg(&img, cx, cy, side, 800).and_then(|j| client.set_album_picture(&id, &j).map_err(|e| e.to_string())))),
                        |(id, r)| Message::AlbumPictureSaved(id, r),
                    ),
                    CropTarget::Avatar => Task::perform(blocking(move || pictures::cut_jpeg(&img, cx, cy, side, 256).and_then(|j| client.set_avatar(&j).map_err(|e| e.to_string()))), Message::PictureSaved),
                }
            }
            Dialog::Collab { id, picked } => {
                self.dialog = None;
                let Some(p) = self.playlist(&id) else { return Task::none() };
                if !p.is_shared() {
                    // inviting someone is what makes it collaborative (it moves to the server, then they're invited)
                    if picked.is_empty() {
                        return Task::none();
                    }
                    let (Some(c), Some(pl)) = (self.client.clone(), self.data.playlists.iter().find(|x| x.id == id).cloned()) else { return Task::none() };
                    let ids = pl.track_ids();
                    self.collab_invite = picked;
                    return Task::perform(blocking(move || c.create_shared(&pl.name, &pl.description, pl.public, &ids)), move |r| Message::SharedCreated(r, id.clone()));
                }
                let current: Vec<String> = p.collaborators.iter().chain(p.invited.iter()).cloned().collect();
                let add: Vec<String> = picked.iter().filter(|x| !current.contains(x)).cloned().collect();
                let drop: Vec<String> = current.iter().filter(|x| !picked.contains(x)).cloned().collect();
                let mut tasks = vec![];
                if !add.is_empty() {
                    self.say("Invites sent");
                    tasks.push(self.update(Message::SharedOp(id.clone(), json!({ "op": "invite", "userIds": add }))));
                } else if !drop.is_empty() {
                    self.say("Updated who can edit");
                }
                for u in drop {
                    tasks.push(self.update(Message::SharedOp(id.clone(), json!({ "op": "uninvite", "userId": u }))));
                }
                Task::batch(tasks)
            }
            Dialog::AddCast { host, busy, .. } => {
                let Some(c) = self.client.clone() else { return Task::none() };
                if busy || host.trim().is_empty() {
                    return Task::none();
                }
                if let Some(Dialog::AddCast { busy, error, .. }) = &mut self.dialog {
                    *busy = true;
                    *error = None;
                }
                Task::perform(blocking(move || c.cast_add(host.trim())), Message::CastAddDone)
            }
            Dialog::PartyStart { invite } => {
                self.dialog = None;
                let Some(c) = self.client.clone() else { return Task::none() };
                let queue = self.queue.items().to_vec();
                let index = self.queue.position().unwrap_or(0);
                let pos_ms = self.status.position.as_millis() as i64;
                let playing = self.status.playing && self.status.track.is_some();
                let names: Vec<String> = self.others().into_iter().filter(|(id, _)| invite.contains(id)).map(|(_, n)| n).collect();
                let text = if names.is_empty() { "Party started".to_string() } else { format!("Party started. Invited {}", names.join(", ")) };
                Task::perform(blocking(move || c.party_start(&invite, &queue, index, pos_ms, playing).map(Some)), move |r| Message::PartyDone(r, text.clone()))
            }
            Dialog::PartyInvite { invite } => {
                self.dialog = None;
                let (Some(c), Some(p)) = (self.client.clone(), self.party.clone()) else { return Task::none() };
                if invite.is_empty() {
                    return Task::none();
                }
                Task::perform(blocking(move || c.party_invite(&p.id, &invite).map(Some)), |r| Message::PartyDone(r, "Invites sent".to_string()))
            }
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
            "crop" => {
                // a made-up wide picture (coloured bands and a grid) to try moving and zooming
                let img = ::image::RgbaImage::from_fn(1200, 800, |x, y| {
                    let line = x % 100 < 3 || y % 100 < 3;
                    if line { ::image::Rgba([255, 255, 255, 255]) } else { ::image::Rgba([(x * 255 / 1200) as u8, (y * 255 / 800) as u8, 160, 255]) }
                });
                let id = self.lib.albums().iter().find(|a| !a.cover).map(|a| a.id.clone()).unwrap_or_default();
                self.dialog = Some(Dialog::Crop(Box::new(CropState::new(CropTarget::Album(id), std::sync::Arc::new(::image::DynamicImage::ImageRgba8(img))))));
            }            "discover" => {
                // pretend Lidarr is connected, and MusicBrainz knows a few more releases by an artist in the library
                let artist = self.lib.artists().first().cloned().unwrap_or_default();
                if let Some(m) = &mut self.me {
                    m.lidarr.configured = true;
                }
                let albums: Vec<FoundItem> = serde_json::from_value(json!([
                    { "foreignId": "a1", "title": "Harbor Songs", "year": "2022", "type": "Album" },
                    { "foreignId": "a2", "title": "Low Tide EP", "year": "2020", "type": "EP" },
                    { "foreignId": "a3", "title": "Night Ferry", "year": "2019", "type": "Single", "requested": true },
                    { "foreignId": "a4", "title": "Already Here", "year": "2018", "type": "Album", "onServer": "x" }
                ]))
                .unwrap_or_default();
                self.discovered.insert("mb1".into(), Discovered { name: artist.clone(), albums, on_server: Some(artist.clone()), image: None });
                self.artist_mbids.insert(artist.to_lowercase(), Some("mb1".into()));
                self.find.can_get = true;
                match std::env::var("VH_DEMO_PAGE").as_deref() {
                    Ok("album") => {
                        self.album_pages.insert("a1".into(), json!({
                            "foreignId": "a1", "title": "Harbor Songs", "artist": artist, "artistId": "mb1", "year": "2022", "type": "Album", "secondary": [],
                            "tracks": [ { "disc": 1, "no": "1", "title": "Low Tide", "lengthMs": 201000 }, { "disc": 1, "no": "2", "title": "Night Ferry", "lengthMs": 187000 }, { "disc": 1, "no": "3", "title": "Harbor Lights", "lengthMs": 242000 } ],
                            "onServer": null, "requested": false, "canRequest": true
                        }));
                        tasks.push(self.go(Page::DiscoverAlbum("a1".into())));
                    }
                    Ok("artist") => tasks.push(self.go(Page::Discover("mb1".into()))),
                    _ => tasks.push(self.go(Page::Artist(artist))),
                }
            }            "requests" => {
                // pretend Lidarr is connected and downloading, to look at the Requests page
                self.find.configured = true;
                self.find.queue = Some(json!({ "items": [
                    { "title": "Harbor Songs", "artist": "Harbor Lights", "progress": 0.42, "state": "downloading", "timeLeft": "00:12:30" },
                    { "title": "Low Tide EP", "artist": "Low Orchard", "progress": 1.0, "state": "importPending", "timeLeft": null }
                ] }));
                tasks.push(self.go(Page::GetMusic));
            }

            "settings" => tasks.push(self.go(Page::Settings)),
            "household" => tasks.push(self.go(Page::Household)),
            "notifications" => tasks.push(self.go(Page::Notifications)),
            "badge" => {
                // look at the bell's count (VH_DEMO_UNREAD = how many)
                self.notes.unread = std::env::var("VH_DEMO_UNREAD").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
            }
            "dialog" => self.dialog = Some(Dialog::NewPlaylist { name: "Sunday morning".into(), description: String::new(), add: vec![], collaborative: false, public: false }),
            "credits" => {
                tasks.push(self.update(Message::PlayAlbum(first.0, 0)));
                self.np_tab = NpTab::Credits;
                self.now_playing = true;
                tasks.push(self.load_lyrics());
            }
            s if s.starts_with("search:") => {
                self.search = s["search:".len()..].to_string();
                tasks.push(self.go(Page::Search));
                tasks.push(self.look_further(self.search.clone()));
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
        let advanced = st.advanced != self.last_advanced;
        self.last_advanced = st.advanced;
        self.status = st;
        if let Some(r) = self.remote.clone() {
            self.mirror_status(&r);
        }
        if advanced {
            tasks.push(self.on_crossfaded());
        }
        self.crossfade_prepare();
        tasks.extend(self.fetch_avatars());
        tasks.extend(self.fetch_playlist_covers());
        self.player.set_eq(self.eq_now()); // follows a change made on the website too
        let dt = self.last_tick.elapsed().as_secs_f64().min(2.0);
        self.last_tick = Instant::now();

        // count plays (not in a private session)
        if self.status.playing && self.remote.is_none() && !self.setting_bool("privateSession", false) {
            if let Some(t) = self.status.track.as_deref().and_then(|id| self.lib.track(id)) {
                let (id, dur) = (t.id.clone(), t.duration);
                let edits = self.counter.tick(&id, dur, dt);
                if !edits.is_empty() {
                    tasks.push(self.edit(edits));
                }
            }
        }
        // the sleep timer: at the time, or when this song ends (then the next song waits, paused)
        if self.sleep_until.map(|t| Instant::now() >= t).unwrap_or(false) {
            self.sleep_until = None;
            self.sleep_choice = "0".into();
            self.player.pause();
            self.say("Sleep timer: paused");
        }
        if ended && self.sleep_song {
            self.sleep_song = false;
            self.sleep_choice = "0".into();
            self.say("Sleep timer: paused");
        } else if ended {
            if self.queue.next(true).is_some() {
                tasks.push(self.start_current(true));
            } else {
                self.end_queue();
            }
        }
        tasks.push(self.resume_last());
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
        tasks.extend(self.devices_tick());
        // keep the song cache topped up with what plays soon
        if Instant::now() >= self.next_cache_at {
            self.next_cache_at = Instant::now() + Duration::from_secs(3);
            tasks.push(self.prefetch());
            if self.page == Page::Settings {
                self.cache_stats = self.song_cache.stats();
            }
        }
        // the server could not be reached: keep looking, and come back by itself when it answers
        if self.offline && self.screen == Screen::Main && Instant::now() >= self.next_reconnect {
            self.next_reconnect = Instant::now() + Duration::from_secs(8);
            if let Some(c) = self.client.clone() {
                tasks.push(Task::perform(blocking(move || c.me().is_ok()), Message::ServerAnswered));
            }
        }
        // pick up what was changed elsewhere (the website, another computer): playlists, likes, recently played, settings
        if Instant::now() >= self.next_pull && self.screen == Screen::Main && !self.offline && self.pending.is_empty() && self.in_flight.is_empty() {
            self.next_pull = Instant::now() + Duration::from_secs(6);
            if self.page == Page::GetMusic && std::env::var("VH_DEMO").as_deref() != Ok("requests") {
                // the Requests page keeps itself up to date while it is open
                if let Some(c) = self.client.clone() {
                    let c2 = c.clone();
                    tasks.push(Task::perform(blocking(move || c2.lidarr_queue()), Message::QueueLoaded));
                    tasks.push(Task::perform(blocking(move || c.requests()), Message::RequestsLoaded));
                }
            }
            if let Some(c) = self.client.clone() {
                tasks.push(Task::perform(blocking(move || c.user_data()), Message::DataPulled));
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
            let now = if share && self.remote.is_none() { (self.status.track.clone(), self.status.playing) } else { (None, false) };
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
        // TVs and speakers: keep the TV in step, check it's still there; and whether a party is going
        if let Some(c) = self.client.clone().filter(|_| !self.offline && self.screen == Screen::Main) {
            if self.cast.is_some() {
                tasks.push(self.cast_sync());
                if Instant::now() >= self.cast_poll {
                    self.cast_poll = Instant::now() + Duration::from_secs(2);
                    let (c2, dev) = (c.clone(), self.cast.as_ref().map(|l| l.device.clone()).unwrap_or_default());
                    tasks.push(Task::perform(blocking(move || c2.cast_status(&dev)), Message::CastStatus));
                }
            }
            if Instant::now() >= self.party_poll {
                self.party_poll = Instant::now() + Duration::from_secs(5);
                tasks.push(Task::perform(blocking(move || c.party_current()), Message::PartyLoaded));
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
        // downloads that were waiting for an unmetered connection
        if self.dl_active.is_none() && !self.dl_queue.is_empty() {
            tasks.push(self.pump_downloads());
        }
        // updates
        if self.settings.auto_update && Instant::now() >= self.next_update_check {
            self.next_update_check = Instant::now() + Duration::from_secs(6 * 3600);
            self.check_update();
        }
        while let Ok(s) = self.update_rx.try_recv() {
            // an installed update starts by itself: the new copy opens and this one exits
            if let update::State::Ready(_) = &s {
                if update::restart() {
                    return iced::exit();
                }
            }
            if let (update::State::Ready(v), Some(t)) = (&s, &self.tray) {
                t.restart.set_enabled(true);
                t.set_tooltip(&format!("Volvix Home: version {v} is ready (restart to finish)"));
            }
            self.update_state = Some(s);
        }
        Task::batch(tasks)
    }

    /// "Resume where I left off": the last queue, song and spot come back, paused. Saved here, every few seconds
    /// while something is loaded; put back once, when the library has arrived.
    fn resume_last(&mut self) -> Task<Message> {
        if self.screen != Screen::Main {
            return Task::none();
        }
        let file = self.dir.join("resume.json");
        if self.resumed {
            if self.status.track.is_some() && self.resume_saved_at.elapsed() >= Duration::from_secs(5) {
                self.resume_saved_at = Instant::now();
                let body = json!({ "ids": self.queue.items(), "index": self.queue.position().unwrap_or(0), "pos": self.status.position.as_millis() as u64 });
                let _ = std::fs::write(&file, body.to_string());
            }
            return Task::none();
        }
        if self.status.track.is_some() || !self.setting_bool("resumeLast", true) || self.resume_saved_at.elapsed() > Duration::from_secs(40) {
            self.resumed = true;
            return Task::none();
        }
        let Some(saved) = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else {
            self.resumed = true;
            return Task::none();
        };
        let ids: Vec<String> = saved["ids"].as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        let index = saved["index"].as_u64().unwrap_or(0) as usize;
        let pos = Duration::from_millis(saved["pos"].as_u64().unwrap_or(0));
        if ids.is_empty() {
            self.resumed = true;
            return Task::none();
        }
        // not yet: the library hasn't loaded this song (it keeps trying for a little while)
        if ids.get(index).map(|id| self.lib.track(id).is_none()).unwrap_or(true) {
            return Task::none();
        }
        self.resumed = true;
        self.queue.play(ids, index, "resume");
        let t = self.start_current(false);
        self.player.seek(pos);
        self.status.position = pos;
        t
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
        // where the pointer is and how big the window is, for pop-up menus (never sends a message)
        subs.push(iced::event::listen_with(|e, _, _| {
            match e {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    if let Ok(mut p) = POINTER.lock() {
                        *p = (position.x, position.y);
                    }
                }
                iced::Event::Window(window::Event::Resized(size)) => {
                    if let Ok(mut w) = WINDOW_SIZE.lock() {
                        *w = (size.width, size.height);
                    }
                }
                _ => {}
            }
            None
        }));
        // a row is picked up: the others ease out of its way, frame by frame
        if self.drag.as_ref().map(|d| d.active).unwrap_or(false) {
            subs.push(window::frames().map(|_| Message::DragFrame));
        }
        // dragging a row: letting go anywhere drops it
        if self.drag.is_some() {
            subs.push(iced::event::listen_with(|e, _, _| match e {
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => Some(Message::DragEnd),
                _ => None,
            }));
        }
        // Escape closes a dialog or Now Playing; Space plays/pauses when no text box is in use
        // the window was maximized or restored some other way (Win+Up, snapping): keep the button right
        subs.push(window::resize_events().map(|_| Message::CheckMaximized));
        // the launch screen animates every frame
        if self.screen == Screen::Loading {
            subs.push(window::frames().map(|_| Message::Frame));
        }
        subs.push(iced::keyboard::on_key_press(|key, mods| {
            use iced::keyboard::{key::Named, Key};
            match key {
                Key::Named(Named::Escape) => Some(Message::DialogCancel),
                Key::Named(Named::ArrowLeft) if mods.alt() => Some(Message::Back),
                Key::Named(Named::BrowserBack) => Some(Message::Back),
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
    // one app identity for the taskbar, the volume mixer and the media overlay, so the sound belongs to Volvix Home
    system::set_app_identity();
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
            // shown once our own title bar is in place (see Message::Hwnd)
            visible: false,
            exit_on_close_request: false,
            ..Default::default()
        })
        .run_with(move || App::new(cell.borrow_mut().take().expect("started once"), start_hidden))
}

/// What the device that is playing said it is playing (its queue is mirrored into this app's queue).
#[derive(Clone)]
struct RemoteNow {
    device: String,
    name: String,
    playing: bool,
    pos_ms: f64,
    updated_at: f64,
    /// the server's clock minus this computer's, in ms
    offset_ms: f64,
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}
