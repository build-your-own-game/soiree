import SwiftUI
import UIKit

/// Raw multitouch, straight from UIKit: SwiftUI gestures merge fingers and wait
/// to recognize a gesture, a pad needs every finger at once and right away.
/// Positions are in the full-screen space the remote is drawn in; times are
/// `UITouch.timestamp`, seconds since boot.
struct TouchSurface: UIViewRepresentable {
    enum Event {
        case down(id: UInt64, at: CGPoint, time: TimeInterval)
        /// One per touch sample: the screen samples faster than UIKit reports,
        /// and a drawing needs every sample (`coalescedTouches`).
        case move(id: UInt64, at: CGPoint, time: TimeInterval)
        case up(id: UInt64)
        /// Size, safe area or orientation changed (first layout, rotation).
        case layout(size: CGSize, insets: UIEdgeInsets, landscapeLeft: Bool)
    }

    let onEvent: (Event) -> Void

    func makeUIView(context: Context) -> SurfaceView {
        let view = SurfaceView()
        view.onEvent = onEvent
        return view
    }

    func updateUIView(_ view: SurfaceView, context: Context) {
        view.onEvent = onEvent
    }

    final class SurfaceView: UIView {
        var onEvent: (Event) -> Void = { _ in }
        private var ids: [ObjectIdentifier: UInt64] = [:]
        private var nextId: UInt64 = 0
        private var lastLayout: (CGSize, UIEdgeInsets, Bool)?

        override init(frame: CGRect) {
            super.init(frame: frame)
            isMultipleTouchEnabled = true
            backgroundColor = .clear
        }

        required init?(coder: NSCoder) {
            fatalError("TouchSurface is created in code")
        }

        override func layoutSubviews() {
            super.layoutSubviews()
            reportLayout()
        }

        override func safeAreaInsetsDidChange() {
            super.safeAreaInsetsDidChange()
            reportLayout()
        }

        private func reportLayout() {
            let left = window?.windowScene?.effectiveGeometry.interfaceOrientation == .landscapeLeft
            guard bounds.width > 0,
                lastLayout?.0 != bounds.size || lastLayout?.1 != safeAreaInsets || lastLayout?.2 != left
            else { return }
            lastLayout = (bounds.size, safeAreaInsets, left)
            onEvent(.layout(size: bounds.size, insets: safeAreaInsets, landscapeLeft: left))
        }

        override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
            for touch in touches {
                let id = nextId
                nextId += 1
                ids[ObjectIdentifier(touch)] = id
                onEvent(.down(id: id, at: touch.location(in: self), time: touch.timestamp))
            }
        }

        override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
            for touch in touches {
                guard let id = ids[ObjectIdentifier(touch)] else { continue }
                for sample in event?.coalescedTouches(for: touch) ?? [touch] {
                    onEvent(.move(id: id, at: sample.location(in: self), time: sample.timestamp))
                }
            }
        }

        override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
            lift(touches)
        }

        override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
            lift(touches)
        }

        private func lift(_ touches: Set<UITouch>) {
            for touch in touches {
                guard let id = ids.removeValue(forKey: ObjectIdentifier(touch)) else { continue }
                onEvent(.up(id: id))
            }
        }
    }
}
