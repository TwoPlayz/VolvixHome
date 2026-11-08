# Volvix Home

A self-hosted music server for your household, built from the Volvix app. It streams your own
library: the folder Lidarr fills, or any folder of music files. It works in any browser, on desktop
and on phones.

- **Your music:** artists, albums and songs read from the files' tags. Untagged files fall back to
  the folder layout `Artist/Album/01 - Title.flac`. Cover art comes from `cover.jpg` / `folder.jpg`
  or the files themselves.
- **Real playback:** queue (drag to reorder), shuffle, repeat, seeking, and lock screen and media-key
  controls. You come back to where you left off.
- **Household accounts:** the first person to open a new server becomes the admin, and the admin
  adds everyone else. Music is shared; playlists, history and settings are personal and follow you
  to every device.
- **Get more music with Lidarr:** search shows albums and artists you don't have yet, with a
  **Get** button. Lidarr finds and downloads them with the sources you've set up there. The
  Requests page shows progress and who asked, and new albums appear as soon as Lidarr imports them
  (webhook). An admin can limit requests to admins.
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

To reach it from outside your home, put it behind a reverse proxy with HTTPS (Caddy, Nginx Proxy
Manager, Tailscale…). Sessions use an HttpOnly cookie.

## Develop

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
| `GET /api/lidarr/search?term=` | artists and albums Lidarr can get, marked if already in the library |
| `POST /api/lidarr/request` | `{kind: album or artist, foreignId}`: ask Lidarr to get it |
| `GET /api/lidarr/queue`, `GET /api/lidarr/requests` | downloads in progress; who requested what |
| `POST /api/hooks/lidarr?token=` | Lidarr's webhook: rescan after an import |

## Roadmap

1. ~~Core: library, streaming, accounts, Docker~~
2. ~~**Lidarr:** search, request, progress, appears automatically~~
3. **SoundCloud streaming** in the same player.
4. **Apps:** the Windows and iPhone shells from Volvix, pointed at your server.
