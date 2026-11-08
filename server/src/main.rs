//! Volvix Home server.
//!
//! Serves the Volvix Home web app and a small JSON API:
//!   accounts (household users, sign-in), the music library (scanned from VH_MUSIC_DIR, e.g. the folder
//!   Lidarr imports into), audio streaming with seeking (HTTP Range), cover art, and per-user app data
//!   (playlists, settings, play position) so every device picks up where you left off.
//!
//! Settings (environment variables):
//!   VH_MUSIC_DIR        music folder              (default /music)
//!   VH_DATA_DIR         accounts and app data     (default /data)
//!   VH_WEB_DIR          the web app               (default ./web, else /app/web)
//!   VH_PORT             port                      (default 4747)
//!   VH_RESCAN_MINUTES   look for new music every N minutes, 0 = off (default 15)

mod auth;
mod library;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

use auth::{Accounts, PublicUser, User};
use library::Library;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const COOKIE: &str = "vh_session";

#[derive(Clone)]
struct Config {
    music_dir: PathBuf,
    data_dir: PathBuf,
    web_dir: PathBuf,
    port: u16,
    rescan_minutes: u64,
}

impl Config {
    fn from_env() -> Config {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let web_default = if PathBuf::from("web").join("index.html").exists() { PathBuf::from("web") } else { PathBuf::from("/app/web") };
        Config {
            music_dir: var("VH_MUSIC_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/music")),
            data_dir: var("VH_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/data")),
            web_dir: var("VH_WEB_DIR").map(PathBuf::from).unwrap_or(web_default),
            port: var("VH_PORT").and_then(|p| p.parse().ok()).unwrap_or(4747),
            rescan_minutes: var("VH_RESCAN_MINUTES").and_then(|p| p.parse().ok()).unwrap_or(15),
        }
    }
}

struct AppState {
    cfg: Config,
    lib: RwLock<Library>,
    accounts: Mutex<Accounts>,
}
type Shared = Arc<AppState>;

/* ---------------- helpers ---------------- */

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn token_from(headers: &HeaderMap) -> Option<String> {
    if let Some(auth) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(t) = auth.strip_prefix("Bearer ") {
            return Some(t.trim().to_string());
        }
    }
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|c| c.trim().strip_prefix(&format!("{COOKIE}=")).map(String::from))
}

/// The signed-in user, or a 401 response.
fn signed_in(st: &AppState, headers: &HeaderMap) -> Result<(String, User), Response> {
    let token = token_from(headers).ok_or_else(|| error(StatusCode::UNAUTHORIZED, "Sign in first."))?;
    let accounts = st.accounts.lock().unwrap();
    let user = accounts.user_for_token(&token).cloned().ok_or_else(|| error(StatusCode::UNAUTHORIZED, "Sign in first."))?;
    Ok((token, user))
}

fn admin(st: &AppState, headers: &HeaderMap) -> Result<User, Response> {
    let (_, user) = signed_in(st, headers)?;
    if !user.admin {
        return Err(error(StatusCode::FORBIDDEN, "Only an admin can do that."));
    }
    Ok(user)
}

fn session_cookie(token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=15552000")).unwrap()
}

fn signed_in_response(user: &User, token: &str) -> Response {
    let mut res = Json(json!({ "user": PublicUser::from(user), "token": token })).into_response();
    res.headers_mut().insert(header::SET_COOKIE, session_cookie(token));
    res
}

fn user_data_path(st: &AppState, user_id: &str) -> PathBuf {
    st.cfg.data_dir.join("userdata").join(format!("{user_id}.json"))
}

/* ---------------- accounts ---------------- */

async fn status(State(st): State<Shared>) -> Response {
    let lib = st.lib.read().unwrap();
    let setup = st.accounts.lock().unwrap().needs_setup();
    Json(json!({
        "name": "Volvix Home", "version": VERSION, "setupNeeded": setup,
        "scanning": lib.scanning, "albums": lib.catalog.albums.len(), "tracks": lib.catalog.tracks.len()
    }))
    .into_response()
}

#[derive(Deserialize)]
struct NewAccount {
    username: String,
    #[serde(default)]
    name: String,
    password: String,
    #[serde(default)]
    admin: bool,
}

