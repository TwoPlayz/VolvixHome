import Foundation
import SwiftUI
import UIKit

struct DeviceInfo: Identifiable, Hashable {
    var id: String
    var name: String
    var active: Bool
}

/// What the device that is playing says it is playing, so this one can show it and control it.
struct RemoteNow: Equatable {
    var deviceID: String
    var name: String
    var queue: [String]
    var index: Int
    var playing: Bool
    var posMs: Double
    var updatedAt: Double
    /// the server's clock minus this phone's, in ms
    var offsetMs: Double
}

/// Your other devices where Volvix Home is open (the website, the Windows app, another phone):
/// see them, and move the music from one to another with the song, the queue and the spot.
@MainActor
final class DeviceHub: ObservableObject {
    @Published private(set) var devices: [DeviceInfo] = []
    @Published private(set) var activeID: String?
    @Published var notice: String?
    /// another device is playing: this one shows it silently and sends it what you tap
    @Published private(set) var remote: RemoteNow?
    private var nowState: [String: Any]?
    /// right after starting here, the old player's last report must not pull this phone back into mirroring it
    private var remoteBlock = Date.distantPast
    private var lastKey = ""
    private var lastSent = Date.distantPast

    let deviceID: String
    private weak var app: AppModel?
    private weak var player: PlayerModel?
    private var started = false

    init() {
        let key = "volvix.device.id"
        if let saved = UserDefaults.standard.string(forKey: key), !saved.isEmpty {
            deviceID = saved
        } else {
            let fresh = "ios-" + String(UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased().prefix(16))
            UserDefaults.standard.set(fresh, forKey: key)
            deviceID = fresh
        }
    }

    var deviceName: String { UIDevice.current.userInterfaceIdiom == .pad ? "iPad · app" : "iPhone · app" }
    var others: [DeviceInfo] { devices.filter { $0.id != deviceID } }
    var playingHere: Bool { activeID == nil || activeID == deviceID }

    func attach(_ app: AppModel, player: PlayerModel) {
        if started { return }
        started = true
        self.app = app
        self.player = player
        player.hub = self
        Task { await self.run() }
    }

    // MARK: the live line to the server

    private func run() async {
        while !Task.isCancelled {
            if let app = app, app.phase == .ready, !app.offline, let api = app.api {
                await listen(api)
            }
            devices = []
            activeID = nil
            try? await Task.sleep(nanoseconds: 3_000_000_000)
        }
    }

