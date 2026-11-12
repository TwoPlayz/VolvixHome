import SwiftUI

// MARK: - Rewind

/// Your year in music. It opens on January 1 for the year that just ended, built from the plays recorded in your data.
struct RewindBanner: View {
    @EnvironmentObject var app: AppModel

    var body: some View {
        let year = Calendar.current.component(.year, from: Date()) - 1
        if app.rewindYears.contains(year) {
            NavigationLink(value: Route.rewind(year)) {
                HStack(spacing: 14) {
                    Text(String(year)).font(.system(size: 16, weight: .heavy)).foregroundColor(.white)
                        .padding(.horizontal, 12).padding(.vertical, 10)
                        .background(LinearGradient(colors: [Color(hex: 0x4B3FD6), Theme.accent], startPoint: .topLeading, endPoint: .bottomTrailing))
                        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Your \(String(year)) Rewind is ready").font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                        Text("See what you played the most").font(.system(size: 13)).foregroundColor(Theme.text2)
                    }
                    Spacer(minLength: 0)
                    Image(systemName: "chevron.right").foregroundColor(Theme.text3)
                }
                .padding(14)
                .background(Theme.surface2)
                .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                .padding(.horizontal, 16)
            }
            .buttonStyle(.plain)
        }
    }
}

extension AppModel {
    /// Finished years you have plays for.
    var rewindYears: [Int] {
        let now = Calendar.current.component(.year, from: Date())
        return stats.sortedYears.filter { $0 < now }
    }
}

