import SwiftUI

// MARK: - Household

struct HouseholdView: View {
    @EnvironmentObject var app: AppModel

    var body: some View {
        VStack(spacing: 0) {
            TopBar()
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    Text("Household").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)
                    Text("What everyone at home is listening to.").font(.system(size: 14)).foregroundColor(Theme.text2)
                    if app.members.isEmpty {
                        Text("No one else is on this server yet.").font(.system(size: 15)).foregroundColor(Theme.text2).padding(.top, 8)
                    }
                    ForEach(app.members) { m in
                        VStack(alignment: .leading, spacing: 10) {
                        HStack(spacing: 14) {
                            AvatarView(id: m.id, name: m.name, size: 48)
                            VStack(alignment: .leading, spacing: 3) {
                                Text(m.name).font(.system(size: 16, weight: .semibold)).foregroundColor(Theme.text)
                                if m.trackTitle.isEmpty {
                                    Text("Not listening right now").font(.system(size: 13)).foregroundColor(Theme.text2)
                                } else {
                                    Text((m.listening ? "Listening now · " : "Last played · ") + m.trackTitle + " · " + m.trackArtist)
                                        .font(.system(size: 13)).foregroundColor(m.listening ? Theme.accent : Theme.text2).lineLimit(2)
                                }
                            }
                            Spacer(minLength: 0)
                            if let a = m.albumID { CoverView(albumID: a, corner: 6).frame(width: 44, height: 44) }
                        }
                        let theirs = app.householdLists.filter { $0.owner == m.id }
                        if !theirs.isEmpty {
                            ScrollView(.horizontal, showsIndicators: false) {
                                HStack(spacing: 10) {
                                    ForEach(theirs) { p in
                                        NavigationLink(value: Route.playlist(p.id)) {
                                            HStack(spacing: 8) {
                                                PlaylistArt(playlist: p).frame(width: 34, height: 34)
                                                Text(p.name).font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                                            }
                                            .padding(6).padding(.trailing, 6).background(Theme.surface3).clipShape(Capsule())
                                        }
                                        .buttonStyle(.plain)
                                    }
                                }
                            }
                        }
                        }
                        .padding(12)
                        .background(Theme.surface2)
                        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                    }
                }
                .padding(16)
            }
            .refreshable { await app.pollOnce() }
        }
        .background(Theme.surface)
        .navigationBarHidden(true)
    }
}

// MARK: - Get more music (Requests and the search that finds it)

/// A Get button that knows if it is already here, asked for, or in progress.
struct GetButton: View {
    @EnvironmentObject var app: AppModel
    let item: FoundItem
    let kind: String            // "album" or "artist": what Lidarr is asked for
    let foreignId: String?
    var label = "Get"
    @Binding var asked: Set<String>
    @State private var busy = false
    @State private var failed: String?

    var body: some View {
        if item.onServer {
            Text("In your library").font(.system(size: 12, weight: .semibold)).foregroundColor(Theme.text2)
        } else if item.requested || (foreignId.map { asked.contains($0) } ?? false) {
            Label("Requested", systemImage: "arrow.down.circle").font(.system(size: 12, weight: .semibold)).foregroundColor(Theme.text2)
        } else if app.me?.canRequest != true {
            Text("Ask an admin").font(.system(size: 12)).foregroundColor(Theme.text3)
        } else if let id = foreignId {
            Button {
                busy = true
                Task {
                    do {
                        try await app.api?.requestMusic(kind: kind, foreignId: id)
                        asked.insert(id)
                    } catch {
                        failed = error.localizedDescription
                    }
                    busy = false
                }
            } label: {
                HStack(spacing: 6) {
                    if busy { ProgressView().tint(.white).scaleEffect(0.7) } else { Image(systemName: "arrow.down.to.line") }
                    Text(busy ? "Asking…" : label).font(.system(size: 13, weight: .semibold))
                }
                .padding(.horizontal, 14).padding(.vertical, 7)
                .background(Theme.accent).foregroundColor(.white).clipShape(Capsule())
            }
            .disabled(busy)
        }
    }
}

