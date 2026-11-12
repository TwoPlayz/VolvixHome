import SwiftUI

/// Chooses what to show: connect, sign in, loading, or the app itself.
struct RootView: View {
    @EnvironmentObject var app: AppModel

    var body: some View {
        Group {
            switch app.phase {
            case .connect:
                ConnectView { url in app.connect(to: url) }
            case .signIn:
                SignInView()
            case .loading:
                LoadingView()
            case .ready:
                MainTabs()
            }
        }
        .background(Theme.surface.ignoresSafeArea())
        .task { await app.start() }
    }
}

struct LoadingView: View {
    @EnvironmentObject var app: AppModel

    var body: some View {
        VStack(spacing: 16) {
            RecordMark().frame(width: 56, height: 56)
            if let m = app.message {
                ProblemBox(text: m).frame(maxWidth: 360)
                PillButton(title: "Try again") { Task { await app.load() } }
                PillButton(title: "Switch server", primary: false) { app.switchServer() }
            } else {
                ProgressView().tint(.white)
                Text("Loading your library…").font(.system(size: 14)).foregroundColor(Theme.text2)
            }
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.surface)
    }
}

struct SignInView: View {
    @EnvironmentObject var app: AppModel
    @State private var name = ""
    @State private var password = ""
    @FocusState private var field: Int?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                HStack(spacing: 10) {
                    RecordMark().frame(width: 30, height: 30)
                    Text("Volvix").font(.system(size: 19, weight: .heavy)).foregroundColor(Theme.text)
                }
                Text("Sign in").font(.system(size: 26, weight: .bold)).foregroundColor(Theme.text)
                Text(app.serverURL).font(.system(size: 13)).foregroundColor(Theme.text2)

                Text("Name").font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text).padding(.top, 8)
                TextField("Your name", text: $name)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .focused($field, equals: 0)
                    .submitLabel(.next)
                    .onSubmit { field = 1 }
                    .padding(12)
                    .background(Theme.surface3)
                    .foregroundColor(Theme.text)
                    .clipShape(RoundedRectangle(cornerRadius: 10))

                Text("Password").font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text)
                SecureField("Password", text: $password)
                    .focused($field, equals: 1)
                    .submitLabel(.go)
                    .onSubmit(submit)
                    .padding(12)
                    .background(Theme.surface3)
                    .foregroundColor(Theme.text)
                    .clipShape(RoundedRectangle(cornerRadius: 10))

                if let m = app.message { ProblemBox(text: m) }

                Button(action: submit) {
                    HStack {
                        if app.busy { ProgressView().tint(.white) }
                        Text(app.busy ? "Signing in…" : "Sign in").font(.system(size: 16, weight: .semibold))
                    }
                    .frame(maxWidth: .infinity).padding(.vertical, 13)
                    .background(Theme.accent)
                    .foregroundColor(.white)
                    .clipShape(Capsule())
                }
                .disabled(app.busy || name.isEmpty || password.isEmpty)
                .padding(.top, 6)

                Button("Use a different server") { app.switchServer() }
                    .font(.system(size: 14)).foregroundColor(Theme.text2)
                    .frame(maxWidth: .infinity)
                    .padding(.top, 8)
            }
            .padding(28)
            .frame(maxWidth: 440)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.surface)
        .onAppear { field = 0 }
    }

    private func submit() {
        if name.isEmpty || password.isEmpty { return }
        Task { await app.signIn(name: name, password: password) }
    }
}

/// The app's screens: Home, Search, Library, Account, with the mini player above the tab bar.
struct MainTabs: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @EnvironmentObject var downloads: DownloadStore
    @EnvironmentObject var devices: DeviceHub
    @EnvironmentObject var casting: CastHub
    @State private var showFull = false

    var body: some View {
        TabView {
            tab(HomeView()).tabItem { Label("Home", systemImage: "house.fill") }
            tab(SearchView()).tabItem { Label("Search", systemImage: "magnifyingglass") }
            tab(LibraryView()).tabItem { Label("Library", systemImage: "books.vertical.fill") }
            tab(HouseholdView()).tabItem { Label("Household", systemImage: "person.2.fill") }
            tab(RequestsView()).tabItem { Label("Requests", systemImage: "arrow.down.circle") }
        }
        .tint(Theme.accent)
        .fullScreenCover(isPresented: $showFull) {
            NowPlayingView().environmentObject(app).environmentObject(player).environmentObject(downloads).environmentObject(devices).environmentObject(casting)
        }
        .safeAreaInset(edge: .top, spacing: 0) {
            if app.offline {
                Text("Offline: the server can't be reached. Downloaded music still plays.")
                    .font(.system(size: 12, weight: .semibold)).foregroundColor(.white)
                    .frame(maxWidth: .infinity).padding(.vertical, 5).background(Theme.accent)
            }
        }
        .onAppear {
            downloads.attach(app)
            player.attach(app, downloads: downloads)
            devices.attach(app, player: player)
            casting.attach(app, player: player)
        }
        .overlay(alignment: .top) {
            if !showFull, let n = devices.notice ?? casting.notice {
                Text(n).font(.system(size: 13, weight: .semibold)).foregroundColor(.white)
                    .padding(.horizontal, 14).padding(.vertical, 8).background(Theme.surface3).clipShape(Capsule())
                    .padding(.top, 8)
                    .task { try? await Task.sleep(nanoseconds: 2_500_000_000); devices.notice = nil; casting.notice = nil }
            }
        }
        .task { await app.pollLoop() }
    }

    private func tab<Content: View>(_ content: Content) -> some View {
        NavigationStack {
            content
                .navigationDestination(for: Route.self) { route in
                    switch route {
                    case .album(let id): AlbumView(id: id)
                    case .artist(let name): ArtistView(name: name)
                    case .playlist(let id): PlaylistView(id: id)
                    case .notifications: NotificationsView()
                    case .account: AccountView()
                    case .discoverAlbum(let id): DiscoverAlbumView(id: id)
                    case .discoverArtist(let id): DiscoverArtistView(id: id)
                    }
                }
        }
        .safeAreaInset(edge: .bottom, spacing: 0) { MiniPlayer(showFull: $showFull) }
    }
}
