//! Windows bits around the window: one copy only, the tray icon, and the media keys / volume
//! overlay (System Media Transport Controls, through souvlaki).
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// Only this PC can reach it. (Not the web-view app's port, so both can run side by side for now.)
const INSTANCE_ADDR: &str = "127.0.0.1:47472";

/// One copy only: the first copy listens; a second launch says "show" and exits.
pub fn single_instance(wait: bool) -> Option<Receiver<()>> {
    let deadline = Instant::now() + if wait { Duration::from_secs(8) } else { Duration::ZERO };
    let listener = loop {
        match TcpListener::bind(INSTANCE_ADDR) {
            Ok(l) => break l,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(250)),
            Err(_) => {
                if let Ok(mut s) = TcpStream::connect(INSTANCE_ADDR) {
                    let _ = s.write_all(b"show");
                }
                return None;
            }
        }
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            if let Ok(mut s) = conn {
                let mut buf = [0u8; 8];
                let _ = s.read(&mut buf);
                let _ = tx.send(());
            }
        }
    });
    Some(rx)
}

/* ---------- the window frame ----------
 * The window keeps Windows' own frame styles (caption, sizing border, minimize/maximize boxes), so
 * minimizing, maximizing, restoring and closing animate like any app and snapping works. We only
 * hide the caption by claiming its space as our own (WM_NCCALCSIZE), and tell Windows which parts
 * of our drawn title bar act as the caption and where the edges resize (WM_NCHITTEST). */

#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[link(name = "comctl32")]
extern "system" {
    fn SetWindowSubclass(hwnd: isize, proc_: SubclassProc, id: usize, data: usize) -> i32;
    fn DefSubclassProc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
}
extern "system" {
    fn IsZoomed(hwnd: isize) -> i32;
    fn GetDpiForWindow(hwnd: isize) -> u32;
    fn GetSystemMetricsForDpi(index: i32, dpi: u32) -> i32;
    fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
    fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
}

type SubclassProc = unsafe extern "system" fn(isize, u32, usize, isize, usize, usize) -> isize;

const WM_NCCALCSIZE: u32 = 0x0083;
const WM_NCHITTEST: u32 = 0x0084;
const HTCLIENT: isize = 1;
const HTCAPTION: isize = 2;
const HTTOP: isize = 12;
const HTTOPLEFT: isize = 13;
const HTTOPRIGHT: isize = 14;

/// Our title bar's height and the width of its window buttons, in logical pixels (×100, packed).
static BAR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn frame_thickness(hwnd: isize) -> i32 {
    // SAFETY: plain Win32 queries
    unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96);
        GetSystemMetricsForDpi(32 /* SM_CXSIZEFRAME */, dpi) + GetSystemMetricsForDpi(92 /* SM_CXPADDEDBORDER */, dpi)
    }
}

