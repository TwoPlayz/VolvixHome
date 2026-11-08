import SwiftUI

/// First-run screen: enter the Volvix Home server address. Validates it by reaching /api/status.
struct ConnectView: View {
    var onConnect: (URL) -> Void

    @State private var address = ""
    @State private var checking = false
    @State private var error: String?
    @FocusState private var focused: Bool

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                HStack(spacing: 10) {
                    RecordMark().frame(width: 30, height: 30)
                    Text("Volvix Home").font(.system(size: 19, weight: .heavy))
                    Text("APP").font(.system(size: 11, weight: .bold))
                        .padding(.horizontal, 6).padding(.vertical, 2)
                        .background(Color(red: 0.9, green: 0.28, blue: 0.3).opacity(0.16))
                        .foregroundColor(Color(red: 0.9, green: 0.28, blue: 0.3))
                        .clipShape(Capsule())
                }
                .padding(.bottom, 4)

                Text("Connect to your server").font(.system(size: 24, weight: .bold))
                Text("Enter the address of your Volvix Home server. Ask whoever set it up if you're not sure.")
                    .font(.system(size: 15)).foregroundColor(.secondary)

                Text("Server address").font(.system(size: 13, weight: .semibold)).padding(.top, 8)
                TextField("http://192.168.1.10:4747", text: $address)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.URL)
                    .focused($focused)
                    .submitLabel(.go)
                    .onSubmit(connect)
                    .padding(12)
                    .background(Color(white: 0.08))
                    .clipShape(RoundedRectangle(cornerRadius: 10))
                    .overlay(RoundedRectangle(cornerRadius: 10).stroke(Color.white.opacity(0.12)))

                Text("For example http://volvix.lan:4747, or a web address if it's set up for the internet.")
                    .font(.system(size: 12)).foregroundColor(Color(white: 0.45))

                if let error {
                    Text(error).font(.system(size: 13))
                        .foregroundColor(Color(red: 1, green: 0.7, blue: 0.7))
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(Color(red: 0.9, green: 0.28, blue: 0.3).opacity(0.14))
                        .clipShape(RoundedRectangle(cornerRadius: 10))
                }

                Button(action: connect) {
                    HStack {
                        if checking { ProgressView().tint(.white) }
                        Text(checking ? "Connecting…" : "Connect").font(.system(size: 16, weight: .semibold))
                    }
                    .frame(maxWidth: .infinity).padding(.vertical, 13)
                    .background(Color(red: 0.9, green: 0.28, blue: 0.3))
                    .foregroundColor(.white)
                    .clipShape(Capsule())
                }
                .disabled(checking)
                .padding(.top, 6)

                Text("Your family runs this server at home. Nothing is sent anywhere else.")
                    .font(.system(size: 12)).foregroundColor(Color(white: 0.45))
                    .frame(maxWidth: .infinity, alignment: .center).padding(.top, 4)
            }
            .padding(28)
            .frame(maxWidth: 440)
            .frame(maxWidth: .infinity)
        }
        .onAppear { focused = true }
    }

    private func normalized() -> URL? {
        var v = address.trimmingCharacters(in: .whitespaces)
        while v.hasSuffix("/") { v.removeLast() }
        if v.isEmpty { return nil }
        if !v.lowercased().hasPrefix("http://") && !v.lowercased().hasPrefix("https://") { v = "http://" + v }
        return URL(string: v)
    }

    private func connect() {
        guard let base = normalized() else { focused = true; return }
        error = nil
        checking = true
        var req = URLRequest(url: base.appendingPathComponent("api/status"))
        req.timeoutInterval = 8
        req.cachePolicy = .reloadIgnoringLocalCacheData
        URLSession.shared.dataTask(with: req) { data, response, _ in
            DispatchQueue.main.async {
                checking = false
                // any HTTP response means a server is there; a Volvix Home server returns JSON with "name"
                if let http = response as? HTTPURLResponse, (200...499).contains(http.statusCode) {
                    let looksRight = (data.flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any])?["name"] != nil
                        || http.statusCode == 200
                    if looksRight { onConnect(base) } else { error = "That address answered, but it isn't a Volvix Home server." }
                } else {
                    error = "Couldn't reach a server there. Check the address and that the server is on."
                }
            }
        }.resume()
    }
}

/// The red record logo, drawn in SwiftUI.
struct RecordMark: View {
    var body: some View {
        GeometryReader { geo in
            let s = min(geo.size.width, geo.size.height)
            ZStack {
                Circle().fill(Color(red: 0.9, green: 0.28, blue: 0.3))
                Circle().stroke(Color.black.opacity(0.22), lineWidth: s * 0.03).frame(width: s * 0.6, height: s * 0.6)
                Circle().fill(Color(white: 0.07)).frame(width: s * 0.22, height: s * 0.22)
            }
        }
    }
}
