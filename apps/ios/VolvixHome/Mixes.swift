import SwiftUI

/// "Made for you": mixes built from what you play and the tags of what is on the server. Nothing leaves the server.
/// They are worked out when you open Home and stay the same for the day.
struct Mix: Identifiable, Hashable {
    let id: String
    let title: String
    let sub: String
    let ids: [String]
    let icon: String
}

struct SeededRandom {
    var a: UInt32
    mutating func next() -> Double {
        a = a &+ 0x6D2B79F5
        var t = (a ^ (a >> 15)) &* (1 | a)
        t = (t &+ ((t ^ (t >> 7)) &* (61 | t))) ^ t
        return Double(t ^ (t >> 14)) / 4294967296
    }
}

func fnvHash(_ s: String) -> UInt32 {
    var h: UInt32 = 2166136261
    for u in s.utf8 { h ^= UInt32(u); h = h &* 16777619 }
    return h
}

@MainActor
enum MixEngine {
    private static let day = 86400

    private static func today() -> Int { Int(Date().timeIntervalSince1970 / 86400) }

    static func mixes(_ app: AppModel, playable: (String) -> Bool) -> [Mix] {
        let now = Int(Date().timeIntervalSince1970)
        let seed = (app.user?.id ?? "me")
        var rand = SeededRandom(a: fnvHash(seed) &+ UInt32(truncatingIfNeeded: today()))

        func decade(_ a: Album?) -> Int { (a?.year).map { ($0 / 10) * 10 } ?? 0 }
        func info(_ id: String) -> (Track, Album?)? {
            guard let t = app.track(id) else { return nil }
            return (t, app.album(t.albumId))
        }

        // what you like: weights per artist, genre and decade, recent plays counting more (30-day half-life)
        var artist: [String: Double] = [:], genre: [String: Double] = [:], dec: [Int: Double] = [:]
        var plays: [String: Int] = [:], last: [String: Int] = [:]
        var total = 0
        for (id, at) in app.stats.log {
            guard let (t, a) = info(id) else { continue }
            let weight = pow(0.5, Double(now - at) / Double(30 * day))
            total += 1
            plays[id, default: 0] += 1
            last[id] = max(last[id] ?? 0, at)
            artist[t.artist.lowercased(), default: 0] += weight
            if let g = a?.genre, !g.isEmpty { genre[g, default: 0] += weight }
            if let y = a?.year, y > 0 { dec[decade(a), default: 0] += weight }
        }
        // plays from before the log existed still count
        for (id, s) in app.stats.tracks where plays[id] == nil {
            plays[id] = s.n
            last[id] = s.last
        }

        func top<K: Hashable>(_ d: [K: Double], _ n: Int) -> [K] {
            Array(d.sorted { $0.value > $1.value }.prefix(n).map { $0.key })
        }
        func topInt(_ d: [String: Int], _ n: Int) -> [String] {
            Array(d.sorted { $0.value > $1.value }.prefix(n).map { $0.key })
        }
        func norm<K: Hashable>(_ d: [K: Double]) -> (K) -> Double {
            let m = max(d.values.max() ?? 0, 0.0001)
            return { (d[$0] ?? 0) / m }
        }
        let affArtist = norm(artist), affGenre = norm(genre), affDecade = norm(dec)
        func score(_ id: String) -> Double {
            guard let (t, a) = info(id) else { return 0 }
            return 3 * affArtist(t.artist.lowercased()) + 2 * ((a?.genre).map { affGenre($0) } ?? 0) + (a == nil ? 0 : affDecade(decade(a)))
        }
        func shuffled(_ list: [String]) -> [String] {
            var a = list
            if a.count < 2 { return a }
            for i in stride(from: a.count - 1, to: 0, by: -1) {
                let j = Int(rand.next() * Double(i + 1))
                a.swapAt(i, j)
            }
            return a
        }
        /// no more than `per` songs from one artist, so a mix isn't one band
        func pick(_ ids: [String], _ n: Int, _ per: Int) -> [String] {
            var seen: [String: Int] = [:], out: [String] = []
            for id in ids {
                if out.count >= n { break }
                guard let (t, _) = info(id) else { continue }
                let k = t.artist.lowercased()
                if (seen[k] ?? 0) >= per { continue }
                seen[k, default: 0] += 1
                out.append(id)
            }
            return out
        }
        func make(_ id: String, _ title: String, _ sub: String, _ ids: [String], _ icon: String) -> Mix? {
            ids.count >= 3 ? Mix(id: id, title: title, sub: sub, ids: ids, icon: icon) : nil
        }

        let all = app.allTracks.map { $0.id }.filter(playable)
        var list: [Mix?] = []

        if total >= 3 {
            var recent: [String: Int] = [:]
            for (id, at) in app.stats.log where now - at < 30 * day && app.track(id) != nil { recent[id, default: 0] += 1 }
            list.append(make("repeat", "On repeat", "What you've played most in the last 30 days",
                             topInt(recent, 40).filter(playable).filter { (recent[$0] ?? 0) >= 2 }, "repeat"))
            var everything: [String: Int] = [:]
            for (id, n) in plays where app.track(id) != nil { everything[id] = n }
            list.append(make("top", "Your top songs", "All-time favourites on this server", topInt(everything, 50).filter(playable), "heart"))
            let forgotten = plays.keys.filter { (plays[$0] ?? 0) >= 3 && now - (last[$0] ?? 0) > 60 * day && app.track($0) != nil && playable($0) }.sorted()
            list.append(make("rediscover", "Rediscover", "Songs you used to play a lot and haven't for a while", Array(shuffled(forgotten).prefix(40)), "history"))

            for ar in top(artist, 3) {
                let own = all.filter { info($0)?.0.artist.lowercased() == ar }
                var genres = Set<String>()
                for id in own { if let g = info(id)?.1?.genre, !g.isEmpty { genres.insert(g) } }
                let like = all.filter { id in
                    guard let (t, a) = info(id) else { return false }
                    return t.artist.lowercased() != ar && (a?.genre).map { genres.contains($0) } == true
                }
                let name = own.first.flatMap { info($0)?.0.artist } ?? ar
                let ids = pick(Array(shuffled(own).prefix(14)) + shuffled(like), 30, 4)
                list.append(make("more-" + String(fnvHash(ar), radix: 36), name + " & more", "Mixed with similar music from your library", ids, "user"))
            }

            let addedCut = now - 90 * day
            let fresh = all.filter { id in
                guard let a = info(id)?.1 else { return false }
                return a.added > addedCut && plays[id] == nil
            }
            list.append(make("new", "New for you", "Recently added music, in the genres you play", pick(fresh.sorted { score($0) > score($1) }, 30, 4), "plus"))

            let unplayed = all.filter { plays[$0] == nil }
            // a little chance in the ranking so it isn't the same top 40 every day
            var jitter: [String: Double] = [:]
            for id in unplayed { jitter[id] = score(id) + rand.next() * 0.6 }
            list.append(make("discover", "Discover", "Songs you haven't played yet that fit your taste", pick(unplayed.sorted { (jitter[$0] ?? 0) > (jitter[$1] ?? 0) }, 40, 3), "search"))
        } else {
            list.append(make("start", "Start here", "A mix of what's on the server. Play a few songs and this gets personal.", pick(shuffled(all), 30, 3), "music"))
        }
        return list.compactMap { $0 }
    }

