//! Un client minimal de `WebDriver` `BiDi`, le protocole par lequel le banc pilote
//! Firefox (PHASE-3, § 8.2, lot 3b) : des commandes numérotées sur une WebSocket, leurs
//! réponses, et les événements qui arrivent entre elles.
//!
//! Comme [`crate::cdp`], synchrone : chaque lecture a un délai, posé sur le
//! `TcpStream`. Un message de `BiDi` dit lui-même ce qu'il est (`type` : `success`,
//! `error` ou `event`) ; une réponse porte l'`id` de sa commande.

use std::io;
use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};
use tungstenite::{Message, WebSocket};

use crate::cdp::hote;
use crate::horloge::{maintenant, reste};

/// Ce qu'un message de `BiDi` est, pour celui qui attend la réponse `id`.
#[derive(Debug, Clone, PartialEq)]
pub enum Recu {
    /// La réponse attendue, ou son erreur.
    Reponse(Result<Json, String>),
    /// Une réponse à une autre commande.
    Autre,
    /// Un événement (`method`, `params`).
    Evenement(Json),
}

/// Lit un message de `BiDi`.
pub fn lire_message(texte: &str, id: u64) -> Result<Recu, String> {
    let v: Json = serde_json::from_str(texte).map_err(|e| format!("BiDi illisible : {e}"))?;
    let sorte = v.get("type").and_then(Json::as_str).unwrap_or_default();
    if sorte == "event" {
        return Ok(Recu::Evenement(v));
    }
    match v.get("id").and_then(Json::as_u64) {
        Some(i) if i == id => match sorte {
            "success" => Ok(Recu::Reponse(Ok(v
                .get("result")
                .cloned()
                .unwrap_or(Json::Null)))),
            "error" => Ok(Recu::Reponse(Err(format!(
                "{} : {}",
                v.get("error").and_then(Json::as_str).unwrap_or("?"),
                v.get("message").and_then(Json::as_str).unwrap_or_default()
            )))),
            autre => Err(format!("BiDi : réponse de type « {autre} » : {v}")),
        },
        Some(_) => Ok(Recu::Autre),
        // Une erreur sans id : une commande illisible pour le navigateur.
        None => Err(format!("BiDi : message sans id ni événement : {v}")),
    }
}

/// Une connexion `BiDi` au navigateur.
pub struct Bidi {
    ws: WebSocket<TcpStream>,
    suivant: u64,
    evenements: Vec<Json>,
}

fn delai_depasse(e: &tungstenite::Error) -> bool {
    matches!(e, tungstenite::Error::Io(e)
        if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut)
}

impl Bidi {
    /// Se connecte à l'adresse d'une session `BiDi` (`ws://…/session`).
    pub fn ouvrir(url: &str) -> Result<Bidi, String> {
        let flux = TcpStream::connect(hote(url)?).map_err(|e| format!("BiDi {url} : {e}"))?;
        flux.set_nodelay(true).map_err(|e| e.to_string())?;
        flux.set_read_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| e.to_string())?;
        let (ws, _) = tungstenite::client(url, flux).map_err(|e| format!("BiDi {url} : {e}"))?;
        Ok(Bidi {
            ws,
            suivant: 1,
            evenements: Vec::new(),
        })
    }

    fn lire(&mut self, echeance: Instant) -> Result<Option<String>, String> {
        let r = reste(echeance).max(Duration::from_millis(1));
        self.ws
            .get_mut()
            .set_read_timeout(Some(r))
            .map_err(|e| e.to_string())?;
        match self.ws.read() {
            Ok(Message::Text(t)) => Ok(Some(t.as_str().to_owned())),
            Ok(Message::Binary(_) | Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => {
                Ok(None)
            }
            Ok(Message::Close(_)) => Err(String::from("BiDi fermé par le navigateur")),
            Err(e) if delai_depasse(&e) => Ok(None),
            Err(e) => Err(format!("BiDi : {e}")),
        }
    }

    /// Envoie `methode` et attend sa réponse au plus `delai`. Les événements reçus
    /// entre-temps sont gardés.
    pub fn envoyer(
        &mut self,
        methode: &str,
        params: &Json,
        delai: Duration,
    ) -> Result<Json, String> {
        let id = self.suivant;
        self.suivant = self.suivant.saturating_add(1);
        let m = json!({ "id": id, "method": methode, "params": params });
        self.ws
            .send(Message::Text(m.to_string().into()))
            .map_err(|e| format!("BiDi {methode} : {e}"))?;
        let echeance = maintenant()
            .checked_add(delai)
            .ok_or_else(|| String::from("délai trop long"))?;
        loop {
            if reste(echeance).is_zero() {
                return Err(format!("BiDi {methode} : pas de réponse en {delai:?}"));
            }
            let Some(t) = self.lire(echeance)? else {
                continue;
            };
            match lire_message(&t, id)? {
                Recu::Reponse(r) => return r.map_err(|e| format!("BiDi {methode} : {e}")),
                Recu::Autre => {}
                Recu::Evenement(e) => self.evenements.push(e),
            }
        }
    }

    /// Garde les événements qui arrivent pendant au plus `delai`.
    pub fn ecouter(&mut self, delai: Duration) -> Result<(), String> {
        let echeance = maintenant()
            .checked_add(delai)
            .ok_or_else(|| String::from("délai trop long"))?;
        while !reste(echeance).is_zero() {
            if let Some(t) = self.lire(echeance)?
                && let Recu::Evenement(e) = lire_message(&t, 0)?
            {
                self.evenements.push(e);
            }
        }
        Ok(())
    }

    /// Les événements gardés depuis la dernière fois.
    pub fn evenements(&mut self) -> Vec<Json> {
        std::mem::take(&mut self.evenements)
    }
}

