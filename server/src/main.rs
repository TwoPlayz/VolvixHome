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
//!   VH_LIDARR_URL       Lidarr address, e.g. http://lidarr:8686 (or set it in the app: Settings → Lidarr)
//!   VH_LIDARR_API_KEY   Lidarr API key

mod artistinfo;
mod auth;
mod cast;
mod devices;
mod library;
mod lidarr;
mod lyrics;
mod maintenance;
mod musicbrainz;
mod party;
mod shares;
mod social;
mod transcode;
mod watch;
mod vault;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
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
    music_dir: RwLock<PathBuf>,
    lib: RwLock<Library>,
    accounts: Mutex<Accounts>,
    lidarr: Mutex<lidarr::Settings>,
    requests: Mutex<Vec<lidarr::Request>>,
    http: lidarr::Client,
    mb: musicbrainz::Client,
    lyrics: lyrics::Client,
    /// recent failed sign-ins by name: (count, first at). Eight in ten minutes locks that name for a while.
    fails: Mutex<HashMap<String, (u32, u64)>>,
    social: social::Social,
    parties: party::Parties,
    devices: devices::Devices,
    cast_key: String,
    transcoder: transcode::Transcoder,
    artists: artistinfo::Client,
    shares: shares::Shares,
    watch: watch::Watch,
    /// a change was noticed while a scan was already running: scan again when it ends
    rescan_pending: std::sync::atomic::AtomicBool,
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

/// The signed-in user, if they're allowed to do perm (admins are allowed everything).
fn require(st: &AppState, headers: &HeaderMap, perm: &str) -> Result<User, Response> {
    let (_, user) = signed_in(st, headers)?;
    if !user.has(perm) {
        return Err(error(StatusCode::FORBIDDEN, "You don't have permission to do that. Ask an admin."));
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
        "scanning": lib.scanning, "albums": lib.catalog.albums.len(), "tracks": lib.catalog.tracks.len(), "watching": st.watch.live(), "transcoding": st.transcoder.available(),
        "rescanMinutes": st.cfg.rescan_minutes
    }))
    .into_response()
}

#[derive(Deserialize)]
struct NewAccount {
    /// optional now: derived from the name
    #[serde(default)]
    username: String,
    #[serde(default)]
    name: String,
    password: String,
    #[serde(default)]
    admin: bool,
    #[serde(default)]
    perms: Option<Vec<String>>,
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
    /// the person's name (the older "username" field still works)
    #[serde(default)]
    name: String,
    #[serde(default)]
    username: String,
    password: String,
}

/// The newest change time (seconds) among the app's own files: the version number of what's on disk.
fn web_build(dir: &std::path::Path) -> u64 {
    fn walk(d: &std::path::Path, depth: u32, best: &mut u64) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() { if depth < 4 { walk(&p, depth + 1, best); } }
            else if let Ok(t) = e.metadata().and_then(|m| m.modified()) {
                if let Ok(s) = t.duration_since(std::time::UNIX_EPOCH) { *best = (*best).max(s.as_secs()); }
            }
        }
    }
    let mut best = 0;
    walk(dir, 0, &mut best);
    best
}

/// index.html with the app's version stamped on its files (styles.css?v=…), so a changed file can never be
/// served from a browser's old copy: the page is re-checked every time and points at fresh addresses.
async fn index_page(State(st): State<Shared>) -> Response {
    let Ok(html) = std::fs::read_to_string(st.cfg.web_dir.join("index.html")) else { return error(StatusCode::NOT_FOUND, "No app here.") };
    let b = web_build(&st.cfg.web_dir).to_string();
    let html = html
        .replace("href=\"css/styles.css\"", &format!("href=\"css/styles.css?v={b}\""))
        .replace("src=\"js/config.js\"", &format!("src=\"js/config.js?v={b}\""))
        .replace("src=\"js/boot.js\"", &format!("src=\"js/boot.js?v={b}\""))
        .replace("<head>", &format!("<head><script>window.VH_BUILD=\"{b}\";</script>"));
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-cache")], html).into_response()
}

