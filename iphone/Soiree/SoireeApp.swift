import AVFoundation
import SwiftUI

/// Une partie ouverte : l'adresse du jeu. Chaque ouverture a son identité,
/// pour qu'ouvrir deux fois la même adresse recharge bien la page.
struct Partie: Identifiable, Equatable {
    let id = UUID()
    let adresse: URL
}

@main
struct SoireeApp: App {
    @State private var partie: Partie?
    @State private var derniere: URL? = Adresse.derniere

    init() {
        // Le son d'un jeu doit sonner même quand l'iPhone est en mode silencieux.
        try? AVAudioSession.sharedInstance().setCategory(.playback)
    }

    var body: some Scene {
        WindowGroup {
            ZStack {
                if let partie {
                    Jeu(adresse: partie.adresse) { self.partie = nil }
                        .id(partie.id)
                } else {
                    Accueil(derniere: derniere, ouvrir: ouvrir)
                }
            }
            .preferredColorScheme(.dark)
            // Le bouton « Ouvrir dans l'app » des pages vues dans Safari.
            .onOpenURL { lien in
                if let adresse = Adresse.lireLien(lien) {
                    ouvrir(adresse)
                }
            }
        }
    }

    private func ouvrir(_ adresse: URL) {
        Adresse.derniere = adresse
        derniere = adresse
        partie = Partie(adresse: adresse)
    }
}
