import SwiftUI
import PhotosUI

/// Add songs to one of your playlists (yours, or one you share), or start a new one with them.
struct AddToPlaylistButton: View {
    @EnvironmentObject var app: AppModel
    let ids: [String]
    var label = "plus"
    var plain = false
    /// a small + for a song's row
    var compact = false
    @State private var naming = false
    @State private var name = ""

    private var inAny: Bool {
        guard ids.count == 1, let id = ids.first else { return false }
        return app.editablePlaylists.contains { $0.trackIDs.contains(id) }
    }

    var body: some View {
        Menu {
            ForEach(app.editablePlaylists) { p in
                Button(p.name) { app.addTracks(ids, to: p.id) }
            }
            Button { naming = true } label: { Label("New playlist…", systemImage: "plus") }
        } label: {
            if compact {
                Image(systemName: "plus").font(.system(size: 16)).foregroundColor(Theme.text2).frame(width: 32, height: 34)
            } else if plain {
                Image(systemName: inAny ? "checkmark" : label).font(.system(size: 20)).foregroundColor(inAny ? Theme.accent : Theme.text).frame(width: 40, height: 44)
            } else {
                Image(systemName: label).font(.system(size: 18, weight: .semibold)).foregroundColor(Theme.text)
                    .frame(width: 44, height: 44).glass(Circle())
            }
        }
        .alert("New playlist", isPresented: $naming) {
            TextField("Name", text: $name)
            Button("Create") { app.createPlaylist(name, tracks: ids); name = "" }
            Button("Cancel", role: .cancel) { name = "" }
        }
    }
}

/// The ⋯ on an album: queue it, share it, see its details.
struct AlbumMenuButton: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    let album: Album
    let ids: [String]
    @State private var sharing = false
    @State private var info = false
    @State private var pickingPhoto = false
    @State private var picked: PhotosPickerItem?
    @State private var photo: UIImage?

    var body: some View {
        Menu {
            Button { player.playNext(ids: ids) } label: { Label("Play next", systemImage: "text.insert") }
            Button { player.addToQueue(ids: ids) } label: { Label("Add to queue", systemImage: "text.append") }
            Button { info = true } label: { Label("Credits & info", systemImage: "book") }
            if !album.cover || album.customCover {
                Button { pickingPhoto = true } label: { Label("Change picture", systemImage: "photo") }
            }
            if album.customCover {
                Button { removePicture() } label: { Label("Remove picture", systemImage: "xmark") }
            }
            if app.can("share") && !ids.isEmpty {
                Button { sharing = true } label: { Label("Share link…", systemImage: "link") }
            }
        } label: {
            Image(systemName: "ellipsis").font(.system(size: 18, weight: .semibold)).foregroundColor(Theme.text)
                .frame(width: 44, height: 44).glass(Circle())
        }
        .sheet(isPresented: $sharing) { ShareLinkSheet(kind: "album", name: album.title, trackIDs: ids).environmentObject(app) }
        .sheet(isPresented: $info) { AlbumInfoSheet(album: album).environmentObject(app) }
        .photosPicker(isPresented: $pickingPhoto, selection: $picked, matching: .images)
        .onChange(of: picked) { item in
            guard let item = item else { return }
            Task {
                if let data = try? await item.loadTransferable(type: Data.self), let img = UIImage(data: data) { photo = img }
                picked = nil
            }
        }
        .sheet(item: Binding(get: { photo.map { PhotoBox(image: $0) } }, set: { if $0 == nil { photo = nil } })) { box in
            PictureCropper(image: box.image, out: 800, title: "Adjust the picture", round: false) { jpeg in
                do {
                    _ = try await app.api?.putImage("/api/album/" + (app.api?.encode(album.id) ?? album.id) + "/picture", jpeg: jpeg)
                    await app.refresh()
                    return nil
                } catch { return error.localizedDescription }
            }
        }
    }

    private func removePicture() {
        Task {
            _ = try? await app.call("DELETE", "/api/album/" + (app.api?.encode(album.id) ?? album.id) + "/picture")
            await app.refresh()
        }
    }
}

struct AlbumInfoSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    let album: Album

    private func row(_ k: String, _ v: String) -> some View {
        HStack(alignment: .top) {
            Text(k).font(.system(size: 13)).foregroundColor(Theme.text2).frame(width: 110, alignment: .leading)
            Text(v).font(.system(size: 14)).foregroundColor(Theme.text).frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    var body: some View {
        let tracks = app.tracks(of: album)
        let total = tracks.reduce(0) { $0 + $1.duration }
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    HStack(spacing: 14) {
                        CoverView(albumID: album.id, corner: 8).frame(width: 84, height: 84)
                        VStack(alignment: .leading, spacing: 3) {
                            Text(album.title).font(.system(size: 18, weight: .bold)).foregroundColor(Theme.text)
                            Text(album.artist).font(.system(size: 14)).foregroundColor(Theme.text2)
                        }
                    }
                    VStack(alignment: .leading, spacing: 9) {
                        if let y = album.year { row("Year", String(y)) }
                        if let g = album.genre, !g.isEmpty { row("Genre", g) }
                        if let l = album.label, !l.isEmpty { row("Label", l) }
                        row("Songs", "\(tracks.count)")
                        if total > 0 { row("Length", formatLength(total)) }
                        if let c = album.copyright, !c.isEmpty { row("Copyright", c) }
                    }
                    Text("People on this album").font(.system(size: 12, weight: .bold)).foregroundColor(Theme.text2).textCase(.uppercase).padding(.top, 4)
                    VStack(alignment: .leading, spacing: 6) {
                        ForEach(people(tracks), id: \.self) { n in
                            if let r = app.personRoute(n) {
                                NavigationLink(value: r) { Text(n).font(.system(size: 14)).foregroundColor(Theme.accent) }
                            } else {
                                Text(n).font(.system(size: 14)).foregroundColor(Theme.text)
                            }
                        }
                    }
                }
                .padding(16)
            }
            .background(Theme.surface)
            .navigationTitle("Credits & info")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
            .routeDestinations()
        }
    }

    private func people(_ tracks: [Track]) -> [String] {
        var seen = Set<String>(), out: [String] = []
        func add(_ n: String) {
            let c = n.trimmingCharacters(in: .whitespaces)
            if !c.isEmpty && !seen.contains(c.lowercased()) { seen.insert(c.lowercased()); out.append(c) }
        }
        add(album.artist)
        for t in tracks {
            add(t.artist)
            for c in t.credits where c.role != "ISRC" { for n in c.names.components(separatedBy: ", ") { add(n) } }
        }
        return Array(out.prefix(60))
    }
}
