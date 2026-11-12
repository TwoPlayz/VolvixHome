import SwiftUI

// MARK: - Home

struct HomeView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @EnvironmentObject var downloads: DownloadStore
    @State private var mixes: [Mix] = []
    @State private var mostPlayed: [Album] = []
    @State private var picks: [Album] = []

    private var greeting: String {
        let h = Calendar.current.component(.hour, from: Date())
        if h < 5 { return "Good night" }
        if h < 12 { return "Good morning" }
        if h < 18 { return "Good afternoon" }
        return "Good evening"
    }

    /// The tiles at the top: Liked Songs, then your recent albums.
    private var quick: [(String, Route, String?)] {
        var out: [(String, Route, String?)] = []
        if let l = app.liked { out.append((l.name, Route.playlist(l.id), nil)) }
        for a in app.recentAlbums.prefix(6) { out.append((a.title, Route.album(a.id), a.id)) }
        return Array(out.prefix(6))
    }

    var body: some View {
        VStack(spacing: 0) {
            TopBar()
            ScrollView {
                VStack(alignment: .leading, spacing: 26) {
                    Text(greeting).font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text).padding(.horizontal, 16)

                    if app.albums.isEmpty {
                        Text("Your library is empty so far. Music shows up here when the server finds it.")
                            .font(.system(size: 15)).foregroundColor(Theme.text2).padding(.horizontal, 16)
                    }

                    RewindBanner()

                    if !quick.isEmpty {
                        LazyVGrid(columns: [GridItem(.flexible(), spacing: 8), GridItem(.flexible(), spacing: 8)], spacing: 8) {
                            ForEach(quick.indices, id: \.self) { i in
                                let q = quick[i]
                                NavigationLink(value: q.1) {
                                    HStack(spacing: 0) {
                                        if let id = q.2 {
                                            CoverView(albumID: id, corner: 0).frame(width: 52, height: 52)
                                        } else {
                                            ZStack {
                                                LinearGradient(colors: [Color(hex: 0x4B3FD6), Theme.accent], startPoint: .topLeading, endPoint: .bottomTrailing)
                                                Image(systemName: "heart.fill").foregroundColor(.white)
                                            }
                                            .frame(width: 52, height: 52)
                                        }
                                        Text(q.0).font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text).lineLimit(2).padding(.horizontal, 10)
                                        Spacer(minLength: 0)
                                    }
                                    .frame(height: 52)
                                    .background(Theme.surface3.opacity(0.7))
                                    .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
                                }
                                .buttonStyle(.plain)
                            }
                        }
                        .padding(.horizontal, 16)
                    }

                    if !mixes.isEmpty {
                        Shelf(title: "Made for you") {
                            ForEach(mixes) { m in MixCard(mix: m) }
                        }
                    }

                    if !app.recentAlbums.isEmpty {
                        Shelf(title: "Recently played") {
                            ForEach(app.recentAlbums.prefix(12)) { a in AlbumCard(album: a) }
                        }
                    }

                    if mostPlayed.count >= 2 {
                        Shelf(title: "Your most played") {
                            ForEach(mostPlayed) { a in AlbumCard(album: a) }
                        }
                    }

                    if !app.albums.isEmpty {
                        Shelf(title: "Recently added") {
                            ForEach(app.newestAlbums.prefix(12)) { a in AlbumCard(album: a) }
                        }
                    }

                    if picks.count >= 3 {
                        Shelf(title: "Picked for today") {
                            ForEach(picks) { a in AlbumCard(album: a) }
                        }
                    }

                    if !app.playlists.isEmpty {
                        Shelf(title: "Your playlists") {
                            ForEach(app.playlists) { p in PlaylistCard(playlist: p) }
                        }
                    }

                    if !app.artistNames.isEmpty {
                        Shelf(title: "Artists") {
                            ForEach(app.artistNames.prefix(12), id: \.self) { n in ArtistCard(name: n) }
                        }
                    }
                }
                .padding(.top, 8)
                .padding(.bottom, 24)
            }
            .refreshable { await app.refresh() }
        }
        .background(Theme.surface)
        .navigationBarHidden(true)
        .task(id: app.stats.log.count * 100000 + app.allTracks.count + (app.offline ? 1 : 0)) {
            mixes = MixEngine.mixes(app, playable: { app.offline ? downloads.has($0) : true })
            mostPlayed = MixEngine.topAlbums(app, 10)
            picks = MixEngine.randomAlbums(app, 10)
        }
    }
}

