import SwiftUI
import PhotosUI

struct Person: Identifiable, Hashable {
    var id: String
    var name: String
    var admin: Bool
    var avatar: Int
}

/// What the server lets everyone do (the names the server uses).
let permissionList: [(String, String, String)] = [
    ("request", "Request music", "Ask Lidarr to download music that isn't on the server yet."),
    ("rescan", "Check for new music", "Look in the music folder for added or removed songs right now."),
    ("parties", "Start listening parties", "Host a party and invite the household."),
    ("members", "Manage members", "Add people, change what they may do, set passwords and remove them."),
    ("settings", "Server settings", "Change the music folder and the Lidarr connection."),
    ("share", "Create share links", "Make a link to an album or playlist that anyone with the link can play for a few days."),
]

extension AppModel {
    func can(_ perm: String) -> Bool {
        guard let u = user else { return false }
        return u.admin || u.perms.contains(perm)
    }

    func avatarVersion(_ id: String) -> Int {
        if let p = people.first(where: { $0.id == id }) { return p.avatar }
        if id == user?.id { return user?.avatar ?? 0 }
        return 0
    }

    func personName(_ id: String) -> String {
        if id == user?.id || id == "u_me" { return user?.name ?? "You" }
        return people.first(where: { $0.id == id })?.name ?? members.first(where: { $0.id == id })?.name ?? "Someone"
    }

    /// Ask the server something and get its answer as a dictionary.
    @discardableResult
    func call(_ method: String, _ path: String, _ json: Any? = nil) async throws -> [String: Any] {
        guard let api = api else { throw ServerError(message: "Not connected.") }
        let data = try await api.send(try api.request(path, method: method, json: json))
        return (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:]
    }
}

extension API {
    func people() async -> [Person] {
        guard let data = try? await send(try request("/api/people")),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let list = o["people"] as? [[String: Any]] else { return [] }
        return list.map { Person(id: ($0["id"] as? String) ?? "", name: ($0["name"] as? String) ?? "", admin: ($0["admin"] as? Bool) ?? false, avatar: wholeNumber($0["avatar"])) }
    }

    func putImage(_ path: String, jpeg: Data) async throws -> [String: Any] {
        var req = try request(path, method: "PUT")
        req.httpBody = jpeg
        req.setValue("image/jpeg", forHTTPHeaderField: "Content-Type")
        let data = try await send(req)
        return (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:]
    }
}

// MARK: - Pictures of people

struct AvatarView: View {
    @EnvironmentObject var app: AppModel
    let id: String
    let name: String
    var size: CGFloat = 36
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            if let image = image {
                Image(uiImage: image).resizable().scaledToFill()
            } else {
                Circle().fill(Theme.tint(name))
                Text(initials(name)).font(.system(size: size * 0.38, weight: .bold)).foregroundColor(.white)
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        .task(id: "\(id)-\(app.avatarVersion(id))") {
            image = nil
            let v = app.avatarVersion(id)
            guard v > 0, let api = app.api, let url = api.url("/api/avatar/" + api.encode(id) + "?v=\(v)") else { return }
            image = await ImageLoader.shared.image(url, auth: api.authHeader)
        }
    }
}

/// Pick a spot and zoom on a photo; what is shown in the square is what is saved.
struct PictureCropper: View {
    let image: UIImage
    var out: CGFloat = 256
    var title = "Adjust your picture"
    var round = true
    let onSave: (Data) async -> String?
    @Environment(\.dismiss) private var dismiss
    @State private var zoom: CGFloat = 1
    @State private var offset: CGSize = .zero
    @State private var dragStart: CGSize = .zero
    @State private var pinchStart: CGFloat = 1
    @State private var saving = false
    @State private var problem: String?
    private let box: CGFloat = 300

    private var base: CGFloat { box / max(1, min(image.size.width, image.size.height)) }
    private var shown: CGSize { CGSize(width: image.size.width * base * zoom, height: image.size.height * base * zoom) }

    private func clamp(_ o: CGSize) -> CGSize {
        let mx = max(0, (shown.width - box) / 2), my = max(0, (shown.height - box) / 2)
        return CGSize(width: min(mx, max(-mx, o.width)), height: min(my, max(-my, o.height)))
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 18) {
                ZStack {
                    Image(uiImage: image).resizable().frame(width: shown.width, height: shown.height).offset(offset)
                    Rectangle().fill(Color.black.opacity(0.55)).reverseMask {
                        if round { Circle().frame(width: box - 6, height: box - 6) } else { Rectangle().frame(width: box - 6, height: box - 6) }
                    }
                    .allowsHitTesting(false)
                }
                .frame(width: box, height: box)
                .clipped()
                .contentShape(Rectangle())
                .gesture(DragGesture()
                    .onChanged { v in offset = clamp(CGSize(width: dragStart.width + v.translation.width, height: dragStart.height + v.translation.height)) }
                    .onEnded { _ in dragStart = offset })
                .gesture(MagnificationGesture()
                    .onChanged { v in zoom = max(1, min(4, pinchStart * v)); offset = clamp(offset) }
                    .onEnded { _ in pinchStart = zoom; dragStart = offset })
                HStack(spacing: 10) {
                    Image(systemName: "minus").foregroundColor(Theme.text2)
                    Slider(value: Binding(get: { Double(zoom) }, set: { zoom = CGFloat($0); pinchStart = zoom; offset = clamp(offset); dragStart = offset }), in: 1...4).tint(Theme.accent)
                    Image(systemName: "plus").foregroundColor(Theme.text2)
                }
                .padding(.horizontal, 30)
                Text("Drag to move. Pinch or use the slider to zoom.").font(.system(size: 13)).foregroundColor(Theme.text2)
                if let p = problem { ProblemBox(text: p).padding(.horizontal, 20) }
                Spacer()
            }
            .padding(.top, 20)
            .background(Theme.surface)
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button(saving ? "Saving…" : "Save") { save() }.disabled(saving)
                }
            }
        }
    }

    private func save() {
        // the part of the photo that is inside the square, in the photo's own points
        let f = image.size.width / shown.width
        let cropX = ((shown.width - box) / 2 - offset.width) * f
        let cropY = ((shown.height - box) / 2 - offset.height) * f
        let side = box * f
        let k = out / side
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        let rendered = UIGraphicsImageRenderer(size: CGSize(width: out, height: out), format: format).image { _ in
            image.draw(in: CGRect(x: -cropX * k, y: -cropY * k, width: image.size.width * k, height: image.size.height * k))
        }
        guard let jpeg = rendered.jpegData(compressionQuality: 0.9) else { problem = "Couldn't prepare that picture."; return }
        saving = true
        Task {
            if let err = await onSave(jpeg) {
                problem = err
                saving = false
            } else {
                dismiss()
            }
        }
    }
}

