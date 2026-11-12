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

    enum CodingKeys: String, CodingKey { case id, title, artist, year, genre, trackIds, cover, coverV, added }

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

    enum CodingKeys: String, CodingKey { case id, title, artist, albumId, no, disc, duration, explicit }

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
    }
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

    enum CodingKeys: String, CodingKey { case id, name, username, admin }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = (try? c.decode(String.self, forKey: .id)) ?? ""
        name = (try? c.decode(String.self, forKey: .name)) ?? ""
        username = (try? c.decode(String.self, forKey: .username)) ?? ""
        admin = (try? c.decode(Bool.self, forKey: .admin)) ?? false
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
}
