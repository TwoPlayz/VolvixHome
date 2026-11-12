import SwiftUI

/// The equalizer as a graph: ten dots, one per band, that you drag up to boost and down to cut.
struct EQGraph: View {
    let gains: [Double]
    let enabled: Bool
    let onChange: ([Double]) -> Void
    let onEnd: () -> Void
    @State private var dragging: Int?

    private let range = 12.0
    private let left: CGFloat = 34
    private let right: CGFloat = 14
    private let top: CGFloat = 16
    private let bottom: CGFloat = 30

    private func label(_ f: Double) -> String { f >= 1000 ? "\(Int(f / 1000))k" : "\(Int(f))" }

    var body: some View {
        GeometryReader { geo in
            let w = geo.size.width, h = geo.size.height
            let pw = w - left - right, ph = h - top - bottom
            let xs = (0..<10).map { left + pw * CGFloat($0) / 9 }
            let ys = gains.map { top + ph * CGFloat((range - $0) / (2 * range)) }
            let zero = top + ph / 2
            ZStack {
                Canvas { ctx, size in
                    // horizontal guides every 6 dB, labelled
                    for d in stride(from: -12, through: 12, by: 6) {
                        let y = top + ph * CGFloat((range - Double(d)) / (2 * range))
                        var line = Path()
                        line.move(to: CGPoint(x: left, y: y))
                        line.addLine(to: CGPoint(x: left + pw, y: y))
                        ctx.stroke(line, with: .color(Theme.text.opacity(d == 0 ? 0.28 : 0.1)), lineWidth: 1)
                        let t = Text(d > 0 ? "+\(d)" : "\(d)").font(.system(size: 10)).foregroundColor(Theme.text2)
                        ctx.draw(t, at: CGPoint(x: left - 8, y: y), anchor: .trailing)
                    }
                    for i in 0..<10 {
                        var v = Path()
                        v.move(to: CGPoint(x: xs[i], y: top))
                        v.addLine(to: CGPoint(x: xs[i], y: top + ph))
                        ctx.stroke(v, with: .color(Theme.text.opacity(0.06)), lineWidth: 1)
                        let t = Text(label(eqBandFrequencies[i])).font(.system(size: 10)).foregroundColor(Theme.text2)
                        ctx.draw(t, at: CGPoint(x: xs[i], y: top + ph + 16), anchor: .center)
                    }
                }
                let pts = (0..<10).map { CGPoint(x: xs[$0], y: ys[$0]) }
                smooth(pts)
                    .stroke(Theme.accent, style: StrokeStyle(lineWidth: 3, lineCap: .round, lineJoin: .round))
                    .opacity(enabled ? 1 : 0.4)
                filled(pts, zero: zero)
                    .fill(Theme.accent.opacity(enabled ? 0.18 : 0.06))
                ForEach(0..<10, id: \.self) { i in
                    Circle()
                        .fill(Theme.accent)
                        .overlay(Circle().stroke(Theme.surface, lineWidth: 3))
                        .frame(width: dragging == i ? 28 : 22, height: dragging == i ? 28 : 22)
                        .position(pts[i])
                        .opacity(enabled ? 1 : 0.45)
                }
                if let i = dragging {
                    Text((gains[i] > 0 ? "+" : "") + "\(Int(gains[i])) dB · \(label(eqBandFrequencies[i])) Hz")
                        .font(.system(size: 12, weight: .semibold)).foregroundColor(Theme.text)
                        .padding(.horizontal, 10).padding(.vertical, 5)
                        .background(Theme.surface3).clipShape(Capsule())
                        .position(x: min(max(xs[i], 70), w - 70), y: max(14, ys[i] - 34))
                }
            }
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0)
                .onChanged { v in
                    if !enabled { return }
                    if dragging == nil {
                        var best = 0
                        var bestD = CGFloat.greatestFiniteMagnitude
                        for i in 0..<10 {
                            let d = abs(xs[i] - v.startLocation.x)
                            if d < bestD { bestD = d; best = i }
                        }
                        dragging = best
                    }
                    guard let i = dragging else { return }
                    var g = range - Double((v.location.y - top) / ph) * 2 * range
                    g = max(-range, min(range, g.rounded()))
                    if g != gains[i] {
                        var next = gains
                        next[i] = g
                        onChange(next)
                    }
                }
                .onEnded { _ in
                    dragging = nil
                    if enabled { onEnd() }
                })
        }
        .frame(height: 300)
    }

    private func smooth(_ p: [CGPoint]) -> Path {
        var path = Path()
        guard p.count > 1 else { return path }
        path.move(to: p[0])
        for i in 0..<(p.count - 1) {
            let p0 = p[max(i - 1, 0)], p1 = p[i], p2 = p[i + 1], p3 = p[min(i + 2, p.count - 1)]
            let c1 = CGPoint(x: p1.x + (p2.x - p0.x) / 6, y: p1.y + (p2.y - p0.y) / 6)
            let c2 = CGPoint(x: p2.x - (p3.x - p1.x) / 6, y: p2.y - (p3.y - p1.y) / 6)
            path.addCurve(to: p2, control1: c1, control2: c2)
        }
        return path
    }

    private func filled(_ p: [CGPoint], zero: CGFloat) -> Path {
        var path = smooth(p)
        guard let first = p.first, let last = p.last else { return path }
        path.addLine(to: CGPoint(x: last.x, y: zero))
        path.addLine(to: CGPoint(x: first.x, y: zero))
        path.closeSubpath()
        return path
    }
}

