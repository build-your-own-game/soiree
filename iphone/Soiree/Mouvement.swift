import CoreMotion
import Foundation

/// Un échantillon de mouvement, dans les unités et les signes des événements
/// `devicemotion` du web.
///
/// ## Ce qui a été retenu, et pourquoi
///
/// Les axes de CoreMotion sont ceux du web : téléphone tenu en portrait face à
/// soi, x va vers la droite de l'écran, y vers le haut, z sort de l'écran.
///
/// **Accélérations : exactement le calcul de Safari sur iOS.** WebKit
/// (`Source/WebCore/platform/ios/WebCoreMotionManager.mm`, `sendMotionData`)
/// fait, à partir du même `CMDeviceMotion` :
///
/// ```
/// acceleration                 = userAcceleration             × 9,80665
/// accelerationIncludingGravity = (userAcceleration + gravity) × 9,80665
/// ```
///
/// sans changer aucun signe. On fait pareil : les signes sont donc ceux de
/// Safari par construction. Conséquence connue : téléphone posé à plat, écran
/// vers le haut, `accelerationIncludingGravity.z` vaut −9,81 (Android et la
/// norme W3C donnent +9,81). Une page écrite pour Safari iOS n'a rien à changer.
///
/// **Vitesses de rotation : en deg/s, signes de CoreMotion** (règle de la main
/// droite autour de chaque axe), comme Safari. Pour le nom des axes, on suit
/// Safari iOS et non la norme W3C : `DeviceMotionClientIOS.mm` range les
/// rotations autour de x, y, z dans `alpha`, `beta`, `gamma`, dans cet ordre
/// (la norme dit z, x, y). Une page se comporte ainsi de la même façon dans
/// l'app et dans Safari sur iPhone.
///
/// **`interval` : en millisecondes** (10 ms à 100 Hz), comme le dit le contrat
/// et la norme. Safari iOS y met une constante à lui.
struct Echantillon: Sendable {
    /// L'accélération de la pesanteur que WebKit utilise, en m/s² par g.
    static let pesanteur = 9.80665

    var ax, ay, az: Double
    var gx, gy, gz: Double
    var alpha, beta, gamma: Double
    var intervalle: Double

    init(_ mouvement: CMDeviceMotion, intervalle: Double) {
        let geste = mouvement.userAcceleration // en g, sans la gravité
        let gravite = mouvement.gravity // en g
        let rotation = mouvement.rotationRate // en rad/s
        let g = Echantillon.pesanteur
        let degres = 180 / Double.pi

        ax = geste.x * g
        ay = geste.y * g
        az = geste.z * g
        gx = (geste.x + gravite.x) * g
        gy = (geste.y + gravite.y) * g
        gz = (geste.z + gravite.z) * g
        alpha = rotation.x * degres // autour de x, comme Safari iOS (la norme W3C dit z)
        beta = rotation.y * degres // autour de y, comme Safari iOS (la norme dit x)
        gamma = rotation.z * degres // autour de z, comme Safari iOS (la norme dit y)
        self.intervalle = intervalle
    }

    #if targetEnvironment(simulator)
    /// Un téléphone immobile, tenu droit en portrait : pour le simulateur.
    static func immobile(intervalle: Double) -> Echantillon {
        Echantillon(gy: -pesanteur, intervalle: intervalle)
    }

    private init(gy: Double, intervalle: Double) {
        ax = 0; ay = 0; az = 0
        gx = 0; self.gy = gy; gz = 0
        alpha = 0; beta = 0; gamma = 0
        self.intervalle = intervalle
    }
    #endif

    /// L'appel de la fonction installée par `pont.js` : court, sans objet à construire.
    var appel: String {
        let valeurs = [ax, ay, az, gx, gy, gz, alpha, beta, gamma, intervalle]
        return "__soireeMouvement(" + valeurs.map { String(format: "%.4f", $0) }.joined(separator: ",") + ")"
    }
}

/// Les capteurs de mouvement, lus à 100 Hz par CoreMotion.
@MainActor
final class Mouvement {
    /// Le temps entre deux échantillons, en secondes.
    static let periode = 1.0 / 100.0

    private let capteurs = CMMotionManager()
    private(set) var enMarche = false
    #if targetEnvironment(simulator)
    private var minuteur: Timer?
    #endif

    /// Démarre les capteurs. `recevoir` est appelée sur le fil principal.
    func demarrer(_ recevoir: @escaping @MainActor (Echantillon) -> Void) {
        guard !enMarche else { return }
        enMarche = true
        let intervalle = Mouvement.periode * 1000

        #if targetEnvironment(simulator)
        // Le simulateur n'a pas de capteurs : on envoie un téléphone immobile,
        // pour qu'une page puisse au moins voir arriver des événements.
        minuteur = Timer.scheduledTimer(withTimeInterval: Mouvement.periode, repeats: true) { _ in
            MainActor.assumeIsolated { recevoir(.immobile(intervalle: intervalle)) }
        }
        #else
        guard capteurs.isDeviceMotionAvailable else { return }
        capteurs.deviceMotionUpdateInterval = Mouvement.periode
        capteurs.startDeviceMotionUpdates(to: .main) { mouvement, _ in
            guard let mouvement else { return }
            let echantillon = Echantillon(mouvement, intervalle: intervalle)
            MainActor.assumeIsolated { recevoir(echantillon) }
        }
        #endif
    }

    func arreter() {
        guard enMarche else { return }
        enMarche = false
        #if targetEnvironment(simulator)
        minuteur?.invalidate()
        minuteur = nil
        #else
        capteurs.stopDeviceMotionUpdates()
        #endif
    }
}
