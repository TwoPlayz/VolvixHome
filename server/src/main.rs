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

mod auth;
mod library;
mod lidarr;
mod musicbrainz;
mod social;

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
    social: social::Social,
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
    /// optional now: derived from the name
    #[serde(default)]
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
    /// the person's name (the older "username" field still works)
    #[serde(default)]
    name: String,
    #[serde(default)]
    username: String,
    password: String,
}

async fn login(State(st): State<Shared>, Json(body): Json<Login>) -> Response {
    let found = {
        let accounts = st.accounts.lock().unwrap();
        let who = if body.name.trim().is_empty() { &body.username } else { &body.name };
        accounts.find(who).filter(|u| auth::verify_password(&body.password, &u.hash)).cloned()
    };
    match found {
        Some(user) => {
            let token = st.accounts.lock().unwrap().start_session(&user.id);
            signed_in_response(&user, &token)
        }
        None => {
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
            Json(json!({ "user": PublicUser::from(&user), "lidarr": { "configured": l.configured(), "canRequest": l.configured() && can_request(&user, &l) } })).into_response()
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

/* ---------------- Lidarr ---------------- */

fn can_request(user: &User, l: &lidarr::Settings) -> bool {
    user.admin || l.everyone_can_request
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
    if user.admin {
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
    if let Err(r) = admin(&st, &headers) {
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
    let s = st.lidarr.lock().unwrap().clone();
    if !can_request(&user, &s) {
        return error(StatusCode::FORBIDDEN, "Ask an admin to get music for you.");
    }
    let term = q.get("term").map(|t| t.trim().to_string()).unwrap_or_default();
    if term.chars().count() < 2 {
        return Json(json!({ "artists": [], "albums": [] })).into_response();
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
    Json(json!({ "songs": songs, "artists": artists, "albums": albums })).into_response()
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

async fn people(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = signed_in(&st, &headers) {
        return r;
    }
    let accounts = st.accounts.lock().unwrap();
    let list: Vec<Value> = accounts.users.iter().map(|u| json!({ "id": u.id, "name": u.name, "admin": u.admin })).collect();
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
    if let Err(r) = admin(&st, &headers) {
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
    if let Err(r) = admin(&st, &headers) {
        return r;
    }
    let chosen = body.path.trim();
    let path = if chosen.is_empty() { st.cfg.music_dir.clone() } else { PathBuf::from(chosen) };
    if !path.is_dir() {
        return error(StatusCode::BAD_REQUEST, "That folder doesn't exist on the server.");
    }
    if std::fs::read_dir(&path).is_err() {
        return error(StatusCode::BAD_REQUEST, "The server isn't allowed to read that folder.");
    }
    let saved = if chosen.is_empty() { json!({}) } else { json!({ "musicDir": path.display().to_string() }) };
    let file = music_dir_file(&st.cfg);
    let tmp = file.with_extension("json.tmp");
    if std::fs::write(&tmp, saved.to_string()).and_then(|_| std::fs::rename(&tmp, &file)).is_err() {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "Couldn't save the setting.");
    }
    *st.music_dir.write().unwrap() = path.clone();
    start_scan(st.clone());
    Json(json!({ "path": path.display().to_string(), "isDefault": path == st.cfg.music_dir, "scanning": true })).into_response()
}

/// List the folders on the server so an admin can pick the music folder without typing a path.
async fn browse_folders(State(st): State<Shared>, headers: HeaderMap, Query(q): Query<HashMap<String, String>>) -> Response {
    if let Err(r) = admin(&st, &headers) {
        return r;
    }
    let mut dir = q.get("path").map(|p| PathBuf::from(p.trim())).filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| st.music_dir.read().unwrap().clone());
    // a path that no longer exists falls back to the nearest folder above it
    while !dir.is_dir() {
        match dir.parent() {
            Some(p) if p != dir => dir = p.to_path_buf(),
            _ => { dir = PathBuf::from("/"); break; }
        }
    }
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
        "songsHere": count_audio_here(&dir)
    })).into_response()
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
        let dir = st.music_dir.read().unwrap().clone();
        let fresh = library::scan(&dir);
        let (a, t) = (fresh.catalog.albums.len(), fresh.catalog.tracks.len());
        *st.lib.write().unwrap() = fresh;
        println!("library: {a} albums, {t} tracks in {}", dir.display());
        notify_ready(&st);
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
    let music_dir = load_music_dir(&cfg);
    if !music_dir.exists() {
        eprintln!("warning: music folder {} doesn't exist yet (choose one in Settings, or set VH_MUSIC_DIR)", music_dir.display());
    }
    let st: Shared = Arc::new(AppState {
        music_dir: RwLock::new(music_dir.clone()),
        accounts: Mutex::new(Accounts::load(&cfg.data_dir)),
        lidarr: Mutex::new(lidarr::Settings::load(&cfg.data_dir)),
        requests: Mutex::new(lidarr::load_requests(&cfg.data_dir)),
        http: lidarr::Client::new(),
        mb: musicbrainz::Client::new(),
        social: social::Social::load(&cfg.data_dir),
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
        .route("/api/library/folder", get(library_folder).put(set_library_folder))
        .route("/api/library/browse", get(browse_folders))
        .route("/api/stream/{id}", get(stream))
        .route("/api/cover/{id}", get(cover))
        .route("/api/lidarr", get(lidarr_status).put(lidarr_config))
        .route("/api/lidarr/search", get(lidarr_search))
        .route("/api/lidarr/request", post(lidarr_request))
        .route("/api/lidarr/queue", get(lidarr_queue))
        .route("/api/lidarr/requests", get(lidarr_requests))
        .route("/api/hooks/lidarr", post(lidarr_hook).put(lidarr_hook))
        .route("/api/credits/{id}", get(track_credits))
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
        music_dir.display(), cfg.data_dir.display(), cfg.web_dir.display());
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .ok();
}