struct RewindView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    let year: Int

    private struct Summary {
        var songs: [(Track, Int)] = []
        var artists: [(String, Int)] = []
        var albums: [(String, Int)] = []
        var genres: [(String, Int)] = []
        var plays = 0
        var minutes = 0
    }

    private func summary() -> Summary {
        var s = Summary()
        guard let y = app.stats.years[year] else { return s }
        var artists: [String: Int] = [:], albums: [String: Int] = [:], genres: [String: Int] = [:]
        for (id, n) in y.plays {
            guard let t = app.track(id) else { continue }
            s.plays += n
            s.songs.append((t, n))
            artists[t.artist, default: 0] += n
            if let a = app.album(t.albumId) {
                albums[a.id, default: 0] += n
                if let g = a.genre, !g.isEmpty { genres[g, default: 0] += n }
            }
        }
        s.songs.sort { $0.1 > $1.1 }
        s.artists = artists.sorted { $0.value > $1.value }.map { ($0.key, $0.value) }
        s.albums = albums.sorted { $0.value > $1.value }.map { ($0.key, $0.value) }
        s.genres = genres.sorted { $0.value > $1.value }.map { ($0.key, $0.value) }
        s.minutes = Int((Double(y.sec) / 60).rounded())
        return s
    }

    var body: some View {
        let r = summary()
        ScrollView {
            if r.plays == 0 {
                let thisYear = Calendar.current.component(.year, from: Date())
                VStack(spacing: 8) {
                    if year >= thisYear {
                        Text("Your \(String(year)) Rewind opens on January 1, \(String(year + 1))").font(.system(size: 18, weight: .semibold)).foregroundColor(Theme.text).multilineTextAlignment(.center)
                        let n = app.stats.years[year]?.plays.count ?? 0
                        Text(n > 0 ? "It's building as you listen: \(n) songs so far." : "Play some music and it starts building.").font(.system(size: 14)).foregroundColor(Theme.text2)
                    } else {
                        Text("No plays in \(String(year))").font(.system(size: 18, weight: .semibold)).foregroundColor(Theme.text)
                        Text("Nothing was recorded that year.").font(.system(size: 14)).foregroundColor(Theme.text2)
                    }
                }
                .padding(40)
            } else {
                VStack(alignment: .leading, spacing: 22) {
                    VStack(spacing: 8) {
                        Text(String(year)).font(.system(size: 58, weight: .heavy)).foregroundColor(.white)
                        Text("Your year in music").font(.system(size: 18, weight: .semibold)).foregroundColor(.white.opacity(0.9))
                        HStack(spacing: 0) {
                            stat(r.minutes.formatted(), "minutes listened")
                            stat(r.plays.formatted(), "plays")
                            stat(r.songs.count.formatted(), "different songs")
                        }
                        .padding(.top, 10)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 28)
                    .background(LinearGradient(colors: [Color(hex: 0x4B3FD6), Theme.accent], startPoint: .topLeading, endPoint: .bottomTrailing))

                    HStack {
                        Text("Top songs").font(.system(size: 22, weight: .bold)).foregroundColor(Theme.text)
                        Spacer()
                        PillButton(title: "Play them", icon: "play.fill", primary: false) { player.play(r.songs.prefix(5).map { $0.0.id }) }
                    }
                    .padding(.horizontal, 16)
                    VStack(spacing: 0) {
                        ForEach(Array(r.songs.prefix(5).enumerated()), id: \.offset) { i, s in
                            HStack(spacing: 12) {
                                Text("\(i + 1)").font(.system(size: 18, weight: .bold)).foregroundColor(Theme.text3).frame(width: 26)
                                CoverView(albumID: s.0.albumId, corner: 6).frame(width: 46, height: 46)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(s.0.title).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                                    Text(s.0.artist).font(.system(size: 13)).foregroundColor(Theme.text2).lineLimit(1)
                                }
                                Spacer(minLength: 6)
                                Text("\(s.1) plays").font(.system(size: 12)).foregroundColor(Theme.text2)
                            }
                            .padding(.horizontal, 16).padding(.vertical, 6)
                        }
                    }

                    SectionTitle(text: "Top artists")
                    VStack(spacing: 0) {
                        ForEach(Array(r.artists.prefix(5).enumerated()), id: \.offset) { i, a in
                            NavigationLink(value: Route.artist(a.0)) {
                                HStack(spacing: 12) {
                                    Text("\(i + 1)").font(.system(size: 18, weight: .bold)).foregroundColor(Theme.text3).frame(width: 26)
                                    ArtistPic(name: a.0).frame(width: 46, height: 46)
                                    Text(a.0).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                                    Spacer(minLength: 6)
                                    Text("\(a.1) plays").font(.system(size: 12)).foregroundColor(Theme.text2)
                                }
                                .padding(.horizontal, 16).padding(.vertical, 6)
                            }
                            .buttonStyle(.plain)
                        }
                    }

                    if let top = r.albums.first, let album = app.album(top.0) {
                        SectionTitle(text: "Top album")
                        HStack { AlbumCard(album: album, width: 170); Spacer() }.padding(.horizontal, 16)
                    }
                    if let g = r.genres.first {
                        (Text("Most played genre: ").foregroundColor(Theme.text2) + Text(g.0).fontWeight(.semibold).foregroundColor(Theme.text))
                            .font(.system(size: 14)).padding(.horizontal, 16)
                    }
                    if app.rewindYears.count > 1 {
                        HStack(spacing: 8) {
                            Text("Other years:").font(.system(size: 14)).foregroundColor(Theme.text2)
                            ForEach(app.rewindYears.filter { $0 != year }, id: \.self) { y in
                                NavigationLink(value: Route.rewind(y)) { Text(String(y)).font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.accent) }
                            }
                        }
                        .padding(.horizontal, 16)
                    }
                }
                .padding(.bottom, 24)
            }
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
    }

    private func stat(_ n: String, _ label: String) -> some View {
        VStack(spacing: 2) {
            Text(n).font(.system(size: 22, weight: .heavy)).foregroundColor(.white)
            Text(label).font(.system(size: 11)).foregroundColor(.white.opacity(0.85))
        }
        .frame(maxWidth: .infinity)
    }
}

// MARK: - Credits

struct Credit: Codable, Hashable {
    var role: String
    var names: String

    enum CodingKeys: String, CodingKey { case role, names }
    init(role: String, names: String) { self.role = role; self.names = names }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        role = (try? c.decode(String.self, forKey: .role)) ?? ""
        names = (try? c.decode(String.self, forKey: .names)) ?? ""
    }
}

extension AppModel {
    /// Every song a person is credited on (as the artist or by role), and how often in each role.
    func creditedSongs(_ name: String) -> (ids: [String], roles: [(String, Int)]) {
        let low = name.lowercased()
        var ids: [String] = []
        var roles: [String: Int] = [:]
        for t in allTracks {
            var rs: [String] = []
            if t.artist.lowercased() == low { rs.append("Artist") }
            for c in t.credits where c.role != "ISRC" {
                if c.names.components(separatedBy: ", ").contains(where: { $0.trimmingCharacters(in: .whitespaces).lowercased() == low }) { rs.append(c.role) }
            }
            if !rs.isEmpty {
                ids.append(t.id)
                for r in rs { roles[r, default: 0] += 1 }
            }
        }
        return (ids, roles.sorted { $0.value > $1.value }.map { ($0.key, $0.value) })
    }

