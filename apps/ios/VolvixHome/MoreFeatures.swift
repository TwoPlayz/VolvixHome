import SwiftUI

// MARK: - Notifications

struct NotificationsView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var party: PartyHub
    @State private var joining: NoteItem?
    /// the ones that were new when the page opened keep their dot while you read
    @State private var fresh: Set<String> = []

    private func icon(_ kind: String) -> String {
        switch kind {
        case "playlist_invite": return "person.2"
        case "playlist_joined": return "checkmark"
        case "party_invite": return "person.3"
        case "request_ready": return "arrow.down.circle"
        default: return "bell"
        }
    }

    @ViewBuilder
    private func actions(_ n: NoteItem) -> some View {
        switch n.kind {
        case "playlist_invite":
            if let d = n.done, !d.isEmpty {
                Text(d == "accepted" ? "Joined" : "Declined").font(.system(size: 12, weight: .semibold)).foregroundColor(Theme.text2)
            } else if let id = n.playlistID {
                HStack(spacing: 10) {
                    Button { app.answerInvite(id, accept: true) } label: {
                        Text("Accept").font(.system(size: 13, weight: .semibold)).foregroundColor(.white).padding(.horizontal, 16).padding(.vertical, 6).background(Theme.accent).clipShape(Capsule())
                    }
                    Button { app.answerInvite(id, accept: false) } label: {
                        Text("Decline").font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text).padding(.horizontal, 16).padding(.vertical, 6).background(Theme.surface3).clipShape(Capsule())
                    }
                }
            }
        case "playlist_joined":
            if let id = n.playlistID {
                NavigationLink(value: Route.playlist(id)) { pill("Open") }.buttonStyle(.plain)
            }
        case "party_invite":
            if let d = n.done, !d.isEmpty {
                Text(d == "joined" ? "Joined" : "Over").font(.system(size: 12, weight: .semibold)).foregroundColor(Theme.text2)
            } else if n.partyID != nil {
                Button { joining = n } label: {
                    Text("Join").font(.system(size: 13, weight: .semibold)).foregroundColor(.white).padding(.horizontal, 16).padding(.vertical, 6).background(Theme.accent).clipShape(Capsule())
                }
            }
        case "request_ready":
            if let t = n.targetID {
                if n.targetKind == "artist" {
                    NavigationLink(value: Route.artist(t)) { pill("Open") }.buttonStyle(.plain)
                } else if app.album(t) != nil {
                    NavigationLink(value: Route.album(t)) { pill("Open") }.buttonStyle(.plain)
                } else {
                    Button { Task { await app.refresh() } } label: { pill("Show it") }
                }
            }
        default:
            EmptyView()
        }
    }

    private func pill(_ text: String) -> some View {
        Text(text).font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text).padding(.horizontal, 16).padding(.vertical, 6).background(Theme.surface3).clipShape(Capsule())
    }

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text("Notifications").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)
                    Spacer()
                    if !app.notes.isEmpty {
                        Button("Mark all as read") { fresh = []; app.markNotesRead() }.font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.accent)
                    }
                }
                .padding(.bottom, 6)
                if app.notes.isEmpty {
                    Text("Nothing new. Playlist invites, party invites and music you requested show up here.").font(.system(size: 15)).foregroundColor(Theme.text2)
                }
                ForEach(app.notes) { n in
                    HStack(alignment: .top, spacing: 12) {
                        Image(systemName: icon(n.kind)).font(.system(size: 16)).foregroundColor(Theme.text2).frame(width: 26, height: 26)
                        VStack(alignment: .leading, spacing: 6) {
                            Text(n.text).font(.system(size: 15)).foregroundColor(Theme.text)
                            Text(ago(n.at)).font(.system(size: 12)).foregroundColor(Theme.text2)
                            actions(n)
                        }
                        Spacer(minLength: 0)
                        if !n.read || fresh.contains(n.id) { Circle().fill(Theme.accent).frame(width: 8, height: 8).padding(.top, 6) }
                    }
                    .padding(12)
                    .background(Theme.surface2)
                    .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                }
            }
            .padding(16)
        }
        .background(Theme.surface.ignoresSafeArea())
        .navigationBarTitleDisplayMode(.inline)
        .sheet(item: $joining) { n in
            JoinPartySheet(partyID: n.partyID ?? "", host: n.from ?? "the host").environmentObject(party)
        }
        .onAppear { fresh = Set(app.notes.filter { !$0.read }.map { $0.id }) }
        .task {
            // they stay marked as new while you look, and count as read once you leave them
            try? await Task.sleep(nanoseconds: 1_500_000_000)
            app.markNotesRead()
        }
    }

    private func ago(_ seconds: Int) -> String {
        let d = Date(timeIntervalSince1970: TimeInterval(seconds))
        let f = RelativeDateTimeFormatter()
        f.unitsStyle = .short
        return f.localizedString(for: d, relativeTo: Date())
    }
}

