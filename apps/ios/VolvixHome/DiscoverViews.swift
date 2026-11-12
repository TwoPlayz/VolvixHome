import SwiftUI

extension FoundItem {
    /// Built by hand, for a button on a page that isn't a search result.
    init(foreignId: String?, title: String?, onServer: Bool, requested: Bool) {
        self.foreignId = foreignId
        self.title = title
        self.onServer = onServer
        self.requested = requested
    }
}

struct DiscoverTrack: Identifiable, Hashable {
    var id: Int
    var disc: Int
    var no: String
    var title: String
    var lengthMs: Int
}

/// An album, EP or single from MusicBrainz that you may not have: its songs, and a Get button for the whole release.
struct DiscoverAlbumView: View {
    @EnvironmentObject var app: AppModel
    let id: String
    @State private var loaded = false
    @State private var title = ""
    @State private var artist = ""
    @State private var artistID: String?
    @State private var meta = ""
    @State private var image: String?
    @State private var tracks: [DiscoverTrack] = []
    @State private var inLibrary: String?
    @State private var requested = false
    @State private var failed: String?
    @State private var asked: Set<String> = []

    var body: some View {
        ScrollView {
            VStack(spacing: 14) {
                if !loaded && failed == nil {
                    ProgressView().tint(.white).padding(.top, 40)
                    Text("Looking it up on MusicBrainz…").font(.system(size: 14)).foregroundColor(Theme.text2)
                }
                if let f = failed { ProblemBox(text: f).padding(16) }
                if loaded {
                    RemoteSquare(path: image, title: title, corner: 10).frame(width: 220, height: 220).padding(.top, 8)
                    VStack(spacing: 4) {
                        Text(title).font(.system(size: 24, weight: .bold)).foregroundColor(Theme.text).multilineTextAlignment(.center)
                        if let a = artistID, inLibrary == nil {
                            NavigationLink(value: Route.discoverArtist(a)) {
                                Text(artist).font(.system(size: 16, weight: .semibold)).foregroundColor(Theme.accent)
                            }
                        } else {
                            Text(artist).font(.system(size: 16, weight: .semibold)).foregroundColor(Theme.text2)
                        }
                        Text(meta).font(.system(size: 13)).foregroundColor(Theme.text2)
                    }
                    .padding(.horizontal, 16)

                    if let lib = inLibrary {
                        NavigationLink(value: Route.album(lib)) {
                            Label("In your library: open it", systemImage: "checkmark")
                                .font(.system(size: 15, weight: .semibold)).foregroundColor(.white)
                                .padding(.horizontal, 22).padding(.vertical, 11).background(Theme.accent).clipShape(Capsule())
                        }
                    } else {
                        GetButton(item: FoundItem(foreignId: id, title: title, onServer: false, requested: requested), kind: "album", foreignId: id, label: "Get this album", asked: $asked)
                    }

                    if tracks.isEmpty {
                        Text("MusicBrainz doesn't have the songs for this release yet. You can still get the album.")
                            .font(.system(size: 14)).foregroundColor(Theme.text2).padding(.horizontal, 16)
                    }
                    let multi = tracks.contains { $0.disc > 1 }
                    LazyVStack(spacing: 0) {
                        ForEach(tracks) { t in
                            HStack(spacing: 12) {
                                Text((multi ? "\(t.disc)." : "") + t.no).font(.system(size: 14)).foregroundColor(Theme.text3).frame(width: 40, alignment: .leading)
                                Text(t.title).font(.system(size: 16)).foregroundColor(Theme.text).lineLimit(1)
                                Spacer()
                                if t.lengthMs > 0 { Text(formatTime(Double(t.lengthMs) / 1000)).font(.system(size: 13)).foregroundColor(Theme.text2) }
                            }
                            .padding(.horizontal, 16).padding(.vertical, 9)
                        }
                    }
                    Text("Lidarr gets the whole release, so every song comes together. Song list from MusicBrainz.")
                        .font(.system(size: 11)).foregroundColor(Theme.text3).padding(.horizontal, 16)
                }
            }
            .padding(.bottom, 24)
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
        .task { await load() }
    }

    private func load() async {
        guard let api = app.api else { return }
        do {
            var req = try api.request("/api/discover/album/" + api.encode(id))
            req.timeoutInterval = 45
            let data = try await api.send(req)
            guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { failed = "The answer wasn't what the app expected."; return }
            title = (o["title"] as? String) ?? ""
            artist = (o["artist"] as? String) ?? ""
            artistID = o["artistId"] as? String
            image = o["image"] as? String
            requested = (o["requested"] as? Bool) ?? false
            if let s = o["onServer"] as? String, app.album(s) != nil { inLibrary = s }
            var bits: [String] = []
            if let y = o["year"] as? String { bits.append(y) }
            if let t = o["type"] as? String { bits.append(t) }
            let sec = ((o["secondary"] as? [String]) ?? []).joined(separator: ", ")
            if !sec.isEmpty { bits.append(sec) }
            var list: [DiscoverTrack] = []
            for (i, t) in ((o["tracks"] as? [[String: Any]]) ?? []).enumerated() {
                let no: String
                if let s = t["no"] as? String { no = s } else if let n = t["no"] as? Int { no = String(n) } else { no = "" }
                list.append(DiscoverTrack(id: i, disc: (t["disc"] as? Int) ?? 1, no: no, title: (t["title"] as? String) ?? "", lengthMs: (t["lengthMs"] as? Int) ?? 0))
            }
            tracks = list
            if !list.isEmpty { bits.append("\(list.count) songs") }
            meta = bits.joined(separator: " · ")
            loaded = true
        } catch {
            failed = error.localizedDescription
        }
    }
}

/// An artist from MusicBrainz: everything they have released, each with a Get button.
struct DiscoverArtistView: View {
    @EnvironmentObject var app: AppModel
    let id: String
    @State private var loaded = false
    @State private var name = ""
    @State private var image: String?
    @State private var inLibrary: String?
    @State private var releases: [FoundItem] = []
    @State private var failed: String?
    @State private var asked: Set<String> = []

