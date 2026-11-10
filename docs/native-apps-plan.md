# Native Volvix Home apps (no web view): plan

Status: **approved 2026-11-10.** Windows UI toolkit: **iced** (chosen over Slint). The native
Windows app is a second program, `volvix-home-native`, inside `apps/windows` until it reaches parity.

## Goal

Real native apps that feel at home on each device, instead of windows that show the website.
The website (`web/`) stays exactly as it is for browsers. The current web-view apps (0.35) keep
shipping until the native ones can do everything they do.

## Shape

```
server/            (unchanged) Rust + axum, JSON API
core/              NEW  volvix-core: everything an app knows that isn't drawing
apps/windows/      today: wry/WebView2 shell  ->  native Rust UI on core/
apps/ios/          today: WKWebView shell     ->  SwiftUI on core/ (via UniFFI), later
web/               (unchanged) the website
```

### 1. `core/` (Rust crate `volvix-core`)

Shared by every native app. No UI code in it.

- **Server client**: `/api/status`, `/api/login`, `/api/me`, `/api/library`, `/api/me/data`,
  `/api/stream/{id}` (range requests), `/api/cover/{id}?s=640&v=…`, `/api/lyrics/{id}`,
  playlists, `/api/shared`, `/api/lidarr/*`, `/api/party/*`, etc. Signs in with
  `Authorization: Bearer <token>` (the server already accepts it), so no cookie jar.
- **Session**: remembers the server address and token on disk; "Sign in first" (401) sends you
  back to sign-in.
- **Library model**: artists/albums/tracks with fast lookups (album's songs in disc/track order,
  artist's albums, recently added), and in-library search (same matching as the website).
- **Play queue**: queue, next/previous, shuffle, repeat (off/all/one), "play next", "add to queue".
  Pure logic, so it's tested without sound.
- **Cover cache**: covers on disk keyed by album + size + version. Cover addresses carry `?v=`
  and never change, so a cached cover is never fetched again.
- **Later phases**: offline downloads, Made For You mixes (`web/js/mixes.js`), stats
  (`web/js/stats.js`), synced and word-by-word lyrics (`web/js/lyrics.js`) move here from
  JavaScript so the Windows and iPhone apps share one copy.
- **Swift access** (phase 4): UniFFI bindings, built as an XCFramework in CI.

Blocking HTTP (`ureq`, already used by the Windows updater) run on worker threads keeps the core
simple and easy to call from Swift; no async runtime is needed.

### 2. Windows app (Rust)

**UI toolkit, your choice:**

| | **Slint** (recommended) | **iced** |
|---|---|---|
| Style | Declarative `.slint` markup + Rust logic, like a small QML | Pure Rust, Elm-style (state → view) |
| Look | Easy to make it look like the website (dark, red accent, rounded cards, blur-free) | Also doable, more hand work for each widget |
| Long lists (library of thousands of songs) | Built-in virtualised `ListView` | Needs care (lazy widgets) |
| Images | Built-in, from memory or files | Built-in |
| Live preview while designing | Yes (VS Code extension) | No |
| Licence | Free "royalty-free" licence for desktop apps (needs a small "Made with Slint" credit in About, or GPL) | MIT, no strings |
| Maturity | 1.x, stable API, commercial company behind it | 0.13, API still changes between versions |

Recommendation: **Slint**, mainly for the virtualised lists, the stable 1.x API and quicker
screen design. Pick iced if the licence credit bothers you.

**Audio**: `symphonia` decodes (MP3, FLAC, AAC/M4A, Ogg/Opus-Vorbis, WAV), `cpal` plays. The
song streams from `/api/stream/{id}` with range requests (seeking jumps, no full download first),
with ReplayGain from the library applied. Gapless and crossfade can come later.

**Windows extras**: media keys + the Windows volume/media overlay (System Media Transport
Controls via the `windows` crate) with cover and title; taskbar thumbnail buttons later. Kept from
today's app: tray icon and menu, single instance, start with Windows, close to tray, and the
GitHub self-updater (`settings.rs` and `update.rs` move over almost unchanged).

### 3. iPhone app (SwiftUI, phase 4)

SwiftUI screens on top of the same core through UniFFI. Audio through `AVPlayer` (background
play, lock screen, Control Center, AirPlay, CarPlay later), fed the stream address and token
from the core. Built and shipped through the existing iOS CI.

## Phases

1. **Core crate**: client, session, library model, search, queue, cover cache; unit tests plus
   tests against a local server (`powershell -File dev.ps1 -Mocks`).
2. **Windows MVP** (separate binary `volvix-home-native` beside today's app, so nothing ships by
   accident): connect + sign in, Home, Library, Album, Artist, Search (in your library),
   playback with the queue, mini player + Now Playing, media keys/overlay, tray, updater.
3. **Windows parity**: playlists (editing, shared/collaborative), lyrics, mixes, Lidarr requests
   and "Get more music", household and notifications, offline downloads, party mode, casting,
   settings and account. When it matches the web-view app, it replaces it under the same name,
   installer and update feed.
4. **iPhone**: UniFFI bindings, SwiftUI app on the core, replaces the WKWebView app when it
   catches up.

## What you'll see at the end of this first session

- This plan, approved.
- `core/` building, with tests passing against a local dev server.
- The Windows MVP signing in to a local dev server, browsing the library and playing a song
  with the queue, with a screenshot. Not released; today's 0.35 apps are untouched.

## Where it stands (2026-11-10)

- **Phase 1 done**: `core/` with 23 unit tests and 5 tests against a dev server
  (`cargo test` in `core/`; with a dev server running: `cargo test -- --ignored`, see `core/tests/server.rs`).
- **Phase 2 MVP working**: `apps/windows/src/native/` (iced 0.13, rodio 0.20 on symphonia).
  Signs in, Home, Library (albums/artists), Album, Artist, Search, playback that streams with
  range requests, queue (play next, add, remove, jump, shuffle, repeat), mini player, Now Playing,
  media keys and the Windows media overlay (souvlaki), tray menu, one copy only, updater wiring.
  Keeps its own data in `%LOCALAPPDATA%\Volvix Home Native` and its own startup entry, so it runs
  beside today's app. The release build of today's app is unchanged (the native binary needs
  `--features native`).

Run it (with `powershell -File dev.ps1` running):

```
cargo run --manifest-path apps/windows/Cargo.toml --features native --bin volvix-home-native
```

Debug builds also accept `VH_DEMO=album|nowplaying|artist|library|search:<words>` to open a screen
and start playing without clicking (used for screenshots).

Not yet checked by hand: clicking through the window (only scripted screens were tried), real
cover art (the dev music has none), the tray and media overlay on screen, and decoding MP3/FLAC/M4A
(the dev music is WAV). Next: those checks, gapless/next-song preloading, keyboard shortcuts,
resume where you left off, then phase 3.

## Questions for you (answered)

1. **Slint or iced** for the Windows app?
2. OK for the native Windows app to live as a second binary (`volvix-home-native`) inside
   `apps/windows` until it reaches parity, then take over?