extension View {
    /// Cut a shape out of this view.
    func reverseMask<Mask: View>(@ViewBuilder _ mask: () -> Mask) -> some View {
        self.mask(
            Rectangle().overlay(mask().blendMode(.destinationOut)).compositingGroup()
        )
    }
}

// MARK: - Profile

struct ProfileView: View {
    @EnvironmentObject var app: AppModel
    @State private var name = ""
    @State private var picked: PhotosPickerItem?
    @State private var photo: UIImage?
    @State private var current = ""
    @State private var fresh = ""
    @State private var again = ""
    @State private var note: String?
    @State private var problem: String?
    @State private var busy = false

    private func field(_ title: String, _ text: Binding<String>, secure: Bool = false) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title).font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text)
            Group {
                if secure { SecureField(title, text: text) } else { TextField(title, text: text) }
            }
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .padding(12).background(Theme.surface3).foregroundColor(Theme.text)
            .clipShape(RoundedRectangle(cornerRadius: 10))
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                Text("Profile").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)
                HStack(spacing: 16) {
                    AvatarView(id: app.user?.id ?? "", name: app.user?.name ?? "", size: 84)
                    VStack(alignment: .leading, spacing: 10) {
                        PhotosPicker(selection: $picked, matching: .images) {
                            Text("Change picture").font(.system(size: 14, weight: .semibold))
                                .padding(.horizontal, 16).padding(.vertical, 8).background(Theme.surface3).foregroundColor(Theme.text).clipShape(Capsule())
                        }
                        if app.avatarVersion(app.user?.id ?? "") > 0 {
                            Button("Remove picture") { removePicture() }.font(.system(size: 14)).foregroundColor(Theme.accent)
                        }
                    }
                }
                if let n = note { Text(n).font(.system(size: 13)).foregroundColor(Theme.text2) }
                if let p = problem { ProblemBox(text: p) }

                VStack(alignment: .leading, spacing: 10) {
                    field("Name", $name)
                    PillButton(title: "Save name", primary: false) { saveName() }.disabled(busy || name.trimmingCharacters(in: .whitespaces).isEmpty || name == app.user?.name)
                }

                VStack(alignment: .leading, spacing: 10) {
                    Text("Change password").font(.system(size: 18, weight: .bold)).foregroundColor(Theme.text)
                    field("Current password", $current, secure: true)
                    field("New password", $fresh, secure: true)
                    field("New password again", $again, secure: true)
                    Text("At least 8 characters.").font(.system(size: 12)).foregroundColor(Theme.text2)
                    PillButton(title: "Change password", primary: false) { savePassword() }.disabled(busy || current.isEmpty || fresh.count < 8 || fresh != again)
                }
            }
            .padding(16)
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
        .onAppear { name = app.user?.name ?? "" }
        .onChange(of: picked) { item in
            guard let item = item else { return }
            Task {
                if let data = try? await item.loadTransferable(type: Data.self), let img = UIImage(data: data) { photo = img }
                picked = nil
            }
        }
        .sheet(item: Binding(get: { photo.map { PhotoBox(image: $0) } }, set: { if $0 == nil { photo = nil } })) { box in
            PictureCropper(image: box.image) { jpeg in
                do {
                    let r = try await app.api?.putImage("/api/me/avatar", jpeg: jpeg)
                    await app.reloadPeople()
                    if let v = r?["avatar"] { app.user?.avatar = wholeNumber(v) }
                    note = "Picture updated"
                    return nil
                } catch {
                    return error.localizedDescription
                }
            }
        }
    }

    private func saveName() {
        busy = true
        problem = nil
        Task {
            do {
                let r = try await app.call("PUT", "/api/me/profile", ["name": name])
                if let u = r["user"] as? [String: Any], let n = u["name"] as? String { app.user?.name = n }
                note = "Name saved"
                await app.pollOnce()
                await app.reloadPeople()
            } catch { problem = error.localizedDescription }
            busy = false
        }
    }

    private func savePassword() {
        busy = true
        problem = nil
        Task {
            do {
                let r = try await app.call("POST", "/api/me/password", ["current": current, "new": fresh])
                // the server signs you in again with a fresh key
                if let t = r["token"] as? String { app.replaceToken(t) }
                current = ""; fresh = ""; again = ""
                note = "Password changed"
            } catch { problem = error.localizedDescription }
            busy = false
        }
    }

    private func removePicture() {
        Task {
            _ = try? await app.call("DELETE", "/api/me/avatar")
            await app.reloadPeople()
            app.user?.avatar = 0
            note = "Picture removed"
        }
    }
}

