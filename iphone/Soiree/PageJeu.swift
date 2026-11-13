import UIKit
import WebKit

/// La page d'un jeu : la vue web, le pont vers la page, les capteurs et les
/// vibrations.
@MainActor
final class PageJeu: NSObject, ObservableObject {
    enum Etat: Equatable {
        case chargement
        case prete
        case erreur(String)
    }

    @Published private(set) var etat = Etat.chargement

    let adresse: URL
    let vue: WKWebView

    private let mouvement = Mouvement()
    private let vibrations = Vibrations()
    /// La dernière page affichée : c'est elle qu'on recharge.
    private var page: URL?
    /// Un document du jeu est affiché, avec le pont installé.
    private var chargee = false
    private var auPremierPlan = true
    private var fermee = false
    /// La page a été perdue pendant que l'app était en arrière-plan.
    private var perdue = false
    /// Les arrêts de la page depuis le dernier chargement réussi.
    private var arrets = 0

    init(adresse: URL) {
        self.adresse = adresse

        let configuration = WKWebViewConfiguration()
        configuration.allowsInlineMediaPlayback = true
        configuration.mediaTypesRequiringUserActionForPlayback = [] // le son sans geste préalable
        for (fichier, moment) in [("pont", WKUserScriptInjectionTime.atDocumentStart), ("reglages", .atDocumentEnd)] {
            guard let chemin = Bundle.main.url(forResource: fichier, withExtension: "js"),
                  let source = try? String(contentsOf: chemin, encoding: .utf8)
            else { continue }
            configuration.userContentController.addUserScript(
                WKUserScript(source: source, injectionTime: moment, forMainFrameOnly: true)
            )
        }

        vue = WKWebView(frame: .zero, configuration: configuration)
        super.init()

        configuration.userContentController.add(Relais(self), name: "soiree")
        vue.navigationDelegate = self
        vue.uiDelegate = self
        vue.allowsLinkPreview = false
        vue.allowsBackForwardNavigationGestures = false
        // Pas d'éclair blanc avant la page.
        vue.isOpaque = false
        vue.backgroundColor = .black
        vue.scrollView.backgroundColor = .black
        // La page s'étend sous l'encoche et gère elle-même les zones sûres (`viewport-fit=cover`).
        vue.scrollView.contentInsetAdjustmentBehavior = .never
        vue.scrollView.bounces = false
        vue.scrollView.alwaysBounceVertical = false
        vue.scrollView.alwaysBounceHorizontal = false
        vue.scrollView.delegate = self
    }

    // MARK: Charger

    /// Charge le jeu, ou le recharge là où il en était.
    func charger() {
        guard !fermee else { return }
        perdue = false
        etat = .chargement
        // Dix secondes : sur le mauvais Wi-Fi, mieux vaut un écran d'erreur qu'une longue attente.
        vue.load(URLRequest(url: page ?? adresse, cachePolicy: .useProtocolCachePolicy, timeoutInterval: 10))
    }

    /// L'app passe au premier plan ou le quitte.
    func premierPlan(_ actif: Bool) {
        auPremierPlan = actif
        if actif {
            vibrations.reveiller()
            if case .erreur = etat {
                charger()
            } else if perdue {
                charger()
            }
        }
        reglerMouvement()
    }

    /// Quitte le jeu : plus de capteurs, plus de vibrations, plus de page.
    func fermer() {
        fermee = true
        chargee = false
        reglerMouvement()
        vibrations.arreter()
        vue.stopLoading()
        vue.configuration.userContentController.removeScriptMessageHandler(forName: "soiree")
    }

    /// Les capteurs ne tournent que si une page est chargée et l'app au premier plan.
    private func reglerMouvement() {
        guard chargee, auPremierPlan, !fermee else {
            mouvement.arreter()
            return
        }
        mouvement.demarrer { [weak self] echantillon in
            self?.vue.evaluateJavaScript(echantillon.appel, completionHandler: nil)
        }
    }

    private func echouer(_ erreur: Error) {
        let erreur = erreur as NSError
        // Un chargement remplacé par un autre n'est pas une erreur.
        if erreur.domain == NSURLErrorDomain, erreur.code == NSURLErrorCancelled { return }
        if erreur.domain == "WebKitErrorDomain", erreur.code == 102 { return }

        chargee = false
        reglerMouvement()
        etat = .erreur(message(erreur))
    }

    private func message(_ erreur: NSError) -> String {
        guard erreur.domain == NSURLErrorDomain else { return erreur.localizedDescription }
        switch erreur.code {
        case NSURLErrorAppTransportSecurityRequiresSecureConnection:
            return "Cette adresse en http n'est pas sur le réseau local. L'app n'ouvre en http que les jeux du Wi-Fi."
        case NSURLErrorTimedOut, NSURLErrorCannotConnectToHost, NSURLErrorCannotFindHost,
             NSURLErrorNetworkConnectionLost, NSURLErrorNotConnectedToInternet, NSURLErrorDNSLookupFailed:
            return "Le jeu ne répond pas. Vérifie que ton téléphone est sur le même Wi-Fi que l'ordinateur du jeu, et que le jeu est lancé."
        default:
            return erreur.localizedDescription
        }
    }
}

// MARK: - Les messages de la page

/// `WKUserContentController` retient fort ce qu'on lui donne. Ce relais évite
/// que la vue web retienne la page, qui retient la vue web.
private final class Relais: NSObject, WKScriptMessageHandler {
    private weak var page: PageJeu?

    init(_ page: PageJeu) {
        self.page = page
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.frameInfo.isMainFrame, let corps = message.body as? [String: Any] else { return }
        MainActor.assumeIsolated { page?.recevoir(corps) }
    }
}

