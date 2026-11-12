import SwiftUI
import AVKit

/// Where a tap on a card or a name goes.
enum Route: Hashable {
    case album(String)
    case artist(String)
    case playlist(String)
    case notifications
    case account
    case discoverAlbum(String)
    case discoverArtist(String)
    case mix(String)
    case rewind(Int)
    case credit(String)
    case playback
    case profile
    case settings
    case members
    case folder
    case lidarr
    case shares
    case browse(String, String)
}

/// An album's cover (a rounded square). Without art it shows the title on a tint, like the website.
struct CoverView: View {
    @EnvironmentObject var app: AppModel
    let albumID: String?
    var corner: CGFloat = 8
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            if let image = image {
                Image(uiImage: image).resizable().scaledToFill()
            } else {
                placeholder
            }
        }
        .aspectRatio(1, contentMode: .fit)
        .clipShape(RoundedRectangle(cornerRadius: corner, style: .continuous))
        .task(id: albumID) { await load() }
    }

    private var placeholder: some View {
        let title = albumID.flatMap { app.album($0)?.title } ?? ""
        return ZStack {
            Theme.tint(title.isEmpty ? "x" : title)
            Text(String(title.prefix(1)).uppercased())
                .font(.system(size: 26, weight: .bold))
                .foregroundColor(Color.white.opacity(0.9))
        }
    }

    private func load() async {
        image = nil
        guard let id = albumID, let album = app.album(id), album.cover, let api = app.api, let url = api.coverURL(id, version: album.coverV) else { return }
        image = await ImageLoader.shared.image(url, auth: api.authHeader)
    }
}

/// A picture the server hands out by path (found music's covers), as a rounded square.
struct RemoteSquare: View {
    @EnvironmentObject var app: AppModel
    let path: String?
    var title: String = ""
    var corner: CGFloat = 8
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            if let image = image {
                Image(uiImage: image).resizable().scaledToFill()
            } else {
                ZStack {
                    Theme.tint(title.isEmpty ? "x" : title)
                    Text(String(title.prefix(1)).uppercased()).font(.system(size: 20, weight: .bold)).foregroundColor(Color.white.opacity(0.9))
                }
            }
        }
        .aspectRatio(1, contentMode: .fit)
        .clipShape(RoundedRectangle(cornerRadius: corner, style: .continuous))
        .task(id: path) {
            image = nil
            guard let p = path, let api = app.api, let url = api.imageURL(p) else { return }
            image = await ImageLoader.shared.image(url, auth: api.authHeader)
        }
    }
}

/// An artist's picture: their photo (a rounded square) when the server has one, else their initials on a tint.
struct ArtistPic: View {
    @EnvironmentObject var app: AppModel
    let name: String
    var corner: CGFloat = 10
    var photo = true
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            if let image = image {
                Image(uiImage: image).resizable().scaledToFill()
            } else {
                ZStack {
                    Theme.tint(name)
                    Text(initials(name)).font(.system(size: 22, weight: .bold)).foregroundColor(Color.white.opacity(0.92))
                }
            }
        }
        .aspectRatio(1, contentMode: .fit)
        .clipShape(RoundedRectangle(cornerRadius: corner, style: .continuous))
        .task(id: name) { await load() }
    }

    private func load() async {
        image = nil
        guard photo, app.me?.artistInfo == true else { return }
        app.loadInfo(name)
        var tries = 0
        while app.info(for: name) == nil && tries < 40 {
            try? await Task.sleep(nanoseconds: 700_000_000)
            if Task.isCancelled { return }
            app.loadInfo(name)
            tries += 1
        }
        guard let url = app.info(for: name)?.imageURL, let api = app.api else { return }
        image = await ImageLoader.shared.image(url, auth: api.authHeader)
    }
}

struct SectionTitle: View {
    let text: String
    var body: some View {
        Text(text)
            .font(.system(size: 22, weight: .bold))
            .foregroundColor(Theme.text)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 16)
    }
}

/// The bar across the top of the main screens: the logo, notifications and your account.
struct TopBar: View {
    @EnvironmentObject var app: AppModel

    var body: some View {
        HStack(spacing: 10) {
            RecordMark().frame(width: 26, height: 26)
            Text("Volvix").font(.system(size: 20, weight: .heavy)).foregroundColor(Theme.text)
            Spacer()
            NavigationLink(value: Route.notifications) {
                ZStack(alignment: .topTrailing) {
                    Image(systemName: "bell").font(.system(size: 19)).foregroundColor(Theme.text2).frame(width: 40, height: 40)
                    if app.unread > 0 {
                        Text(app.unread > 99 ? "99+" : "\(app.unread)")
                            .font(.system(size: 10, weight: .bold)).foregroundColor(.white)
                            .padding(.horizontal, 5).frame(minWidth: 17, minHeight: 17)
                            .background(Theme.accent).clipShape(Capsule())
                            .offset(x: 2, y: 2)
                    }
                }
            }
            .buttonStyle(.plain)
            NavigationLink(value: Route.account) {
                AvatarView(id: app.user?.id ?? "", name: app.user?.name ?? "me", size: 32)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 6)
    }
}

/// A square card with a cover, a title and a line under it.
struct AlbumCard: View {
    let album: Album
    var width: CGFloat = 150

