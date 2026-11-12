import SwiftUI

/// The small player above the tab bar. Tap it for the full player.
struct MiniPlayer: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @Binding var showFull: Bool

    var body: some View {
        if let id = player.currentID, let t = app.track(id) {
            VStack(spacing: 0) {
                HStack(spacing: 12) {
                    CoverView(albumID: t.albumId, corner: 6).frame(width: 42, height: 42)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(t.title).font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                        Text(player.remoteName.map { "Playing on " + $0 } ?? t.artist).font(.system(size: 12)).foregroundColor(player.remoteName == nil ? Theme.text2 : Theme.accent).lineLimit(1)
                    }
                    Spacer(minLength: 8)
                    Button { app.toggleLike(t.id) } label: {
                        Image(systemName: app.isLiked(t.id) ? "heart.fill" : "heart")
                            .font(.system(size: 18)).foregroundColor(app.isLiked(t.id) ? Theme.accent : Theme.text2)
                            .frame(width: 36, height: 36)
                    }
                    Button { player.toggle() } label: {
                        Image(systemName: player.isPlaying ? "pause.fill" : "play.fill")
                            .font(.system(size: 20)).foregroundColor(Theme.text)
                            .frame(width: 40, height: 40)
                    }
                }
                .padding(8)
                MiniProgress(clock: player.clock)
                    .frame(height: 2)
                    .padding(.horizontal, 12)
                    .padding(.bottom, 4)
            }
            .glass(RoundedRectangle(cornerRadius: 24, style: .continuous))
            .padding(.horizontal, 8)
            .padding(.bottom, 6)
            .contentShape(Rectangle())
            .onTapGesture { showFull = true }
        }
    }
}

struct MiniProgress: View {
    @ObservedObject var clock: PlayClock

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule().fill(Theme.text.opacity(0.14))
                Capsule().fill(Theme.text).frame(width: geo.size.width * CGFloat(clock.duration > 0 ? min(1, clock.position / clock.duration) : 0))
            }
        }
    }
}

/// The song's progress: drag to move to another spot.
struct ScrubBar: View {
    let player: PlayerModel
    @ObservedObject var clock: PlayClock
    @State private var scrubbing = false
    @State private var scrubTo: Double = 0

    var body: some View {
        VStack(spacing: 4) {
            Slider(value: Binding(get: { scrubbing ? scrubTo : clock.position }, set: { scrubTo = $0 }),
                   in: 0...max(1, clock.duration),
                   onEditingChanged: { editing in
                       if editing { scrubbing = true; scrubTo = clock.position }
                       else { player.seek(to: scrubTo); scrubbing = false }
                   })
                .tint(Theme.text)
            HStack {
                Text(formatTime(scrubbing ? scrubTo : clock.position)).font(.system(size: 12)).foregroundColor(Theme.text2)
                Spacer()
                Text(formatTime(clock.duration)).font(.system(size: 12)).foregroundColor(Theme.text2)
            }
        }
    }
}

