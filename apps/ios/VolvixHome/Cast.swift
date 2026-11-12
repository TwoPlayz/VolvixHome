import Foundation
import SwiftUI

struct CastDevice: Identifiable, Hashable {
    var id: String
    var name: String
    var model: String
}

/// TVs and speakers (Google Cast). The server finds them and talks to them; the TV fetches each song from the server.
/// While casting, this phone keeps the queue and the clock but stays silent, and follows the TV's play / pause.
@MainActor
final class CastHub: ObservableObject {
    @Published private(set) var devices: [CastDevice] = []
    @Published private(set) var active: CastDevice?
    @Published var notice: String?

    private weak var app: AppModel?
    private weak var player: PlayerModel?
    private var started = false
    private var followTask: Task<Void, Never>?
    private var quietUntil = Date.distantPast
    private var sentPlaying: Bool?
    private var loading = false

    func attach(_ app: AppModel, player: PlayerModel) {
        if started { return }
        started = true
        self.app = app
        self.player = player
        player.cast = self
        Task { await self.refreshLoop() }
    }

    // MARK: finding devices

    private func refreshLoop() async {
        while !Task.isCancelled {
            if app?.phase == .ready, app?.offline != true { await refresh() }
            try? await Task.sleep(nanoseconds: 30_000_000_000)
        }
    }

    func refresh() async {
        guard let api = app?.api,
              let data = try? await api.send(try api.request("/api/cast/devices")),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
        var list: [CastDevice] = []
        for d in (o["devices"] as? [[String: Any]]) ?? [] {
            list.append(CastDevice(id: (d["id"] as? String) ?? "", name: (d["name"] as? String) ?? "TV", model: (d["model"] as? String) ?? ""))
        }
        devices = list
    }

    /// Add a TV by its address, for when the server can't find it by itself.
    func add(host: String) async {
        guard let api = app?.api else { return }
        do {
            try await api.send(try api.request("/api/cast/devices", method: "POST", json: ["host": host.trimmingCharacters(in: .whitespaces)]))
            await refresh()
            notice = "Added"
        } catch {
            notice = error.localizedDescription
        }
    }

    // MARK: casting

    var isCasting: Bool { active != nil }

    func start(_ dev: CastDevice) {
        guard let player = player, player.hasCurrent, let id = player.currentID else {
            notice = "Start a song first, then cast it"
            return
        }
        active = dev
        player.setSilent(true)
        notice = "Connecting to \(dev.name)…"
        load(trackID: id, at: player.position, autoplay: player.isPlaying, first: true)
        followTask?.cancel()
        followTask = Task { await self.follow() }
    }

    func stop() {
        guard let dev = active else { return }
        finish("Stopped casting")
        control(on: dev.id, action: "stop", value: nil)
    }

    private func finish(_ message: String) {
        followTask?.cancel()
        followTask = nil
        active = nil
        sentPlaying = nil
        player?.setSilent(false)
        notice = message
    }

    /// Tell the TV to play this song from this spot.
    func load(trackID: String, at pos: Double, autoplay: Bool, first: Bool = false) {
        guard let api = app?.api, let dev = active else { return }
        loading = true
        sentPlaying = autoplay
        quietUntil = Date().addingTimeInterval(3)
        let body: [String: Any] = ["device": dev.id, "track": trackID, "at": max(0, pos), "autoplay": autoplay, "origin": api.base]
        Task {
            do {
                var req = try api.request("/api/cast/play", method: "POST", json: body)
                req.timeoutInterval = 45
                try await api.send(req)
                loading = false
                if first { notice = "Casting to \(dev.name)" }
            } catch {
                loading = false
                if first { finish(error.localizedDescription) } else { notice = error.localizedDescription }
            }
        }
    }

    /// play, pause, seek (value = seconds) or stop, sent to the TV.
    func control(_ action: String, value: Double? = nil) {
        guard let dev = active else { return }
        quietUntil = Date().addingTimeInterval(3)
        if action == "play" { sentPlaying = true }
        if action == "pause" { sentPlaying = false }
        control(on: dev.id, action: action, value: value)
    }

    private func control(on device: String, action: String, value: Double?) {
        guard let api = app?.api else { return }
        var body: [String: Any] = ["device": device, "action": action]
        if let v = value { body["value"] = v }
        Task {
            if let r = try? api.request("/api/cast/control", method: "POST", json: body) { _ = try? await api.send(r) }
        }
    }

    /// The TV is the clock: follow its play / pause (its own remote, the Google Home app) and its position.
    private func follow() async {
        while !Task.isCancelled {
            try? await Task.sleep(nanoseconds: 1_500_000_000)
            guard let api = app?.api, let dev = active, !loading else { continue }
            let q = dev.id.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? dev.id
            guard let data = try? await api.send(try api.request("/api/cast/status?device=" + q)),
                  let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let s = o["status"] as? [String: Any] else { continue }
            if active == nil { return }
            if (s["connected"] as? Bool) != true { finish("Lost the connection to \(dev.name)"); return }
            let state = s["playerState"] as? String
            if state == "IDLE", (s["idleReason"] as? String) == "CANCELLED" { finish("\(dev.name) is playing something else now"); return }
            if Date() < quietUntil { continue }
            if state == "PLAYING" || state == "PAUSED" {
                let remotePlaying = state == "PLAYING"
                if remotePlaying != sentPlaying {
                    sentPlaying = remotePlaying
                    player?.castFollow(playing: remotePlaying, position: nil)
                }
                if let t = s["currentTime"] as? Double, let here = player?.position, abs(here - t) > 2 {
                    player?.castFollow(playing: nil, position: t)
                }
            }
        }
    }
}