/// Basic hardening headers on every response.
async fn security_headers(req: Request, next: axum::middleware::Next) -> Response {
    let is_api = req.uri().path().starts_with("/api/");
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    h.insert("referrer-policy", HeaderValue::from_static("same-origin"));
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    // the app's own files are checked with the server every time, so an update shows up straight away
    if !is_api && !h.contains_key(header::CACHE_CONTROL) { h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache")); }
    res
}

async fn login(State(st): State<Shared>, Json(body): Json<Login>) -> Response {
    let who_key = (if body.name.trim().is_empty() { &body.username } else { &body.name }).trim().to_lowercase();
    {
        let fails = st.fails.lock().unwrap();
        if let Some((n, first)) = fails.get(&who_key) {
            if *n >= 8 && auth::now() < first + 600 {
                return error(StatusCode::TOO_MANY_REQUESTS, "Too many tries. Wait a few minutes and try again.");
            }
        }
    }
    let found = {
        let accounts = st.accounts.lock().unwrap();
        let who = if body.name.trim().is_empty() { &body.username } else { &body.name };
        accounts.find(who).filter(|u| auth::verify_password(&body.password, &u.hash)).cloned()
    };
    match found {
        Some(user) => {
            st.fails.lock().unwrap().remove(&who_key);
            let token = st.accounts.lock().unwrap().start_session(&user.id);
            signed_in_response(&user, &token)
        }
        None => {
            {
                let mut fails = st.fails.lock().unwrap();
                let now = auth::now();
                let e = fails.entry(who_key).or_insert((0, now));
                if now > e.1 + 600 { *e = (0, now); }
                e.0 += 1;
            }
            tokio::time::sleep(Duration::from_millis(400)).await; // slow down guessing
            error(StatusCode::UNAUTHORIZED, "Name or password is incorrect.")
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
        Ok((_, user)) => {
            let l = st.lidarr.lock().unwrap();
            Json(json!({ "user": PublicUser::from(&user), "lidarr": { "configured": l.configured(), "canRequest": l.configured() && can_request(&user, &l) }, "transcoding": st.transcoder.available(), "artistInfo": st.artists.enabled })).into_response()
        }
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

/// A person who can manage members still can't touch an admin, or anyone with more permissions than they have.
fn can_manage(actor: &User, target: &User) -> bool {
    actor.admin || (actor.has("members") && !target.admin && target.effective().iter().all(|p| actor.has(p)))
}

/// Keep only real permission names, and (unless admin) only ones the person granting them has.
fn grantable(actor: &User, wanted: &[String]) -> Result<Vec<String>, String> {
    let list: Vec<String> = auth::PERMS.iter().filter(|p| wanted.iter().any(|w| w == *p)).map(|p| p.to_string()).collect();
    if let Some(p) = list.iter().find(|p| !actor.has(p)) {
        return Err(format!("You can't give someone a permission you don't have ({p})."));
    }
    Ok(list)
}

async fn list_users(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = require(&st, &headers, "members") {
        return r;
    }
    let accounts = st.accounts.lock().unwrap();
    let users: Vec<PublicUser> = accounts.users.iter().map(PublicUser::from).collect();
    Json(json!({ "users": users })).into_response()
}

async fn add_user(State(st): State<Shared>, headers: HeaderMap, Json(body): Json<NewAccount>) -> Response {
    let me = match require(&st, &headers, "members") {
        Ok(u) => u,
        Err(r) => return r,
    };
    // new members get the basics; permissions can be changed afterwards under Manage
    let perms = match body.perms.as_ref().map(|p| grantable(&me, p)).transpose() {
        Ok(p) => p,
        Err(e) => return error(StatusCode::FORBIDDEN, &e),
    };
    if body.admin && !me.admin {
        return error(StatusCode::FORBIDDEN, "Only an admin can make another admin.");
    }
    let mut accounts = st.accounts.lock().unwrap();
    match accounts.create(&body.username, &body.name, &body.password, body.admin) {
        Ok(user) => {
            let user = match perms { Some(p) => { let _ = accounts.set_perms(&user.id, &p); accounts.by_id(&user.id).cloned().unwrap_or(user) } None => user };
            Json(json!({ "user": PublicUser::from(&user) })).into_response()
        }
        Err(e) => error(StatusCode::BAD_REQUEST, &e),
    }
}

async fn remove_user(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let me = match require(&st, &headers, "members") {
        Ok(u) => u,
        Err(r) => return r,
    };
    if me.id == id {
        return error(StatusCode::BAD_REQUEST, "You can't remove yourself.");
    }
    if !st.accounts.lock().unwrap().by_id(&id).map(|t| can_manage(&me, t)).unwrap_or(false) {
        return error(StatusCode::FORBIDDEN, "You can't remove someone with more permissions than you.");
    }
    match st.accounts.lock().unwrap().remove(&id) {
        Ok(()) => {
            let _ = std::fs::remove_file(user_data_path(&st, &id));
            let _ = std::fs::remove_file(avatar_path(&st, &id));
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => error(StatusCode::BAD_REQUEST, &e),
    }
}

#[derive(Deserialize)]
struct UserEdit {
    #[serde(default, rename = "hideExplicit")]
    hide_explicit: Option<bool>,
    #[serde(default)]
    admin: Option<bool>,
    /// the full list of what they may do (replaces the old one)
    #[serde(default)]
    perms: Option<Vec<String>>,
}

/// Someone with the Members permission changes a person: explicit songs on or off, and what they may do.
async fn edit_user(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<UserEdit>) -> Response {
    let me = match require(&st, &headers, "members") {
        Ok(u) => u,
        Err(r) => return r,
    };
    let mut accounts = st.accounts.lock().unwrap();
    let Some(target) = accounts.by_id(&id).cloned() else { return error(StatusCode::NOT_FOUND, "No such person.") };
    if me.id == id && (body.perms.is_some() || body.admin.is_some()) {
        return error(StatusCode::BAD_REQUEST, "You can't change your own permissions.");
    }
    if !can_manage(&me, &target) {
        return error(StatusCode::FORBIDDEN, "You can't change someone with more permissions than you.");
    }
    // dmin (older) means everything / the basics
    let wanted: Option<Vec<String>> = body.perms.clone().or_else(|| body.admin.map(|a| if a { auth::PERMS.iter().map(|p| p.to_string()).collect() } else { auth::DEFAULT_PERMS.iter().map(|p| p.to_string()).collect() }));
    if let Some(w) = wanted {
        match grantable(&me, &w) {
            Ok(list) => { if let Err(e) = accounts.set_perms(&id, &list) { return error(StatusCode::BAD_REQUEST, &e); } }
            Err(e) => return error(StatusCode::FORBIDDEN, &e),
        }
    }
    if let Some(on) = body.hide_explicit {
        if let Err(e) = accounts.set_hide_explicit(&id, on) {
            return error(StatusCode::BAD_REQUEST, &e);
        }
    }
    let user = accounts.by_id(&id).map(PublicUser::from);
    Json(json!({ "user": user })).into_response()
}

#[derive(Deserialize)]
struct NewPassword {
    password: String,
}

/// An admin sets a new password for someone else. They're signed out everywhere and use the new one.
async fn reset_password(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<NewPassword>) -> Response {
    let me = match require(&st, &headers, "members") {
        Ok(u) => u,
        Err(r) => return r,
    };
    if me.id == id {
        return error(StatusCode::BAD_REQUEST, "Change your own password under Profile.");
    }
    if !st.accounts.lock().unwrap().by_id(&id).map(|t| can_manage(&me, t)).unwrap_or(false) {
        return error(StatusCode::FORBIDDEN, "You can't change the password of someone with more permissions than you.");
    }
    match st.accounts.lock().unwrap().set_password(&id, &body.password) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error(StatusCode::BAD_REQUEST, &e),
    }
}

fn avatar_path(st: &AppState, id: &str) -> PathBuf {
    st.cfg.data_dir.join("avatars").join(id)
}

/// Your profile picture. The app shrinks it to a small square first; anything over 1 MB is refused.
async fn put_avatar(State(st): State<Shared>, headers: HeaderMap, body: axum::body::Bytes) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let b = &body[..];
    let is_image = b.starts_with(&[0xFF, 0xD8]) || b.starts_with(&[0x89, b'P', b'N', b'G']) || (b.starts_with(b"RIFF") && b.len() > 12 && &b[8..12] == b"WEBP");
    if !is_image {
        return error(StatusCode::BAD_REQUEST, "That isn't a picture (use a JPEG, PNG or WebP).");
    }
    if b.len() > 1_000_000 {
        return error(StatusCode::PAYLOAD_TOO_LARGE, "That picture is too big.");
    }
    let path = avatar_path(&st, &user.id);
    if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
    if std::fs::write(&path, b).is_err() {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "Couldn't save the picture.");
    }
    let version = auth::now();
    st.accounts.lock().unwrap().set_avatar(&user.id, version);
    Json(json!({ "avatar": version })).into_response()
}

async fn delete_avatar(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let _ = std::fs::remove_file(avatar_path(&st, &user.id));
    st.accounts.lock().unwrap().set_avatar(&user.id, 0);
    StatusCode::NO_CONTENT.into_response()
}

async fn get_avatar(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Err(r) = signed_in(&st, &headers) {
        return r;
    }
    if !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return error(StatusCode::BAD_REQUEST, "Bad id.");
    }
    match std::fs::read(avatar_path(&st, &id)) {
        Ok(b) => {
            let kind = if b.starts_with(&[0x89, b'P', b'N', b'G']) { "image/png" } else if b.starts_with(b"RIFF") { "image/webp" } else { "image/jpeg" };
            ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, "private, max-age=31536000, immutable")], b).into_response()
        }
        Err(_) => error(StatusCode::NOT_FOUND, "No picture."),
    }
}

/* ---------------- per-user app data (playlists, settings, resume point) ---------------- */

async fn get_data(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let body = vault::read(&user_data_path(&st, &user.id)).unwrap_or_else(|| "null".into());
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
    if !vault::write(&path, &body) {
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
    if let Err(r) = require(&st, &headers, "rescan") {
        return r;
    }
    start_scan(st.clone());
    (StatusCode::ACCEPTED, Json(json!({ "scanning": true }))).into_response()
}

/// Audio file with Range support, so the player can seek and browsers can buffer.
/// The ?cast= token on a request, if it's valid: the person it was issued to.
fn cast_user(st: &AppState, uri: &axum::http::Uri) -> Option<String> {
    let q = uri.query()?;
    let token = q.split('&').find_map(|p| p.strip_prefix("cast="))?;
    let uid = cast::verify(&st.cast_key, token, auth::now())?;
    st.accounts.lock().unwrap().by_id(&uid).map(|u| u.id.clone())
}

fn load_cast_key(dir: &std::path::Path) -> String {
    let path = dir.join("cast.key");
    if let Some(k) = vault::read(&path).map(|k| k.trim().to_string()).filter(|k| k.len() >= 32) { return k; }
    let k = auth::random_hex(32);
    vault::write(&path, &k);
    k
}

/// Players on a TV fetch songs from another origin, so answer their CORS questions.
fn with_cors(mut res: Response) -> Response {
    let h = res.headers_mut();
    h.insert("access-control-allow-origin", HeaderValue::from_static("*"));
    h.insert("access-control-expose-headers", HeaderValue::from_static("content-length, content-range, accept-ranges"));
    res
}

async fn cast_preflight() -> Response {
    let mut res = StatusCode::NO_CONTENT.into_response();
    res.headers_mut().insert("access-control-allow-headers", HeaderValue::from_static("range, content-type"));
    res.headers_mut().insert("access-control-allow-methods", HeaderValue::from_static("GET, HEAD, OPTIONS"));
    with_cors(res)
}

/* ---------------- share links ---------------- */

#[derive(Deserialize)]
struct NewShare {
    kind: String,
    #[serde(default)]
    name: String,
    tracks: Vec<String>,
    #[serde(default)]
    days: u64,
}

fn share_json(s: &shares::Share) -> Value {
    json!({ "token": s.token, "kind": s.kind, "name": s.name, "by": s.by_name, "count": s.tracks.len(), "created": s.created, "expires": s.expires, "path": format!("/s/{}", s.token) })
}

async fn share_create(State(st): State<Shared>, headers: HeaderMap, Json(b): Json<NewShare>) -> Response {
    let user = match require(&st, &headers, "share") { Ok(u) => u, Err(r) => return r };
    // only songs that really are on the server
    let known: Vec<String> = {
        let lib = st.lib.read().unwrap();
        b.tracks.into_iter().filter(|id| lib.track_path(id).is_some()).collect()
    };
    match st.shares.create(&b.kind, b.name.trim(), &user.id, &user.name, known, if b.days == 0 { 7 } else { b.days }, auth::now()) {
        Ok(s) => Json(json!({ "share": share_json(&s) })).into_response(),
        Err(e) => error(StatusCode::BAD_REQUEST, &e),
    }
}

async fn share_list(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    let all = user.has("members");
    let list: Vec<Value> = st.shares.mine(&user.id, all, auth::now()).iter().map(share_json).collect();
    Json(json!({ "shares": list })).into_response()
}

async fn share_cancel(State(st): State<Shared>, headers: HeaderMap, Path(token): Path<String>) -> Response {
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    if st.shares.revoke(&token, &user.id, user.has("members")) { StatusCode::NO_CONTENT.into_response() } else { error(StatusCode::NOT_FOUND, "No such link.") }
}

/// The page a share link opens (no sign-in).
async fn share_page(State(st): State<Shared>) -> Response {
    match std::fs::read_to_string(st.cfg.web_dir.join("share.html")) {
        Ok(h) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-cache"), (header::HeaderName::from_static("x-robots-tag"), "noindex, nofollow")], h).into_response(),
        Err(_) => error(StatusCode::NOT_FOUND, "No page."),
    }
}

/// What a share link holds: its name and songs.
async fn share_info(State(st): State<Shared>, Path(token): Path<String>) -> Response {
    let Some(s) = st.shares.get(&token, auth::now()) else { return error(StatusCode::NOT_FOUND, "This link has expired or was cancelled.") };
    let lib = st.lib.read().unwrap();
    let tracks: Vec<Value> = s.tracks.iter().filter_map(|id| {
        let t = lib.catalog.tracks.iter().find(|t| &t.id == id)?;
        let al = lib.catalog.albums.iter().find(|a| a.id == t.album_id);
        Some(json!({ "id": t.id, "title": t.title, "artist": t.artist, "album": al.map(|a| a.title.clone()), "albumId": t.album_id, "duration": t.duration, "cover": al.map(|a| a.cover).unwrap_or(false) }))
    }).collect();
    Json(json!({ "name": s.name, "kind": s.kind, "by": s.by_name, "expires": s.expires, "serverNow": auth::now(), "tracks": tracks })).into_response()
}

async fn share_stream(State(st): State<Shared>, Path((token, id)): Path<(String, String)>, req: Request) -> Response {
    let Some(s) = st.shares.get(&token, auth::now()) else { return error(StatusCode::NOT_FOUND, "This link has expired or was cancelled.") };
    if !s.tracks.iter().any(|t| *t == id) { return error(StatusCode::NOT_FOUND, "Not in this link."); }
    let Some(path) = st.lib.read().unwrap().track_path(&id).cloned() else { return error(StatusCode::NOT_FOUND, "No such track.") };
    match ServeFile::new(path).oneshot(req).await {
        Ok(res) => res.map(Body::new).into_response(),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "Couldn't read the file."),
    }
}