    var body: some View {
        NavigationLink(value: Route.album(album.id)) {
            VStack(alignment: .leading, spacing: 6) {
                CoverView(albumID: album.id, corner: 8).frame(width: width, height: width)
                Text(album.title).font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                Text(album.artist).font(.system(size: 12)).foregroundColor(Theme.text2).lineLimit(1)
            }
            .frame(width: width, alignment: .leading)
        }
        .buttonStyle(.plain)
    }
}

struct ArtistCard: View {
    let name: String
    var width: CGFloat = 150

    var body: some View {
        NavigationLink(value: Route.artist(name)) {
            VStack(alignment: .leading, spacing: 6) {
                ArtistPic(name: name).frame(width: width, height: width)
                Text(name).font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                Text("Artist").font(.system(size: 12)).foregroundColor(Theme.text2)
            }
            .frame(width: width, alignment: .leading)
        }
        .buttonStyle(.plain)
    }
}

/// A row of cards that scrolls sideways, under a title.
struct Shelf<Content: View>: View {
    let title: String
    let content: Content

    init(title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            SectionTitle(text: title)
            ScrollView(.horizontal, showsIndicators: false) {
                LazyHStack(alignment: .top, spacing: 14) { content }
                    .padding(.horizontal, 16)
            }
        }
    }
}

/// One song in a list. Tap to play; the menu has the rest.
struct TrackRow: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @EnvironmentObject var downloads: DownloadStore
    let track: Track
    var number: Int? = nil
    var showCover = false
    var onRemove: (() -> Void)? = nil
    let onPlay: () -> Void
    @State private var showCredits = false

    var body: some View {
        HStack(spacing: 12) {
            if showCover {
                CoverView(albumID: track.albumId, corner: 5).frame(width: 44, height: 44)
            } else if let n = number {
                Text("\(n)").font(.system(size: 14)).foregroundColor(Theme.text3).frame(width: 26)
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(track.title)
                    .font(.system(size: 16))
                    .foregroundColor(player.currentID == track.id ? Theme.accent : Theme.text)
                    .lineLimit(1)
                Text(track.artist).font(.system(size: 13)).foregroundColor(Theme.text2).lineLimit(1)
            }
            Spacer(minLength: 8)
            if app.isLiked(track.id) {
                Image(systemName: "heart.fill").font(.system(size: 13)).foregroundColor(Theme.accent)
            }
            Menu {
                Button { player.playNext(track.id) } label: { Label("Play next", systemImage: "text.insert") }
                Button { player.addToQueue(track.id) } label: { Label("Add to queue", systemImage: "text.append") }
                Button { app.toggleLike(track.id) } label: {
                    Label(app.isLiked(track.id) ? "Remove from Liked Songs" : "Add to Liked Songs", systemImage: app.isLiked(track.id) ? "heart.slash" : "heart")
                }
                Button {
                    if downloads.has(track.id) { downloads.remove([track.id]) } else { downloads.add([track.id]) }
                } label: {
                    Label(downloads.has(track.id) ? "Remove download" : "Download", systemImage: downloads.has(track.id) ? "trash" : "arrow.down.circle")
                }
                if !app.yourPlaylists.isEmpty {
                    Menu {
                        ForEach(app.yourPlaylists) { p in
                            Button(p.name) { app.addTracks([track.id], to: p.id) }
                        }
                    } label: { Label("Add to playlist", systemImage: "plus.square.on.square") }
                }
                Button { showCredits = true } label: { Label("Credits", systemImage: "book") }
                if let remove = onRemove {
                    Button(role: .destructive) { remove() } label: { Label("Remove from this playlist", systemImage: "minus.circle") }
                }
            } label: {
                Image(systemName: "ellipsis").font(.system(size: 16)).foregroundColor(Theme.text2).frame(width: 34, height: 34)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 6)
        .opacity(app.localOnly && !downloads.has(track.id) ? 0.4 : 1)
        .contentShape(Rectangle())
        .onTapGesture { onPlay() }
        .sheet(isPresented: $showCredits) {
            CreditsSheet(trackID: track.id).environmentObject(app).environmentObject(player).environmentObject(downloads)
        }
    }
}

struct PillButton: View {
    let title: String
    var icon: String? = nil
    var primary = true
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                if let icon = icon { Image(systemName: icon) }
                Text(title).font(.system(size: 15, weight: .semibold))
            }
            .padding(.horizontal, 22)
            .padding(.vertical, 11)
            .background(primary ? Theme.accent : Theme.surface3)
            .foregroundColor(.white)
            .clipShape(Capsule())
        }
    }
}

struct ProblemBox: View {
    let text: String
    var body: some View {
        Text(text)
            .font(.system(size: 13))
            .foregroundColor(Color(red: 1, green: 0.72, blue: 0.72))
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.accent.opacity(0.14))
            .clipShape(RoundedRectangle(cornerRadius: 10))
    }
}

/// The system's own speaker picker: AirPlay speakers and TVs, and Bluetooth headphones.
struct AirPlayButton: UIViewRepresentable {
    func makeUIView(context: Context) -> AVRoutePickerView {
        let v = AVRoutePickerView()
        v.tintColor = UIColor(Theme.text)
        v.activeTintColor = UIColor(Theme.accent)
        v.prioritizesVideoDevices = false
        return v
    }

    func updateUIView(_ uiView: AVRoutePickerView, context: Context) {}
}
