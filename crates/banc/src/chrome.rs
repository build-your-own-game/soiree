//! Chrome sans interface, sur un profil neuf, piloté par CDP (PHASE-3, § 8.2).
//!
//! `--headless=new` est le vrai navigateur sans fenêtre ; `--remote-debugging-port=0`
//! choisit un port libre, que Chrome écrit sur sa sortie d'erreur. Le profil est un
//! dossier neuf sous `target/banc/profils/`, effacé à la fermeture.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::cdp::{Cdp, ligne_d_evenement};
use crate::horloge::{maintenant, reste};

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

/// Un profil neuf : un dossier qui n'existait pas.
fn profil_neuf(dossier: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dossier).map_err(|e| format!("{}: {e}", dossier.display()))?;
    for n in 0u32..1000 {
        let p = dossier.join(format!("chrome-{}-{n}", std::process::id()));
        if std::fs::create_dir(&p).is_ok() {
            return Ok(p);
        }
    }
    Err(format!("aucun profil neuf sous {}", dossier.display()))
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
        let profil = profil_neuf(profils)?;
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
        let Some(erreur) = processus.stderr.take() else {
            let _: std::io::Result<()> = processus.kill();
            return Err(String::from("la sortie d'erreur de Chrome manque"));
        };
        let (envoi, recu) = mpsc::channel::<String>();
        // Le fil lit la sortie d'erreur jusqu'au bout : un tuyau plein bloquerait Chrome.
        let _: std::io::Result<thread::JoinHandle<()>> = thread::Builder::new()
            .name(String::from("banc-chrome-stderr"))
            .spawn(move || {
                for l in BufReader::new(erreur).lines() {
                    let Ok(l) = l else { break };
                    let _: Result<(), mpsc::SendError<String>> = envoi.send(l);
                }
            });
        let echeance = maintenant()
            .checked_add(Duration::from_secs(30))
            .ok_or_else(|| String::from("délai"))?;
        let mut vu = Vec::new();
        let adresse = loop {
            let Ok(l) = recu.recv_timeout(reste(echeance)) else {
                let _: std::io::Result<()> = processus.kill();
                let _: std::io::Result<std::process::ExitStatus> = processus.wait();
                effacer(&profil);
                return Err(format!(
                    "Chrome n'a pas annoncé son adresse CDP en 30 s : {}",
                    vu.join(" | ")
                ));
            };
            if let Some(a) = adresse_annoncee(&l) {
                break String::from(a);
            }
            vu.push(l);
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
        let echeance = maintenant().checked_add(Duration::from_secs(10));
        loop {
            match self.processus.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => {}
            }
            if echeance.is_none_or(|e| reste(e).is_zero()) {
                let _: std::io::Result<()> = self.processus.kill();
                let _: std::io::Result<std::process::ExitStatus> = self.processus.wait();
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
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

/// Efface un profil ; les processus fils de Chrome y écrivent encore un instant après
/// sa fin.
fn effacer(profil: &Path) {
    for _ in 0..20 {
        if std::fs::remove_dir_all(profil).is_ok() || !profil.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(250));
    }
    eprintln!("profil laissé : {}", profil.display());
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
