import SwiftUI

extension Color {
    init(hex: UInt32, alpha: Double = 1) {
        let r = Double((hex >> 16) & 0xFF) / 255
        let g = Double((hex >> 8) & 0xFF) / 255
        let b = Double(hex & 0xFF) / 255
        self.init(.sRGB, red: r, green: g, blue: b, opacity: alpha)
    }
}

/// The same colours as the website's dark theme.
enum Theme {
    static let frame = Color(hex: 0x000000)
    static let surface = Color(hex: 0x121214)
    static let surface2 = Color(hex: 0x19191C)
    static let surface3 = Color(hex: 0x242428)
    static let accent = Color(hex: 0xE5484D)
    static let text = Color(hex: 0xF5F5F7)
    static let text2 = Color(hex: 0xA1A1A8)
    static let text3 = Color(hex: 0x86868E)
    static let border = Color.white.opacity(0.08)
    static let hover = Color.white.opacity(0.06)

    /// A steady colour for a name (the same name always gets the same tint), for pictures that have none.
    static func tint(_ seed: String) -> Color {
        var h: UInt32 = 5381
        for u in seed.unicodeScalars { h = (h &* 33) &+ u.value }
        let hue = Double(h % 360) / 360
        return Color(hue: hue, saturation: 0.35, brightness: 0.38)
    }
}

func formatTime(_ seconds: Double) -> String {
    if !seconds.isFinite || seconds < 0 { return "0:00" }
    let s = Int(seconds.rounded(.down))
    return "\(s / 60):" + String(format: "%02d", s % 60)
}

func formatLength(_ seconds: Int) -> String {
    if seconds >= 3600 { return "\(seconds / 3600) h \((seconds % 3600) / 60) min" }
    return "\(max(1, seconds / 60)) min"
}

func initials(_ name: String) -> String {
    let parts = name.split(separator: " ").filter { p in p.contains(where: { $0.isLetter || $0.isNumber }) }
    let letters = parts.prefix(2).compactMap { $0.first }
    return String(letters).uppercased()
}