struct PhotoBox: Identifiable {
    let id = UUID()
    let image: UIImage
}

// MARK: - Settings

struct SettingsView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var downloads: DownloadStore
    @State private var status: [String: Any]?
    @State private var scanning = false
    @State private var note: String?

    private func toggle(_ title: String, _ detail: String, _ value: Bool, _ set: @escaping (Bool) -> Void) -> some View {
        Toggle(isOn: Binding(get: { value }, set: set)) {
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                Text(detail).font(.system(size: 13)).foregroundColor(Theme.text2)
            }
        }
        .tint(Theme.accent)
    }

    private func group<C: View>(_ title: String, @ViewBuilder _ content: () -> C) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(title).font(.system(size: 20, weight: .bold)).foregroundColor(Theme.text)
            content()
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.surface2)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
    }

    var body: some View {
        let fx = app.fx
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                Text("Settings").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)

                group("Privacy") {
                    toggle("Share what I'm listening to", "Others in your household see your current song on the Household page.", fx.shareActivity) { v in app.updateSettings { $0["shareActivity"] = v } }
                    toggle("Private session", "Stops sharing, and keeps what you play out of your listening numbers, until you turn it off.", fx.privateSession) { v in app.updateSettings { $0["privateSession"] = v } }
                }

                group("Playback") {
                    toggle("Resume where I left off", "Opening Volvix shows the last song and spot, paused.", fx.resumeLast) { v in app.updateSettings { $0["resumeLast"] = v } }
                    toggle("Only play downloads", "Use only the songs saved on this phone. Everything else is greyed out and skipped. This also happens by itself when the server can't be reached.", fx.offlineOnly) { v in app.updateSettings { $0["offlineOnly"] = v } }
                }

                group("Downloads") {
                    toggle("Download on mobile data", "Off: downloads wait for Wi-Fi, so a big album can't use up your data plan.", fx.downloadMobile) { v in app.updateSettings { $0["downloadMobile"] = v } }
                    if app.me?.transcoding == true {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("Download quality").font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                            Text("Smaller downloads use less storage. \"Same as streaming\" follows the streaming quality. Songs already downloaded stay as they are.").font(.system(size: 13)).foregroundColor(Theme.text2)
                            Picker("", selection: Binding(get: { fx.downloadQuality }, set: { v in app.updateSettings { $0["downloadQuality"] = v } })) {
                                Text("Same").tag("stream"); Text("Original").tag("original"); Text("High").tag("high"); Text("Normal").tag("normal"); Text("Saver").tag("low")
                            }
                            .pickerStyle(.segmented)
                        }
                    }
                    HStack {
                        VStack(alignment: .leading, spacing: 3) {
                            Text("On this phone").font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                            Text("\(downloads.count) song\(downloads.count == 1 ? "" : "s") · \(formatBytes(downloads.bytes))").font(.system(size: 13)).foregroundColor(Theme.text2)
                        }
                        Spacer()
                        if downloads.count > 0 {
                            Button("Remove all") { downloads.removeAll() }.font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.accent)
                        }
                    }
                }

                group("Library") {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Music on this server").font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                        Text(statusText).font(.system(size: 13)).foregroundColor(Theme.text2)
                    }
                    if app.can("rescan") {
                        PillButton(title: scanning ? "Checking…" : "Check for new music", icon: "arrow.clockwise", primary: false) { rescan() }.disabled(scanning)
                    }
                    if let n = note { Text(n).font(.system(size: 13)).foregroundColor(Theme.text2) }
                    if app.can("settings") {
                        NavigationLink(value: Route.folder) { linkRow("folder", "Music folder", "Where the server looks for music") }.buttonStyle(.plain)
                        NavigationLink(value: Route.lidarr) { linkRow("arrow.down.circle", "Lidarr", "Get music you don't have yet") }.buttonStyle(.plain)
                    }
                }

                if app.can("share") || app.can("members") {
                    group("Share links") {
                        NavigationLink(value: Route.shares) { linkRow("link", "Open share links", "Cancel a link to stop it working straight away") }.buttonStyle(.plain)
                    }
                }

                group("About") {
                    HStack {
                        Text("Volvix Home").font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                        Spacer()
                        Text((status?["version"] as? String).map { "Version " + $0 } ?? "").font(.system(size: 13)).foregroundColor(Theme.text2)
                    }
                    Text(app.serverURL).font(.system(size: 13)).foregroundColor(Theme.text2)
                    PillButton(title: "Switch server", icon: "arrow.triangle.2.circlepath", primary: false) { app.switchServer() }
                }
            }
            .padding(16)
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
        .task { status = try? await app.call("GET", "/api/status") }
    }

    private var statusText: String {
        guard let s = status else { return "…" }
        let albums = wholeNumber(s["albums"]), tracks = wholeNumber(s["tracks"])
        var t = "\(albums) albums · \(tracks) songs"
        if (s["scanning"] as? Bool) == true { t += " · checking the folder…" }
        if (s["watching"] as? Bool) == true { t += "\nWatching the music folder: songs added or removed show up within seconds." }
        return t
    }

    private func linkRow(_ icon: String, _ title: String, _ sub: String) -> some View {
        HStack(spacing: 12) {
            Image(systemName: icon).font(.system(size: 17)).foregroundColor(Theme.accent).frame(width: 26)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                Text(sub).font(.system(size: 12)).foregroundColor(Theme.text2)
            }
            Spacer()
            Image(systemName: "chevron.right").font(.system(size: 13)).foregroundColor(Theme.text3)
        }
    }

    private func rescan() {
        scanning = true
        note = nil
        Task {
            _ = try? await app.call("POST", "/api/library/rescan")
            // wait for the server to finish, then bring the new library in
            for _ in 0..<60 {
                try? await Task.sleep(nanoseconds: 1_200_000_000)
                status = try? await app.call("GET", "/api/status")
                if (status?["scanning"] as? Bool) != true { break }
            }
            await app.refresh()
            note = "Library updated"
            scanning = false
        }
    }
}

