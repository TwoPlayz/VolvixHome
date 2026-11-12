import SwiftUI
import UIKit

/// The system's back button opens a list of earlier pages when it is held down, which here is a list with nothing in it.
/// This one is just a button (and the swipe from the edge still goes back).
struct CustomBack: ViewModifier {
    @Environment(\.dismiss) private var dismiss

    func body(content: Content) -> some View {
        content
            .navigationBarBackButtonHidden(true)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button { dismiss() } label: {
                        Image(systemName: "chevron.left").font(.system(size: 16, weight: .semibold)).foregroundColor(Theme.text)
                            .frame(width: 36, height: 36).glass(Circle())
                    }
                }
            }
    }
}

// with the system button hidden, keep the swipe from the screen's edge
extension UINavigationController: UIGestureRecognizerDelegate {
    open override func viewDidLoad() {
        super.viewDidLoad()
        interactivePopGestureRecognizer?.delegate = self
    }

    public func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        viewControllers.count > 1
    }
}