struct PlaylistCard: View {
    @EnvironmentObject var app: AppModel
    let playlist: PlaylistInfo
    var width: CGFloat = 150

    var body: some View {
        NavigationLink(value: Route.playlist(playlist.id)) {
            VStack(alignment: .leading, spacing: 6) {
                PlaylistArt(playlist: playlist).frame(width: width, height: width)
                Text(playlist.name).font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                Text("\(playlist.trackIDs.count) song\(playlist.trackIDs.count == 1 ? "" : "s")").font(.system(size: 12)).foregroundColor(Theme.text2)
            }
            .frame(width: width, alignment: .leading)
        }
        .buttonStyle(.plain)
    }
}

/// A playlist's picture: a heart for Liked Songs, else its first album's cover.
struct PlaylistArt: View {
    @EnvironmentObject var app: AppModel
    let playlist: PlaylistInfo

    var body: some View {
        if playlist.liked {
            ZStack {
                LinearGradient(colors: [Color(hex: 0x4B3FD6), Theme.accent], startPoint: .topLeading, endPoint: .bottomTrailing)
                Image(systemName: "heart.fill").font(.system(size: 34)).foregroundColor(.white)
            }
            .aspectRatio(1, contentMode: .fit)
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        } else {
            let first = playlist.trackIDs.first.flatMap { app.track($0)?.albumId }
            if first != nil {
                CoverView(albumID: first, corner: 8)
            } else {
                ZStack {
                    Theme.tint(playlist.name)
                    Text(String(playlist.name.prefix(1)).uppercased()).font(.system(size: 28, weight: .bold)).foregroundColor(.white.opacity(0.9))
                }
                .aspectRatio(1, contentMode: .fit)
                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            }
        }
    }
}

// MARK: - Library

struct LibraryView: View {
    enum Tab: String, CaseIterable { case albums = "Albums", artists = "Artists", songs = "Songs", playlists = "Playlists", downloaded = "Saved" }
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var downloads: DownloadStore
    @EnvironmentObject var player: PlayerModel
    @State private var tab: Tab = .albums
    @State private var allArtists = false
    @State private var creating = false
    @State private var newName = ""

    var body: some View {
        VStack(spacing: 0) {
            TopBar()
            Text("Your Library").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)
                .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 16).padding(.top, 4)
            Picker("", selection: $tab) {
                ForEach(Tab.allCases, id: \.self) { t in Text(t.rawValue).tag(t) }
            }
            .pickerStyle(.segmented)
            .padding(16)