// MARK: - Music folder and Lidarr (people who may change server settings)

struct FolderView: View {
    @EnvironmentObject var app: AppModel
    @State private var path = ""
    @State private var typed = ""
    @State private var parent: String?
    @State private var dirs: [(String, String)] = []
    @State private var songs = 0
    @State private var message: String?
    @State private var problem: String?
    @State private var saved = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Music folder").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)
                Text("Pick the folder that holds your music (the one Lidarr saves to), or type its path. Volvix looks inside it, including subfolders.").font(.system(size: 14)).foregroundColor(Theme.text2)
                HStack {
                    TextField("/path/to/music", text: $typed).textInputAutocapitalization(.never).autocorrectionDisabled()
                        .padding(10).background(Theme.surface3).foregroundColor(Theme.text).clipShape(RoundedRectangle(cornerRadius: 10))
                        .onSubmit { Task { await show(typed) } }
                    Button("Go") { Task { await show(typed) } }.font(.system(size: 15, weight: .semibold))
                }
                if let p = problem { ProblemBox(text: p) }
                if let m = message { Text(m).font(.system(size: 13)).foregroundColor(Theme.text2) }
                HStack(spacing: 8) {
                    Image(systemName: "folder.fill").foregroundColor(Theme.accent)
                    Text(path).font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text).lineLimit(2)
                }
                Text(songs > 0 ? "\(songs) song\(songs == 1 ? "" : "s") directly in this folder" : "No songs directly in this folder (subfolders count too)").font(.system(size: 13)).foregroundColor(Theme.text2)
                VStack(spacing: 0) {
                    if let p = parent {
                        Button { Task { await show(p) } } label: { row("arrow.up", "Up one level") }.buttonStyle(.plain)
                    }
                    ForEach(dirs, id: \.1) { d in
                        Button { Task { await show(d.1) } } label: { row("folder", d.0) }.buttonStyle(.plain)
                    }
                    if dirs.isEmpty { Text("No folders in here.").font(.system(size: 13)).foregroundColor(Theme.text2).padding(.vertical, 8) }
                }
                PillButton(title: saved ? "Saved" : "Use this folder", icon: saved ? "checkmark" : nil) { use() }
            }
            .padding(16)
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
        .task { await show("") }
    }

    private func row(_ icon: String, _ title: String) -> some View {
        HStack(spacing: 12) {
            Image(systemName: icon).foregroundColor(Theme.text2).frame(width: 24)
            Text(title).font(.system(size: 15)).foregroundColor(Theme.text).lineLimit(1)
            Spacer()
        }
        .padding(.vertical, 9)
        .contentShape(Rectangle())
    }

    private func show(_ p: String) async {
        let q = p.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? p
        do {
            let r = try await app.call("GET", "/api/library/browse" + (p.isEmpty ? "" : "?path=" + q))
            path = (r["path"] as? String) ?? ""
            typed = path
            parent = r["parent"] as? String
            dirs = ((r["dirs"] as? [[String: Any]]) ?? []).map { (($0["name"] as? String) ?? "", ($0["path"] as? String) ?? "") }
            songs = wholeNumber(r["songsHere"])
            problem = nil
            var notes: [String] = []
            if (r["denied"] as? Bool) == true { notes.append("Volvix isn't allowed to read this folder. The user it runs as needs read access.") }
            if (r["missing"] as? Bool) == true { notes.append("That folder doesn't exist on the server, so this is the closest one that does.") }
            if (r["docker"] as? Bool) == true { notes.append("Volvix is running in Docker, so it only sees folders you mounted into the container.") }
            message = notes.isEmpty ? nil : notes.joined(separator: " ")
            saved = false
        } catch { problem = error.localizedDescription }
    }

    private func use() {
        Task {
            do {
                let r = try await app.call("PUT", "/api/library/folder", ["path": path])
                message = "Using " + ((r["path"] as? String) ?? path) + ". Looking for music…"
                saved = true
                for _ in 0..<60 {
                    try? await Task.sleep(nanoseconds: 1_200_000_000)
                    let s = try? await app.call("GET", "/api/status")
                    if (s?["scanning"] as? Bool) != true { break }
                }
                await app.refresh()
                message = "Library updated"
            } catch { problem = error.localizedDescription }
        }
    }
}

