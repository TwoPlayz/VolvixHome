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
                GeometryReader { geo in
                    ZStack(alignment: .leading) {
                        Capsule().fill(Color.white.opacity(0.14))
                        Capsule().fill(Theme.text).frame(width: geo.size.width * CGFloat(player.duration > 0 ? min(1, player.position / player.duration) : 0))
                    }
                }
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

/// The full player: big cover, progress, controls, and the queue.
struct NowPlayingView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @Environment(\.dismiss) private var dismiss
    @State private var scrubbing = false
    @State private var scrubTo: Double = 0
    @State private var showQueue = false

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
                        Text("Now Playing").font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text2)
                        Spacer()
                        Button { showQueue = true } label: {
                            Image(systemName: "list.bullet").font(.system(size: 18)).foregroundColor(Theme.text).frame(width: 44, height: 44)
                        }
                    }
                    Spacer(minLength: 0)
                    CoverView(albumID: t.albumId, corner: 12)
                        .frame(maxWidth: 340, maxHeight: 340)
                        .padding(.horizontal, 28)
                        .shadow(color: Color.black.opacity(0.4), radius: 20, y: 10)
                    Spacer(minLength: 0)
                    HStack(alignment: .center) {
                        VStack(alignment: .leading, spacing: 3) {
                            Text(t.title).font(.system(size: 22, weight: .bold)).foregroundColor(Theme.text).lineLimit(1)
                            Text(t.artist).font(.system(size: 16)).foregroundColor(Theme.text2).lineLimit(1)
                        }
                        Spacer()
                        Button { app.toggleLike(t.id) } label: {
                            Image(systemName: app.isLiked(t.id) ? "heart.fill" : "heart")
                                .font(.system(size: 22)).foregroundColor(app.isLiked(t.id) ? Theme.accent : Theme.text)
                                .frame(width: 44, height: 44)
                        }
                    }
                    .padding(.horizontal, 24)

                    VStack(spacing: 4) {
                        Slider(value: Binding(get: { scrubbing ? scrubTo : player.position }, set: { scrubTo = $0 }),
                               in: 0...max(1, player.duration),
                               onEditingChanged: { editing in
                                   if editing { scrubbing = true; scrubTo = player.position }
                                   else { player.seek(to: scrubTo); scrubbing = false }
                               })
                            .tint(Theme.text)
                        HStack {
                            Text(formatTime(scrubbing ? scrubTo : player.position)).font(.system(size: 12)).foregroundColor(Theme.text2)
                            Spacer()
                            Text(formatTime(player.duration)).font(.system(size: 12)).foregroundColor(Theme.text2)
                        }
                    }
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
                                Circle().fill(Color.white).frame(width: 68, height: 68)
                                Image(systemName: player.isPlaying ? "pause.fill" : "play.fill").font(.system(size: 28)).foregroundColor(.black)
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
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .background(Theme.surface)
            .navigationTitle("Queue")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
        .preferredColorScheme(.dark)
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
