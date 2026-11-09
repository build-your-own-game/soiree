//! Le HTTP du relais, sans entrées-sorties (PHASE-3, § 2.5) : lire la requête avant de
//! choisir entre une page et le WebSocket, sur le seul port du QR ; juger **qui parle**
//! (SAL-33) ; les routes, servies d'une table en mémoire, sans chemin de fichier
//! (SAL-150) ; les réponses et leurs en-têtes.
//!
//! # Qui parle au relais (SAL-33)
//!
//! L'adresse d'où vient la connexion ne suffit pas : n'importe quel site ouvert sur
//! l'ordinateur de la soirée peut ouvrir `ws://127.0.0.1:7878/ws` (une WebSocket n'est pas
//! soumise au CORS), et un rebond DNS atteint les routes de la boucle locale. Donc :
//!
//! - **aucun en-tête de mandataire** (`Forwarded`, `X-Forwarded-For`, `X-Forwarded-Host`,
//!   `X-Real-IP`, `Via`) ;
//! - **depuis la boucle locale**, un `Host` parmi `localhost:<port>`, `127.0.0.1:<port>`
//!   et `[::1]:<port>` ; pour le WebSocket, une `Origin` exactement
//!   `http://localhost:<port>` donne tous les rôles ([`Provenance::BoucleLocale`]), et
//!   aucune `Origin` aussi, pour les outils natifs du banc, avec `--banc` seulement ; une
//!   `Origin` d'une autre page du relais (`http://127.0.0.1:<port>`, `http://[::1]:<port>`
//!   ou une adresse annoncée) ne donne que le rôle `Appareil` ;
//! - **depuis le réseau**, un `Host` et une `Origin` d'une adresse que le relais annonce :
//!   le rôle `Appareil` seulement ([`Provenance::Reseau`]).
//!
//! Le reste est étranger : `403`, avant tout WebSocket.

use std::fmt::Write as _;
use std::net::IpAddr;

use liaison::CodeSalle;

use crate::coeur::Provenance;

/// La plus grande en-tête de requête lue : 16 Kio (choisi).
pub const ENTETE_MAX: usize = 16 * 1024;

/// Les en-têtes lus au plus.
const ENTETES_MAX: usize = 48;

/// Une requête lue : sa méthode, son chemin, et ses en-têtes, noms en minuscules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requete {
    /// La méthode.
    pub methode: String,
    /// Le chemin, tel qu'écrit.
    pub chemin: String,
    /// Les en-têtes, dans l'ordre, le nom en minuscules.
    pub entetes: Vec<(String, String)>,
}

impl Requete {
    /// La valeur d'un en-tête (nom en minuscules), s'il est là une seule fois. Deux
    /// valeurs du même en-tête se lisent comme aucune : on ne choisit pas.
    pub fn entete(&self, nom: &str) -> Option<&str> {
        let mut vus = self.entetes.iter().filter(|(n, _)| n == nom);
        let premier = vus.next()?;
        if vus.next().is_some() {
            return None;
        }
        Some(premier.1.as_str())
    }

    /// L'en-tête est-il là, une fois ou plus ?
    pub fn a(&self, nom: &str) -> bool {
        self.entetes.iter().any(|(n, _)| n == nom)
    }
}

/// Une requête illisible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Illisible {
    /// L'en-tête dépasse [`ENTETE_MAX`].
    TropLongue,
    /// Ce n'est pas du HTTP/1.1.
    Forme,
}

/// Lit l'en-tête d'une requête au début de `octets`. `Ok(None)` : il en faut plus.
/// `Ok(Some((requete, n)))` : l'en-tête occupe les `n` premiers octets.
pub fn lire(octets: &[u8]) -> Result<Option<(Requete, usize)>, Illisible> {
    let mut entetes = [httparse::EMPTY_HEADER; ENTETES_MAX];
    let mut r = httparse::Request::new(&mut entetes);
    match r.parse(octets) {
        Ok(httparse::Status::Complete(n)) => {
            let methode = r.method.ok_or(Illisible::Forme)?.to_owned();
            let chemin = r.path.ok_or(Illisible::Forme)?.to_owned();
            let mut liste = Vec::new();
            for e in r.headers.iter() {
                let valeur = core::str::from_utf8(e.value).map_err(|_| Illisible::Forme)?;
                liste.push((e.name.to_ascii_lowercase(), valeur.trim().to_owned()));
            }
            Ok(Some((
                Requete {
                    methode,
                    chemin,
                    entetes: liste,
                },
                n,
            )))
        }
        Ok(httparse::Status::Partial) => {
            if octets.len() >= ENTETE_MAX {
                Err(Illisible::TropLongue)
            } else {
                Ok(None)
            }
        }
        Err(_) => Err(Illisible::Forme),
    }
}

