import Foundation
import Network

/// Whether the player refused the local network: only a `NWConnection` can
/// tell (TN3179). The refusal cannot be played on a simulator: S6.
enum LocalNetwork {
    /// The connection is thrown away: it only asks.
    @MainActor
    static func denied(host: String, port: UInt16) async -> Bool {
        guard let port = NWEndpoint.Port(rawValue: port) else { return false }
        let probe = NWConnection(host: NWEndpoint.Host(host), port: port, using: .tcp)
        let states = AsyncStream<NWConnection.State> { continuation in
            probe.stateUpdateHandler = { continuation.yield($0) }
        }
        let giveUp = Task {
            try? await Task.sleep(for: .seconds(2))
            probe.cancel()
        }
        defer {
            giveUp.cancel()
            probe.cancel()
        }
        probe.start(queue: .main)
        for await state in states {
            switch state {
            case .setup, .preparing:
                continue
            case .waiting:
                return probe.currentPath?.unsatisfiedReason == .localNetworkDenied
            case .ready, .failed, .cancelled:
                return false
            @unknown default:
                return false
            }
        }
        return false
    }
}
