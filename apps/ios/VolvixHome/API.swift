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
    var base: String
    /// the address you signed in with; `base` is the home-network address instead while that one answers
    private(set) var publicBase: String
    var token: String?
    /// the server answered "use https instead": told to whoever keeps the saved address
    var onMoved: ((String) -> Void)?
    private let keeper = RedirectKeeper()
    /// "original", or "high" / "normal" / "low" when the server can convert songs (needs ffmpeg there)
    var quality: String = "original"
    private let session: URLSession

    init(base: String, token: String?) {
        var b = base
        while b.hasSuffix("/") { b.removeLast() }
        self.base = b
        self.publicBase = b
        self.token = token
        let cfg = URLSessionConfiguration.default
        cfg.timeoutIntervalForRequest = 30
        cfg.requestCachePolicy = .reloadIgnoringLocalCacheData
        self.session = URLSession(configuration: cfg, delegate: keeper, delegateQueue: nil)
        keeper.moved = { [weak self] newBase in
            guard let self = self else { return }
            // only the public address is remembered (a home address is found again each time)
            if self.base == self.publicBase {
                self.base = newBase
                self.publicBase = newBase
                self.onMoved?(newBase)
            }
        }
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

    /// Does the server answer, signed in as us, at this address? (A different machine on the same address says 401.)
    func reachable(_ address: String) async -> Bool {
        var a = address
        while a.hasSuffix("/") { a.removeLast() }
        guard let u = URL(string: a + "/api/me") else { return false }
        var r = URLRequest(url: u)
        r.timeoutInterval = 1.5
        if let h = authHeader { r.setValue(h, forHTTPHeaderField: "Authorization") }
        guard let (_, resp) = try? await URLSession.shared.data(for: r), let http = resp as? HTTPURLResponse else { return false }
        return http.statusCode == 200
    }

    /// Is this phone on the same network as that address (its own address and netmask line up)? A name is just tried.
    func phoneShares(_ address: String) -> Bool {
        guard let host = URL(string: address)?.host else { return false }
        let parts = host.split(separator: ".").compactMap { UInt32($0) }
        guard parts.count == 4, host.split(separator: ".").count == 4 else { return true }
        let target = (parts[0] << 24) | (parts[1] << 16) | (parts[2] << 8) | parts[3]
        var list: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&list) == 0, let first = list else { return true }
        defer { freeifaddrs(list) }
        var p: UnsafeMutablePointer<ifaddrs>? = first
        while let i = p {
            let f = i.pointee
            if let a = f.ifa_addr, let m = f.ifa_netmask, a.pointee.sa_family == UInt8(AF_INET), (f.ifa_flags & UInt32(IFF_UP)) != 0 {
                let ip = a.withMemoryRebound(to: sockaddr_in.self, capacity: 1) { UInt32(bigEndian: $0.pointee.sin_addr.s_addr) }
                let mask = m.withMemoryRebound(to: sockaddr_in.self, capacity: 1) { UInt32(bigEndian: $0.pointee.sin_addr.s_addr) }
                if ip != 0x7f000001, (ip & mask) == (target & mask) { return true }
            }
            p = f.ifa_next
        }
        return false
    }

    /// Use the home-network address (or go back to the public one).
    func use(_ address: String?) {
        var a = address ?? publicBase
        while a.hasSuffix("/") { a.removeLast() }
        base = a
    }

    var onHomeNetwork: Bool { base != publicBase }

    // MARK: addresses

    func streamURL(_ trackID: String, quality override: String? = nil) -> URL? {
        var p = "/api/stream/" + encode(trackID)
        let q = override ?? quality
        if q == "high" || q == "normal" || q == "low" { p += "?q=" + q }
        return url(p)
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

/// A redirect (http to https at a domain) must keep the request as it was: sent on as a GET, a sign-in would be answered "405".
final class RedirectKeeper: NSObject, URLSessionTaskDelegate {
    var moved: ((String) -> Void)?

    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        var next = request
        if let old = task.originalRequest {
            next.httpMethod = old.httpMethod
            next.httpBody = old.httpBody
            for (k, v) in old.allHTTPHeaderFields ?? [:] where next.value(forHTTPHeaderField: k) == nil { next.setValue(v, forHTTPHeaderField: k) }
            // same machine, now over https: remember the better address
            if let a = old.url, let b = next.url, a.host == b.host, a.scheme == "http", b.scheme == "https" {
                var c = URLComponents(url: b, resolvingAgainstBaseURL: false)
                c?.path = ""
                c?.query = nil
                if let s = c?.string { moved?(s) }
            }
        }
        completionHandler(next)
    }
}

/// Downloads pictures once and keeps them (in memory) so scrolling stays smooth.
actor ImageLoader {
    static let shared = ImageLoader()
    /// kept by the picture's path (not the server's address), so switching between the domain and the home address keeps them
    nonisolated static let cache: NSCache<NSString, UIImage> = {
        let c = NSCache<NSString, UIImage>()
        c.totalCostLimit = 160_000_000
        return c
    }()
    private var waiting: [String: Task<UIImage?, Never>] = [:]

    nonisolated static func key(_ url: URL) -> String { url.path + "?" + (url.query ?? "") }
    /// A picture already loaded, at once (so a row scrolled back into view doesn't blink).
    nonisolated static func cached(_ url: URL) -> UIImage? { cache.object(forKey: key(url) as NSString) }

    func image(_ url: URL, auth: String?) async -> UIImage? {
        let key = Self.key(url)
        if let hit = Self.cache.object(forKey: key as NSString) { return hit }
        if let t = waiting[key] { return await t.value }
        let big = (url.query ?? "").contains("s=1200")
        let task = Task<UIImage?, Never> {
            var req = URLRequest(url: url)
            if let a = auth { req.setValue(a, forHTTPHeaderField: "Authorization") }
            guard let result = try? await URLSession.shared.data(for: req), let raw = UIImage(data: result.0) else { return nil }
            // decode now, here, at the size it is shown (otherwise the first draw does it, on the main thread, mid-scroll)
            let side: CGFloat = big ? 1200 : 420
            if raw.size.width > side || raw.size.height > side, let small = await raw.byPreparingThumbnail(ofSize: CGSize(width: side, height: side)) { return small }
            return await raw.byPreparingForDisplay() ?? raw
        }
        waiting[key] = task
        let img = await task.value
        waiting[key] = nil
        if let img = img {
            let cost = Int(img.size.width * img.size.height * img.scale * img.scale * 4)
            Self.cache.setObject(img, forKey: key as NSString, cost: cost)
        }
        return img
    }
}
