import Foundation
import Observation
import QuartzCore
import SwiftUI

/// The link to one game: the WebSocket, the Rust session that reads and
/// writes its messages, and what the game asked to show. It reconnects by
/// itself, saying hello again with the same player.
@MainActor
@Observable
final class GameConnection {
    enum Status: Equatable {
        case connecting
        case connected
        /// Lost, or never reached; trying again.
        case reconnecting
        /// Trying again, but the player refused the local network: only they
        /// can change that.
        case localNetworkDenied
        case refused(String)
    }

    enum Content: Equatable {
        case waiting
        case pad
        case web(URL)
    }

    private(set) var status = Status.connecting
    private(set) var content = Content.waiting
    /// The pad's state, while the game shows one.
    private(set) var pad: RemoteModel?
    /// The game's tint, from its last feedback.
    private(set) var tint: Color?
    /// The game's page failed to come, and the web view is trying again.
    private(set) var pageFailing = false

    let target: JoinTarget
    @ObservationIgnored let session: Session
    @ObservationIgnored private var sockets: URLSession?
    @ObservationIgnored private var socket: URLSessionWebSocketTask?
    /// The open handshake is over: frames sent earlier die with a failed try.
    @ObservationIgnored private var opened = false
    @ObservationIgnored private var attempt = 0
    @ObservationIgnored private var ticker: Ticker?
    @ObservationIgnored private let haptics = Haptics()
    @ObservationIgnored private var closed = false
    /// The game answered once: failing now is losing it, not missing it.
    @ObservationIgnored private var reached = false
    @ObservationIgnored private let localNetworkDenied: @MainActor (String, UInt16) async -> Bool

    init(
        target: JoinTarget,
        localNetworkDenied: @escaping @MainActor (String, UInt16) async -> Bool = LocalNetwork.denied
    ) {
        self.target = target
        self.localNetworkDenied = localNetworkDenied
        session = Session(
            player: Self.player, name: Self.name, screen: Size(width: 0, height: 0),
            insets: Insets(top: 0, left: 0, bottom: 0, right: 0))
    }

    func start() {
        closed = false
        let delegate = SocketDelegate { [weak self] socket in
            Task { @MainActor in self?.didOpen(socket) }
        }
        sockets = URLSession(configuration: .default, delegate: delegate, delegateQueue: nil)
        ticker = Ticker { [weak self] in self?.tick() }
        connect()
    }

    /// The player left the game.
    func stop() {
        closed = true
        Orientation.allow(.landscape)
        ticker?.stop()
        ticker = nil
        pad?.stop()
        socket?.cancel(with: .goingAway, reason: nil)
        socket = nil
        sockets?.invalidateAndCancel()
        sockets = nil
    }

    func page(failing: Bool) {
        if pageFailing != failing {
            pageFailing = failing
        }
    }

    private func connect() {
        guard !closed, let sockets, let url = URL(string: target.socket) else { return }
        let socket = sockets.webSocketTask(with: url)
        self.socket = socket
        opened = false
        socket.resume()
        receive(on: socket)
    }

    /// Hello goes first, on a socket that has really opened.
    private func didOpen(_ socket: URLSessionWebSocketTask) {
        guard socket === self.socket, !closed else { return }
        opened = true
        session.connected()
        flush()
    }

    private func receive(on socket: URLSessionWebSocketTask) {
        Task { [weak self] in
            while true {
                let message: URLSessionWebSocketTask.Message
                do {
                    message = try await socket.receive()
                } catch {
                    self?.lost(socket)
                    return
                }
                guard case let .data(frame) = message else { continue }
                self?.handle(frame)
            }
        }
    }

    private func handle(_ frame: Data) {
        attempt = 0
        status = .connected
        reached = true
        switch session.receive(frame: frame) {
        case .showPad:
            Orientation.allow(.landscape)
            if pad == nil {
                pad = RemoteModel(session: session)
            }
            pad?.reload()
            content = .pad
        case let .showWeb(url):
            pad?.stop()
            pad = nil
            if let page = URL(string: target.page), let resolved = URL(string: url, relativeTo: page) {
                content = .web(resolved.absoluteURL)
            }
        case let .feedback(haptic, color):
            haptics.play(haptic)
            tint = color.map { Color(rgb: $0) }
        case let .refused(reason):
            status = .refused(reason)
            stop()
        case .unreadable:
            break
        }
    }

