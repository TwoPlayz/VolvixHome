import Foundation
import SwiftUI

struct ArtistInfoData: Hashable {
    var bio: String?
    var imageURL: URL?
}

/// Everything the app knows: the server, who is signed in, the library and your saved data.
@MainActor
final class AppModel: ObservableObject {
    enum Phase { case connect, signIn, loading, ready }

    @Published var phase: Phase = .connect
    @Published var user: User?
    @Published var me: MeInfo?
    @Published var message: String?          // a problem to show on the current screen
    @Published var albums: [Album] = []
    @Published var artistNames: [String] = []
    @Published var recent: [String] = []     // album ids, newest first
    @Published var playlists: [PlaylistInfo] = []
    @Published var follows: Set<String> = []
    @Published var busy = false
    /// the server can't be reached: the app runs from its saved copy, and only downloaded songs play
    @Published var offline = false
    @Published var notes: [NoteItem] = []
    @Published var unread = 0
    @Published var members: [Member] = []
    @Published var stats = Stats()
    @Published var fx = FxSettings()
    @Published var people: [Person] = []
    @Published var sharedLists: [PlaylistInfo] = []
    @Published var invites: [PlaylistInvite] = []
    @Published var householdLists: [PlaylistInfo] = []
    @Published var followedIDs: [String] = []
    /// a page to open now, from somewhere that can't push it itself (the full player)
    @Published var openRoute: Route?
    @Published var artistInfos: [String: ArtistInfoData] = [:]
    @Published var quality: String = UserDefaults.standard.string(forKey: "volvix.quality") ?? "original"
    /// "system", "dark" or "light"
    @Published var appearance: String = UserDefaults.standard.string(forKey: "volvix.appearance") ?? "system"

    private(set) var api: API?
    private(set) var serverURL: String = ""
    private(set) var trackByID: [String: Track] = [:]
    private(set) var albumByID: [String: Album] = [:]
    private(set) var albumsByArtist: [String: [Album]] = [:]
    private(set) var allTracks: [Track] = []
    /// every name in the credits (lower case -> as written), for links to a person's credits page
    private(set) var creditNames: [String: String] = [:]
    private(set) var appearsByArtist: [String: [Album]] = [:]
    var userData: [String: Any] = [:]
    private var infoAsked: Set<String> = []
    private var infoRunning = 0

    private let serverKey = "volvix.home.server"
    private let tokenKey = "volvix.home.token"

    init() {
        let d = UserDefaults.standard
        if let s = d.string(forKey: serverKey), !s.isEmpty {
            serverURL = s
            let token = d.string(forKey: tokenKey)
            api = API(base: s, token: token)
            api?.quality = quality
            phase = (token ?? "").isEmpty ? .signIn : .loading
        }
        hookMoved()
    }

    /// If the server sends us from http to https, keep using https from now on.
    private func hookMoved() {
        api?.onMoved = { [weak self] newBase in
            Task { @MainActor in
                self?.serverURL = newBase
                UserDefaults.standard.set(newBase, forKey: "volvix.home.server")
            }
        }
    }

    /// Called once when the app opens: with a saved sign-in, load the library.
    func start() async {
        if phase == .loading { await load() }
    }

    // MARK: connecting and signing in

    func connect(to url: URL) {
        serverURL = url.absoluteString
        UserDefaults.standard.set(serverURL, forKey: serverKey)
        UserDefaults.standard.removeObject(forKey: tokenKey)
        api = API(base: serverURL, token: nil)
        api?.quality = quality
        hookMoved()
        message = nil
        phase = .signIn
    }

    func switchServer() {
        UserDefaults.standard.removeObject(forKey: serverKey)
        UserDefaults.standard.removeObject(forKey: tokenKey)
        api = nil
        serverURL = ""
        user = nil
        me = nil
        clearLibrary()
        forgetSavedCopy()
        phase = .connect
    }

    func signIn(name: String, password: String) async {
        guard let api = api else { return }
        busy = true
        message = nil
        defer { busy = false }
        do {
            let a = try await api.login(name: name, password: password)
            api.token = a.token
            user = a.user
            UserDefaults.standard.set(a.token, forKey: tokenKey)
            phase = .loading
            await load()
        } catch {
            message = error.localizedDescription
        }
    }