    private func listen(_ api: API) async {
        let name = deviceName.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? "iPhone"
        guard var req = try? api.request("/api/devices/events?device=\(deviceID)&name=\(name)") else { return }
        req.timeoutInterval = 120
        req.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        do {
            let (bytes, response) = try await URLSession.shared.bytes(for: req)
            guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else { return }
            for try await line in bytes.lines {
                if Task.isCancelled { return }
                if app?.phase != .ready { return }
                guard line.hasPrefix("data:") else { continue }
                let body = line.dropFirst(5).trimmingCharacters(in: .whitespaces)
                if let data = body.data(using: .utf8), let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                    handle(obj)
                }
            }
        } catch {
            // the connection dropped: run() looks again in a few seconds
        }
    }

    private func handle(_ e: [String: Any]) {
        switch e["type"] as? String {
        case "devices":
            let before = activeID
            var list: [DeviceInfo] = []
            for d in (e["devices"] as? [[String: Any]]) ?? [] {
                list.append(DeviceInfo(id: (d["id"] as? String) ?? "", name: (d["name"] as? String) ?? "Device", active: (d["active"] as? Bool) ?? false))
            }
            devices = list
            activeID = e["active"] as? String
            nowState = e["now"] as? [String: Any]
            // another device took over: this one goes quiet (only if this phone itself was playing; when it was only
            // showing the other device, pausing would pause that device)
            if let a = activeID, a != deviceID, a != before, player?.isPlaying == true, remote == nil {
                player?.pause()
                let n = list.first(where: { $0.id == a })?.name ?? "another device"
                notice = "Playing on \(n)"
            }
            syncRemote()
            if activeID == deviceID { report(force: true) }
        case "transfer":
            if (e["to"] as? String) == deviceID, let s = e["state"] as? [String: Any] { receive(s) }
        case "now":
            if (e["device"] as? String) == activeID, activeID != deviceID {
                nowState = e["state"] as? [String: Any]
                syncRemote()
            }
        case "command":
            if (e["to"] as? String) == deviceID, let c = e["cmd"] as? [String: Any] { player?.handleCommand(c) }
        default:
            break
        }
    }

    /// Music moved to this device: carry on with the same queue, song and spot.
    private func receive(_ s: [String: Any]) {
        guard let app = app, let player = player else { return }
        let ids = ((s["queue"] as? [String]) ?? []).filter { app.track($0) != nil }
        let wantIndex = (s["index"] as? Int) ?? 0
        let all = (s["queue"] as? [String]) ?? []
        guard wantIndex >= 0, wantIndex < all.count, let at = ids.firstIndex(of: all[wantIndex]) else { return }
        let pos = (s["pos"] as? Double) ?? 0
        let playing = (s["playing"] as? Bool) ?? true
        let repeatText = (s["repeat"] as? String) ?? "off"
        player.restore(ids, index: at, position: pos, playing: playing, repeatMode: repeatText == "all" ? .all : (repeatText == "one" ? .one : .off))
        notice = playing ? "Playing here" : "Press play to continue here"
        if playing { announce() }
    }

    // MARK: remote control

    /// Show what the playing device plays (or stop, when nothing else is playing).
    private func syncRemote() {
        guard let player = player, Date() >= remoteBlock else { return }
        if player.partyOn { return }
        if let a = activeID, a != deviceID, let n = nowState, let q = n["queue"] as? [String], !q.isEmpty {
            let name = devices.first(where: { $0.id == a })?.name ?? "another device"
            let server = decimal(n["serverNow"])
            let r = RemoteNow(deviceID: a, name: name, queue: q, index: wholeNumber(n["index"]), playing: (n["playing"] as? Bool) ?? false,
                              posMs: decimal(n["posMs"]), updatedAt: decimal(n["updatedAt"]),
                              offsetMs: server > 0 ? server - Date().timeIntervalSince1970 * 1000 : 0)
            if r != remote { remote = r }
            player.applyRemote(r)
        } else if remote != nil {
            remote = nil
            player.clearRemote()
        }
    }

    /// Ask the playing device to do something (play, pause, next, previous, seek, jump in its queue).
    func command(_ op: String, _ extra: [String: Any] = [:]) {
        guard let api = app?.api, let r = remote else { return }
        var cmd = extra
        cmd["op"] = op
        Task { _ = try? await api.send(try api.request("/api/devices/command", method: "POST", json: ["device": deviceID, "to": r.deviceID, "cmd": cmd])) }
    }

    /// The playing device tells the others what it is playing.
    func report(force: Bool = false) {
        // the one playing reports; a phone that is playing but not yet listed as the active one does too (the server then lists it)
        guard let api = app?.api, let p = player, activeID == deviceID || p.isPlaying, remote == nil, p.hasCurrent, !p.partyOn, app?.offline != true else { return }
        let key = "\(p.currentID ?? "")|\(p.index)|\(p.isPlaying)|\(p.queue.count)"
        if !force && key == lastKey && Date().timeIntervalSince(lastSent) < 5 { return }
        lastKey = key
        lastSent = Date()
        let state = p.partySnapshot()
        let me = deviceID
        Task { _ = try? await api.send(try api.request("/api/devices/state", method: "POST", json: ["device": me, "state": state])) }
    }

    /// Take the music over from the device that is playing: same queue, song and spot, now playing here.
    func takeOver() {
        guard let r = remote, let player = player, let app = app, r.index >= 0, r.index < r.queue.count else { return }
        let ids = r.queue.filter { app.track($0) != nil }
        guard let at = ids.firstIndex(of: r.queue[r.index]) else { return }
        let pos = player.position
        remote = nil
        nowState = nil
        player.clearRemote()
        player.restore(ids, index: at, position: pos, playing: r.playing, repeatMode: .off)
        announce()
        notice = "Playing here"
    }

    /// Stop following another device (you started something here).
    func leaveRemote() {
        if remote == nil { return }
        remote = nil
        nowState = nil
        player?.clearRemote()
    }

    // MARK: things you do

    /// This device is the one playing now: the others pause.
    func announce() {
        guard let api = app?.api, app?.offline != true else { return }
        remoteBlock = Date().addingTimeInterval(2.5)
        let me = deviceID
        Task {
            if let r = try? api.request("/api/devices/active", method: "POST", json: ["device": me]) { _ = try? await api.send(r) }
        }
    }

    /// Move what is playing to another of your devices.
    func transfer(to id: String) {
        guard let api = app?.api, let player = player, player.hasCurrent else {
            notice = "Start a song first, then move it"
            return
        }
        let state = player.snapshot()
        let name = devices.first(where: { $0.id == id })?.name ?? "that device"
        if remote == nil { player.pause() }
        notice = "Playing on \(name)"
        let me = deviceID
        Task {
            do {
                try await api.send(try api.request("/api/devices/transfer", method: "POST", json: ["device": me, "to": id, "state": state]))
            } catch {
                notice = error.localizedDescription
            }
        }
    }
}