/// Ce qu'un événement de la page dit, en une ligne, s'il dit quelque chose : une ligne
/// de la console, une exception (`log.entryAdded`).
pub fn ligne_d_evenement(e: &Json) -> Option<String> {
    if e.get("method")?.as_str()? != "log.entryAdded" {
        return None;
    }
    let p = e.get("params")?;
    let niveau = p.get("level").and_then(Json::as_str).unwrap_or("?");
    let texte = p.get("text").and_then(Json::as_str).unwrap_or("?");
    match p.get("type").and_then(Json::as_str) {
        Some("javascript") => Some(format!("exception : {texte}")),
        Some("console") => {
            let methode = p.get("method").and_then(Json::as_str).unwrap_or(niveau);
            Some(format!("console.{methode} : {texte}"))
        }
        _ => Some(format!("journal {niveau} : {texte}")),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Recu, ligne_d_evenement, lire_message};

    #[test]
    fn une_reponse_un_evenement_une_erreur() {
        assert_eq!(
            lire_message(r#"{"type":"success","id":3,"result":{"a":1}}"#, 3).unwrap(),
            Recu::Reponse(Ok(json!({"a": 1})))
        );
        assert_eq!(
            lire_message(r#"{"type":"success","id":2,"result":{}}"#, 3).unwrap(),
            Recu::Autre
        );
        assert!(matches!(
            lire_message(
                r#"{"type":"error","id":3,"error":"unknown command","message":"non"}"#,
                3
            )
            .unwrap(),
            Recu::Reponse(Err(e)) if e.contains("unknown command") && e.contains("non")
        ));
        assert!(matches!(
            lire_message(
                r#"{"type":"event","method":"log.entryAdded","params":{}}"#,
                3
            )
            .unwrap(),
            Recu::Evenement(_)
        ));
        assert!(lire_message(r#"{"type":"error","error":"invalid argument"}"#, 1).is_err());
        assert!(lire_message("pas du json", 1).is_err());
    }

    #[test]
    fn une_ligne_par_evenement_qui_parle() {
        let e = json!({"type": "event", "method": "log.entryAdded",
            "params": {"type": "console", "method": "error", "level": "error", "text": "zut 3"}});
        assert_eq!(ligne_d_evenement(&e).unwrap(), "console.error : zut 3");
        let e = json!({"type": "event", "method": "log.entryAdded",
            "params": {"type": "javascript", "level": "error", "text": "RuntimeError: unreachable"}});
        assert_eq!(
            ligne_d_evenement(&e).unwrap(),
            "exception : RuntimeError: unreachable"
        );
        assert_eq!(
            ligne_d_evenement(
                &json!({"type": "event", "method": "browsingContext.load", "params": {}})
            ),
            None
        );
    }
}
