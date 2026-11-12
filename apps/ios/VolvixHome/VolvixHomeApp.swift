import SwiftUI
import AVFoundation

/// Volvix Home for iPhone: a native app for your own Volvix Home server.
/// First launch asks for the server address, then your name and password; after that it opens straight to your library.
@main
struct VolvixHomeApp: App {
    @StateObject private var app = AppModel()
    @StateObject private var player = PlayerModel()
    @StateObject private var downloads = DownloadStore()
    @StateObject private var devices = DeviceHub()
    @StateObject private var casting = CastHub()

    init() {
        // sound keeps playing when the app is in the background or the screen locks
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .default)
        try? AVAudioSession.sharedInstance().setActive(true)
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(app)
                .environmentObject(player)
                .environmentObject(downloads)
                .environmentObject(devices)
                .environmentObject(casting)
                .background(Color(UIColor.volvixSurface).ignoresSafeArea())
                .preferredColorScheme(.dark)
        }
    }
}

extension UIColor {
    /// The web app's surface colour (--surface), shown behind the status bar and while loading.
    static let volvixSurface = UIColor(red: 18 / 255, green: 18 / 255, blue: 20 / 255, alpha: 1)
}
