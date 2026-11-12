import SwiftUI
import PhotosUI

/// An invitation to edit someone's playlist.
struct PlaylistInvite: Identifiable, Hashable {
    var id: String
    var name: String
    var ownerName: String
    var count: Int
}

extension AppModel {
    var myID: String { user?.id ?? "" }

    /// Playlists you can add songs to: your own, and the ones you share with others.
    var editablePlaylists: [PlaylistInfo] { (playlists + sharedLists).filter { $0.editable && !$0.liked } }

    /// Everything of yours (Liked Songs first), then the ones you follow.
    var followedLists: [PlaylistInfo] { householdLists.filter { followedIDs.contains($0.id) } }

    func playlistInfo(shared o: [String: Any]) -> PlaylistInfo? {
        guard let id = o["id"] as? String else { return nil }
        let owner = (o["owner"] as? String) ?? ""
        let collab = (o["collaborators"] as? [String]) ?? []
        let entries = (o["tracks"] as? [[String: Any]]) ?? []
        var p = PlaylistInfo(id: id, name: (o["name"] as? String) ?? "Playlist", description: (o["description"] as? String) ?? "",
                             trackIDs: entries.compactMap { $0["id"] as? String }, liked: false)
        p.owner = owner
        p.mine = owner == myID
        p.shared = true
        p.collaborators = collab
        p.invited = (o["invited"] as? [String]) ?? []
        p.isPublic = (o["public"] as? Bool) ?? false
        p.addedBy = entries.map { ($0["addedBy"] as? String) ?? owner }
        p.rev = wholeNumber(o["rev"])
        p.editable = p.mine || collab.contains(myID)
        p.cover = wholeNumber(((userData["sharedCovers"] as? [String: Any]) ?? [:])[id])
        return p
    }

    /// The playlists several people edit, and invitations to join some.
    func loadShared() async {
        guard let r = try? await call("GET", "/api/shared") else { return }
        sharedLists = ((r["playlists"] as? [[String: Any]]) ?? []).compactMap { playlistInfo(shared: $0) }
        invites = ((r["invites"] as? [[String: Any]]) ?? []).map {
            PlaylistInvite(id: ($0["id"] as? String) ?? "", name: ($0["name"] as? String) ?? "", ownerName: ($0["ownerName"] as? String) ?? "", count: wholeNumber($0["count"]))
        }
    }

    private func applyShared(_ r: [String: Any]) {
        guard let o = r["playlist"] as? [String: Any], let id = o["id"] as? String else { return }
        if (r["gone"] as? Bool) == true {
            sharedLists.removeAll { $0.id == id }
        } else if let p = playlistInfo(shared: o) {
            if let i = sharedLists.firstIndex(where: { $0.id == id }) { sharedLists[i] = p } else { sharedLists.append(p) }
        }
    }

    /// One change to a playlist several people edit. The server answers with the playlist as it is now.
    @discardableResult
    func sharedOp(_ id: String, _ body: [String: Any]) async -> Bool {
        do {
            let r = try await call("POST", "/api/shared/" + (api?.encode(id) ?? id) + "/op", body)
            applyShared(r)
            return true
        } catch {
            message = error.localizedDescription
            await loadShared()
            return false
        }
    }

    func toggleFollowPlaylist(_ id: String) {
        let on = !followedIDs.contains(id)
        if on { followedIDs.append(id) } else { followedIDs.removeAll { $0 == id } }
        edit { d in
            var list = (d["followed"] as? [String]) ?? []
            list.removeAll { $0 == id }
            if on { list.append(id) }
            d["followed"] = list
        }
    }