/// One thing found on MusicBrainz, with its Get button.
struct FoundRow: View {
    @EnvironmentObject var app: AppModel
    let item: FoundItem
    let isArtist: Bool
    let isSong: Bool
    @Binding var asked: Set<String>

    var body: some View {
        HStack(spacing: 12) {
            // tap the picture or the name for the page: an artist's releases, or the release's songs
            NavigationLink(value: route) {
                HStack(spacing: 12) {
                    RemoteSquare(path: item.image, title: item.label, corner: 8).frame(width: 52, height: 52)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(item.label).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                        Text(subtitle).font(.system(size: 12)).foregroundColor(Theme.text2).lineLimit(1)
                    }
                    Spacer(minLength: 6)
                }
            }
            .buttonStyle(.plain)
            GetButton(item: item, kind: isArtist ? "artist" : "album", foreignId: target, label: isArtist ? "Get all" : "Get", asked: $asked)
        }
        .padding(10)
        .background(Theme.surface2)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .padding(.horizontal, 16)
    }

    private var route: Route {
        if isArtist { return .discoverArtist(item.foreignId ?? "") }
        if let s = item.onServerValue, !isSong, app.album(s) != nil { return .album(s) }
        if isSong { return .discoverAlbum(item.albumId ?? item.singleId ?? "") }
        return .discoverAlbum(item.foreignId ?? "")
    }

    /// Lidarr gets whole releases: a song gets its single when there is one, else its album.
    private var target: String? {
        if isSong { return item.singleId ?? item.albumId }
        return item.foreignId
    }

    private var subtitle: String {
        var parts: [String] = []
        if isArtist { parts.append("Artist") }
        if let a = item.artist { parts.append(a) }
        if let k = item.kind, !isArtist, k != "Album" { parts.append(k) }
        if let y = item.year { parts.append(y) }
        if let d = item.disambiguation, isArtist { parts.append(d) }
        return parts.joined(separator: " · ")
    }
}

/// Under search results: music from MusicBrainz that you can ask Lidarr for.
struct GetMoreSection: View {
    @EnvironmentObject var app: AppModel
    let query: String
    @State private var albums: [FoundItem] = []
    @State private var songs: [FoundItem] = []
    @State private var artists: [FoundItem] = []
    @State private var looking = false
    @State private var asked: Set<String> = []
    @State private var failed: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if app.me?.lidarrConfigured == true, query.trimmingCharacters(in: .whitespaces).count >= 2 {
                SectionTitle(text: "Get more music")
                if looking { HStack { ProgressView().tint(.white); Text("Looking…").font(.system(size: 13)).foregroundColor(Theme.text2) }.padding(.horizontal, 16) }
                if let f = failed { ProblemBox(text: f).padding(.horizontal, 16) }
                if !artists.isEmpty { subtitle("Artists"); ForEach(artists.prefix(4)) { a in FoundRow(item: a, isArtist: true, isSong: false, asked: $asked) } }
                if !albums.isEmpty { subtitle("Albums"); ForEach(albums.prefix(6)) { a in FoundRow(item: a, isArtist: false, isSong: false, asked: $asked) } }
                if !songs.isEmpty { subtitle("Songs"); ForEach(songs.prefix(6)) { a in FoundRow(item: a, isArtist: false, isSong: true, asked: $asked) } }
                if !looking && failed == nil && artists.isEmpty && albums.isEmpty && songs.isEmpty {
                    Text("Nothing else found.").font(.system(size: 14)).foregroundColor(Theme.text2).padding(.horizontal, 16)
                }
                Text("Results from MusicBrainz. Lidarr gets whole releases, so a song comes with its album or single.")
                    .font(.system(size: 11)).foregroundColor(Theme.text3).padding(.horizontal, 16)
            }
        }
        .task(id: query) { await search() }
    }

    private func subtitle(_ t: String) -> some View {
        Text(t).font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text2).padding(.horizontal, 16)
    }

    private func search() async {
        albums = []; songs = []; artists = []; failed = nil
        let q = query.trimmingCharacters(in: .whitespaces)
        guard q.count >= 2, app.me?.lidarrConfigured == true, let api = app.api else { return }
        // wait for the typing to settle
        try? await Task.sleep(nanoseconds: 600_000_000)
        if Task.isCancelled { return }
        looking = true
        async let a = try? api.find(q, kind: "albums")
        async let s = try? api.find(q, kind: "songs")
        async let r = try? api.find(q, kind: "artists")
        let (ra, rs, rr) = await (a, s, r)
        if Task.isCancelled { return }
        albums = ra?.albums ?? []
        songs = rs?.songs ?? []
        artists = rr?.artists ?? []
        if ra == nil && rs == nil && rr == nil { failed = "Couldn't look that up right now." }
        looking = false
    }
}

