import Foundation

extension API {
    func meData() async throws -> Data { try await send(try request("/api/me")) }
    func libraryData() async throws -> Data { try await send(try request("/api/library")) }

    func meInfo() async throws -> MeInfo {
        try await decode(MeInfo.self, "/api/me")
    }

    /// What MusicBrainz knows about who made a song (the server looks it up).
    func credits(_ trackID: String) async -> [Credit]? {
        guard let data = try? await send(try request("/api/credits/" + encode(trackID))),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let list = o["credits"] as? [[String: Any]] else { return nil }
        return list.compactMap { c in
            guard let r = c["role"] as? String, let n = c["names"] as? String else { return nil }
            return Credit(role: r, names: n)
        }
    }

    func lyrics(_ trackID: String) async -> LyricsData? {
        let a = try? await decode(LyricsAnswer.self, "/api/lyrics/" + encode(trackID))
        return a?.lyrics
    }

    /// Music you don't have yet, from MusicBrainz (the server asks Lidarr's metadata).
    func find(_ term: String, kind: String) async throws -> FindAnswer {
        let q = term.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? term
        var req = try request("/api/lidarr/search?kind=\(kind)&term=\(q)")
        req.timeoutInterval = 45
        let data = try await send(req)
        guard let a = try? JSONDecoder().decode(FindAnswer.self, from: data) else { throw ServerError(message: "The search answer wasn't what the app expected.") }
        return a
    }

    func requestMusic(kind: String, foreignId: String) async throws {
        var req = try request("/api/lidarr/request", method: "POST", json: ["kind": kind, "foreignId": foreignId])
        req.timeoutInterval = 60
        try await send(req)
    }

    func cancelRequest(_ foreignId: String) async throws {
        try await send(try request("/api/lidarr/request/" + encode(foreignId), method: "DELETE"))
    }

    func requests() async -> [RequestItem] {
        let a = try? await decode(RequestsAnswer.self, "/api/lidarr/requests")
        return a?.requests ?? []
    }

    func downloads() async -> [DownloadItem] {
        guard let data = try? await send(try request("/api/lidarr/queue")),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let items = o["items"] as? [[String: Any]] else { return [] }
        var out: [DownloadItem] = []
        for (i, d) in items.enumerated() {
            out.append(DownloadItem(
                id: "\(i)-" + ((d["title"] as? String) ?? ""),
                title: (d["title"] as? String) ?? "",
                artist: (d["artist"] as? String) ?? "",
                state: (d["state"] as? String) ?? "",
                progress: (d["progress"] as? Double) ?? 0))
        }
        return out
    }

    func notifications() async -> (items: [NoteItem], unread: Int) {
        guard let data = try? await send(try request("/api/notifications")),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return ([], 0) }
        var out: [NoteItem] = []
        for n in (o["items"] as? [[String: Any]]) ?? [] {
            out.append(NoteItem(
                id: (n["id"] as? String) ?? UUID().uuidString,
                kind: (n["kind"] as? String) ?? "",
                text: (n["text"] as? String) ?? "",
                at: (n["at"] as? Int) ?? 0,
                read: (n["read"] as? Bool) ?? false,
                done: n["done"] as? String,
                partyID: (n["data"] as? [String: Any])?["partyId"] as? String,
                from: (n["data"] as? [String: Any])?["from"] as? String))
        }
        return (out, (o["unread"] as? Int) ?? 0)
    }

    func markNotificationsRead() async {
        if let r = try? request("/api/notifications/read", method: "POST", json: [String: Any]()) { _ = try? await send(r) }
    }

    func household() async -> [Member] {
        guard let data = try? await send(try request("/api/household")),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let list = o["members"] as? [[String: Any]] else { return [] }
        var out: [Member] = []
        for m in list {
            let now = m["now"] as? [String: Any]
            out.append(Member(
                id: (m["id"] as? String) ?? UUID().uuidString,
                name: (m["name"] as? String) ?? "",
                trackID: now?["id"] as? String,
                trackTitle: (now?["title"] as? String) ?? "",
                trackArtist: (now?["artist"] as? String) ?? "",
                albumID: now?["albumId"] as? String,
                listening: (now?["listening"] as? Bool) ?? false))
        }
        return out
    }

    /// A short bio and a photo for an artist (from Wikipedia, kept by the server). The photo is a server path.
    func artistInfo(_ name: String) async -> (bio: String?, image: String?)? {
        let q = name.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? name
        guard let data = try? await send(try request("/api/artist/info?name=" + q)),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        return (o["bio"] as? String, o["image"] as? String)
    }

    /// A picture the server hands out by path (found music's covers, artist photos).
    func imageURL(_ path: String) -> URL? { url(path) }
}
