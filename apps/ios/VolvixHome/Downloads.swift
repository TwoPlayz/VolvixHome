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
    /// how far each song that is coming down has got (0...1), for the little animated marks and the progress bar
    let live = DownloadProgress()
    /// songs asked for in this go, and how many of them are finished (so "3 of 12")
    @Published private(set) var batchTotal = 0
    @Published private(set) var batchDone = 0
    private var active: Set<String> = []
    private let fetcher = DownloadFetcher()

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
    func isComing(_ id: String) -> Bool { pending.contains(id) }

    /// Songs being fetched right now, in the order they were asked for.
    var comingNow: [String] { pending.filter { active.contains($0) } }

    /// How much of a group of songs is on the phone (0...1), counting the ones part way.
    func fraction(_ ids: [String]) -> Double {
        if ids.isEmpty { return 0 }
        let sum = ids.reduce(0.0) { $0 + (files[$1] != nil ? 1.0 : max(0, live.values[$1] ?? 0)) }
        return sum / Double(ids.count)
    }

    /// Everything in this go, as a fraction.
    var overall: Double {
        guard batchTotal > 0 else { return 0 }
        let part = live.values.values.reduce(0) { $0 + max(0, $1) }
        return min(1, (Double(batchDone) + part) / Double(batchTotal))
    }

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
        for id in ids where files[id] == nil && !pending.contains(id) { pending.append(id); batchTotal += 1 }
        start()
    }

    func remove(_ ids: [String]) {
        pending.removeAll { ids.contains($0) }
        for id in ids { live.values[id] = nil }
        if pending.isEmpty { batchTotal = 0; batchDone = 0 }
        for id in ids {
            if let name = files[id] { try? FileManager.default.removeItem(at: folder.appendingPathComponent(name)) }
            files[id] = nil
        }
        save()
    }

    /// Stop the songs that haven't come down yet. What is already on the phone stays.
    func cancel(_ ids: [String]) {
        let stopped = pending.filter { ids.contains($0) }.count
        pending.removeAll { ids.contains($0) }
        for id in ids where files[id] == nil { live.values[id] = nil }
        batchTotal = max(0, batchTotal - stopped)
        if pending.isEmpty { batchTotal = 0; batchDone = 0 }
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

    /// Several songs come down at once: one at a time leaves the connection idle between songs, which is what made
    /// albums crawl on a slow link.
    private static let parallel = 4

    private func run() async {
        await withTaskGroup(of: Void.self) { group in
            var inFlight = 0
            while true {
                if costly && app?.fx.downloadMobile != true && inFlight == 0 {
                    problem = "Waiting for Wi-Fi. Turn on \"Download on mobile data\" in Settings to download now."
                    break
                }
                if inFlight < Self.parallel, !(costly && app?.fx.downloadMobile != true), let id = pending.first(where: { !active.contains($0) }) {
                    active.insert(id)
                    working = id
                    inFlight += 1
                    group.addTask { @MainActor in await self.fetch(id) }
                } else if inFlight == 0 {
                    break
                } else {
                    await group.next()
                    inFlight -= 1
                }
            }
            await group.waitForAll()
        }
        working = nil
        running = false
        if pending.isEmpty { batchTotal = 0; batchDone = 0; live.values = [:] }
    }

    private func fetch(_ id: String) async {
        defer {
            let wanted = pending.contains(id)
            pending.removeAll { $0 == id }
            active.remove(id)
            live.values[id] = nil
            if wanted { batchDone += 1 }
        }
        guard let api = app?.api else { return }
        var q = app?.fx.downloadQuality ?? "stream"
        if q == "stream" { q = app?.quality ?? "original" }
        guard let url = api.streamURL(id, quality: q) else { return }
        var req = URLRequest(url: url)
        // a converted song is not sent until the server has made it, which can take minutes for a big one
        req.timeoutInterval = 300
        if let h = api.authHeader { req.setValue(h, forHTTPHeaderField: "Authorization") }
        // until the first bytes arrive it is "coming, can't say how far": a moving ring, not a stuck one
        live.values[id] = -1
        // one more try if the connection drops part way (a hotspot does that)
        for attempt in 0..<2 {
            do {
                let last = ProgressGate()
                let result = try await fetcher.fetch(req) { [weak self] p in
                    guard last.pass(p) else { return }
                    Task { @MainActor in
                        guard let me = self, me.pending.contains(id) else { return }
                        me.live.values[id] = p
                    }
                }
                guard (200..<300).contains(result.1.statusCode) else {
                    try? FileManager.default.removeItem(at: result.0)
                    problem = result.1.statusCode == 404
                        ? "A song isn't on the server any more. Refreshing the library."
                        : "The server wouldn't send one of the songs."
                    if result.1.statusCode == 404 { Task { await app?.refresh() } }
                    return
                }
                // asked to stop while it came down
                if !pending.contains(id) { try? FileManager.default.removeItem(at: result.0); return }
                let name = id + "." + fileExtension(result.1.value(forHTTPHeaderField: "Content-Type"))
                var dest = folder.appendingPathComponent(name)
                try? FileManager.default.removeItem(at: dest)
                try FileManager.default.moveItem(at: result.0, to: dest)
                var values = URLResourceValues()
                values.isExcludedFromBackup = true
                try? dest.setResourceValues(values)
                files[id] = name
                save()
                return
            } catch {
                if attempt == 1 { problem = "A download didn't finish. Check the connection and try again." }
            }
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

/// How far each song has got. Separate from the store so a tick of progress redraws only the marks that show it.
final class DownloadProgress: ObservableObject {
    @Published var values: [String: Double] = [:]
}

/// Moves in small steps, so the progress isn't redrawn for every few bytes.
final class ProgressGate: @unchecked Sendable {
    private var last = -1.0
    func pass(_ p: Double) -> Bool {
        if p >= 1 || p - last >= 0.03 || last < 0 { last = p; return true }
        return false
    }
}

/// Downloads songs with progress, several at a time.
final class DownloadFetcher: NSObject, URLSessionDownloadDelegate, @unchecked Sendable {
    private struct Job {
        var progress: (Double) -> Void
        var done: CheckedContinuation<(URL, HTTPURLResponse), Error>
        var file: URL?
        var response: HTTPURLResponse?
    }

    private var jobs: [Int: Job] = [:]
    private let lock = NSLock()
    private lazy var session: URLSession = {
        let c = URLSessionConfiguration.default
        c.httpMaximumConnectionsPerHost = 6
        c.timeoutIntervalForRequest = 300
        c.timeoutIntervalForResource = 3600
        c.requestCachePolicy = .reloadIgnoringLocalCacheData
        return URLSession(configuration: c, delegate: self, delegateQueue: nil)
    }()

    func fetch(_ req: URLRequest, progress: @escaping (Double) -> Void) async throws -> (URL, HTTPURLResponse) {
        try await withCheckedThrowingContinuation { cont in
            let task = session.downloadTask(with: req)
            lock.lock(); jobs[task.taskIdentifier] = Job(progress: progress, done: cont); lock.unlock()
            task.resume()
        }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        lock.lock(); let job = jobs[downloadTask.taskIdentifier]; lock.unlock()
        // no size from the server (a converted song is sent as it is made): -1 = "coming, can't say how far"
        guard totalBytesExpectedToWrite > 0 else { job?.progress(-1); return }
        job?.progress(min(1, Double(totalBytesWritten) / Double(totalBytesExpectedToWrite)))
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        // the file is deleted when this returns: move it somewhere first
        let keep = FileManager.default.temporaryDirectory.appendingPathComponent("dl-" + UUID().uuidString)
        try? FileManager.default.moveItem(at: location, to: keep)
        lock.lock()
        jobs[downloadTask.taskIdentifier]?.file = keep
        jobs[downloadTask.taskIdentifier]?.response = downloadTask.response as? HTTPURLResponse
        lock.unlock()
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        lock.lock(); let job = jobs.removeValue(forKey: task.taskIdentifier); lock.unlock()
        guard let job = job else { return }
        if let error = error {
            if let f = job.file { try? FileManager.default.removeItem(at: f) }
            job.done.resume(throwing: error)
        } else if let f = job.file, let r = job.response ?? (task.response as? HTTPURLResponse) {
            job.progress(1)
            job.done.resume(returning: (f, r))
        } else {
            job.done.resume(throwing: ServerError(message: "The server didn't answer."))
        }
    }
}

/// The little icon beside a song that is kept on the phone, coming down (it moves), or waiting.
struct DownloadMark: View {
    @EnvironmentObject var downloads: DownloadStore
    let id: String
    var size: CGFloat = 14

    var body: some View {
        if downloads.has(id) {
            Image(systemName: "arrow.down")
                .font(.system(size: size * 0.55, weight: .bold)).foregroundColor(.white)
                .frame(width: size, height: size).background(Circle().fill(Theme.accent))
                .accessibilityLabel("Downloaded")
        } else if downloads.isComing(id) {
            ComingMark(live: downloads.live, id: id, size: size)
                .accessibilityLabel("Downloading")
        }
    }
}

struct ComingMark: View {
    @ObservedObject var live: DownloadProgress
    let id: String
    let size: CGFloat
    var body: some View { DownloadingGlyph(size: size, progress: live.values[id]) }
}

struct OverallGlyph: View {
    @ObservedObject var live: DownloadProgress
    @EnvironmentObject var downloads: DownloadStore
    var ids: [String]? = nil
    let size: CGFloat
    var body: some View { DownloadingGlyph(size: size, progress: ids.map { downloads.fraction($0) } ?? downloads.overall) }
}

/// A ring that fills as the song comes down, with an arrow that keeps moving so it is clear something is happening.
struct DownloadingGlyph: View {
    var size: CGFloat = 14
    var progress: Double?
    @State private var bounce = false
    @State private var spin = false

    var body: some View {
        let known = (progress ?? 0) >= 0
        ZStack {
            Circle().stroke(Theme.text3.opacity(0.35), lineWidth: max(1.5, size / 8))
            Circle().trim(from: 0, to: known ? CGFloat(max(0.1, progress ?? 0.1)) : 0.3)
                .stroke(Theme.accent, style: StrokeStyle(lineWidth: max(1.5, size / 8), lineCap: .round))
                .rotationEffect(.degrees(known ? -90 : (spin ? 270 : -90)))
            Image(systemName: "arrow.down").font(.system(size: size * 0.5, weight: .bold)).foregroundColor(Theme.accent)
                .offset(y: bounce ? size * 0.12 : -size * 0.12)
        }
        .frame(width: size, height: size)
        .animation(.easeInOut(duration: 0.25), value: progress)
        .onAppear {
            withAnimation(.easeInOut(duration: 0.7).repeatForever(autoreverses: true)) { bounce = true }
            withAnimation(.linear(duration: 1.0).repeatForever(autoreverses: false)) { spin = true }
        }
    }
}

/// While songs are coming down: what is happening, wherever you are in the app.
struct DownloadBanner: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var downloads: DownloadStore

    var body: some View {
        if !downloads.pending.isEmpty && downloads.batchTotal > 0 {
            let total = downloads.batchTotal
            let n = min(total, downloads.batchDone + 1)
            let title = downloads.comingNow.first.flatMap { app.track($0)?.title }
            VStack(spacing: 0) {
                HStack(spacing: 10) {
                    OverallGlyph(live: downloads.live, size: 20)
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Downloading \(n) of \(total)").font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text)
                        Text(downloads.problem ?? title ?? "Starting…").font(.system(size: 11)).foregroundColor(Theme.text2).lineLimit(1)
                    }
                    Spacer()
                    OverallPercent(live: downloads.live)
                }
                .padding(.horizontal, 14).padding(.vertical, 8)
                OverallBar(live: downloads.live)
            }
            .background(Theme.surface2)
        }
    }
}

struct OverallPercent: View {
    @ObservedObject var live: DownloadProgress
    @EnvironmentObject var downloads: DownloadStore
    var body: some View {
        Text("\(Int((downloads.overall * 100).rounded()))%").font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text2).monospacedDigit()
    }
}

