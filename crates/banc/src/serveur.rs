//! Un petit serveur HTTP sur la boucle locale : il sert les fichiers des pages de test et
//! reçoit leurs rapports (`POST /banc/rapport`).
//!
//! Une requête par connexion (`Connection: close`), lue en entier avant de répondre, sur
//! son propre fil. Il pose COOP et COEP : une page isolée (`crossOriginIsolated`) a
//! l'horloge la plus fine du navigateur, comme celle du harnais de mesure
//! (`mesures/navigateur/serveur.mjs`), et un Worker de l'autorité pourra s'y lancer.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Le chemin où une page envoie son rapport.
pub const RAPPORT: &str = "/banc/rapport";

/// La plus grande en-tête lue, en octets.
const ENTETE_MAX: usize = 64 * 1024;
/// Le plus grand corps reçu, en octets.
const CORPS_MAX: usize = 16 * 1024 * 1024;

/// Une requête lue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requete {
    /// `GET`, `POST`…
    pub methode: String,
    /// Le chemin, requête comprise (`/banc/rapport?suite=machine`).
    pub cible: String,
    /// Le corps, vide sans `Content-Length`.
    pub corps: Vec<u8>,
}

impl Requete {
    /// Le chemin, sans la requête.
    pub fn chemin(&self) -> &str {
        self.cible.split('?').next().unwrap_or_default()
    }
}

/// Lit une requête HTTP/1.1 : la ligne de requête, les en-têtes, puis `Content-Length`
/// octets de corps.
pub fn lire_requete(r: &mut impl BufRead) -> Result<Requete, String> {
    let mut entete = Vec::new();
    loop {
        let mut ligne = Vec::new();
        let n = r
            .by_ref()
            .take(u64::try_from(ENTETE_MAX).unwrap_or(u64::MAX))
            .read_until(b'\n', &mut ligne)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Err(String::from("connexion fermée avant la fin de l'en-tête"));
        }
        if ligne == b"\r\n" || ligne == b"\n" {
            break;
        }
        entete.extend_from_slice(&ligne);
        if entete.len() > ENTETE_MAX {
            return Err(String::from("en-tête trop longue"));
        }
    }
    let texte = String::from_utf8(entete).map_err(|_| String::from("en-tête non UTF-8"))?;
    let mut lignes = texte.lines();
    let premiere = lignes.next().unwrap_or_default();
    let mut mots = premiere.split(' ');
    let (Some(methode), Some(cible), Some(version)) = (mots.next(), mots.next(), mots.next())
    else {
        return Err(format!("ligne de requête illisible : {premiere}"));
    };
    if !version.starts_with("HTTP/1.") {
        return Err(format!("version illisible : {version}"));
    }
    let mut longueur = 0usize;
    for l in lignes {
        if let Some((nom, valeur)) = l.split_once(':')
            && nom.trim().eq_ignore_ascii_case("content-length")
        {
            longueur = valeur
                .trim()
                .parse()
                .map_err(|_| format!("Content-Length illisible : {valeur}"))?;
        }
    }
    if longueur > CORPS_MAX {
        return Err(format!(
            "corps de {longueur} octets, au-delà de {CORPS_MAX}"
        ));
    }
    let mut corps = vec![0; longueur];
    r.read_exact(&mut corps).map_err(|e| e.to_string())?;
    Ok(Requete {
        methode: String::from(methode),
        cible: String::from(cible),
        corps,
    })
}

/// Le fichier qu'un chemin désigne : sous la route dont il commence par le préfixe, sans
/// `..` ni chemin absolu. `None` si aucune route ne le sert.
pub fn resoudre(routes: &[(String, PathBuf)], chemin: &str) -> Option<PathBuf> {
    for (prefixe, dossier) in routes {
        let Some(reste) = chemin.strip_prefix(prefixe.as_str()) else {
            continue;
        };
        if reste.is_empty() || reste.contains('%') || reste.contains('\\') {
            return None;
        }
        let relatif = Path::new(reste);
        if !relatif
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            return None;
        }
        return Some(dossier.join(relatif));
    }
    None
}

/// Le type d'un fichier servi, par son extension.
pub fn type_de(chemin: &Path) -> &'static str {
    match chemin.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

fn repondre(flux: &mut TcpStream, code: &str, sorte: &str, corps: &[u8]) -> io::Result<()> {
    let entete = format!(
        "HTTP/1.1 {code}\r\nContent-Type: {sorte}\r\nContent-Length: {}\r\nConnection: close\r\n\
         Cache-Control: no-store\r\nCross-Origin-Opener-Policy: same-origin\r\n\
         Cross-Origin-Embedder-Policy: require-corp\r\nCross-Origin-Resource-Policy: same-origin\r\n\r\n",
        corps.len()
    );
    flux.write_all(entete.as_bytes())?;
    flux.write_all(corps)?;
    flux.flush()
}

