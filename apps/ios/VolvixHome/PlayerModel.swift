import Foundation
import AVFoundation
import MediaPlayer
import UIKit
import Combine

/// The song's position and length. Only the views that show them watch it.
@MainActor
final class PlayClock: ObservableObject {
    @Published var position: Double = 0
    @Published var duration: Double = 0
}

/// The music player: a queue, play/pause/seek, and the lock screen and headphone buttons.
/// Plays through the system's own audio engine, so it keeps going in the background.
@MainActor
final class PlayerModel: ObservableObject {
    enum Repeat { case off, all, one }

    @Published private(set) var queue: [String] = []     // track ids
    @Published private(set) var index: Int = -1
    @Published private(set) var isPlaying = false
    /// where the song is: kept apart so only the progress bar and lyrics redraw twice a second, not every list
    let clock = PlayClock()
    var position: Double {
        get { clock.position }
        set { if clock.position != newValue { clock.position = newValue } }
    }
    var duration: Double {
        get { clock.duration }
        set { if clock.duration != newValue { clock.duration = newValue } }
    }
    @Published private(set) var loading = false
    @Published var shuffle = false
    @Published var repeatMode: Repeat = .off
    @Published var problem: String?
    /// when the sleep timer pauses the music (nil = off); and "stop when this song ends"
    @Published private(set) var sleepAt: Date?
    @Published private(set) var sleepAfterSong = false

    private var player = AVPlayer()
    private var observedPlayer: AVPlayer?
    private var timeObserver: Any?
    /// in a listening party the shared queue drives this player
    weak var party: PartyHub?
    @Published private(set) var partyOn = false
    /// what the music is playing from (an album, a playlist…), for the full player
    @Published private(set) var contextName: String?
    private var applyingParty = false
    /// songs fetched by the app itself when the player couldn't open the stream (song id -> file)
    private var fetched: [String: URL] = [:]
    private var retriedFor: String?
    /// another device is playing: this phone mirrors it (silently) and sends it what you tap
    @Published private(set) var remoteName: String?
    private var remoteState: RemoteNow?
    private var remoteTimer: Timer?
    private var silent = false
    private var xfading = false
    private var fadingOut: AVPlayer?
    private var currentProc: EQProcessor?
    private var fxWatch: AnyCancellable?
    private let recorder = PlayRecorder()
    private var lastDriftCheck = Date.distantPast
    private var endObserver: NSObjectProtocol?
    private var statusObserver: NSKeyValueObservation?
    private weak var app: AppModel?
    private weak var downloads: DownloadStore?
    /// the live line to your other devices (set by DeviceHub)
    weak var hub: DeviceHub?
    /// TVs and speakers: while casting this phone stays silent and keeps the clock
    weak var cast: CastHub?
    private var suppressCast = false
    private var pendingSeek: Double?
    private var lastReported: String?
    private var artworkFor: String?
    private var artwork: MPMediaItemArtwork?

    var currentID: String? { (index >= 0 && index < queue.count) ? queue[index] : nil }
    var upcoming: [String] { index + 1 < queue.count ? Array(queue[(index + 1)...]) : [] }
    var hasCurrent: Bool { currentID != nil }