    /// Where a person's name leads: their artist page, or their credits page, or nowhere.
    func personRoute(_ name: String) -> Route? {
        let low = name.lowercased()
        if let a = artistNames.first(where: { $0.lowercased() == low }) { return .artist(a) }
        if creditNames[low] != nil { return .credit(creditNames[low] ?? name) }
        return nil
    }
}

struct PersonName: View {
    @EnvironmentObject var app: AppModel
    let name: String
    var body: some View {
        if let r = app.personRoute(name) {
            NavigationLink(value: r) { Text(name).foregroundColor(Theme.accent) }.buttonStyle(.plain)
        } else {
            Text(name).foregroundColor(Theme.text)
        }
    }
}

/// A person's credits: every song in your library they worked on, and in what role.
struct CreditView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    let name: String
    @State private var found: (ids: [String], roles: [(String, Int)]) = ([], [])
    @State private var looked = false

    var body: some View {
        let tracks = found.ids.compactMap { app.track($0) }
        ScrollView {
            if !looked {
                ProgressView().tint(Theme.text2).padding(40)
            } else if found.ids.isEmpty {
                VStack(spacing: 8) {
                    Text("No credits found").font(.system(size: 18, weight: .semibold)).foregroundColor(Theme.text)
                    Text("Nobody called \"\(name)\" is credited in your library.").font(.system(size: 14)).foregroundColor(Theme.text2)
                }
                .padding(40)
            } else {
                VStack(alignment: .leading, spacing: 14) {
                    HStack(spacing: 16) {
                        ArtistPic(name: name, corner: 16, photo: false).frame(width: 96, height: 96)
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Credits").font(.system(size: 13)).foregroundColor(Theme.text2)
                            Text(name).font(.system(size: 24, weight: .bold)).foregroundColor(Theme.text)
                            Text("\(found.ids.count) song\(found.ids.count == 1 ? "" : "s") in your library").font(.system(size: 13)).foregroundColor(Theme.text2)
                        }
                        Spacer(minLength: 0)
                    }
                    .padding(.horizontal, 16)
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 8) {
                            ForEach(found.roles, id: \.0) { r in
                                Text("\(r.0) · \(r.1)").font(.system(size: 12, weight: .semibold)).foregroundColor(Theme.text)
                                    .padding(.horizontal, 12).padding(.vertical, 6).background(Theme.surface3).clipShape(Capsule())
                            }
                        }
                        .padding(.horizontal, 16)
                    }
                    HStack(spacing: 10) {
                        PillButton(title: "Play", icon: "play.fill") { player.play(found.ids) }
                        if let a = app.artistNames.first(where: { $0.lowercased() == name.lowercased() }) {
                            NavigationLink(value: Route.artist(a)) {
                                HStack(spacing: 8) { Image(systemName: "person"); Text("Artist page").font(.system(size: 15, weight: .semibold)) }
                                    .padding(.horizontal, 22).padding(.vertical, 11).background(Theme.surface3).foregroundColor(Theme.text).clipShape(Capsule())
                            }
                        }
                    }
                    .padding(.horizontal, 16)
                    SectionTitle(text: "Songs")
                    LazyVStack(spacing: 0) {
                        ForEach(Array(tracks.enumerated()), id: \.offset) { i, t in
                            TrackRow(track: t, showCover: true) { player.play(found.ids, startAt: i) }
                        }
                    }
                }
                .padding(.top, 8)
                .padding(.bottom, 24)
            }
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
        .task(id: name) {
            found = app.creditedSongs(name)
            looked = true
        }
    }
}