/// Ce qu'il faut pour juger une requête.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contexte {
    /// Le port du relais (celui du QR).
    pub port: u16,
    /// Les adresses que le relais annonce aux téléphones.
    pub annoncees: Vec<IpAddr>,
    /// `--banc` : les outils natifs du banc, sans `Origin`, prennent tous les rôles depuis
    /// la boucle locale.
    pub banc: Banc,
}

/// Le relais du banc, ou celui d'une soirée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Banc {
    /// `--banc`.
    Oui,
    /// Une soirée.
    Non,
}

/// D'où vient une requête, une fois jugée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Jugement {
    /// Une provenance admise.
    Admise(Provenance),
    /// Étrangère, avec la raison (qui n'est dite qu'au journal du relais).
    Etrangere(&'static str),
}

/// Les en-têtes de mandataire qu'une requête ne porte jamais (SAL-33).
pub const ENTETES_DE_MANDATAIRE: [&str; 5] = [
    "forwarded",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-real-ip",
    "via",
];

/// L'écriture d'une adresse dans un `Host` ou une `Origin` : `[::1]` pour IPv6.
pub fn hote(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(v4) => format!("{v4}:{port}"),
        IpAddr::V6(v6) => format!("[{v6}]:{port}"),
    }
}

fn hotes_locaux(port: u16) -> [String; 3] {
    [
        format!("localhost:{port}"),
        format!("127.0.0.1:{port}"),
        format!("[::1]:{port}"),
    ]
}

/// L'`Origin` de l'écran : `http://localhost:<port>`, la seule origine de la page de
/// l'écran et de son Worker (P14).
pub fn origine_de_l_ecran(port: u16) -> String {
    format!("http://localhost:{port}")
}

/// Une adresse IP de la boucle locale, IPv4 ou IPv6 (y compris `::ffff:127.0.0.1`).
pub fn est_boucle(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
    }
}

/// Juge qui ouvre le WebSocket (SAL-33).
pub fn juger_websocket(pair: IpAddr, r: &Requete, c: &Contexte) -> Jugement {
    if ENTETES_DE_MANDATAIRE.iter().any(|e| r.a(e)) {
        return Jugement::Etrangere("un en-tête de mandataire");
    }
    let Some(host) = r.entete("host") else {
        return Jugement::Etrangere("sans Host, ou deux");
    };
    let origine = match r.entete("origin") {
        None if r.a("origin") => return Jugement::Etrangere("deux Origin"),
        o => o,
    };
    let annoncees_hotes: Vec<String> = c.annoncees.iter().map(|ip| hote(*ip, c.port)).collect();
    if est_boucle(pair) {
        if !hotes_locaux(c.port).iter().any(|h| h == host) {
            // Un rebond DNS : un nom étranger qui mène à la boucle locale.
            return Jugement::Etrangere("un Host qui n'est pas local (rebond DNS)");
        }
        return match origine {
            None if c.banc == Banc::Oui => Jugement::Admise(Provenance::BoucleLocale),
            None => Jugement::Etrangere("sans Origin, hors du banc"),
            Some(o) if o == origine_de_l_ecran(c.port) => {
                Jugement::Admise(Provenance::BoucleLocale)
            }
            Some(o) => {
                let pages_du_relais = hotes_locaux(c.port)
                    .iter()
                    .skip(1)
                    .chain(annoncees_hotes.iter())
                    .any(|h| o == format!("http://{h}"));
                if pages_du_relais {
                    Jugement::Admise(Provenance::Reseau)
                } else {
                    Jugement::Etrangere("une Origin étrangère")
                }
            }
        };
    }
    if !annoncees_hotes.iter().any(|h| h == host) {
        return Jugement::Etrangere("un Host qui n'est pas une adresse annoncée");
    }
    match origine {
        Some(o) if annoncees_hotes.iter().any(|h| o == format!("http://{h}")) => {
            Jugement::Admise(Provenance::Reseau)
        }
        Some(_) | None => Jugement::Etrangere("une Origin qui n'est pas une adresse annoncée"),
    }
}

