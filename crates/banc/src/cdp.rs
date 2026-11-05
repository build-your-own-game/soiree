//! Un client minimal du protocole `DevTools` de Chrome (CDP) : des requêtes numérotées sur
//! une WebSocket, leurs réponses, et les événements qui arrivent entre elles.
//!
//! Synchrone : chaque lecture a un délai, posé sur le `TcpStream`. Le mode « à plat »
//! (`Target.attachToTarget` avec `flatten`) fait passer une page par la même WebSocket
//! que le navigateur, chaque message portant son `sessionId`.

use std::io;
use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};
use tungstenite::{Message, WebSocket};

use crate::horloge::{maintenant, reste};

/// Ce qu'un message de CDP est, pour celui qui attend la réponse `id`.
#[derive(Debug, Clone, PartialEq)]
pub enum Recu {
    /// La réponse attendue, ou son erreur.
    Reponse(Result<Json, String>),
    /// Une réponse à une autre requête.
    Autre,
    /// Un événement (`method`, `params`, `sessionId`).
    Evenement(Json),
}

/// Lit un message de CDP.
pub fn lire_message(texte: &str, id: u64) -> Result<Recu, String> {
    let v: Json = serde_json::from_str(texte).map_err(|e| format!("CDP illisible : {e}"))?;
    match v.get("id").and_then(Json::as_u64) {
        Some(i) if i == id => {
            if let Some(e) = v.get("error") {
                Ok(Recu::Reponse(Err(e.to_string())))
            } else {
                Ok(Recu::Reponse(Ok(v
                    .get("result")
                    .cloned()
                    .unwrap_or(Json::Null))))
            }
        }
        Some(_) => Ok(Recu::Autre),
        None => Ok(Recu::Evenement(v)),
    }
}

/// L'hôte et le port d'une adresse `ws://hôte:port/chemin`.
pub fn hote(url: &str) -> Result<&str, String> {
    url.strip_prefix("ws://")
        .and_then(|r| r.split('/').next())
        .filter(|h| !h.is_empty())
        .ok_or_else(|| format!("adresse CDP illisible : {url}"))
}

/// Une connexion CDP au navigateur.
pub struct Cdp {
    ws: WebSocket<TcpStream>,
    suivant: u64,
    evenements: Vec<Json>,
}

fn delai_depasse(e: &tungstenite::Error) -> bool {
    matches!(e, tungstenite::Error::Io(e)
        if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut)
}