async fn share_cover(State(st): State<Shared>, Path((token, album)): Path<(String, String)>) -> Response {
    let Some(s) = st.shares.get(&token, auth::now()) else { return error(StatusCode::NOT_FOUND, "Expired.") };
    // only albums that one of its songs is on
    let ok = { let lib = st.lib.read().unwrap(); s.tracks.iter().any(|id| lib.catalog.tracks.iter().any(|t| &t.id == id && t.album_id == album)) };
    if !ok { return error(StatusCode::NOT_FOUND, "Not in this link."); }
    match st.lib.read().unwrap().cover(&album) {
        Some(b) => {
            let kind = if b.starts_with(&[0x89, b'P', b'N', b'G']) { "image/png" } else if b.starts_with(b"RIFF") { "image/webp" } else { "image/jpeg" };
            ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, "private, max-age=3600")], b).into_response()
        }
        None => error(StatusCode::NOT_FOUND, "No cover."),
    }
}

/// An artist's bio and photo (from Wikipedia, via MusicBrainz), kept on the server for a month.
async fn artist_info(State(st): State<Shared>, headers: HeaderMap, Query(q): Query<HashMap<String, String>>) -> Response {
    if let Err(r) = signed_in(&st, &headers) {
        return r;
    }
    let name = q.get("name").map(|n| n.trim().to_string()).unwrap_or_default();
    if name.is_empty() || name.chars().count() > 120 {
        return error(StatusCode::BAD_REQUEST, "Which artist?");
    }
    let slug: String = name.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
    let slug = format!("{}-{}", slug.chars().take(40).collect::<String>(), &vault::sha256_hex(&name)[..8]);
    let dir = st.cfg.data_dir.join("cache").join("artists");
    let file = dir.join(format!("{slug}.json"));
    let answer = |v: &Value| Json(json!({ "bio": v["bio"], "image": if v["hasImage"].as_bool() == Some(true) { json!(format!("/api/artist/image/{slug}")) } else { Value::Null }, "source": "Wikipedia" })).into_response();
    if let Some(v) = vault::read(&file).and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        let age = if v["bio"].is_null() { 3 * 86400 } else { 30 * 86400 };
        if v["at"].as_u64().map(|at| at + age > auth::now()).unwrap_or(false) { return answer(&v); }
    }
    match st.artists.find(&st.mb, &name).await {
        Ok(found) => {
            let _ = std::fs::create_dir_all(&dir);
            let mut v = json!({ "at": auth::now(), "bio": Value::Null, "hasImage": false });
            if let Some(info) = found {
                v["bio"] = json!(info.bio);
                if let Some(img) = info.image { if std::fs::write(dir.join(format!("{slug}.img")), img).is_ok() { v["hasImage"] = json!(true); } }
            }
            if st.artists.enabled { vault::write(&file, &v.to_string()); }
            answer(&v)
        }
        Err(e) => error(StatusCode::BAD_GATEWAY, &e),
    }
}

async fn artist_image(State(st): State<Shared>, headers: HeaderMap, Path(slug): Path<String>) -> Response {
    if let Err(r) = signed_in(&st, &headers) {
        return r;
    }
    if !slug.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return error(StatusCode::BAD_REQUEST, "Bad name.");
    }
    match std::fs::read(st.cfg.data_dir.join("cache").join("artists").join(format!("{slug}.img"))) {
        Ok(b) => {
            let kind = if b.starts_with(&[0x89, b'P', b'N', b'G']) { "image/png" } else if b.starts_with(b"RIFF") { "image/webp" } else { "image/jpeg" };
            ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, "private, max-age=2592000")], b).into_response()
        }
        Err(_) => error(StatusCode::NOT_FOUND, "No picture."),
    }
}

/// A short-lived link token so a cast device can fetch songs (see cast.rs).
async fn cast_token(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    let exp = auth::now() + cast::LIFETIME;
    Json(json!({ "token": cast::sign(&st.cast_key, &user.id, exp), "expires": exp })).into_response()
}

async fn stream(State(st): State<Shared>, Path(id): Path<String>, req: Request) -> Response {
    let by_cast = cast_user(&st, req.uri()).is_some();
    if !by_cast {
        if let Err(r) = signed_in(&st, req.headers()) {
            return r;
        }
    }
    let (path, lossless, kbps) = {
        let lib = st.lib.read().unwrap();
        let Some(p) = lib.track_path(&id).cloned() else { return error(StatusCode::NOT_FOUND, "No such track.") };
        let q = lib.catalog.tracks.iter().find(|t| t.id == id).and_then(|t| lib.catalog.albums.iter().find(|a| a.id == t.album_id)).map(|a| (a.quality.lossless, a.quality.kbps));
        (p, q.map(|x| x.0).unwrap_or(true), q.and_then(|x| x.1))
    };
    // ?q=high|normal|low: a smaller copy for a phone (made once with ffmpeg, then kept)
    let path = match req.uri().query().and_then(|q| q.split('&').find_map(|p| p.strip_prefix("q="))).and_then(|q| transcode::kbps_of(q).map(|k| (q.to_string(), k))) {
        Some((q, target)) if st.transcoder.available() && !transcode::already_small(lossless, kbps, target) => {
            match st.transcoder.ensure(&path, &id, &q).await { Ok(p) => p, Err(_) => path } // if converting fails, send the original
        }
        _ => path,
    };
    match ServeFile::new(path).oneshot(req).await {
        Ok(res) => { let r = res.map(Body::new).into_response(); if by_cast { with_cors(r) } else { r } }
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "Couldn't read the file."),
    }
}

async fn cover(State(st): State<Shared>, headers: HeaderMap, uri: axum::http::Uri, Path(id): Path<String>) -> Response {
    let by_cast = cast_user(&st, &uri).is_some();
    if !by_cast {
        if let Err(r) = signed_in(&st, &headers) {
            return r;
        }
    }
    let bytes = st.lib.read().unwrap().cover(&id);
    match bytes {
        Some(b) => {
            let kind = if b.starts_with(&[0x89, b'P', b'N', b'G']) { "image/png" } else if b.starts_with(b"RIFF") { "image/webp" } else { "image/jpeg" };
            let r = ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, "private, max-age=86400")], b).into_response();
            if by_cast { with_cors(r) } else { r }
        }
        None => error(StatusCode::NOT_FOUND, "No cover."),
    }
}

/* ---------------- Lidarr ---------------- */

fn can_request(user: &User, l: &lidarr::Settings) -> bool {
    let _ = l; // (the old household-wide switch is gone: it's the Request permission now)
    user.has("request")
}

/// letters and digits only, lowercase: "The Velvet Room (Deluxe)" ~ "thevelvetroomdeluxe"
fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// Is this album (or artist) already in the library?
fn on_server(st: &AppState, title: Option<&str>, artist: &str) -> Option<String> {
    let lib = st.lib.read().unwrap();
    let a = norm(artist);
    match title {
        Some(t) => {
            let t = norm(t);
            lib.catalog.albums.iter().find(|x| norm(&x.artist) == a && (norm(&x.title) == t || norm(&x.title).starts_with(&t))).map(|x| x.id.clone())
        }
        None => lib.catalog.artists.iter().find(|x| norm(&x.name) == a).map(|x| x.name.clone()),
    }
}