    func addTracks(_ ids: [String], to playlistID: String) {
        guard let p = playlist(playlistID), p.editable else { return }
        var seen = Set(p.trackIDs)
        let unique = ids.filter { seen.insert($0).inserted }
        if unique.isEmpty { return }
        let now = Int(Date().timeIntervalSince1970)
        if p.shared {
            if let i = sharedLists.firstIndex(where: { $0.id == playlistID }) {
                sharedLists[i].trackIDs.append(contentsOf: unique)
                sharedLists[i].addedBy.append(contentsOf: unique.map { _ in myID })
            }
            Task { await sharedOp(playlistID, ["op": "add", "ids": unique]) }
            return
        }
        if let i = playlists.firstIndex(where: { $0.id == playlistID }) { playlists[i].trackIDs.append(contentsOf: unique) }
        edit { d in
            var lists = (d["playlists"] as? [[String: Any]]) ?? []
            guard let i = lists.firstIndex(where: { ($0["id"] as? String) == playlistID }) else { return }
            var entries = (lists[i]["tracks"] as? [[String: Any]]) ?? []
            for t in unique { entries.append(["id": t, "addedBy": "u_me", "at": now]) }
            lists[i]["tracks"] = entries
            d["playlists"] = lists
        }
    }

    func movePlaylistTrack(from: Int, to: Int, in playlistID: String) {
        guard let p = playlist(playlistID), p.editable, from != to, from >= 0, to >= 0, from < p.trackIDs.count, to < p.trackIDs.count else { return }
        if p.shared {
            if let i = sharedLists.firstIndex(where: { $0.id == playlistID }) {
                let t = sharedLists[i].trackIDs.remove(at: from)
                sharedLists[i].trackIDs.insert(t, at: to)
                if sharedLists[i].addedBy.count > max(from, to) {
                    let a = sharedLists[i].addedBy.remove(at: from)
                    sharedLists[i].addedBy.insert(a, at: to)
                }
            }
            Task { await sharedOp(playlistID, ["op": "move", "from": from, "to": to]) }
            return
        }
        if let i = playlists.firstIndex(where: { $0.id == playlistID }) {
            let t = playlists[i].trackIDs.remove(at: from)
            playlists[i].trackIDs.insert(t, at: to)
        }
        edit { d in
            var lists = (d["playlists"] as? [[String: Any]]) ?? []
            guard let i = lists.firstIndex(where: { ($0["id"] as? String) == playlistID }) else { return }
            var entries = (lists[i]["tracks"] as? [[String: Any]]) ?? []
            if from < entries.count && to < entries.count {
                let e = entries.remove(at: from)
                entries.insert(e, at: to)
            }
            lists[i]["tracks"] = entries
            d["playlists"] = lists
        }
    }

    /// Name, description and who in the household can see it.
    func updatePlaylist(_ id: String, name: String, description: String, isPublic: Bool) {
        let clean = name.trimmingCharacters(in: .whitespaces)
        guard let p = playlist(id), p.mine, !clean.isEmpty else { return }
        if p.shared {
            if let i = sharedLists.firstIndex(where: { $0.id == id }) {
                sharedLists[i].name = clean
                sharedLists[i].description = description
                sharedLists[i].isPublic = isPublic
            }
            Task { await sharedOp(id, ["op": "update", "name": clean, "description": description, "public": isPublic]) }
            return
        }
        if let i = playlists.firstIndex(where: { $0.id == id }) {
            playlists[i].name = clean
            playlists[i].description = description
            playlists[i].isPublic = isPublic
        }
        edit { d in
            var lists = (d["playlists"] as? [[String: Any]]) ?? []
            guard let i = lists.firstIndex(where: { ($0["id"] as? String) == id }) else { return }
            lists[i]["name"] = clean
            lists[i]["description"] = description
            lists[i]["public"] = isPublic
            d["playlists"] = lists
        }
    }

    func removeSharedTrack(at position: Int, from id: String) {
        guard let i = sharedLists.firstIndex(where: { $0.id == id }), position >= 0, position < sharedLists[i].trackIDs.count else { return }
        let t = sharedLists[i].trackIDs.remove(at: position)
        if sharedLists[i].addedBy.count > position { sharedLists[i].addedBy.remove(at: position) }
        Task { await sharedOp(id, ["op": "remove", "index": position, "trackId": t]) }
    }

