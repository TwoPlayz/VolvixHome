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
                ProgressView().tint(Theme.text2)
                Text("Loading your library…").font(.system(size: 14)).foregroundColor(Theme.text2)
            }
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.surface.ignoresSafeArea())
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
                        if app.busy { ProgressView().tint(Theme.text2) }
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
        .background(Theme.surface.ignoresSafeArea())
        .onAppear { field = 0 }
    }

    private func submit() {
        if name.isEmpty || password.isEmpty { return }
        Task { await app.signIn(name: name, password: password) }
    }
}

extension View {
    /// Where every Route leads (used by each tab, and by sheets that can open pages).
    func routeDestinations() -> some View {
        navigationDestination(for: Route.self) { route in
            Group { switch route {
            case .album(let id): AlbumView(id: id)
            case .artist(let name): ArtistView(name: name)
            case .playlist(let id): PlaylistView(id: id)
            case .notifications: NotificationsView()
            case .account: AccountView()
            case .discoverAlbum(let id): DiscoverAlbumView(id: id)
            case .discoverArtist(let id): DiscoverArtistView(id: id)
            case .mix(let id): MixView(id: id)
            case .rewind(let year): RewindView(year: year)
            case .credit(let name): CreditView(name: name)
            case .playback: PlaybackSettingsView()
            case .profile: AccountHub(tab: .profile)
            case .settings: AccountHub(tab: .settings)
            case .members: AccountHub(tab: .members)
            case .folder: FolderView()
            case .lidarr: LidarrSettingsView()
            case .shares: SharesView()
            case .browse(let kind, let value): BrowseView(kind: kind, value: value)
            } }
            .modifier(CustomBack())
        }
    }
}

/// The app's screens: Home, Search, Library, Account, with the mini player above the tab bar.
struct MainTabs: View {
    @EnvironmentObject var app: AppModel
    @EnvironmentObject var player: PlayerModel
    @EnvironmentObject var downloads: DownloadStore
    @EnvironmentObject var devices: DeviceHub
    @EnvironmentObject var casting: CastHub
    @EnvironmentObject var party: PartyHub
    @State private var showFull = false
    @State private var selected = 0
    @State private var paths: [NavigationPath] = Array(repeating: NavigationPath(), count: 5)

    var body: some View {
        TabView(selection: Binding(get: { selected }, set: { new in
            if new == selected { tappedAgain(new) }
            selected = new
        })) {
            tab(HomeView(), 0).tabItem { Label("Home", systemImage: "house.fill") }.tag(0)
            tab(SearchView(), 1).tabItem { Label("Search", systemImage: "magnifyingglass") }.tag(1)
            tab(LibraryView(), 2).tabItem { Label("Library", systemImage: "books.vertical.fill") }.tag(2)
            tab(HouseholdView(), 3).tabItem { Label("Household", systemImage: "person.2.fill") }.tag(3)
            tab(RequestsView(), 4).tabItem { Label("Requests", systemImage: "arrow.down.circle") }.tag(4)
        }
        .onChange(of: app.openRoute) { route in
            guard let r = route else { return }
            showFull = false
            paths[selected].append(r)
            app.openRoute = nil
        }
        .tint(Theme.accent)
        .overlay {
            ZStack {
                if showFull {
                    NowPlayingView(onClose: { showFull = false })
                        .environmentObject(app).environmentObject(player).environmentObject(downloads).environmentObject(devices).environmentObject(casting).environmentObject(party)
                        .transition(.move(edge: .bottom))
                        .zIndex(1)
                }
            }
            .animation(.easeInOut(duration: 0.3), value: showFull)
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
            party.attach(app, player: player)
        }
        .overlay(alignment: .top) {
            if !showFull, let n = devices.notice ?? casting.notice ?? party.notice {
                Text(n).font(.system(size: 13, weight: .semibold)).foregroundColor(.white)
                    .padding(.horizontal, 14).padding(.vertical, 8).background(Theme.surface3).clipShape(Capsule())
                    .padding(.top, 8)
                    .task { try? await Task.sleep(nanoseconds: 2_500_000_000); devices.notice = nil; casting.notice = nil; party.notice = nil }
            }
        }
        .task { await app.pollLoop() }
    }

    /// The tab you are already on was tapped: back to its first page, then to the top of the list.
    private func tappedAgain(_ i: Int) {
        if !paths[i].isEmpty {
            paths[i] = NavigationPath()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { scrollActiveScrollViewToTop() }
        } else {
            scrollActiveScrollViewToTop()
        }
    }

    private func tab<Content: View>(_ content: Content, _ i: Int) -> some View {
        NavigationStack(path: $paths[i]) {
            content.routeDestinations()
        }
        .safeAreaInset(edge: .bottom, spacing: 0) { MiniPlayer(showFull: $showFull) }
    }
}