/// An artist from MusicBrainz and what they've released, each marked: already here, or already asked for.
async fn discover_artist(State(st): State<Shared>, headers: HeaderMap, Path(mbid): Path<String>) -> Response {
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    if !mbid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') { return error(StatusCode::BAD_REQUEST, "Bad id."); }
    let mut v = match st.mb.artist_page(&mbid).await { Ok(v) => v, Err(e) => return error(StatusCode::BAD_GATEWAY, &e) };
    let s = st.lidarr.lock().unwrap().clone();
    let requested: Vec<String> = st.requests.lock().unwrap().iter().map(|r| r.foreign_id.clone()).collect();
    let name = v["name"].as_str().unwrap_or("").to_string();
    v["onServer"] = json!(on_server(&st, None, &name));
    v["requested"] = json!(requested.iter().any(|r| *r == mbid));
    if let Some(list) = v["albums"].as_array_mut() {
        for a in list.iter_mut() {
            let title = a["title"].as_str().unwrap_or("").to_string();
            a["onServer"] = json!(on_server(&st, Some(&title), &name));
            a["requested"] = json!(a["foreignId"].as_str().map(|id| requested.iter().any(|r| r == id)).unwrap_or(false));
        }
    }
    v["canRequest"] = json!(s.configured() && can_request(&user, &s));
    v["configured"] = json!(s.configured());
    Json(v).into_response()
}

/// One album from MusicBrainz with its songs.
async fn discover_album(State(st): State<Shared>, headers: HeaderMap, Path(rgid): Path<String>) -> Response {
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    if !rgid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') { return error(StatusCode::BAD_REQUEST, "Bad id."); }
    let mut v = match st.mb.album_page(&rgid).await { Ok(v) => v, Err(e) => return error(StatusCode::BAD_GATEWAY, &e) };
    let s = st.lidarr.lock().unwrap().clone();
    let requested: Vec<String> = st.requests.lock().unwrap().iter().map(|r| r.foreign_id.clone()).collect();
    let (title, artist) = (v["title"].as_str().unwrap_or("").to_string(), v["artist"].as_str().unwrap_or("").to_string());
    v["onServer"] = json!(on_server(&st, Some(&title), &artist));
    v["requested"] = json!(requested.iter().any(|r| *r == rgid));
    v["canRequest"] = json!(s.configured() && can_request(&user, &s));
    v["configured"] = json!(s.configured());
    Json(v).into_response()
}

/// Is a song (by title + artist) already in the library? Returns its album id.
fn track_on_server(st: &AppState, title: &str, artist: &str) -> Option<String> {
    let lib = st.lib.read().unwrap();
    let (t, a) = (norm(title), norm(artist));
    let track = lib.catalog.tracks.iter().find(|x| norm(&x.title) == t && norm(&x.artist) == a)?;
    Some(track.album_id.clone())
}

async fn lidarr_status(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let s = st.lidarr.lock().unwrap().clone();
    let mut out = json!({ "configured": s.configured(), "canRequest": s.configured() && can_request(&user, &s) });
    if user.has("settings") {
        out["settings"] = json!({
            "url": s.url, "apiKeySet": !s.api_key.is_empty(), "rootFolder": s.root_folder,
            "qualityProfileId": s.quality_profile_id, "metadataProfileId": s.metadata_profile_id,
            "everyoneCanRequest": s.everyone_can_request,
            "hookPath": format!("/api/hooks/lidarr?token={}", s.hook_token)
        });
        if s.configured() {
            match st.http.overview(&s).await {
                Ok(o) => out["lidarr"] = o,
                Err(e) => out["error"] = json!(e),
            }
        }
    }
    Json(out).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LidarrConfig {
    url: String,
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    root_folder: String,
    #[serde(default)]
    quality_profile_id: i64,
    #[serde(default)]
    metadata_profile_id: i64,
    #[serde(default = "default_true")]
    everyone_can_request: bool,
}
fn default_true() -> bool {
    true
}

/// Save Lidarr's address and key, but only after Lidarr answers with them.
async fn lidarr_config(State(st): State<Shared>, headers: HeaderMap, Json(body): Json<LidarrConfig>) -> Response {
    if let Err(r) = require(&st, &headers, "settings") {
        return r;
    }
    let mut s = st.lidarr.lock().unwrap().clone();
    let mut url = body.url.trim().trim_end_matches('/').to_string();
    if url.is_empty() {
        // turning it off
        s.url.clear();
        s.api_key.clear();
        s.save(&st.cfg.data_dir);
        *st.lidarr.lock().unwrap() = s;
        return lidarr_status(State(st), headers).await;
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        url = format!("http://{url}");
    }
    s.url = url;
    if !body.api_key.trim().is_empty() {
        s.api_key = body.api_key.trim().to_string();
    }
    s.root_folder = body.root_folder;
    s.quality_profile_id = body.quality_profile_id;
    s.metadata_profile_id = body.metadata_profile_id;
    s.everyone_can_request = body.everyone_can_request;
    let s = match st.http.defaults(&s).await {
        Ok(s) => s,
        Err(e) => return error(StatusCode::BAD_REQUEST, &e),
    };
    s.save(&st.cfg.data_dir);
    *st.lidarr.lock().unwrap() = s;
    lidarr_status(State(st), headers).await
}

async fn lidarr_search(State(st): State<Shared>, headers: HeaderMap, Query(q): Query<HashMap<String, String>>) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    // Searching only needs MusicBrainz, so everyone can look. Getting it needs Lidarr (and permission).
    let s = st.lidarr.lock().unwrap().clone();
    let (configured, can) = (s.configured(), s.configured() && can_request(&user, &s));
    let term = q.get("term").map(|t| t.trim().to_string()).unwrap_or_default();
    if term.chars().count() < 2 {
        return Json(json!({ "artists": [], "albums": [], "songs": [], "canRequest": can, "configured": configured })).into_response();
    }
    // Searching is MusicBrainz (the database Lidarr uses); requesting hands the ids to Lidarr.
    let found = match st.mb.search(&term).await {
        Ok(r) => r,
        Err(e) => return error(StatusCode::BAD_GATEWAY, &e),
    };
    let requested: Vec<String> = st.requests.lock().unwrap().iter().map(|r| r.foreign_id.clone()).collect();
    let is_requested = |v: &Value| -> bool {
        ["foreignId", "albumId", "singleId"].iter().any(|k| v[k].as_str().map(|id| requested.iter().any(|r| r == id)).unwrap_or(false))
    };
    let artists: Vec<Value> = found.artists.iter().map(|a| {
        let name = a["name"].as_str().unwrap_or("").to_string();
        let mut v = a.clone();
        v["onServer"] = json!(on_server(&st, None, &name));
        v["requested"] = json!(is_requested(a));
        v
    }).collect();
    let albums: Vec<Value> = found.albums.iter().map(|a| {
        let title = a["title"].as_str().map(String::from);
        let artist = a["artist"].as_str().unwrap_or("").to_string();
        let mut v = a.clone();
        v["onServer"] = json!(on_server(&st, title.as_deref(), &artist));
        v["requested"] = json!(is_requested(a));
        v
    }).collect();
    let songs: Vec<Value> = found.songs.iter().map(|sg| {
        let title = sg["title"].as_str().unwrap_or("").to_string();
        let artist = sg["artist"].as_str().unwrap_or("").to_string();
        let mut v = sg.clone();
        v["onServer"] = json!(track_on_server(&st, &title, &artist));
        v["requested"] = json!(is_requested(sg));
        v
    }).collect();
    Json(json!({ "songs": songs, "artists": artists, "albums": albums, "canRequest": can, "configured": configured })).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WantIt {
    kind: String,
    foreign_id: String,
}

/// Ask Lidarr to get an album (or everything by an artist) and remember who asked.
async fn lidarr_request(State(st): State<Shared>, headers: HeaderMap, Json(body): Json<WantIt>) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let s = st.lidarr.lock().unwrap().clone();
    if !can_request(&user, &s) {
        return error(StatusCode::FORBIDDEN, "Ask an admin to get music for you.");
    }
    let s = match st.http.defaults(&s).await {
        Ok(s) => s,
        Err(e) => return error(StatusCode::BAD_GATEWAY, &e),
    };
    let id = body.foreign_id.trim();
    let kind = body.kind.as_str();
    let found = match kind {
        "album" => st.http.get(&s, &format!("album/lookup?term=lidarr:{id}")).await,
        "artist" => st.http.get(&s, &format!("artist/lookup?term=lidarr:{id}")).await,
        _ => return error(StatusCode::BAD_REQUEST, "kind must be album or artist"),
    };
    let item = match found.map(|v| v.as_array().and_then(|a| a.first().cloned())) {
        Ok(Some(v)) => v,
        Ok(None) => return error(StatusCode::NOT_FOUND, "Lidarr couldn't find that any more."),
        Err(e) => return error(StatusCode::BAD_GATEWAY, &e),
    };

    let result = if kind == "album" {
        if lidarr::in_lidarr(&item) {
            // already known to Lidarr: make sure it's wanted, then search for it now
            let album_id = item["id"].as_i64().unwrap_or(0);
            match st.http.put(&s, "album/monitor", &json!({ "albumIds": [album_id], "monitored": true })).await {
                Ok(_) => st.http.post(&s, "command", &json!({ "name": "AlbumSearch", "albumIds": [album_id] })).await,
                Err(e) => Err(e),
            }
        } else {
            let mut album = item.clone();
            album["monitored"] = json!(true);
            album["addOptions"] = json!({ "searchForNewAlbum": true });
            // the artist is added too if Lidarr doesn't have them yet, but only this album is wanted
            album["artist"] = lidarr::artist_body(&item["artist"], &s, false);
            st.http.post(&s, "album", &album).await
        }
    } else if lidarr::in_lidarr(&item) {
        st.http.post(&s, "command", &json!({ "name": "ArtistSearch", "artistId": item["id"] })).await
    } else {
        st.http.post(&s, "artist", &lidarr::artist_body(&item, &s, true)).await
    };
    if let Err(e) = result {
        return error(StatusCode::BAD_GATEWAY, &e);
    }

    let req = lidarr::Request {
        kind: kind.to_string(),
        foreign_id: id.to_string(),
        title: if kind == "album" { item["title"].as_str() } else { item["artistName"].as_str() }.unwrap_or("").to_string(),
        artist: if kind == "album" { item["artist"]["artistName"].as_str() } else { item["artistName"].as_str() }.unwrap_or("").to_string(),
        image: if kind == "album" { lidarr::image(&item, &["cover"]) } else { lidarr::image(&item, &["poster", "fanart"]) },
        year: lidarr::year(&item),
        by: user.name.clone(),
        by_id: user.id.clone(),
        at: auth::now(),
        notified: false,
    };
    {
        let mut list = st.requests.lock().unwrap();
        list.retain(|r| r.foreign_id != req.foreign_id);
        list.insert(0, req.clone());
        list.truncate(300);
        lidarr::save_requests(&st.cfg.data_dir, &list);
    }
    Json(json!({ "request": req })).into_response()
}

