// Volvix Home for Windows.
//
// A thin native window onto your own Volvix Home server. On first launch it asks for the server's
// address (a small built-in page), remembers it, and from then on opens straight to your library.
// Everything is served by your server; this app is just the window, with its own saved sign-in.
//
// It also: runs one copy only (opening it again brings the open window forward), starts with Windows,
// closes to the system tray so music keeps playing, and updates itself from GitHub releases.
// Start with Windows, close to tray and automatic updates can each be turned off (tray menu or Settings).
#![windows_subsystem = "windows"]

mod settings;
mod update;

use std::borrow::Cow;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Icon, Theme, WindowBuilder};
use tao::dpi::LogicalSize;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use wry::http::header::CONTENT_TYPE;
use wry::{NewWindowResponse, WebContext, WebViewBuilder};

use settings::Settings;

// host must be "localhost"; on Windows WebView2 serves custom protocols as http://vh.localhost/...
const CONNECT_URL: &str = "vh://localhost/connect.html";
const CONNECT_HTML: &str = include_str!("../assets/connect.html");
/// Only this PC can reach it: a second launch tells the first one to come forward.
const INSTANCE_ADDR: &str = "127.0.0.1:47471";
/// The release file the updater downloads (see update.rs) and the Windows startup entry (settings.rs).
pub const RELEASE_EXE: &str = "volvix-home.exe";
pub const RUN_NAME: &str = "Volvix Home";

/// UI-thread messages from the page (IPC), the tray and background threads.
enum UserEvent {
    /// Open a URL in the window (after the connect page's address is saved).
    Navigate(String),
    /// Someone opened the app again, or the tray icon was clicked.
    Show,
    Menu(MenuId),
    /// A setting changed in the page's Settings.
    Setting(String, bool),
    CheckUpdate,
    /// Update progress for the tray and the page: (state, version).
    Update(update::State),
    Restart,
}

/// %LOCALAPPDATA%\Volvix Home — holds the saved settings and the WebView's data (sign-in).
pub fn app_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("Volvix Home")
}

