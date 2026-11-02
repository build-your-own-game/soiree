import Network
import SwiftUI
import WebKit

/// The bench's app: the real `GameScreen`, `GameConnection`, `RemoteModel`
/// and `WebRemote`, in a real window of a real simulator, with this file in
/// place of `RemoteApp.swift`. A Rust test tells it what a player would do,
/// one line at a time, over a socket; the game it joins is also the test's.
/// Touches enter at `RemoteModel.handle`, where `TouchSurface` hands them over.
@main
struct BenchApp: App {
    @UIApplicationDelegateAdaptor private var delegate: AppDelegate
    @State private var bench = Bench()

    var body: some Scene {
        WindowGroup {
            Group {
                if let connection = bench.connection {
                    GameScreen(connection: connection) { bench.connection = nil }
                } else {
                    Color.black.ignoresSafeArea()
                }
            }
            .onAppear { bench.start() }
        }
    }
}

@MainActor
@Observable
final class Bench {
    var connection: GameConnection?
    @ObservationIgnored private var control: NWConnection?
    @ObservationIgnored private var received = Data()

    func start() {
        guard control == nil,
            let port = ProcessInfo.processInfo.environment["BENCH_CONTROL"].flatMap(UInt16.init),
            let port = NWEndpoint.Port(rawValue: port)
        else { return }
        let control = NWConnection(host: "127.0.0.1", port: port, using: .tcp)
        self.control = control
        control.start(queue: .main)
        readLine()
    }

    private func readLine() {
        control?.receive(minimumIncompleteLength: 1, maximumLength: 65536) { [weak self] data, _, done, _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                if let data {
                    self.received.append(data)
                }
                self.answerLines()
                if !done {
                    self.readLine()
                }
            }
        }
    }

    private func answerLines() {
        while let end = received.firstIndex(of: 10) {
            let line = String(decoding: received[received.startIndex..<end], as: UTF8.self)
            received = Data(received[received.index(after: end)...])
            Task {
                let answer = await run(line)
                let clean = answer.replacingOccurrences(of: "\n", with: " ")
                control?.send(content: Data((clean + "\n").utf8), completion: .idempotent)
            }
        }
    }

    private func run(_ line: String) async -> String {
        let words = line.split(separator: " ", maxSplits: 1).map(String.init)
        let rest = words.count > 1 ? words[1] : ""
        let numbers = rest.split(separator: " ").compactMap { Double($0) }
        switch words.first ?? "" {
        case "join":
            guard let target = joinTarget(text: rest) else { return "not a game" }
            connection = GameConnection(target: target)
            return "ok"
        case "join-denied":
            guard let target = joinTarget(text: rest) else { return "not a game" }
            connection = GameConnection(target: target) { _, _ in true }
            return "ok"
        case "notice":
            return connection?.notice ?? "none"
        case "status":
            guard let status = connection?.status else { return "none" }
            return switch status {
            case .connecting: "connecting"
            case .connected: "connected"
            case .reconnecting: "reconnecting"
            case .localNetworkDenied: "denied"
            case .refused: "refused"
            }
        case "content":
            guard let content = connection?.content else { return "none" }
            return switch content {
            case .waiting: "waiting"
            case .pad: "pad"
            case let .web(url): "web \(url.absoluteString)"
            }
        case "page-failing":
            return String(connection?.pageFailing ?? false)
        case "turned":
            // The rotation to landscape has reached the pad's layout. Before
            // that, a resize would lift the fingers a test just put down.
            guard let area = connection?.pad?.layout?.area else { return "no" }
            let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
            let landscape = scene?.effectiveGeometry.interfaceOrientation.isLandscape == true
            guard landscape, area.right > area.bottom else { return "no" }
            return "\(area.left) \(area.top) \(area.right) \(area.bottom)"
        case "idle":
            return String(UIApplication.shared.isIdleTimerDisabled)
        case "orientation":
            return "\(Orientation.current)"
        case "landscape-left":
            return String(connection?.pad?.landscapeLeft ?? false)
        case "half-turn":
            // What holding the phone the other way up does: same size, same
            // margins. The rotation's own animation does not matter here.
            guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else { return "no scene" }
            let mask: UIInterfaceOrientationMask =
                scene.effectiveGeometry.interfaceOrientation == .landscapeLeft ? .landscapeRight : .landscapeLeft
            scene.requestGeometryUpdate(.iOS(interfaceOrientations: mask)) { _ in }
            return "ok"
        case "control":
            let id = UInt8(numbers.first ?? 0)
            let found = connection?.pad?.layout?.controls.first { $0.id == id }
            return found.map { "\($0.center.x) \($0.center.y)" } ?? "none"
        case "stick":
            let found = connection?.pad?.layout?.sticks.first
            return found.map { "\($0.center.x) \($0.center.y)" } ?? "none"
        case "canvas":
            let found = connection?.pad?.layout?.canvas
            return found.map { "\(($0.left + $0.right) / 2) \(($0.top + $0.bottom) / 2)" } ?? "none"
        case "strokes":
            return String(connection?.pad?.strokes.count ?? -1)
        case "down":
            connection?.pad?.handle(.down(id: UInt64(numbers[0]), at: point(numbers), time: uptime))
            return "ok"
        case "move":
            connection?.pad?.handle(.move(id: UInt64(numbers[0]), at: point(numbers), time: uptime))
            return "ok"
        case "up":
            connection?.pad?.handle(.up(id: UInt64(numbers[0])))
            return "ok"
        case "tap":
            // Down and up with no refresh between them: a press under a frame.
            connection?.pad?.handle(.down(id: UInt64(numbers[0]), at: point(numbers), time: uptime))
            connection?.pad?.handle(.up(id: UInt64(numbers[0])))
            return "ok"
        case "leave":
            connection?.stop()
            connection = nil
            return "ok"
        case "eval":
            return await evaluate(rest)
        case "kill-page":
            // WebKit's own test hook: the shell cannot reach this process.
            webView?.perform(NSSelectorFromString("_killWebContentProcess"))
            return "ok"
        default:
            return "unknown: \(line)"
        }
    }

    private var uptime: TimeInterval { ProcessInfo.processInfo.systemUptime }

    private func point(_ numbers: [Double]) -> CGPoint {
        CGPoint(x: numbers[1], y: numbers[2])
    }

    private func evaluate(_ script: String) async -> String {
        guard let web = webView else { return "no page" }
        do {
            let value = try await web.evaluateJavaScript(script)
            return value.map { String(describing: $0) } ?? "nil"
        } catch {
            return "error: \(error)"
        }
    }

    var webView: WKWebView? {
        for case let scene as UIWindowScene in UIApplication.shared.connectedScenes {
            for window in scene.windows {
                if let found = find(in: window) { return found }
            }
        }
        return nil
    }

    private func find(in view: UIView) -> WKWebView? {
        if let web = view as? WKWebView { return web }
        for subview in view.subviews {
            if let found = find(in: subview) { return found }
        }
        return nil
    }
}