/// First run only: whoever opens a fresh server creates the admin account.
async fn setup(State(st): State<Shared>, Json(body): Json<NewAccount>) -> Response {
    let mut accounts = st.accounts.lock().unwrap();
    if !accounts.needs_setup() {
        return error(StatusCode::CONFLICT, "This server is already set up. Sign in instead.");
    }
    match accounts.create(&body.username, &body.name, &body.password, true) {
        Ok(user) => {
            let token = accounts.start_session(&user.id);
            signed_in_response(&user, &token)
        }
        Err(e) => error(StatusCode::BAD_REQUEST, &e),
    }
}

#[derive(Deserialize)]
struct Login {
    username: String,
    password: String,
}

async fn login(State(st): State<Shared>, Json(body): Json<Login>) -> Response {
    let found = {
        let accounts = st.accounts.lock().unwrap();
        accounts.find(&body.username).filter(|u| auth::verify_password(&body.password, &u.hash)).cloned()
    };
    match found {
        Some(user) => {
            let token = st.accounts.lock().unwrap().start_session(&user.id);
            signed_in_response(&user, &token)
        }
        None => {
            tokio::time::sleep(Duration::from_millis(400)).await; // slow down guessing
            error(StatusCode::UNAUTHORIZED, "Username or password is incorrect.")
        }
    }
}

async fn logout(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Some(t) = token_from(&headers) {
        st.accounts.lock().unwrap().end_session(&t);
    }
    let mut res = StatusCode::NO_CONTENT.into_response();
    res.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_static("vh_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"));
    res
}

async fn me(State(st): State<Shared>, headers: HeaderMap) -> Response {
    match signed_in(&st, &headers) {
        Ok((_, user)) => Json(json!({ "user": PublicUser::from(&user) })).into_response(),
        Err(r) => r,
    }
}

#[derive(Deserialize)]
struct PasswordChange {
    current: String,
    new: String,
}

async fn change_password(State(st): State<Shared>, headers: HeaderMap, Json(body): Json<PasswordChange>) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    if !auth::verify_password(&body.current, &user.hash) {
        return error(StatusCode::BAD_REQUEST, "Your current password is incorrect.");
    }
    let mut accounts = st.accounts.lock().unwrap();
    if let Err(e) = accounts.set_password(&user.id, &body.new) {
        return error(StatusCode::BAD_REQUEST, &e);
    }
    let token = accounts.start_session(&user.id); // stay signed in here
    signed_in_response(&user, &token)
}

/* ---------------- household (admin) ---------------- */

async fn list_users(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = admin(&st, &headers) {
        return r;
    }
    let accounts = st.accounts.lock().unwrap();
    let users: Vec<PublicUser> = accounts.users.iter().map(PublicUser::from).collect();
    Json(json!({ "users": users })).into_response()
}

async fn add_user(State(st): State<Shared>, headers: HeaderMap, Json(body): Json<NewAccount>) -> Response {
    if let Err(r) = admin(&st, &headers) {
        return r;
    }
    match st.accounts.lock().unwrap().create(&body.username, &body.name, &body.password, body.admin) {
        Ok(user) => Json(json!({ "user": PublicUser::from(&user) })).into_response(),
        Err(e) => error(StatusCode::BAD_REQUEST, &e),
    }
}

async fn remove_user(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let me = match admin(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    if me.id == id {
        return error(StatusCode::BAD_REQUEST, "You can't remove yourself.");
    }
    match st.accounts.lock().unwrap().remove(&id) {
        Ok(()) => {
            let _ = std::fs::remove_file(user_data_path(&st, &id));
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => error(StatusCode::BAD_REQUEST, &e),
    }
}

/* ---------------- per-user app data (playlists, settings, resume point) ---------------- */

async fn get_data(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let body = std::fs::read_to_string(user_data_path(&st, &user.id)).unwrap_or_else(|_| "null".into());
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

async fn put_data(State(st): State<Shared>, headers: HeaderMap, body: String) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    if serde_json::from_str::<serde_json::Value>(&body).is_err() {
        return error(StatusCode::BAD_REQUEST, "Expected JSON.");
    }
    let path = user_data_path(&st, &user.id);
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, body).and_then(|_| std::fs::rename(&tmp, &path)).is_err() {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "Couldn't save.");
    }
    StatusCode::NO_CONTENT.into_response()
}

/* ---------------- library ---------------- */