    func deleteSharedPlaylist(_ id: String) {
        sharedLists.removeAll { $0.id == id }
        Task { await sharedOp(id, ["op": "delete"]) }
    }

    func leavePlaylist(_ id: String) {
        sharedLists.removeAll { $0.id == id }
        Task { await sharedOp(id, ["op": "leave"]); await loadShared() }
    }

    func answerInvite(_ id: String, accept: Bool) {
        invites.removeAll { $0.id == id }
        Task {
            await sharedOp(id, ["op": accept ? "accept" : "decline"])
            await loadShared()
            await pollOnce()
        }
    }

    /// Turn one of your playlists into one other people can edit too (you stay the owner).
    func makeShared(_ id: String) async -> PlaylistInfo? {
        guard let p = playlist(id), p.mine else { return nil }
        if p.shared { return p }
        do {
            let r = try await call("POST", "/api/shared", ["name": p.name, "description": p.description, "public": p.isPublic, "tracks": p.trackIDs])
            guard let o = r["playlist"] as? [String: Any], let made = playlistInfo(shared: o) else { return nil }
            sharedLists.append(made)
            playlists.removeAll { $0.id == id }
            edit { d in
                var lists = (d["playlists"] as? [[String: Any]]) ?? []
                lists.removeAll { ($0["id"] as? String) == id }
                d["playlists"] = lists
            }
            return made
        } catch {
            message = error.localizedDescription
            return nil
        }
    }

    /// Hand a shared playlist back to just you. Everyone else loses access.
    func unshare(_ id: String) async {
        do {
            let r = try await call("POST", "/api/shared/" + (api?.encode(id) ?? id) + "/op", ["op": "unshare"])
            guard let o = r["playlist"] as? [String: Any], let made = playlistInfo(shared: o) else { return }
            sharedLists.removeAll { $0.id == id }
            let now = Int(Date().timeIntervalSince1970)
            let entries: [[String: Any]] = made.trackIDs.map { ["id": $0, "addedBy": "u_me", "at": now] }
            let pl: [String: Any] = ["id": made.id, "name": made.name, "description": made.description, "owner": "u_me", "public": made.isPublic,
                                      "collaborative": false, "collaborators": [String](), "tracks": entries]
            var local = made
            local.shared = false
            local.collaborators = []
            local.invited = []
            playlists.append(local)
            edit { d in
                var lists = (d["playlists"] as? [[String: Any]]) ?? []
                lists.append(pl)
                d["playlists"] = lists
            }
        } catch { message = error.localizedDescription }
    }

    func invitePeople(_ ids: [String], to playlistID: String) async {
        var id = playlistID
        if let p = playlist(playlistID), !p.shared {
            guard let made = await makeShared(playlistID) else { return }
            id = made.id
        }
        await sharedOp(id, ["op": "invite", "userIds": ids])
    }

    func uninvite(_ userID: String, from playlistID: String) {
        Task { await sharedOp(playlistID, ["op": "uninvite", "userId": userID]) }
    }

    func copyPlaylist(_ id: String) {
        guard let p = playlist(id) else { return }
        createPlaylist(p.name + " (copy)", tracks: p.trackIDs, description: p.description)
    }

    /// A playlist's own picture, kept on the server.
    func setPlaylistPicture(_ id: String, jpeg: Data) async -> String? {
        guard let p = playlist(id) else { return "That playlist isn't here." }
        do {
            guard let r = try await api?.putImage("/api/playlists/" + (api?.encode(id) ?? id) + "/cover", jpeg: jpeg) else { return nil }
            let v = wholeNumber(r["cover"])
            applyCover(id, v, shared: p.shared)
            return nil
        } catch { return error.localizedDescription }
    }

    func clearPlaylistPicture(_ id: String) {
        guard let p = playlist(id) else { return }
        Task {
            _ = try? await call("DELETE", "/api/playlists/" + (api?.encode(id) ?? id) + "/cover")
            applyCover(id, 0, shared: p.shared)
        }
    }