/// Une route de la boucle locale (la page de l'écran, `GET /salle`) est-elle permise à
/// cette requête ? Depuis la boucle locale, d'un `Host` local, sans en-tête de
/// mandataire, sans `Origin` étrangère (SAL-33).
pub fn boucle_locale_permise(pair: IpAddr, r: &Requete, c: &Contexte) -> bool {
    if !est_boucle(pair) || ENTETES_DE_MANDATAIRE.iter().any(|e| r.a(e)) {
        return false;
    }
    let Some(host) = r.entete("host") else {
        return false;
    };
    if !hotes_locaux(c.port).iter().any(|h| h == host) {
        return false;
    }
    match r.entete("origin") {
        None => !r.a("origin"),
        Some(o) => o == origine_de_l_ecran(c.port),
    }
}

/// Une route que tous peuvent prendre (la page d'un téléphone) : sans en-tête de
/// mandataire, d'un `Host` local ou annoncé.
pub fn publique_permise(pair: IpAddr, r: &Requete, c: &Contexte) -> bool {
    if ENTETES_DE_MANDATAIRE.iter().any(|e| r.a(e)) {
        return false;
    }
    let Some(host) = r.entete("host") else {
        return false;
    };
    let annoncee = c.annoncees.iter().any(|ip| hote(*ip, c.port) == host);
    annoncee || (est_boucle(pair) && hotes_locaux(c.port).iter().any(|h| h == host))
}

/// La page d'un téléphone vient-elle du réseau ? Rend l'adresse que l'aide suit
/// (`crate::aide`) : celle du pair, hors de la boucle locale. Avec `--banc` seulement, une
/// page demandée depuis la boucle d'un `Host` annoncé compte aussi comme venue du réseau :
/// le banc joue ainsi un téléphone du Wi-Fi sans rien ouvrir sur le Wi-Fi (l'adresse que
/// le relais du banc annonce sans l'écouter, `Reglages::annonce_du_banc`).
pub fn page_du_reseau(pair: IpAddr, r: &Requete, c: &Contexte) -> Option<IpAddr> {
    if !est_boucle(pair) {
        return Some(pair);
    }
    let host = r.entete("host")?;
    (c.banc == Banc::Oui && c.annoncees.iter().any(|ip| hote(*ip, c.port) == host)).then_some(pair)
}

/// Ce qu'une requête demande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// `GET /` : la page de démarrage, en mode écran (boucle locale).
    Demarrage,
    /// `GET /s/<salle>` : la même page, en mode téléphone (tous).
    Telephone(CodeSalle),
    /// `GET /web/<empreinte>/<fichier>` : le paquet de la page (tous), servi de la table.
    Web(String),
    /// `GET /ws` : le WebSocket.
    Ws,
    /// `GET /salle` : le code de salle que le relais garde (boucle locale, P41).
    Salle,
    /// `GET /jeux/<reste>` : le catalogue (`catalogue`), un module livré
    /// (`<empreinte>.wasm`) ou son rapport d'épreuve (`<empreinte>.json`), pour l'écran
    /// (boucle locale, P16).
    Jeux(String),
    /// `GET /aide` : ce que le relais sait des téléphones qui n'arrivent pas, et du
    /// pare-feu, pour l'aide de l'écran (boucle locale ; lot 10b).
    Aide,
    /// `GET /mesure` : la page en mode mesure du Wi-Fi, pour un téléphone (tous ; lot 14,
    /// `crate::mesure`).
    Mesure,
    /// `GET /mesure/ws` : la WebSocket de la mesure, échos et tics (tous ; lot 14).
    MesureWs,
    /// `GET /mesure/bilans` : les adresses de la page de mesure et les derniers bilans,
    /// pour la télé (boucle locale ; lot 14).
    MesureBilans,
    /// Rien de connu : `404`.
    Introuvable,
    /// Une autre méthode que `GET` : `405`.
    Methode,
}

/// Le chemin, sans sa requête (`?…`).
fn sans_requete(chemin: &str) -> &str {
    chemin.split_once('?').map_or(chemin, |(c, _)| c)
}

