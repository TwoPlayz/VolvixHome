import Foundation
import SwiftUI

/// Everything the app knows: the server, who is signed in, the library and your saved data.
@MainActor
final class AppModel: ObservableObject {
    enum Phase { case connect, signIn, loading, ready }

    @Published var phase: Phase = .connect
    @Published var user: User?
    @Published var message: String?          // a problem to show on the current screen
    @Published var albums: [Album] = []
    @Published var artistNames: [String] = []
    @Published var recent: [String] = []     // album ids, newest first
    @Published var playlists: [PlaylistInfo] = []
    @Published var busy = false

    private(set) var api: API?
    private(set) var serverURL: String = ""
    private(set) var trackByID: [String: Track] = [:]
    private(set) var albumByID: [String: Album] = [:]
    private(set) var albumsByArtist: [String: [Album]] = [:]
    private(set) var allTracks: [Track] = []
    private var userData: [String: Any] = [:]

    private let serverKey = "volvix.home.server"
    private let tokenKey = "volvix.home.token"

    init() {
        let d = UserDefaults.standard
        if let s = d.string(forKey: serverKey), !s.isEmpty {
            serverURL = s
            let token = d.string(forKey: tokenKey)
            api = API(base: s, token: token)
            phase = (token ?? "").isEmpty ? .signIn : .loading
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
        message = nil
        phase = .signIn
    }

    func switchServer() {
        UserDefaults.standard.removeObject(forKey: serverKey)
        UserDefaults.standard.removeObject(forKey: tokenKey)
        api = nil
        serverURL = ""
        user = nil
        clearLibrary()
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
        clearLibrary()
        message = nil
        phase = .signIn
    }

    // MARK: loading

    func load() async {
        guard let api = api else { return }
        do {
            let me = try await api.me()
            let catalog = try await api.library()
            let data = (try? await api.userData()) ?? [:]
            user = me
            index(catalog)
            userData = data
            readUserData()
            message = nil
            phase = .ready
        } catch let e as ServerError {
            if e.signedOut {
                UserDefaults.standard.removeObject(forKey: tokenKey)
                api.token = nil
                message = "Your sign-in ran out. Sign in again."
                phase = .signIn
            } else {
                message = e.message
                phase = .loading
            }
        } catch {
            message = error.localizedDescription
            phase = .loading
        }
    }

    /// Pull down to refresh: the library and your data again.
    func refresh() async {
        guard let api = api else { return }
        if let catalog = try? await api.library() { index(catalog) }
        if let data = try? await api.userData() {
            userData = data
            readUserData()
        }
    }

    private func clearLibrary() {
        albums = []
        artistNames = []
        recent = []
        playlists = []
        trackByID = [:]
        albumByID = [:]
        albumsByArtist = [:]
        allTracks = []
        userData = [:]
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
    }

    private func readUserData() {
        recent = (userData["recent"] as? [String]) ?? []
        var list: [PlaylistInfo] = []
        let raw = (userData["playlists"] as? [[String: Any]]) ?? []
        for p in raw {
            let id = (p["id"] as? String) ?? ""
            if id.isEmpty { continue }
            let entries = (p["tracks"] as? [[String: Any]]) ?? []
            let ids = entries.compactMap { $0["id"] as? String }
            let liked = (p["liked"] as? Bool) == true || id == "pl_liked"
            list.append(PlaylistInfo(id: id, name: (p["name"] as? String) ?? "Playlist", description: (p["description"] as? String) ?? "", trackIDs: ids, liked: liked))
        }
        playlists = list
    }

    // MARK: looking things up

    func track(_ id: String) -> Track? { trackByID[id] }
    func album(_ id: String) -> Album? { albumByID[id] }
    func albums(by artist: String) -> [Album] { albumsByArtist[artist] ?? [] }
    func tracks(of album: Album) -> [Track] {
        album.trackIds.compactMap { trackByID[$0] }.sorted { a, b in
            if a.disc != b.disc { return a.disc < b.disc }
            return a.no < b.no
        }
    }
    func playlist(_ id: String) -> PlaylistInfo? { playlists.first { $0.id == id } }
    var liked: PlaylistInfo? { playlists.first { $0.liked } }
    func isLiked(_ trackID: String) -> Bool { liked?.trackIDs.contains(trackID) ?? false }

    var recentAlbums: [Album] { recent.compactMap { albumByID[$0] } }
    var newestAlbums: [Album] { albums.sorted { $0.added > $1.added } }

    func search(_ q: String) -> (artists: [String], albums: [Album], tracks: [Track]) {
        let needle = q.trimmingCharacters(in: .whitespaces)
        if needle.count < 1 { return ([], [], []) }
        func hit(_ s: String) -> Bool { s.range(of: needle, options: [.caseInsensitive, .diacriticInsensitive]) != nil }
        let artists = artistNames.filter { hit($0) }
        let found = albums.filter { hit($0.title) || hit($0.artist) }
        let songs = allTracks.filter { hit($0.title) || hit($0.artist) }
        return (Array(artists.prefix(8)), Array(found.prefix(20)), Array(songs.prefix(40)))
    }

    // MARK: changing your data (fetch the server's copy, change it, send it back, like the other apps)

    private func edit(_ change: @escaping (inout [String: Any]) -> Void) {
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
        let now = Int(Date().timeIntervalSince1970 * 1000)
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
}