            ScrollView {
                switch tab {
                case .albums: albums
                case .artists: artists
                case .songs: songs
                case .playlists: playlists
                case .downloaded: saved
                }
            }
            .refreshable { await app.refresh() }
        }
        .background(Theme.surface)
        .navigationBarHidden(true)
        .alert("New playlist", isPresented: $creating) {
            TextField("Name", text: $newName)
            Button("Create") { app.createPlaylist(newName); newName = "" }
            Button("Cancel", role: .cancel) { newName = "" }
        }
    }

    private var albums: some View {
        let cols = [GridItem(.flexible(), spacing: 14), GridItem(.flexible(), spacing: 14)]
        return LazyVGrid(columns: cols, spacing: 18) {
            ForEach(app.albums.sorted { $0.title.localizedCaseInsensitiveCompare($1.title) == .orderedAscending }) { a in
                NavigationLink(value: Route.album(a.id)) {
                    VStack(alignment: .leading, spacing: 6) {
                        CoverView(albumID: a.id)
                        Text(a.title).font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                        Text(a.artist).font(.system(size: 12)).foregroundColor(Theme.text2).lineLimit(1)
                    }
                }
                .buttonStyle(.plain)
            }
        }
        .padding(16)
    }

    private var shownArtists: [String] {
        allArtists ? app.artistNames : app.artistNames.filter { app.isFollowing($0) }
    }

    private var artists: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                chip("Following", on: !allArtists) { allArtists = false }
                chip("All artists", on: allArtists) { allArtists = true }
            }
            .padding(.horizontal, 16)
            if shownArtists.isEmpty {
                Text("You're not following any artists yet. Open an artist and press Follow.")
                    .font(.system(size: 15)).foregroundColor(Theme.text2).padding(.horizontal, 16).padding(.top, 8)
            }
            LazyVStack(spacing: 0) {
                ForEach(shownArtists, id: \.self) { n in
                    NavigationLink(value: Route.artist(n)) {
                        HStack(spacing: 14) {
                            ArtistPic(name: n).frame(width: 56, height: 56)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(n).font(.system(size: 16)).foregroundColor(Theme.text).lineLimit(1)
                                let c = app.albums(by: n).count
                                Text(c == 0 ? "Appears on albums" : "\(c) album\(c == 1 ? "" : "s")").font(.system(size: 13)).foregroundColor(Theme.text2)
                            }
                            Spacer()
                            Image(systemName: "chevron.right").font(.system(size: 13)).foregroundColor(Theme.text3)
                        }
                        .padding(.horizontal, 16).padding(.vertical, 6)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    private func chip(_ title: String, on: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title).font(.system(size: 14, weight: .semibold))
                .padding(.horizontal, 16).padding(.vertical, 8)
                .background(on ? Theme.text : Theme.surface3)
                .foregroundColor(on ? Theme.surface : Theme.text)
                .clipShape(Capsule())
        }
    }

    private var songs: some View {
        let list = Array(app.allTracks.sorted { $0.title.localizedCaseInsensitiveCompare($1.title) == .orderedAscending }.prefix(2000))
        let ids = list.map { $0.id }
        return LazyVStack(spacing: 0) {
            ForEach(Array(list.enumerated()), id: \.element.id) { i, t in
                TrackRow(track: t, showCover: true) { player.play(ids, startAt: i) }
            }
        }
    }

    /// What is kept on this phone.
    private var saved: some View {
        let tracks = app.allTracks.filter { downloads.has($0.id) }.sorted { a, b in
            if a.albumId != b.albumId { return a.albumId < b.albumId }
            if a.disc != b.disc { return a.disc < b.disc }
            return a.no < b.no
        }
        let ids = tracks.map { $0.id }
        return VStack(alignment: .leading, spacing: 10) {
            Text("\(tracks.count) song\(tracks.count == 1 ? "" : "s") · \(formatBytes(downloads.bytes))")
                .font(.system(size: 14)).foregroundColor(Theme.text2).padding(.horizontal, 16)
            if !downloads.pending.isEmpty {
                Text("Downloading \(downloads.pending.count) more…").font(.system(size: 13)).foregroundColor(Theme.text2).padding(.horizontal, 16)
            }
            if let p = downloads.problem { ProblemBox(text: p).padding(.horizontal, 16) }
            if tracks.isEmpty {
                Text("Nothing saved yet. Open an album or playlist and tap the download button to keep it here for listening without a connection.")
                    .font(.system(size: 15)).foregroundColor(Theme.text2).padding(.horizontal, 16).padding(.top, 6)
            } else {
                HStack(spacing: 12) {
                    PillButton(title: "Play", icon: "play.fill") { player.play(ids) }
                    PillButton(title: "Remove all", icon: "trash", primary: false) { downloads.removeAll() }
                }
                .padding(.horizontal, 16)
                LazyVStack(spacing: 0) {
                    ForEach(Array(tracks.enumerated()), id: \.element.id) { i, t in
                        TrackRow(track: t, showCover: true) { player.play(ids, startAt: i) }
                    }
                }
            }
        }
    }

    private var playlists: some View {
        VStack(alignment: .leading, spacing: 8) {
            PillButton(title: "New playlist", icon: "plus", primary: false) { creating = true }
                .padding(.horizontal, 16)
            LazyVStack(spacing: 0) {
                ForEach(app.playlists) { p in
                    NavigationLink(value: Route.playlist(p.id)) {
                        HStack(spacing: 14) {
                            PlaylistArt(playlist: p).frame(width: 56, height: 56)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(p.name).font(.system(size: 16)).foregroundColor(Theme.text).lineLimit(1)
                                Text("\(p.trackIDs.count) song\(p.trackIDs.count == 1 ? "" : "s")").font(.system(size: 13)).foregroundColor(Theme.text2)
                            }
                            Spacer()
                            Image(systemName: "chevron.right").font(.system(size: 13)).foregroundColor(Theme.text3)
                        }
                        .padding(.horizontal, 16).padding(.vertical, 6)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }
}

// MARK: - Album, artist, playlist

struct AlbumView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    let id: String

    var body: some View {
        ScrollView {
            if let album = app.album(id) {
                let tracks = app.tracks(of: album)
                let ids = tracks.map { $0.id }
                VStack(spacing: 14) {
                    CoverView(albumID: album.id, corner: 10).frame(width: 240, height: 240).padding(.top, 8)
                    VStack(spacing: 4) {
                        Text(album.title).font(.system(size: 24, weight: .bold)).foregroundColor(Theme.text).multilineTextAlignment(.center)
                        NavigationLink(value: Route.artist(album.artist)) {
                            Text(album.artist).font(.system(size: 16, weight: .semibold)).foregroundColor(Theme.accent)
                        }
                        let meta = [album.year.map { String($0) }, album.genre, "\(tracks.count) songs"].compactMap { $0 }.joined(separator: " · ")
                        Text(meta).font(.system(size: 13)).foregroundColor(Theme.text2)
                    }
                    .padding(.horizontal, 16)
                    HStack(spacing: 12) {
                        PillButton(title: "Play", icon: "play.fill") { player.play(ids) }
                        PillButton(title: "Shuffle", icon: "shuffle", primary: false) { player.play(ids, shuffled: true) }
                        DownloadButton(ids: ids)
                    }
                    LazyVStack(spacing: 0) {
                        ForEach(Array(tracks.enumerated()), id: \.element.id) { i, t in
                            TrackRow(track: t, number: i + 1) { player.play(ids, startAt: i) }
                        }
                    }
                    .padding(.top, 6)
                }
                .padding(.bottom, 24)
            } else {
                Text("That album isn't in the library any more.").foregroundColor(Theme.text2).padding(24)
            }
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
    }
}

struct ArtistView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    let name: String
    @State private var more = false

    var body: some View {
        let albums = app.albums(by: name).sorted { ($0.year ?? 0) > ($1.year ?? 0) }
        let ids = albums.flatMap { app.tracks(of: $0).map { $0.id } }
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(spacing: 16) {
                    ArtistPic(name: name, corner: 16).frame(width: 120, height: 120)
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Artist").font(.system(size: 13)).foregroundColor(Theme.text2)
                        Text(name).font(.system(size: 26, weight: .bold)).foregroundColor(Theme.text)
                        Text("\(albums.count) album\(albums.count == 1 ? "" : "s")").font(.system(size: 13)).foregroundColor(Theme.text2)
                    }
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 16)
                .padding(.top, 8)

                HStack(spacing: 12) {
                    if !ids.isEmpty {
                        PillButton(title: "Play", icon: "play.fill") { player.play(ids) }
                        PillButton(title: "Shuffle", icon: "shuffle", primary: false) { player.play(ids, shuffled: true) }
                    }
                    PillButton(title: app.isFollowing(name) ? "Following" : "Follow", icon: app.isFollowing(name) ? "checkmark" : "plus", primary: false) { app.toggleFollow(name) }
                }
                .padding(.horizontal, 16)

                if let bio = app.info(for: name)?.bio, !bio.isEmpty {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(bio).font(.system(size: 14)).foregroundColor(Theme.text2).lineLimit(more ? nil : 4)
                        Button(more ? "Show less" : "Read more") { more.toggle() }.font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text)
                        Text("From Wikipedia").font(.system(size: 11)).foregroundColor(Theme.text3)
                    }
                    .padding(.horizontal, 16)
                }

                if !albums.isEmpty {
                    SectionTitle(text: "Albums")
                    LazyVGrid(columns: [GridItem(.flexible(), spacing: 14), GridItem(.flexible(), spacing: 14)], spacing: 18) {
                        ForEach(albums) { a in
                            NavigationLink(value: Route.album(a.id)) {
                                VStack(alignment: .leading, spacing: 6) {
                                    CoverView(albumID: a.id)
                                    Text(a.title).font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                                    Text(a.year.map { String($0) } ?? "").font(.system(size: 12)).foregroundColor(Theme.text2)
                                }
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.horizontal, 16)
                }
            }
            .padding(.bottom, 24)
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
        .task { app.loadInfo(name) }
    }
}