/// What Lidarr is downloading right now.
async fn lidarr_queue(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = signed_in(&st, &headers) {
        return r;
    }
    let s = st.lidarr.lock().unwrap().clone();
    if !s.configured() {
        return Json(json!({ "items": [] })).into_response();
    }
    let q = match st.http.get(&s, "queue?page=1&pageSize=100&includeArtist=true&includeAlbum=true").await {
        Ok(q) => q,
        Err(e) => return error(StatusCode::BAD_GATEWAY, &e),
    };
    let records = q["records"].as_array().cloned().unwrap_or_default();
    // Lidarr lists one record per release; fold them into one row per album
    let mut items: Vec<Value> = Vec::new();
    for r in records {
        let album_id = r["album"]["foreignAlbumId"].as_str().unwrap_or("").to_string();
        let size = r["size"].as_f64().unwrap_or(0.0);
        let left = r["sizeleft"].as_f64().unwrap_or(0.0);
        let progress = if size > 0.0 { ((size - left) / size).clamp(0.0, 1.0) } else { 0.0 };
        let state = r["trackedDownloadState"].as_str().or_else(|| r["status"].as_str()).unwrap_or("queued").to_string();
        let message = r["statusMessages"].as_array().and_then(|m| m.first()).and_then(|m| m["messages"].get(0)).and_then(|m| m.as_str())
            .or_else(|| r["errorMessage"].as_str()).map(String::from);
        if let Some(existing) = items.iter_mut().find(|i| i["foreignAlbumId"] == album_id && !album_id.is_empty()) {
            existing["progress"] = json!((existing["progress"].as_f64().unwrap_or(0.0) + progress) / 2.0);
            continue;
        }
        items.push(json!({
            "foreignAlbumId": album_id,
            "title": r["album"]["title"].as_str().or_else(|| r["title"].as_str()).unwrap_or(""),
            "artist": r["artist"]["artistName"].as_str().unwrap_or(""),
            "image": lidarr::image(&r["album"], &["cover"]),
            "progress": progress, "timeLeft": r["timeleft"], "state": state,
            "status": r["trackedDownloadStatus"].as_str().unwrap_or("ok"), "message": message
        }));
    }
    Json(json!({ "items": items })).into_response()
}

/// Everything that's been requested, with where it stands.
async fn lidarr_requests(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = signed_in(&st, &headers) {
        return r;
    }
    let list = st.requests.lock().unwrap().clone();
    let out: Vec<Value> = list.iter().map(|r| {
        let here = if r.kind == "album" { on_server(&st, Some(&r.title), &r.artist) } else { on_server(&st, None, &r.artist) };
        let mut v = serde_json::to_value(r).unwrap_or(Value::Null);
        v["onServer"] = json!(here);
        v
    }).collect();
    Json(json!({ "requests": out })).into_response()
}

/// Lidarr → Settings → Connect → Webhook: on import, look for the new music right away.
async fn lidarr_hook(State(st): State<Shared>, Query(q): Query<HashMap<String, String>>) -> Response {
    let token = st.lidarr.lock().unwrap().hook_token.clone();
    let given = q.get("token").cloned().unwrap_or_default();
    // compare without stopping at the first difference
    let same = token.len() == given.len() && token.bytes().zip(given.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0;
    if token.is_empty() || !same {
        return error(StatusCode::UNAUTHORIZED, "Bad token.");
    }
    // Lidarr may send several events for one import; a short delay folds them into one scan
    let s = st.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        start_scan(s);
    });
    StatusCode::NO_CONTENT.into_response()
}

/* ---------------- household: people, presence, notifications, shared playlists ---------------- */

fn names_map(st: &AppState) -> HashMap<String, String> {
    st.accounts.lock().unwrap().users.iter().map(|u| (u.id.clone(), u.name.clone())).collect()
}

fn track_json(st: &AppState, id: &str) -> Option<Value> {
    let lib = st.lib.read().unwrap();
    lib.catalog.tracks.iter().find(|t| t.id == id).map(|t| json!({ "trackId": t.id, "title": t.title, "artist": t.artist, "albumId": t.album_id }))
}

/// Credits for a song from MusicBrainz (the file's own tags are already in the library). Cached on disk:
/// MusicBrainz allows one request a second, and credits rarely change.
async fn track_credits(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Err(r) = signed_in(&st, &headers) {
        return r;
    }
    if !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return error(StatusCode::BAD_REQUEST, "Bad id.");
    }
    let file = st.cfg.data_dir.join("cache").join("credits").join(format!("{id}.json"));
    if let Some(v) = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        let fresh = v["at"].as_u64().map(|at| at + 30 * 86400 > auth::now()).unwrap_or(false);
        if fresh { return Json(json!({ "credits": v["credits"] })).into_response(); }
    }
    let (title, artist) = match track_json(&st, &id) {
        Some(t) => (t["title"].as_str().unwrap_or("").to_string(), t["artist"].as_str().unwrap_or("").to_string()),
        None => return error(StatusCode::NOT_FOUND, "No such track."),
    };
    match st.mb.credits(&title, &artist).await {
        Ok(list) => {
            let credits: Vec<Value> = list.iter().map(|(r, n)| json!({ "role": r, "names": n })).collect();
            if let Some(dir) = file.parent() { let _ = std::fs::create_dir_all(dir); }
            let _ = std::fs::write(&file, json!({ "at": auth::now(), "credits": credits }).to_string());
            Json(json!({ "credits": credits })).into_response()
        }
        Err(e) => error(StatusCode::BAD_GATEWAY, &e),
    }
}

