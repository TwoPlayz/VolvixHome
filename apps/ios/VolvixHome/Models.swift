import Foundation

struct Artist: Codable, Identifiable, Hashable {
    let id: String
    let name: String
}

struct Album: Codable, Identifiable, Hashable {
    let id: String
    let title: String
    let artist: String
    var year: Int?
    var genre: String?
    var trackIds: [String]
    var cover: Bool
    var coverV: Int
    var added: Int
    var label: String?
    var copyright: String?
    /// the art is a picture someone chose (not the album's own), so it can be changed or removed
    var customCover: Bool
    var quality: AlbumQuality

    enum CodingKeys: String, CodingKey { case id, title, artist, year, genre, trackIds, cover, coverV, added, label, copyright, customCover, quality }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id)
        title = (try? c.decode(String.self, forKey: .title)) ?? ""
        artist = (try? c.decode(String.self, forKey: .artist)) ?? ""
        year = try? c.decode(Int.self, forKey: .year)
        genre = try? c.decode(String.self, forKey: .genre)
        trackIds = (try? c.decode([String].self, forKey: .trackIds)) ?? []
        cover = (try? c.decode(Bool.self, forKey: .cover)) ?? false
        coverV = (try? c.decode(Int.self, forKey: .coverV)) ?? 0
        added = (try? c.decode(Int.self, forKey: .added)) ?? 0
        label = try? c.decode(String.self, forKey: .label)
        copyright = try? c.decode(String.self, forKey: .copyright)
        customCover = (try? c.decode(Bool.self, forKey: .customCover)) ?? false
        quality = (try? c.decode(AlbumQuality.self, forKey: .quality)) ?? AlbumQuality()
    }
}

struct Track: Codable, Identifiable, Hashable {
    let id: String
    let title: String
    let artist: String
    let albumId: String
    var no: Int
    var disc: Int
    var duration: Int
    var explicit: Bool
    var credits: [Credit]
    var replayGain: ReplayGain?

    enum CodingKeys: String, CodingKey { case id, title, artist, albumId, no, disc, duration, explicit, credits, replayGain }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id)
        title = (try? c.decode(String.self, forKey: .title)) ?? ""
        artist = (try? c.decode(String.self, forKey: .artist)) ?? ""
        albumId = (try? c.decode(String.self, forKey: .albumId)) ?? ""
        no = (try? c.decode(Int.self, forKey: .no)) ?? 0
        disc = (try? c.decode(Int.self, forKey: .disc)) ?? 1
        duration = (try? c.decode(Int.self, forKey: .duration)) ?? 0
        explicit = (try? c.decode(Bool.self, forKey: .explicit)) ?? false
        credits = (try? c.decode([Credit].self, forKey: .credits)) ?? []
        replayGain = try? c.decode(ReplayGain.self, forKey: .replayGain)
    }
}

/// What the files of an album are: lossless (with bit depth and sample rate) or a lossy format with its bit rate.
struct AlbumQuality: Codable, Hashable {
    var lossless = false
    var bits: Int?
    var khz: Double?
    var codec = ""
    var kbps: Int?

    init() {}

    enum CodingKeys: String, CodingKey { case lossless, bits, khz, codec, kbps }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        lossless = (try? c.decode(Bool.self, forKey: .lossless)) ?? false
        bits = try? c.decode(Int.self, forKey: .bits)
        khz = try? c.decode(Double.self, forKey: .khz)
        codec = (try? c.decode(String.self, forKey: .codec)) ?? ""
        kbps = try? c.decode(Int.self, forKey: .kbps)
    }

    var hires: Bool { lossless && ((bits ?? 16) >= 24 || (khz ?? 44.1) > 48) }
    /// 1 low, 2 normal, 3 high, 4 lossless
    var level: Int {
        if lossless { return 4 }
        let k = kbps ?? 0
        return k >= 256 ? 3 : (k >= 128 ? 2 : 1)
    }
    var label: String {
        if lossless { return hires ? "Hi-Res Lossless" : "Lossless" }
        let k = kbps ?? 0
        return k >= 256 ? "High Quality" : (k >= 128 ? "Normal" : "Low")
    }
    var detail: String {
        if lossless {
            return [bits.map { "\($0)-bit" }, khz.map { "\($0) kHz" }, codec.isEmpty ? nil : codec].compactMap { $0 }.joined(separator: " / ")
        }
        return (codec.isEmpty ? "Audio" : codec) + ((kbps ?? 0) > 0 ? " \(kbps ?? 0) kbps" : "")
    }
}

struct ReplayGain: Codable, Hashable {
    var track: Double?
    var album: Double?
    var peak: Double?
}

struct Catalog: Decodable {
    var artists: [Artist]
    var albums: [Album]
    var tracks: [Track]

    enum CodingKeys: String, CodingKey { case artists, albums, tracks }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        artists = (try? c.decode([Artist].self, forKey: .artists)) ?? []
        albums = (try? c.decode([Album].self, forKey: .albums)) ?? []
        tracks = (try? c.decode([Track].self, forKey: .tracks)) ?? []
    }
}

struct User: Codable, Hashable {
    var id: String
    var name: String
    var username: String
    var admin: Bool
    var avatar: Int
    var perms: [String]
    var hideExplicit: Bool

    enum CodingKeys: String, CodingKey { case id, name, username, admin, avatar, perms, hideExplicit }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = (try? c.decode(String.self, forKey: .id)) ?? ""
        name = (try? c.decode(String.self, forKey: .name)) ?? ""
        username = (try? c.decode(String.self, forKey: .username)) ?? ""
        admin = (try? c.decode(Bool.self, forKey: .admin)) ?? false
        avatar = (try? c.decode(Int.self, forKey: .avatar)) ?? 0
        perms = (try? c.decode([String].self, forKey: .perms)) ?? []
        hideExplicit = (try? c.decode(Bool.self, forKey: .hideExplicit)) ?? false
    }
}

struct SignedIn: Decodable {
    let user: User
    let token: String
}

struct MeAnswer: Decodable {
    let user: User
}

/// One of your playlists, read from your saved data.
struct PlaylistInfo: Identifiable, Hashable {
    let id: String
    var name: String
    var description: String
    var trackIDs: [String]
    var liked: Bool
    /// the server's id of whoever made it
    var owner: String = ""
    var mine = true
    var editable = true
    /// kept on the server so several people edit the same copy
    var shared = false
    var isPublic = false
    var collaborators: [String] = []
    var invited: [String] = []
    var addedBy: [String] = []
    var cover = 0
    var rev = 0
}