fn servir(mut flux: TcpStream, routes: &[(String, PathBuf)], rapports: &Sender<Requete>) {
    let _: io::Result<()> = flux.set_read_timeout(Some(Duration::from_secs(10)));
    let lu = {
        let mut r = BufReader::new(&mut flux);
        lire_requete(&mut r)
    };
    let issue = match lu {
        Err(e) => repondre(&mut flux, "400 Bad Request", "text/plain", e.as_bytes()),
        Ok(q) if q.methode == "POST" && q.chemin() == RAPPORT => {
            let fait = rapports.send(q).is_ok();
            let code = if fait {
                "204 No Content"
            } else {
                "503 Service Unavailable"
            };
            repondre(&mut flux, code, "text/plain", b"")
        }
        Ok(q) if q.methode == "GET" => match resoudre(routes, q.chemin()) {
            Some(f) => match std::fs::read(&f) {
                Ok(o) => repondre(&mut flux, "200 OK", type_de(&f), &o),
                Err(_) => repondre(&mut flux, "404 Not Found", "text/plain", b""),
            },
            None => repondre(&mut flux, "404 Not Found", "text/plain", b""),
        },
        Ok(_) => repondre(&mut flux, "405 Method Not Allowed", "text/plain", b""),
    };
    let _: io::Result<()> = issue;
    let _: io::Result<()> = flux.shutdown(Shutdown::Both);
}

/// Le serveur, qui tourne tant qu'on ne l'arrête pas.
pub struct Serveur {
    adresse: SocketAddr,
    arret: Arc<AtomicBool>,
    fil: Option<JoinHandle<()>>,
}

impl Serveur {
    /// Démarre sur `127.0.0.1`, sur un port libre. `routes` associe un préfixe de chemin
    /// (`/banc/`) à un dossier ; chaque `POST /banc/rapport` part sur `rapports`.
    pub fn demarrer(
        routes: Vec<(String, PathBuf)>,
        rapports: Sender<Requete>,
    ) -> io::Result<Serveur> {
        let ecoute = TcpListener::bind("127.0.0.1:0")?;
        let adresse = ecoute.local_addr()?;
        let arret = Arc::new(AtomicBool::new(false));
        let a = Arc::clone(&arret);
        let routes = Arc::new(routes);
        let fil = thread::Builder::new()
            .name(String::from("banc-serveur"))
            .spawn(move || {
                for flux in ecoute.incoming() {
                    if a.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(flux) = flux else { continue };
                    let routes = Arc::clone(&routes);
                    let rapports = rapports.clone();
                    let _: io::Result<JoinHandle<()>> = thread::Builder::new()
                        .name(String::from("banc-requete"))
                        .spawn(move || servir(flux, &routes, &rapports));
                }
            })?;
        Ok(Serveur {
            adresse,
            arret,
            fil: Some(fil),
        })
    }

    /// `http://127.0.0.1:<port>`.
    pub fn origine(&self) -> String {
        format!("http://{}", self.adresse)
    }

    /// Arrête le serveur et attend son fil.
    pub fn arreter(mut self) {
        self.fermer();
    }

    fn fermer(&mut self) {
        self.arret.store(true, Ordering::SeqCst);
        // Une connexion à soi-même débloque `accept`.
        let _: io::Result<TcpStream> = TcpStream::connect(self.adresse);
        if let Some(f) = self.fil.take() {
            let _: thread::Result<()> = f.join();
        }
    }
}

impl Drop for Serveur {
    fn drop(&mut self) {
        self.fermer();
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{lire_requete, resoudre, type_de};

    #[test]
    fn une_requete_et_son_corps() {
        let mut r: &[u8] =
            b"POST /banc/rapport?suite=machine HTTP/1.1\r\nHost: x\r\ncontent-length: 5\r\n\r\nbonjour";
        let q = lire_requete(&mut r).unwrap();
        assert_eq!(q.methode, "POST");
        assert_eq!(q.chemin(), "/banc/rapport");
        assert_eq!(q.corps, b"bonjo");
    }

    #[test]
    fn une_requete_coupee_est_refusee() {
        let mut r: &[u8] = b"GET / HTTP/1.1\r\nHost: x\r\n";
        assert!(lire_requete(&mut r).is_err());
        let mut r: &[u8] = b"POST / HTTP/1.1\r\nContent-Length: 10\r\n\r\ncourt";
        assert!(lire_requete(&mut r).is_err());
        let mut r: &[u8] = b"POST / HTTP/1.1\r\nContent-Length: 999999999\r\n\r\n";
        assert!(lire_requete(&mut r).is_err());
    }

    #[test]
    fn un_chemin_ne_sort_pas_de_sa_route() {
        let routes = vec![(String::from("/banc/"), PathBuf::from("/pages"))];
        assert_eq!(
            resoudre(&routes, "/banc/machine.html"),
            Some(PathBuf::from("/pages/machine.html"))
        );
        for mauvais in [
            "/banc/../secret",
            "/banc/",
            "/banc//etc/passwd",
            "/banc/%2e%2e/x",
            "/autre/x",
            "/banc/a\\b",
        ] {
            assert_eq!(resoudre(&routes, mauvais), None, "{mauvais}");
        }
    }

    #[test]
    fn le_module_est_servi_comme_tel() {
        assert_eq!(type_de(Path::new("a/b.wasm")), "application/wasm");
        assert_eq!(type_de(Path::new("x.js")), "text/javascript; charset=utf-8");
    }
}
