//! Safari, piloté par `safaridriver` (PHASE-3, § 8.2, lot 3c) : `WebDriver` classique,
//! en HTTP sur la boucle locale.
//!
//! Safari n'a pas de mode sans interface : une vraie fenêtre d'automatisation s'ouvre.
//! `safaridriver` ne crée une session que si l'hôte a coché « Autoriser
//! l'automatisation à distance » (Safari, Réglages, Développeur), et peut-être lancé une
//! fois `safaridriver --enable` (mot de passe administrateur). Le banc ne touche jamais
//! à ces réglages : sans eux, la session est refusée, et chaque contrôle Safari est « pas
//! exécuté » avec le message de `safaridriver`, jamais réussi (OUT-01 ; PHASE-3,
//! question 8).
//!
//! Il n'y a pas de profil neuf : `safaridriver` ouvre sa propre fenêtre, et le banc n'a
//! rien à effacer. `WebDriver` classique ne dit ni la console ni les exceptions d'une
//! page : [`Safari::ecouter`] attend, et ne rapporte rien ; la page rapporte elle-même
//! (`POST /banc/rapport`), comme partout.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::horloge::{maintenant, reste};

/// Le `safaridriver` de la machine de l'hôte (macOS).
pub const SAFARIDRIVER: &str = "/usr/bin/safaridriver";

/// Le plus long qu'on attend une réponse de `safaridriver` : il abandonne lui-même une
/// session qui ne se crée pas au bout de 30 s (constaté le 2026-11-05).
const DELAI: Duration = Duration::from_secs(90);

/// Un `safaridriver` lancé, et sa session.
pub struct Safari {
    processus: Child,
    port: u16,
    session: String,
    /// `browserName` et `browserVersion` de la session (`safari 27.2`).
    pub version: String,
}

/// Un port libre de la boucle locale, que le système choisit.
fn port_libre() -> Result<u16, String> {
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("port libre : {e}"))?;
    l.local_addr()
        .map(|a| a.port())
        .map_err(|e| format!("port libre : {e}"))
}

/// Une requête HTTP/1.1 à `safaridriver`, et sa réponse : son statut et son corps
/// JSON.
pub(crate) fn requete(
    port: u16,
    methode: &str,
    chemin: &str,
    corps: Option<&Json>,
) -> Result<(u16, Json), String> {
    let mut flux = TcpStream::connect(("127.0.0.1", port))
        .map_err(|e| format!("safaridriver injoignable : {e}"))?;
    flux.set_read_timeout(Some(DELAI))
        .map_err(|e| e.to_string())?;
    let corps = corps.map(Json::to_string).unwrap_or_default();
    let entete = format!(
        "{methode} {chemin} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        corps.len()
    );
    flux.write_all(entete.as_bytes())
        .and_then(|()| flux.write_all(corps.as_bytes()))
        .map_err(|e| format!("safaridriver : {e}"))?;
    lire_reponse(&mut BufReader::new(flux))
}

/// Lit une réponse HTTP/1.1 : la ligne de statut, les en-têtes, puis `Content-Length`
/// octets de corps (ou tout jusqu'à la fermeture), en JSON.
pub(crate) fn lire_reponse(r: &mut impl BufRead) -> Result<(u16, Json), String> {
    let mut ligne = String::new();
    r.read_line(&mut ligne)
        .map_err(|e| format!("safaridriver : {e}"))?;
    let statut: u16 = ligne
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("réponse illisible : {}", ligne.trim()))?;
    let mut longueur = None;
    loop {
        let mut l = String::new();
        let n = r
            .read_line(&mut l)
            .map_err(|e| format!("safaridriver : {e}"))?;
        if n == 0 || l == "\r\n" || l == "\n" {
            break;
        }
        if let Some((nom, valeur)) = l.split_once(':')
            && nom.trim().eq_ignore_ascii_case("content-length")
        {
            longueur = valeur.trim().parse::<usize>().ok();
        }
    }
    let mut corps = Vec::new();
    match longueur {
        Some(n) => {
            corps.resize(n, 0);
            r.read_exact(&mut corps)
                .map_err(|e| format!("safaridriver : {e}"))?;
        }
        None => {
            r.read_to_end(&mut corps)
                .map_err(|e| format!("safaridriver : {e}"))?;
        }
    }
    let v = if corps.is_empty() {
        Json::Null
    } else {
        serde_json::from_slice(&corps).map_err(|e| format!("réponse JSON illisible : {e}"))?
    };
    Ok((statut, v))
}