struct LidarrSettingsView: View {
    @EnvironmentObject var app: AppModel
    @State private var loaded = false
    @State private var configured = false
    @State private var url = ""
    @State private var key = ""
    @State private var keySet = false
    @State private var rootFolder = ""
    @State private var quality = 0
    @State private var metadata = 0
    @State private var roots: [String] = []
    @State private var qualities: [(Int, String)] = []
    @State private var metadatas: [(Int, String)] = []
    @State private var version = ""
    @State private var problem: String?
    @State private var note: String?
    @State private var busy = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text("Lidarr").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)
                if !loaded {
                    ProgressView().tint(Theme.text2)
                } else {
                    if configured, problem == nil { Label("Connected to Lidarr \(version)", systemImage: "checkmark.circle.fill").font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text2) }
                    if !configured { Text("Connect Lidarr to search for music you don't have and add it to this server automatically.").font(.system(size: 14)).foregroundColor(Theme.text2) }
                    if let p = problem { ProblemBox(text: p) }
                    if let n = note { Text(n).font(.system(size: 13)).foregroundColor(Theme.text2) }
                    field("Lidarr address", $url, "http://lidarr:8686")
                    VStack(alignment: .leading, spacing: 5) {
                        Text("API key").font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text)
                        SecureField(keySet ? "Saved (leave empty to keep it)" : "Lidarr → Settings → General → API Key", text: $key)
                            .textInputAutocapitalization(.never).autocorrectionDisabled()
                            .padding(12).background(Theme.surface3).foregroundColor(Theme.text).clipShape(RoundedRectangle(cornerRadius: 10))
                    }
                    if configured && !roots.isEmpty {
                        picker("Add music to", selection: $rootFolder, options: roots.map { ($0, $0) })
                        picker("Quality profile", selection: Binding(get: { String(quality) }, set: { quality = Int($0) ?? 0 }), options: qualities.map { (String($0.0), $0.1) })
                        picker("Metadata profile", selection: Binding(get: { String(metadata) }, set: { metadata = Int($0) ?? 0 }), options: metadatas.map { (String($0.0), $0.1) })
                    }
                    HStack(spacing: 10) {
                        PillButton(title: busy ? "Saving…" : (configured ? "Save" : "Connect")) { save() }.disabled(busy || url.isEmpty)
                        if configured { PillButton(title: "Disconnect", primary: false) { disconnect() } }
                    }
                }
            }
            .padding(16)
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
        .task { await load() }
    }

    private func field(_ title: String, _ text: Binding<String>, _ hint: String) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title).font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text)
            TextField(hint, text: text).textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
                .padding(12).background(Theme.surface3).foregroundColor(Theme.text).clipShape(RoundedRectangle(cornerRadius: 10))
        }
    }

    private func picker(_ title: String, selection: Binding<String>, options: [(String, String)]) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title).font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text)
            Picker(title, selection: selection) { ForEach(options, id: \.0) { Text($0.1).tag($0.0) } }
                .pickerStyle(.menu).tint(Theme.text)
                .padding(.horizontal, 6).frame(maxWidth: .infinity, alignment: .leading)
                .background(Theme.surface3).clipShape(RoundedRectangle(cornerRadius: 10))
        }
    }

    private func apply(_ r: [String: Any]) {
        configured = (r["configured"] as? Bool) ?? false
        problem = r["error"] as? String
        if let s = r["settings"] as? [String: Any] {
            url = (s["url"] as? String) ?? ""
            keySet = (s["apiKeySet"] as? Bool) ?? false
            rootFolder = (s["rootFolder"] as? String) ?? ""
            quality = wholeNumber(s["qualityProfileId"])
            metadata = wholeNumber(s["metadataProfileId"])
        }
        if let l = r["lidarr"] as? [String: Any] {
            version = (l["version"] as? String) ?? ""
            roots = ((l["rootFolders"] as? [[String: Any]]) ?? []).compactMap { $0["path"] as? String }
            qualities = ((l["qualityProfiles"] as? [[String: Any]]) ?? []).map { (wholeNumber($0["id"]), ($0["name"] as? String) ?? "") }
            metadatas = ((l["metadataProfiles"] as? [[String: Any]]) ?? []).map { (wholeNumber($0["id"]), ($0["name"] as? String) ?? "") }
        }
        key = ""
    }

    private func load() async {
        if let r = try? await app.call("GET", "/api/lidarr") { apply(r) }
        loaded = true
    }

    private func save() {
        busy = true
        note = nil
        Task {
            do {
                let r = try await app.call("PUT", "/api/lidarr", ["url": url, "apiKey": key, "rootFolder": rootFolder, "qualityProfileId": quality, "metadataProfileId": metadata])
                apply(r)
                note = "Saved"
                if let m = try? await app.api?.meInfo() { app.me = m }
            } catch { problem = error.localizedDescription }
            busy = false
        }
    }

    private func disconnect() {
        Task {
            if let r = try? await app.call("PUT", "/api/lidarr", ["url": ""]) { apply(r) }
            url = ""
            if let m = try? await app.api?.meInfo() { app.me = m }
        }
    }
}

// MARK: - Members (people who may manage them)

struct ServerUser: Identifiable, Hashable {
    var id: String
    var name: String
    var admin: Bool
    var perms: [String]
    var hideExplicit: Bool

    init(_ o: [String: Any]) {
        id = (o["id"] as? String) ?? ""
        name = (o["name"] as? String) ?? ""
        admin = (o["admin"] as? Bool) ?? false
        perms = (o["perms"] as? [String]) ?? []
        hideExplicit = (o["hideExplicit"] as? Bool) ?? false
    }

