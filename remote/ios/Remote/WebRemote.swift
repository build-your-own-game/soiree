import CoreHaptics
import CoreMotion
import Network
import SwiftUI
import WebKit

/// The free path: the game's own page, full screen. The page talks to its
/// game however it likes. For what a browser cannot do, it posts messages to
/// the app's handler, `webkit.messageHandlers.telecommande`, and the app
/// answers with `telecommande:…` events on `window`. The shapes are in
/// `CONTRAT.md`; the `remote-web` crate wraps them in Rust. The page also gets
/// the microphone and camera through the usual web APIs.
struct WebRemote: UIViewRepresentable {
    let url: URL
    /// The player's identity and first name, for the page.
    let player: Data
    let name: String

    func makeCoordinator() -> Bridge {
        Bridge(host: url.host() ?? "")
    }

    func makeUIView(context: Context) -> WKWebView {
        // Every page starts in landscape, like the pads, until it asks.
        Orientation.allow(.landscape)
        let configuration = WKWebViewConfiguration()
        let scripts = configuration.userContentController
        scripts.addUserScript(WKUserScript(
            source: Bridge.script(player: player, name: name), injectionTime: .atDocumentStart, forMainFrameOnly: true))
        scripts.add(context.coordinator, name: Bridge.name)
        configuration.allowsInlineMediaPlayback = true
        configuration.mediaTypesRequiringUserActionForPlayback = []
        let web = RemoteWebView(frame: .zero, configuration: configuration)
        let bridge = context.coordinator
        web.onResize = { [weak bridge] in bridge?.orientationChanged() }
        web.uiDelegate = bridge
        web.navigationDelegate = bridge
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
        Orientation.allow(.landscape)
    }

    /// Messages from the page, and what goes back to it.
    @MainActor
    final class Bridge: NSObject, WKScriptMessageHandler, WKUIDelegate, WKNavigationDelegate {
        static let name = "telecommande"

        weak var web: WKWebView?
        /// The game's host: datagrams go there, and only its pages get the
        /// microphone and camera.
        private let host: String
        private let haptics = Haptics()
        private var engine: CHHapticEngine?
        private var motion: MotionReader?
        private var sockets: [Int: NWConnection] = [:]

        init(host: String) {
            self.host = host
        }

        /// The only script the app adds: data, before the page's own
        /// scripts. `window.telecommande` says the page runs in the app, who
        /// plays and how the phone is turned.
        static func script(player: Data, name: String) -> String {
            let info: [String: String] = [
                "player": player.map { String(format: "%02x", $0) }.joined(),
                "name": name,
                "orientation": Orientation.current,
            ]
            let json = (try? JSONSerialization.data(withJSONObject: info)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
            return "window.telecommande = \(json);"
        }

        func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
            guard let body = message.body as? [String: Any] else { return }
            if let kind = body["vibrate"] as? String {
                haptics.play(Self.haptic(kind))
            }
            if let events = body["haptic"] as? [[String: Any]] {
                playPattern(events)
            }
            if let on = body["motion"] as? Bool {
                if on, motion == nil {
                    motion = MotionReader { [weak self] sample in self?.send(sample) }
                } else if !on {
                    motion?.stop()
                    motion = nil
                }
            }
            if let which = body["orientation"] as? String {
                switch which {
                case "portrait": Orientation.allow(.portrait)
                case "any": Orientation.allow(.allButUpsideDown)
                default: Orientation.allow(.landscape)
                }
            }
            if let datagram = body["datagram"] as? [String: Any] {
                handleDatagram(datagram)
            }
        }

        /// The page's own origin gets the microphone and camera without a
        /// second question: iOS already asks the player once for the app.
        func webView(
            _ webView: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin,
            initiatedByFrame frame: WKFrameInfo, type: WKMediaCaptureType,
            decisionHandler: @escaping @MainActor (WKPermissionDecision) -> Void
        ) {
            decisionHandler(origin.host == host ? .grant : .prompt)
        }

        /// A turn during the load reached the page before it listened: say
        /// it again once loaded.
        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            orientationChanged()
        }

        func stop() {
            motion?.stop()
            motion = nil
            for socket in sockets.values {
                socket.cancel()
            }
            sockets.removeAll()
            engine?.stop()
        }

        /// The orientation changed: the page learns it before its next
        /// motion sample.
        func orientationChanged() {
            let orientation = Self.quoted(Orientation.current)
            call("if (window.telecommande) window.telecommande.orientation = \(orientation);")
            dispatch("orientation", detail: orientation)
        }

        private func send(_ sample: CMDeviceMotion) {
            let list = { (values: [Double]) in values.map { String(format: "%.4f", $0) }.joined(separator: ",") }
            let q = sample.attitude.quaternion
            let fields = [
                "gravity:[\(list([sample.gravity.x, sample.gravity.y, sample.gravity.z]))]",
                "rotation:[\(list([sample.rotationRate.x, sample.rotationRate.y, sample.rotationRate.z]))]",
                "acceleration:[\(list([sample.userAcceleration.x, sample.userAcceleration.y, sample.userAcceleration.z]))]",
                "attitude:[\(list([q.x, q.y, q.z, q.w]))]",
                "time:\(String(format: "%.4f", sample.timestamp))",
            ]
            dispatch("motion", detail: "{\(fields.joined(separator: ","))}")
        }

