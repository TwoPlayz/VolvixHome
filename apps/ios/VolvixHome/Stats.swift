import Foundation

/// A whole number out of saved data, whether it arrived as Int, Double or text.
func wholeNumber(_ any: Any?) -> Int {
    if let i = any as? Int { return i }
    if let d = any as? Double { return Int(d) }
    if let s = any as? String, let i = Int(s) { return i }
    return 0
}

struct TrackStat: Hashable {
    var first: Int
    var n: Int
    var last: Int
}

struct YearStat: Hashable {
    var plays: [String: Int] = [:]
    var sec: Int = 0
}

/// Your listening numbers, kept in your own saved data (the same place the website keeps them, so they follow you everywhere).
struct Stats {
    var tracks: [String: TrackStat] = [:]
    var years: [Int: YearStat] = [:]
    /// [song, seconds] oldest first
    var log: [(String, Int)] = []

    init() {}

    init(_ raw: [String: Any]?) {
        guard let raw = raw else { return }
        for (id, v) in (raw["tracks"] as? [String: Any]) ?? [:] {
            guard let o = v as? [String: Any] else { continue }
            tracks[id] = TrackStat(first: wholeNumber(o["first"]), n: wholeNumber(o["n"]), last: wholeNumber(o["last"]))
        }
        for (y, v) in (raw["years"] as? [String: Any]) ?? [:] {
            guard let year = Int(y), let o = v as? [String: Any] else { continue }
            var ys = YearStat()
            for (id, n) in (o["plays"] as? [String: Any]) ?? [:] { ys.plays[id] = wholeNumber(n) }
            ys.sec = wholeNumber(o["sec"])
            years[year] = ys
        }
        for e in (raw["log"] as? [[Any]]) ?? [] {
            if e.count >= 2, let id = e[0] as? String { log.append((id, wholeNumber(e[1]))) }
        }
    }

    var sortedYears: [Int] { years.keys.sorted() }
}

/// The sound settings and a few more, as the website saves them in your data (`settings`).
struct FxSettings {
    var crossfade = 0
    var crossfadeAlbums = false
    /// "off", "track" or "album"
    var leveling = "off"
    var eqOn = false
    var eqPreset = "flat"
    var eqPre = 0.0
    var eqGains = [Double](repeating: 0, count: 10)
    var privateSession = false

    init() {}

    init(_ raw: [String: Any]?) {
        guard let s = raw else { return }
        crossfade = wholeNumber(s["crossfade"])
        crossfadeAlbums = (s["crossfadeAlbums"] as? Bool) ?? false
        leveling = (s["leveling"] as? String) ?? "off"
        if leveling.isEmpty { leveling = "off" }
        privateSession = (s["privateSession"] as? Bool) ?? false
        if let e = s["eq"] as? [String: Any] {
            eqOn = (e["on"] as? Bool) ?? false
            eqPreset = (e["preset"] as? String) ?? "flat"
            eqPre = (e["pre"] as? Double) ?? Double(wholeNumber(e["pre"]))
            if let g = e["gains"] as? [Any], g.count == 10 {
                eqGains = g.map { ($0 as? Double) ?? Double(wholeNumber($0)) }
            }
        }
    }

    /// Anything that changes the sound itself (the equalizer or loudness levelling).
    var processing: Bool { eqOn || leveling != "off" }
}

let eqBandFrequencies: [Double] = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]

struct EQPreset {
    let key: String
    let label: String
    let pre: Double
    let gains: [Double]
}

