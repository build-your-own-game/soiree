import SwiftUI

/// One game, full screen: what the game asked to show, a word while it
/// connects, and a way out.
struct GameScreen: View {
    let onLeave: () -> Void
    @State private var connection: GameConnection
    @State private var confirmingLeave = false
    @Environment(\.scenePhase) private var scenePhase

    init(target: JoinTarget, onLeave: @escaping () -> Void) {
        self.onLeave = onLeave
        _connection = State(initialValue: GameConnection(target: target))
    }

    var body: some View {
        ZStack {
            Palette.body.overlay(
                RadialGradient(
                    colors: [connection.tint ?? Palette.bodyLight, Palette.body],
                    center: .top, startRadius: 0, endRadius: 700))
            content
            if connection.status == .reconnecting {
                Notice(text: "Connexion perdue, on réessaie…")
            }
            if case let .refused(reason) = connection.status {
                Refused(reason: reason, onLeave: onLeave)
            }
            LeaveButton { confirmingLeave = true }
        }
        .ignoresSafeArea()
        .confirmationDialog("Quitter la partie ?", isPresented: $confirmingLeave, titleVisibility: .visible) {
            SwiftUI.Button("Quitter", role: .destructive) {
                connection.stop()
                onLeave()
            }
            SwiftUI.Button("Continuer à jouer", role: .cancel) {}
        }
        .statusBarHidden()
        .persistentSystemOverlays(.hidden)
        .defersSystemGestures(on: .all)
        .onAppear {
            UIApplication.shared.isIdleTimerDisabled = true
            connection.start()
        }
        .onDisappear { connection.stop() }
        .onChange(of: scenePhase) { _, phase in
            if phase != .active { connection.pad?.releaseAll() }
        }
    }

    @ViewBuilder private var content: some View {
        switch connection.content {
        case .waiting:
            Notice(text: "Connexion au jeu…")
        case .pad:
            if let pad = connection.pad {
                PadScreen(model: pad)
            }
        case let .web(url):
            WebRemote(url: url).ignoresSafeArea()
        }
    }
}

private struct Notice: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 15, weight: .semibold, design: .rounded))
            .foregroundStyle(.white.opacity(0.8))
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .background(.black.opacity(0.5), in: Capsule())
            .allowsHitTesting(false)
    }
}

private struct Refused: View {
    let reason: String
    let onLeave: () -> Void

    var body: some View {
        VStack(spacing: 16) {
            Text("Le jeu refuse ce téléphone")
                .font(.system(size: 20, weight: .bold, design: .rounded))
            Text(reason).foregroundStyle(.white.opacity(0.7))
            SwiftUI.Button("Retour", action: onLeave).buttonStyle(.borderedProminent)
        }
        .foregroundStyle(.white)
        .padding(28)
        .background(Palette.body, in: RoundedRectangle(cornerRadius: 20))
    }
}

/// A tap asks before leaving: a thumb brushing it in a game only opens a
/// question. It sits in the top-left corner: in the notch's margin on
/// iPhones that have one, beside the controls.
private struct LeaveButton: View {
    let action: () -> Void

    var body: some View {
        SwiftUI.Button(action: action) {
            Image(systemName: "xmark")
                .font(.system(size: 12, weight: .bold))
                .foregroundStyle(.white.opacity(0.5))
                .frame(width: 30, height: 30)
                .background(.white.opacity(0.1), in: Circle())
        }
        .buttonStyle(.plain)
        .position(x: 21, y: 21)
        .accessibilityLabel("Quitter la partie")
    }
}