struct OverallBar: View {
    @ObservedObject var live: DownloadProgress
    @EnvironmentObject var downloads: DownloadStore
    var body: some View {
        GeometryReader { g in
            Rectangle().fill(Theme.accent).frame(width: g.size.width * CGFloat(downloads.overall), height: 2)
                .animation(.easeInOut(duration: 0.3), value: downloads.overall)
        }
        .frame(height: 2)
    }
}

/// A download button for a group of songs (an album, a playlist): tap to keep them on the phone, tap again to remove them.
struct DownloadButton: View {
    @EnvironmentObject var downloads: DownloadStore
    let ids: [String]

    var body: some View {
        let done = downloads.hasAll(ids)
        let busy = downloads.isBusy(ids)
        Button {
            if done { downloads.remove(ids) } else if busy { downloads.cancel(ids) } else { downloads.add(ids) }
        } label: {
            ZStack {
                Circle().fill(Theme.surface3).frame(width: 44, height: 44)
                if busy {
                    OverallGlyph(live: downloads.live, ids: ids, size: 24)
                } else {
                    Image(systemName: done ? "checkmark.circle.fill" : "arrow.down.circle")
                        .font(.system(size: 20)).foregroundColor(done ? Theme.accent : Theme.text)
                }
            }
        }
        .accessibilityLabel(done ? "Remove download" : "Download")
    }
}