/// Playback settings: crossfade, volume levelling, the equalizer, private session.
struct PlaybackSettingsView: View {
    @EnvironmentObject var app: AppModel

    private func segmented(_ title: String, _ detail: String?, selection: Binding<String>, _ options: [(String, String)]) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
            if let d = detail { Text(d).font(.system(size: 13)).foregroundColor(Theme.text2) }
            Picker("", selection: selection) {
                ForEach(options, id: \.0) { o in Text(o.1).tag(o.0) }
            }
            .pickerStyle(.segmented)
        }
    }

    private func toggleRow(_ title: String, _ detail: String, _ value: Binding<Bool>) -> some View {
        Toggle(isOn: value) {
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                Text(detail).font(.system(size: 13)).foregroundColor(Theme.text2)
            }
        }
        .tint(Theme.accent)
    }

    var body: some View {
        let fx = app.fx
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                Text("Playback").font(.system(size: 30, weight: .bold)).foregroundColor(Theme.text)

                segmented("Crossfade", "Songs overlap this long. Off plays them one after another.",
                          selection: Binding(get: { String(fx.crossfade) }, set: { v in app.updateSettings { $0["crossfade"] = Int(v) ?? 0 } }),
                          [("0", "Off"), ("2", "2 s"), ("4", "4 s"), ("6", "6 s"), ("8", "8 s"), ("12", "12 s")])
                if fx.crossfade > 0 {
                    toggleRow("Crossfade songs on the same album", "Off keeps albums flowing the way they were recorded (live albums, mixes), and only crossfades between different albums.",
                              Binding(get: { fx.crossfadeAlbums }, set: { v in app.updateSettings { $0["crossfadeAlbums"] = v } }))
                }

                segmented("Volume leveling", "Quiet and loud songs sound about equally loud (needs ReplayGain tags in the files).",
                          selection: Binding(get: { fx.leveling }, set: { v in app.updateSettings { $0["leveling"] = v } }),
                          [("off", "Off"), ("track", "By song"), ("album", "By album")])

                VStack(alignment: .leading, spacing: 12) {
                    toggleRow("Equalizer", "Shape the sound: drag the dots up to boost and down to cut. It works on this phone while it plays (not when casting to a TV).",
                              Binding(get: { fx.eqOn }, set: { v in app.setEQ(on: v) }))
                    HStack(spacing: 10) {
                        Menu {
                            ForEach(eqPresets, id: \.key) { p in
                                Button(p.label) { app.setEQ(preset: p.key, pre: p.pre, gains: p.gains) }
                            }
                        } label: {
                            HStack(spacing: 6) {
                                Text(eqPresets.first { $0.key == fx.eqPreset }?.label ?? "Custom").font(.system(size: 14, weight: .semibold))
                                Image(systemName: "chevron.up.chevron.down").font(.system(size: 11))
                            }
                            .padding(.horizontal, 14).padding(.vertical, 8)
                            .background(Theme.surface3).foregroundColor(Theme.text).clipShape(Capsule())
                        }
                        .disabled(!fx.eqOn)
                        Spacer()
                        Button("Reset") { app.setEQ(preset: "flat", pre: 0, gains: [Double](repeating: 0, count: 10)) }
                            .font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.accent)
                            .disabled(!fx.eqOn)
                    }
                    EQGraph(gains: fx.eqGains, enabled: fx.eqOn,
                            onChange: { app.previewEQ(gains: $0) },
                            onEnd: { app.saveEQ() })
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Text("Preamp").font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text)
                            Spacer()
                            Text((fx.eqPre > 0 ? "+" : "") + "\(Int(fx.eqPre)) dB").font(.system(size: 13)).foregroundColor(Theme.text2)
                        }
                        Slider(value: Binding(get: { fx.eqPre }, set: { app.previewEQ(pre: $0.rounded()) }), in: -12...6, step: 1) { editing in
                            if !editing { app.saveEQ() }
                        }
                        .tint(Theme.accent)
                        .disabled(!fx.eqOn)
                    }
                }
                .padding(14)
                .background(Theme.surface2)
                .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))

                toggleRow("Private session", "Songs you play now aren't counted in your listening numbers, mixes or Rewind.",
                          Binding(get: { fx.privateSession }, set: { v in app.updateSettings { $0["privateSession"] = v } }))
            }
            .padding(16)
        }
        .background(Theme.surface)
        .navigationBarTitleDisplayMode(.inline)
    }
}