/// Lyrics for a song: next to the file, inside it, or from LRCLIB. What LRCLIB says is cached on disk
/// (found for 90 days, "not found" for a day, so a later upload there gets picked up).
async fn track_lyrics(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Err(r) = signed_in(&st, &headers) {
        return r;
    }
    if !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return error(StatusCode::BAD_REQUEST, "Bad id.");
    }
    let (path, title, artist, album, duration) = {
        let lib = st.lib.read().unwrap();
        let Some(t) = lib.catalog.tracks.iter().find(|t| t.id == id) else { return error(StatusCode::NOT_FOUND, "No such track.") };
        let album = lib.catalog.albums.iter().find(|a| a.id == t.album_id).map(|a| a.title.clone()).unwrap_or_default();
        (lib.track_path(&id).cloned(), t.title.clone(), t.artist.clone(), album, t.duration)
    };
    // 1 + 2: the files themselves (read off the async threads)
    if let Some(p) = path {
        if let Ok(Some(l)) = tokio::task::spawn_blocking(move || lyrics::from_files(&p)).await {
            return Json(json!({ "lyrics": l.to_json(), "online": st.lyrics.online() })).into_response();
        }
    }
    // 3: LRCLIB, through the cache
    let file = st.cfg.data_dir.join("cache").join("lyrics").join(format!("{id}.json"));
    if let Some(v) = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        let found = !v["lyrics"].is_null();
        let age = if found { 90 * 86400 } else { 86400 };
        if v["at"].as_u64().map(|at| at + age > auth::now()).unwrap_or(false) {
            return Json(json!({ "lyrics": v["lyrics"], "online": st.lyrics.online() })).into_response();
        }
    }
    match st.lyrics.find(&title, &artist, &album, duration).await {
        Ok(found) => {
            let value = found.map(|l| l.to_json()).unwrap_or(Value::Null);
            if st.lyrics.online() {
                if let Some(dir) = file.parent() { let _ = std::fs::create_dir_all(dir); }
                let _ = std::fs::write(&file, json!({ "at": auth::now(), "lyrics": value }).to_string());
            }
            Json(json!({ "lyrics": value, "online": st.lyrics.online() })).into_response()
        }
        Err(e) => error(StatusCode::BAD_GATEWAY, &e),
    }
}

async fn people(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = signed_in(&st, &headers) {
        return r;
    }
    let accounts = st.accounts.lock().unwrap();
    let list: Vec<Value> = accounts.users.iter().map(|u| json!({ "id": u.id, "name": u.name, "admin": u.admin, "avatar": u.avatar })).collect();
    Json(json!({ "people": list })).into_response()
}

#[derive(Deserialize)]
struct NowBody {
    #[serde(default, rename = "trackId")]
    track_id: Option<String>,
    #[serde(default)]
    playing: bool,
}

/// What I'm playing, for the Household page. Sending no track stops sharing (private session, or turned off).
async fn put_now(State(st): State<Shared>, headers: HeaderMap, Json(body): Json<NowBody>) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    match body.track_id.filter(|t| track_json(&st, t).is_some()) {
        Some(track_id) => st.social.set_presence(&user.id, Some(social::Presence { track_id, playing: body.playing, at: auth::now() })),
        None => st.social.set_presence(&user.id, None),
    }
    StatusCode::NO_CONTENT.into_response()
}

/// Everyone else in the household: what they're listening to, and their public playlists.
async fn household(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let (_, me) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let users: Vec<(String, String)> = st.accounts.lock().unwrap().users.iter().map(|u| (u.id.clone(), u.name.clone())).collect();
    let members: Vec<Value> = users.iter().filter(|(id, _)| *id != me.id).map(|(id, name)| {
        let now = st.social.presence_of(id).and_then(|(track_id, live, at)| {
            track_json(&st, &track_id).map(|mut t| { t["listening"] = json!(live); t["at"] = json!(at); t })
        });
        json!({ "id": id, "name": name, "now": now, "playlists": st.social.public_playlists(id) })
    }).collect();
    Json(json!({ "members": members, "serverNow": auth::now() })).into_response()
}

async fn notifications(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let items = st.social.notes_for(&user.id);
    let unread = items.iter().filter(|n| !n.read).count();
    Json(json!({ "items": items, "unread": unread })).into_response()
}

#[derive(Deserialize)]
struct ReadBody {
    #[serde(default)]
    ids: Option<Vec<String>>,
}

async fn notifications_read(State(st): State<Shared>, headers: HeaderMap, Json(body): Json<ReadBody>) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    st.social.mark_read(&user.id, body.ids.as_deref());
    StatusCode::NO_CONTENT.into_response()
}

async fn shared_list(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let names = names_map(&st);
    let (mine, invites) = st.social.list_for(&user.id);
    let invites: Vec<Value> = invites.iter().map(|s| json!({
        "id": s.id, "name": s.name, "owner": s.owner, "ownerName": names.get(&s.owner).cloned().unwrap_or_default(), "count": s.tracks.len()
    })).collect();
    Json(json!({ "playlists": mine, "invites": invites })).into_response()
}

#[derive(Deserialize)]
struct SharedNew {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    tracks: Vec<String>,
    #[serde(default)]
    public: bool,
}

/// Turn a playlist into a shared one (you're the owner).
async fn shared_create(State(st): State<Shared>, headers: HeaderMap, Json(body): Json<SharedNew>) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let known: std::collections::HashSet<String> = st.lib.read().unwrap().catalog.tracks.iter().map(|t| t.id.clone()).collect();
    let tracks: Vec<String> = body.tracks.into_iter().filter(|t| known.contains(t)).collect();
    let pl = st.social.create(&user.id, &body.name, &body.description, tracks, body.public);
    Json(json!({ "playlist": pl })).into_response()
}

async fn shared_op(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<Value>) -> Response {
    let (_, user) = match signed_in(&st, &headers) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let names = names_map(&st);
    match st.social.op(&user.id, &id, &body, &names) {
        Ok(r) => Json(json!({ "playlist": r.playlist, "gone": r.gone })).into_response(),
        Err((code, msg)) => error(StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_REQUEST), &msg),
    }
}

/// After a scan: tell people whose Lidarr request has arrived in the library.
fn notify_ready(st: &AppState) {
    let mut reqs = st.requests.lock().unwrap();
    let mut changed = false;
    for r in reqs.iter_mut().filter(|r| !r.notified) {
        let here = if r.kind == "album" { on_server(st, Some(&r.title), &r.artist) } else { on_server(st, None, &r.artist) };
        if let Some(found) = here {
            r.notified = true;
            changed = true;
            let text = if r.kind == "album" { format!("{} by {} is in your library", r.title, r.artist) } else { format!("{} is now in your library", r.artist) };
            st.social.add_note(&r.by_id, "request_ready", text, json!({ "albumId": found, "kind": r.kind }));
        }
    }
    if changed {
        lidarr::save_requests(&st.cfg.data_dir, &reqs);
    }
}

/* ---------------- listening parties ---------------- */

fn track_known(st: &AppState) -> impl Fn(&str) -> bool + '_ {
    move |id: &str| st.lib.read().unwrap().track_path(id).is_some()
}
fn track_dur_ms(st: &AppState) -> impl Fn(&str) -> i64 + '_ {
    move |id: &str| st.lib.read().unwrap().catalog.tracks.iter().find(|t| t.id == id).map(|t| t.duration as i64 * 1000).unwrap_or(0)
}
fn party_fail(f: party::Fail) -> Response {
    error(StatusCode::from_u16(f.0).unwrap_or(StatusCode::BAD_REQUEST), &f.1)
}
fn host_name(st: &AppState, uid: &str) -> String {
    st.accounts.lock().unwrap().by_id(uid).map(|u| u.name.clone()).unwrap_or_else(|| "Someone".into())
}
/// Tell people they're invited (a notification with a Join button).
fn send_party_invites(st: &AppState, party_id: &str, from: &str, to: &[String]) {
    let name = host_name(st, from);
    for u in to {
        st.social.add_note(u, "party_invite", format!("{name} invited you to a listening party"), json!({ "partyId": party_id, "from": name, "fromId": from }));
    }
}
/// A party ended: its pending invites are moot.
fn close_party_invites(st: &AppState, party_id: &str, invited: &[String]) {
    for u in invited {
        st.social.set_done(u, "party_invite", "partyId", party_id, "over");
    }
}

async fn party_mine(State(st): State<Shared>, headers: HeaderMap) -> Response {
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    Json(json!({ "party": st.parties.of(&user.id) })).into_response()
}

#[derive(Deserialize)]
struct PartyNew {
    #[serde(default)]
    invite: Vec<String>,
    #[serde(default)]
    queue: Vec<String>,
    #[serde(default)]
    index: i64,
    #[serde(default, rename = "posMs")]
    pos_ms: i64,
    #[serde(default)]
    playing: bool,
}

async fn party_create(State(st): State<Shared>, headers: HeaderMap, Json(b): Json<PartyNew>) -> Response {
    let user = match require(&st, &headers, "parties") { Ok(u) => u, Err(r) => return r };
    let names = names_map(&st);
    let invite: Vec<String> = b.invite.into_iter().filter(|u| names.contains_key(u)).collect();
    let known = track_known(&st);
    let queue: Vec<String> = b.queue.into_iter().filter(|t| known(t)).collect();
    match st.parties.create(&user.id, &invite, queue, b.index, b.pos_ms, b.playing) {
        Ok(state) => {
            let id = state["id"].as_str().unwrap_or("").to_string();
            let invited: Vec<String> = state["invited"].as_array().cloned().unwrap_or_default().iter().filter_map(|v| v.as_str().map(String::from)).collect();
            send_party_invites(&st, &id, &user.id, &invited);
            Json(json!({ "party": state })).into_response()
        }
        Err(f) => party_fail(f),
    }
}

