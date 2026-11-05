//! Chrome sans interface, sur un profil neuf, piloté par CDP (PHASE-3, § 8.2).
//!
//! `--headless=new` est le vrai navigateur sans fenêtre ; `--remote-debugging-port=0`
//! choisit un port libre, que Chrome écrit sur sa sortie d'erreur. Le profil est un
//! dossier neuf sous `target/banc/profils/`, effacé à la fermeture.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::cdp::{Cdp, ligne_d_evenement};
use crate::navigateur::{arreter_processus, attendre_l_adresse, effacer, profil_neuf};

/// Le Chrome de la machine de l'hôte (macOS).
pub const CHROME: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

/// Le délai d'une requête CDP.
const DELAI: Duration = Duration::from_secs(20);

/// Un Chrome lancé, et sa connexion CDP.
pub struct Chrome {
    processus: Child,
    profil: PathBuf,
    cdp: Cdp,
    /// `product` de `Browser.getVersion` (`HeadlessChrome/156.0…`).
    pub version: String,
    /// La page ouverte, s'il y en a une : son `sessionId`.
    session: Option<String>,
}

/// L'adresse CDP que Chrome écrit sur sa sortie d'erreur.
pub fn adresse_annoncee(ligne: &str) -> Option<&str> {
    ligne
        .split_once("DevTools listening on ")
        .map(|(_, a)| a.trim())
        .filter(|a| a.starts_with("ws://"))
}

impl Chrome {
    /// Lance `executable` sans interface, sur un profil neuf sous `profils`, et s'y
    /// connecte par CDP.
    pub fn lancer(executable: &Path, profils: &Path) -> Result<Chrome, String> {
        if !executable.exists() {
            return Err(format!("Chrome absent : {}", executable.display()));
        }
        let profil = profil_neuf(profils, "chrome")?;
        let mut processus = Command::new(executable)
            .args([
                "--headless=new",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-extensions",
                "--disable-component-update",
                "--disable-sync",
                "--remote-debugging-port=0",
            ])
            .arg(format!("--user-data-dir={}", profil.display()))
            .arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("lancer Chrome : {e}"))?;
        let adresse = match attendre_l_adresse(&mut processus, "Chrome", adresse_annoncee) {
            Ok(a) => a,
            Err(e) => {
                effacer(&profil);
                return Err(e);
            }
        };
        let mut chrome = Chrome {
            processus,
            profil,
            cdp: Cdp::ouvrir(&adresse)?,
            version: String::new(),
            session: None,
        };
        let v = chrome
            .cdp
            .envoyer("Browser.getVersion", &json!({}), None, DELAI)?;
        v.get("product")
            .and_then(Json::as_str)
            .unwrap_or("?")
            .clone_into(&mut chrome.version);
        Ok(chrome)
    }

    /// Ouvre un onglet vierge, s'y attache, écoute sa console et ses exceptions, puis le
    /// mène à `url`.
    pub fn ouvrir(&mut self, url: &str) -> Result<(), String> {
        let cible = self.cdp.envoyer(
            "Target.createTarget",
            &json!({ "url": "about:blank" }),
            None,
            DELAI,
        )?;
        let id = cible
            .get("targetId")
            .and_then(Json::as_str)
            .ok_or_else(|| String::from("Target.createTarget sans targetId"))?
            .to_owned();
        let attache = self.cdp.envoyer(
            "Target.attachToTarget",
            &json!({ "targetId": id, "flatten": true }),
            None,
            DELAI,
        )?;
        let session = attache
            .get("sessionId")
            .and_then(Json::as_str)
            .ok_or_else(|| String::from("Target.attachToTarget sans sessionId"))?
            .to_owned();
        for m in ["Runtime.enable", "Log.enable", "Page.enable"] {
            self.cdp.envoyer(m, &json!({}), Some(&session), DELAI)?;
        }
        self.cdp.envoyer(
            "Page.navigate",
            &json!({ "url": url }),
            Some(&session),
            DELAI,
        )?;
        self.session = Some(session);
        Ok(())
    }

    /// Écoute la page au plus `delai`, et rend ce que sa console et ses exceptions ont dit.
    pub fn ecouter(&mut self, delai: Duration) -> Result<Vec<String>, String> {
        self.cdp.ecouter(delai)?;
        Ok(self
            .cdp
            .evenements()
            .iter()
            .filter_map(ligne_d_evenement)
            .collect())
    }

    /// Ferme Chrome (`Browser.close`, puis au plus 10 s avant de le tuer) et efface son
    /// profil.
    pub fn fermer(mut self) {
        self.arreter();
    }

    fn arreter(&mut self) {
        let _: Result<Json, String> =
            self.cdp
                .envoyer("Browser.close", &json!({}), None, Duration::from_secs(5));
        arreter_processus(&mut self.processus);
        effacer(&self.profil);
    }
}

impl Drop for Chrome {
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
    fn l_adresse_que_chrome_annonce() {
        assert_eq!(
            adresse_annoncee("DevTools listening on ws://127.0.0.1:51234/devtools/browser/x-y"),
            Some("ws://127.0.0.1:51234/devtools/browser/x-y")
        );
        assert_eq!(adresse_annoncee("[1105/…] autre chose"), None);
    }
}