impl Cdp {
    /// Se connecte à l'adresse que Chrome donne (`DevTools listening on ws://…`).
    pub fn ouvrir(url: &str) -> Result<Cdp, String> {
        let flux = TcpStream::connect(hote(url)?).map_err(|e| format!("CDP {url} : {e}"))?;
        flux.set_nodelay(true).map_err(|e| e.to_string())?;
        flux.set_read_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| e.to_string())?;
        let (ws, _) = tungstenite::client(url, flux).map_err(|e| format!("CDP {url} : {e}"))?;
        Ok(Cdp {
            ws,
            suivant: 1,
            evenements: Vec::new(),
        })
    }

    /// Lit un message avant `echeance` ; `None` si rien n'est venu.
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
            Ok(Message::Close(_)) => Err(String::from("CDP fermé par le navigateur")),
            Err(e) if delai_depasse(&e) => Ok(None),
            Err(e) => Err(format!("CDP : {e}")),
        }
    }

    /// Envoie `methode` (dans la page `session`, s'il y en a une) et attend sa réponse au
    /// plus `delai`. Les événements reçus entre-temps sont gardés.
    pub fn envoyer(
        &mut self,
        methode: &str,
        params: &Json,
        session: Option<&str>,
        delai: Duration,
    ) -> Result<Json, String> {
        let id = self.suivant;
        self.suivant = self.suivant.saturating_add(1);
        let mut m = json!({ "id": id, "method": methode, "params": params });
        if let (Some(s), Some(o)) = (session, m.as_object_mut()) {
            o.insert(String::from("sessionId"), Json::from(s));
        }
        self.ws
            .send(Message::Text(m.to_string().into()))
            .map_err(|e| format!("CDP {methode} : {e}"))?;
        let echeance = maintenant()
            .checked_add(delai)
            .ok_or_else(|| String::from("délai trop long"))?;
        loop {
            if reste(echeance).is_zero() {
                return Err(format!("CDP {methode} : pas de réponse en {delai:?}"));
            }
            let Some(t) = self.lire(echeance)? else {
                continue;
            };
            match lire_message(&t, id)? {
                Recu::Reponse(r) => return r.map_err(|e| format!("CDP {methode} : {e}")),
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

/// Ce qu'un événement de la page dit, en une ligne, s'il dit quelque chose : une
/// exception, une ligne de la console, une entrée du journal du navigateur.
pub fn ligne_d_evenement(e: &Json) -> Option<String> {
    let methode = e.get("method")?.as_str()?;
    let p = e.get("params")?;
    match methode {
        "Runtime.exceptionThrown" => {
            let d = p.get("exceptionDetails")?;
            let texte = d
                .pointer("/exception/description")
                .and_then(Json::as_str)
                .or_else(|| d.get("text").and_then(Json::as_str))
                .unwrap_or("?");
            Some(format!("exception : {texte}"))
        }
        "Runtime.consoleAPICalled" => {
            let sorte = p.get("type").and_then(Json::as_str).unwrap_or("?");
            let args: Vec<String> = p
                .get("args")?
                .as_array()?
                .iter()
                .map(|a| {
                    a.get("value")
                        .map(|v| v.as_str().map_or_else(|| v.to_string(), String::from))
                        .or_else(|| {
                            a.get("description")
                                .and_then(Json::as_str)
                                .map(String::from)
                        })
                        .unwrap_or_default()
                })
                .collect();
            Some(format!("console.{sorte} : {}", args.join(" ")))
        }
        "Log.entryAdded" => {
            let e = p.get("entry")?;
            Some(format!(
                "journal {} : {} {}",
                e.get("level").and_then(Json::as_str).unwrap_or("?"),
                e.get("text").and_then(Json::as_str).unwrap_or("?"),
                e.get("url").and_then(Json::as_str).unwrap_or_default()
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Recu, hote, ligne_d_evenement, lire_message};

    #[test]
    fn une_reponse_un_evenement_une_erreur() {
        assert_eq!(
            lire_message(r#"{"id":3,"result":{"a":1}}"#, 3).unwrap(),
            Recu::Reponse(Ok(json!({"a": 1})))
        );
        assert_eq!(
            lire_message(r#"{"id":2,"result":{}}"#, 3).unwrap(),
            Recu::Autre
        );
        assert!(matches!(
            lire_message(r#"{"id":3,"error":{"code":-32000,"message":"non"}}"#, 3).unwrap(),
            Recu::Reponse(Err(e)) if e.contains("non")
        ));
        assert!(matches!(
            lire_message(r#"{"method":"Page.loadEventFired","params":{}}"#, 3).unwrap(),
            Recu::Evenement(_)
        ));
        assert!(lire_message("pas du json", 1).is_err());
    }

    #[test]
    fn l_hote_d_une_adresse() {
        assert_eq!(
            hote("ws://127.0.0.1:9222/devtools/browser/abc").unwrap(),
            "127.0.0.1:9222"
        );
        assert!(hote("http://x/").is_err());
        assert!(hote("ws:///x").is_err());
    }

    #[test]
    fn une_ligne_par_evenement_qui_parle() {
        let e = json!({"method": "Runtime.consoleAPICalled",
            "params": {"type": "error", "args": [{"type": "string", "value": "zut"}, {"type": "number", "value": 3}]}});
        assert_eq!(ligne_d_evenement(&e).unwrap(), "console.error : zut 3");
        let e = json!({"method": "Runtime.exceptionThrown",
            "params": {"exceptionDetails": {"text": "Uncaught", "exception": {"description": "RuntimeError: unreachable"}}}});
        assert_eq!(
            ligne_d_evenement(&e).unwrap(),
            "exception : RuntimeError: unreachable"
        );
        assert_eq!(
            ligne_d_evenement(&json!({"method": "Page.frameNavigated", "params": {}})),
            None
        );
    }
}