struct RequestsView: View {
    @EnvironmentObject var app: AppModel
    @State private var downloads: [DownloadItem] = []
    @State private var requests: [RequestItem] = []
    @State private var loaded = false

    var body: some View {
        VStack(spacing: 0) {
            TopBar()
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    Text("Requests").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)
                    if app.me?.lidarrConfigured != true {
                        Text("Lidarr isn't connected. Ask whoever runs this server to connect it in Settings, then you can search for music you don't have and get it from here.")
                            .font(.system(size: 15)).foregroundColor(Theme.text2)
                    } else {
                        Text("Music Lidarr is getting for you. It appears in your library as soon as it's imported.")
                            .font(.system(size: 14)).foregroundColor(Theme.text2)

                        Text("Downloading").font(.system(size: 20, weight: .bold)).foregroundColor(Theme.text)
                        if downloads.isEmpty { Text("Nothing downloading right now.").font(.system(size: 14)).foregroundColor(Theme.text2) }
                        ForEach(downloads) { d in
                            HStack(spacing: 12) {
                                RemoteSquare(path: d.image, title: d.title, corner: 8).frame(width: 52, height: 52)
                                VStack(alignment: .leading, spacing: 6) {
                                    Text(d.title).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                                    Text(d.artist + " · " + stateText(d)).font(.system(size: 12)).foregroundColor(Theme.text2).lineLimit(1)
                                    ProgressView(value: min(1, max(0, d.progress))).tint(Theme.accent)
                                }
                            }
                            .padding(12).background(Theme.surface2).clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                        }

                        Text("Requested").font(.system(size: 20, weight: .bold)).foregroundColor(Theme.text).padding(.top, 6)
                        if requests.isEmpty && loaded {
                            Text("Nothing requested yet. Search for an artist or album and anything not in your library can be requested from there.")
                                .font(.system(size: 14)).foregroundColor(Theme.text2)
                        }
                        ForEach(requests) { r in
                            HStack(spacing: 12) {
                                RemoteSquare(path: r.image, title: r.title, corner: 8).frame(width: 52, height: 52)
                                VStack(alignment: .leading, spacing: 3) {
                                    Text(r.kind == "artist" ? "Everything by \(r.artist)" : r.title).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                                    Text(r.kind == "artist" ? "Requested by \(r.by)" : "\(r.artist) · requested by \(r.by)").font(.system(size: 12)).foregroundColor(Theme.text2).lineLimit(1)
                                    if let p = r.problem { Text(p).font(.system(size: 12)).foregroundColor(Theme.accent).lineLimit(2) }
                                }
                                Spacer(minLength: 4)
                                if r.onServer {
                                    Text("In your library").font(.system(size: 12, weight: .semibold)).foregroundColor(Theme.text2)
                                } else if r.problem == nil {
                                    Text("Waiting").font(.system(size: 12)).foregroundColor(Theme.text2)
                                }
                            }
                            .padding(10).background(Theme.surface2).clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                            .contextMenu {
                                if r.by == app.user?.name && !r.onServer {
                                    Button(role: .destructive) { cancel(r) } label: { Label("Cancel request", systemImage: "xmark") }
                                }
                            }
                        }
                    }
                }
                .padding(16)
            }
            .refreshable { await reload() }
        }
        .background(Theme.surface)
        .navigationBarHidden(true)
        .task {
            // keep it up to date while this screen is open
            while !Task.isCancelled {
                await reload()
                try? await Task.sleep(nanoseconds: 6_000_000_000)
            }
        }
    }

    private func reload() async {
        guard let api = app.api, app.me?.lidarrConfigured == true else { return }
        downloads = await api.downloads()
        requests = await api.requests()
        loaded = true
    }

    private func cancel(_ r: RequestItem) {
        requests.removeAll { $0.id == r.id }
        Task { try? await app.api?.cancelRequest(r.foreignId) }
    }

    private func stateText(_ d: DownloadItem) -> String {
        switch d.state {
        case "downloading": return "Downloading \(Int((d.progress * 100).rounded()))%"
        case "importPending": return "Ready to import"
        case "importing": return "Importing"
        case "imported": return "Imported"
        case "queued", "delay": return "Waiting"
        case "paused": return "Paused"
        case "failed", "failedPending": return "Problem"
        default: return d.state
        }
    }
}