    private var missing: Int { releases.filter { !$0.onServer && !$0.requested && !asked.contains($0.foreignId ?? "") }.count }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if !loaded && failed == nil {
                    HStack { ProgressView().tint(.white); Text("Looking it up on MusicBrainz…").font(.system(size: 14)).foregroundColor(Theme.text2) }.padding(16)
                }
                if let f = failed { ProblemBox(text: f).padding(16) }
                if loaded {
                    HStack(spacing: 16) {
                        RemoteSquare(path: image, title: name, corner: 16).frame(width: 120, height: 120)
                        VStack(alignment: .leading, spacing: 6) {
                            Text(inLibrary == nil ? "Not in your library yet" : "Artist").font(.system(size: 13)).foregroundColor(Theme.text2)
                            Text(name).font(.system(size: 26, weight: .bold)).foregroundColor(Theme.text)
                            Text("\(releases.count) releases").font(.system(size: 13)).foregroundColor(Theme.text2)
                        }
                        Spacer(minLength: 0)
                    }
                    .padding(.horizontal, 16)
                    HStack(spacing: 12) {
                        if let n = inLibrary {
                            NavigationLink(value: Route.artist(n)) {
                                Label("Open in your library", systemImage: "checkmark")
                                    .font(.system(size: 14, weight: .semibold)).foregroundColor(.white)
                                    .padding(.horizontal, 18).padding(.vertical, 10).background(Theme.surface3).clipShape(Capsule())
                            }
                        }
                        if missing > 0 && app.me?.canRequest == true {
                            GetEverything(count: missing, id: id, asked: $asked)
                        }
                    }
                    .padding(.horizontal, 16)

                    let albums = releases.filter { $0.kind != "Single" }
                    let singles = releases.filter { $0.kind == "Single" }
                    if !albums.isEmpty {
                        SectionTitle(text: "Albums & EPs")
                        ForEach(albums) { r in ReleaseRow(item: r, asked: $asked) }
                    }
                    if !singles.isEmpty {
                        SectionTitle(text: "Singles")
                        ForEach(singles) { r in ReleaseRow(item: r, asked: $asked) }
                    }
                    if releases.isEmpty {
                        Text("MusicBrainz doesn't list any albums for this artist yet.").font(.system(size: 14)).foregroundColor(Theme.text2).padding(.horizontal, 16)
                    }
                }
            }
            .padding(.bottom, 24)
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
        .task { await load() }
    }

    private func load() async {
        guard let api = app.api else { return }
        do {
            var req = try api.request("/api/discover/artist/" + api.encode(id))
            req.timeoutInterval = 45
            let data = try await api.send(req)
            guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { failed = "The answer wasn't what the app expected."; return }
            name = (o["name"] as? String) ?? ""
            image = o["image"] as? String
            inLibrary = o["onServer"] as? String
            if let raw = try? JSONSerialization.data(withJSONObject: (o["albums"] as? [Any]) ?? []),
               let items = try? JSONDecoder().decode([FoundItem].self, from: raw) {
                releases = items
            }
            loaded = true
        } catch {
            failed = error.localizedDescription
        }
    }
}

struct GetEverything: View {
    @EnvironmentObject var app: AppModel
    let count: Int
    let id: String
    @Binding var asked: Set<String>
    @State private var busy = false

    var body: some View {
        Button {
            busy = true
            Task {
                do { try await app.api?.requestMusic(kind: "artist", foreignId: id); asked.insert(id) } catch {}
                busy = false
            }
        } label: {
            HStack(spacing: 6) {
                if busy { ProgressView().tint(.white).scaleEffect(0.7) } else { Image(systemName: "arrow.down.to.line") }
                Text("Get everything (\(count))").font(.system(size: 14, weight: .semibold))
            }
            .padding(.horizontal, 18).padding(.vertical, 10).background(Theme.accent).foregroundColor(.white).clipShape(Capsule())
        }
        .disabled(busy)
    }
}

/// A release in an artist's list: tap for its songs, or press Get.
struct ReleaseRow: View {
    @EnvironmentObject var app: AppModel
    let item: FoundItem
    @Binding var asked: Set<String>

    var body: some View {
        HStack(spacing: 12) {
            NavigationLink(value: route) {
                HStack(spacing: 12) {
                    RemoteSquare(path: item.image, title: item.label, corner: 8).frame(width: 52, height: 52)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(item.label).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                        Text([item.year, item.kind].compactMap { $0 }.joined(separator: " · ")).font(.system(size: 12)).foregroundColor(Theme.text2)
                    }
                    Spacer(minLength: 0)
                }
            }
            .buttonStyle(.plain)
            GetButton(item: item, kind: "album", foreignId: item.foreignId, asked: $asked)
        }
        .padding(10)
        .background(Theme.surface2)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .padding(.horizontal, 16)
    }

    private var route: Route {
        if let s = item.onServerValue, app.album(s) != nil { return .album(s) }
        return .discoverAlbum(item.foreignId ?? "")
    }
}
