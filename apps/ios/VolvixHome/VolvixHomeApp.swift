import SwiftUI
import AVFoundation

/// Volvix Home for iPhone: a native window onto your own Volvix Home server.
/// First launch asks for the server address; after that it opens straight to your library.
@main
struct VolvixHomeApp: App {
    @StateObject private var state = AppState()

    init() {
        // allow audio to keep playing when the app is backgrounded or the screen locks
        Self.activateAudio()
        let nc = NotificationCenter.default
        // take the audio session back after a call, Siri or anything else that interrupted it, and when leaving the app
        nc.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { n in
            if let v = n.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt, AVAudioSession.InterruptionType(rawValue: v) == .ended { Self.activateAudio() }
        }
        nc.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { _ in Self.activateAudio() }
        nc.addObserver(forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main) { _ in Self.activateAudio() }
    }

    private static func activateAudio() {
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .default)
        try? AVAudioSession.sharedInstance().setActive(true)
    }

    var body: some Scene {
        WindowGroup {
            Group {
                if let server = state.server {
                    ServerView(server: server, onDisconnect: state.clear)
                        .ignoresSafeArea(.container, edges: .all) // the page fills the whole screen; its CSS keeps clear of the notch and the home bar itself
                } else {
                    ConnectView { state.connect($0) }
                }
            }
            .background(Color(UIColor.volvixSurface).ignoresSafeArea())
            .preferredColorScheme(.dark)
        }
    }
}

/// The saved server address (UserDefaults), shared across the app.
final class AppState: ObservableObject {
    private static let key = "volvix.home.server"
    @Published var server: URL?

    init() {
        if let s = UserDefaults.standard.string(forKey: Self.key), let u = URL(string: s) {
            server = u
        }
    }
    func connect(_ url: URL) {
        UserDefaults.standard.set(url.absoluteString, forKey: Self.key)
        server = url
    }
    func clear() {
        UserDefaults.standard.removeObject(forKey: Self.key)
        server = nil
    }
}

extension UIColor {
    /// The web app's surface colour (--surface), shown behind the status bar and while loading.
    static let volvixSurface = UIColor(red: 18 / 255, green: 18 / 255, blue: 20 / 255, alpha: 1)
}