    func signOut() async {
        if let api = api { await api.logout() }
        UserDefaults.standard.removeObject(forKey: tokenKey)
        api?.token = nil
        user = nil
        me = nil
        clearLibrary()
        forgetSavedCopy()
        message = nil
        phase = .signIn
    }

    private func forgetSavedCopy() {
        Persist.remove("library.json")
        Persist.remove("me.json")
        Persist.remove("userdata.json")
        offline = false
    }

    var colorScheme: ColorScheme? {
        switch appearance {
        case "dark": return .dark
        case "light": return .light
        default: return nil
        }
    }

    func setAppearance(_ a: String) {
        appearance = a
        UserDefaults.standard.set(a, forKey: "volvix.appearance")
    }

    func setQuality(_ q: String) {
        quality = q
        UserDefaults.standard.set(q, forKey: "volvix.quality")
        api?.quality = q
    }

    // MARK: loading

    func load() async {
        guard let api = api else { return }
        do {
            let meRaw = try await api.meData()
            let libRaw = try await api.libraryData()
            guard let info = try? JSONDecoder().decode(MeInfo.self, from: meRaw),
                  let catalog = try? JSONDecoder().decode(Catalog.self, from: libRaw) else {
                throw ServerError(message: "The server's answer wasn't what the app expected.")
            }
            let data = (try? await api.userData()) ?? [:]
            me = info
            user = info.user
            index(catalog)
            userData = data
            readUserData()
            // keep a copy, so the app opens without the server too
            Persist.write(meRaw, "me.json")
            Persist.write(libRaw, "library.json")
            if let d = try? JSONSerialization.data(withJSONObject: data) { Persist.write(d, "userdata.json") }
            offline = false
            message = nil
            phase = .ready
            await pollOnce()
        } catch let e as ServerError {
            if e.signedOut {
                UserDefaults.standard.removeObject(forKey: tokenKey)
                api.token = nil
                message = "Your sign-in ran out. Sign in again."
                phase = .signIn
            } else if e.status == 0 && loadOffline() {
                return
            } else {
                message = e.message
                phase = .loading
            }
        } catch {
            message = error.localizedDescription
            phase = .loading
        }
    }

    /// No server: open from the copy kept last time. False when there isn't one.
    private func loadOffline() -> Bool {
        guard let libRaw = Persist.read("library.json"), let meRaw = Persist.read("me.json"),
              let catalog = try? JSONDecoder().decode(Catalog.self, from: libRaw),
              let info = try? JSONDecoder().decode(MeInfo.self, from: meRaw) else { return false }
        me = info
        user = info.user
        index(catalog)
        if let d = Persist.read("userdata.json"), let o = (try? JSONSerialization.jsonObject(with: d)) as? [String: Any] {
            userData = o
            readUserData()
        }
        offline = true
        message = nil
        phase = .ready
        return true
    }

    /// Pull down to refresh: the library and your data again.
    func refresh() async {
        guard let api = api else { return }
        if let catalog = try? await api.library() { index(catalog) }
        if let data = try? await api.userData() {
            userData = data
            readUserData()
        }
        await pollOnce()
    }

    /// The things that change while the app is open: notifications and what the household is playing.
    func pollOnce() async {
        guard let api = api else { return }
        let n = await api.notifications()
        notes = n.items
        unread = n.unread
        let h = await api.household()
        members = h.members
        householdLists = h.playlists.compactMap { householdInfo($0) }
        people = await api.people()
        await loadShared()
    }

    func reloadPeople() async {
        if let api = api { people = await api.people() }
    }

    /// The server gave a new key (after changing the password): use it from now on.
    func replaceToken(_ t: String) {
        UserDefaults.standard.set(t, forKey: tokenKey)
        api?.token = t
    }

    /// Songs can only come from this phone: no server, or "Only play downloads" is on.
    var localOnly: Bool { offline || fx.offlineOnly }