    var role: String {
        if admin || perms.count == permissionList.count { return "Admin" }
        let basic = perms.count == 2 && perms.contains("request") && perms.contains("parties")
        return basic ? "Member" : "Member · custom permissions"
    }
}

struct MembersView: View {
    @EnvironmentObject var app: AppModel
    @State private var users: [ServerUser] = []
    @State private var loaded = false
    @State private var name = ""
    @State private var password = ""
    @State private var problem: String?
    @State private var busy = false
    @State private var managing: ServerUser?

    private func canManage(_ u: ServerUser) -> Bool {
        guard let me = app.user else { return false }
        return me.admin || (app.can("members") && !u.admin && u.perms.allSatisfy { app.can($0) })
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Members").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)
                Text("Everyone shares the same music. Playlists, play history and settings are personal.").font(.system(size: 14)).foregroundColor(Theme.text2)
                if !loaded { ProgressView().tint(Theme.text2) }
                ForEach(users) { u in
                    HStack(spacing: 12) {
                        AvatarView(id: u.id, name: u.name, size: 42)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(u.name + (u.id == app.user?.id ? " (you)" : "")).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                            Text(u.role + (u.hideExplicit ? " · Explicit off" : "")).font(.system(size: 12)).foregroundColor(Theme.text2)
                        }
                        Spacer()
                        if u.id != app.user?.id && canManage(u) {
                            Button("Manage") { managing = u }.font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.accent)
                        }
                    }
                    .padding(12).background(Theme.surface2).clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                }
                Text("Add a member").font(.system(size: 20, weight: .bold)).foregroundColor(Theme.text).padding(.top, 8)
                VStack(alignment: .leading, spacing: 10) {
                    TextField("Name (they sign in with this)", text: $name).textInputAutocapitalization(.never).autocorrectionDisabled()
                        .padding(12).background(Theme.surface3).foregroundColor(Theme.text).clipShape(RoundedRectangle(cornerRadius: 10))
                    SecureField("Password (at least 8 characters)", text: $password)
                        .padding(12).background(Theme.surface3).foregroundColor(Theme.text).clipShape(RoundedRectangle(cornerRadius: 10))
                    Text("New members can request music and start parties. Change that any time under Manage.").font(.system(size: 12)).foregroundColor(Theme.text2)
                    if let p = problem { ProblemBox(text: p) }
                    PillButton(title: busy ? "Adding…" : "Add to household", icon: "plus") { add() }.disabled(busy || name.trimmingCharacters(in: .whitespaces).isEmpty || password.count < 8)
                }
            }
            .padding(16)
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
        .task { await load() }
        .sheet(item: $managing) { u in
            ManageMemberSheet(user: u) { await load() }.environmentObject(app)
        }
    }

    private func load() async {
        if let r = try? await app.call("GET", "/api/users") {
            users = ((r["users"] as? [[String: Any]]) ?? []).map { ServerUser($0) }
        }
        loaded = true
        await app.reloadPeople()
    }

    private func add() {
        busy = true
        problem = nil
        Task {
            do {
                try await app.call("POST", "/api/users", ["name": name, "password": password])
                name = ""; password = ""
                await load()
                await app.pollOnce()
            } catch { problem = error.localizedDescription }
            busy = false
        }
    }
}

struct ManageMemberSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    @State var user: ServerUser
    let changed: () async -> Void
    @State private var password = ""
    @State private var problem: String?
    @State private var note: String?
    @State private var confirmRemove = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    HStack(spacing: 12) {
                        AvatarView(id: user.id, name: user.name, size: 52)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(user.name).font(.system(size: 18, weight: .bold)).foregroundColor(Theme.text)
                            Text(user.role).font(.system(size: 13)).foregroundColor(Theme.text2)
                        }
                    }
                    if let p = problem { ProblemBox(text: p) }
                    if let n = note { Text(n).font(.system(size: 13)).foregroundColor(Theme.text2) }
                    Toggle(isOn: Binding(get: { user.hideExplicit }, set: { v in save(["hideExplicit": v]) })) {
                        VStack(alignment: .leading, spacing: 3) {
                            Text("Turn off explicit music").font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                            Text("Songs marked explicit stay listed but are skipped for them.").font(.system(size: 13)).foregroundColor(Theme.text2)
                        }
                    }
                    .tint(Theme.accent)
                    Text("What \(user.name) can do").font(.system(size: 12, weight: .bold)).foregroundColor(Theme.text2).textCase(.uppercase)
                    ForEach(permissionList, id: \.0) { p in
                        Toggle(isOn: Binding(get: { user.perms.contains(p.0) }, set: { on in
                            var list = user.perms.filter { $0 != p.0 }
                            if on { list.append(p.0) }
                            save(["perms": list])
                        })) {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(p.1).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                                Text(p.2 + (app.can(p.0) ? "" : " (you don't have this one, so you can't give it)")).font(.system(size: 12)).foregroundColor(Theme.text2)
                            }
                        }
                        .tint(Theme.accent)
                        .disabled(!app.can(p.0))
                    }
                    HStack(spacing: 10) {
                        PillButton(title: "Everything (admin)", primary: false) { save(["perms": permissionList.map { $0.0 }]) }.disabled(app.user?.admin != true)
                        PillButton(title: "Basics only", primary: false) { save(["perms": ["request", "parties"]]) }
                    }
                    Text("New password").font(.system(size: 12, weight: .bold)).foregroundColor(Theme.text2).textCase(.uppercase).padding(.top, 6)
                    SecureField("At least 8 characters", text: $password)
                        .padding(12).background(Theme.surface3).foregroundColor(Theme.text).clipShape(RoundedRectangle(cornerRadius: 10))
                    PillButton(title: "Set new password", primary: false) { setPassword() }.disabled(password.count < 8)
                    Text("They're signed out everywhere and use this password next time.").font(.system(size: 12)).foregroundColor(Theme.text2)
                    Button(role: .destructive) { confirmRemove = true } label: {
                        Label("Remove \(user.name)", systemImage: "xmark").font(.system(size: 15, weight: .semibold)).frame(maxWidth: .infinity).padding(.vertical, 12)
                    }
                    .padding(.top, 10)
                }
                .padding(16)
            }
            .background(Theme.surface)
            .navigationTitle("Manage")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
            .confirmationDialog("Remove \(user.name)?", isPresented: $confirmRemove, titleVisibility: .visible) {
                Button("Remove", role: .destructive) { remove() }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("They won't be able to sign in, and their playlists and history are deleted. The music stays.")
            }
        }
    }

    private func save(_ body: [String: Any]) {
        Task {
            do {
                let r = try await app.call("PUT", "/api/users/" + (app.api?.encode(user.id) ?? user.id), body)
                if let u = r["user"] as? [String: Any] { user = ServerUser(u) }
                problem = nil
                note = "Saved"
                await changed()
            } catch { problem = error.localizedDescription }
        }
    }

    private func setPassword() {
        Task {
            do {
                try await app.call("POST", "/api/users/" + (app.api?.encode(user.id) ?? user.id) + "/password", ["password": password])
                password = ""
                problem = nil
                note = "New password set for \(user.name)"
            } catch { problem = error.localizedDescription }
        }
    }

    private func remove() {
        Task {
            do {
                try await app.call("DELETE", "/api/users/" + (app.api?.encode(user.id) ?? user.id))
                await changed()
                dismiss()
            } catch { problem = error.localizedDescription }
        }
    }
}