    func attach(_ app: AppModel, downloads: DownloadStore) {
        if self.app === app { return }
        self.app = app
        self.downloads = downloads
        setUpAudioSession()
        setUpRemoteCommands()
        observe(player)
        fxWatch = app.$fx.sink { [weak self] f in Task { @MainActor in self?.fxChanged(f) } }
        NotificationCenter.default.addObserver(forName: .eqNeedsRate, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor in if let f = self?.app?.fx { self?.fxChanged(f) } }
        }
        NotificationCenter.default.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] n in
            let raw = (n.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt) ?? 0
            let opts = (n.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt) ?? 0
            Task { @MainActor in self?.interruption(typeRaw: raw, optionsRaw: opts) }
        }
        resumeLast()
    }

    // MARK: starting music

    /// Play these songs, starting at one of them.
    func play(_ ids: [String], startAt: Int = 0, shuffled: Bool = false, from: String? = nil) {
        guard !ids.isEmpty else { return }
        if inParty { partyPlay(ids, startAt, shuffled); return }
        if inRemote { hub?.leaveRemote() }
        contextName = from
        var list = ids
        var start = max(0, min(startAt, ids.count - 1))
        // with no server, only what is downloaded can play
        if app?.localOnly == true, let d = downloads {
            let wanted = ids[start]
            list = ids.filter { d.has($0) }
            if list.isEmpty {
                problem = "None of these songs are downloaded, and the server can't be reached."
                return
            }
            start = list.firstIndex(of: wanted) ?? 0
        }
        if shuffled {
            let first = list[start]
            list.remove(at: start)
            list.shuffle()
            list.insert(first, at: 0)
            start = 0
            shuffle = true
        }
        queue = list
        index = start
        problem = nil
        hub?.announce()
        load(autoplay: true)
        if let id = currentID, let t = app?.track(id) { app?.playedAlbum(t.albumId) }
    }

    func playNext(_ id: String) {
        if inRemote { hub?.notice = "Not available while you control another device"; return }
        if inParty { party?.fire("addNext", ["ids": [id]]); party?.notice = "Playing next in the party"; return }
        if queue.isEmpty { play([id]); return }
        queue.insert(id, at: min(index + 1, queue.count))
    }

    func playNext(ids: [String]) {
        if inRemote { hub?.notice = "Not available while you control another device"; return }
        if inParty { party?.fire("addNext", ["ids": ids]); party?.notice = "Playing next in the party"; return }
        if queue.isEmpty { play(ids); return }
        queue.insert(contentsOf: ids, at: min(index + 1, queue.count))
    }

    func addToQueue(ids: [String]) {
        if inRemote { hub?.notice = "Not available while you control another device"; return }
        if inParty { party?.fire("add", ["ids": ids]); party?.notice = "Added to the party queue"; return }
        if queue.isEmpty { play(ids); return }
        queue.append(contentsOf: ids)
    }

    func addToQueue(_ id: String) {
        if inRemote { hub?.notice = "Not available while you control another device"; return }
        if inParty { party?.fire("add", ["ids": [id]]); party?.notice = "Added to the party queue"; return }
        if queue.isEmpty { play([id]); return }
        queue.append(id)
    }

    func jump(to i: Int) {
        guard i >= 0, i < queue.count else { return }
        if inParty && !applyingParty { party?.fire("playAt", ["index": i]); return }
        if inRemote { hub?.command("playAt", ["index": i]); return }
        index = i
        load(autoplay: true)
    }

    func removeFromQueue(at i: Int) {
        guard i > index, i < queue.count else { return }
        if inParty { party?.fire("remove", ["index": i]); return }
        if inRemote { hub?.notice = "Not available while you control another device"; return }
        queue.remove(at: i)
    }

    /// What another device needs to carry on: the queue around now, the song, the spot.
    func snapshot() -> [String: Any] {
        let from = max(0, index - 50)
        let to = min(queue.count, index + 450)
        let slice = Array(queue[from..<to])
        return [
            "queue": slice,
            "index": index - from,
            "pos": position,
            "playing": isPlaying,
            "context": NSNull(),
            "shuffle": shuffle,
            "repeat": repeatMode == .all ? "all" : (repeatMode == .one ? "one" : "off"),
        ]
    }

    /// Carry on with a queue handed over from another device.
    func restore(_ ids: [String], index i: Int, position: Double, playing: Bool, repeatMode mode: Repeat) {
        guard !ids.isEmpty, i >= 0, i < ids.count else { return }
        if inParty { return }
        queue = ids
        index = i
        repeatMode = mode
        problem = nil
        pendingSeek = position > 0 ? position : nil
        load(autoplay: playing)
    }

    // MARK: controls

    func toggle() {
        guard hasCurrent else { return }
        if isPlaying { pause() } else { resume() }
    }

    func resume() {
        guard hasCurrent else { return }
        if inParty && !applyingParty { party?.fire("play"); return }
        if inRemote { hub?.command("play"); return }
        // after another device stopped playing nothing is loaded here yet: load the song and carry on from the spot
        if player.currentItem == nil {
            pendingSeek = position > 1 ? position : nil
            load(autoplay: true)
            return
        }
        if cast?.isCasting != true { hub?.announce() }
        if cast?.isCasting == true && !suppressCast { cast?.control("play") }
        player.play()
        isPlaying = true
        publishNowPlaying()
        report()
    }

    func pause() {
        if inParty && !applyingParty { party?.fire("pause"); return }
        if inRemote { hub?.command("pause"); return }
        fadingOut?.pause()
        if cast?.isCasting == true && !suppressCast { cast?.control("pause") }
        player.pause()
        isPlaying = false
        publishNowPlaying()
        report()
    }

    func next() {
        if queue.isEmpty { return }
        if inParty && !applyingParty { party?.fire("next"); return }
        if inRemote { hub?.command("next"); return }
        if index + 1 < queue.count {
            index += 1
            load(autoplay: true)
        } else if repeatMode == .all {
            index = 0
            load(autoplay: true)
        } else {
            pause()
            seek(to: 0)
        }
    }

    func previous() {
        if inParty && !applyingParty { party?.fire("prev"); return }
        if inRemote { hub?.command("prev"); return }
        // a few seconds in: back to the start of the song; at the start: the one before
        if position > 3 || index <= 0 {
            seek(to: 0)
        } else {
            index -= 1
            load(autoplay: true)
        }
    }

    /// Keep playing but make no sound here (the TV has it).
    func setSilent(_ on: Bool) {
        silent = on
        player.isMuted = on
    }

    /// The TV changed (its remote, the Google Home app): follow it without telling it back.
    func castFollow(playing: Bool?, position pos: Double?) {
        suppressCast = true
        defer { suppressCast = false }
        if let p = playing {
            if p { resume() } else { pause() }
        }
        if let s = pos { seek(to: s) }
    }

    func seek(to seconds: Double) {
        if inParty && !applyingParty { party?.fire("seek", ["posMs": Int(max(0, seconds) * 1000)]); return }
        if inRemote { hub?.command("seek", ["pos": max(0, seconds)]); return }
        if cast?.isCasting == true && !suppressCast { cast?.control("seek", value: max(0, seconds)) }
        seekLocal(seconds)
    }

    private func seekLocal(_ seconds: Double) {
        defer { hub?.report(force: true) }
        let t = CMTime(seconds: max(0, seconds), preferredTimescale: 600)
        player.seek(to: t, toleranceBefore: .zero, toleranceAfter: .zero)
        position = max(0, seconds)
        publishNowPlaying()
    }

    func cycleRepeat() {
        switch repeatMode {
        case .off: repeatMode = .all
        case .all: repeatMode = .one
        case .one: repeatMode = .off
        }
    }

    func toggleShuffle() {
        shuffle.toggle()
        if shuffle && queue.count > index + 2 {
            var rest = Array(queue[(index + 1)...])
            rest.shuffle()
            queue = Array(queue[...index]) + rest
        }
    }

    // MARK: loading a song

    private func load(autoplay: Bool) {
        guard let app = app, let api = app.api, let id = currentID, let url = api.streamURL(id) else { return }
        if app.user?.hideExplicit == true, !inParty, let t = app.track(id), t.explicit {
            if index + 1 < queue.count {
                index += 1
                load(autoplay: autoplay)
            } else {
                problem = "Explicit songs are turned off for you."
                isPlaying = false
            }
            return
        }
        problem = nil
        loading = true
        position = 0
        let t = app.track(id)
        duration = Double(t?.duration ?? 0)
        var options: [String: Any] = [:]
        if let h = api.authHeader { options["AVURLAssetHTTPHeaderFieldsKey"] = ["Authorization": h] }
        let asset: AVURLAsset
        if let local = downloads?.localURL(id) ?? fetched[id] {
            asset = AVURLAsset(url: local)
        } else if app.localOnly {
            loading = false
            isPlaying = false
            problem = "This song isn't downloaded, and the server can't be reached."
            return
        } else {
            asset = AVURLAsset(url: url, options: options)
        }
        let item = AVPlayerItem(asset: asset)
        if let old = endObserver { NotificationCenter.default.removeObserver(old) }
        endObserver = NotificationCenter.default.addObserver(forName: .AVPlayerItemDidPlayToEndTime, object: item, queue: .main) { [weak self] _ in
            Task { @MainActor in self?.finished() }
        }
        statusObserver = item.observe(\.status, options: [.new]) { [weak self] item, _ in
            let failed = item.status == .failed
            let ready = item.status == .readyToPlay
            let secs = item.duration.seconds
            var text = item.error?.localizedDescription
            if let e = item.error as NSError? {
                text = (text ?? "This song couldn't be played.") + " (\(e.domain) \(e.code))"
                if let log = item.errorLog()?.events.last, log.errorStatusCode != 0 { text = (text ?? "") + " The server answered \(log.errorStatusCode)." }
            }
            Task { @MainActor in self?.itemStatus(failed: failed, ready: ready, duration: secs, error: text) }
        }
        player.replaceCurrentItem(with: item)
        player.isMuted = silent
        attachEffects(item, id: id)
        if autoplay {
            player.play()
            isPlaying = true
        }
        // casting: the TV plays this song too (this phone is the silent clock)
        if cast?.isCasting == true && !suppressCast {
            cast?.load(trackID: id, at: pendingSeek ?? 0, autoplay: autoplay)
        }
        artworkFor = nil
        publishNowPlaying()
        report()
        loadArtwork()
    }

    private func itemStatus(failed: Bool, ready: Bool, duration secs: Double, error: String?) {
        if failed {
            // the player couldn't open the stream: fetch the song ourselves (the same request, with redirects kept) and play the file
            if let id = currentID, retriedFor != id, fetched[id] == nil, downloads?.localURL(id) == nil {
                recover(id, wasPlaying: isPlaying, original: error)
                return
            }
            loading = false
            problem = error ?? "This song couldn't be played."
            isPlaying = false
            publishNowPlaying()
        } else if ready {
            loading = false
            if secs.isFinite && secs > 0 { duration = secs }
            if let p = pendingSeek {
                pendingSeek = nil
                seekLocal(p)
            }
            publishNowPlaying()
        }
    }

    private func recover(_ id: String, wasPlaying: Bool, original: String?) {
        retriedFor = id
        guard let api = app?.api, let url = api.streamURL(id) else { return }
        loading = true
        Task {
            var req = URLRequest(url: url)
            req.timeoutInterval = 120
            if let h = api.authHeader { req.setValue(h, forHTTPHeaderField: "Authorization") }
            do {
                let (tmp, resp) = try await URLSession.shared.download(for: req)
                guard let http = resp as? HTTPURLResponse else { throw ServerError(message: "The server didn't answer.") }
                if http.statusCode == 401 {
                    loading = false; isPlaying = false
                    problem = "Your sign-in ran out. Sign out in Account and sign in again."
                    return
                }
                guard (200..<300).contains(http.statusCode) else {
                    loading = false; isPlaying = false
                    problem = "The server answered \(http.statusCode) for this song."
                    return
                }
                let t = (http.value(forHTTPHeaderField: "Content-Type") ?? "").lowercased()
                let ext = t.contains("flac") ? "flac" : (t.contains("mpeg") || t.contains("mp3")) ? "mp3" : (t.contains("mp4") || t.contains("m4a") || t.contains("aac")) ? "m4a" : t.contains("wav") ? "wav" : t.contains("ogg") ? "ogg" : "audio"
                let dest = FileManager.default.temporaryDirectory.appendingPathComponent("stream-" + id + "." + ext)
                try? FileManager.default.removeItem(at: dest)
                try FileManager.default.moveItem(at: tmp, to: dest)
                // keep only the latest few so the phone's storage isn't filled
                if fetched.count >= 3, let old = fetched.keys.first(where: { $0 != id }) {
                    if let u = fetched[old] { try? FileManager.default.removeItem(at: u) }
                    fetched[old] = nil
                }
                fetched[id] = dest
                if currentID == id { load(autoplay: wasPlaying) }
            } catch {
                loading = false; isPlaying = false
                problem = (original ?? "This song couldn't be played.") + " " + error.localizedDescription
            }
        }
    }

    /// 0 = off, minutes, or -1 = when this song ends.
    func setSleep(minutes: Int) {
        sleepAt = nil
        sleepAfterSong = false
        if minutes > 0 { sleepAt = Date().addingTimeInterval(Double(minutes) * 60) }
        if minutes < 0 { sleepAfterSong = true }
    }

    /// Move songs in the part of the queue that is still to come.
    func moveUpcoming(from offsets: IndexSet, to destination: Int) {
        if inRemote { hub?.notice = "Not available while you control another device"; return }
        if inParty {
            if let rel = offsets.first {
                let to = destination > rel ? destination - 1 : destination
                party?.fire("move", ["from": index + 1 + rel, "to": index + 1 + to])
            }
            return
        }
        var rest = upcoming
        rest.move(fromOffsets: offsets, toOffset: destination)
        queue = Array(queue[...max(index, 0)]) + rest
    }

    private func finished() {
        if inParty {
            // the shared queue moves on; a device that is making sound tells the party its song ended
            if party?.output == "own" { party?.fire("ended", ["index": index]) }
            return
        }
        if sleepAfterSong {
            sleepAfterSong = false
            pause()
            seekLocal(0)
            return
        }
        if repeatMode == .one {
            seekLocal(0)
            player.play()
            return
        }
        next()
    }

    private func tick(_ time: CMTime) {
        let s = time.seconds
        if s.isFinite { position = s }
        if let item = player.currentItem {
            let d = item.duration.seconds
            if d.isFinite && d > 0 { duration = d }
        }
        if let at = sleepAt, Date() >= at {
            sleepAt = nil
            quiet()
        }
        if isPlaying, let app = app, let id = currentID, let t = app.track(id) {
            recorder.tick(track: t, dt: 0.5, app: app)
        }
        maybeCrossfade()
        keepInStep()
        if isPlaying && Date().timeIntervalSince(lastSaved) > 5 { saveLast() }
        hub?.report()
        let playing = player.timeControlStatus == .playing
        if playing != isPlaying && player.timeControlStatus != .waitingToPlayAtSpecifiedRate {
            isPlaying = playing
            publishNowPlaying()
        }
        loading = player.timeControlStatus == .waitingToPlayAtSpecifiedRate && isPlaying
    }

    // MARK: audio session, lock screen, interruptions

    private func setUpAudioSession() {
        let s = AVAudioSession.sharedInstance()
        try? s.setCategory(.playback, mode: .default)
        try? s.setActive(true)
    }

    private func interruption(typeRaw: UInt, optionsRaw: UInt) {
        guard let type = AVAudioSession.InterruptionType(rawValue: typeRaw) else { return }
        if type == .began {
            isPlaying = false
            publishNowPlaying()
        } else {
            setUpAudioSession()
            if AVAudioSession.InterruptionOptions(rawValue: optionsRaw).contains(.shouldResume) { resume() }
        }
    }

    private func setUpRemoteCommands() {
        let c = MPRemoteCommandCenter.shared()
        c.playCommand.isEnabled = true
        c.pauseCommand.isEnabled = true
        c.togglePlayPauseCommand.isEnabled = true
        c.nextTrackCommand.isEnabled = true
        c.previousTrackCommand.isEnabled = true
        c.changePlaybackPositionCommand.isEnabled = true
        c.playCommand.addTarget { [weak self] _ in Task { @MainActor in self?.resume() }; return .success }
        c.pauseCommand.addTarget { [weak self] _ in Task { @MainActor in self?.pause() }; return .success }
        c.togglePlayPauseCommand.addTarget { [weak self] _ in Task { @MainActor in self?.toggle() }; return .success }
        c.nextTrackCommand.addTarget { [weak self] _ in Task { @MainActor in self?.next() }; return .success }
        c.previousTrackCommand.addTarget { [weak self] _ in Task { @MainActor in self?.previous() }; return .success }
        c.changePlaybackPositionCommand.addTarget { [weak self] event in
            if let e = event as? MPChangePlaybackPositionCommandEvent {
                let p = e.positionTime
                Task { @MainActor in self?.seek(to: p) }
            }
            return .success
        }
    }

    private func publishNowPlaying() {
        guard let id = currentID, let t = app?.track(id) else {
            MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
            return
        }
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: t.title,
            MPMediaItemPropertyArtist: t.artist,
            MPMediaItemPropertyPlaybackDuration: duration,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: position,
            MPNowPlayingInfoPropertyPlaybackRate: isPlaying ? 1.0 : 0.0,
        ]
        if let a = app?.album(t.albumId) { info[MPMediaItemPropertyAlbumTitle] = a.title }
        if let art = artwork, artworkFor == t.albumId { info[MPMediaItemPropertyArtwork] = art }
        MPNowPlayingInfoCenter.default().nowPlayingInfo = info
    }

    private func loadArtwork() {
        guard let app = app, let api = app.api, let id = currentID, let t = app.track(id), let a = app.album(t.albumId), a.cover,
              let url = api.coverURL(a.id, version: a.coverV) else { return }
        let auth = api.authHeader
        let albumID = a.id
        Task {
            guard let img = await ImageLoader.shared.image(url, auth: auth) else { return }
            let art = MPMediaItemArtwork(boundsSize: img.size) { _ in img }
            await MainActor.run {
                if self.currentID.flatMap({ self.app?.track($0)?.albumId }) == albumID {
                    self.artwork = art
                    self.artworkFor = albumID
                    self.publishNowPlaying()
                }
            }
        }
    }

    // MARK: resume where you left off

    private var lastKey: String { "volvix.last." + (app?.user?.id ?? "") }
    private var lastSaved = Date.distantPast

    private func saveLast() {
        guard !inParty, !queue.isEmpty, index >= 0 else { return }
        lastSaved = Date()
        let from = max(0, index - 20), to = min(queue.count, index + 200)
        UserDefaults.standard.set(["queue": Array(queue[from..<to]), "index": index - from, "pos": position] as [String: Any], forKey: lastKey)
    }

    /// Reopening the app shows the last song and spot, paused.
    private func resumeLast() {
        guard queue.isEmpty, app?.fx.resumeLast != false,
              let d = UserDefaults.standard.dictionary(forKey: lastKey),
              let q = d["queue"] as? [String], let i = d["index"] as? Int, i >= 0, i < q.count else { return }
        let ids = q.filter { app?.track($0) != nil }
        guard let at = ids.firstIndex(of: q[i]) else { return }
        let pos = (d["pos"] as? Double) ?? 0
        queue = ids
        index = at
        pendingSeek = pos > 2 ? pos : nil
        load(autoplay: false)
    }

    // MARK: controlling another device

    private var inRemote: Bool { remoteState != nil }

    /// Mirror the device that is playing: its queue, song and spot. No sound is made here.
    func applyRemote(_ r: RemoteNow) {
        let first = remoteState == nil
        if first {
            player.pause()
            player.replaceCurrentItem(with: nil)
            if let o = endObserver { NotificationCenter.default.removeObserver(o); endObserver = nil }
        }
        remoteState = r
        remoteName = r.name
        queue = r.queue
        index = r.index
        isPlaying = r.playing
        contextName = nil
        duration = Double(currentID.flatMap { app?.track($0)?.duration } ?? 0)
        updateRemotePosition()
        if remoteTimer == nil {
            remoteTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
                Task { @MainActor in self?.updateRemotePosition() }
            }
        }
        artworkFor = nil
        publishNowPlaying()
        loadArtwork()
    }

    private func updateRemotePosition() {
        guard let r = remoteState else { return }
        let serverNow = Date().timeIntervalSince1970 * 1000 + r.offsetMs
        var p = (r.playing ? r.posMs + (serverNow - r.updatedAt) : r.posMs) / 1000
        if duration > 0 { p = min(p, duration) }
        position = max(0, p)
    }

    /// The other device stopped (or this one took over): back to being a player of its own.
    func clearRemote() {
        remoteTimer?.invalidate()
        remoteTimer = nil
        remoteState = nil
        remoteName = nil
        isPlaying = false
        publishNowPlaying()
    }

    /// Another phone, the website or the Windows app asked this device (the one playing) to do something.
    func handleCommand(_ c: [String: Any]) {
        guard !inRemote, !inParty else { return }
        switch c["op"] as? String {
        case "play": if !isPlaying { resume() }
        case "pause": if isPlaying { pause() }
        case "next": next()
        case "prev": previous()
        case "seek": seek(to: decimal(c["pos"]))
        case "playAt": jump(to: wholeNumber(c["index"]))
        default: break
        }
        Task {
            try? await Task.sleep(nanoseconds: 300_000_000)
            hub?.report(force: true)
        }
    }

    // MARK: listening party

    private var inParty: Bool { party?.active == true }

    /// Pause only this phone (the sleep timer): in a party nobody else is stopped.
    private func quiet() {
        applyingParty = true
        pause()
        applyingParty = false
    }

    /// What a party starts from: the queue around now, the song and the spot.
    func partySnapshot() -> [String: Any] {
        let from = max(0, min(index - 50, queue.count))
        let to = min(queue.count, from + 500)
        let slice = Array(queue[from..<to])
        return ["queue": slice, "index": max(0, index - from), "posMs": Int(position * 1000), "playing": isPlaying]
    }

    private func partyPlay(_ ids: [String], _ startAt: Int, _ shuffled: Bool) {
        guard let p = party else { return }
        var list = ids
        var start = max(0, min(startAt, ids.count - 1))
        if p.isHost {
            // the host replaces the shared queue
            if shuffled || shuffle {
                let first = list.remove(at: start)
                list.shuffle()
                list.insert(first, at: 0)
                start = 0
            }
            p.fire("setQueue", ["ids": Array(list.prefix(500)), "start": start, "playing": true])
        } else {
            // guests add to the shared queue instead of replacing it for everyone
            let add = start > 0 ? [ids[start]] : ids
            p.fire("add", ["ids": shuffled ? add.shuffled() : add])
            p.notice = add.count == 1 ? "Added to the party queue" : "Added \(add.count) songs to the party queue"
        }
    }

    /// Follow the party: its queue, its song and where it is in it. With "host" output this phone stays silent.
    func applyParty(_ s: PartyState, output: String, offsetMs: Double) {
        applyingParty = true
        defer { applyingParty = false }
        partyOn = true
        let before = currentID
        queue = s.queue
        index = s.index
        setSilent(output == "host")
        guard s.index >= 0, s.index < s.queue.count else {
            player.pause()
            isPlaying = false
            publishNowPlaying()
            return
        }
        let serverNow = Date().timeIntervalSince1970 * 1000 + offsetMs
        let expected = (s.playing ? s.posMs + (serverNow - s.updatedAt) : s.posMs) / 1000
        if currentID != before || player.currentItem == nil {
            pendingSeek = expected > 0.5 ? expected : nil
            load(autoplay: s.playing)
            return
        }
        if abs(position - expected) > 1.5 { seekLocal(max(0, expected)) }
        if s.playing && !isPlaying {
            player.play()
            isPlaying = true
            publishNowPlaying()
        } else if !s.playing && isPlaying {
            player.pause()
            isPlaying = false
            publishNowPlaying()
        }
    }

    func releaseParty() {
        partyOn = false
        setSilent(cast?.isCasting == true)
    }

    /// If the shared position and ours have drifted apart (a stall, a slow start), catch up.
    private func keepInStep() {
        guard inParty, isPlaying, let s = party?.state, s.playing, Date().timeIntervalSince(lastDriftCheck) > 5 else { return }
        lastDriftCheck = Date()
        let offset = s.serverNow - Date().timeIntervalSince1970 * 1000
        applyParty(s, output: party?.output ?? "own", offsetMs: offset)
    }

    // MARK: sound: equalizer, loudness, crossfade

    private func observe(_ p: AVPlayer) {
        if let o = timeObserver, let old = observedPlayer { old.removeTimeObserver(o) }
        observedPlayer = p
        let interval = CMTime(seconds: 0.5, preferredTimescale: 600)
        timeObserver = p.addPeriodicTimeObserver(forInterval: interval, queue: .main) { [weak self] t in
            Task { @MainActor in self?.tick(t) }
        }
    }

    private func fxChanged(_ f: FxSettings) {
        let shared = EQShared.shared
        shared.configure(on: f.eqOn, gains: f.eqGains, pre: f.eqPre, sampleRate: shared.sampleRate)
        if let id = currentID {
            currentProc?.levelGain = Float(pow(10, AudioFX.levelDb(app?.track(id), mode: f.leveling) / 20))
            if f.processing, let item = player.currentItem, item.audioMix == nil { attachEffects(item, id: id) }
        }
    }

    /// Nothing is processed unless the equalizer or leveling is switched on.
    private func attachEffects(_ item: AVPlayerItem, id: String) {
        guard let app = app, app.fx.processing else { return }
        let level = AudioFX.levelDb(app.track(id), mode: app.fx.leveling)
        Task { [weak self] in
            let proc = await AudioFX.attach(to: item, levelDb: level)
            await MainActor.run {
                if let me = self, me.player.currentItem === item { me.currentProc = proc }
            }
        }
    }

    /// Near the end of a song, the next one comes in on a second player while this one fades out.
    private func maybeCrossfade() {
        guard let app = app, !xfading, isPlaying, app.fx.crossfade > 0, repeatMode != .one, cast?.isCasting != true, !inParty, !sleepAfterSong else { return }
        let fade = Double(app.fx.crossfade)
        let left = duration - position
        guard duration > fade + 4, left <= fade, left > 0.4 else { return }
        var next = index + 1
        if next >= queue.count {
            if repeatMode == .all { next = 0 } else { return }
        }
        if !app.fx.crossfadeAlbums, let a = currentID.flatMap({ app.track($0) }), let b = app.track(queue[next]), a.albumId == b.albumId, b.no == a.no + 1 { return }
        if app.localOnly, let d = downloads, !d.has(queue[next]) { return }
        xfading = true
        let outgoing = player
        fadingOut = outgoing
        let incoming = AVPlayer()
        incoming.volume = 0
        player = incoming
        observe(incoming)
        index = next
        load(autoplay: true)
        let steps = max(1, Int(fade * 20))
        Task { [weak self] in
            for s in 1...steps {
                try? await Task.sleep(nanoseconds: 50_000_000)
                let f = Float(s) / Float(steps)
                incoming.volume = f
                outgoing.volume = 1 - f
            }
            outgoing.pause()
            outgoing.replaceCurrentItem(with: nil)
            incoming.volume = 1
            self?.fadingOut = nil
            self?.xfading = false
        }
    }

    /// Tell the server (the household page shows what you're playing).
    private func report() {
        guard let api = app?.api else { return }
        // "Share what I'm listening to" and Private session decide what the household sees
        let share = app?.fx.shareActivity != false && app?.fx.privateSession != true
        let id = share ? currentID : nil
        let playing = share && isPlaying
        let key = "\(id ?? "-")|\(playing)"
        if key == lastReported { return }
        lastReported = key
        Task { await api.setNowPlaying(trackID: id, playing: playing) }
    }
}