    private func householdInfo(_ o: [String: Any]) -> PlaylistInfo? {
        guard let id = o["id"] as? String else { return nil }
        let owner = (o["owner"] as? String) ?? ""
        let entries = (o["tracks"] as? [[String: Any]]) ?? []
        var p = PlaylistInfo(id: id, name: (o["name"] as? String) ?? "Playlist", description: (o["description"] as? String) ?? "",
                             trackIDs: entries.compactMap { $0["id"] as? String }, liked: false)
        p.owner = owner
        p.mine = false
        p.editable = false
        p.shared = (o["shared"] as? Bool) ?? false
        p.isPublic = true
        p.collaborators = (o["collaborators"] as? [String]) ?? []
        p.addedBy = entries.map { ($0["addedBy"] as? String) ?? owner }
        return p
    }

    /// Keep looking every few seconds while the app is on screen.
    func pollLoop() async {
        while !Task.isCancelled {
            try? await Task.sleep(nanoseconds: 12_000_000_000)
            if phase != .ready { continue }
            if offline {
                // look for the server again; when it answers, load everything fresh
                if (try? await api?.meInfo()) != nil { await load() }
                continue
            }
            await pollOnce()
        }
    }

    func markNotesRead() {
        guard let api = api, unread > 0 else { return }
        unread = 0
        notes = notes.map { n in var m = n; m.read = true; return m }
        Task { await api.markNotificationsRead() }
    }

    private func clearLibrary() {
        albums = []
        artistNames = []
        recent = []
        playlists = []
        follows = []
        notes = []
        unread = 0
        members = []
        artistInfos = [:]
        infoAsked = []
        trackByID = [:]
        albumByID = [:]
        albumsByArtist = [:]
        allTracks = []
        creditNames = [:]
        appearsByArtist = [:]
        userData = [:]
        stats = Stats()
        fx = FxSettings()
        people = []
        sharedLists = []
        invites = []
        householdLists = []
        followedIDs = []
    }