    private func applyCover(_ id: String, _ v: Int, shared: Bool) {
        if shared {
            if let i = sharedLists.firstIndex(where: { $0.id == id }) { sharedLists[i].cover = v }
            edit { d in
                var c = (d["sharedCovers"] as? [String: Any]) ?? [:]
                if v > 0 { c[id] = v } else { c.removeValue(forKey: id) }
                d["sharedCovers"] = c
            }
        } else {
            if let i = playlists.firstIndex(where: { $0.id == id }) { playlists[i].cover = v }
            edit { d in
                var lists = (d["playlists"] as? [[String: Any]]) ?? []
                guard let i = lists.firstIndex(where: { ($0["id"] as? String) == id }) else { return }
                lists[i]["cover"] = v
                d["playlists"] = lists
            }
        }
    }
}

// MARK: - Screens

struct PlaylistView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @EnvironmentObject var downloads: DownloadStore
    @Environment(\.dismiss) private var dismiss
    let id: String
    @State private var confirmDelete = false
    @State private var editing = false
    @State private var inviting = false
    @State private var reordering = false
    @State private var sharing = false
    @State private var picking: PhotosPickerItem?
    @State private var photo: UIImage?
    @State private var pickingPhoto = false
    @State private var query = ""

    var body: some View {
        ScrollView {
            if let p = app.playlist(id) ?? (id == "pl_liked" ? PlaylistInfo(id: "pl_liked", name: "Liked Songs", description: "Songs you've liked.", trackIDs: [], liked: true) : nil) {
                let tracks = p.trackIDs.compactMap { app.track($0) }
                let ids = tracks.map { $0.id }
                let total = tracks.reduce(0) { $0 + $1.duration }
                VStack(spacing: 14) {
                    PlaylistArt(playlist: p).frame(width: 220, height: 220).padding(.top, 8)
                    VStack(spacing: 6) {
                        Text(p.shared ? "Collaborative playlist" : "Playlist").font(.system(size: 12, weight: .semibold)).foregroundColor(Theme.text2)
                        Text(p.name).font(.system(size: 24, weight: .bold)).foregroundColor(Theme.text).multilineTextAlignment(.center)
                        if !p.description.isEmpty { Text(p.description).font(.system(size: 14)).foregroundColor(Theme.text2).multilineTextAlignment(.center) }
                        HStack(spacing: 8) {
                            if p.shared {
                                HStack(spacing: -8) {
                                    ForEach(([p.owner] + p.collaborators).prefix(5), id: \.self) { u in
                                        AvatarView(id: u, name: app.personName(u), size: 24).overlay(Circle().stroke(Theme.surface, lineWidth: 2))
                                    }
                                }
                            } else if !p.liked && !p.mine {
                                AvatarView(id: p.owner, name: app.personName(p.owner), size: 24)
                            }
                            Text((p.liked || p.mine ? "" : app.personName(p.owner) + " · ") + "\(tracks.count) song\(tracks.count == 1 ? "" : "s")" + (total > 0 ? ", " + formatLength(total) : ""))
                                .font(.system(size: 13)).foregroundColor(Theme.text2)
                        }
                    }
                    .padding(.horizontal, 16)
                    if !ids.isEmpty {
                        HStack(spacing: 10) {
                            PillButton(title: "Play", icon: "play.fill") { player.play(ids, from: p.name) }
                            PillButton(title: "Shuffle", icon: "shuffle", primary: false) { player.play(ids, shuffled: true, from: p.name) }
                            DownloadButton(ids: ids)
                        }
                    }
                    HStack(spacing: 10) {
                        if !p.mine && !p.liked && !p.shared {
                            PillButton(title: app.followedIDs.contains(p.id) ? "Following" : "Follow", icon: app.followedIDs.contains(p.id) ? "checkmark" : "plus", primary: false) { app.toggleFollowPlaylist(p.id) }
                        }
                        if p.editable && tracks.count > 1 {
                            PillButton(title: "Reorder", icon: "arrow.up.arrow.down", primary: false) { reordering = true }
                        }
                        menu(p, ids: ids)
                    }
                    LazyVStack(spacing: 0) {
                        ForEach(Array(p.trackIDs.enumerated()), id: \.offset) { i, tid in
                            if let t = app.track(tid) {
                                let rm: (() -> Void)? = (p.editable && !p.liked) ? { remove(p, at: i, track: t) } : nil
                                TrackRow(track: t, showCover: true, onRemove: rm) { player.play(ids, startAt: ids.firstIndex(of: tid) ?? 0, from: p.name) }
                            }
                        }
                    }
                    if !p.editable && !p.liked {
                        Label("This is \(app.personName(p.owner))'s playlist. Follow it, or save a copy to make changes.", systemImage: "lock")
                            .font(.system(size: 12)).foregroundColor(Theme.text2).padding(.horizontal, 16)
                    }
                    if p.editable { AddSongsSection(playlist: p) }
                }
                .padding(.bottom, 24)
            } else {
                Text("That playlist isn't here any more.").foregroundColor(Theme.text2).padding(24)
            }
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
        .confirmationDialog("Delete this playlist?", isPresented: $confirmDelete, titleVisibility: .visible) {
            Button("Delete", role: .destructive) {
                if app.playlist(id)?.shared == true { app.deleteSharedPlaylist(id) } else { app.deletePlaylist(id) }
                dismiss()
            }
            Button("Cancel", role: .cancel) {}
        }
        .sheet(isPresented: $editing) { if let p = app.playlist(id) { EditPlaylistSheet(playlist: p).environmentObject(app) } }
        .sheet(isPresented: $inviting) { WhoCanEditSheet(playlistID: id).environmentObject(app) }
        .sheet(isPresented: $reordering) { if let p = app.playlist(id) { ReorderSheet(playlist: p).environmentObject(app) } }
        .sheet(isPresented: $sharing) { if let p = app.playlist(id) { ShareLinkSheet(kind: "playlist", name: p.name, trackIDs: p.trackIDs).environmentObject(app) } }
        .photosPicker(isPresented: $pickingPhoto, selection: $picking, matching: .images)
        .onChange(of: picking) { item in
            guard let item = item else { return }
            Task {
                if let data = try? await item.loadTransferable(type: Data.self), let img = UIImage(data: data) { photo = img }
                picking = nil
            }
        }
        .sheet(item: Binding(get: { photo.map { PhotoBox(image: $0) } }, set: { if $0 == nil { photo = nil } })) { box in
            PictureCropper(image: box.image, out: 512, title: "Adjust the picture", round: false) { jpeg in await app.setPlaylistPicture(id, jpeg: jpeg) }
        }
        .task { await app.loadShared() }
    }

    private func menu(_ p: PlaylistInfo, ids: [String]) -> some View {
        Menu {
            if p.mine && !p.liked {
                Button { editing = true } label: { Label("Edit details", systemImage: "pencil") }
            }
            if !p.liked && (p.mine || p.shared) {
                Button { pickingPhoto = true } label: { Label("Change picture", systemImage: "photo") }
                if p.cover > 0 { Button { app.clearPlaylistPicture(p.id) } label: { Label("Remove picture", systemImage: "xmark") } }
            }
            if p.mine && !p.liked {
                Button { inviting = true } label: { Label("Who can edit", systemImage: "person.2") }
            }
            if p.shared && !p.mine {
                Button(role: .destructive) { app.leavePlaylist(p.id); dismiss() } label: { Label("Leave playlist", systemImage: "rectangle.portrait.and.arrow.right") }
            }
            if !p.mine && !p.shared && !p.liked {
                Button { app.toggleFollowPlaylist(p.id) } label: { Label(app.followedIDs.contains(p.id) ? "Unfollow" : "Follow", systemImage: app.followedIDs.contains(p.id) ? "xmark" : "plus") }
                Button { app.copyPlaylist(p.id) } label: { Label("Save a copy to my playlists", systemImage: "doc.on.doc") }
            }
            if !ids.isEmpty {
                Button { player.playNext(ids: ids) } label: { Label("Play next", systemImage: "text.insert") }
                Button { player.addToQueue(ids: ids) } label: { Label("Add to queue", systemImage: "text.append") }
            }
            if !ids.isEmpty && app.can("share") {
                Button { sharing = true } label: { Label("Share link…", systemImage: "link") }
            }
            if p.mine && !p.liked {
                Button(role: .destructive) { confirmDelete = true } label: { Label("Delete playlist", systemImage: "trash") }
            }
        } label: {
            Image(systemName: "ellipsis").font(.system(size: 18, weight: .semibold)).foregroundColor(Theme.text)
                .frame(width: 44, height: 44).glass(Circle())
        }
    }

    private func remove(_ p: PlaylistInfo, at i: Int, track: Track) {
        if p.liked { app.toggleLike(track.id) }
        else if p.shared { app.removeSharedTrack(at: i, from: p.id) }
        else { app.removeTrack(at: i, from: p.id) }
    }
}