// MARK: - Share links

/// Make a link to an album or playlist that anyone can play for a while.
struct ShareLinkSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    let kind: String
    let name: String
    let trackIDs: [String]
    @State private var days = 7
    @State private var link: String?
    @State private var problem: String?
    @State private var busy = false

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                Text("Anyone with the link can play \(name) (\(trackIDs.count) song\(trackIDs.count == 1 ? "" : "s")) in their browser. They can't see anything else on your server, and you can cancel the link any time in Settings.")
                    .font(.system(size: 14)).foregroundColor(Theme.text2)
                Picker("", selection: $days) {
                    Text("1 day").tag(1); Text("7 days").tag(7); Text("30 days").tag(30)
                }
                .pickerStyle(.segmented)
                .disabled(link != nil)
                if let p = problem { ProblemBox(text: p) }
                if let l = link {
                    Text(l).font(.system(size: 13)).foregroundColor(Theme.text).textSelection(.enabled)
                        .padding(12).frame(maxWidth: .infinity, alignment: .leading).background(Theme.surface3).clipShape(RoundedRectangle(cornerRadius: 10))
                    if let u = URL(string: l) {
                        ShareLink(item: u) { Label("Share link", systemImage: "square.and.arrow.up").font(.system(size: 15, weight: .semibold)).padding(.horizontal, 22).padding(.vertical, 11).background(Theme.accent).foregroundColor(.white).clipShape(Capsule()) }
                    }
                    PillButton(title: "Copy link", icon: "doc.on.doc", primary: false) { UIPasteboard.general.string = l }
                } else {
                    PillButton(title: busy ? "Creating…" : "Create link", icon: "link") { create() }.disabled(busy)
                }
                Spacer()
            }
            .padding(20)
            .background(Theme.surface)
            .navigationTitle("Share \(kind == "album" ? "this album" : "this playlist")")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
        .presentationDetents([.medium, .large])
    }

    private func create() {
        busy = true
        Task {
            do {
                let r = try await app.call("POST", "/api/shares", ["kind": kind, "name": name, "tracks": trackIDs, "days": days])
                if let s = r["share"] as? [String: Any], let p = s["path"] as? String {
                    link = app.serverURL.trimmingCharacters(in: CharacterSet(charactersIn: "/")) + p
                }
            } catch { problem = error.localizedDescription }
            busy = false
        }
    }
}

struct SharesView: View {
    @EnvironmentObject var app: AppModel
    @State private var shares: [[String: Any]] = []
    @State private var loaded = false

