import Foundation

/// What /api/me says about you and what the server can do.
struct MeInfo: Decodable {
    var user: User
    var artistInfo: Bool
    var lidarrConfigured: Bool
    var canRequest: Bool
    var transcoding: Bool

    enum CodingKeys: String, CodingKey { case user, artistInfo, lidarr, transcoding }
    enum LidarrKeys: String, CodingKey { case configured, canRequest }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        user = try c.decode(User.self, forKey: .user)
        artistInfo = (try? c.decode(Bool.self, forKey: .artistInfo)) ?? false
        transcoding = (try? c.decode(Bool.self, forKey: .transcoding)) ?? false
        if let l = try? c.nestedContainer(keyedBy: LidarrKeys.self, forKey: .lidarr) {
            lidarrConfigured = (try? l.decode(Bool.self, forKey: .configured)) ?? false
            canRequest = (try? l.decode(Bool.self, forKey: .canRequest)) ?? false
        } else {
            lidarrConfigured = false
            canRequest = false
        }
    }
}

/// One artist, album or song found through MusicBrainz that you may not have yet.
struct FoundItem: Decodable, Identifiable, Hashable {
    var foreignId: String?
    var name: String?
    var title: String?
    var artist: String?
    var year: String?
    var kind: String?
    var albumId: String?
    var singleId: String?
    var image: String?
    var onServer: Bool
    /// when it is here: the id of the album / name of the artist in your library
    var onServerValue: String?
    var requested: Bool
    var disambiguation: String?

    var id: String { (foreignId ?? albumId ?? singleId ?? "") + "|" + label + "|" + (artist ?? "") }
    var label: String { title ?? name ?? "" }

    enum CodingKeys: String, CodingKey { case foreignId, name, title, artist, year, type, albumId, singleId, image, onServer, requested, disambiguation }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        foreignId = try? c.decode(String.self, forKey: .foreignId)
        name = try? c.decode(String.self, forKey: .name)
        title = try? c.decode(String.self, forKey: .title)
        artist = try? c.decode(String.self, forKey: .artist)
        year = try? c.decode(String.self, forKey: .year)
        kind = try? c.decode(String.self, forKey: .type)
        albumId = try? c.decode(String.self, forKey: .albumId)
        singleId = try? c.decode(String.self, forKey: .singleId)
        image = try? c.decode(String.self, forKey: .image)
        disambiguation = try? c.decode(String.self, forKey: .disambiguation)
        requested = (try? c.decode(Bool.self, forKey: .requested)) ?? false
        // "onServer" is false / null (not here), true, or the id / name of what is here
        if let b = try? c.decode(Bool.self, forKey: .onServer) {
            onServer = b
        } else if let s = try? c.decode(String.self, forKey: .onServer) {
            onServer = !s.isEmpty
            onServerValue = s.isEmpty ? nil : s
        } else {
            onServer = false
        }
    }
}

struct FindAnswer: Decodable {
    var artists: [FoundItem]
    var albums: [FoundItem]
    var songs: [FoundItem]
    var canRequest: Bool

    enum CodingKeys: String, CodingKey { case artists, albums, songs, canRequest }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        artists = (try? c.decode([FoundItem].self, forKey: .artists)) ?? []
        albums = (try? c.decode([FoundItem].self, forKey: .albums)) ?? []
        songs = (try? c.decode([FoundItem].self, forKey: .songs)) ?? []
        canRequest = (try? c.decode(Bool.self, forKey: .canRequest)) ?? false
    }
}

/// Something someone asked Lidarr for.
struct RequestItem: Decodable, Identifiable, Hashable {
    var kind: String
    var by: String
    var at: Int
    var title: String
    var artist: String
    var image: String?
    var problem: String?
    var foreignId: String
    var onServer: Bool

    var id: String { foreignId }

    enum CodingKeys: String, CodingKey { case kind, by, at, title, artist, image, problem, foreignId, onServer }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        kind = (try? c.decode(String.self, forKey: .kind)) ?? "album"
        by = (try? c.decode(String.self, forKey: .by)) ?? ""
        at = (try? c.decode(Int.self, forKey: .at)) ?? 0
        title = (try? c.decode(String.self, forKey: .title)) ?? ""
        artist = (try? c.decode(String.self, forKey: .artist)) ?? ""
        image = try? c.decode(String.self, forKey: .image)
        problem = try? c.decode(String.self, forKey: .problem)
        foreignId = (try? c.decode(String.self, forKey: .foreignId)) ?? ""
        if let b = try? c.decode(Bool.self, forKey: .onServer) {
            onServer = b
        } else if let s = try? c.decode(String.self, forKey: .onServer) {
            onServer = !s.isEmpty
        } else {
            onServer = false
        }
    }
}

struct RequestsAnswer: Decodable {
    var requests: [RequestItem]
    enum CodingKeys: String, CodingKey { case requests }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        requests = (try? c.decode([RequestItem].self, forKey: .requests)) ?? []
    }
}

/// A download Lidarr is working on.
struct DownloadItem: Identifiable, Hashable {
    var id: String
    var title: String
    var artist: String
    var state: String
    var progress: Double
}

struct NoteItem: Identifiable, Hashable {
    var id: String
    var kind: String
    var text: String
    var at: Int
    var read: Bool
    var done: String?
}

struct Member: Identifiable, Hashable {
    var id: String
    var name: String
    var trackID: String?
    var trackTitle: String
    var trackArtist: String
    var albumID: String?
    var listening: Bool
}

struct LyricWord: Decodable, Hashable {
    var ms: Int
    var text: String
}

struct LyricLine: Decodable, Hashable, Identifiable {
    var ms: Int
    var text: String
    var words: [LyricWord]
    var id: Int { ms }

    enum CodingKeys: String, CodingKey { case ms, text, words }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        ms = (try? c.decode(Int.self, forKey: .ms)) ?? 0
        text = (try? c.decode(String.self, forKey: .text)) ?? ""
        words = (try? c.decode([LyricWord].self, forKey: .words)) ?? []
    }
}

struct LyricsData: Decodable {
    var synced: [LyricLine]
    var plain: String
    var instrumental: Bool
    var source: String

    enum CodingKeys: String, CodingKey { case synced, plain, instrumental, source }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        synced = (try? c.decode([LyricLine].self, forKey: .synced)) ?? []
        plain = (try? c.decode(String.self, forKey: .plain)) ?? ""
        instrumental = (try? c.decode(Bool.self, forKey: .instrumental)) ?? false
        source = (try? c.decode(String.self, forKey: .source)) ?? ""
    }
}

struct LyricsAnswer: Decodable {
    var lyrics: LyricsData?
    enum CodingKeys: String, CodingKey { case lyrics }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        lyrics = try? c.decode(LyricsData.self, forKey: .lyrics)
    }
}
