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

/// Red record icon, drawn so no image decoder is needed (same as the web-view app).
pub fn icon_rgba(n: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((n * n * 4) as usize);
    let c = (n as f32 - 1.0) / 2.0;
    for y in 0..n {
        for x in 0..n {
            let d = (((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt()) / c;
            let px = if d > 1.0 {
                [0, 0, 0, 0]
            } else if d < 0.22 {
                [17, 17, 17, 255]
            } else if (d - 0.6).abs() < 0.035 {
                [200, 60, 65, 255]
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

impl Tray {
    pub fn new(start_with_windows: bool, close_to_tray: bool, auto_update: bool) -> Option<Tray> {
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
