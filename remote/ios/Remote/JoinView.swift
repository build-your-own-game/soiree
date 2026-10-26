import SwiftUI
import VisionKit

/// The app's only screen of its own: scan the TV's QR code. The game then
/// decides everything else. Without a camera (the Simulator), the game's
/// address can be typed instead.
struct JoinView: View {
    let onJoin: (JoinTarget) -> Void
    @State private var address = ""
    @State private var name = GameConnection.name
    @State private var problem: String?

    var body: some View {
        ZStack {
            Palette.body.ignoresSafeArea()
            HStack(spacing: 28) {
                if QRScanner.isAvailable {
                    QRScanner { text in join(text) }
                        .clipShape(RoundedRectangle(cornerRadius: 20))
                        .overlay(RoundedRectangle(cornerRadius: 20).stroke(.white.opacity(0.2), lineWidth: 2))
                        .aspectRatio(1, contentMode: .fit)
                }
                VStack(alignment: .leading, spacing: 14) {
                    Text("Télécommande")
                        .font(.system(size: 28, weight: .black, design: .rounded))
                    Text(QRScanner.isAvailable ? "Scanne le QR code affiché sur la TV." : "Entre l'adresse du jeu affichée sur la TV.")
                        .font(.system(size: 17, weight: .medium, design: .rounded))
                        .foregroundStyle(.white.opacity(0.7))
                    TextField(
                        "Prénom", text: $name,
                        prompt: Text("Ton prénom").foregroundStyle(.white.opacity(0.35)))
                        .textContentType(.givenName)
                        .submitLabel(.done)
                        .onChange(of: name) { _, typed in GameConnection.name = String(typed.prefix(24)) }
                        .padding(10)
                        .background(.white.opacity(0.1), in: RoundedRectangle(cornerRadius: 10))
                    HStack {
                        TextField(
                            "Adresse", text: $address,
                            prompt: Text("Adresse du jeu").foregroundStyle(.white.opacity(0.35)))
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .keyboardType(.URL)
                            .submitLabel(.join)
                            .onSubmit { join(address) }
                            .padding(10)
                            .background(.white.opacity(0.1), in: RoundedRectangle(cornerRadius: 10))
                        SwiftUI.Button("Rejoindre") { join(address) }
                            .buttonStyle(.borderedProminent)
                    }
                    if let problem {
                        Text(problem).foregroundStyle(Palette.warning)
                    }
                }
                .foregroundStyle(.white)
                .frame(maxWidth: 420)
            }
            .padding(24)
        }
        .statusBarHidden()
    }

    private func join(_ text: String) {
        if let target = joinTarget(text: text) {
            problem = nil
            onJoin(target)
        } else {
            problem = "Ce n'est pas l'adresse d'un jeu."
        }
    }
}

/// VisionKit's live QR code scanner. The first code that reads as a game
/// joins it.
private struct QRScanner: UIViewControllerRepresentable {
    let onCode: (String) -> Void

    static var isAvailable: Bool {
        DataScannerViewController.isSupported && DataScannerViewController.isAvailable
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(onCode: onCode)
    }

    func makeUIViewController(context: Context) -> DataScannerViewController {
        let scanner = DataScannerViewController(
            recognizedDataTypes: [.barcode(symbologies: [.qr])],
            qualityLevel: .fast,
            isHighlightingEnabled: true)
        scanner.delegate = context.coordinator
        Task { try? scanner.startScanning() }
        return scanner
    }

    func updateUIViewController(_ scanner: DataScannerViewController, context: Context) {}

    static func dismantleUIViewController(_ scanner: DataScannerViewController, coordinator: Coordinator) {
        scanner.stopScanning()
    }

    @MainActor
    final class Coordinator: NSObject, DataScannerViewControllerDelegate {
        let onCode: (String) -> Void

        init(onCode: @escaping (String) -> Void) {
            self.onCode = onCode
        }

        func dataScanner(
            _ scanner: DataScannerViewController, didAdd items: [RecognizedItem], allItems: [RecognizedItem]
        ) {
            for case let .barcode(code) in items {
                if let text = code.payloadStringValue, joinTarget(text: text) != nil {
                    onCode(text)
                    return
                }
            }
        }
    }
}
