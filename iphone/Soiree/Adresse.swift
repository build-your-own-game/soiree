import Foundation

/// Les adresses que l'app accepte d'ouvrir : seulement du `http` ou du `https`.
///
/// Trois sources passent par ici, avec les mêmes vérifications : un QR code,
/// une adresse tapée ou collée, et le lien `soiree://ouvrir?u=…`.
enum Adresse {
    private static let cleDerniere = "derniereAdresse"

    /// Lit une adresse tapée, collée ou scannée.
    ///
    /// Une adresse sans schéma (`192.168.1.20:8080/t`) est comprise comme du
    /// `http`. Un lien `soiree://ouvrir?u=…` est déplié.
    static func lire(_ texte: String) -> URL? {
        let texte = texte.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !texte.isEmpty, texte.count < 2048 else { return nil }

        if texte.lowercased().hasPrefix("soiree:"), let lien = URL(string: texte) {
            return lireLien(lien)
        }
        let complet = texte.contains("://") ? texte : "http://" + texte
        return verifier(complet)
    }

    /// Lit le lien `soiree://ouvrir?u=<adresse encodée>`.
    static func lireLien(_ lien: URL) -> URL? {
        guard lien.scheme?.lowercased() == "soiree",
              lien.host()?.lowercased() == "ouvrir",
              let composants = URLComponents(url: lien, resolvingAgainstBaseURL: false),
              let adresse = composants.queryItems?.first(where: { $0.name == "u" })?.value
        else { return nil }
        // Ici, pas de schéma deviné : le lien doit nommer une adresse entière.
        return verifier(adresse.trimmingCharacters(in: .whitespacesAndNewlines))
    }

    /// N'accepte qu'une adresse `http` ou `https` avec un hôte.
    private static func verifier(_ texte: String) -> URL? {
        guard let composants = URLComponents(string: texte),
              let schema = composants.scheme?.lowercased(),
              schema == "http" || schema == "https",
              let hote = composants.host, !hote.isEmpty,
              let adresse = composants.url
        else { return nil }
        return adresse
    }

    /// La dernière adresse ouverte, pour « Rejouer ».
    static var derniere: URL? {
        get { UserDefaults.standard.string(forKey: cleDerniere).flatMap(verifier) }
        set { UserDefaults.standard.set(newValue?.absoluteString, forKey: cleDerniere) }
    }
}