struct PlaylistView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @Environment(\.dismiss) private var dismiss
    let id: String
    @State private var confirmDelete = false

    var body: some View {
        ScrollView {
            if let p = app.playlist(id) {
                let tracks = p.trackIDs.compactMap { app.track($0) }
                let ids = tracks.map { $0.id }
                VStack(spacing: 14) {
                    PlaylistArt(playlist: p).frame(width: 220, height: 220).padding(.top, 8)
                    VStack(spacing: 4) {
                        Text(p.name).font(.system(size: 24, weight: .bold)).foregroundColor(Theme.text).multilineTextAlignment(.center)
                        if !p.description.isEmpty { Text(p.description).font(.system(size: 14)).foregroundColor(Theme.text2).multilineTextAlignment(.center) }
                        Text("\(tracks.count) song\(tracks.count == 1 ? "" : "s")").font(.system(size: 13)).foregroundColor(Theme.text2)
                    }
                    .padding(.horizontal, 16)
                    if !ids.isEmpty {
                        HStack(spacing: 12) {
                            PillButton(title: "Play", icon: "play.fill") { player.play(ids) }
                            PillButton(title: "Shuffle", icon: "shuffle", primary: false) { player.play(ids, shuffled: true) }
                            DownloadButton(ids: ids)
                        }
                    }
                    LazyVStack(spacing: 0) {
                        ForEach(Array(tracks.enumerated()), id: \.offset) { i, t in
                            TrackRow(track: t, showCover: true, onRemove: { remove(p, at: i, track: t) }) { player.play(ids, startAt: i) }
                        }
                    }
                    if !p.liked {
                        Button("Delete playlist", role: .destructive) { confirmDelete = true }
                            .font(.system(size: 14)).padding(.top, 12)
                    }
                }
                .padding(.bottom, 24)
            } else {
                Text("That playlist isn't here any more.").foregroundColor(Theme.text2).padding(24)
            }
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
        .confirmationDialog("Delete this playlist?", isPresented: $confirmDelete, titleVisibility: .visible) {
            Button("Delete", role: .destructive) { app.deletePlaylist(id); dismiss() }
            Button("Cancel", role: .cancel) {}
        }
    }

    private func remove(_ p: PlaylistInfo, at i: Int, track: Track) {
        if p.liked { app.toggleLike(track.id) } else { app.removeTrack(at: i, from: p.id) }
    }
}