/// La route d'une requête. Un chemin n'est jamais un chemin de fichier : il se compare à
/// des noms, et rien d'autre n'est servi (SAL-150).
pub fn router(r: &Requete) -> Route {
    if r.methode != "GET" {
        return Route::Methode;
    }
    let chemin = sans_requete(&r.chemin);
    match chemin {
        "/" => Route::Demarrage,
        "/ws" => Route::Ws,
        "/salle" => Route::Salle,
        "/aide" => Route::Aide,
        "/mesure" => Route::Mesure,
        "/mesure/ws" => Route::MesureWs,
        "/mesure/bilans" => Route::MesureBilans,
        _ => {
            if let Some(code) = chemin.strip_prefix("/s/") {
                return CodeSalle::lire(code).map_or(Route::Introuvable, Route::Telephone);
            }
            if let Some(reste) = chemin.strip_prefix("/web/") {
                return Route::Web(reste.to_owned());
            }
            if let Some(reste) = chemin.strip_prefix("/jeux/") {
                return Route::Jeux(reste.to_owned());
            }
            Route::Introuvable
        }
    }
}

/// La politique de contenu de chaque page (PHASE-3, § 2.5). *Au lot 10a* : `img-src 'self'
/// blob:` de plus : la télé décode une image SVG par un élément `<img>` sur un `blob:` de
/// ses octets (P23), que `default-src 'self'` refusait (constaté dans Chrome et Firefox,
/// les SVG de Tier list).
pub const POLITIQUE: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; img-src 'self' blob:; connect-src 'self'";

/// Une réponse HTTP entière, qui ferme la connexion.
pub fn reponse(statut: &str, type_: &str, corps: &[u8], entetes: &[(&str, &str)]) -> Vec<u8> {
    let mut tete = String::new();
    let _: Result<(), core::fmt::Error> = write!(
        tete,
        "HTTP/1.1 {statut}\r\nContent-Type: {type_}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: {POLITIQUE}\r\n",
        corps.len()
    );
    for (n, v) in entetes {
        let _: Result<(), core::fmt::Error> = write!(tete, "{n}: {v}\r\n");
    }
    tete.push_str("\r\n");
    let mut o = tete.into_bytes();
    o.extend_from_slice(corps);
    o
}

/// Une page HTML : jamais mise en cache.
pub fn page(corps: &[u8], isolee: Isolement) -> Vec<u8> {
    let mut e = vec![("Cache-Control", "no-store")];
    if isolee == Isolement::Oui {
        // L'écran est isolé (SAL-149) : `performance.now()` à 5 µs dans Chrome.
        e.push(("Cross-Origin-Opener-Policy", "same-origin"));
        e.push(("Cross-Origin-Embedder-Policy", "require-corp"));
    }
    reponse("200 OK", "text/html; charset=utf-8", corps, &e)
}

/// Une page isolée (COOP et COEP), ou non.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Isolement {
    /// La page de l'écran.
    Oui,
    /// La page d'un téléphone.
    Non,
}

/// Un fichier du paquet : son chemin porte l'empreinte du paquet, il ne change jamais
/// (`immutable`). L'écran est isolé (COOP, COEP) : chaque fichier le dit aussi, et dit
/// qu'il se charge de la même origine seulement (CORP).
pub fn fichier_du_paquet(type_: &str, corps: &[u8]) -> Vec<u8> {
    reponse(
        "200 OK",
        type_,
        corps,
        &[
            ("Cache-Control", "public, max-age=31536000, immutable"),
            ("Cross-Origin-Resource-Policy", "same-origin"),
            ("Cross-Origin-Opener-Policy", "same-origin"),
            ("Cross-Origin-Embedder-Policy", "require-corp"),
        ],
    )
}

/// Une erreur, en texte.
pub fn erreur(statut: &str) -> Vec<u8> {
    reponse(
        statut,
        "text/plain; charset=utf-8",
        statut.as_bytes(),
        &[("Cache-Control", "no-store")],
    )
}

/// La réponse au WebSocket, si la requête en est une demande valide (RFC 6455, § 4.2).
pub fn accepter_websocket(r: &Requete) -> Option<Vec<u8>> {
    let upgrade = r.entete("upgrade")?.eq_ignore_ascii_case("websocket");
    let connexion = r
        .entete("connection")?
        .split(',')
        .any(|m| m.trim().eq_ignore_ascii_case("upgrade"));
    let version = r.entete("sec-websocket-version")? == "13";
    let cle = r.entete("sec-websocket-key")?;
    if !(upgrade && connexion && version) {
        return None;
    }
    let accepte = tungstenite::handshake::derive_accept_key(cle.as_bytes());
    Some(
        format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accepte}\r\n\r\n"
        )
        .into_bytes(),
    )
}