/// Red record icon for the title bar / taskbar, drawn so no image decoder is needed.
fn icon_rgba(n: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((n * n * 4) as usize);
    let c = (n as f32 - 1.0) / 2.0;
    for y in 0..n {
        for x in 0..n {
            let d = (((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt()) / c;
            let px = if d > 1.0 { [0, 0, 0, 0] }
                else if d < 0.22 { [17, 17, 17, 255] }
                else if (d - 0.6).abs() < 0.035 { [200, 60, 65, 255] }
                else { [229, 72, 77, 255] };
            rgba.extend_from_slice(&px);
        }
    }
    rgba
}
fn window_icon() -> Option<Icon> { Icon::from_rgba(icon_rgba(64), 64, 64).ok() }

fn host_of(url: &str) -> &str {
    url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or("")
}
fn same_host(a: &str, b: &str) -> bool {
    !a.is_empty() && host_of(a) == host_of(b)
}

/// The app only ever shows the built-in connect page or your own server. It never opens your browser.
fn allowed(url: &str, server: &str) -> bool {
    url.starts_with("vh://")
        || host_of(url) == "vh.localhost"   // the built-in connect page as WebView2 rewrites it on Windows
        || url.starts_with("about:")
        || url.starts_with("data:")
        || url.starts_with("blob:")
        || same_host(server, url)
}

/// One copy only. The first copy listens on a local port; a second launch connects, says "show", and exits.
/// (After an update the new copy waits a moment for the old one to let go.)
fn single_instance(wait: bool) -> Option<TcpListener> {
    let deadline = Instant::now() + if wait { Duration::from_secs(8) } else { Duration::ZERO };
    loop {
        match TcpListener::bind(INSTANCE_ADDR) {
            Ok(l) => return Some(l),
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(250)),
            Err(_) => {
                if let Ok(mut s) = TcpStream::connect(INSTANCE_ADDR) { let _ = s.write_all(b"show"); }
                return None;
            }
        }
    }
}

fn main() -> wry::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let background = args.iter().any(|a| a == "--background");
    let after_update = args.iter().any(|a| a == "--after-update");

    let Some(listener) = single_instance(after_update) else { return Ok(()) };
    update::clean_old();

    let settings = Arc::new(Mutex::new(Settings::load()));
    // first run: start with Windows is on unless the person turns it off
    settings.lock().unwrap().apply_autostart();

    let server = Arc::new(Mutex::new(settings.lock().unwrap().server.clone()));
    let start_url = server.lock().unwrap().clone().map(|s| format!("{s}/")).unwrap_or_else(|| CONNECT_URL.to_string());
    let saved_for_script = server.lock().unwrap().clone().unwrap_or_default();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    // a second launch connects to this port: bring the window forward
    {
        let p = proxy.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                if let Ok(mut s) = conn { let mut buf = [0u8; 8]; let _ = s.read(&mut buf); let _ = p.send_event(UserEvent::Show); }
            }
        });
    }

    // started by Windows at sign-in (--background) and living in the tray: don't pop a window up
    let start_hidden = background && settings.lock().unwrap().close_to_tray;
    let window = WindowBuilder::new()
        .with_title("Volvix Home")
        .with_inner_size(LogicalSize::new(1180.0, 800.0))
        .with_min_inner_size(LogicalSize::new(380.0, 560.0))
        .with_window_icon(window_icon())
        .with_theme(Some(Theme::Dark))
        .with_visible(!start_hidden)
        .build(&event_loop)
        .expect("could not create the window");

    // keep sign-in and settings between launches
    let mut context = WebContext::new(Some(app_dir().join("WebView")));

    // Injected into every page: marks this as the app (the web app can show "Switch server" and the Windows
    // app settings), hands the connect page the last address, and makes Ctrl+Shift+S reopen the connect screen.
    let init = format!(
        "window.VOLVIX_NATIVE = {{ app: 'windows', version: '{}', settings: {} }};\n\
         window.VH_SAVED_SERVER = {saved};\n\
         window.addEventListener('keydown', function (e) {{ if (e.ctrlKey && e.shiftKey && (e.key === 'S' || e.key === 's')) {{ e.preventDefault(); location.href = 'http://vh.localhost/connect.html'; }} }});",
        env!("CARGO_PKG_VERSION"),
        settings.lock().unwrap().to_json(),
        saved = serde_json::to_string(&saved_for_script).unwrap_or_else(|_| "\"\"".into())
    );

    let ipc_proxy = proxy.clone();
    let ipc_server = server.clone();
    let ipc_settings = settings.clone();
    let nav_server = server.clone();
    let popup_server = server.clone();
    let webview = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(&start_url)
        .with_initialization_script(&init)
        .with_custom_protocol("vh".into(), move |_id, request| {
            // only the built-in connect page is served locally
            if request.uri().path().ends_with("connect.html") {
                wry::http::Response::builder().header(CONTENT_TYPE, "text/html; charset=utf-8")
                    .body(Cow::Borrowed(CONNECT_HTML.as_bytes())).unwrap()
            } else {
                wry::http::Response::builder().status(404).body(Cow::Borrowed(&b"Not found"[..])).unwrap()
            }
        })
        // messages from the page: the connect page's address, and the Windows-app settings
        .with_ipc_handler(move |request| {
            let Ok(msg) = serde_json::from_str::<serde_json::Value>(request.body()) else { return };
            match msg.get("type").and_then(|t| t.as_str()) {
                Some("connect") => {
                    if let Some(url) = msg.get("url").and_then(|u| u.as_str()) {
                        let url = url.trim_end_matches('/').to_string();
                        ipc_settings.lock().unwrap().set_server(&url);
                        *ipc_server.lock().unwrap() = Some(url.clone());
                        let _ = ipc_proxy.send_event(UserEvent::Navigate(format!("{url}/")));
                    }
                }
                Some("setting") => {
                    if let (Some(k), Some(v)) = (msg.get("key").and_then(|k| k.as_str()), msg.get("value").and_then(|v| v.as_bool())) {
                        let _ = ipc_proxy.send_event(UserEvent::Setting(k.to_string(), v));
                    }
                }
                Some("checkUpdate") => { let _ = ipc_proxy.send_event(UserEvent::CheckUpdate); }
                Some("restart") => { let _ = ipc_proxy.send_event(UserEvent::Restart); }
                _ => {}
            }
        })
        // Stay on the connect page or your own server. A link anywhere else is simply ignored — the app
        // never opens your browser or navigates away on its own.
        .with_navigation_handler(move |url| {
            let server = nav_server.lock().unwrap().clone().unwrap_or_default();
            allowed(&url, &server)
        })
        // window.open / target=_blank: don't spawn anything (same rule as navigation)
        .with_new_window_req_handler(move |url, _features| {
            let server = popup_server.lock().unwrap().clone().unwrap_or_default();
            if allowed(&url, &server) { NewWindowResponse::Allow } else { NewWindowResponse::Deny }
        })
        .build(&window)?;

    /* ---------- the tray icon ---------- */
    let s0 = settings.lock().unwrap().clone();
    let m_open = MenuItem::new("Open Volvix Home", true, None);
    let m_start = CheckMenuItem::new("Start with Windows", true, s0.start_with_windows, None);
    let m_tray = CheckMenuItem::new("Close to the system tray", true, s0.close_to_tray, None);
    let m_auto = CheckMenuItem::new("Update automatically", true, s0.auto_update, None);
    let m_check = MenuItem::new("Check for updates", true, None);
    let m_restart = MenuItem::new("Restart to finish the update", false, None);
    let m_quit = MenuItem::new("Quit Volvix Home", true, None);
    let menu = Menu::new();
    let _ = menu.append_items(&[&m_open, &PredefinedMenuItem::separator(), &m_start, &m_tray, &m_auto, &PredefinedMenuItem::separator(), &m_check, &m_restart, &PredefinedMenuItem::separator(), &m_quit]);
    let tray_image = tray_icon::Icon::from_rgba(icon_rgba(32), 32, 32).ok();
    let mut tray_builder = TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("Volvix Home");
    if let Some(i) = tray_image { tray_builder = tray_builder.with_icon(i); }
    let _tray = tray_builder.build().ok(); // no tray (rare): the close button then really closes the app
    let have_tray = _tray.is_some();
    {
        let p = proxy.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| { let _ = p.send_event(UserEvent::Menu(e.id)); }));
        let p = proxy.clone();
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e { let _ = p.send_event(UserEvent::Show); }
        }));
    }

    // look for an update shortly after start, then every six hours
    let check = {
        let p = proxy.clone();
        let s = settings.clone();
        move |manual: bool| {
            let p = p.clone();
            let auto = s.lock().unwrap().auto_update;
            std::thread::spawn(move || {
                if !manual && !auto { return; }
                let _ = p.send_event(UserEvent::Update(update::State::Checking));
                let q = p.clone();
                let done = update::run(&move |s| { let _ = q.send_event(UserEvent::Update(s)); });
                let _ = p.send_event(UserEvent::Update(done));
            });
        }
    };
    {
        let c = check.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(20));
            loop { c(false); std::thread::sleep(Duration::from_secs(6 * 3600)); }
        });
    }

    let page_state = |wv: &wry::WebView, js: String| { let _ = wv.evaluate_script(&js); };
    let mut update_ready = false;
    let settings_loop = settings.clone();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        let show = |w: &tao::window::Window| { w.set_visible(true); w.set_minimized(false); w.set_focus(); };
        match event {
            Event::UserEvent(UserEvent::Navigate(url)) => { let _ = webview.load_url(&url); }
            Event::UserEvent(UserEvent::Show) => show(&window),
            Event::UserEvent(UserEvent::Setting(key, value)) => {
                let mut s = settings_loop.lock().unwrap();
                s.set(&key, value);
                match key.as_str() {
                    "startWithWindows" => m_start.set_checked(value),
                    "closeToTray" => m_tray.set_checked(value),
                    "autoUpdate" => m_auto.set_checked(value),
                    _ => {}
                }
            }
            Event::UserEvent(UserEvent::Menu(id)) => {
                if id == *m_open.id() { show(&window); }
                else if id == *m_quit.id() { *control_flow = ControlFlow::Exit; }
                else if id == *m_check.id() { check(true); }
                else if id == *m_restart.id() { if update::restart() { *control_flow = ControlFlow::Exit; } }
                else {
                    let (key, value) = if id == *m_start.id() { ("startWithWindows", m_start.is_checked()) }
                        else if id == *m_tray.id() { ("closeToTray", m_tray.is_checked()) }
                        else if id == *m_auto.id() { ("autoUpdate", m_auto.is_checked()) }
                        else { return };
                    settings_loop.lock().unwrap().set(key, value);
                    page_state(&webview, format!("window.VOLVIX_NATIVE && (window.VOLVIX_NATIVE.settings['{key}'] = {value});"));
                }
            }
            Event::UserEvent(UserEvent::CheckUpdate) => check(true),
            Event::UserEvent(UserEvent::Restart) => { if update::restart() { *control_flow = ControlFlow::Exit; } }
            Event::UserEvent(UserEvent::Update(state)) => {
                if let update::State::Ready(_) = state { update_ready = true; m_restart.set_enabled(true); }
                let _ = update_ready;
                let js = serde_json::to_string(&state.to_json()).unwrap_or_else(|_| "null".into());
                page_state(&webview, format!("if (window.VOLVIX_NATIVE) {{ window.VOLVIX_NATIVE.update = {js}; window.dispatchEvent(new Event('vh-native-update')); }}"));
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                // the close button hides the window and the music keeps playing; Quit in the tray really exits
                if have_tray && settings_loop.lock().unwrap().close_to_tray { window.set_visible(false); }
                else { *control_flow = ControlFlow::Exit; }
            }
            _ => {}
        }
    });
}