// MARK: - Lyrics

struct LyricsView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    let trackID: String
    @ObservedObject var clock: PlayClock
    @State private var lyrics: LyricsData?
    @State private var loaded = false

    private var current: Int? {
        guard let l = lyrics, !l.synced.isEmpty else { return nil }
        let ms = Int(clock.position * 1000) + 250
        var found: Int? = nil
        for (i, line) in l.synced.enumerated() {
            if line.ms <= ms { found = i } else { break }
        }
        return found
    }

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    if let l = lyrics, !l.synced.isEmpty {
                        ForEach(Array(l.synced.enumerated()), id: \.offset) { i, line in
                            lineText(line, i)
                                .font(.system(size: 26, weight: .bold))
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .contentShape(Rectangle())
                                .onTapGesture { player.seek(to: Double(line.ms) / 1000) }
                                .id(i)
                        }
                    } else if let l = lyrics, l.instrumental {
                        Text("Instrumental").font(.system(size: 18)).foregroundColor(Theme.text2)
                    } else if let l = lyrics, !l.plain.isEmpty {
                        Text(l.plain).font(.system(size: 20, weight: .semibold)).foregroundColor(Theme.text).frame(maxWidth: .infinity, alignment: .leading)
                    } else if loaded {
                        Text("No lyrics found for this song.").font(.system(size: 16)).foregroundColor(Theme.text2)
                    } else {
                        ProgressView().tint(.white)
                    }
                }
                .padding(.horizontal, 24)
                .padding(.vertical, 24)
            }
            .onChange(of: current) { value in
                if let i = value { withAnimation(.easeInOut(duration: 0.4)) { proxy.scrollTo(i, anchor: .center) } }
            }
        }
        .task(id: trackID) {
            loaded = false
            lyrics = nil
            lyrics = await app.api?.lyrics(trackID)
            loaded = true
        }
    }

    /// A line of lyrics; if the song has word timing, the words already sung are lit.
    private func lineText(_ line: LyricLine, _ i: Int) -> Text {
        if i == current, !line.words.isEmpty {
            let ms = Int(clock.position * 1000) + 250
            var t = Text("")
            for w in line.words { t = t + Text(w.text).foregroundColor(ms >= w.ms ? Theme.text : Theme.text3) }
            return t
        }
        return Text(line.text.isEmpty ? "♪" : line.text).foregroundColor(color(i))
    }

    private func color(_ i: Int) -> Color {
        guard let c = current else { return Theme.text }
        if i == c { return Theme.text }
        return i < c ? Theme.text3 : Theme.text2
    }
}