#[derive(Deserialize)]
struct PartyJoin {
    #[serde(default)]
    output: String,
}

async fn party_join(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>, Json(b): Json<PartyJoin>) -> Response {
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    match st.parties.join(&user.id, &id, &b.output) {
        Ok(state) => {
            st.social.set_done(&user.id, "party_invite", "partyId", &id, "joined");
            Json(json!({ "party": state })).into_response()
        }
        Err(f) => party_fail(f),
    }
}

async fn party_leave(State(st): State<Shared>, headers: HeaderMap, Path(_id): Path<String>) -> Response {
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    if let Some(party::Left::Ended { id, invited }) = st.parties.leave(&user.id) {
        close_party_invites(&st, &id, &invited);
    }
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
struct PartyInvite {
    #[serde(default, rename = "userIds")]
    user_ids: Vec<String>,
}

async fn party_invite(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>, Json(b): Json<PartyInvite>) -> Response {
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    let names = names_map(&st);
    match st.parties.invite(&user.id, &id, &b.user_ids, &|u| names.contains_key(u)) {
        Ok((state, fresh)) => {
            send_party_invites(&st, &id, &user.id, &fresh);
            Json(json!({ "party": state })).into_response()
        }
        Err(f) => party_fail(f),
    }
}

async fn party_output(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>, Json(b): Json<PartyJoin>) -> Response {
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    match st.parties.set_output(&user.id, &id, &b.output) {
        Ok(state) => Json(json!({ "party": state })).into_response(),
        Err(f) => party_fail(f),
    }
}

async fn party_action(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<Value>) -> Response {
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    let (known, dur) = (track_known(&st), track_dur_ms(&st));
    match st.parties.action(&user.id, &id, &body, &known, &dur) {
        Ok(state) => Json(json!({ "party": state })).into_response(),
        Err(f) => party_fail(f),
    }
}

/// Live updates for a party (Server-Sent Events): the current state, then every change.
async fn party_events(State(st): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    use futures_util::StreamExt;
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    let (rx, first) = match st.parties.subscribe(&user.id, &id) { Ok(x) => x, Err(f) => return party_fail(f) };
    let first = json!({ "type": "state", "state": first }).to_string();
    let live = tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|m| async move { m.ok() }); // a lagging reader just skips ahead
    let stream = futures_util::stream::once(async move { first }).chain(live)
        .map(|data| Ok::<_, std::convert::Infallible>(axum::response::sse::Event::default().data(data)));
    let mut res = axum::response::Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response();
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    res.headers_mut().insert("X-Accel-Buffering", HeaderValue::from_static("no")); // reverse proxies shouldn't hold events back
    res
}

/* ---------------- your devices ---------------- */

#[derive(Deserialize)]
struct DeviceQuery {
    device: String,
    #[serde(default)]
    name: String,
}

/// Dropped when the connection closes, which is how a device goes offline.
struct DeviceGuard {
    st: Shared,
    uid: String,
    dev: String,
}
impl Drop for DeviceGuard {
    fn drop(&mut self) {
        self.st.devices.disconnect(&self.uid, &self.dev);
    }
}

fn valid_device(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

async fn device_events(State(st): State<Shared>, headers: HeaderMap, Query(q): Query<DeviceQuery>) -> Response {
    use futures_util::StreamExt;
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    if !valid_device(&q.device) {
        return error(StatusCode::BAD_REQUEST, "Bad device id.");
    }
    let (rx, first) = st.devices.connect(&user.id, &q.device, &q.name);
    let guard = DeviceGuard { st: st.clone(), uid: user.id.clone(), dev: q.device.clone() };
    let live = tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|m| async move { m.ok() });
    let stream = futures_util::stream::once(async move { first }).chain(live)
        .map(move |data| { let _keep = &guard; Ok::<_, std::convert::Infallible>(axum::response::sse::Event::default().data(data)) });
    let mut res = axum::response::Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response();
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    res.headers_mut().insert("X-Accel-Buffering", HeaderValue::from_static("no"));
    res
}

#[derive(Deserialize)]
struct DeviceBody {
    device: String,
    #[serde(default)]
    to: String,
    #[serde(default)]
    state: Value,
}

async fn device_active(State(st): State<Shared>, headers: HeaderMap, Json(b): Json<DeviceBody>) -> Response {
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    if st.devices.set_active(&user.id, &b.device) { StatusCode::NO_CONTENT.into_response() } else { error(StatusCode::NOT_FOUND, "This device isn't connected.") }
}

async fn device_transfer(State(st): State<Shared>, headers: HeaderMap, Json(b): Json<DeviceBody>) -> Response {
    let (_, user) = match signed_in(&st, &headers) { Ok(u) => u, Err(r) => return r };
    match st.devices.transfer(&user.id, &b.device, &b.to, b.state) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error(StatusCode::NOT_FOUND, &e),
    }
}

/* ---------------- music folder ---------------- */

fn music_dir_file(cfg: &Config) -> PathBuf {
    cfg.data_dir.join("library.json")
}

/// The folder an admin picked in Settings, else VH_MUSIC_DIR, else /music.
fn load_music_dir(cfg: &Config) -> PathBuf {
    std::fs::read_to_string(music_dir_file(cfg)).ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["musicDir"].as_str().map(PathBuf::from))
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| cfg.music_dir.clone())
}

fn count_audio_here(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir).map(|rd| rd.filter_map(|e| e.ok()).filter(|e| {
        e.path().extension().and_then(|x| x.to_str()).map(|x| library::AUDIO.contains(&x.to_lowercase().as_str())).unwrap_or(false)
    }).count()).unwrap_or(0)
}

async fn library_folder(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = require(&st, &headers, "settings") {
        return r;
    }
    let path = st.music_dir.read().unwrap().clone();
    Json(json!({
        "path": path.display().to_string(), "exists": path.is_dir(),
        "default": st.cfg.music_dir.display().to_string(),
        "isDefault": path == st.cfg.music_dir
    })).into_response()
}

#[derive(Deserialize)]
struct FolderBody {
    #[serde(default)]
    path: String,
}

/// Choose the music folder (empty = back to the default), then look for music in it.
async fn set_library_folder(State(st): State<Shared>, headers: HeaderMap, Json(body): Json<FolderBody>) -> Response {
    if let Err(r) = require(&st, &headers, "settings") {
        return r;
    }
    let chosen = body.path.trim();
    let path = if chosen.is_empty() { st.cfg.music_dir.clone() } else { PathBuf::from(chosen) };
    if !path.is_dir() {
        return error(StatusCode::BAD_REQUEST, "That folder doesn't exist on the server.");
    }
    if std::fs::read_dir(&path).is_err() {
        return error(StatusCode::BAD_REQUEST, "The server isn't allowed to read that folder. Give the user in your stack's `user:` line read access to it (on TrueNAS: chown -R 1000:1000 on the folder, or edit the dataset permissions).");
    }
    let saved = if chosen.is_empty() { json!({}) } else { json!({ "musicDir": path.display().to_string() }) };
    let file = music_dir_file(&st.cfg);
    let tmp = file.with_extension("json.tmp");
    if std::fs::write(&tmp, saved.to_string()).and_then(|_| std::fs::rename(&tmp, &file)).is_err() {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "Couldn't save the setting.");
    }
    *st.music_dir.write().unwrap() = path.clone();
    st.watch.watch(&path); // follow the new folder
    start_scan(st.clone());
    Json(json!({ "path": path.display().to_string(), "isDefault": path == st.cfg.music_dir, "scanning": true })).into_response()
}

/// List the folders on the server so an admin can pick the music folder without typing a path.
async fn browse_folders(State(st): State<Shared>, headers: HeaderMap, Query(q): Query<HashMap<String, String>>) -> Response {
    if let Err(r) = require(&st, &headers, "settings") {
        return r;
    }
    let mut dir = q.get("path").map(|p| PathBuf::from(p.trim())).filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| st.music_dir.read().unwrap().clone());
    let missing = !dir.is_dir(); // typed a folder that isn't there: we show the closest one that is
    // a path that no longer exists falls back to the nearest folder above it
    while !dir.is_dir() {
        match dir.parent() {
            Some(p) if p != dir => dir = p.to_path_buf(),
            _ => { dir = PathBuf::from("/"); break; }
        }
    }
    // can this server read the folder at all? (the container's user may not be allowed to)
    let denied = matches!(std::fs::read_dir(&dir), Err(ref e) if e.kind() == std::io::ErrorKind::PermissionDenied);
    let mut dirs: Vec<(String, String)> = std::fs::read_dir(&dir).map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| {
        let name = e.file_name().to_string_lossy().to_string();
        (e.path().is_dir() && !name.starts_with('.')).then(|| (name, e.path().display().to_string()))
    }).collect()).unwrap_or_default();
    dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    dirs.truncate(500);
    Json(json!({
        "path": dir.display().to_string(),
        "parent": dir.parent().filter(|p| *p != dir.as_path()).map(|p| p.display().to_string()),
        "dirs": dirs.iter().map(|(n, p)| json!({ "name": n, "path": p })).collect::<Vec<_>>(),
        "songsHere": count_audio_here(&dir),
        "missing": missing,
        "denied": denied,
        // in Docker only the folders you mounted into the container exist
        "docker": std::path::Path::new("/.dockerenv").exists()
    })).into_response()
}

