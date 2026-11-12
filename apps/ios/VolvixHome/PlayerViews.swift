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
                        Text(t.artist).font(.system(size: 12)).foregroundColor(Theme.text2).lineLimit(1)
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
            .background(Theme.surface3)
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
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

/// The full player: big cover, progress, controls, and the queue.
struct NowPlayingView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @EnvironmentObject var devices: DeviceHub
    @EnvironmentObject var casting: CastHub
    @EnvironmentObject var party: PartyHub
    @Environment(\.dismiss) private var dismiss
    @State private var showParty = false
    @State private var showCredits = false
    @State private var showTrackMenu = false
    @State private var addingTV = false
    @State private var tvAddress = ""
    @State private var scrubbing = false
    @State private var scrubTo: Double = 0
    @State private var showQueue = false
    @State private var showLyrics = false

    var body: some View {
        ZStack {
            Theme.surface.ignoresSafeArea()
            if let id = player.currentID, let t = app.track(id) {
                VStack(spacing: 18) {
                    HStack {
                        Button { dismiss() } label: {
                            Image(systemName: "chevron.down").font(.system(size: 20, weight: .semibold)).foregroundColor(Theme.text).frame(width: 44, height: 44)
                        }
                        Spacer()
                        Text(player.partyOn ? "Listening party" : (player.contextName.map { "Playing from " + $0 } ?? "Now Playing")).font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text2).lineLimit(1)
                        Spacer()
                        HStack(spacing: 0) {
                            Button { showLyrics.toggle() } label: {
                                Image(systemName: showLyrics ? "quote.bubble.fill" : "quote.bubble").font(.system(size: 18))
                                    .foregroundColor(showLyrics ? Theme.accent : Theme.text).frame(width: 44, height: 44)
                            }
                            Menu {
                                Button("Off") { player.setSleep(minutes: 0) }
                                Button("15 minutes") { player.setSleep(minutes: 15) }
                                Button("30 minutes") { player.setSleep(minutes: 30) }
                                Button("1 hour") { player.setSleep(minutes: 60) }
                                Button("End of this song") { player.setSleep(minutes: -1) }
                            } label: {
                                Image(systemName: (player.sleepAt != nil || player.sleepAfterSong) ? "moon.zzz.fill" : "moon.zzz").font(.system(size: 18))
                                    .foregroundColor((player.sleepAt != nil || player.sleepAfterSong) ? Theme.accent : Theme.text).frame(width: 44, height: 44)
                            }
                            Menu {
                                Section("Play on") {
                                    Button { } label: { Label(devices.deviceName + " (this device)", systemImage: devices.playingHere ? "checkmark" : "iphone") }
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
                            } label: {
                                Image(systemName: "laptopcomputer.and.iphone").font(.system(size: 18)).foregroundColor(Theme.text).frame(width: 44, height: 44)
                            }
                            AirPlayButton().frame(width: 36, height: 36).frame(width: 44, height: 44)
                            Button { showParty = true } label: {
                                Image(systemName: party.active ? "person.3.fill" : "person.3").font(.system(size: 17))
                                    .foregroundColor(party.active ? Theme.accent : Theme.text).frame(width: 44, height: 44)
                            }
                            Button { showQueue = true } label: {
                                Image(systemName: "list.bullet").font(.system(size: 18)).foregroundColor(Theme.text).frame(width: 44, height: 44)
                            }
                        }
                    }
                    Spacer(minLength: 0)
                    if showLyrics {
                        LyricsView(trackID: t.id, clock: player.clock)
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                    } else {
                        CoverView(albumID: t.albumId, corner: 12)
                            .frame(maxWidth: 340, maxHeight: 340)
                            .padding(.horizontal, 28)
                            .shadow(color: Color.black.opacity(0.4), radius: 20, y: 10)
                    }
                    Spacer(minLength: 0)
                    HStack(alignment: .center) {
                        VStack(alignment: .leading, spacing: 3) {
                            Text(t.title).font(.system(size: 22, weight: .bold)).foregroundColor(Theme.text).lineLimit(1)
                            Text(t.artist).font(.system(size: 16)).foregroundColor(Theme.text2).lineLimit(1)
                        }
                        .contentShape(Rectangle())
                        .onTapGesture { showTrackMenu = true }
                        Spacer()
                        AddToPlaylistButton(ids: [t.id])
                        Button { app.toggleLike(t.id) } label: {
                            Image(systemName: app.isLiked(t.id) ? "heart.fill" : "heart")
                                .font(.system(size: 22)).foregroundColor(app.isLiked(t.id) ? Theme.accent : Theme.text)
                                .frame(width: 44, height: 44)
                        }
                    }
                    .padding(.horizontal, 24)

                    ScrubBar(player: player, clock: player.clock)
                        .padding(.horizontal, 24)

                    if let p = player.problem { ProblemBox(text: p).padding(.horizontal, 24) }

                    HStack(spacing: 0) {
                        Button { player.toggleShuffle() } label: {
                            Image(systemName: "shuffle").font(.system(size: 20)).foregroundColor(player.shuffle ? Theme.accent : Theme.text2).frame(maxWidth: .infinity)
                        }
                        Button { player.previous() } label: {
                            Image(systemName: "backward.fill").font(.system(size: 26)).foregroundColor(Theme.text).frame(maxWidth: .infinity)
                        }
                        Button { player.toggle() } label: {
                            ZStack {
                                Circle().fill(Theme.text).frame(width: 68, height: 68)
                                Image(systemName: player.isPlaying ? "pause.fill" : "play.fill").font(.system(size: 28)).foregroundColor(Theme.surface)
                            }
                            .frame(maxWidth: .infinity)
                        }
                        Button { player.next() } label: {
                            Image(systemName: "forward.fill").font(.system(size: 26)).foregroundColor(Theme.text).frame(maxWidth: .infinity)
                        }
                        Button { player.cycleRepeat() } label: {
                            Image(systemName: player.repeatMode == .one ? "repeat.1" : "repeat").font(.system(size: 20))
                                .foregroundColor(player.repeatMode == .off ? Theme.text2 : Theme.accent).frame(maxWidth: .infinity)
                        }
                    }
                    .padding(.horizontal, 12)
                    .padding(.bottom, 20)
                }
            } else {
                VStack(spacing: 12) {
                    Text("Nothing playing").foregroundColor(Theme.text2)
                    PillButton(title: "Close", primary: false) { dismiss() }
                }
            }
        }
        .sheet(isPresented: $showQueue) { QueueView().environmentObject(app).environmentObject(player) }
        .confirmationDialog("", isPresented: $showTrackMenu, titleVisibility: .hidden) {
            if let id = player.currentID, let t = app.track(id) {
                Button("Go to album") { app.openRoute = .album(t.albumId); dismiss() }
                Button("Go to artist") { app.openRoute = .artist(t.artist); dismiss() }
                Button("Credits") { showCredits = true }
            }
            Button("Cancel", role: .cancel) {}
        }
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
                Text(n).font(.system(size: 13, weight: .semibold)).foregroundColor(.white)
                    .padding(.horizontal, 14).padding(.vertical, 8).background(Theme.surface3).clipShape(Capsule())
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
