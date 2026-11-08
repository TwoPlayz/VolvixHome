// Volvix Home for Windows.
//
// A thin native window onto your own Volvix Home server. On first launch it asks for the server's
// address (a small built-in page), remembers it, and from then on opens straight to your library.
// Everything is served by your server; this app is just the window, with its own saved sign-in.
#![windows_subsystem = "windows"]

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Icon, Theme, WindowBuilder};
use tao::dpi::LogicalSize;
use wry::http::header::CONTENT_TYPE;
use wry::{NewWindowResponse, WebContext, WebViewBuilder};

// host must be "localhost"; on Windows WebView2 serves custom protocols as http://vh.localhost/...
const CONNECT_URL: &str = "vh://localhost/connect.html";
const CONNECT_HTML: &str = include_str!("../assets/connect.html");

/// UI-thread messages from the page (IPC) and elsewhere.
enum UserEvent {
    /// Open a URL in the window (after the connect page's address is saved).
    Navigate(String),
}

/// %LOCALAPPDATA%\Volvix Home — holds the saved server address and the WebView's data (sign-in).
fn app_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("Volvix Home")
}

fn config_path() -> PathBuf {
    let dir = app_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("config.json")
}

fn load_server() -> Option<String> {
    let text = std::fs::read_to_string(config_path()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("server").and_then(|s| s.as_str()).map(|s| s.trim_end_matches('/').to_string()).filter(|s| !s.is_empty())
}

fn save_server(url: &str) {
    let body = serde_json::json!({ "server": url.trim_end_matches('/') }).to_string();
    let _ = std::fs::write(config_path(), body);
}

/// Red record icon for the title bar / taskbar, drawn so no image decoder is needed.
fn window_icon() -> Option<Icon> {
    let n = 64u32;
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
    Icon::from_rgba(rgba, n, n).ok()
}

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

fn main() -> wry::Result<()> {
    let server = Arc::new(Mutex::new(load_server()));
    let start_url = server.lock().unwrap().clone().map(|s| format!("{s}/")).unwrap_or_else(|| CONNECT_URL.to_string());
    let saved_for_script = server.lock().unwrap().clone().unwrap_or_default();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("Volvix Home")
        .with_inner_size(LogicalSize::new(1180.0, 800.0))
        .with_min_inner_size(LogicalSize::new(380.0, 560.0))
        .with_window_icon(window_icon())
        .with_theme(Some(Theme::Dark))
        .build(&event_loop)
        .expect("could not create the window");

    // keep sign-in and settings between launches
    let mut context = WebContext::new(Some(app_dir().join("WebView")));

    // Injected into every page: marks this as the app (the web app can show "Switch server"),
    // hands the connect page the last address, and makes Ctrl+Shift+S reopen the connect screen.
    let init = format!(
        "window.VOLVIX_NATIVE = {{ app: 'windows', version: '{}' }};\n\
         window.VH_SAVED_SERVER = {saved};\n\
         window.addEventListener('keydown', function (e) {{ if (e.ctrlKey && e.shiftKey && (e.key === 'S' || e.key === 's')) {{ e.preventDefault(); location.href = '{CONNECT_URL}'; }} }});",
        env!("CARGO_PKG_VERSION"),
        saved = serde_json::to_string(&saved_for_script).unwrap_or_else(|_| "\"\"".into())
    );

    let proxy = event_loop.create_proxy();
    let ipc_proxy = proxy.clone();
    let ipc_server = server.clone();
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
        // the connect page posts {type:'connect', url} once the server answers; we save it, then open it
        .with_ipc_handler(move |request| {
            let Ok(msg) = serde_json::from_str::<serde_json::Value>(request.body()) else { return };
            if msg.get("type").and_then(|t| t.as_str()) == Some("connect") {
                if let Some(url) = msg.get("url").and_then(|u| u.as_str()) {
                    let url = url.trim_end_matches('/').to_string();
                    save_server(&url);
                    *ipc_server.lock().unwrap() = Some(url.clone());
                    let _ = ipc_proxy.send_event(UserEvent::Navigate(format!("{url}/")));
                }
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

    let _keep = proxy;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::Navigate(url)) => { let _ = webview.load_url(&url); }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}
