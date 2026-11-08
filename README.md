# Volvix Home

A self-hosted music server for your household, built from the Volvix app. It streams your own
library: the folder Lidarr fills, or any folder of music files. It works in any browser, on desktop
and on phones.

- **Your music:** artists, albums and songs read from the files' tags. Untagged files fall back to
  the folder layout `Artist/Album/01 - Title.flac`. Cover art comes from `cover.jpg` / `folder.jpg`
  or the files themselves.
- **Real playback:** queue (drag to reorder), shuffle, repeat, seeking, and lock screen and media-key
  controls. You come back to where you left off.
- **Household accounts:** sign in with just your name and a password (no username). The first person to
  open a new server becomes the admin, and the admin adds everyone else (Account → People).
- **Liked Songs:** a heart on the player, Now Playing and every song row adds or removes the song in one
  built-in Liked Songs playlist.
- **Music folder:** admins pick it under Settings → Library by browsing the server's folders (or set
  `VH_MUSIC_DIR`). Lidarr should save to the same folder. Music is shared; playlists, history and settings are personal and follow you
  to every device.
- **Get more music with Lidarr:** search (powered by MusicBrainz) shows songs, albums and artists
  you don't have yet. A song offers **Get the single** and **Get the album**; albums and artists have
  **Get**. Lidarr fetches the release with the sources you've set up there. The Requests page shows
  progress and who asked, and new music appears as soon as Lidarr imports it (webhook). An admin can
  limit requests to admins.
- **Original quality:** files are streamed untouched (FLAC, ALAC, WAV, MP3, AAC, OGG/Opus). The app
  shows the real format, bit depth and sample rate.

## Run it (Docker)

See [docker-compose.yml](docker-compose.yml): Volvix Home next to Lidarr, both pointed at the same
music folder.

```sh
docker compose up -d
# open http://<server>:4747 and create the admin account
```

The image is published by GitHub Actions as `ghcr.io/<you>/volvix-home:latest` for amd64 and
arm64, so it runs on most NAS boxes and on a Raspberry Pi.

| Setting | Default | |
|---|---|---|
| `VH_MUSIC_DIR` | `/music` | your music (read-only is fine) |
| `VH_DATA_DIR` | `/data` | accounts, playlists, settings |
| `VH_PORT` | `4747` | |
| `VH_RESCAN_MINUTES` | `15` | how often to look for new music, `0` = only on start or on demand |
| `VH_LIDARR_URL` | | e.g. `http://lidarr:8686` (or set it in Settings → Lidarr) |
| `VH_LIDARR_API_KEY` | | Lidarr → Settings → General → API Key |
| `VH_MUSICBRAINZ_URL` | musicbrainz.org | set only to use a MusicBrainz mirror |

To reach it from outside your home, put it behind a reverse proxy with HTTPS (Caddy, Nginx Proxy
Manager, Tailscale…). Sessions use an HttpOnly cookie.

## Apps

Native apps for Windows and iPhone. They don't bundle anything from the server — each just asks for your
server's address once, remembers it, and opens your library. They never open your browser or leave your
server.

- **Windows** (`apps/windows`, Rust + WebView2): download `volvix-home.exe` from the Apps workflow
  and run it. First launch asks for the address (e.g. `http://volvix.lan:4747`). Ctrl+Shift+S reopens
  the connect screen to switch servers.
- **iPhone** (`apps/ios`, Swift + WKWebView): built by CI as an unsigned `VolvixHome.ipa`; install it
  with LiveContainer, SideStore/AltStore or Sideloadly. First launch asks for the address. Audio keeps
  playing in the background.

Both are built by GitHub Actions (`.github/workflows/apps.yml`) — run the **Apps** workflow and download
`Volvix-Home-Windows` and `Volvix-Home-iOS`. To build the Windows app yourself:
`cargo build --release --manifest-path apps/windows/Cargo.toml`.

## Develop

Quickest way to try changes on your own PC (no Docker, nothing to host):

```powershell
powershell -File dev.ps1          # server + sample music on http://localhost:4747
powershell -File dev.ps1 -Mocks   # also a fake Lidarr and MusicBrainz, so Get more music works
```

The site is served straight from `web/`, so refresh the page after editing JS or CSS. Only Rust changes
(`server/src`) need Ctrl+C and a re-run. State lives in `.dev/`; delete it to start fresh.

Or run the server by hand:

```sh
cd server
VH_MUSIC_DIR=/path/to/music VH_DATA_DIR=./data VH_WEB_DIR=../web cargo run
```

- `server/`: Rust (axum). `library.rs` scans the folder, `auth.rs` handles accounts and sessions,
  `main.rs` is the HTTP API.
- `web/`: the app, plain HTML, CSS and JS. `js/boot.js` signs you in and loads the library;
  `js/data.js` turns it into the shapes the views use.

### API

| | |
|---|---|
| `GET /api/status` | name, version, `setupNeeded`, scan state |
| `POST /api/setup` | first run: create the admin `{username, name, password}` |
| `POST /api/login`, `POST /api/logout`, `GET /api/me` | sessions (cookie, or `Authorization: Bearer <token>`) |
| `POST /api/me/password` | `{current, new}` |
| `GET/PUT /api/me/data` | your app data (playlists, settings) |
| `GET/POST /api/users`, `DELETE /api/users/{id}` | household (admin) |
| `GET /api/library`, `POST /api/library/rescan` | catalog; rescan (admin) |
| `GET /api/stream/{trackId}` | audio, with HTTP Range for seeking |
| `GET /api/cover/{albumId}` | album art |
| `GET/PUT /api/lidarr` | Lidarr status; connect and choose folder/profiles (admin) |
| `GET /api/lidarr/search?term=` | songs, albums, artists from MusicBrainz, marked if already in the library |
| `POST /api/lidarr/request` | `{kind: album or artist, foreignId}`: ask Lidarr to get it |
| `GET /api/lidarr/queue`, `GET /api/lidarr/requests` | downloads in progress; who requested what |
| `POST /api/hooks/lidarr?token=` | Lidarr's webhook: rescan after an import |

## Roadmap

1. ~~Core: library, streaming, accounts, Docker~~
2. ~~**Lidarr:** search, request, progress, appears automatically~~
3. ~~**Apps:** Windows and iPhone, pointed at your server~~
