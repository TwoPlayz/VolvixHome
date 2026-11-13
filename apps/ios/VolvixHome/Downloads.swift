import Foundation
import Network
import SwiftUI

/// A small place on the phone for what the app keeps: the library copy, your data, and downloaded songs.
enum Persist {
    static var dir: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let u = base.appendingPathComponent("Volvix", isDirectory: true)
        try? FileManager.default.createDirectory(at: u, withIntermediateDirectories: true)
        return u
    }

    static func write(_ data: Data, _ name: String) {
        try? data.write(to: dir.appendingPathComponent(name), options: .atomic)
    }

    static func read(_ name: String) -> Data? {
        try? Data(contentsOf: dir.appendingPathComponent(name))
    }

    static func remove(_ name: String) {
        try? FileManager.default.removeItem(at: dir.appendingPathComponent(name))
    }
}

/// Songs kept on this phone, so they play without the server (on a plane, with no signal).
@MainActor
final class DownloadStore: ObservableObject {
    @Published private(set) var files: [String: String] = [:]   // song id -> file name
    @Published private(set) var pending: [String] = []
    @Published private(set) var working: String?
    @Published var problem: String?

    private weak var app: AppModel?
    private var running = false
    private let monitor = NWPathMonitor()
    /// on mobile data (or another connection that costs): downloads wait for Wi-Fi unless the setting says otherwise
    private var costly = false

    init() {
        monitor.pathUpdateHandler = { [weak self] path in
            let costly = path.isExpensive || path.usesInterfaceType(.cellular)
            Task { @MainActor in
                guard let me = self else { return }
                let was = me.costly
                me.costly = costly
                if was && !costly && !me.pending.isEmpty { me.problem = nil; me.start() }
            }
        }
        monitor.start(queue: DispatchQueue(label: "volvix.network"))
    }

    private var folder: URL {
        let u = Persist.dir.appendingPathComponent("Downloads", isDirectory: true)
        try? FileManager.default.createDirectory(at: u, withIntermediateDirectories: true)
        return u
    }

    func attach(_ app: AppModel) {
        if self.app === app { return }
        self.app = app
        if let data = Persist.read("downloads.json"),
           let m = try? JSONSerialization.jsonObject(with: data) as? [String: String] {
            // forget anything whose file went missing
            files = m.filter { FileManager.default.fileExists(atPath: folder.appendingPathComponent($0.value).path) }
        }
    }

    func has(_ id: String) -> Bool { files[id] != nil }

    func localURL(_ id: String) -> URL? {
        guard let name = files[id] else { return nil }
        return folder.appendingPathComponent(name)
    }

    func hasAll(_ ids: [String]) -> Bool { !ids.isEmpty && ids.allSatisfy { files[$0] != nil } }
    func isBusy(_ ids: [String]) -> Bool { ids.contains { pending.contains($0) } }

    var count: Int { files.count }

    var bytes: Int64 {
        var total: Int64 = 0
        for name in files.values {
            let p = folder.appendingPathComponent(name).path
            if let a = try? FileManager.default.attributesOfItem(atPath: p), let n = a[.size] as? NSNumber { total += n.int64Value }
        }
        return total
    }

    func add(_ ids: [String]) {
        problem = nil
        for id in ids where files[id] == nil && !pending.contains(id) { pending.append(id) }
        start()
    }

    func remove(_ ids: [String]) {
        pending.removeAll { ids.contains($0) }
        for id in ids {
            if let name = files[id] { try? FileManager.default.removeItem(at: folder.appendingPathComponent(name)) }
            files[id] = nil
        }
        save()
    }

    func removeAll() {
        pending = []
        remove(Array(files.keys))
    }

    private func save() {
        if let data = try? JSONSerialization.data(withJSONObject: files) { Persist.write(data, "downloads.json") }
    }

    private func start() {
        if running { return }
        running = true
        Task { await run() }
    }

    private func run() async {
        while let id = pending.first {
            if costly && app?.fx.downloadMobile != true {
                problem = "Waiting for Wi-Fi. Turn on \"Download on mobile data\" in Settings to download now."
                working = nil
                running = false
                return
            }
            working = id
            await fetch(id)
            pending.removeAll { $0 == id }
        }
        working = nil
        running = false
    }

    private func fetch(_ id: String) async {
        guard let api = app?.api else { return }
        var q = app?.fx.downloadQuality ?? "stream"
        if q == "stream" { q = app?.quality ?? "original" }
        guard let url = api.streamURL(id, quality: q) else { return }
        var req = URLRequest(url: url)
        req.timeoutInterval = 120
        if let h = api.authHeader { req.setValue(h, forHTTPHeaderField: "Authorization") }
        do {
            let result = try await URLSession.shared.download(for: req)
            guard let http = result.1 as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
                problem = "The server wouldn't send one of the songs."
                return
            }
            let name = id + "." + fileExtension(http.value(forHTTPHeaderField: "Content-Type"))
            var dest = folder.appendingPathComponent(name)
            try? FileManager.default.removeItem(at: dest)
            try FileManager.default.moveItem(at: result.0, to: dest)
            var values = URLResourceValues()
            values.isExcludedFromBackup = true
            try? dest.setResourceValues(values)
            files[id] = name
            save()
        } catch {
            problem = "A download didn't finish. Check the connection and try again."
        }
    }

    private func fileExtension(_ contentType: String?) -> String {
        let t = (contentType ?? "").lowercased()
        if t.contains("mpeg") || t.contains("mp3") { return "mp3" }
        if t.contains("flac") { return "flac" }
        if t.contains("mp4") || t.contains("m4a") || t.contains("aac") { return "m4a" }
        if t.contains("wav") { return "wav" }
        if t.contains("aiff") { return "aiff" }
        if t.contains("ogg") { return "ogg" }
        return "audio"
    }
}

func formatBytes(_ n: Int64) -> String {
    let f = ByteCountFormatter()
    f.countStyle = .file
    return f.string(fromByteCount: n)
}

/// A download button for a group of songs (an album, a playlist): tap to keep them on the phone, tap again to remove them.
struct DownloadButton: View {
    @EnvironmentObject var downloads: DownloadStore
    let ids: [String]

    var body: some View {
        let done = downloads.hasAll(ids)
        let busy = downloads.isBusy(ids)
        Button {
            if done || busy { downloads.remove(ids) } else { downloads.add(ids) }
        } label: {
            ZStack {
                Circle().fill(Theme.surface3).frame(width: 44, height: 44)
                if busy {
                    ProgressView().tint(Theme.text2)
                } else {
                    Image(systemName: done ? "checkmark.circle.fill" : "arrow.down.circle")
                        .font(.system(size: 20)).foregroundColor(done ? Theme.accent : Theme.text)
                }
            }
        }
        .accessibilityLabel(done ? "Remove download" : "Download")
    }
}
