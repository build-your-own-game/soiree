import CoreMotion
import SwiftUI
import WebKit

/// The free path: the game's own page, full screen. The page talks to its
/// game however it likes; the app only adds `window.telecommande`, for what
/// a browser cannot do: haptics and motion without a permission prompt.
struct WebRemote: UIViewRepresentable {
    let url: URL

    func makeCoordinator() -> Bridge {
        Bridge()
    }

    func makeUIView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        let scripts = configuration.userContentController
        scripts.addUserScript(WKUserScript(source: Bridge.script, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        scripts.add(context.coordinator, name: Bridge.name)
        configuration.allowsInlineMediaPlayback = true
        let web = WKWebView(frame: .zero, configuration: configuration)
        web.scrollView.isScrollEnabled = false
        web.scrollView.bounces = false
        web.scrollView.contentInsetAdjustmentBehavior = .never
        web.isOpaque = false
        web.backgroundColor = .clear
        web.isInspectable = true
        context.coordinator.web = web
        web.load(URLRequest(url: url))
        return web
    }

    func updateUIView(_ web: WKWebView, context: Context) {
        if web.url != url, !web.isLoading {
            web.load(URLRequest(url: url))
        }
    }

    static func dismantleUIView(_ web: WKWebView, coordinator: Bridge) {
        coordinator.stop()
        web.configuration.userContentController.removeScriptMessageHandler(forName: Bridge.name)
    }

    /// Messages from the page, and motion back to it.
    @MainActor
    final class Bridge: NSObject, WKScriptMessageHandler {
        static let name = "telecommande"

        /// `window.telecommande`: `vibrate(kind)` with kind "press",
        /// "release", "success" or "warning", and `motion(callback)`, which
        /// calls back about 60 times a second with `{gravity: [x, y, z]}` in
        /// g and `{rotation: [x, y, z]}` in radians per second, in the
        /// device's axes.
        static let script = """
            window.telecommande = {
              app: true,
              vibrate(kind) { window.webkit.messageHandlers.telecommande.postMessage({ vibrate: String(kind) }); },
              motion(callback) {
                window.__telecommandeMotion = callback;
                window.webkit.messageHandlers.telecommande.postMessage({ motion: true });
              },
            };
            """

        weak var web: WKWebView?
        private let haptics = Haptics()
        private var motion: MotionReader?

        func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
            guard let body = message.body as? [String: Any] else { return }
            if let kind = body["vibrate"] as? String {
                haptics.play(Self.haptic(kind))
            }
            if body["motion"] as? Bool == true, motion == nil {
                motion = MotionReader { [weak self] sample in self?.send(sample) }
            }
        }

        func stop() {
            motion?.stop()
            motion = nil
        }

        private func send(_ sample: CMDeviceMotion) {
            let gravity = [sample.gravity.x, sample.gravity.y, sample.gravity.z]
            let rotation = [sample.rotationRate.x, sample.rotationRate.y, sample.rotationRate.z]
            let list = { (values: [Double]) in values.map { String(format: "%.4f", $0) }.joined(separator: ",") }
            let call = "window.__telecommandeMotion && window.__telecommandeMotion({gravity:[\(list(gravity))],rotation:[\(list(rotation))]})"
            web?.evaluateJavaScript(call, completionHandler: nil)
        }

        private static func haptic(_ kind: String) -> Haptic? {
            switch kind {
            case "press": .press
            case "release": .release
            case "success": .success
            case "warning": .warning
            default: nil
            }
        }
    }
}