// MARK: - Search

struct SearchHits {
    var artists: [String] = []
    var albums: [Album] = []
    var tracks: [Track] = []
    var playlists: [PlaylistInfo] = []
}

struct SearchView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @State private var query = ""
    @State private var type = "all"
    @State private var hits = SearchHits()
    @FocusState private var focused: Bool

    private let types = [("all", "All"), ("artists", "Artists"), ("albums", "Albums"), ("songs", "Songs"), ("playlists", "Playlists")]

    private func norm(_ s: String) -> String { s.folding(options: [.diacriticInsensitive, .caseInsensitive], locale: nil) }

    private func find(_ q: String) -> SearchHits {
        let n = norm(q).trimmingCharacters(in: .whitespaces)
        let words = n.split(separator: " ").map(String.init)
        func hit(_ text: String) -> Bool { let t = norm(text); return words.allSatisfy { t.contains($0) } }
        func starts(_ s: String) -> Int { norm(s).hasPrefix(n) ? 0 : 1 }
        var r = SearchHits()
        r.albums = app.albums.filter { hit($0.title + " " + $0.artist + " " + ($0.year.map { String($0) } ?? "") + " " + ($0.genre ?? "")) }.sorted { starts($0.title) < starts($1.title) }
        r.artists = app.artistNames.filter { hit($0) }.sorted { starts($0) < starts($1) }
        r.tracks = app.allTracks.filter { hit($0.title + " " + $0.artist) }.sorted { starts($0.title) < starts($1.title) }
        r.playlists = (app.playlists + app.sharedLists + app.householdLists).filter { hit($0.name + " " + $0.description) }
        return r
    }

    private func show(_ t: String) -> Bool { type == "all" || type == t }

    var body: some View {
        let typed = !query.trimmingCharacters(in: .whitespaces).isEmpty
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 22) {

                    if !typed {
                        if app.albums.isEmpty {
                            Text("Music added to the server shows up here.").font(.system(size: 15)).foregroundColor(Theme.text2).padding(.top, 30).frame(maxWidth: .infinity)
                        } else {
                            SectionTitle(text: "Everything on your server")
                            LazyVGrid(columns: [GridItem(.flexible(), spacing: 14), GridItem(.flexible(), spacing: 14)], spacing: 18) {
                                ForEach(app.newestAlbums.prefix(60)) { a in
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
                            .padding(.horizontal, 16)
                        }
                    } else {
                        let all = type == "all"
                        if hits.artists.isEmpty && hits.albums.isEmpty && hits.tracks.isEmpty && hits.playlists.isEmpty {
                            Text("Nothing in your library for \"\(query)\".").font(.system(size: 15)).foregroundColor(Theme.text2).padding(.horizontal, 16)
                        }
                        if show("artists") && !hits.artists.isEmpty {
                            SectionTitle(text: "Artists")
                            LazyVStack(spacing: 0) {
                                ForEach(Array(hits.artists.prefix(all ? 6 : 200)), id: \.self) { n in
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
                            if all && hits.artists.count > 6 { more("artists") }
                        }
                        if show("songs") && !hits.tracks.isEmpty {
                            SectionTitle(text: "Songs")
                            let list = Array(hits.tracks.prefix(all ? 5 : 200))
                            let ids = list.map { $0.id }
                            LazyVStack(spacing: 0) {
                                ForEach(Array(list.enumerated()), id: \.element.id) { i, t in
                                    TrackRow(track: t, showCover: true) { player.play(ids, startAt: i, from: "Search") }
                                }
                            }
                            if all && hits.tracks.count > 5 { more("songs") }
                        }
                        if show("albums") && !hits.albums.isEmpty {
                            SectionTitle(text: "Albums")
                            LazyVStack(spacing: 0) {
                                ForEach(Array(hits.albums.prefix(all ? 12 : 200))) { a in
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
                            if all && hits.albums.count > 12 { more("albums") }
                        }
                        if show("playlists") && !hits.playlists.isEmpty {
                            SectionTitle(text: "Playlists")
                            LazyVStack(spacing: 0) {
                                ForEach(hits.playlists) { p in
                                    NavigationLink(value: Route.playlist(p.id)) {
                                        HStack(spacing: 14) {
                                            PlaylistArt(playlist: p).frame(width: 48, height: 48)
                                            VStack(alignment: .leading, spacing: 2) {
                                                Text(p.name).font(.system(size: 16)).foregroundColor(Theme.text).lineLimit(1)
                                                Text(p.mine ? "Playlist" : "Playlist · \(app.personName(p.owner))").font(.system(size: 13)).foregroundColor(Theme.text2)
                                            }
                                            Spacer()
                                        }
                                        .padding(.horizontal, 16).padding(.vertical, 5)
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }
                        if type != "playlists" { GetMoreSection(query: query) }
                    }
                }
                .padding(.bottom, 24)
            }
            .scrollDismissesKeyboard(.interactively)
            .safeAreaInset(edge: .top, spacing: 0) {
                VStack(spacing: 0) {
                    TopBar()
            Text("Search").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)
                .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 16).padding(.top, 4)
            HStack(spacing: 10) {
                Image(systemName: "magnifyingglass").foregroundColor(Theme.text2)
                TextField("Artists, albums, songs, playlists", text: $query)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .focused($focused)
                    .foregroundColor(Theme.text)
                if !query.isEmpty {
                    Button { query = "" } label: { Image(systemName: "xmark.circle.fill").foregroundColor(Theme.text3) }
                }
            }
            .padding(.horizontal, 16).padding(.vertical, 12)
            .glass(Capsule())
            .padding(.horizontal, 16).padding(.top, 6)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(types, id: \.0) { t in
                        Button { type = t.0 } label: {
                            Text(t.1).font(.system(size: 14, weight: .semibold)).padding(.horizontal, 16).padding(.vertical, 8)
                                .foregroundColor(type == t.0 ? Theme.surface : Theme.text).glass(Capsule(), tint: type == t.0 ? Theme.text : nil)
                        }
                    }
                }
                .padding(.horizontal, 16)
            }
            .padding(.vertical, 10)
                }
            }
        }
        .background(Theme.surface.ignoresSafeArea())
        .navigationBarHidden(true)
        .task(id: query) {
            if query.trimmingCharacters(in: .whitespaces).isEmpty { hits = SearchHits(); return }
            try? await Task.sleep(nanoseconds: 140_000_000)
            if Task.isCancelled { return }
            hits = find(query)
        }
    }

    private func more(_ t: String) -> some View {
        Button { type = t } label: { Text("Show all").font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.accent) }
            .padding(.horizontal, 16)
    }
}

