import SwiftUI

/// Where a tap on a card or a name goes.
enum Route: Hashable {
    case album(String)
    case artist(String)
    case playlist(String)
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

/// An artist's picture stand-in: their initials on a tint (a rounded square).
struct ArtistPic: View {
    let name: String
    var corner: CGFloat = 10

    var body: some View {
        ZStack {
            Theme.tint(name)
            Text(initials(name))
                .font(.system(size: 22, weight: .bold))
                .foregroundColor(Color.white.opacity(0.92))
        }
        .aspectRatio(1, contentMode: .fit)
        .clipShape(RoundedRectangle(cornerRadius: corner, style: .continuous))
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
    let track: Track
    var number: Int? = nil
    var showCover = false
    let onPlay: () -> Void

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
            } label: {
                Image(systemName: "ellipsis").font(.system(size: 16)).foregroundColor(Theme.text2).frame(width: 34, height: 34)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 6)
        .contentShape(Rectangle())
        .onTapGesture { onPlay() }
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
