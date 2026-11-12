import SwiftUI

/// The website's own icons (24 x 24 outline drawings), so a few places can look exactly like the website and the Windows app.
enum SVGIcons {
    static let table: [String: [String]] = [
        "search": ["M18 11a7 7 0 1 0 -14 0a7 7 0 1 0 14 0", "m20 20-3.5-3.5"],
        "plus": ["M12 5v14M5 12h14"],
        "repeat": ["m17 2 4 4-4 4", "M3 11v-1a4 4 0 0 1 4-4h14", "m7 22-4-4 4-4", "M21 13v1a4 4 0 0 1-4 4H3"],
        "history": ["M3 12a9 9 0 1 0 2.6-6.4L3 8", "M3 3v5h5", "M12 7v5l3 2"],
        "user": ["M16 8a4 4 0 1 0 -8 0a4 4 0 1 0 8 0", "M4 21a8 8 0 0 1 16 0"],
        "music": ["M9 18V5l12-2v13", "M9 18a3 3 0 1 0 -6 0a3 3 0 1 0 6 0", "M21 16a3 3 0 1 0 -6 0a3 3 0 1 0 6 0"],
        "heart": ["M12 20.5s-7.5-4.8-9.6-9.4C1 8 2.9 4.6 6.2 4.6c1.9 0 3.4 1 4.3 2.4L12 9l1.5-2c.9-1.4 2.4-2.4 4.3-2.4 3.3 0 5.2 3.4 3.8 6.5-2.1 4.6-9.6 9.4-9.6 9.4z"],
    ]

    static func path(_ name: String) -> Path {
        var all = Path()
        for d in table[name] ?? table["music"] ?? [] { all.addPath(parse(d)) }
        return all
    }

    // MARK: a small SVG path reader (M L H V C S A Z, upper and lower case)

    private static func numbers(_ s: Substring) -> [CGFloat] {
        var out: [CGFloat] = []
        var cur = ""
        var seenDot = false
        var seenExp = false
        func flush() { if !cur.isEmpty, let v = Double(cur) { out.append(CGFloat(v)) }; cur = ""; seenDot = false; seenExp = false }
        for c in s {
            if c == "-" || c == "+" {
                if !cur.isEmpty && !(cur.last == "e" || cur.last == "E") { flush() }
                cur.append(c)
            } else if c == "." {
                if seenDot { flush() }
                seenDot = true
                cur.append(c)
            } else if c.isNumber {
                cur.append(c)
            } else if (c == "e" || c == "E") && !seenExp && !cur.isEmpty {
                seenExp = true
                cur.append(c)
            } else {
                flush()
            }
        }
        flush()
        return out
    }

    static func parse(_ d: String) -> Path {
        var path = Path()
        var cur = CGPoint.zero, start = CGPoint.zero
        var lastCubic: CGPoint?
        // split into commands with their numbers
        var cmds: [(Character, [CGFloat])] = []
        var letter: Character?
        var chunk = Substring("")
        var i = d.startIndex
        var begin = d.startIndex
        func close(_ end: String.Index) {
            if let l = letter { cmds.append((l, numbers(d[begin..<end]))) }
            _ = chunk
        }
        while i < d.endIndex {
            let c = d[i]
            if c.isLetter && c != "e" && c != "E" {
                close(i)
                letter = c
                begin = d.index(after: i)
            }
            i = d.index(after: i)
        }
        close(d.endIndex)

        for (cmd, n) in cmds {
            let rel = cmd.isLowercase
            let upper = Character(cmd.uppercased())
            var k = 0
            func pt(_ x: CGFloat, _ y: CGFloat) -> CGPoint { rel ? CGPoint(x: cur.x + x, y: cur.y + y) : CGPoint(x: x, y: y) }
            if upper == "Z" {
                path.closeSubpath()
                cur = start
                lastCubic = nil
                continue
            }
            var first = true
            while k < n.count || (first && n.isEmpty) {
                switch upper {
                case "M":
                    guard k + 1 < n.count else { k = n.count; break }
                    let p = pt(n[k], n[k + 1]); k += 2
                    if first { path.move(to: p); start = p } else { path.addLine(to: p) }
                    cur = p; lastCubic = nil
                case "L":
                    guard k + 1 < n.count else { k = n.count; break }
                    let p = pt(n[k], n[k + 1]); k += 2
                    path.addLine(to: p); cur = p; lastCubic = nil
                case "H":
                    guard k < n.count else { break }
                    let p = CGPoint(x: rel ? cur.x + n[k] : n[k], y: cur.y); k += 1
                    path.addLine(to: p); cur = p; lastCubic = nil
                case "V":
                    guard k < n.count else { break }
                    let p = CGPoint(x: cur.x, y: rel ? cur.y + n[k] : n[k]); k += 1
                    path.addLine(to: p); cur = p; lastCubic = nil
                case "C":
                    guard k + 5 < n.count else { k = n.count; break }
                    let c1 = pt(n[k], n[k + 1]), c2 = pt(n[k + 2], n[k + 3]), p = pt(n[k + 4], n[k + 5]); k += 6
                    path.addCurve(to: p, control1: c1, control2: c2); cur = p; lastCubic = c2
                case "S":
                    guard k + 3 < n.count else { k = n.count; break }
                    let c1 = lastCubic.map { CGPoint(x: 2 * cur.x - $0.x, y: 2 * cur.y - $0.y) } ?? cur
                    let c2 = pt(n[k], n[k + 1]), p = pt(n[k + 2], n[k + 3]); k += 4
                    path.addCurve(to: p, control1: c1, control2: c2); cur = p; lastCubic = c2
                case "A":
                    guard k + 6 < n.count else { k = n.count; break }
                    let p = pt(n[k + 5], n[k + 6])
                    arc(&path, from: cur, rx: n[k], ry: n[k + 1], rot: n[k + 2], large: n[k + 3] != 0, sweep: n[k + 4] != 0, to: p)
                    k += 7; cur = p; lastCubic = nil
                default:
                    k = n.count
                }
                first = false
                if n.isEmpty { break }
            }
        }
        return path
    }

