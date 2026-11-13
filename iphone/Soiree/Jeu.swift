import SwiftUI
import WebKit

/// L'écran d'un jeu : sa page en plein écran, un petit bouton pour quitter,
/// et un écran d'erreur quand la page ne charge pas.
struct Jeu: View {
    @StateObject private var page: PageJeu
    private let quitter: () -> Void

    @State private var confirmation = false
    @Environment(\.scenePhase) private var phase

    init(adresse: URL, quitter: @escaping () -> Void) {
        _page = StateObject(wrappedValue: PageJeu(adresse: adresse))
        self.quitter = quitter
    }

    var body: some View {
        ZStack(alignment: .topTrailing) {
            Color.black.ignoresSafeArea()
            VueWeb(vue: page.vue)
                .ignoresSafeArea()
                .opacity(page.etat == .prete ? 1 : 0)

            switch page.etat {
            case .prete:
                EmptyView()
            case .chargement:
                ProgressView("Connexion au jeu…")
                    .tint(.white)
                    .foregroundStyle(.white.opacity(0.8))
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            case .erreur(let message):
                erreur(message)
            }

            boutonQuitter
        }
        .statusBarHidden()
        .onAppear {
            // L'écran reste allumé pendant un jeu.
            UIApplication.shared.isIdleTimerDisabled = true
            page.charger()
        }
        .onDisappear {
            UIApplication.shared.isIdleTimerDisabled = false
            page.fermer()
        }
        .onChange(of: phase) { _, nouvelle in
            page.premierPlan(nouvelle == .active)
        }
        // Une confirmation, pour ne pas quitter en plein coup.
        .confirmationDialog("Quitter la partie ?", isPresented: $confirmation, titleVisibility: .visible) {
            Button("Quitter", role: .destructive, action: quitter)
            Button("Rester", role: .cancel) {}
        }
    }

    private var boutonQuitter: some View {
        Button {
            confirmation = true
        } label: {
            Image(systemName: "xmark")
                .font(.system(size: 11, weight: .bold))
                .foregroundStyle(.white.opacity(0.7))
                .frame(width: 28, height: 28)
                .background(.black.opacity(0.35), in: Circle())
                .padding(4)
                .contentShape(Circle())
        }
        .padding(.trailing, 8)
        .accessibilityLabel("Quitter la partie")
    }

    private func erreur(_ message: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "wifi.exclamationmark")
                .font(.system(size: 48, weight: .light))
            Text("Le jeu ne s'ouvre pas")
                .font(.title2.bold())
            Text(message)
                .font(.callout)
                .multilineTextAlignment(.center)
                .foregroundStyle(.white.opacity(0.8))
            Text(page.adresse.absoluteString)
                .font(.footnote.monospaced())
                .foregroundStyle(.white.opacity(0.5))
                .lineLimit(2)
            Button("Réessayer") { page.charger() }
                .buttonStyle(.borderedProminent)
                .foregroundStyle(.black)
                .controlSize(.large)
                .padding(.top, 8)
            Button("Revenir au scanner", action: quitter)
                .font(.callout)
        }
        .foregroundStyle(.white)
        .padding(.horizontal, 32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// La vue web de la page, posée telle quelle dans SwiftUI.
private struct VueWeb: UIViewRepresentable {
    let vue: WKWebView

    func makeUIView(context: Context) -> WKWebView { vue }

    func updateUIView(_ vue: WKWebView, context: Context) {}
}
