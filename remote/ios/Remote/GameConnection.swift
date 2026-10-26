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
        /// Lost; trying again.
        case reconnecting
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

    let target: JoinTarget
    @ObservationIgnored let session: Session
    @ObservationIgnored private var socket: URLSessionWebSocketTask?
    @ObservationIgnored private var attempt = 0
    @ObservationIgnored private var ticker: Ticker?
    @ObservationIgnored private let haptics = Haptics()
    @ObservationIgnored private var closed = false

    init(target: JoinTarget) {
        self.target = target
        session = Session(
            player: Self.player, name: "", screen: Size(width: 0, height: 0),
            insets: Insets(top: 0, left: 0, bottom: 0, right: 0))
    }

    func start() {
        closed = false
        ticker = Ticker { [weak self] in self?.tick() }
        connect()
    }

    /// The player left the game.
    func stop() {
        closed = true
        ticker?.stop()
        ticker = nil
        pad?.stop()
        socket?.cancel(with: .goingAway, reason: nil)
        socket = nil
    }

    private func connect() {
        guard !closed, let url = URL(string: target.socket) else { return }
        let socket = URLSession.shared.webSocketTask(with: url)
        self.socket = socket
        socket.resume()
        session.connected()
        flush()
        receive(on: socket)
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
        switch session.receive(frame: frame) {
        case .showPad:
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
        pad?.releaseAll()
        status = .reconnecting
        let delay = min(0.5 * pow(2, Double(attempt)), 5)
        attempt += 1
        Task { [weak self] in
            try? await Task.sleep(for: .seconds(delay))
            self?.connect()
        }
    }

    /// Each screen refresh: what is due goes out.
    private func tick() {
        session.tick(now: ProcessInfo.processInfo.systemUptime)
        flush()
    }

    private func flush() {
        let frames = session.drain()
        guard let socket, socket.state == .running else { return }
        for frame in frames {
            socket.send(.data(frame)) { _ in }
        }
    }

    /// 16 random bytes, made once and kept: the player's identity in every
    /// game, without an account.
    private static var player: Data {
        let key = "player"
        if let saved = UserDefaults.standard.data(forKey: key), saved.count == 16 {
            return saved
        }
        let made = withUnsafeBytes(of: UUID().uuid) { Data($0) }
        UserDefaults.standard.set(made, forKey: key)
        return made
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