    /// Albums by how much you've played them.
    static func topAlbums(_ app: AppModel, _ n: Int) -> [Album] {
        var plays: [String: Int] = [:]
        for (id, s) in app.stats.tracks { if let t = app.track(id) { plays[t.albumId, default: 0] += s.n } }
        return plays.sorted { $0.value > $1.value }.prefix(n).compactMap { app.album($0.key) }
    }

    /// Albums picked at random for today (the same picks all day).
    static func randomAlbums(_ app: AppModel, _ n: Int) -> [Album] {
        var r = SeededRandom(a: fnvHash("r" + (app.user?.id ?? "me")) &+ UInt32(truncatingIfNeeded: today()))
        var a = app.albums
        if a.count < 2 { return a }
        for i in stride(from: a.count - 1, to: 0, by: -1) {
            let j = Int(r.next() * Double(i + 1))
            a.swapAt(i, j)
        }
        return Array(a.prefix(n))
    }
}

/// A card for a mix, as on the website and the Windows app: red fading into the mix's own colour, its icon and its name in the
/// middle.
struct MixCard: View {
    @EnvironmentObject var app: AppModel
    let mix: Mix
    var width: CGFloat = 150

    var body: some View {
        NavigationLink(value: Route.mix(mix.id)) {
            VStack(alignment: .leading, spacing: 6) {
                MixTile(mix: mix, icon: 30, font: 16)
                    .frame(width: width, height: width)
                    .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
                Text(mix.title).font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                Text(mix.sub).font(.system(size: 12)).foregroundColor(Theme.text2).lineLimit(2).multilineTextAlignment(.leading)
            }
            .frame(width: width, alignment: .leading)
        }
        .buttonStyle(.plain)
    }
}

/// The colours the website gives each mix (picked by its id).
func mixTint(_ seed: String) -> Color {
    let tints: [UInt32] = [0x3A2A4F, 0x1F3B4D, 0x4A2C2C, 0x244034, 0x453A22, 0x2B2F4A]
    var h: UInt32 = 5381
    for b in seed.utf8 { h = (h &* 33) ^ UInt32(b) }
    return Color(hex: tints[Int(h % UInt32(tints.count))])
}

struct MixTile: View {
    let mix: Mix
    var icon: CGFloat = 30
    var font: CGFloat = 16