        /// A page's haptic pattern, with every value kept in range: it comes
        /// from the network.
        private func playPattern(_ events: [[String: Any]]) {
            guard CHHapticEngine.capabilitiesForHardware().supportsHaptics else { return }
            let number = { (event: [String: Any], key: String, fallback: Double) in
                let value = (event[key] as? NSNumber)?.doubleValue ?? fallback
                return value.isFinite ? value : fallback
            }
            let pattern = events.prefix(64).map { event in
                let duration = min(max(number(event, "duration", 0), 0), 5)
                return CHHapticEvent(
                    eventType: duration > 0 ? .hapticContinuous : .hapticTransient,
                    parameters: [
                        CHHapticEventParameter(parameterID: .hapticIntensity, value: Float(min(max(number(event, "intensity", 1), 0), 1))),
                        CHHapticEventParameter(parameterID: .hapticSharpness, value: Float(min(max(number(event, "sharpness", 0.5), 0), 1))),
                    ],
                    relativeTime: min(max(number(event, "time", 0), 0), 10),
                    duration: duration)
            }
            do {
                let engine = try self.engine ?? makeEngine()
                try engine.start()
                try engine.makePlayer(with: CHHapticPattern(events: pattern, parameters: [])).start(atTime: CHHapticTimeImmediate)
            } catch {
                engine = nil
            }
        }

        private func makeEngine() throws -> CHHapticEngine {
            let engine = try CHHapticEngine()
            engine.playsHapticsOnly = true
            // After a reset (audio interruption, background), start again on
            // the next pattern.
            engine.resetHandler = { [weak self] in
                Task { @MainActor in self?.engine = nil }
            }
            self.engine = engine
            return engine
        }

        private func handleDatagram(_ message: [String: Any]) {
            guard let id = (message["id"] as? NSNumber)?.intValue, let op = message["op"] as? String else { return }
            switch op {
            case "open":
                guard sockets[id] == nil, sockets.count < 8,
                    let port = (message["port"] as? NSNumber).flatMap({ NWEndpoint.Port(rawValue: $0.uint16Value) })
                else { return }
                let socket = NWConnection(host: NWEndpoint.Host(host), port: port, using: .udp)
                sockets[id] = socket
                socket.start(queue: .main)
                receive(on: socket, id: id)
            case "send":
                guard let socket = sockets[id], let bytes = message["data"] as? [NSNumber], bytes.count <= 65_507 else {
                    return
                }
                socket.send(content: Data(bytes.map(\.uint8Value)), completion: .idempotent)
            case "close":
                sockets.removeValue(forKey: id)?.cancel()
            default:
                break
            }
        }

        private func receive(on socket: NWConnection, id: Int) {
            socket.receiveMessage { [weak self] data, _, _, error in
                MainActor.assumeIsolated {
                    guard let self, self.sockets[id] === socket else { return }
                    if let data {
                        self.dispatch("datagram", detail: "{id:\(id),data:\(Self.quoted(data.base64EncodedString()))}")
                    }
                    if error == nil {
                        self.receive(on: socket, id: id)
                    }
                }
            }
        }

        /// A `telecommande:<name>` event on `window`, with `detail` given as a
        /// JavaScript expression.
        private func dispatch(_ name: String, detail: String) {
            call("window.dispatchEvent(new CustomEvent(\"telecommande:\(name)\", { detail: \(detail) }))")
        }

        private func call(_ script: String) {
            web?.evaluateJavaScript(script, completionHandler: nil)
        }

        private static func quoted(_ text: String) -> String {
            "\"\(text.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\""))\""
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

/// A web view that says when its size changes, so the page learns the new
/// orientation.
final class RemoteWebView: WKWebView {
    var onResize: (() -> Void)?
    private var lastSize = CGSize.zero

    override func layoutSubviews() {
        super.layoutSubviews()
        if bounds.size != lastSize {
            lastSize = bounds.size
            onResize?()
        }
    }
}

/// Which orientations the app allows now. The pads are landscape; a game's
/// own page may ask for portrait.
@MainActor
enum Orientation {
    private(set) static var allowed = UIInterfaceOrientationMask.landscape

    static var current: String {
        let scene = UIApplication.shared.connectedScenes.first { $0 is UIWindowScene } as? UIWindowScene
        switch scene?.effectiveGeometry.interfaceOrientation {
        case .portrait, .portraitUpsideDown: return "portrait"
        case .landscapeRight: return "landscapeRight"
        default: return "landscapeLeft"
        }
    }

    static func allow(_ mask: UIInterfaceOrientationMask) {
        guard mask != allowed else { return }
        allowed = mask
        for case let scene as UIWindowScene in UIApplication.shared.connectedScenes {
            scene.keyWindow?.rootViewController?.setNeedsUpdateOfSupportedInterfaceOrientations()
            scene.requestGeometryUpdate(.iOS(interfaceOrientations: mask)) { _ in }
        }
    }
}

/// Answers UIKit's question about orientations from `Orientation`.
final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication, supportedInterfaceOrientationsFor window: UIWindow?)
        -> UIInterfaceOrientationMask
    {
        Orientation.allowed
    }
}
