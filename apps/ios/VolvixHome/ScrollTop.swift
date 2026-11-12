import SwiftUI
import UIKit

/// Scrolls the list that is on screen back to its top (the tab bar's "tap the tab you are on again").
@MainActor
func scrollActiveScrollViewToTop() {
    guard let window = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).flatMap({ $0.windows }).first(where: { $0.isKeyWindow }) else { return }
    var best: UIScrollView?
    var area: CGFloat = 0
    func visit(_ v: UIView) {
        if let s = v as? UIScrollView, !s.isHidden, s.alpha > 0.01, s.window != nil {
            let r = s.convert(s.bounds, to: window).intersection(window.bounds)
            let a = r.width * r.height
            // a page's own list: it scrolls up and down (the rows of filters scroll sideways)
            let vertical = s.contentSize.height > s.bounds.height * 0.9 && s.contentSize.width <= s.bounds.width + 1
            if vertical && a > area { best = s; area = a }
        }
        for c in v.subviews { visit(c) }
    }
    visit(window)
    if let s = best { s.setContentOffset(CGPoint(x: 0, y: -s.adjustedContentInset.top), animated: true) }
}