/// The full player, laid out like the website's phone player: where it is playing from, the cover, title and artist with
/// like / add / more, progress, controls, and a row of tools along the bottom.
struct NowPlayingView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @EnvironmentObject var devices: DeviceHub
    @EnvironmentObject var casting: CastHub
    @EnvironmentObject var party: PartyHub
    @Environment(\.dismiss) private var dismiss
    @State private var showParty = false
    @State private var showCredits = false
    @State private var addingTV = false
    @State private var tvAddress = ""
    @State private var showQueue = false
    @State private var showLyrics = false

    private func go(_ r: Route) {
        app.openRoute = r
        dismiss()
    }

    private func tool(_ icon: String, on: Bool = false) -> some View {
        Image(systemName: icon).font(.system(size: 20)).foregroundColor(on ? Theme.accent : Theme.text)
            .frame(width: 48, height: 44)
    }

    var body: some View {
        ZStack {
            Theme.surface.ignoresSafeArea()
            if let id = player.currentID, let t = app.track(id) {
                let album = app.album(t.albumId)
                VStack(spacing: 0) {
                    // where it is playing from
                    HStack {
                        Button { dismiss() } label: {
                            Image(systemName: "chevron.down").font(.system(size: 22, weight: .semibold)).foregroundColor(Theme.text).frame(width: 44, height: 44)
                        }
                        Spacer()
                        VStack(spacing: 1) {
                            Text(player.partyOn ? "Listening party" : (player.remoteName != nil ? "Controlling" : "Playing from")).font(.system(size: 11)).foregroundColor(Theme.text2)
                            Text(player.partyOn ? "Everyone hears the same queue" : (player.remoteName ?? player.contextName ?? album?.title ?? "")).font(.system(size: 14, weight: .bold)).foregroundColor(Theme.text).lineLimit(1)
                        }
                        Spacer()
                        Color.clear.frame(width: 44, height: 44)
                    }

                    // the cover (or the lyrics)
                    Group {
                        if showLyrics {
                            LyricsView(trackID: t.id, clock: player.clock)
                        } else {
                            CoverView(albumID: t.albumId, corner: 14)
                                .shadow(color: Color.black.opacity(0.35), radius: 18, y: 8)
                                .padding(.vertical, 14)
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)

                    // title, artist, like / add / more
                    HStack(alignment: .center, spacing: 4) {
                        VStack(alignment: .leading, spacing: 2) {
                            Button { go(.album(t.albumId)) } label: {
                                HStack(spacing: 6) {
                                    Text(t.title).font(.system(size: 22, weight: .bold)).foregroundColor(Theme.text).lineLimit(1)
                                    if t.explicit { Text("E").font(.system(size: 10, weight: .bold)).foregroundColor(Theme.surface).padding(.horizontal, 4).padding(.vertical, 1).background(Theme.text2).clipShape(RoundedRectangle(cornerRadius: 3)) }
                                }
                            }
                            .buttonStyle(.plain)
                            Button { go(.artist(t.artist)) } label: {
                                Text(t.artist).font(.system(size: 16)).foregroundColor(Theme.text2).lineLimit(1)
                            }
                            .buttonStyle(.plain)
                        }
                        Spacer(minLength: 4)
                        Button { app.toggleLike(t.id) } label: {
                            Image(systemName: app.isLiked(t.id) ? "heart.fill" : "heart")
                                .font(.system(size: 22)).foregroundColor(app.isLiked(t.id) ? Theme.accent : Theme.text)
                                .frame(width: 40, height: 44)
                        }
                        AddToPlaylistButton(ids: [t.id], plain: true)
                        Menu {
                            Button { go(.album(t.albumId)) } label: { Label("Go to album", systemImage: "square.stack") }
                            Button { go(.artist(t.artist)) } label: { Label("Go to artist", systemImage: "person") }
                            Button { showCredits = true } label: { Label("Credits", systemImage: "book") }
                            Menu {
                                Button("Off") { player.setSleep(minutes: 0) }
                                Button("15 minutes") { player.setSleep(minutes: 15) }
                                Button("30 minutes") { player.setSleep(minutes: 30) }
                                Button("1 hour") { player.setSleep(minutes: 60) }
                                Button("End of this song") { player.setSleep(minutes: -1) }
                            } label: { Label("Sleep timer", systemImage: "moon.zzz") }
                        } label: {
                            Image(systemName: "ellipsis").font(.system(size: 20)).foregroundColor(Theme.text).frame(width: 40, height: 44)
                        }
                    }

                    ScrubBar(player: player, clock: player.clock).padding(.top, 8)

                    if let p = player.problem { ProblemBox(text: p).padding(.top, 6) }

                    // controls
                    HStack(spacing: 0) {
                        Button { player.toggleShuffle() } label: {
                            Image(systemName: "shuffle").font(.system(size: 20)).foregroundColor(player.shuffle ? Theme.accent : Theme.text2).frame(maxWidth: .infinity, minHeight: 44)
                        }
                        Button { player.previous() } label: {
                            Image(systemName: "backward.fill").font(.system(size: 26)).foregroundColor(Theme.text).frame(maxWidth: .infinity, minHeight: 44)
                        }
                        Button { player.toggle() } label: {
                            Image(systemName: player.isPlaying ? "pause.fill" : "play.fill").font(.system(size: 28)).foregroundColor(Theme.text)
                                .frame(width: 72, height: 72)
                                .glass(Circle())
                                .frame(maxWidth: .infinity)
                        }
                        Button { player.next() } label: {
                            Image(systemName: "forward.fill").font(.system(size: 26)).foregroundColor(Theme.text).frame(maxWidth: .infinity, minHeight: 44)
                        }
                        Button { player.cycleRepeat() } label: {
                            Image(systemName: player.repeatMode == .one ? "repeat.1" : "repeat").font(.system(size: 20))
                                .foregroundColor(player.repeatMode == .off ? Theme.text2 : Theme.accent).frame(maxWidth: .infinity, minHeight: 44)
                        }
                    }
                    .padding(.top, 6)

                    // tools along the bottom: lyrics, output, party, queue
                    HStack(spacing: 0) {
                        Button { showLyrics.toggle() } label: { tool(showLyrics ? "quote.bubble.fill" : "quote.bubble", on: showLyrics) }.frame(maxWidth: .infinity)
                        Menu {
                            Section("Play on") {
                                Button { devices.takeOver() } label: { Label(devices.deviceName + " (this device)", systemImage: devices.playingHere ? "checkmark" : "iphone") }
                                ForEach(devices.others) { d in
                                    Button { devices.transfer(to: d.id) } label: {
                                        Label(d.name + (d.active ? " · playing" : ""), systemImage: d.active ? "checkmark" : "laptopcomputer.and.iphone")
                                    }
                                }
                                if devices.others.isEmpty { Text("No other devices open right now") }
                            }
                            Section("TVs and speakers") {
                                if let a = casting.active {
                                    Button { } label: { Label("Casting to \(a.name)", systemImage: "checkmark") }
                                    Button(role: .destructive) { casting.stop() } label: { Label("Stop casting", systemImage: "xmark") }
                                } else {
                                    ForEach(casting.devices) { d in
                                        Button { casting.start(d) } label: { Label(d.name + (d.model.isEmpty ? "" : " · " + d.model), systemImage: "tv") }
                                    }
                                    if casting.devices.isEmpty { Text("No TVs or speakers found yet") }
                                    Button { addingTV = true } label: { Label(casting.devices.isEmpty ? "Can't find it? Add by address…" : "Not listed? Add by address…", systemImage: "plus") }
                                }
                            }
                        } label: { tool("laptopcomputer.and.iphone", on: !devices.playingHere || casting.active != nil) }.frame(maxWidth: .infinity)
                        AirPlayButton().frame(width: 36, height: 36).frame(width: 48, height: 44).frame(maxWidth: .infinity)
                        Button { showParty = true } label: { tool(party.active ? "person.3.fill" : "person.3", on: party.active) }.frame(maxWidth: .infinity)
                        Button { showQueue = true } label: { tool("list.bullet") }.frame(maxWidth: .infinity)
                    }
                    .padding(.top, 4)
                }
                .padding(.horizontal, 24)
                .padding(.bottom, 8)
            } else {
                VStack(spacing: 12) {
                    Text("Nothing playing").foregroundColor(Theme.text2)
                    PillButton(title: "Close", primary: false) { dismiss() }
                }
            }
        }
        .sheet(isPresented: $showQueue) { QueueView().environmentObject(app).environmentObject(player) }
        .sheet(isPresented: $showParty) { PartySheet().environmentObject(app).environmentObject(player).environmentObject(party) }
        .sheet(isPresented: $showCredits) {
            if let id = player.currentID { CreditsSheet(trackID: id).environmentObject(app).environmentObject(player) }
        }
        .alert("Add a TV or speaker", isPresented: $addingTV) {
            TextField("192.168.1.40", text: $tvAddress)
            Button("Add") { let a = tvAddress; tvAddress = ""; Task { await casting.add(host: a) } }
            Button("Cancel", role: .cancel) { tvAddress = "" }
        } message: {
            Text("Its address on your network. The Google Home app shows it under the device's settings.")
        }
        .environmentObject(app)
        .overlay(alignment: .top) {
            if let n = devices.notice ?? casting.notice ?? party.notice {
                Text(n).font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text)
                    .padding(.horizontal, 14).padding(.vertical, 8).glass(Capsule())
                    .padding(.top, 8)
                    .task { try? await Task.sleep(nanoseconds: 2_500_000_000); devices.notice = nil; casting.notice = nil; party.notice = nil }
            }
        }
    }
}
struct QueueView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                if let id = player.currentID, let t = app.track(id) {
                    Section("Now playing") {
                        row(t, index: nil)
                    }
                }
                Section("Next up") {
                    ForEach(Array(player.upcoming.enumerated()), id: \.offset) { i, id in
                        if let t = app.track(id) { row(t, index: player.index + 1 + i) }
                    }
                    .onDelete { offsets in
                        for o in offsets.sorted(by: >) { player.removeFromQueue(at: player.index + 1 + o) }
                    }
                    .onMove { offsets, destination in player.moveUpcoming(from: offsets, to: destination) }
                }
            }
            .listStyle(.insetGrouped)
            .environment(\.editMode, .constant(.active))
            .scrollContentBackground(.hidden)
            .background(Theme.surface)
            .navigationTitle(player.partyOn ? "Party queue" : "Queue")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }

    private func row(_ t: Track, index: Int?) -> some View {
        HStack(spacing: 12) {
            CoverView(albumID: t.albumId, corner: 5).frame(width: 40, height: 40)
            VStack(alignment: .leading, spacing: 2) {
                Text(t.title).font(.system(size: 15)).foregroundColor(Theme.text).lineLimit(1)
                Text(t.artist).font(.system(size: 12)).foregroundColor(Theme.text2).lineLimit(1)
            }
            Spacer()
        }
        .contentShape(Rectangle())
        .onTapGesture { if let i = index { player.jump(to: i) } }
        .listRowBackground(Theme.surface2)
    }
}