    /// Back-off as Sidequest's phones do: 0.5 s, doubling up to 5 s.
    private func lost(_ socket: URLSessionWebSocketTask) {
        guard socket === self.socket, !closed else { return }
        self.socket = nil
        opened = false
        // A silent socket does not know yet.
        socket.cancel(with: .goingAway, reason: nil)
        session.disconnected()
        pad?.releaseAll()
        if status != .localNetworkDenied {
            status = .reconnecting
        }
        let delay = min(0.5 * pow(2, Double(attempt)), 5)
        attempt += 1
        Task { [weak self] in
            try? await Task.sleep(for: .seconds(delay))
            self?.connect()
        }
        Task { [weak self] in await self?.askAboutTheLocalNetwork() }
    }

    /// `URLSession` says only that it failed; `WKWebView` is exempt from the
    /// permission, so a page would work while `/remote` never can.
    private func askAboutTheLocalNetwork() async {
        guard let url = URL(string: target.socket), let host = url.host() else { return }
        let port = url.port.flatMap(UInt16.init) ?? (url.scheme == "wss" ? 443 : 80)
        let denied = await localNetworkDenied(host, port)
        if status == .reconnecting || status == .localNetworkDenied {
            status = denied ? .localNetworkDenied : .reconnecting
        }
    }

    /// Each screen refresh: what is due goes out.
    private func tick() {
        let now = ProcessInfo.processInfo.systemUptime
        session.tick(now: now)
        flush()
        guard let socket, opened else { return }
        switch session.watch(now: now) {
        case .quiet: break
        case .ping: ping(socket)
        case .lost: lost(socket)
        }
    }

    private func ping(_ socket: URLSessionWebSocketTask) {
        socket.sendPing { [weak self] error in
            Task { @MainActor in
                guard let self, socket === self.socket else { return }
                if error == nil {
                    self.session.pong()
                } else {
                    self.lost(socket)
                }
            }
        }
    }

    /// Only with a socket to take them: the session keeps what waits.
    private func flush() {
        guard let socket, opened else { return }
        for frame in session.drain() {
            socket.send(.data(frame)) { _ in }
        }
    }

    /// What the player reads, if anything: one thing at a time. `Refused`
    /// has its own screen.
    var notice: String? {
        switch status {
        case .connecting:
            "Connexion au jeu…"
        case .reconnecting where reached:
            "Connexion perdue, on réessaie…"
        case .reconnecting:
            "Le jeu ne répond pas, on réessaie…"
        case .localNetworkDenied:
            "Réseau local refusé. Autorise Télécommande dans Réglages > Confidentialité et sécurité > Réseau local."
        case .connected where content == .waiting:
            "Connexion au jeu…"
        case .connected:
            pageFailing ? "La page du jeu ne répond pas, on réessaie…" : nil
        case .refused:
            nil
        }
    }

    /// The first name the player typed on the join screen, or "".
    static var name: String {
        get { UserDefaults.standard.string(forKey: "name") ?? "" }
        set { UserDefaults.standard.set(newValue, forKey: "name") }
    }

    /// 16 random bytes, made once and kept: the player's identity in every
    /// game, without an account.
    static var player: Data {
        let key = "player"
        if let saved = UserDefaults.standard.data(forKey: key), saved.count == 16 {
            return saved
        }
        let made = withUnsafeBytes(of: UUID().uuid) { Data($0) }
        UserDefaults.standard.set(made, forKey: key)
        return made
    }
}

/// Says when a socket has finished opening.
private final class SocketDelegate: NSObject, URLSessionWebSocketDelegate, Sendable {
    private let onOpen: @Sendable (URLSessionWebSocketTask) -> Void

    init(onOpen: @escaping @Sendable (URLSessionWebSocketTask) -> Void) {
        self.onOpen = onOpen
    }

    func urlSession(
        _ session: URLSession, webSocketTask: URLSessionWebSocketTask, didOpenWithProtocol protocol: String?
    ) {
        onOpen(webSocketTask)
    }
}

/// Calls back once per screen refresh, on the main actor.
@MainActor
final class Ticker {
    private var link: CADisplayLink?
    private let onTick: () -> Void

    init(onTick: @escaping () -> Void) {
        self.onTick = onTick
        let link = CADisplayLink(target: Target(self), selector: #selector(Target.fire))
        link.add(to: .main, forMode: .common)
        self.link = link
    }

    /// The display link runs until stopped, whoever still holds the ticker.
    func stop() {
        link?.invalidate()
        link = nil
    }

    fileprivate func fire() {
        onTick()
    }

    /// CADisplayLink retains its target: a weak hop avoids a cycle.
    private final class Target: NSObject {
        weak var ticker: Ticker?

        init(_ ticker: Ticker) {
            self.ticker = ticker
        }

        @MainActor @objc func fire() {
            ticker?.fire()
        }
    }
}
