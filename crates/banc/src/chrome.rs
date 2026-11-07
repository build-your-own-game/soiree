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
    /// Les onglets ouverts, dans l'ordre : leur `targetId` et leur `sessionId` (lot 10a).
    onglets: Vec<(String, String)>,
    /// L'onglet vierge passé devant pour cacher les autres, s'il y en a un.
    devant: Option<String>,
    /// Les événements de la trace (`Tracing.dataCollected`), gardés à part de la console.
    trace: Vec<Json>,
    /// La trace est-elle finie (`Tracing.tracingComplete`) ?
    trace_finie: bool,
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
            onglets: Vec::new(),
            devant: None,
            trace: Vec::new(),
            trace_finie: false,
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
        let (id, session) = self.onglet(url)?;
        self.onglets.push((id, session.clone()));
        self.session = Some(session);
        Ok(())
    }

    /// Ouvre un onglet de plus à `url` (le second onglet de l'écran, lot 10a) ; rend son
    /// rang parmi les onglets ouverts.
    pub fn nouvel_onglet(&mut self, url: &str) -> Result<usize, String> {
        let o = self.onglet(url)?;
        self.onglets.push(o);
        Ok(self.onglets.len().saturating_sub(1))
    }

    fn session_de(&self, onglet: usize) -> Result<String, String> {
        self.onglets
            .get(onglet)
            .map(|(_, s)| s.clone())
            .ok_or_else(|| format!("pas d'onglet {onglet}"))
    }

    /// Évalue `expression` dans l'onglet, sa promesse attendue ; rend sa valeur.
    pub fn evaluer(&mut self, onglet: usize, expression: &str) -> Result<Json, String> {
        let s = self.session_de(onglet)?;
        let r = self.cdp.envoyer(
            "Runtime.evaluate",
            &json!({ "expression": expression, "returnByValue": true, "awaitPromise": true }),
            Some(&s),
            DELAI,
        )?;
        if let Some(e) = r.get("exceptionDetails") {
            return Err(format!("{expression} : {e}"));
        }
        Ok(r.pointer("/result/value").cloned().unwrap_or(Json::Null))
    }

    /// Recharge l'onglet.
    pub fn recharger(&mut self, onglet: usize) -> Result<(), String> {
        let s = self.session_de(onglet)?;
        self.cdp
            .envoyer("Page.reload", &json!({}), Some(&s), DELAI)
            .map(|_| ())
    }

    /// Ferme l'onglet ; son rang reste pris.
    pub fn fermer_onglet(&mut self, onglet: usize) -> Result<(), String> {
        let id = self
            .onglets
            .get(onglet)
            .map(|(t, _)| t.clone())
            .ok_or_else(|| format!("pas d'onglet {onglet}"))?;
        self.cdp
            .envoyer(
                "Target.closeTarget",
                &json!({ "targetId": id }),
                None,
                DELAI,
            )
            .map(|_| ())
    }

    /// Passe un onglet vierge devant les autres : ils sont cachés (comme la mesure 6).
    pub fn cacher(&mut self) -> Result<(), String> {
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
        self.cdp.envoyer(
            "Target.activateTarget",
            &json!({ "targetId": id }),
            None,
            DELAI,
        )?;
        self.devant = Some(id);
        Ok(())
    }

    /// Remet l'onglet devant.
    pub fn montrer(&mut self, onglet: usize) -> Result<(), String> {
        let id = self
            .onglets
            .get(onglet)
            .map(|(t, _)| t.clone())
            .ok_or_else(|| format!("pas d'onglet {onglet}"))?;
        self.cdp.envoyer(
            "Target.activateTarget",
            &json!({ "targetId": id }),
            None,
            DELAI,
        )?;
        if let Some(d) = self.devant.take() {
            let _: Result<Json, String> =
                self.cdp
                    .envoyer("Target.closeTarget", &json!({ "targetId": d }), None, DELAI);
        }
        Ok(())
    }

    /// Appuie sur F8, par la chaîne d'entrées du navigateur (`Input.dispatchKeyEvent`).
    pub fn touche_f8(&mut self, onglet: usize) -> Result<(), String> {
        let s = self.session_de(onglet)?;
        for sorte in ["rawKeyDown", "keyUp"] {
            self.cdp.envoyer(
                "Input.dispatchKeyEvent",
                &json!({ "type": sorte, "key": "F8", "code": "F8", "windowsVirtualKeyCode": 119, "nativeVirtualKeyCode": 119 }),
                Some(&s),
                DELAI,
            )?;
        }
        Ok(())
    }

    /// Un onglet neuf, attaché, sa console et ses exceptions écoutées, mené à `url`.
    fn onglet(&mut self, url: &str) -> Result<(String, String), String> {
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
        Ok((id, session))
    }

    /// Écoute la page au plus `delai`, et rend ce que sa console et ses exceptions ont dit.
    pub fn ecouter(&mut self, delai: Duration) -> Result<Vec<String>, String> {
        self.cdp.ecouter(delai)?;
        let mut lignes = Vec::new();
        for e in self.cdp.evenements() {
            match e.get("method").and_then(Json::as_str) {
                Some("Tracing.dataCollected") => {
                    if let Some(v) = e.pointer("/params/value").and_then(Json::as_array) {
                        self.trace.extend(v.iter().cloned());
                    }
                }
                Some("Tracing.tracingComplete") => self.trace_finie = true,
                _ => lignes.extend(ligne_d_evenement(&e)),
            }
        }
        Ok(lignes)
    }

    /// Commence une trace du navigateur (`Tracing.start`), dans ces catégories, ses
    /// événements rapportés au fil de l'eau.
    pub fn tracer(&mut self, categories: &[&str]) -> Result<(), String> {
        self.trace.clear();
        self.trace_finie = false;
        self.cdp
            .envoyer(
                "Tracing.start",
                &json!({
                    "transferMode": "ReportEvents",
                    "traceConfig": { "includedCategories": categories, "recordMode": "recordAsMuchAsPossible" },
                }),
                None,
                DELAI,
            )
            .map(|_| ())
    }

    /// Finit la trace (`Tracing.end`) et rend ses événements, au plus `delai` après.
    pub fn fin_de_trace(&mut self, delai: Duration) -> Result<Vec<Json>, String> {
        self.cdp.envoyer("Tracing.end", &json!({}), None, DELAI)?;
        let echeance = crate::horloge::maintenant()
            .checked_add(delai)
            .ok_or_else(|| String::from("délai"))?;
        while !self.trace_finie {
            if crate::horloge::reste(echeance).is_zero() {
                return Err(format!("la trace n'a pas fini en {delai:?}"));
            }
            let _: Vec<String> = self.ecouter(Duration::from_millis(200))?;
        }
        Ok(std::mem::take(&mut self.trace))
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