    private func left(_ expires: Int) -> String {
        let d = max(0, Double(expires) - Date().timeIntervalSince1970)
        let days = Int((d / 86400).rounded())
        if d / 86400 >= 1 { return "\(days) day\(days == 1 ? "" : "s") left" }
        return "\(max(1, Int((d / 3600).rounded()))) hours left"
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Share links").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)
                Text("Open links, newest first. Cancelling one stops it working straight away.").font(.system(size: 14)).foregroundColor(Theme.text2)
                if loaded && shares.isEmpty {
                    Text("No links are open. Share an album or playlist from its ⋯ menu.").font(.system(size: 14)).foregroundColor(Theme.text2)
                }
                ForEach(Array(shares.enumerated()), id: \.offset) { _, s in
                    let token = (s["token"] as? String) ?? ""
                    let path = (s["path"] as? String) ?? ""
                    let link = app.serverURL.trimmingCharacters(in: CharacterSet(charactersIn: "/")) + path
                    VStack(alignment: .leading, spacing: 8) {
                        Text((s["name"] as? String) ?? "").font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                        Text("\((s["kind"] as? String) ?? "") · \(wholeNumber(s["count"])) songs · by \((s["by"] as? String) ?? "") · \(left(wholeNumber(s["expires"])))")
                            .font(.system(size: 12)).foregroundColor(Theme.text2)
                        HStack(spacing: 14) {
                            Button("Copy link") { UIPasteboard.general.string = link }.font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.accent)
                            Button("Cancel link") { cancel(token) }.font(.system(size: 14)).foregroundColor(Theme.text2)
                        }
                    }
                    .padding(12).frame(maxWidth: .infinity, alignment: .leading).background(Theme.surface2).clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                }
            }
            .padding(16)
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
        .task { await load() }
    }

    private func load() async {
        if let r = try? await app.call("GET", "/api/shares") { shares = (r["shares"] as? [[String: Any]]) ?? [] }
        loaded = true
    }

    private func cancel(_ token: String) {
        Task {
            try? await app.call("DELETE", "/api/shares/" + token)
            await load()
        }
    }
}

// MARK: - Browse: your library by genre, decade and label

struct BrowseSections: View {
    @EnvironmentObject var app: AppModel
    private let palette: [UInt32] = [0xE5484D, 0x5E548E, 0x335C67, 0xBC6C25, 0x3D5A80, 0x4A6C2F, 0x7A4069, 0x9E2A2B]

    private func groups(_ kind: String) -> [(String, Int)] {
        var map: [String: Int] = [:]
        for a in app.albums {
            if let v = browseValue(a, kind) { map[v, default: 0] += 1 }
        }
        if kind == "decade" { return map.sorted { (Int($0.key.dropLast()) ?? 0) > (Int($1.key.dropLast()) ?? 0) }.map { ($0.key, $0.value) } }
        return map.sorted { $0.value != $1.value ? $0.value > $1.value : $0.key < $1.key }.map { ($0.key, $0.value) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            let any = ["genre", "decade", "label"].contains { !groups($0).isEmpty }
            if !any {
                Text("Genres, years and labels come from your files' tags. Nothing to browse yet.").font(.system(size: 15)).foregroundColor(Theme.text2).padding(.horizontal, 16)
            }
            ForEach(["genre", "decade", "label"], id: \.self) { kind in
                let g = groups(kind)
                if !g.isEmpty {
                    SectionTitle(text: kind == "genre" ? "Genres" : (kind == "decade" ? "Decades" : "Labels"))
                    LazyVGrid(columns: [GridItem(.flexible(), spacing: 10), GridItem(.flexible(), spacing: 10)], spacing: 10) {
                        ForEach(Array(g.prefix(kind == "label" ? 24 : 40).enumerated()), id: \.offset) { i, x in
                            NavigationLink(value: Route.browse(kind, x.0)) {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(x.0).font(.system(size: 16, weight: .bold)).foregroundColor(.white).lineLimit(2)
                                    Text("\(x.1) album\(x.1 == 1 ? "" : "s")").font(.system(size: 12)).foregroundColor(.white.opacity(0.85))
                                }
                                .frame(maxWidth: .infinity, minHeight: 72, alignment: .topLeading)
                                .padding(12)
                                .background(Color(hex: palette[i % palette.count]))
                                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.horizontal, 16)
                }
            }
        }
    }
}

func browseValue(_ a: Album, _ kind: String) -> String? {
    switch kind {
    case "genre": return (a.genre?.isEmpty == false) ? a.genre : nil
    case "decade": return a.year.map { String(($0 / 10) * 10) + "s" }
    default: return (a.label?.isEmpty == false) ? a.label : nil
    }
}

struct BrowseView: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    let kind: String
    let value: String

    var body: some View {
        let albums = app.albums.filter { browseValue($0, kind) == value }.sorted { ($0.year ?? 0) > ($1.year ?? 0) }
        let ids = albums.flatMap { $0.trackIds }
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(kind.capitalized).font(.system(size: 13)).foregroundColor(Theme.text2)
                    Text(value).font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)
                    Text("\(albums.count) album\(albums.count == 1 ? "" : "s") · \(ids.count) songs").font(.system(size: 13)).foregroundColor(Theme.text2)
                }
                .padding(.horizontal, 16)
                if albums.isEmpty {
                    Text("No albums with that \(kind).").font(.system(size: 15)).foregroundColor(Theme.text2).padding(.horizontal, 16)
                } else {
                    HStack(spacing: 10) {
                        PillButton(title: "Play", icon: "play.fill") { player.play(ids) }
                        PillButton(title: "Shuffle", icon: "shuffle", primary: false) { player.play(ids, shuffled: true) }
                    }
                    .padding(.horizontal, 16)
                    LazyVGrid(columns: [GridItem(.flexible(), spacing: 14), GridItem(.flexible(), spacing: 14)], spacing: 18) {
                        ForEach(albums) { a in
                            NavigationLink(value: Route.album(a.id)) {
                                VStack(alignment: .leading, spacing: 6) {
                                    CoverView(albumID: a.id)
                                    Text(a.title).font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text).lineLimit(1)
                                    Text(a.artist + (a.year.map { " · \($0)" } ?? "")).font(.system(size: 12)).foregroundColor(Theme.text2).lineLimit(1)
                                }
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.horizontal, 16)
                }
            }
            .padding(.top, 8).padding(.bottom, 24)
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
    }
}