unsafe extern "system" fn frame_proc(hwnd: isize, msg: u32, wparam: usize, lparam: isize, _id: usize, _data: usize) -> isize {
    match msg {
        WM_NCCALCSIZE if wparam != 0 => {
            // the first rect of NCCALCSIZE_PARAMS is the new window rect; what we leave is the client area
            let r = &mut *(lparam as *mut Rect);
            let f = frame_thickness(hwnd);
            if IsZoomed(hwnd) != 0 {
                // maximized windows hang over the screen edge by the frame: keep everything on screen
                r.left += f;
                r.right -= f;
                r.top += f;
                r.bottom -= f;
            } else {
                // keep the side and bottom sizing borders (invisible on Windows 10/11), drop the caption
                r.left += f;
                r.right -= f;
                r.bottom -= f;
            }
            0
        }
        WM_NCHITTEST => {
            let hit = DefSubclassProc(hwnd, msg, wparam, lparam);
            if hit != HTCLIENT {
                return hit; // sides and bottom: Windows' own sizing
            }
            let mut w = Rect { left: 0, top: 0, right: 0, bottom: 0 };
            GetWindowRect(hwnd, &mut w);
            let (x, y) = ((lparam & 0xFFFF) as i16 as i32, ((lparam >> 16) & 0xFFFF) as i16 as i32);
            let scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
            let packed = BAR.load(std::sync::atomic::Ordering::Relaxed);
            let (bar_h, buttons_w) = (((packed >> 32) as f32 / 100.0 * scale) as i32, ((packed & 0xFFFF_FFFF) as f32 / 100.0 * scale) as i32);
            let zoomed = IsZoomed(hwnd) != 0;
            let f = frame_thickness(hwnd);
            // the top edge resizes (we took the caption, and with it the top border)
            if !zoomed && y < w.top + f {
                return if x < w.left + 2 * f { HTTOPLEFT } else if x > w.right - 2 * f { HTTOPRIGHT } else { HTTOP };
            }
            // our title bar: caption (drag, double-click, snap) except over the window buttons
            let top = if zoomed { w.top + f } else { w.top };
            let right = w.right - f; // the client area ends a frame's width in, maximized or not
            if y < top + bar_h && x < right - buttons_w {
                return HTCAPTION;
            }
            HTCLIENT
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

/// Hide the standard title bar, keeping the frame. `bar_h` and `buttons_w` are our drawn title
/// bar's height and window buttons' width, in logical pixels.
pub fn install_frame(hwnd: isize, bar_h: f32, buttons_w: f32) {
    BAR.store((((bar_h * 100.0) as u64) << 32) | (buttons_w * 100.0) as u64, std::sync::atomic::Ordering::Relaxed);
    // SAFETY: subclassing our own window, on its own thread (the UI thread), then asking Windows to
    // recalculate the frame
    unsafe {
        SetWindowSubclass(hwnd, frame_proc, 1, 0);
        const SWP: u32 = 0x0020 /* FRAMECHANGED */ | 0x0002 /* NOMOVE */ | 0x0001 /* NOSIZE */ | 0x0004 /* NOZORDER */ | 0x0010 /* NOACTIVATE */;
        SetWindowPos(hwnd, 0, 0, 0, 0, 0, SWP);
    }
}

/// The Volvix logo for the window and tray (web/index.html: disc r=15, ring r=9.5 at 22% black,
/// centre r=3.5 #111), drawn so no image decoder is needed.
pub fn icon_rgba(n: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((n * n * 4) as usize);
    let c = (n as f32 - 1.0) / 2.0;
    for y in 0..n {
        for x in 0..n {
            let d = (((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt()) / c;
            let px = if d > 1.0 {
                [0, 0, 0, 0]
            } else if d < 3.5 / 15.0 {
                [17, 17, 17, 255]
            } else if (d - 9.5 / 15.0).abs() < 0.5 / 15.0 + 0.5 / c {
                [179, 56, 60, 255]
            } else {
                [229, 72, 77, 255]
            };
            rgba.extend_from_slice(&px);
        }
    }
    rgba
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrayAction {
    Show,
    PlayPause,
    Next,
    StartWithWindows(bool),
    CloseToTray(bool),
    AutoUpdate(bool),
    CheckUpdate,
    Restart,
    Quit,
}

pub struct Tray {
    _icon: TrayIcon,
    open: MenuItem,
    play: MenuItem,
    next: MenuItem,
    start: CheckMenuItem,
    to_tray: CheckMenuItem,
    auto: CheckMenuItem,
    check: MenuItem,
    pub restart: MenuItem,
    quit: MenuItem,
}

/// Does Windows use its light theme for apps right now? (Settings → Personalization → Colors.) Asked at most every 2 seconds.
pub fn system_uses_light() -> bool {
    static LAST: std::sync::Mutex<Option<(Instant, bool)>> = std::sync::Mutex::new(None);
    let Ok(mut last) = LAST.lock() else { return false };
    if let Some((at, v)) = *last {
        if at.elapsed() < Duration::from_secs(2) {
            return v;
        }
    }
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let v = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .and_then(|k| k.get_value::<u32, _>("AppsUseLightTheme"))
        .map(|n| n != 0)
        .unwrap_or(false);
    *last = Some((Instant::now(), v));
    v
}

/// Is the connection in use one Windows calls metered (a mobile hotspot, or one set as "metered connection")?
/// Asked at most every 30 seconds; if Windows can't say, it counts as not metered.
pub fn on_metered_connection() -> bool {
    static LAST: std::sync::Mutex<Option<(Instant, bool)>> = std::sync::Mutex::new(None);
    fn ask() -> Option<bool> {
        use windows::Networking::Connectivity::{NetworkCostType, NetworkInformation};
        let cost = NetworkInformation::GetInternetConnectionProfile().ok()?.GetConnectionCost().ok()?;
        let kind = cost.NetworkCostType().ok()?;
        Some(kind == NetworkCostType::Fixed || kind == NetworkCostType::Variable || cost.OverDataLimit().unwrap_or(false))
    }
    let Ok(mut last) = LAST.lock() else { return false };
    if let Some((at, v)) = *last {
        if at.elapsed() < Duration::from_secs(30) {
            return v;
        }
    }
    let v = ask().unwrap_or(false);
    *last = Some((Instant::now(), v));
    v
}

/// Let Windows draw this program's pop-up menus (the tray menu) in the system's light or dark theme.
/// (uxtheme has no public call for it: ordinal 135 is SetPreferredAppMode, 136 flushes the menu theme cache.)
fn follow_system_menu_theme() {
    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> isize;
        fn GetProcAddress(module: isize, name: usize) -> usize;
    }
    let name: Vec<u16> = "uxtheme.dll\0".encode_utf16().collect();
    unsafe {
        let lib = LoadLibraryW(name.as_ptr());
        if lib == 0 {
            return;
        }
        let set = GetProcAddress(lib, 135);
        if set != 0 {
            let set: extern "system" fn(i32) -> i32 = std::mem::transmute(set);
            set(1); // AllowDark: follows the system setting
        }
        let flush = GetProcAddress(lib, 136);
        if flush != 0 {
            let flush: extern "system" fn() = std::mem::transmute(flush);
            flush();
        }
    }
}

impl Tray {
    pub fn new(start_with_windows: bool, close_to_tray: bool, auto_update: bool) -> Option<Tray> {
        follow_system_menu_theme();
        let open = MenuItem::new("Open Volvix Home", true, None);
        let play = MenuItem::new("Play / pause", true, None);
        let next = MenuItem::new("Next song", true, None);
        let start = CheckMenuItem::new("Start with Windows", true, start_with_windows, None);
        let to_tray = CheckMenuItem::new("Close to the system tray", true, close_to_tray, None);
        let auto = CheckMenuItem::new("Update automatically", true, auto_update, None);
        let check = MenuItem::new("Check for updates", true, None);
        let restart = MenuItem::new("Restart to finish the update", false, None);
        let quit = MenuItem::new("Quit Volvix Home", true, None);
        let menu = Menu::new();
        let sep = PredefinedMenuItem::separator;
        menu.append_items(&[&open, &sep(), &play, &next, &sep(), &start, &to_tray, &auto, &sep(), &check, &restart, &sep(), &quit]).ok()?;
        let mut b = TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("Volvix Home");
        if let Ok(i) = tray_icon::Icon::from_rgba(icon_rgba(32), 32, 32) {
            b = b.with_icon(i);
        }
        let icon = b.build().ok()?;
        Some(Tray { _icon: icon, open, play, next, start, to_tray, auto, check, restart, quit })
    }

    /// Keep the menu's ticks in step with a change made in Settings.
    pub fn set_checks(&self, start: Option<bool>, to_tray: Option<bool>, auto: Option<bool>) {
        if let Some(v) = start {
            self.start.set_checked(v);
        }
        if let Some(v) = to_tray {
            self.to_tray.set_checked(v);
        }
        if let Some(v) = auto {
            self.auto.set_checked(v);
        }
    }

    pub fn set_tooltip(&self, s: &str) {
        let _ = self._icon.set_tooltip(Some(s));
    }

    /// Menu clicks and icon clicks since last time.
    pub fn poll(&self) -> Vec<TrayAction> {
        let mut out = vec![];
        while let Ok(e) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                out.push(TrayAction::Show);
            }
        }
        while let Ok(MenuEvent { id }) = MenuEvent::receiver().try_recv() {
            let a = if id == *self.open.id() {
                TrayAction::Show
            } else if id == *self.play.id() {
                TrayAction::PlayPause
            } else if id == *self.next.id() {
                TrayAction::Next
            } else if id == *self.start.id() {
                TrayAction::StartWithWindows(self.start.is_checked())
            } else if id == *self.to_tray.id() {
                TrayAction::CloseToTray(self.to_tray.is_checked())
            } else if id == *self.auto.id() {
                TrayAction::AutoUpdate(self.auto.is_checked())
            } else if id == *self.check.id() {
                TrayAction::CheckUpdate
            } else if id == *self.restart.id() {
                TrayAction::Restart
            } else if id == *self.quit.id() {
                TrayAction::Quit
            } else {
                continue;
            };
            out.push(a);
        }
        out
    }
}

/// Media keys, the Windows volume/media overlay and the lock screen.
pub struct Media {
    controls: MediaControls,
    rx: Receiver<MediaControlEvent>,
    last: Option<(String, bool)>,
}

impl Media {
    /// `hwnd` is the main window's handle.
    pub fn new(hwnd: isize) -> Option<Media> {
        let config = PlatformConfig { dbus_name: "volvix_home", display_name: "Volvix Home", hwnd: Some(hwnd as *mut std::ffi::c_void) };
        let mut controls = MediaControls::new(config).ok()?;
        let (tx, rx) = mpsc::channel();
        controls.attach(move |e| {
            let _ = tx.send(e);
        }).ok()?;
        Some(Media { controls, rx, last: None })
    }

    pub fn poll(&self) -> Vec<MediaControlEvent> {
        self.rx.try_iter().collect()
    }

    /// Tell Windows what's playing. `cover_file` is a cached cover on disk (Windows reads it itself).
    pub fn show(&mut self, track_id: &str, title: &str, artist: &str, album: &str, cover_file: Option<&std::path::Path>, duration: Option<Duration>, playing: bool, position: Duration) {
        let key = (track_id.to_string(), playing);
        if self.last.as_ref() == Some(&key) {
            return;
        }
        if self.last.as_ref().map(|l| l.0.as_str()) != Some(track_id) {
            let url = cover_file.map(|p| format!("file://{}", p.display()));
            let _ = self.controls.set_metadata(MediaMetadata { title: Some(title), artist: Some(artist), album: Some(album), cover_url: url.as_deref(), duration });
        }
        let progress = Some(MediaPosition(position));
        let _ = self.controls.set_playback(if playing { MediaPlayback::Playing { progress } } else { MediaPlayback::Paused { progress } });
        self.last = Some(key);
    }

    pub fn clear(&mut self) {
        let _ = self.controls.set_playback(MediaPlayback::Stopped);
        self.last = None;
    }
}

/// Tell Windows this process is the app "Volvix Home" (the name and icon in the volume mixer, taskbar and media overlay).
pub fn set_app_identity() {
    use windows::core::w;
    unsafe {
        let _ = windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(w!("app.volvix.home"));
    }
    // Windows names an app in the volume mixer and the media overlay from what is registered for its id; without this it says
    // "Unknown app". Registering the name and icon needs no installer and no admin rights.
    if let Ok(exe) = std::env::current_exe() {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        if let Ok((key, _)) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(r"Software\Classes\AppUserModelId\app.volvix.home") {
            let _ = key.set_value("DisplayName", &"Volvix Home");
            let _ = key.set_value("IconUri", &exe.display().to_string());
            let _ = key.set_value("IconBackgroundColor", &"FFE5484D");
        }
    }
}
