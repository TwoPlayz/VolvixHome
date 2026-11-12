import Foundation
import UIKit

struct ServerError: LocalizedError {
    let message: String
    var status: Int = 0
    var errorDescription: String? { message }
    var signedOut: Bool { status == 401 }
}

/// Talks to your Volvix Home server (the same API the website and the Windows app use).
final class API {
    let base: String
    var token: String?
    private let session: URLSession

    init(base: String, token: String?) {
        var b = base
        while b.hasSuffix("/") { b.removeLast() }
        self.base = b
        self.token = token
        let cfg = URLSessionConfiguration.default
        cfg.timeoutIntervalForRequest = 30
        cfg.requestCachePolicy = .reloadIgnoringLocalCacheData
        self.session = URLSession(configuration: cfg)
    }

    var authHeader: String? {
        if let t = token { return "Bearer " + t }
        return nil
    }

    func url(_ path: String) -> URL? { URL(string: base + path) }

    func request(_ path: String, method: String = "GET", json: Any? = nil) throws -> URLRequest {
        guard let u = url(path) else { throw ServerError(message: "That server address isn't valid.") }
        var r = URLRequest(url: u)
        r.httpMethod = method
        if let h = authHeader { r.setValue(h, forHTTPHeaderField: "Authorization") }
        if let json = json {
            r.httpBody = try JSONSerialization.data(withJSONObject: json)
            r.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        return r
    }

    @discardableResult
    func send(_ req: URLRequest) async throws -> Data {
        let result: (Data, URLResponse)
        do {
            result = try await session.data(for: req)
        } catch {
            throw ServerError(message: "Couldn't reach the server. Check the connection.")
        }
        guard let http = result.1 as? HTTPURLResponse else { throw ServerError(message: "The server didn't answer.") }
        if (200..<300).contains(http.statusCode) { return result.0 }
        var message = "The server answered \(http.statusCode)."
        if let o = try? JSONSerialization.jsonObject(with: result.0) as? [String: Any], let e = o["error"] as? String { message = e }
        throw ServerError(message: message, status: http.statusCode)
    }

    func decode<T: Decodable>(_ type: T.Type, _ path: String) async throws -> T {
        let data = try await send(try request(path))
        do {
            return try JSONDecoder().decode(T.self, from: data)
        } catch {
            throw ServerError(message: "The server's answer wasn't what the app expected.")
        }
    }

    // MARK: before and while signed in

    func login(name: String, password: String) async throws -> SignedIn {
        let data = try await send(try request("/api/login", method: "POST", json: ["name": name, "password": password]))
        guard let a = try? JSONDecoder().decode(SignedIn.self, from: data) else { throw ServerError(message: "The server's answer wasn't what the app expected.") }
        return a
    }

    func me() async throws -> User {
        let a = try await decode(MeAnswer.self, "/api/me")
        return a.user
    }

    func library() async throws -> Catalog {
        try await decode(Catalog.self, "/api/library")
    }

    /// Your saved data (playlists, recently played, settings) as the server keeps it. Empty for someone who never saved.
    func userData() async throws -> [String: Any] {
        let data = try await send(try request("/api/me/data"))
        let obj = try? JSONSerialization.jsonObject(with: data)
        return (obj as? [String: Any]) ?? [:]
    }

    func saveUserData(_ d: [String: Any]) async throws {
        var d = d
        d["savedAt"] = Int(Date().timeIntervalSince1970 * 1000)
        try await send(try request("/api/me/data", method: "PUT", json: d))
    }

    func logout() async {
        if let r = try? request("/api/logout", method: "POST") { _ = try? await send(r) }
    }

    func setNowPlaying(trackID: String?, playing: Bool) async {
        var body: [String: Any] = ["playing": playing]
        body["trackId"] = trackID ?? NSNull()
        if let r = try? request("/api/me/now", method: "PUT", json: body) { _ = try? await send(r) }
    }

    // MARK: addresses

    func streamURL(_ trackID: String) -> URL? {
        url("/api/stream/" + encode(trackID))
    }

    func coverURL(_ albumID: String, version: Int, large: Bool = false) -> URL? {
        var q: [String] = []
        q.append("s=" + (large ? "1200" : "640"))
        if version > 0 { q.append("v=\(version)") }
        return url("/api/cover/" + encode(albumID) + "?" + q.joined(separator: "&"))
    }

    func encode(_ s: String) -> String {
        s.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? s
    }
}

/// Downloads pictures once and keeps them (in memory) so scrolling stays smooth.
actor ImageLoader {
    static let shared = ImageLoader()
    private let cache = NSCache<NSString, UIImage>()
    private var waiting: [String: Task<UIImage?, Never>] = [:]

    func image(_ url: URL, auth: String?) async -> UIImage? {
        let key = url.absoluteString
        if let hit = cache.object(forKey: key as NSString) { return hit }
        if let t = waiting[key] { return await t.value }
        let task = Task<UIImage?, Never> {
            var req = URLRequest(url: url)
            if let a = auth { req.setValue(a, forHTTPHeaderField: "Authorization") }
            guard let result = try? await URLSession.shared.data(for: req) else { return nil }
            return UIImage(data: result.0)
        }
        waiting[key] = task
        let img = await task.value
        waiting[key] = nil
        if let img = img { cache.setObject(img, forKey: key as NSString) }
        return img
    }
}