/// Search for songs to add, or start from a few from your library.
struct AddSongsSection: View {
    @EnvironmentObject var app: AppModel
    let playlist: PlaylistInfo
    @State private var query = ""

    private var results: [Track] {
        let have = Set(playlist.trackIDs)
        let q = query.trimmingCharacters(in: .whitespaces)
        if !q.isEmpty {
            func hit(_ s: String) -> Bool { s.range(of: q, options: [.caseInsensitive, .diacriticInsensitive]) != nil }
            return Array(app.allTracks.filter { !have.contains($0.id) && (hit($0.title) || hit($0.artist)) }.prefix(8))
        }
        // a few picked for this playlist: the same ones until it changes
        let pool = app.allTracks.filter { !have.contains($0.id) }
        guard !pool.isEmpty else { return [] }
        var seed = 0
        for u in playlist.id.unicodeScalars { seed = (seed &* 31 &+ Int(u.value)) & 0xFFFFFF }
        var out: [Track] = []
        for i in 0..<min(pool.count, 5) { out.append(pool[(seed + i * 7) % pool.count]) }
        var seen = Set<String>()
        return out.filter { seen.insert($0.id).inserted }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(playlist.trackIDs.isEmpty ? "Let's find something for your playlist" : "Add songs").font(.system(size: 20, weight: .bold)).foregroundColor(Theme.text)
            HStack(spacing: 10) {
                Image(systemName: "magnifyingglass").foregroundColor(Theme.text2)
                TextField("Search for songs to add", text: $query).textInputAutocapitalization(.never).autocorrectionDisabled().foregroundColor(Theme.text)
                if !query.isEmpty { Button { query = "" } label: { Image(systemName: "xmark.circle.fill").foregroundColor(Theme.text3) } }
            }
            .padding(12).background(Theme.surface3).clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            Text(query.isEmpty ? "Recommended from your library" : (results.isEmpty ? "No songs match \"\(query)\"" : "Results")).font(.system(size: 12, weight: .semibold)).foregroundColor(Theme.text2)
            ForEach(results) { t in
                HStack(spacing: 12) {
                    CoverView(albumID: t.albumId, corner: 5).frame(width: 44, height: 44)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(t.title).font(.system(size: 15)).foregroundColor(Theme.text).lineLimit(1)
                        Text(t.artist).font(.system(size: 12)).foregroundColor(Theme.text2).lineLimit(1)
                    }
                    Spacer()
                    Button { app.addTracks([t.id], to: playlist.id) } label: {
                        Text("Add").font(.system(size: 13, weight: .semibold)).padding(.horizontal, 16).padding(.vertical, 6).background(Theme.surface3).foregroundColor(Theme.text).clipShape(Capsule())
                    }
                }
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 10)
    }
}

