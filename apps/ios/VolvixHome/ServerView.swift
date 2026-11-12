import SwiftUI
import WebKit
import AVFoundation

/// Shows your Volvix Home server in a WKWebView.
///  - Stays on the server; it never opens Safari or navigates away on its own.
///  - Injects window.VOLVIX_NATIVE so the web app can offer "Switch server", which calls back here.
///  - Keeps its own sign-in; audio keeps playing in the background (set in VolvixHomeApp).
struct ServerView: UIViewRepresentable {
    let server: URL
    var onDisconnect: () -> Void

    func makeCoordinator() -> Coordinator { Coordinator(server: server, onDisconnect: onDisconnect) }

    func makeUIView(context: Context) -> WKWebView {
        let c = context.coordinator
        let config = WKWebViewConfiguration()
        config.allowsInlineMediaPlayback = true
        config.mediaTypesRequiringUserActionForPlayback = []
        config.websiteDataStore = .default()

        let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0"
        let inject = "window.VOLVIX_NATIVE = { app: 'ios', version: '\(version)' };"
        config.userContentController.addUserScript(WKUserScript(source: inject, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        // the web app posts { type: 'disconnect' } here to return to the connect screen
        config.userContentController.add(WeakHandler(c), name: "volvixhome")

        let web = WKWebView(frame: .zero, configuration: config)
        let bg = UIColor.volvixSurface
        web.isOpaque = false
        web.backgroundColor = bg
        web.scrollView.backgroundColor = bg
        web.scrollView.contentInsetAdjustmentBehavior = .never
        web.scrollView.bounces = false
        web.scrollView.minimumZoomScale = 1
        web.scrollView.maximumZoomScale = 1
        web.allowsBackForwardNavigationGestures = true
        web.navigationDelegate = c
        web.uiDelegate = c
        if #available(iOS 16.4, *) { web.isInspectable = true }
        c.webView = web
        web.load(URLRequest(url: server))
        return web
    }

    func updateUIView(_ uiView: WKWebView, context: Context) {}

    final class Coordinator: NSObject, WKNavigationDelegate, WKUIDelegate, WKScriptMessageHandler {
        let server: URL
        let onDisconnect: () -> Void
        weak var webView: WKWebView?

        private var observers: [NSObjectProtocol] = []

        init(server: URL, onDisconnect: @escaping () -> Void) {
            self.server = server
            self.onDisconnect = onDisconnect
            super.init()
            watchAudio()
        }

        deinit { observers.forEach { NotificationCenter.default.removeObserver($0) } }

        /// Tell the page's Playback log (Settings) what iOS is doing to the audio, to find out why it stops in the background.
        private func note(_ text: String) {
            let js = "window.Spin&&Spin.audioLog&&Spin.audioLog.add('native: \(text)')"
            DispatchQueue.main.async { [weak self] in self?.webView?.evaluateJavaScript(js, completionHandler: nil) }
        }

        private func watchAudio() {
            let nc = NotificationCenter.default
            let session = AVAudioSession.sharedInstance()
            observers.append(nc.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] n in
                let type = (n.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt) ?? 99
                let reason = (n.userInfo?[AVAudioSessionInterruptionReasonKey] as? UInt) ?? 99
                self?.note("audio interruption \(type == 1 ? "began" : "ended"), reason \(reason)")
            })
            observers.append(nc.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] n in
                let reason = (n.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt) ?? 99
                self?.note("audio route changed, reason \(reason)")
            })
            let events: [(Notification.Name, String)] = [
                (UIApplication.willResignActiveNotification, "app will resign active"),
                (UIApplication.didEnterBackgroundNotification, "app entered background"),
                (UIApplication.willEnterForegroundNotification, "app entering foreground"),
            ]
            for (name, text) in events {
                observers.append(nc.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                    self?.note("\(text); session \(session.category.rawValue), other audio playing: \(session.isOtherAudioPlaying)")
                })
            }
        }

        private func sameHost(_ url: URL?) -> Bool {
            guard let host = url?.host else { return false }
            return host == server.host
        }

        // MARK: keep navigation on the server; ignore links elsewhere (never open Safari)

        func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
                     decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
            let url = action.request.url
            let scheme = url?.scheme?.lowercased() ?? ""
            if sameHost(url) || scheme == "about" || scheme == "data" || scheme == "blob" {
                decisionHandler(.allow)
            } else {
                // a link off your server: do nothing (don't leave the app)
                decisionHandler(.cancel)
            }
        }

        // window.open / target=_blank: load it here only if it's on the server, else ignore
        func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
                     for action: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
            if sameHost(action.request.url) { webView.load(action.request) }
            return nil
        }

        // An app, not a web page: no pinch-zoom (the page also turns off double-tap zoom once it sees VOLVIX_NATIVE).
        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            webView.scrollView.pinchGestureRecognizer?.isEnabled = false
            webView.scrollView.minimumZoomScale = 1
            webView.scrollView.maximumZoomScale = 1
        }

        // If the server can't be reached, show a simple retry instead of a blank page.
        func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { showError(webView) }
        func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { showError(webView) }

        private func showError(_ webView: WKWebView) {
            let html = """
            <html><head><meta name='viewport' content='width=device-width, initial-scale=1'></head>
            <body style="margin:0;height:100vh;display:grid;place-items:center;background:#121214;color:#f4f4f6;font:16px -apple-system,sans-serif;text-align:center">
            <div style="padding:24px"><p style="font-weight:600">Can't reach your server</p>
            <p style="color:#a0a0ab;font-size:14px">Check that Volvix Home is on and you're on the same network.</p>
            <button onclick="location.href='\(server.absoluteString)'" style="margin-top:8px;padding:10px 18px;border:0;border-radius:99px;background:#e5484d;color:#fff;font:inherit;font-weight:600">Try again</button>
            <button onclick="window.webkit.messageHandlers.volvixhome.postMessage('disconnect')" style="margin-top:8px;padding:10px 18px;border:0;border-radius:99px;background:#2a2a30;color:#f4f4f6;font:inherit">Change server</button>
            </div></body></html>
            """
            webView.loadHTMLString(html, baseURL: server)
        }

        // MARK: messages from the page

        func userContentController(_ ucc: WKUserContentController, didReceive message: WKScriptMessage) {
            if (message.body as? String) == "disconnect" { onDisconnect() }
        }

        // alert() / confirm()
        func webView(_ webView: WKWebView, runJavaScriptAlertPanelWithMessage message: String,
                     initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping () -> Void) {
            present(message, cancel: false) { _ in completionHandler() }
        }
        func webView(_ webView: WKWebView, runJavaScriptConfirmPanelWithMessage message: String,
                     initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping (Bool) -> Void) {
            present(message, cancel: true, done: completionHandler)
        }
        private func present(_ message: String, cancel: Bool, done: @escaping (Bool) -> Void) {
            let alert = UIAlertController(title: nil, message: message, preferredStyle: .alert)
            if cancel { alert.addAction(UIAlertAction(title: "Cancel", style: .cancel) { _ in done(false) }) }
            alert.addAction(UIAlertAction(title: "OK", style: .default) { _ in done(true) })
            let root = UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.keyWindow }.first?.rootViewController
            root?.present(alert, animated: true) ?? done(false)
        }

        func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { webView.reload() }
    }
}

/// Avoids a retain cycle: WKUserContentController holds handlers strongly.
private final class WeakHandler: NSObject, WKScriptMessageHandler {
    weak var target: WKScriptMessageHandler?
    init(_ target: WKScriptMessageHandler) { self.target = target }
    func userContentController(_ ucc: WKUserContentController, didReceive message: WKScriptMessage) {
        target?.userContentController(ucc, didReceive: message)
    }
}