let eqPresets: [EQPreset] = [
    EQPreset(key: "flat", label: "Flat", pre: 0, gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    EQPreset(key: "bass", label: "Bass boost", pre: -3, gains: [6, 5, 4, 2, 0, 0, 0, 0, 0, 0]),
    EQPreset(key: "treble", label: "Treble boost", pre: -3, gains: [0, 0, 0, 0, 0, 1, 2, 4, 5, 6]),
    EQPreset(key: "vocal", label: "Vocal", pre: -2, gains: [-2, -2, -1, 1, 3, 4, 3, 1, 0, -1]),
    EQPreset(key: "rock", label: "Rock", pre: -3, gains: [5, 4, 2, -1, -2, -1, 2, 4, 5, 5]),
    EQPreset(key: "pop", label: "Pop", pre: -2, gains: [-1, 1, 3, 4, 3, 0, -1, -1, 1, 2]),
    EQPreset(key: "jazz", label: "Jazz", pre: -2, gains: [3, 2, 1, 2, -2, -2, 0, 1, 2, 3]),
    EQPreset(key: "classical", label: "Classical", pre: -2, gains: [4, 3, 2, 1, -1, -1, 0, 2, 3, 4]),
    EQPreset(key: "electronic", label: "Electronic", pre: -3, gains: [5, 4, 1, 0, -2, 2, 1, 1, 4, 5]),
    EQPreset(key: "loudness", label: "Loudness", pre: -4, gains: [6, 4, 0, 0, -1, 0, -1, 0, 3, 5]),
    EQPreset(key: "late", label: "Late night", pre: 0, gains: [-3, -2, 0, 1, 1, 1, 0, -1, -3, -4]),
]

/// Counts a play once a song has been heard for 30 seconds (or half of a short one), and adds up listening time.
@MainActor
final class PlayRecorder {
    private var heardID: String?
    private var heard = 0.0
    private var counted = false
    private var pendingSeconds = 0.0
    private var lastFlush = Date()

    func tick(track: Track, dt: Double, app: AppModel) {
        if app.fx.privateSession { return }
        if heardID != track.id {
            heardID = track.id
            heard = 0
            counted = false
        }
        heard += dt
        pendingSeconds += dt
        let need = min(30, Double(track.duration > 0 ? track.duration : 60) / 2)
        if !counted && heard >= need {
            counted = true
            app.recordPlay(track.id)
        } else if counted && track.duration > 0 && heard >= Double(track.duration) {
            heard = 0
            counted = false
        }
        if Date().timeIntervalSince(lastFlush) > 60 { flush(app) }
    }

    func flush(_ app: AppModel) {
        let add = Int(pendingSeconds.rounded())
        if add <= 0 { return }
        pendingSeconds = 0
        lastFlush = Date()
        app.addListening(seconds: add)
    }
}

extension AppModel {
    /// One more play of a song: shown straight away, then saved.
    func recordPlay(_ id: String) {
        let now = Int(Date().timeIntervalSince1970)
        let year = Calendar.current.component(.year, from: Date())
        var t = stats.tracks[id] ?? TrackStat(first: now, n: 0, last: now)
        t.n += 1
        t.last = now
        stats.tracks[id] = t
        stats.log.append((id, now))
        if stats.log.count > 3000 { stats.log.removeFirst(stats.log.count - 3000) }
        var y = stats.years[year] ?? YearStat()
        y.plays[id, default: 0] += 1
        stats.years[year] = y

        edit { d in
            var s = (d["stats"] as? [String: Any]) ?? [:]
            var tracks = (s["tracks"] as? [String: Any]) ?? [:]
            var tr = (tracks[id] as? [String: Any]) ?? ["first": now, "n": 0]
            tr["n"] = wholeNumber(tr["n"]) + 1
            tr["last"] = now
            tracks[id] = tr
            s["tracks"] = tracks
            var log = (s["log"] as? [[Any]]) ?? []
            log.append([id, now])
            if log.count > 3000 { log.removeFirst(log.count - 3000) }
            s["log"] = log
            var years = (s["years"] as? [String: Any]) ?? [:]
            var yo = (years["\(year)"] as? [String: Any]) ?? ["plays": [String: Any](), "sec": 0]
            var plays = (yo["plays"] as? [String: Any]) ?? [:]
            plays[id] = wholeNumber(plays[id]) + 1
            yo["plays"] = plays
            years["\(year)"] = yo
            s["years"] = years
            d["stats"] = s
        }
    }

    /// Time spent listening this year (for Rewind's minutes).
    func addListening(seconds: Int) {
        let year = Calendar.current.component(.year, from: Date())
        var y = stats.years[year] ?? YearStat()
        y.sec += seconds
        stats.years[year] = y
        edit { d in
            var s = (d["stats"] as? [String: Any]) ?? [:]
            var years = (s["years"] as? [String: Any]) ?? [:]
            var yo = (years["\(year)"] as? [String: Any]) ?? ["plays": [String: Any](), "sec": 0]
            yo["sec"] = wholeNumber(yo["sec"]) + seconds
            years["\(year)"] = yo
            s["years"] = years
            d["stats"] = s
        }
    }

    /// Change a sound or playback setting: it takes effect now, and is saved with your data.
    func updateSettings(_ change: @escaping (inout [String: Any]) -> Void) {
        var s = (userData["settings"] as? [String: Any]) ?? [:]
        change(&s)
        userData["settings"] = s
        fx = FxSettings(s)
        edit { d in
            var s = (d["settings"] as? [String: Any]) ?? [:]
            change(&s)
            d["settings"] = s
        }
    }

    func setEQ(on: Bool? = nil, preset: String? = nil, pre: Double? = nil, gains: [Double]? = nil) {
        updateSettings { s in
            var e = (s["eq"] as? [String: Any]) ?? ["on": false, "preset": "flat", "pre": 0, "gains": [Double](repeating: 0, count: 10)]
            if let v = on { e["on"] = v }
            if let v = preset { e["preset"] = v }
            if let v = pre { e["pre"] = v }
            if let v = gains { e["gains"] = v }
            s["eq"] = e
        }
    }

    /// While a dot is being dragged: the sound follows at once, nothing is saved until you let go.
    func previewEQ(gains: [Double]? = nil, pre: Double? = nil) {
        var f = fx
        if let g = gains { f.eqGains = g; f.eqPreset = "custom" }
        if let p = pre { f.eqPre = p }
        fx = f
    }

    func saveEQ() {
        let g = fx.eqGains
        let p = fx.eqPre
        let key = fx.eqPreset
        setEQ(preset: key, pre: p, gains: g)
    }
}