struct EditPlaylistSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    let playlist: PlaylistInfo
    @State private var name = ""
    @State private var text = ""
    @State private var isPublic = false

    var body: some View {
        NavigationStack {
            Form {
                Section("Name") { TextField("Name", text: $name) }
                Section("Description") { TextField("Description", text: $text, axis: .vertical).lineLimit(1...4) }
                Section {
                    Toggle("Visible to the household", isOn: $isPublic).tint(Theme.accent)
                } footer: { Text("Others on this server can find this playlist on the Household page and follow it.") }
            }
            .scrollContentBackground(.hidden)
            .background(Theme.surface)
            .navigationTitle("Edit details")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { app.updatePlaylist(playlist.id, name: name, description: text, isPublic: isPublic); dismiss() }
                        .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
            .onAppear { name = playlist.name; text = playlist.description; isPublic = playlist.isPublic }
        }
        .presentationDetents([.medium])
    }
}

/// Who may edit a playlist: everyone you invite adds and removes songs in the same copy.
struct WhoCanEditSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    let playlistID: String
    @State private var chosen: Set<String> = []

    var body: some View {
        let p = app.playlist(playlistID)
        let inside = Set(([p?.owner ?? ""]) + (p?.collaborators ?? []) + (p?.invited ?? []))
        let others = app.people.filter { $0.id != app.myID && !inside.contains($0.id) }
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    Text("People you invite can add, remove and reorder songs in this playlist. You stay the owner.").font(.system(size: 14)).foregroundColor(Theme.text2)
                    if let p = p {
                        row(p.owner, "Owner", nil)
                        ForEach(p.collaborators, id: \.self) { u in row(u, "Can edit", { app.uninvite(u, from: p.id) }) }
                        ForEach(p.invited, id: \.self) { u in row(u, "Invited", { app.uninvite(u, from: p.id) }) }
                    }
                    if !others.isEmpty {
                        Text("Invite people").font(.system(size: 12, weight: .bold)).foregroundColor(Theme.text2).textCase(.uppercase).padding(.top, 6)
                        ForEach(others) { o in
                            Button { if chosen.contains(o.id) { chosen.remove(o.id) } else { chosen.insert(o.id) } } label: {
                                HStack(spacing: 12) {
                                    AvatarView(id: o.id, name: o.name, size: 38)
                                    Text(o.name).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                                    Spacer()
                                    Image(systemName: chosen.contains(o.id) ? "checkmark.circle.fill" : "circle").font(.system(size: 22)).foregroundColor(chosen.contains(o.id) ? Theme.accent : Theme.text3)
                                }
                                .padding(10).background(Theme.surface2).clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                            }
                            .buttonStyle(.plain)
                        }
                        if !chosen.isEmpty {
                            PillButton(title: "Send invites", icon: "paperplane.fill") {
                                let ids = Array(chosen)
                                chosen = []
                                Task { await app.invitePeople(ids, to: playlistID) }
                            }
                        }
                    }
                    if let p = p, p.shared, p.mine {
                        Button(role: .destructive) { Task { await app.unshare(p.id); dismiss() } } label: {
                            Text("Stop sharing (everyone else loses access)").font(.system(size: 14, weight: .semibold)).frame(maxWidth: .infinity).padding(.vertical, 10)
                        }
                        .padding(.top, 8)
                    }
                }
                .padding(16)
            }
            .background(Theme.surface)
            .navigationTitle("Who can edit")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
            .task { await app.reloadPeople() }
        }
    }

    private func row(_ user: String, _ tag: String, _ remove: (() -> Void)?) -> some View {
        HStack(spacing: 12) {
            AvatarView(id: user, name: app.personName(user), size: 38)
            VStack(alignment: .leading, spacing: 1) {
                Text(app.personName(user)).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                Text(tag).font(.system(size: 12)).foregroundColor(Theme.text2)
            }
            Spacer()
            if let r = remove { Button("Remove", action: r).font(.system(size: 13)).foregroundColor(Theme.accent) }
        }
    }
}

