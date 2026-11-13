import AVFoundation
import SwiftUI

/// Ce que l'accueil doit savoir de l'appareil photo.
enum EtatCamera {
    case attente
    case active
    /// Le joueur a refusé l'appareil photo.
    case refusee
    /// Pas d'appareil photo (le simulateur, par exemple).
    case absente
}

/// Le scanner de QR code : l'image de l'appareil photo, en plein écran.
struct Scanner: UIViewRepresentable {
    @Binding var etat: EtatCamera
    /// Reçoit le texte de chaque QR code vu.
    let trouve: (String) -> Void

    func makeUIView(context: Context) -> VueScanner {
        let vue = VueScanner()
        vue.trouve = trouve
        vue.etatChange = { nouvel in etat = nouvel }
        vue.demarrer()
        return vue
    }

    func updateUIView(_ vue: VueScanner, context: Context) {
        vue.trouve = trouve
    }

    static func dismantleUIView(_ vue: VueScanner, coordinator: ()) {
        vue.arreter()
    }
}

final class VueScanner: UIView, AVCaptureMetadataOutputObjectsDelegate {
    var trouve: (String) -> Void = { _ in }
    var etatChange: (EtatCamera) -> Void = { _ in }

    private let session = AVCaptureSession()
    /// `startRunning` bloque : la session se pilote hors du fil principal.
    private let file = DispatchQueue(label: "soiree.scanner")
    private var dernierCode = ""
    private var dernierInstant = Date.distantPast

    override class var layerClass: AnyClass { AVCaptureVideoPreviewLayer.self }

    private var apercu: AVCaptureVideoPreviewLayer? { layer as? AVCaptureVideoPreviewLayer }

    func demarrer() {
        backgroundColor = .black
        guard AVCaptureDevice.default(for: .video) != nil else {
            signaler(.absente)
            return
        }
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            configurer()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { [weak self] accorde in
                DispatchQueue.main.async {
                    if accorde {
                        self?.configurer()
                    } else {
                        self?.signaler(.refusee)
                    }
                }
            }
        default:
            signaler(.refusee)
        }
    }

    func arreter() {
        let session = session
        file.async {
            if session.isRunning {
                session.stopRunning()
            }
        }
    }

    private func configurer() {
        guard let camera = AVCaptureDevice.default(for: .video),
              let entree = try? AVCaptureDeviceInput(device: camera),
              session.canAddInput(entree)
        else {
            signaler(.absente)
            return
        }
        let sortie = AVCaptureMetadataOutput()
        guard session.canAddOutput(sortie) else {
            signaler(.absente)
            return
        }
        session.beginConfiguration()
        session.addInput(entree)
        session.addOutput(sortie)
        // Après l'ajout : les types disponibles dépendent de l'entrée.
        sortie.setMetadataObjectsDelegate(self, queue: .main)
        if sortie.availableMetadataObjectTypes.contains(.qr) {
            sortie.metadataObjectTypes = [.qr]
        }
        session.commitConfiguration()

        apercu?.session = session
        apercu?.videoGravity = .resizeAspectFill

        let session = session
        file.async { session.startRunning() }
        signaler(.active)
    }

    /// Prévient l'accueil au tour suivant : jamais pendant que SwiftUI dessine.
    private func signaler(_ etat: EtatCamera) {
        DispatchQueue.main.async { [weak self] in self?.etatChange(etat) }
    }

    nonisolated func metadataOutput(
        _ output: AVCaptureMetadataOutput,
        didOutput metadataObjects: [AVMetadataObject],
        from connection: AVCaptureConnection
    ) {
        guard let code = (metadataObjects.first as? AVMetadataMachineReadableCodeObject)?.stringValue else { return }
        // La sortie livre sur le fil principal (voir `configurer`).
        MainActor.assumeIsolated { recevoir(code) }
    }

    private func recevoir(_ code: String) {
        // L'appareil photo revoit le même code à chaque image : une fois toutes les deux secondes suffit.
        let maintenant = Date()
        if code == dernierCode, maintenant.timeIntervalSince(dernierInstant) < 2 { return }
        dernierCode = code
        dernierInstant = maintenant
        trouve(code)
    }
}