extension PageJeu {
    /// Un message de `pont.js`.
    fileprivate func recevoir(_ message: [String: Any]) {
        guard auPremierPlan, !fermee else { return }
        switch message["type"] as? String {
        case "haptic":
            let genre = message["kind"] as? String ?? "impact"
            let intensite = (message["intensity"] as? NSNumber)?.doubleValue ?? 1
            vibrations.retour(genre, intensite: intensite)
        case "vibrate":
            let durees = (message["pattern"] as? [NSNumber])?.map(\.doubleValue) ?? []
            vibrations.vibrer(durees)
        default:
            break
        }
    }
}

// MARK: - La navigation

extension PageJeu: WKNavigationDelegate {
    func webView(
        _ webView: WKWebView,
        decidePolicyFor action: WKNavigationAction,
        decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void
    ) {
        // Dans le cadre principal, seulement du http ou du https, comme à l'accueil.
        let schema = action.request.url?.scheme?.lowercased()
        let principal = action.targetFrame?.isMainFrame ?? true
        if principal, schema != "http", schema != "https" {
            decisionHandler(.cancel)
        } else {
            decisionHandler(.allow)
        }
    }

    func webView(_ webView: WKWebView, didCommit navigation: WKNavigation!) {
        // Le document est là, et `pont.js` avec lui.
        page = webView.url ?? page
        chargee = true
        etat = .prete
        reglerMouvement()
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        arrets = 0
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        echouer(error)
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        echouer(error)
    }

    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        // iOS a repris la mémoire de la page, souvent pendant que l'app était en arrière-plan.
        chargee = false
        reglerMouvement()
        arrets += 1
        if arrets > 2 {
            etat = .erreur("La page du jeu s'est arrêtée plusieurs fois.")
            arrets = 0
        } else if auPremierPlan {
            charger()
        } else {
            perdue = true
        }
    }
}

// MARK: - Les demandes de la page

extension PageJeu: WKUIDelegate {
    /// Un lien qui voudrait une nouvelle fenêtre s'ouvre ici : il n'y a qu'une page.
    func webView(
        _ webView: WKWebView,
        createWebViewWith configuration: WKWebViewConfiguration,
        for action: WKNavigationAction,
        windowFeatures: WKWindowFeatures
    ) -> WKWebView? {
        if action.targetFrame == nil {
            webView.load(action.request)
        }
        return nil
    }

    /// Pas de menu au toucher long.
    func webView(
        _ webView: WKWebView,
        contextMenuConfigurationFor elementInfo: WKContextMenuElementInfo
    ) async -> UIContextMenuConfiguration? {
        nil
    }

    /// Les capteurs de WebKit restent fermés : c'est l'app qui envoie les
    /// événements `devicemotion`, et il n'en faut pas deux flux.
    func webView(
        _ webView: WKWebView,
        requestDeviceOrientationAndMotionPermissionFor origin: WKSecurityOrigin,
        initiatedByFrame frame: WKFrameInfo
    ) async -> WKPermissionDecision {
        .deny
    }

    func webView(
        _ webView: WKWebView,
        runJavaScriptAlertPanelWithMessage message: String,
        initiatedByFrame frame: WKFrameInfo
    ) async {
        await withCheckedContinuation { suite in
            let alerte = UIAlertController(title: nil, message: message, preferredStyle: .alert)
            alerte.addAction(UIAlertAction(title: "OK", style: .default) { _ in suite.resume() })
            presenter(alerte) { suite.resume() }
        }
    }

    func webView(
        _ webView: WKWebView,
        runJavaScriptConfirmPanelWithMessage message: String,
        initiatedByFrame frame: WKFrameInfo
    ) async -> Bool {
        await withCheckedContinuation { suite in
            let alerte = UIAlertController(title: nil, message: message, preferredStyle: .alert)
            alerte.addAction(UIAlertAction(title: "Annuler", style: .cancel) { _ in suite.resume(returning: false) })
            alerte.addAction(UIAlertAction(title: "OK", style: .default) { _ in suite.resume(returning: true) })
            presenter(alerte) { suite.resume(returning: false) }
        }
    }

    func webView(
        _ webView: WKWebView,
        runJavaScriptTextInputPanelWithPrompt prompt: String,
        defaultText: String?,
        initiatedByFrame frame: WKFrameInfo
    ) async -> String? {
        await withCheckedContinuation { suite in
            let alerte = UIAlertController(title: nil, message: prompt, preferredStyle: .alert)
            alerte.addTextField { $0.text = defaultText }
            alerte.addAction(UIAlertAction(title: "Annuler", style: .cancel) { _ in suite.resume(returning: nil) })
            alerte.addAction(UIAlertAction(title: "OK", style: .default) { [weak alerte] _ in
                suite.resume(returning: alerte?.textFields?.first?.text ?? "")
            })
            presenter(alerte) { suite.resume(returning: nil) }
        }
    }

    /// Montre une alerte par-dessus le jeu. `sinon` est appelée s'il n'y a nulle part où la montrer.
    private func presenter(_ alerte: UIAlertController, sinon: () -> Void) {
        var dessus = vue.window?.rootViewController
        while let suivant = dessus?.presentedViewController, !suivant.isBeingDismissed {
            dessus = suivant
        }
        guard let dessus else {
            sinon()
            return
        }
        dessus.present(alerte, animated: true)
    }
}

// MARK: - Pas de zoom

extension PageJeu: UIScrollViewDelegate {
    func scrollViewWillBeginZooming(_ scrollView: UIScrollView, with view: UIView?) {
        scrollView.pinchGestureRecognizer?.isEnabled = false
    }
}