/// Drag the handles to put the songs in a new order.
struct ReorderSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    let playlist: PlaylistInfo

    var body: some View {
        let ids = app.playlist(playlist.id)?.trackIDs ?? playlist.trackIDs
        NavigationStack {
            List {
                ForEach(Array(ids.enumerated()), id: \.offset) { _, id in
                    if let t = app.track(id) {
                        HStack(spacing: 12) {
                            CoverView(albumID: t.albumId, corner: 5).frame(width: 40, height: 40)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(t.title).font(.system(size: 15)).foregroundColor(Theme.text).lineLimit(1)
                                Text(t.artist).font(.system(size: 12)).foregroundColor(Theme.text2).lineLimit(1)
                            }
                        }
                        .listRowBackground(Theme.surface2)
                    }
                }
                .onMove { offsets, destination in
                    guard let from = offsets.first else { return }
                    let to = destination > from ? destination - 1 : destination
                    app.movePlaylistTrack(from: from, to: to, in: playlist.id)
                }
            }
            .listStyle(.insetGrouped)
            .environment(\.editMode, .constant(.active))
            .scrollContentBackground(.hidden)
            .background(Theme.surface)
            .navigationTitle("Reorder")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }
}

/// The playlists tab of the library: yours, invitations, and the ones you follow.
struct PlaylistsTab: View {
    @EnvironmentObject var app: AppModel
    @State private var creating = false
    @State private var newName = ""

