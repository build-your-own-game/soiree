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
        static let name = webHandlerName()

        weak var web: WKWebView?
        /// The game's host: datagrams go there, and only its pages get the
        /// microphone and camera.
        private let host: String
        private let haptics = Haptics()
        private var engine: CHHapticEngine?
        private var motion: MotionReader?
        private var sockets: [UInt32: NWConnection] = [:]

        init(host: String) {
            self.host = host
        }

        /// The only script the app adds: data, before the page's own
        /// scripts. `window.telecommande` says the page runs in the app, who
        /// plays and how the phone is turned.
        static func script(player: Data, name: String) -> String {
            webBootstrap(player: player, name: name, orientation: Orientation.current) ?? ""
        }

        func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
            // WebKit supplies an untyped JavaScript object. Rust validates
            // it into the same contract used by the page, before any effect.
            guard JSONSerialization.isValidJSONObject(message.body),
                let data = try? JSONSerialization.data(withJSONObject: message.body),
                let json = String(data: data, encoding: .utf8)
            else { return }
            for command in webCommands(json: json) {
                switch command {
                case .vibrate(let haptic): haptics.play(haptic)
                case .haptic(let events): playPattern(events)
                case .motion(let enabled):
                    if enabled, motion == nil {
                        motion = MotionReader { [weak self] sample in self?.send(sample) }
                    } else if !enabled {
                        motion?.stop()
                        motion = nil
                    }
                case .orientation(let allowed):
                    switch allowed {
                    case .portrait: Orientation.allow(.portrait)
                    case .landscape: Orientation.allow(.landscape)
                    case .any: Orientation.allow(.allButUpsideDown)
                    }
                case .openDatagram(let id, let port):
                    guard sockets[id] == nil, sockets.count < 8,
                        let port = NWEndpoint.Port(rawValue: port) else { continue }
                    let socket = NWConnection(host: NWEndpoint.Host(host), port: port, using: .udp)
                    sockets[id] = socket
                    socket.start(queue: .main)
                    receive(on: socket, id: id)
                case .sendDatagram(let id, let data):
                    sockets[id]?.send(content: data, completion: .idempotent)
                case .closeDatagram(let id): sockets.removeValue(forKey: id)?.cancel()
                }
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
            call(webOrientation(orientation: Orientation.current))
        }

        private func send(_ sample: CMDeviceMotion) {
            let q = sample.attitude.quaternion
            call(webMotion(sample: WebMotion(
                gravity: WebVector3(x: sample.gravity.x, y: sample.gravity.y, z: sample.gravity.z),
                rotation: WebVector3(x: sample.rotationRate.x, y: sample.rotationRate.y, z: sample.rotationRate.z),
                acceleration: WebVector3(x: sample.userAcceleration.x, y: sample.userAcceleration.y, z: sample.userAcceleration.z),
                attitude: WebQuaternion(x: q.x, y: q.y, z: q.z, w: q.w),
                time: sample.timestamp)))
        }

        /// A page's haptic pattern, with every value kept in range: it comes
        /// from the network.
        private func playPattern(_ events: [WebHapticEvent]) {
            guard CHHapticEngine.capabilitiesForHardware().supportsHaptics else { return }
            let pattern = events.map { event in
                CHHapticEvent(
                    eventType: event.duration > 0 ? .hapticContinuous : .hapticTransient,
                    parameters: [
                        CHHapticEventParameter(parameterID: .hapticIntensity, value: Float(event.intensity)),
                        CHHapticEventParameter(parameterID: .hapticSharpness, value: Float(event.sharpness)),
                    ], relativeTime: event.time, duration: event.duration)
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

        private func receive(on socket: NWConnection, id: UInt32) {
            socket.receiveMessage { [weak self] data, _, _, error in
                MainActor.assumeIsolated {
                    guard let self, self.sockets[id] === socket else { return }
                    if let data {
                        self.call(webDatagram(id: id, data: data))
                    }
                    if error == nil {
                        self.receive(on: socket, id: id)
                    }
                }
            }
        }

        private func call(_ script: String?) {
            guard let script else { return }
            web?.evaluateJavaScript(script, completionHandler: nil)
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

    static var current: WebOrientation {
        let scene = UIApplication.shared.connectedScenes.first { $0 is UIWindowScene } as? UIWindowScene
        switch scene?.effectiveGeometry.interfaceOrientation {
        case .portrait, .portraitUpsideDown: return .portrait
        case .landscapeRight: return .landscapeRight
        default: return .landscapeLeft
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
