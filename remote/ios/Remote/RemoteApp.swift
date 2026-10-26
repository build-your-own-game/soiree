import SwiftUI

/// The player never picks a remote: they join a game, by scanning its QR
/// code or through a `telecommande://join?url=…` link from its page, and the
/// game shows the remote it wants.
@main
struct RemoteApp: App {
    @State private var target: JoinTarget?

    var body: some Scene {
        WindowGroup {
            Group {
                if let target {
                    GameScreen(target: target) { self.target = nil }
                        .id(target.socket)
                } else {
                    JoinView { target = $0 }
                }
            }
            .onOpenURL { url in
                if let joined = joinTarget(text: url.absoluteString) {
                    target = joined
                }
            }
        }
    }
}