    private func rowFor(_ p: PlaylistInfo, sub: String) -> some View {
        NavigationLink(value: Route.playlist(p.id)) {
            HStack(spacing: 14) {
                PlaylistArt(playlist: p).frame(width: 56, height: 56)
                VStack(alignment: .leading, spacing: 2) {
                    Text(p.name).font(.system(size: 16)).foregroundColor(Theme.text).lineLimit(1)
                    Text(sub).font(.system(size: 13)).foregroundColor(Theme.text2).lineLimit(1)
                }
                Spacer()
                Image(systemName: "chevron.right").font(.system(size: 13)).foregroundColor(Theme.text3)
            }
            .padding(.horizontal, 16).padding(.vertical, 6)
        }
        .buttonStyle(.plain)
    }

    private func count(_ p: PlaylistInfo) -> String {
        "\(p.trackIDs.count) song\(p.trackIDs.count == 1 ? "" : "s")" + (p.shared ? " · shared" : "")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            PillButton(title: "New playlist", icon: "plus", primary: false) { creating = true }.padding(.horizontal, 16)
            if !app.invites.isEmpty {
                Text("Invitations").font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text2).padding(.horizontal, 16).padding(.top, 6)
                ForEach(app.invites) { i in
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(i.name).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                            Text("\(i.ownerName) invited you to edit · \(i.count) songs").font(.system(size: 12)).foregroundColor(Theme.text2).lineLimit(1)
                        }
                        Spacer()
                        Button("Accept") { app.answerInvite(i.id, accept: true) }
                            .font(.system(size: 13, weight: .semibold)).padding(.horizontal, 14).padding(.vertical, 6).background(Theme.accent).foregroundColor(.white).clipShape(Capsule())
                        Button("Decline") { app.answerInvite(i.id, accept: false) }.font(.system(size: 13)).foregroundColor(Theme.text2)
                    }
                    .padding(12).background(Theme.surface2).clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous)).padding(.horizontal, 16)
                }
            }
            LazyVStack(spacing: 0) {
                ForEach(app.playlists + app.sharedLists) { p in rowFor(p, sub: count(p)) }
            }
            if !app.followedLists.isEmpty {
                Text("Following").font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text2).padding(.horizontal, 16).padding(.top, 10)
                LazyVStack(spacing: 0) {
                    ForEach(app.followedLists) { p in rowFor(p, sub: "\(app.personName(p.owner)) · " + count(p)) }
                }
            }
        }
        .task { await app.loadShared() }
        .alert("New playlist", isPresented: $creating) {
            TextField("Name", text: $newName)
            Button("Create") { app.createPlaylist(newName); newName = "" }
            Button("Cancel", role: .cancel) { newName = "" }
        }
    }
}
