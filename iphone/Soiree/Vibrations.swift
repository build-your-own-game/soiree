import CoreHaptics
import UIKit

/// Les vibrations : Core Haptics quand l'iPhone le permet, sinon les
/// vibrations toutes faites d'UIKit.
@MainActor
final class Vibrations {
    private var moteur: CHHapticEngine?
    private var enMarche = false
    /// Le motif de `navigator.vibrate` en cours, pour pouvoir l'arrêter.
    private var motif: CHHapticPatternPlayer?

    init() {
        guard CHHapticEngine.capabilitiesForHardware().supportsHaptics,
              let moteur = try? CHHapticEngine()
        else { return }
        moteur.playsHapticsOnly = true
        // Le système arrête le moteur pendant une interruption (un appel, le
        // passage en arrière-plan). On le note, et on le redémarre à la
        // prochaine vibration ou au retour au premier plan.
        moteur.stoppedHandler = { [weak self] _ in
            Task { @MainActor in self?.enMarche = false }
        }
        // Le serveur de vibrations a redémarré : il faut redémarrer le moteur.
        moteur.resetHandler = { [weak self] in
            Task { @MainActor in
                self?.enMarche = false
                self?.reveiller()
            }
        }
        self.moteur = moteur
        reveiller()
    }

    /// (Re)démarre le moteur s'il est arrêté.
    func reveiller() {
        guard let moteur, !enMarche else { return }
        do {
            try moteur.start()
            enMarche = true
        } catch {
            enMarche = false
        }
    }

    func arreter() {
        try? motif?.stop(atTime: CHHapticTimeImmediate)
        motif = nil
        moteur?.stop()
        enMarche = false
    }

    // MARK: window.soiree.haptic

    /// `genre` : "impact", "success", "failure" ou "tick". `intensite` : de 0 à 1.
    func retour(_ genre: String, intensite: Double) {
        let i = Float(min(1, max(0, intensite)))
        guard i > 0 else { return }

        // L'intensité règle la force et la netteté : un coup fort est aussi plus sec.
        let coups: [CHHapticEvent] = switch genre {
        case "tick":
            [coup(a: 0, force: 0.5 * i, nettete: 1)]
        case "success":
            [coup(a: 0, force: 0.6 * i, nettete: 0.5), coup(a: 0.13, force: i, nettete: 0.9)]
        case "failure":
            [
                coup(a: 0, force: i, nettete: 0.3),
                coup(a: 0.11, force: 0.8 * i, nettete: 0.2),
                coup(a: 0.22, force: 0.6 * i, nettete: 0.1),
            ]
        default: // "impact", et tout genre inconnu
            [coup(a: 0, force: i, nettete: 0.3 + 0.6 * i)]
        }
        if jouer(coups) != nil { return }

        // Le repli, sans Core Haptics.
        switch genre {
        case "tick":
            UIImpactFeedbackGenerator(style: .light).impactOccurred(intensity: CGFloat(i))
        case "success":
            UINotificationFeedbackGenerator().notificationOccurred(.success)
        case "failure":
            UINotificationFeedbackGenerator().notificationOccurred(.error)
        default:
            UIImpactFeedbackGenerator(style: .heavy).impactOccurred(intensity: CGFloat(i))
        }
    }

    // MARK: navigator.vibrate

    /// `durees` : le motif du web, en ms : [vibre, pause, vibre, …].
    /// Un motif vide ou nul arrête la vibration en cours.
    func vibrer(_ durees: [Double]) {
        try? motif?.stop(atTime: CHHapticTimeImmediate)
        motif = nil

        // Les débuts et les durées des vibrations, en secondes. Dix secondes au plus.
        var vibrations: [(debut: Double, duree: Double)] = []
        var instant = 0.0
        for (rang, duree) in durees.enumerated() where instant < 10 {
            let secondes = min(max(0, duree) / 1000, 10 - instant)
            if rang.isMultiple(of: 2), secondes > 0 {
                vibrations.append((instant, secondes))
            }
            instant += secondes
        }
        guard !vibrations.isEmpty else { return }

        let evenements = vibrations.map { vibration in
            CHHapticEvent(
                eventType: .hapticContinuous,
                parameters: [
                    CHHapticEventParameter(parameterID: .hapticIntensity, value: 1),
                    CHHapticEventParameter(parameterID: .hapticSharpness, value: 0.5),
                ],
                relativeTime: vibration.debut,
                duration: vibration.duree
            )
        }
        if let lecteur = jouer(evenements) {
            motif = lecteur
            return
        }

        // Le repli : un coup au début de chaque vibration.
        for vibration in vibrations.prefix(8) {
            DispatchQueue.main.asyncAfter(deadline: .now() + vibration.debut) {
                UIImpactFeedbackGenerator(style: .heavy).impactOccurred()
            }
        }
    }

    // MARK: Core Haptics

    private func coup(a instant: TimeInterval, force: Float, nettete: Float) -> CHHapticEvent {
        CHHapticEvent(
            eventType: .hapticTransient,
            parameters: [
                CHHapticEventParameter(parameterID: .hapticIntensity, value: force),
                CHHapticEventParameter(parameterID: .hapticSharpness, value: nettete),
            ],
            relativeTime: instant
        )
    }

    /// Joue des événements tout de suite. Renvoie `nil` si Core Haptics n'a pas pu.
    private func jouer(_ evenements: [CHHapticEvent]) -> CHHapticPatternPlayer? {
        reveiller()
        guard let moteur, enMarche else { return nil }
        do {
            let lecteur = try moteur.makePlayer(with: CHHapticPattern(events: evenements, parameters: []))
            try lecteur.start(atTime: CHHapticTimeImmediate)
            return lecteur
        } catch {
            // Le moteur s'est arrêté sans prévenir : on réessaiera à la prochaine vibration.
            enMarche = false
            return nil
        }
    }
}