async fn get_library(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = signed_in(&st, &headers) {
        return r;
    }
    let body = {
        let lib = st.lib.read().unwrap();
        serde_json::to_string(&json!({
            "artists": lib.catalog.artists, "albums": lib.catalog.albums, "tracks": lib.catalog.tracks,
            "scanning": lib.scanning
        }))
        .unwrap_or_else(|_| "{}".into())
    };
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

async fn rescan(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = admin(&st, &headers) {
        return r;
    }
    start_scan(st.clone());
    (StatusCode::ACCEPTED, Json(json!({ "scanning": true }))).into_response()
}

/// Audio file with Range support, so the player can seek and browsers can buffer.
async fn stream(State(st): State<Shared>, Path(id): Path<String>, req: Request) -> Response {
    if let Err(r) = signed_in(&st, req.headers()) {
        return r;
    }
    let path = match st.lib.read().unwrap().track_path(&id).cloned() {
        Some(p) => p,
        None => return error(StatusCode::NOT_FOUND, "No such track."),
    };
    match ServeFile::new(path).oneshot(req).await {
        Ok(res) => res.map(Body::new).into_response(),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "Couldn't read the file."),
    }
}

async fn cover(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Err(r) = signed_in(&st, &headers) {
        return r;
    }
    let bytes = st.lib.read().unwrap().cover(&id);
    match bytes {
        Some(b) => {
            let kind = if b.starts_with(&[0x89, b'P', b'N', b'G']) { "image/png" } else if b.starts_with(b"RIFF") { "image/webp" } else { "image/jpeg" };
            ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, "private, max-age=86400")], b).into_response()
        }
        None => error(StatusCode::NOT_FOUND, "No cover."),
    }
}

/* ---------------- scanning ---------------- */

fn start_scan(st: Shared) {
    {
        let mut lib = st.lib.write().unwrap();
        if lib.scanning {
            return;
        }
        lib.scanning = true;
    }
    tokio::task::spawn_blocking(move || {
        let fresh = library::scan(&st.cfg.music_dir);
        let (a, t) = (fresh.catalog.albums.len(), fresh.catalog.tracks.len());
        *st.lib.write().unwrap() = fresh;
        println!("library: {a} albums, {t} tracks in {}", st.cfg.music_dir.display());
    });
}

#[tokio::main]
async fn main() {
    let cfg = Config::from_env();
    for dir in [cfg.data_dir.clone(), cfg.data_dir.join("userdata")] {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("can't create {}: {e}", dir.display());
            std::process::exit(1);
        }
    }
    if !cfg.music_dir.exists() {
        eprintln!("warning: music folder {} doesn't exist yet (set VH_MUSIC_DIR)", cfg.music_dir.display());
    }
    let st: Shared = Arc::new(AppState {
        accounts: Mutex::new(Accounts::load(&cfg.data_dir)),
        lib: RwLock::new(Library::default()),
        cfg: cfg.clone(),
    });
    start_scan(st.clone());
    if cfg.rescan_minutes > 0 {
        let s = st.clone();
        tokio::spawn(async move {
            let mut every = tokio::time::interval(Duration::from_secs(cfg.rescan_minutes * 60));
            every.tick().await; // the first tick fires immediately; the startup scan covers it
            loop {
                every.tick().await;
                start_scan(s.clone());
            }
        });
    }

    let api = Router::new()
        .route("/api/status", get(status))
        .route("/api/setup", post(setup))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/me", get(me))
        .route("/api/me/password", post(change_password))
        .route("/api/me/data", get(get_data).put(put_data))
        .route("/api/users", get(list_users).post(add_user))
        .route("/api/users/{id}", delete(remove_user))
        .route("/api/library", get(get_library))
        .route("/api/library/rescan", post(rescan))
        .route("/api/stream/{id}", get(stream))
        .route("/api/cover/{id}", get(cover))
        .with_state(st.clone());

    // everything else is the web app (single page: unknown paths get index.html)
    let web = ServeDir::new(&cfg.web_dir).fallback(ServeFile::new(cfg.web_dir.join("index.html")));
    let app = api.fallback_service(web);

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("can't listen on {addr}: {e}");
            std::process::exit(1);
        }
    };
    println!("Volvix Home {VERSION} on http://{addr}  (music: {}, data: {}, web: {})",
        cfg.music_dir.display(), cfg.data_dir.display(), cfg.web_dir.display());
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .ok();
}