/* ---------------- scanning ---------------- */

fn start_scan(st: Shared) {
    {
        let mut lib = st.lib.write().unwrap();
        if lib.scanning {
            st.rescan_pending.store(true, std::sync::atomic::Ordering::Relaxed); // look again when this one ends
            return;
        }
        lib.scanning = true;
    }
    tokio::task::spawn_blocking(move || {
        let dir = st.music_dir.read().unwrap().clone();
        let fresh = library::scan(&dir);
        let (a, t) = (fresh.catalog.albums.len(), fresh.catalog.tracks.len());
        *st.lib.write().unwrap() = fresh;
        println!("library: {a} albums, {t} tracks in {}", dir.display());
        notify_ready(&st);
        if st.rescan_pending.swap(false, std::sync::atomic::Ordering::Relaxed) { start_scan(st.clone()); }
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
    for f in ["users.json", "sessions.json", "lidarr.json"] {
        if let Err(e) = vault::check(&cfg.data_dir.join(f)) {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
    let music_dir = load_music_dir(&cfg);
    if !music_dir.exists() {
        eprintln!("warning: music folder {} doesn't exist yet (choose one in Settings, or set VH_MUSIC_DIR)", music_dir.display());
    }
    let watch_tx = watch::Watch::new();
    let mut watch_rx = watch_tx.1;
    let st: Shared = Arc::new(AppState {
        music_dir: RwLock::new(music_dir.clone()),
        accounts: Mutex::new(Accounts::load(&cfg.data_dir)),
        lidarr: Mutex::new(lidarr::Settings::load(&cfg.data_dir)),
        requests: Mutex::new(lidarr::load_requests(&cfg.data_dir)),
        http: lidarr::Client::new(),
        mb: musicbrainz::Client::new(),
        lyrics: lyrics::Client::new(),
        fails: Mutex::new(HashMap::new()),
        social: social::Social::load(&cfg.data_dir),
        parties: party::Parties::new(),
        devices: devices::Devices::new(),
        cast_key: load_cast_key(&cfg.data_dir),
        transcoder: transcode::Transcoder::new(&cfg.data_dir),
        artists: artistinfo::Client::new(),
        shares: shares::Shares::load(&cfg.data_dir),
        watch: watch_tx.0,
        rescan_pending: std::sync::atomic::AtomicBool::new(false),
        lib: RwLock::new(Library::default()),
        cfg: cfg.clone(),
    });
    start_scan(st.clone());
    // watch the music folder: changes show up in seconds (the timed scan below is the safety net)
    {
        let dir = st.music_dir.read().unwrap().clone();
        if st.watch.watch(&dir) { println!("watching {} for changes", dir.display()); } else { println!("can't watch {} for changes: relying on the timed scan", dir.display()); }
        let s = st.clone();
        tokio::spawn(async move {
            while watch_rx.recv().await.is_some() {
                // let a burst of changes (a whole album being copied in) settle before looking
                let began = std::time::Instant::now();
                loop {
                    match tokio::time::timeout(Duration::from_secs(3), watch_rx.recv()).await {
                        Ok(Some(_)) if began.elapsed() < Duration::from_secs(60) => continue,
                        _ => break,
                    }
                }
                start_scan(s.clone());
                // files still being written when we looked get one more look a little later
                tokio::time::sleep(Duration::from_secs(20)).await;
                start_scan(s.clone());
            }
        });
    }
    if vault::enabled() { println!("data at rest: encrypted (VH_DATA_KEY is set)"); }
    // nightly housekeeping (see maintenance.rs): expired sign-ins, a dated backup, old cache
    {
        let s = st.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            loop {
                let keep = std::env::var("VH_BACKUP_KEEP").ok().and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(7);
                s.accounts.lock().unwrap().purge_expired();
                s.shares.purge(auth::now());
                if keep > 0 {
                    match maintenance::backup(&s.cfg.data_dir, auth::now(), keep) {
                        Ok(p) => println!("backup: {}", p.display()),
                        Err(e) => eprintln!("backup failed: {e}"),
                    }
                }
                let mb = std::env::var("VH_TRANSCODE_CACHE_MB").ok().and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(5000);
                let tc = transcode::clean(s.transcoder.dir(), mb * 1_048_576, 30);
                if tc > 0 { println!("transcodes: removed {tc} old converted files"); }
                let n = maintenance::clean_cache(&s.cfg.data_dir, 120);
                if n > 0 { println!("cache: removed {n} old look-ups"); }
                tokio::time::sleep(Duration::from_secs(86400)).await;
            }
        });
    }
    // parties: move on to the next song when one ends, and resend the state every 5 s so devices stay in step
    {
        let s = st.clone();
        tokio::spawn(async move {
            let mut every = tokio::time::interval(Duration::from_secs(1));
            let mut n: u64 = 0;
            loop {
                every.tick().await;
                n += 1;
                let dur = track_dur_ms(&s);
                s.parties.tick(n % 5 == 0, &dur);
            }
        });
    }
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
        .route("/api/users/{id}", delete(remove_user).put(edit_user))
        .route("/api/users/{id}/password", post(reset_password))
        .route("/api/me/avatar", axum::routing::put(put_avatar).delete(delete_avatar))
        .route("/api/avatar/{id}", get(get_avatar))
        .route("/api/library", get(get_library))
        .route("/api/library/rescan", post(rescan))
        .route("/api/library/folder", get(library_folder).put(set_library_folder))
        .route("/api/library/browse", get(browse_folders))
        .route("/api/stream/{id}", get(stream).options(cast_preflight))
        .route("/api/cover/{id}", get(cover).options(cast_preflight))
        .route("/api/cast/token", post(cast_token))
        .route("/api/shares", get(share_list).post(share_create))
        .route("/api/shares/{token}", delete(share_cancel))
        .route("/s/{token}", get(share_page))
        .route("/api/share/{token}", get(share_info))
        .route("/api/share/{token}/stream/{id}", get(share_stream))
        .route("/api/share/{token}/cover/{album}", get(share_cover))
        .route("/api/artist/info", get(artist_info))
        .route("/api/artist/image/{slug}", get(artist_image))
        .route("/api/lidarr", get(lidarr_status).put(lidarr_config))
        .route("/api/lidarr/search", get(lidarr_search))
        .route("/api/discover/artist/{id}", get(discover_artist))
        .route("/api/discover/album/{id}", get(discover_album))
        .route("/api/lidarr/request", post(lidarr_request))
        .route("/api/lidarr/queue", get(lidarr_queue))
        .route("/api/lidarr/requests", get(lidarr_requests))
        .route("/api/hooks/lidarr", post(lidarr_hook).put(lidarr_hook))
        .route("/api/credits/{id}", get(track_credits))
        .route("/api/lyrics/{id}", get(track_lyrics))
        .route("/api/party", get(party_mine).post(party_create))
        .route("/api/party/{id}/events", get(party_events))
        .route("/api/party/{id}/join", post(party_join))
        .route("/api/party/{id}/leave", post(party_leave))
        .route("/api/party/{id}/invite", post(party_invite))
        .route("/api/party/{id}/output", post(party_output))
        .route("/api/party/{id}/action", post(party_action))
        .route("/api/devices/events", get(device_events))
        .route("/api/devices/active", post(device_active))
        .route("/api/devices/transfer", post(device_transfer))
        .route("/", get(index_page))
        .route("/index.html", get(index_page))
        .route("/api/people", get(people))
        .route("/api/me/now", axum::routing::put(put_now))
        .route("/api/household", get(household))
        .route("/api/notifications", get(notifications))
        .route("/api/notifications/read", post(notifications_read))
        .route("/api/shared", get(shared_list).post(shared_create))
        .route("/api/shared/{id}/op", post(shared_op))
        .with_state(st.clone());

    // everything else is the web app (single page: unknown paths get index.html)
    let web = ServeDir::new(&cfg.web_dir).fallback(ServeFile::new(cfg.web_dir.join("index.html")));
    let app = api.fallback_service(web).layer(axum::middleware::from_fn(security_headers));

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("can't listen on {addr}: {e}");
            std::process::exit(1);
        }
    };
    println!("Volvix Home {VERSION} on http://{addr}  (music: {}, data: {}, web: {})",
        music_dir.display(), cfg.data_dir.display(), cfg.web_dir.display());
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .ok();
}
