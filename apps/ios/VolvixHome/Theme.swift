import SwiftUI

extension Color {
    init(hex: UInt32, alpha: Double = 1) {
        let r = Double((hex >> 16) & 0xFF) / 255
        let g = Double((hex >> 8) & 0xFF) / 255
        let b = Double(hex & 0xFF) / 255
        self.init(.sRGB, red: r, green: g, blue: b, opacity: alpha)
    }
}

extension UIColor {
    convenience init(hex: UInt32) {
        self.init(red: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255, blue: CGFloat(hex & 0xFF) / 255, alpha: 1)
    }
}

/// The same colours as the website, in its dark and light themes (they follow the phone, or the choice in Account).
enum Theme {
    private static func pick(_ dark: UInt32, _ light: UInt32) -> Color {
        Color(UIColor { trait in trait.userInterfaceStyle == .dark ? UIColor(hex: dark) : UIColor(hex: light) })
    }

    static let frame = pick(0x000000, 0xE9E9EC)
    static let surface = pick(0x121214, 0xFFFFFF)
    static let surface2 = pick(0x19191C, 0xF6F6F8)
    static let surface3 = pick(0x242428, 0xEDEDF0)
    static let accent = pick(0xE5484D, 0xCF3439)
    static let text = pick(0xF5F5F7, 0x1D1D1F)
    static let text2 = pick(0xA1A1A8, 0x5F5F65)
    static let text3 = pick(0x86868E, 0x74747A)
    static let border = Color.primary.opacity(0.08)
    static let hover = Color.primary.opacity(0.06)

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
