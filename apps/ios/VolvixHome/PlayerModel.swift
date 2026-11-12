import Foundation
import AVFoundation
import MediaPlayer
import UIKit

/// The music player: a queue, play/pause/seek, and the lock screen and headphone buttons.
/// Plays through the system's own audio engine, so it keeps going in the background.
@MainActor
final class PlayerModel: ObservableObject {
    enum Repeat { case off, all, one }

    @Published private(set) var queue: [String] = []     // track ids
    @Published private(set) var index: Int = -1
    @Published private(set) var isPlaying = false
    @Published private(set) var position: Double = 0
    @Published private(set) var duration: Double = 0
    @Published private(set) var loading = false
    @Published var shuffle = false
    @Published var repeatMode: Repeat = .off
    @Published var problem: String?
    /// when the sleep timer pauses the music (nil = off); and "stop when this song ends"
    @Published private(set) var sleepAt: Date?
    @Published private(set) var sleepAfterSong = false

    private let player = AVPlayer()
    private var timeObserver: Any?
    private var endObserver: NSObjectProtocol?
    private var statusObserver: NSKeyValueObservation?
    private weak var app: AppModel?
    private weak var downloads: DownloadStore?
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
        let interval = CMTime(seconds: 0.5, preferredTimescale: 600)
        timeObserver = player.addPeriodicTimeObserver(forInterval: interval, queue: .main) { [weak self] t in
            Task { @MainActor in self?.tick(t) }
        }
        NotificationCenter.default.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] n in
            let raw = (n.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt) ?? 0
            let opts = (n.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt) ?? 0
            Task { @MainActor in self?.interruption(typeRaw: raw, optionsRaw: opts) }
        }
    }

    // MARK: starting music

    /// Play these songs, starting at one of them.
    func play(_ ids: [String], startAt: Int = 0, shuffled: Bool = false) {
        guard !ids.isEmpty else { return }
        var list = ids
        var start = max(0, min(startAt, ids.count - 1))
        // with no server, only what is downloaded can play
        if app?.offline == true, let d = downloads {
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
        load(autoplay: true)
        if let id = currentID, let t = app?.track(id) { app?.playedAlbum(t.albumId) }
    }

    func playNext(_ id: String) {
        if queue.isEmpty { play([id]); return }
        queue.insert(id, at: min(index + 1, queue.count))
    }

    func addToQueue(_ id: String) {
        if queue.isEmpty { play([id]); return }
        queue.append(id)
    }

    func jump(to i: Int) {
        guard i >= 0, i < queue.count else { return }
        index = i
        load(autoplay: true)
    }

    func removeFromQueue(at i: Int) {
        guard i > index, i < queue.count else { return }
        queue.remove(at: i)
    }

    // MARK: controls

    func toggle() {
        guard hasCurrent else { return }
        if isPlaying { pause() } else { resume() }
    }

    func resume() {
        guard hasCurrent else { return }
        player.play()
        isPlaying = true
        publishNowPlaying()
        report()
    }

    func pause() {
        player.pause()
        isPlaying = false
        publishNowPlaying()
        report()
    }

    func next() {
        if queue.isEmpty { return }
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
        // a few seconds in: back to the start of the song; at the start: the one before
        if position > 3 || index <= 0 {
            seek(to: 0)
        } else {
            index -= 1
            load(autoplay: true)
        }
    }

    func seek(to seconds: Double) {
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
        problem = nil
        loading = true
        position = 0
        let t = app.track(id)
        duration = Double(t?.duration ?? 0)
        var options: [String: Any] = [:]
        if let h = api.authHeader { options["AVURLAssetHTTPHeaderFieldsKey"] = ["Authorization": h] }
        let asset: AVURLAsset
        if let local = downloads?.localURL(id) {
            asset = AVURLAsset(url: local)
        } else if app.offline {
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
            let text = item.error?.localizedDescription
            Task { @MainActor in self?.itemStatus(failed: failed, ready: ready, duration: secs, error: text) }
        }
        player.replaceCurrentItem(with: item)
        if autoplay {
            player.play()
            isPlaying = true
        }
        artworkFor = nil
        publishNowPlaying()
        report()
        loadArtwork()
    }

    private func itemStatus(failed: Bool, ready: Bool, duration secs: Double, error: String?) {
        if failed {
            loading = false
            problem = error ?? "This song couldn't be played."
            isPlaying = false
            publishNowPlaying()
        } else if ready {
            loading = false
            if secs.isFinite && secs > 0 { duration = secs }
            publishNowPlaying()
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
        var rest = upcoming
        rest.move(fromOffsets: offsets, toOffset: destination)
        queue = Array(queue[...max(index, 0)]) + rest
    }

    private func finished() {
        if sleepAfterSong {
            sleepAfterSong = false
            pause()
            seek(to: 0)
            return
        }
        if repeatMode == .one {
            seek(to: 0)
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
            pause()
        }
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

    /// Tell the server (the household page shows what you're playing).
    private func report() {
        guard let api = app?.api else { return }
        let id = currentID
        let playing = isPlaying
        let key = "\(id ?? "-")|\(playing)"
        if key == lastReported { return }
        lastReported = key
        Task { await api.setNowPlaying(trackID: id, playing: playing) }
    }
}