    var body: some View {
        ZStack {
            LinearGradient(colors: [Theme.accent, mixTint(mix.id)], startPoint: .bottomLeading, endPoint: .topTrailing)
            VStack(spacing: 10) {
                SVGIcon(name: mix.icon, size: icon, color: .white, lineWidth: 1.8)
                Text(mix.title).font(.system(size: font, weight: .semibold)).foregroundColor(.white).multilineTextAlignment(.center).lineLimit(3)
            }
            .padding(12)
        }
    }
}

/// Up to four covers in a square: 4 = a 2x2 grid, 3 = one big and two small, 2 = side by side, 1 = just that cover.
struct Collage: View {
    @EnvironmentObject var app: AppModel
    let ids: [String]

    private var albums: [String] {
        var out: [String] = []
        for id in ids {
            if let t = app.track(id), !out.contains(t.albumId) { out.append(t.albumId) }
            if out.count == 4 { break }
        }
        return out
    }

    private func tile(_ id: String, _ w: CGFloat, _ h: CGFloat) -> some View {
        CoverView(albumID: id, corner: 0).frame(width: max(w, h), height: max(w, h)).frame(width: w, height: h).clipped()
    }

    var body: some View {
        let a = albums
        GeometryReader { geo in
            let s = min(geo.size.width, geo.size.height)
            let half = s / 2
            Group {
                switch a.count {
                case 0:
                    ZStack { Theme.tint("mix"); Image(systemName: "music").font(.system(size: s * 0.3)).foregroundColor(.white.opacity(0.7)) }
                case 1:
                    tile(a[0], s, s)
                case 2:
                    HStack(spacing: 0) { tile(a[0], half, s); tile(a[1], half, s) }
                case 3:
                    HStack(spacing: 0) {
                        tile(a[0], half, s)
                        VStack(spacing: 0) { tile(a[1], half, half); tile(a[2], half, half) }
                    }
                default:
                    VStack(spacing: 0) {
                        HStack(spacing: 0) { tile(a[0], half, half); tile(a[1], half, half) }
                        HStack(spacing: 0) { tile(a[2], half, half); tile(a[3], half, half) }
                    }
                }
            }
            .frame(width: s, height: s)
        }
        .aspectRatio(1, contentMode: .fit)
    }
}

struct MixView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @EnvironmentObject var downloads: DownloadStore
    @Environment(\.dismiss) private var dismiss
    let id: String
    @State private var saved = false
    @State private var mix: Mix?
    @State private var looked = false

    var body: some View {
        ScrollView {
            if let m = mix {
                let tracks = m.ids.compactMap { app.track($0) }
                VStack(spacing: 14) {
                    ZStack {
                        LinearGradient(colors: [Theme.accent, mixTint(m.id)], startPoint: .bottomLeading, endPoint: .topTrailing)
                        SVGIcon(name: m.icon, size: 64, color: .white, lineWidth: 1.8)
                    }
                    .frame(width: 220, height: 220)
                    .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
                    .padding(.top, 8)
                    VStack(spacing: 4) {
                        Text("Made for you").font(.system(size: 12, weight: .semibold)).foregroundColor(Theme.text2)
                        Text(m.title).font(.system(size: 24, weight: .bold)).foregroundColor(Theme.text).multilineTextAlignment(.center)
                        Text(m.sub + " · \(m.ids.count) songs").font(.system(size: 13)).foregroundColor(Theme.text2).multilineTextAlignment(.center)
                    }
                    .padding(.horizontal, 16)
                    HStack(spacing: 10) {
                        PillButton(title: "Play", icon: "play.fill") { player.play(m.ids, from: m.title) }
                        PillButton(title: "Shuffle", icon: "shuffle", primary: false) { player.play(m.ids, shuffled: true, from: m.title) }
                        PillButton(title: saved ? "Saved" : "Save", icon: saved ? "checkmark" : "plus", primary: false) {
                            if saved { return }
                            saved = true
                            let f = DateFormatter()
                            f.dateFormat = "MMM d"
                            app.createPlaylist(m.title + " · " + f.string(from: Date()), adding: nil, tracks: m.ids, description: m.sub)
                        }
                    }
                    LazyVStack(spacing: 0) {
                        ForEach(Array(tracks.enumerated()), id: \.offset) { i, t in
                            TrackRow(track: t, showCover: true) { player.play(m.ids, startAt: i, from: m.title) }
                        }
                    }
                    .padding(.top, 6)
                }
                .padding(.bottom, 24)
            } else if !looked {
                ProgressView().tint(Theme.text2).padding(40)
            } else {
                VStack(spacing: 10) {
                    Text("That mix isn't available").font(.system(size: 18, weight: .semibold)).foregroundColor(Theme.text)
                    Text("Mixes change as you listen. Open Made for you on Home.").font(.system(size: 14)).foregroundColor(Theme.text2)
                }
                .padding(40)
            }
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
        .task(id: id) {
            mix = MixEngine.mixes(app, playable: { app.localOnly ? downloads.has($0) : true }).first { $0.id == id }
            looked = true
        }
    }
}