    private func index(_ c: Catalog) {
        var byAlbum: [String: Album] = [:]
        for a in c.albums { byAlbum[a.id] = a }
        var byTrack: [String: Track] = [:]
        for t in c.tracks { byTrack[t.id] = t }
        var byArtist: [String: [Album]] = [:]
        for a in c.albums { byArtist[a.artist, default: []].append(a) }
        albumByID = byAlbum
        trackByID = byTrack
        albumsByArtist = byArtist
        allTracks = c.tracks
        albums = c.albums
        var names = Set<String>()
        for a in c.artists { names.insert(a.name) }
        for a in c.albums { names.insert(a.artist) }
        artistNames = names.filter { !$0.isEmpty }.sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }
        var credit: [String: String] = [:]
        for t in c.tracks {
            for cr in t.credits where cr.role != "ISRC" {
                for n in cr.names.components(separatedBy: ", ") {
                    let clean = n.trimmingCharacters(in: .whitespaces)
                    if !clean.isEmpty && credit[clean.lowercased()] == nil { credit[clean.lowercased()] = clean }
                }
            }
        }
        creditNames = credit
        // albums an artist is on without being the album's artist
        var appears: [String: [String: Album]] = [:]
        for t in c.tracks {
            guard let a = byAlbum[t.albumId], !t.artist.isEmpty, a.artist.lowercased() != t.artist.lowercased() else { continue }
            appears[t.artist, default: [:]][a.id] = a
        }
        appearsByArtist = appears.mapValues { Array($0.values) }
    }

    private func readUserData() {
        recent = (userData["recent"] as? [String]) ?? []
        follows = Set(((userData["followedArtists"] as? [String]) ?? []).map { $0.lowercased() })
        var list: [PlaylistInfo] = []
        let raw = (userData["playlists"] as? [[String: Any]]) ?? []
        for p in raw {
            let id = (p["id"] as? String) ?? ""
            if id.isEmpty { continue }
            let entries = (p["tracks"] as? [[String: Any]]) ?? []
            let ids = entries.compactMap { $0["id"] as? String }
            let liked = (p["liked"] as? Bool) == true || id == "pl_liked"
            var info = PlaylistInfo(id: id, name: (p["name"] as? String) ?? "Playlist", description: (p["description"] as? String) ?? "", trackIDs: ids, liked: liked)
            info.owner = user?.id ?? ""
            info.isPublic = (p["public"] as? Bool) ?? false
            info.cover = wholeNumber(p["cover"])
            info.addedBy = entries.map { ($0["addedBy"] as? String) ?? "u_me" }
            list.append(info)
        }
        playlists = list
        followedIDs = (userData["followed"] as? [String]) ?? []
        stats = Stats(userData["stats"] as? [String: Any])
        fx = FxSettings(userData["settings"] as? [String: Any])
    }

    // MARK: looking things up

    func track(_ id: String) -> Track? { trackByID[id] }
    func album(_ id: String) -> Album? { albumByID[id] }
    func albums(by artist: String) -> [Album] { albumsByArtist[artist] ?? [] }
    func appearsOn(_ artist: String) -> [Album] {
        let own = Set(albums(by: artist).map { $0.id })
        return (appearsByArtist[artist] ?? []).filter { !own.contains($0.id) }
    }
    func tracks(of album: Album) -> [Track] {
        album.trackIds.compactMap { trackByID[$0] }.sorted { a, b in
            if a.disc != b.disc { return a.disc < b.disc }
            return a.no < b.no
        }
    }
    func playlist(_ id: String) -> PlaylistInfo? {
        playlists.first { $0.id == id } ?? sharedLists.first { $0.id == id } ?? householdLists.first { $0.id == id }
    }
    var liked: PlaylistInfo? { playlists.first { $0.liked } }
    func isLiked(_ trackID: String) -> Bool { liked?.trackIDs.contains(trackID) ?? false }
    var yourPlaylists: [PlaylistInfo] { editablePlaylists }

    var recentAlbums: [Album] { recent.compactMap { albumByID[$0] } }
    var newestAlbums: [Album] { albums.sorted { $0.added > $1.added } }

    func isFollowing(_ artist: String) -> Bool { follows.contains(artist.lowercased()) }

    func search(_ q: String) -> (artists: [String], albums: [Album], tracks: [Track]) {
        let needle = q.trimmingCharacters(in: .whitespaces)
        if needle.count < 1 { return ([], [], []) }
        func hit(_ s: String) -> Bool { s.range(of: needle, options: [.caseInsensitive, .diacriticInsensitive]) != nil }
        let artists = artistNames.filter { hit($0) }
        let found = albums.filter { hit($0.title) || hit($0.artist) }
        let songs = allTracks.filter { hit($0.title) || hit($0.artist) }
        return (Array(artists.prefix(8)), Array(found.prefix(20)), Array(songs.prefix(40)))
    }

    // MARK: artist photos (a few at a time, kept)

    func info(for artist: String) -> ArtistInfoData? { artistInfos[artist] }

    func loadInfo(_ artist: String) {
        guard let api = api, me?.artistInfo == true else { return }
        if infoAsked.contains(artist) || infoRunning >= 3 { return }
        infoAsked.insert(artist)
        infoRunning += 1
        Task {
            let r = await api.artistInfo(artist)
            var data = ArtistInfoData(bio: r?.bio, imageURL: nil)
            if let p = r?.image, let url = api.imageURL(p) { data.imageURL = url }
            artistInfos[artist] = data
            infoRunning -= 1
        }
    }

    // MARK: changing your data (fetch the server's copy, change it, send it back, like the other apps)

    func edit(_ change: @escaping (inout [String: Any]) -> Void) {
        guard let api = api else { return }
        Task {
            var d = (try? await api.userData()) ?? userData
            change(&d)
            try? await api.saveUserData(d)
            userData = d
            readUserData()
        }
    }

    func playedAlbum(_ id: String) {
        // show it straight away, then save
        recent.removeAll { $0 == id }
        recent.insert(id, at: 0)
        edit { d in
            var r = (d["recent"] as? [String]) ?? []
            r.removeAll { $0 == id }
            r.insert(id, at: 0)
            d["recent"] = Array(r.prefix(30))
        }
    }

    func toggleLike(_ trackID: String) {
        let on = !isLiked(trackID)
        let now = Int(Date().timeIntervalSince1970)
        // show it straight away
        if let i = playlists.firstIndex(where: { $0.liked }) {
            playlists[i].trackIDs.removeAll { $0 == trackID }
            if on { playlists[i].trackIDs.insert(trackID, at: 0) }
        } else if on {
            playlists.insert(PlaylistInfo(id: "pl_liked", name: "Liked Songs", description: "Songs you've liked.", trackIDs: [trackID], liked: true), at: 0)
        }
        edit { d in
            var lists = (d["playlists"] as? [[String: Any]]) ?? []
            var idx = lists.firstIndex { ($0["id"] as? String) == "pl_liked" }
            if idx == nil {
                if !on { return }
                lists.insert(["id": "pl_liked", "name": "Liked Songs", "description": "Songs you've liked.", "owner": "u_me", "liked": true, "tracks": [[String: Any]]()], at: 0)
                idx = 0
            }
            guard let i = idx else { return }
            var entries = (lists[i]["tracks"] as? [[String: Any]]) ?? []
            entries.removeAll { ($0["id"] as? String) == trackID }
            if on { entries.insert(["id": trackID, "addedBy": "u_me", "at": now], at: 0) }
            lists[i]["tracks"] = entries
            d["playlists"] = lists
        }
    }

    func toggleFollow(_ artist: String) {
        let key = artist.lowercased()
        let on = !follows.contains(key)
        if on { follows.insert(key) } else { follows.remove(key) }
        edit { d in
            var list = (d["followedArtists"] as? [String]) ?? []
            list.removeAll { $0.lowercased() == key }
            if on { list.append(artist) }
            d["followedArtists"] = list
        }
    }

    func createPlaylist(_ name: String, adding trackID: String? = nil, tracks: [String] = [], description: String = "") {
        let clean = name.trimmingCharacters(in: .whitespaces)
        if clean.isEmpty { return }
        let id = "pl_" + String(UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased().prefix(12))
        let now = Int(Date().timeIntervalSince1970)
        var entries: [[String: Any]] = []
        for t in tracks { entries.append(["id": t, "addedBy": "u_me", "at": now]) }
        if let t = trackID { entries.append(["id": t, "addedBy": "u_me", "at": now]) }
        let pl: [String: Any] = ["id": id, "name": clean, "description": description, "owner": "u_me", "public": false, "collaborative": false, "collaborators": [String](), "tracks": entries]
        playlists.append(PlaylistInfo(id: id, name: clean, description: description, trackIDs: tracks + (trackID.map { [$0] } ?? []), liked: false))
        edit { d in
            var lists = (d["playlists"] as? [[String: Any]]) ?? []
            let at = (lists.first.map { ($0["liked"] as? Bool) == true || ($0["id"] as? String) == "pl_liked" } ?? false) ? 1 : 0
            lists.insert(pl, at: min(at, lists.count))
            d["playlists"] = lists
        }
    }

    func addTrack(_ trackID: String, to playlistID: String) {
        let now = Int(Date().timeIntervalSince1970)
        if let i = playlists.firstIndex(where: { $0.id == playlistID }) { playlists[i].trackIDs.append(trackID) }
        edit { d in
            var lists = (d["playlists"] as? [[String: Any]]) ?? []
            guard let i = lists.firstIndex(where: { ($0["id"] as? String) == playlistID }) else { return }
            var entries = (lists[i]["tracks"] as? [[String: Any]]) ?? []
            entries.append(["id": trackID, "addedBy": "u_me", "at": now])
            lists[i]["tracks"] = entries
            d["playlists"] = lists
        }
    }

    func removeTrack(at position: Int, from playlistID: String) {
        if let i = playlists.firstIndex(where: { $0.id == playlistID }), position >= 0, position < playlists[i].trackIDs.count {
            playlists[i].trackIDs.remove(at: position)
        }
        edit { d in
            var lists = (d["playlists"] as? [[String: Any]]) ?? []
            guard let i = lists.firstIndex(where: { ($0["id"] as? String) == playlistID }) else { return }
            var entries = (lists[i]["tracks"] as? [[String: Any]]) ?? []
            if position >= 0 && position < entries.count { entries.remove(at: position) }
            lists[i]["tracks"] = entries
            d["playlists"] = lists
        }
    }

    func deletePlaylist(_ playlistID: String) {
        playlists.removeAll { $0.id == playlistID }
        edit { d in
            var lists = (d["playlists"] as? [[String: Any]]) ?? []
            lists.removeAll { ($0["id"] as? String) == playlistID }
            d["playlists"] = lists
        }
    }
}
