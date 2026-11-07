//! Firefox sans interface, sur un profil neuf, piloté par `WebDriver` `BiDi` (PHASE-3,
//! § 8.2, lot 3b), sans `geckodriver` : Firefox ouvre lui-même `BiDi` sous
//! `--remote-debugging-port` (le harnais de mesure l'a fait le 2026-11-05).
//!
//! `--headless` est le vrai navigateur sans fenêtre (sauf [`Interface::Avec`], lot 10b :
//! sa fenêtre sur l'écran de l'hôte, seul moyen d'y avoir le plein écran au clic) ;
//! `--remote-debugging-port 0` choisit un port libre, que Firefox écrit sur sa sortie
//! d'erreur (`WebDriver BiDi listening on ws://…`). Le profil est un dossier neuf sous
//! `target/banc/profils/`, avec un `user.js` sans première page, sans télémétrie ni mise
//! à jour, effacé à la fermeture. Les préférences sont celles du harnais
//! (`mesures/navigateur/navigateurs.mjs`).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::bidi::{Bidi, ligne_d_evenement};
use crate::chrome::GesteSouris;
use crate::navigateur::{Interface, arreter_processus, attendre_l_adresse, effacer, profil_neuf};

/// Le Firefox de la machine de l'hôte (macOS).
pub const FIREFOX: &str = "/Applications/Firefox.app/Contents/MacOS/firefox";

/// Le délai d'une commande `BiDi`.
const DELAI: Duration = Duration::from_secs(20);

/// Les préférences du profil : ni première page, ni télémétrie, ni mise à jour, ni
/// avertissement à la fermeture.
const PREFERENCES: [(&str, &str); 7] = [
    ("browser.shell.checkDefaultBrowser", "false"),
    ("browser.startup.homepage_override.mstone", "\"ignore\""),
    ("browser.aboutwelcome.enabled", "false"),
    ("datareporting.policy.dataSubmissionEnabled", "false"),
    ("toolkit.telemetry.reportingpolicy.firstRun", "false"),
    ("app.update.disabledForTesting", "true"),
    ("browser.tabs.warnOnClose", "false"),
];

/// Un Firefox lancé, et sa session `BiDi`.
pub struct Firefox {
    processus: Child,
    profil: PathBuf,
    bidi: Bidi,
    /// `browserName` et `browserVersion` de la session (`firefox 158.0.1`).
    pub version: String,
    /// Les onglets ouverts, dans l'ordre : leur contexte `BiDi` (lot 10a).
    onglets: Vec<String>,
    /// Avec ou sans fenêtre (lot 10b).
    interface: Interface,
}

/// L'adresse `BiDi` que Firefox écrit sur sa sortie d'erreur.
pub fn adresse_annoncee(ligne: &str) -> Option<&str> {
    ligne
        .split_once("WebDriver BiDi listening on ")
        .map(|(_, a)| a.trim())
        .filter(|a| a.starts_with("ws://"))
}

impl Firefox {
    /// Lance `executable` sans interface, sur un profil neuf sous `profils`, ouvre une
    /// session `BiDi` et s'abonne au journal des pages.
    pub fn lancer(executable: &Path, profils: &Path) -> Result<Firefox, String> {
        Firefox::lancer_avec(executable, profils, Interface::Sans)
    }