/// Le message d'une erreur `WebDriver` (`{ value: { error, message } }`), s'il y en a
/// une.
pub(crate) fn erreur_webdriver(statut: u16, v: &Json) -> Option<String> {
    let valeur = v.get("value");
    let erreur = valeur.and_then(|x| x.get("error")).and_then(Json::as_str);
    if statut < 400 && erreur.is_none() {
        return None;
    }
    let message = valeur
        .and_then(|x| x.get("message"))
        .and_then(Json::as_str)
        .unwrap_or_default();
    Some(format!(
        "{} (HTTP {statut}) : {message}",
        erreur.unwrap_or("erreur")
    ))
}

impl Safari {
    /// Lance `safaridriver` (`executable`) sur un port libre, et ouvre une session
    /// Safari. `Err` : il ne part pas, ou il refuse la session (l'automatisation à
    /// distance n'est pas autorisée) ; le message de `safaridriver` est rendu tel quel.
    pub fn lancer(executable: &Path) -> Result<Safari, String> {
        if !executable.exists() {
            return Err(format!("safaridriver absent : {}", executable.display()));
        }
        let port = port_libre()?;
        let processus = Command::new(executable)
            .arg("--port")
            .arg(port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("lancer safaridriver : {e}"))?;
        let mut s = Safari {
            processus,
            port,
            session: String::new(),
            version: String::new(),
        };
        // `safaridriver` écoute au bout de quelques dixièmes de seconde.
        let echeance = maintenant()
            .checked_add(Duration::from_secs(10))
            .ok_or_else(|| String::from("délai"))?;
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            if reste(echeance).is_zero() {
                return Err(String::from("safaridriver n'écoute pas en 10 s"));
            }
            thread::sleep(Duration::from_millis(100));
        }
        let (statut, v) = requete(
            port,
            "POST",
            "/session",
            Some(&json!({ "capabilities": { "alwaysMatch": { "browserName": "safari" } } })),
        )?;
        if let Some(e) = erreur_webdriver(statut, &v) {
            return Err(format!("safaridriver refuse la session : {e}"));
        }
        let valeur = v.get("value").unwrap_or(&Json::Null);
        valeur
            .get("sessionId")
            .and_then(Json::as_str)
            .ok_or_else(|| format!("session sans identifiant : {v}"))?
            .clone_into(&mut s.session);
        let caps = valeur.get("capabilities").unwrap_or(&Json::Null);
        s.version = format!(
            "{} {}",
            caps.get("browserName")
                .and_then(Json::as_str)
                .unwrap_or("safari"),
            caps.get("browserVersion")
                .and_then(Json::as_str)
                .unwrap_or("?")
        );
        Ok(s)
    }

    /// Ouvre `url` dans la fenêtre de la session.
    pub fn ouvrir(&mut self, url: &str) -> Result<(), String> {
        let (statut, v) = requete(
            self.port,
            "POST",
            &format!("/session/{}/url", self.session),
            Some(&json!({ "url": url })),
        )?;
        erreur_webdriver(statut, &v).map_or(Ok(()), Err)
    }

    /// Attend `delai` : `WebDriver` classique ne dit ni la console ni les exceptions.
    pub fn ecouter(&mut self, delai: Duration) -> Vec<String> {
        thread::sleep(delai);
        Vec::new()
    }

    /// Ferme la session, puis `safaridriver`.
    pub fn fermer(mut self) {
        self.arreter();
    }

    fn arreter(&mut self) {
        if !self.session.is_empty() {
            let _: Result<(u16, Json), String> = requete(
                self.port,
                "DELETE",
                &format!("/session/{}", self.session),
                None,
            );
            self.session.clear();
        }
        // `safaridriver` ne s'arrête pas de lui-même.
        let _: std::io::Result<()> = self.processus.kill();
        let _: std::io::Result<std::process::ExitStatus> = self.processus.wait();
    }
}

impl Drop for Safari {
    fn drop(&mut self) {
        if matches!(self.processus.try_wait(), Ok(None)) {
            self.arreter();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use super::{erreur_webdriver, lire_reponse};

    #[test]
    fn une_reponse_de_safaridriver_se_lit() {
        let r = b"HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: 63\r\n\r\n{\"value\":{\"error\":\"session not created\",\"message\":\"timed out\"}}";
        let (statut, v) = lire_reponse(&mut BufReader::new(&r[..])).unwrap();
        assert_eq!(statut, 500);
        assert_eq!(
            erreur_webdriver(statut, &v).unwrap(),
            "session not created (HTTP 500) : timed out"
        );
        let ok = b"HTTP/1.1 200 OK\r\n\r\n{\"value\":{\"sessionId\":\"ab\"}}";
        let (statut, v) = lire_reponse(&mut BufReader::new(&ok[..])).unwrap();
        assert_eq!(statut, 200);
        assert_eq!(erreur_webdriver(statut, &v), None);
        assert!(lire_reponse(&mut BufReader::new(&b"pas http"[..])).is_err());
    }
}
