import Foundation
import SwiftUI
import UIKit

struct DeviceInfo: Identifiable, Hashable {
    var id: String
    var name: String
    var active: Bool
}

/// Your other devices where Volvix Home is open (the website, the Windows app, another phone):
/// see them, and move the music from one to another with the song, the queue and the spot.
@MainActor
final class DeviceHub: ObservableObject {
    @Published private(set) var devices: [DeviceInfo] = []
    @Published private(set) var activeID: String?
    @Published var notice: String?

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
            // another device took over: this one goes quiet
            if let a = activeID, a != deviceID, a != before, player?.isPlaying == true {
                player?.pause()
                let n = list.first(where: { $0.id == a })?.name ?? "another device"
                notice = "Playing on \(n)"
            }
        case "transfer":
            if (e["to"] as? String) == deviceID, let s = e["state"] as? [String: Any] { receive(s) }
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

    // MARK: things you do

    /// This device is the one playing now: the others pause.
    func announce() {
        guard let api = app?.api, app?.offline != true else { return }
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
        player.pause()
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