    /// Comme [`Firefox::lancer`] ; avec [`Interface::Avec`], sans `--headless` : une vraie
    /// fenêtre sur l'écran de l'hôte, où Firefox donne le plein écran à un vrai clic du
    /// banc, ce qu'il refuse sans interface (lot 10b, constaté).
    pub fn lancer_avec(
        executable: &Path,
        profils: &Path,
        interface: Interface,
    ) -> Result<Firefox, String> {
        if !executable.exists() {
            return Err(format!("Firefox absent : {}", executable.display()));
        }
        let profil = profil_neuf(profils, "firefox")?;
        let mut prefs = PREFERENCES.iter().fold(String::new(), |mut s, (k, v)| {
            let _: std::fmt::Result = writeln!(s, "user_pref(\"{k}\", {v});");
            s
        });
        if interface == Interface::Avec {
            // Constaté le 2026-11-07 (Firefox 158.0.1, macOS 27.2,
            // `target/travail/lot-10b-corr/ff/essai.py`) : la page menée par `BiDi` n'a pas le
            // focus (il reste à la barre d'adresse : `document.hasFocus()` faux, le plein écran
            // refusé, « Fullscreen request denied »), et ni `browsingContext.activate` ni
            // `focusmanager.testmode` ne le lui donnent ; un `window.focus()` de la page le
            // lui donne si les pages peuvent passer leur fenêtre devant ([`Firefox::montrer`]).
            prefs.push_str("user_pref(\"dom.disable_window_flip\", false);\n");
            // Le plein écran natif de macOS (un bureau à part) y est quitté aussitôt entré,
            // sans Échap ni rien d'autre (constaté, cause non trouvée) ; le plein écran de
            // Firefox dans sa fenêtre tient. Échap est jugé par la page, qui ne voit pas la
            // différence (`fullscreenchange`, `document.fullscreenElement`).
            prefs.push_str("user_pref(\"full-screen-api.macos-native-full-screen\", false);\n");
        }
        // Les téléchargements vont dans le profil, sous leur nom, sans question (lot 11) :
        // le banc les y lit avant la fermeture, qui efface le profil.
        let telechargements = profil.join("telechargements");
        std::fs::create_dir_all(&telechargements)
            .map_err(|e| format!("{} : {e}", telechargements.display()))?;
        let dossier = serde_json::to_string(&telechargements.display().to_string())
            .map_err(|e| format!("{e}"))?;
        for (k, v) in [
            ("browser.download.folderList", "2"),
            ("browser.download.dir", dossier.as_str()),
            ("browser.download.useDownloadDir", "true"),
            (
                "browser.download.always_ask_before_handling_new_types",
                "false",
            ),
            ("browser.download.manager.showWhenStarting", "false"),
            ("browser.download.alwaysOpenPanel", "false"),
            ("browser.download.start_downloads_in_tmp_dir", "false"),
            (
                "browser.helperApps.neverAsk.saveToDisk",
                "\"application/json,image/png,image/webp,application/octet-stream\"",
            ),
        ] {
            let _: std::fmt::Result = writeln!(prefs, "user_pref(\"{k}\", {v});");
        }
        std::fs::write(profil.join("user.js"), prefs)
            .map_err(|e| format!("{} : {e}", profil.display()))?;
        let mut commande = Command::new(executable);
        if interface == Interface::Sans {
            commande.arg("--headless");
        }
        let mut processus = commande
            .args(["--no-remote", "--profile"])
            .arg(&profil)
            .args(["--remote-debugging-port", "0", "about:blank"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("lancer Firefox : {e}"))?;
        let adresse = match attendre_l_adresse(&mut processus, "Firefox", adresse_annoncee) {
            Ok(a) => a,
            Err(e) => {
                effacer(&profil);
                return Err(e);
            }
        };
        let session = format!("{}/session", adresse.trim_end_matches('/'));
        let bidi = match Bidi::ouvrir(&session) {
            Ok(b) => b,
            Err(e) => {
                arreter_processus(&mut processus);
                effacer(&profil);
                return Err(e);
            }
        };
        let mut firefox = Firefox {
            processus,
            profil,
            bidi,
            version: String::new(),
            onglets: Vec::new(),
            interface,
        };
        let s = firefox
            .bidi
            .envoyer("session.new", &json!({ "capabilities": {} }), DELAI)?;
        let c = s.get("capabilities").cloned().unwrap_or(Json::Null);
        firefox.version = format!(
            "{} {}",
            c.get("browserName").and_then(Json::as_str).unwrap_or("?"),
            c.get("browserVersion")
                .and_then(Json::as_str)
                .unwrap_or("?")
        );
        firefox.bidi.envoyer(
            "session.subscribe",
            &json!({ "events": ["log.entryAdded"] }),
            DELAI,
        )?;
        Ok(firefox)
    }

    /// Mène l'onglet de la session à `url`, sans attendre son chargement : la page
    /// rapporte elle-même.
    pub fn ouvrir(&mut self, url: &str) -> Result<(), String> {
        let arbre = self
            .bidi
            .envoyer("browsingContext.getTree", &json!({}), DELAI)?;
        let contexte = arbre
            .pointer("/contexts/0/context")
            .and_then(Json::as_str)
            .ok_or_else(|| format!("browsingContext.getTree sans onglet : {arbre}"))?
            .to_owned();
        self.bidi.envoyer(
            "browsingContext.navigate",
            &json!({ "context": contexte, "url": url, "wait": "none" }),
            DELAI,
        )?;
        self.onglets.push(contexte);
        Ok(())
    }

    fn contexte(&self, onglet: usize) -> Result<String, String> {
        self.onglets
            .get(onglet)
            .cloned()
            .ok_or_else(|| format!("pas d'onglet {onglet}"))
    }

    /// Ouvre un onglet de plus à `url` ; rend son rang parmi les onglets ouverts.
    pub fn nouvel_onglet(&mut self, url: &str) -> Result<usize, String> {
        let c = self
            .bidi
            .envoyer("browsingContext.create", &json!({ "type": "tab" }), DELAI)?;
        let contexte = c
            .get("context")
            .and_then(Json::as_str)
            .ok_or_else(|| format!("browsingContext.create sans contexte : {c}"))?
            .to_owned();
        self.bidi.envoyer(
            "browsingContext.navigate",
            &json!({ "context": contexte, "url": url, "wait": "none" }),
            DELAI,
        )?;
        self.onglets.push(contexte);
        Ok(self.onglets.len().saturating_sub(1))
    }

    /// Évalue `expression` dans l'onglet, sa promesse attendue ; rend sa valeur, si elle
    /// est primitive (une chaîne, un nombre, un booléen).
    pub fn evaluer(&mut self, onglet: usize, expression: &str) -> Result<Json, String> {
        let contexte = self.contexte(onglet)?;
        let r = self.bidi.envoyer(
            "script.evaluate",
            &json!({ "expression": expression, "target": { "context": contexte }, "awaitPromise": true, "resultOwnership": "none" }),
            DELAI,
        )?;
        if r.get("type").and_then(Json::as_str) == Some("exception") {
            return Err(format!(
                "{expression} : {}",
                r.get("exceptionDetails").cloned().unwrap_or(Json::Null)
            ));
        }
        Ok(r.pointer("/result/value").cloned().unwrap_or(Json::Null))
    }

    /// Donne à l'onglet une fenêtre de `largeur` × `hauteur` pixels CSS, au rapport de
    /// pixels `dpr` (`browsingContext.setViewport`, lot 11 ; `dpr` depuis la remontée du
    /// lot 10b).
    pub fn fenetre(
        &mut self,
        onglet: usize,
        largeur: u32,
        hauteur: u32,
        dpr: f64,
    ) -> Result<(), String> {
        let contexte = self.contexte(onglet)?;
        self.bidi
            .envoyer(
                "browsingContext.setViewport",
                &json!({ "context": contexte, "viewport": { "width": largeur, "height": hauteur }, "devicePixelRatio": dpr }),
                DELAI,
            )
            .map(|_| ())
    }

    /// Le dossier où vont les téléchargements de ce Firefox (lot 11).
    pub fn telechargements(&self) -> PathBuf {
        self.profil.join("telechargements")
    }

    /// Donne des fichiers à l'`<input type=file>` que `selecteur` (CSS) désigne, par
    /// `input.setFiles` : la page reçoit `change` comme d'un vrai choix (lot 11).
    pub fn fichiers(
        &mut self,
        onglet: usize,
        selecteur: &str,
        fichiers: &[PathBuf],
    ) -> Result<(), String> {
        let contexte = self.contexte(onglet)?;
        let r = self.bidi.envoyer(
            "script.evaluate",
            &json!({ "expression": format!("document.querySelector({})", json!(selecteur)),
                     "target": { "context": contexte }, "awaitPromise": false, "resultOwnership": "none" }),
            DELAI,
        )?;
        let noeud = r
            .pointer("/result/sharedId")
            .and_then(Json::as_str)
            .ok_or_else(|| format!("aucun élément {selecteur} : {r}"))?
            .to_owned();
        self.bidi
            .envoyer(
                "input.setFiles",
                &json!({ "context": contexte, "element": { "sharedId": noeud }, "files": fichiers }),
                DELAI,
            )
            .map(|_| ())
    }

    /// Joue des gestes de la souris, en pixels de la fenêtre, par la chaîne d'entrées du
    /// navigateur (`input.performActions`, lot 11) : la page reçoit des événements de
    /// pointeur vrais (`isTrusted`).
    pub fn souris(&mut self, onglet: usize, gestes: &[GesteSouris]) -> Result<(), String> {
        let contexte = self.contexte(onglet)?;
        let point = |x: f64, y: f64| (x.round(), y.round());
        let actions: Vec<Json> = gestes
            .iter()
            .flat_map(|g| match *g {
                GesteSouris::Appui(x, y) => {
                    let (x, y) = point(x, y);
                    vec![
                        json!({ "type": "pointerMove", "x": x, "y": y, "origin": "viewport", "duration": 0 }),
                        json!({ "type": "pointerDown", "button": 0 }),
                    ]
                }
                GesteSouris::Deplace(x, y) => {
                    let (x, y) = point(x, y);
                    vec![json!({ "type": "pointerMove", "x": x, "y": y, "origin": "viewport", "duration": 0 })]
                }
                GesteSouris::Relache(x, y) => {
                    let (x, y) = point(x, y);
                    vec![
                        json!({ "type": "pointerMove", "x": x, "y": y, "origin": "viewport", "duration": 0 }),
                        json!({ "type": "pointerUp", "button": 0 }),
                    ]
                }
                GesteSouris::Pause(ms) => vec![json!({ "type": "pause", "duration": ms })],
            })
            .collect();
        self.bidi
            .envoyer(
                "input.performActions",
                &json!({ "context": contexte, "actions": [{ "type": "pointer", "id": "souris",
                         "parameters": { "pointerType": "mouse" }, "actions": actions }] }),
                DELAI,
            )
            .map(|_| ())
    }

    /// Tape un texte dans l'élément qui a le focus, caractère après caractère (lot 11).
    pub fn texte(&mut self, onglet: usize, texte: &str) -> Result<(), String> {
        let contexte = self.contexte(onglet)?;
        let actions: Vec<Json> = texte
            .chars()
            .flat_map(|c| {
                let v = c.to_string();
                [
                    json!({ "type": "keyDown", "value": v }),
                    json!({ "type": "keyUp", "value": v }),
                ]
            })
            .collect();
        self.bidi
            .envoyer(
                "input.performActions",
                &json!({ "context": contexte, "actions": [{ "type": "key", "id": "clavier", "actions": actions }] }),
                DELAI,
            )
            .map(|_| ())
    }

    /// Appuie sur une touche (`Enter`, `Escape`), enfoncée puis relâchée (lot 11).
    pub fn touche(&mut self, onglet: usize, code: &str) -> Result<(), String> {
        let contexte = self.contexte(onglet)?;
        let valeur = match code {
            "Enter" => "\u{E007}",
            "Escape" => "\u{E00C}",
            "Tab" => "\u{E004}",
            autre => return Err(format!("touche inconnue du banc : {autre}")),
        };
        self.bidi
            .envoyer(
                "input.performActions",
                &json!({ "context": contexte, "actions": [{ "type": "key", "id": "clavier", "actions": [
                    { "type": "keyDown", "value": valeur }, { "type": "keyUp", "value": valeur }
                ] }] }),
                DELAI,
            )
            .map(|_| ())
    }

    /// Recharge l'onglet.
    pub fn recharger(&mut self, onglet: usize) -> Result<(), String> {
        let contexte = self.contexte(onglet)?;
        self.bidi
            .envoyer(
                "browsingContext.reload",
                &json!({ "context": contexte, "wait": "none" }),
                DELAI,
            )
            .map(|_| ())
    }

    /// Ferme l'onglet ; son rang reste pris.
    pub fn fermer_onglet(&mut self, onglet: usize) -> Result<(), String> {
        let contexte = self.contexte(onglet)?;
        self.bidi
            .envoyer(
                "browsingContext.close",
                &json!({ "context": contexte }),
                DELAI,
            )
            .map(|_| ())
    }

    /// Passe un onglet vierge devant les autres : ils sont cachés (comme la mesure 6).
    pub fn cacher(&mut self) -> Result<(), String> {
        let c = self
            .bidi
            .envoyer("browsingContext.create", &json!({ "type": "tab" }), DELAI)?;
        let contexte = c
            .get("context")
            .and_then(Json::as_str)
            .ok_or_else(|| format!("browsingContext.create sans contexte : {c}"))?
            .to_owned();
        self.bidi
            .envoyer(
                "browsingContext.activate",
                &json!({ "context": contexte }),
                DELAI,
            )
            .map(|_| ())
    }

    /// Remet l'onglet devant ; avec sa fenêtre ([`Interface::Avec`]), lui donne aussi le
    /// focus, par un `window.focus()` de la page (voir [`Firefox::lancer_avec`]).
    pub fn montrer(&mut self, onglet: usize) -> Result<(), String> {
        let contexte = self.contexte(onglet)?;
        self.bidi.envoyer(
            "browsingContext.activate",
            &json!({ "context": contexte }),
            DELAI,
        )?;
        if self.interface == Interface::Avec {
            self.evaluer(onglet, "window.focus()")?;
            // Le focus passe par le processus parent : l'attendre, au plus 2 s.
            for _ in 0..20 {
                if self.evaluer(onglet, "document.hasFocus()")? == Json::Bool(true) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        Ok(())
    }

    /// Appuie sur F8, par la chaîne d'entrées du navigateur (`input.performActions`).
    pub fn touche_f8(&mut self, onglet: usize) -> Result<(), String> {
        let contexte = self.contexte(onglet)?;
        self.bidi
            .envoyer(
                "input.performActions",
                &json!({ "context": contexte, "actions": [{ "type": "key", "id": "clavier", "actions": [
                    { "type": "keyDown", "value": "\u{E038}" }, { "type": "keyUp", "value": "\u{E038}" }
                ] }] }),
                DELAI,
            )
            .map(|_| ())
    }

    /// Appuie sur Échap, par la chaîne d'entrées du navigateur (lot 10b, P38).
    pub fn touche_echap(&mut self, onglet: usize) -> Result<(), String> {
        let contexte = self.contexte(onglet)?;
        self.bidi
            .envoyer(
                "input.performActions",
                &json!({ "context": contexte, "actions": [{ "type": "key", "id": "clavier", "actions": [
                    { "type": "keyDown", "value": "\u{E00C}" }, { "type": "keyUp", "value": "\u{E00C}" }
                ] }] }),
                DELAI,
            )
            .map(|_| ())
    }

    /// Un clic gauche en `(x, y)`, pixels de la fenêtre (`input.performActions`) : un vrai
    /// geste (lot 10b).
    pub fn cliquer(&mut self, onglet: usize, x: f64, y: f64) -> Result<(), String> {
        let contexte = self.contexte(onglet)?;
        // `BiDi` veut des entiers pour la position d'un pointeur.
        let (x, y) = (x.round(), y.round());
        self.bidi
            .envoyer(
                "input.performActions",
                &json!({ "context": contexte, "actions": [{ "type": "pointer", "id": "souris", "parameters": { "pointerType": "mouse" }, "actions": [
                    { "type": "pointerMove", "x": x, "y": y }, { "type": "pointerDown", "button": 0 }, { "type": "pointerUp", "button": 0 }
                ] }] }),
                DELAI,
            )
            .map(|_| ())
    }

    /// Écoute la page au plus `delai`, et rend ce que sa console et ses exceptions ont dit.
    pub fn ecouter(&mut self, delai: Duration) -> Result<Vec<String>, String> {
        self.bidi.ecouter(delai)?;
        Ok(self
            .bidi
            .evenements()
            .iter()
            .filter_map(ligne_d_evenement)
            .collect())
    }

    /// Ferme Firefox (`browser.close`, puis au plus 10 s avant de le tuer) et efface son
    /// profil.
    pub fn fermer(mut self) {
        self.arreter();
    }

    fn arreter(&mut self) {
        let _: Result<Json, String> =
            self.bidi
                .envoyer("browser.close", &json!({}), Duration::from_secs(5));
        arreter_processus(&mut self.processus);
        effacer(&self.profil);
    }
}

impl Drop for Firefox {
    fn drop(&mut self) {
        if matches!(self.processus.try_wait(), Ok(None)) {
            self.arreter();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::adresse_annoncee;

    #[test]
    fn l_adresse_que_firefox_annonce() {
        assert_eq!(
            adresse_annoncee("WebDriver BiDi listening on ws://127.0.0.1:52011"),
            Some("ws://127.0.0.1:52011")
        );
        assert_eq!(adresse_annoncee("console.error: autre chose"), None);
    }
}