    private static func arc(_ path: inout Path, from p0: CGPoint, rx rx0: CGFloat, ry ry0: CGFloat, rot: CGFloat, large: Bool, sweep: Bool, to p1: CGPoint) {
        var rx = abs(rx0), ry = abs(ry0)
        if rx == 0 || ry == 0 || p0 == p1 { path.addLine(to: p1); return }
        let phi = rot * .pi / 180
        let cosP = cos(phi), sinP = sin(phi)
        let dx2 = (p0.x - p1.x) / 2, dy2 = (p0.y - p1.y) / 2
        let x1p = cosP * dx2 + sinP * dy2
        let y1p = -sinP * dx2 + cosP * dy2
        let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry)
        if lambda > 1 { let s = sqrt(lambda); rx *= s; ry *= s }
        let sign: CGFloat = (large == sweep) ? -1 : 1
        let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p
        let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p
        let coef = sign * sqrt(max(0, num / den))
        let cxp = coef * rx * y1p / ry
        let cyp = -coef * ry * x1p / rx
        let cx = cosP * cxp - sinP * cyp + (p0.x + p1.x) / 2
        let cy = sinP * cxp + cosP * cyp + (p0.y + p1.y) / 2
        func angle(_ ux: CGFloat, _ uy: CGFloat, _ vx: CGFloat, _ vy: CGFloat) -> CGFloat { atan2(ux * vy - uy * vx, ux * vx + uy * vy) }
        let th1 = angle(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry)
        var dth = angle((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry)
        if !sweep && dth > 0 { dth -= 2 * .pi } else if sweep && dth < 0 { dth += 2 * .pi }
        let segs = max(1, Int(ceil(abs(dth) / (.pi / 2))))
        let delta = dth / CGFloat(segs)
        let t = 8.0 / 3.0 * pow(sin(delta / 4), 2) / sin(delta / 2)
        func map(_ x: CGFloat, _ y: CGFloat) -> CGPoint {
            CGPoint(x: cx + rx * cosP * x - ry * sinP * y, y: cy + rx * sinP * x + ry * cosP * y)
        }
        var th = th1
        for _ in 0..<segs {
            let c1 = cos(th), s1 = sin(th), c2 = cos(th + delta), s2 = sin(th + delta)
            path.addCurve(to: map(c2, s2), control1: map(c1 - t * s1, s1 + t * c1), control2: map(c2 + t * s2, s2 - t * c2))
            th += delta
        }
    }
}

/// One of the website's icons, drawn as an outline.
struct SVGIcon: View {
    let name: String
    var size: CGFloat = 24
    var color: Color = .white
    var lineWidth: CGFloat = 1.8

    var body: some View {
        Canvas { ctx, sz in
            let k = min(sz.width, sz.height) / 24
            let p = SVGIcons.path(name).applying(CGAffineTransform(scaleX: k, y: k))
            ctx.stroke(p, with: .color(color), style: StrokeStyle(lineWidth: lineWidth * k, lineCap: .round, lineJoin: .round))
        }
        .frame(width: size, height: size)
    }
}

/// The website's sound-quality mark: a waveform that gets fuller with better quality.
struct QualityIcon: View {
    /// 1 low, 2 normal, 3 high, 4 lossless
    let level: Int
    var size: CGFloat = 22
    var color: Color = .white

    var body: some View {
        Canvas { ctx, sz in
            let k = min(sz.width, sz.height) / 24
            let shape: [CGFloat] = [0.35, 0.7, 1, 0.55, 0.85, 0.4]
            let amp: CGFloat = [0, 7, 11, 15, 19][max(1, min(4, level))]
            for (i, s) in shape.enumerated() {
                let h = max(2.5, s * amp)
                let r = CGRect(x: (2.6 + CGFloat(i) * 3.4) * k, y: (12 - h / 2) * k, width: 2.2 * k, height: h * k)
                ctx.fill(Path(roundedRect: r, cornerRadius: 1.1 * k), with: .color(color))
            }
        }
        .frame(width: size, height: size)
    }
}
