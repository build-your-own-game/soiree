import SwiftUI

/// L'accueil : le scanner de QR code en plein écran, et de quoi ouvrir un jeu
/// sans appareil photo (une adresse à taper, ou la dernière partie).
struct Accueil: View {
    let derniere: URL?
    let ouvrir: (URL) -> Void

    @State private var camera = EtatCamera.attente
    @State private var saisie = ""
    @State private var refus: String?
    @FocusState private var saisieActive: Bool

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            Scanner(etat: $camera, trouve: scanner)
                .ignoresSafeArea()
                .onTapGesture { saisieActive = false }

            if let message = messageCamera {
                VStack(spacing: 12) {
                    Image(systemName: "qrcode.viewfinder")
                        .font(.system(size: 56, weight: .light))
                    Text(message)
                        .font(.callout)
                        .multilineTextAlignment(.center)
                    if camera == .refusee {
                        Button("Ouvrir les Réglages") {
                            if let reglages = URL(string: UIApplication.openSettingsURLString) {
                                UIApplication.shared.open(reglages)
                            }
                        }
                        .font(.callout.weight(.semibold))
                    }
                }
                .foregroundStyle(.white.opacity(0.75))
                .padding(.horizontal, 40)
            }

            VStack(spacing: 0) {
                entete
                Spacer(minLength: 0)
                ouverture
            }
        }
    }

    private var entete: some View {
        VStack(spacing: 6) {
            Text("Soirée")
                .font(.largeTitle.bold())
            Text("Scanne le QR code affiché sur la télé : ton téléphone devient la manette.")
                .font(.callout)
                .multilineTextAlignment(.center)
                .foregroundStyle(.white.opacity(0.85))
        }
        .foregroundStyle(.white)
        .padding(.horizontal, 24)
        .padding(.top, 12)
        .padding(.bottom, 28)
        .frame(maxWidth: .infinity)
        .background(
            LinearGradient(colors: [.black.opacity(0.75), .clear], startPoint: .top, endPoint: .bottom)
                .ignoresSafeArea()
        )
    }

    private var ouverture: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                TextField("Adresse du jeu", text: $saisie, prompt: Text("192.168.1.20:8080/t"))
                    .keyboardType(.URL)
                    .textContentType(.URL)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .submitLabel(.go)
                    .focused($saisieActive)
                    .onSubmit(valider)
                    .padding(.horizontal, 12)
                    .frame(height: 44)
                    .background(.black.opacity(0.35), in: RoundedRectangle(cornerRadius: 10))
                Button("Ouvrir", action: valider)
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)
                    .disabled(saisie.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            if let refus {
                Text(refus)
                    .font(.footnote)
                    .foregroundStyle(.orange)
            }
            if let derniere {
                Button {
                    ouvrir(derniere)
                } label: {
                    Label("Rejouer : \(derniere.absoluteString)", systemImage: "arrow.clockwise")
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .font(.callout)
            }
        }
        .padding(16)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 18))
        .padding(.horizontal, 12)
        .padding(.bottom, 8)
    }

    private var messageCamera: String? {
        switch camera {
        case .attente, .active:
            nil
        case .refusee:
            "L'appareil photo n'est pas autorisé. Autorise-le dans les Réglages, ou tape l'adresse du jeu ci-dessous."
        case .absente:
            "Pas d'appareil photo ici. Tape l'adresse du jeu ci-dessous."
        }
    }

    private func valider() {
        guard let adresse = Adresse.lire(saisie) else {
            refus = "Cette adresse n'est pas valable. Il faut une adresse http ou https, comme celle écrite sous le QR code."
            return
        }
        refus = nil
        ouvrir(adresse)
    }

    private func scanner(_ code: String) {
        guard let adresse = Adresse.lire(code) else {
            refus = "Ce QR code n'est pas l'adresse d'un jeu."
            return
        }
        UINotificationFeedbackGenerator().notificationOccurred(.success)
        ouvrir(adresse)
    }
}