// MARK: - Search

struct SearchView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @State private var query = ""
    @FocusState private var focused: Bool

    var body: some View {
        let r = app.search(query)
        VStack(spacing: 0) {
            TopBar()
            Text("Search").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)
                .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 16).padding(.top, 4)
            HStack(spacing: 10) {
                Image(systemName: "magnifyingglass").foregroundColor(Theme.text2)
                TextField("Artists, albums, songs", text: $query)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .focused($focused)
                    .foregroundColor(Theme.text)
                if !query.isEmpty {
                    Button { query = "" } label: { Image(systemName: "xmark.circle.fill").foregroundColor(Theme.text3) }
                }
            }
            .padding(12)
            .background(Theme.surface3)
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            .padding(16)

            ScrollView {
                VStack(alignment: .leading, spacing: 22) {
                    if query.trimmingCharacters(in: .whitespaces).isEmpty {
                        Text("Find anything in your library.").font(.system(size: 15)).foregroundColor(Theme.text2).padding(.top, 40).frame(maxWidth: .infinity)
                    } else {
                        if r.artists.isEmpty && r.albums.isEmpty && r.tracks.isEmpty {
                            Text("Nothing in your library for \"\(query)\".").font(.system(size: 15)).foregroundColor(Theme.text2).padding(.horizontal, 16)
                        }
                        if !r.artists.isEmpty {
                            SectionTitle(text: "Artists")
                            LazyVStack(spacing: 0) {
                                ForEach(r.artists, id: \.self) { n in
                                    NavigationLink(value: Route.artist(n)) {
                                        HStack(spacing: 14) {
                                            ArtistPic(name: n).frame(width: 48, height: 48)
                                            Text(n).font(.system(size: 16)).foregroundColor(Theme.text)
                                            Spacer()
                                        }
                                        .padding(.horizontal, 16).padding(.vertical, 5)
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }
                        if !r.albums.isEmpty {
                            SectionTitle(text: "Albums")
                            LazyVStack(spacing: 0) {
                                ForEach(r.albums) { a in
                                    NavigationLink(value: Route.album(a.id)) {
                                        HStack(spacing: 14) {
                                            CoverView(albumID: a.id, corner: 6).frame(width: 48, height: 48)
                                            VStack(alignment: .leading, spacing: 2) {
                                                Text(a.title).font(.system(size: 16)).foregroundColor(Theme.text).lineLimit(1)
                                                Text(a.artist).font(.system(size: 13)).foregroundColor(Theme.text2).lineLimit(1)
                                            }
                                            Spacer()
                                        }
                                        .padding(.horizontal, 16).padding(.vertical, 5)
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }
                        if !r.tracks.isEmpty {
                            SectionTitle(text: "Songs")
                            let ids = r.tracks.map { $0.id }
                            LazyVStack(spacing: 0) {
                                ForEach(Array(r.tracks.enumerated()), id: \.element.id) { i, t in
                                    TrackRow(track: t, showCover: true) { player.play(ids, startAt: i) }
                                }
                            }
                        }
                        GetMoreSection(query: query)
                    }
                }
                .padding(.bottom, 24)
            }
            .scrollDismissesKeyboard(.interactively)
        }
        .background(Theme.surface)
        .navigationBarHidden(true)
    }
}

// MARK: - Account

struct AccountView: View {
    @EnvironmentObject var app: AppModel

    private let qualities: [(String, String)] = [("original", "Original"), ("high", "High"), ("normal", "Normal"), ("low", "Data saver")]

    private func accountRow(_ icon: String, _ title: String, _ sub: String) -> some View {
        HStack(spacing: 14) {
            Image(systemName: icon).font(.system(size: 18)).foregroundColor(Theme.accent).frame(width: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                Text(sub).font(.system(size: 12)).foregroundColor(Theme.text2)
            }
            Spacer()
            Image(systemName: "chevron.right").font(.system(size: 13)).foregroundColor(Theme.text3)
        }
        .padding(12)
        .background(Theme.surface2)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                Text("Account").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)
                if let u = app.user {
                    HStack(spacing: 14) {
                        ArtistPic(name: u.name, corner: 28, photo: false).frame(width: 56, height: 56)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(u.name).font(.system(size: 18, weight: .semibold)).foregroundColor(Theme.text)
                            Text(u.admin ? "Admin" : "Member").font(.system(size: 13)).foregroundColor(Theme.text2)
                        }
                    }
                }
                VStack(alignment: .leading, spacing: 4) {
                    Text("Server").font(.system(size: 13)).foregroundColor(Theme.text2)
                    Text(app.serverURL).font(.system(size: 15)).foregroundColor(Theme.text)
                }
                VStack(alignment: .leading, spacing: 8) {
                    Text("Appearance").font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                    Picker("", selection: Binding(get: { app.appearance }, set: { app.setAppearance($0) })) {
                        Text("System").tag("system")
                        Text("Dark").tag("dark")
                        Text("Light").tag("light")
                    }
                    .pickerStyle(.segmented)
                }
                if app.me?.transcoding == true {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Streaming quality").font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                        Text("Smaller files use less data. Original sends the file as it is.").font(.system(size: 13)).foregroundColor(Theme.text2)
                        Picker("", selection: Binding(get: { app.quality }, set: { app.setQuality($0) })) {
                            ForEach(qualities, id: \.0) { q in Text(q.1).tag(q.0) }
                        }
                        .pickerStyle(.segmented)
                    }
                }
                NavigationLink(value: Route.playback) { accountRow("slider.horizontal.3", "Playback", "Crossfade, volume leveling, equalizer") }.buttonStyle(.plain)
                if let y = app.rewindYears.last {
                    NavigationLink(value: Route.rewind(y)) { accountRow("gobackward", "Rewind \(String(y))", "Your year in music") }.buttonStyle(.plain)
                }
                PillButton(title: "Sign out", icon: "rectangle.portrait.and.arrow.right", primary: false) { Task { await app.signOut() } }
                PillButton(title: "Switch server", icon: "arrow.triangle.2.circlepath", primary: false) { app.switchServer() }
                Text("Volvix Home for iPhone · native app (preview)").font(.system(size: 12)).foregroundColor(Theme.text3).padding(.top, 8)
            }
            .padding(16)
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
    }
}