/// Who made a song: what is tagged in the file, plus what MusicBrainz knows, and the details and your plays.
struct CreditsSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    let trackID: String
    @State private var extra: [Credit] = []
    @State private var note = "Looking up more…"
    @State private var asked = false

    private func row(_ k: String, _ v: String) -> some View {
        HStack(alignment: .top) {
            Text(k).font(.system(size: 13)).foregroundColor(Theme.text2).frame(width: 110, alignment: .leading)
            Text(v).font(.system(size: 14)).foregroundColor(Theme.text).frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func names(_ list: String) -> some View {
        let parts = list.components(separatedBy: ", ").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        return VStack(alignment: .leading, spacing: 2) {
            ForEach(parts, id: \.self) { n in
                if let r = app.personRoute(n) {
                    NavigationLink(value: r) { Text(n).font(.system(size: 14)).foregroundColor(Theme.accent) }
                } else {
                    Text(n).font(.system(size: 14)).foregroundColor(Theme.text)
                }
            }
        }
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                if let t = app.track(trackID) {
                    let al = app.album(t.albumId)
                    let own = t.credits.filter { $0.role != "ISRC" }
                    let have = Set(own.map { $0.role })
                    let more = extra.filter { !have.contains($0.role) }
                    VStack(alignment: .leading, spacing: 14) {
                        HStack(spacing: 14) {
                            CoverView(albumID: t.albumId, corner: 8).frame(width: 72, height: 72)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(t.title).font(.system(size: 17, weight: .bold)).foregroundColor(Theme.text)
                                Text(t.artist).font(.system(size: 14)).foregroundColor(Theme.text2)
                                if let al = al { Text(al.title + (al.year.map { " · \($0)" } ?? "")).font(.system(size: 13)).foregroundColor(Theme.text2) }
                            }
                        }
                        Text("Credits").font(.system(size: 12, weight: .bold)).foregroundColor(Theme.text2).textCase(.uppercase)
                        VStack(alignment: .leading, spacing: 10) {
                            HStack(alignment: .top) {
                                Text("Artist").font(.system(size: 13)).foregroundColor(Theme.text2).frame(width: 110, alignment: .leading)
                                names(t.artist)
                            }
                            ForEach(own + more, id: \.role) { c in
                                HStack(alignment: .top) {
                                    Text(c.role).font(.system(size: 13)).foregroundColor(Theme.text2).frame(width: 110, alignment: .leading)
                                    names(c.names)
                                }
                            }
                        }
                        if !note.isEmpty {
                            Text(note).font(.system(size: 12)).foregroundColor(Theme.text3)
                        }
                        Text("Details").font(.system(size: 12, weight: .bold)).foregroundColor(Theme.text2).textCase(.uppercase).padding(.top, 6)
                        VStack(alignment: .leading, spacing: 8) {
                            if let al = al { row("Album", al.title) }
                            if let y = al?.year { row("Year", String(y)) }
                            if let l = al?.label, !l.isEmpty { row("Label", l) }
                            if let g = al?.genre, !g.isEmpty { row("Genre", g) }
                            if t.duration > 0 { row("Length", formatTime(Double(t.duration))) }
                            if let c = al?.copyright, !c.isEmpty { row("Copyright", c) }
                            if let isrc = t.credits.first(where: { $0.role == "ISRC" })?.names, !isrc.isEmpty { row("ISRC", isrc) }
                        }
                        Text("Your listening").font(.system(size: 12, weight: .bold)).foregroundColor(Theme.text2).textCase(.uppercase).padding(.top, 6)
                        VStack(alignment: .leading, spacing: 8) {
                            if let s = app.stats.tracks[trackID] {
                                row("Plays", String(s.n))
                                row("First played", day(s.first))
                                row("Last played", day(s.last))
                            } else {
                                row("Plays", "Not played yet")
                            }
                        }
                    }
                    .padding(16)
                    .task { await lookUp(t) }
                }
            }
            .background(Theme.surface)
            .navigationTitle("Credits")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
            .routeDestinations()
        }
    }

    private func day(_ s: Int) -> String {
        let f = DateFormatter()
        f.dateStyle = .medium
        return f.string(from: Date(timeIntervalSince1970: TimeInterval(s)))
    }

    private func lookUp(_ t: Track) async {
        if asked { return }
        asked = true
        guard let api = app.api, !app.offline else { note = ""; return }
        if let list = await api.credits(trackID) {
            extra = list
            let have = Set(t.credits.map { $0.role })
            let added = list.filter { !have.contains($0.role) }
            if list.isEmpty { note = t.credits.isEmpty ? "No credits found for this song in the file or on MusicBrainz." : "" }
            else { note = added.isEmpty ? "From the file, and matches MusicBrainz." : "Added credits the file doesn't have, from MusicBrainz." }
        } else {
            note = t.credits.isEmpty ? "Couldn't reach MusicBrainz." : ""
        }
    }
}