// MARK: - Artist

struct ArtistView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    let name: String
    @State private var more = false
    @State private var releasesID: String?
    @State private var allSongs = false

    var body: some View {
        let albums = app.albums(by: name).sorted { ($0.year ?? 0) > ($1.year ?? 0) }
        let appears = app.appearsOn(name).sorted { ($0.year ?? 0) > ($1.year ?? 0) }
        let songs = songIDs(albums + appears)
        let own = albums.flatMap { app.tracks(of: $0).map { $0.id } }
        let playable = songs.isEmpty ? own : songs
        return ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(spacing: 16) {
                    ArtistPic(name: name, corner: 16).frame(width: 120, height: 120)
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Artist").font(.system(size: 13)).foregroundColor(Theme.text2)
                        Text(name).font(.system(size: 26, weight: .bold)).foregroundColor(Theme.text)
                        Text([albums.isEmpty ? "" : "\(albums.count) album\(albums.count == 1 ? "" : "s")", "\(songs.count) song\(songs.count == 1 ? "" : "s")"].filter { !$0.isEmpty }.joined(separator: " · "))
                            .font(.system(size: 13)).foregroundColor(Theme.text2)
                    }
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 16)
                .padding(.top, 8)

                HStack(spacing: 12) {
                    if !playable.isEmpty {
                        PillButton(title: "Play", icon: "play.fill") { player.play(playable, from: name) }
                        PillButton(title: "Shuffle", icon: "shuffle", primary: false) { player.play(playable, shuffled: true, from: name) }
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

                if !songs.isEmpty {
                    SectionTitle(text: "Songs")
                    let shown = Array(songs.prefix(allSongs ? 200 : 10))
                    LazyVStack(spacing: 0) {
                        ForEach(Array(shown.enumerated()), id: \.offset) { i, id in
                            if let t = app.track(id) { TrackRow(track: t, showCover: true) { player.play(songs, startAt: i, from: name) } }
                        }
                    }
                    if songs.count > 10 {
                        Button { allSongs.toggle() } label: { Text(allSongs ? "Show fewer" : "Show all \(songs.count)").font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.accent) }
                            .padding(.horizontal, 16)
                    }
                }

                if !albums.isEmpty {
                    SectionTitle(text: "Albums")
                    grid(albums)
                }
                if !appears.isEmpty {
                    SectionTitle(text: "Appears on")
                    grid(appears)
                }
                if let id = releasesID {
                    NavigationLink(value: Route.discoverArtist(id)) {
                        HStack(spacing: 12) {
                            Image(systemName: "arrow.down.circle").foregroundColor(Theme.accent)
                            VStack(alignment: .leading, spacing: 2) {
                                Text("Get more by \(name)").font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                                Text("See all releases, including what isn't on this server yet").font(.system(size: 12)).foregroundColor(Theme.text2)
                            }
                            Spacer()
                            Image(systemName: "chevron.right").font(.system(size: 13)).foregroundColor(Theme.text3)
                        }
                        .padding(12).background(Theme.surface2).clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous)).padding(.horizontal, 16)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.bottom, 24)
        }
        .background(Theme.surface.ignoresSafeArea())
        .navigationBarTitleDisplayMode(.inline)
        .task {
            app.loadInfo(name)
            guard app.me?.lidarrConfigured == true, let api = app.api, !app.offline else { return }
            if let r = try? await api.find(name, kind: "artists"),
               let hit = r.artists.first(where: { ($0.name ?? "").lowercased() == name.lowercased() && $0.foreignId != nil }) {
                releasesID = hit.foreignId
            }
        }
    }

    private func songIDs(_ list: [Album]) -> [String] {
        var out: [String] = []
        for a in list {
            for t in app.tracks(of: a) where t.artist.lowercased() == name.lowercased() { out.append(t.id) }
        }
        return out
    }

    private func grid(_ albums: [Album]) -> some View {
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
