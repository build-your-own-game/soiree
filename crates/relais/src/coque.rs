//! La coque `std` du relais (PHASE-3, § 2.5) : **sans runtime asynchrone**.
//!
//! - **Les écoutes** : `127.0.0.1` **et** `[::1]` sur le même port (Chrome essaie aussi
//!   `::1` pour `localhost`), plus chaque adresse annoncée du réseau local. Si le port est
//!   pris dans l'une des deux familles de la boucle locale, le relais refuse de démarrer,
//!   haut et clair : un autre programme sur `[::1]:7878` capterait sinon `localhost`.
//! - **L'admission avant le fil** (SAL-143) : chaque connexion acceptée est admise ou
//!   fermée sur le fil d'écoute, avant qu'un fil ne lui soit donné.
//! - **Par lien, un fil de lecture et un fil d'écriture** (SAL-145, SAL-146) : un envoi
//!   n'attend jamais une lecture ; les deux écrivent des trames entières sous un seul
//!   verrou (le fil de lecture n'écrit que les réponses automatiques de WebSocket, un
//!   `Pong` ou la fermeture) ; la fin de la lecture ferme la file, rejoint l'écrivain,
//!   puis libère la place (SAL-145). `TCP_NODELAY` (REM-22). Chaque connexion tourne sous
//!   `catch_unwind` : une panique ne coûte que son lien (SAL-79).
//! - **Un fil possède les salles** (SAL-141) : le [`Coeur`], à qui tout arrive par un
//!   canal, dans l'ordre ; il pousse dans la file de chaque lien ([`FileSortante`]).
//! - **Un fil fait la garde sur disque**, dans l'ordre des demandes : un gros point ne
//!   retient pas le temps réel, et `Ecrit` part dans l'ordre des `Ecrire`.
//! - **L'enregistrement** (`--enregistrer`) : chaque trame reçue et envoyée.

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use liaison::{AdresseAnnoncee, CauseFermeture, CodeSalle, Lien, TRAME_MAX};
use tungstenite::protocol::frame::Frame;
use tungstenite::protocol::frame::coding::{Data, OpCode};
use tungstenite::protocol::{Role, WebSocket, WebSocketConfig};
use tungstenite::{Bytes, Message};

use crate::admission::{Admission, Pair};
use crate::adresses::{self, Interface};
use crate::coeur::{CASE_MAX, Coeur, Fait, OpGarde, Provenance, RoleAdmis, Sortie};
use crate::enregistrement::{Enregistreur, Sens};
use crate::file::{Envoi, FileSortante};
use crate::garde::{Dossier, GARDE_MAX, PreuvesSurDisque};
use crate::horloge::maintenant_us;
use crate::http::{self, Banc, Contexte, Isolement, Jugement, Route};
use crate::paquet::{self, Jeux, Paquet};

/// Le port du relais par défaut (P14).
pub const PORT: u16 = 7878;

/// La file d'un appareil : 8 Mio (choisi), au-dessus d'un module compressé
/// (`TAILLE_MODULE_MAX`, 4 Mio, § 2.8) et de la plus grande vue d'un téléphone (128 Kio
/// chez Sidequest), pour qu'un téléphone qui revient ne tourne pas en boucle (SAL-120).
pub const FILE_APPAREIL: usize = 8 * 1024 * 1024;

/// La file de l'autorité : 32 Mio (choisi) ; elle reçoit les trames de tous les
/// appareils.
pub const FILE_AUTORITE: usize = 32 * 1024 * 1024;

/// La file de la garde : toute la garde d'une salle, et un peu plus.
pub const FILE_GARDE: usize = 257 * 1024 * 1024;

const _: () = assert!(FILE_GARDE as u64 > GARDE_MAX);

/// La plus longue attente d'une requête HTTP entière : 5 s (choisi, SAL-49).
const REQUETE_MAX: Duration = Duration::from_secs(5);

/// Le temps qu'un lien fermé par le cœur a pour partir : 2 s (choisi).
const FERMETURE_MAX_US: u64 = 2_000_000;

/// La pile d'un fil de lien : 256 Kio (choisi ; M15 mesure la mémoire du relais à 71
/// liens).
const PILE: usize = 256 * 1024;

/// Ce qu'il faut pour lancer le relais.
#[derive(Debug, Clone)]
pub struct Reglages {
    /// Le port : [`PORT`] en soirée ; 0, un port libre, pour les tests.
    pub port: u16,
    /// `--adresse` : l'adresse du réseau local à annoncer, seule.
    pub adresse: Option<IpAddr>,
    /// Le dossier de la garde.
    pub garde: PathBuf,
    /// `--banc`.
    pub banc: Banc,
    /// `--enregistrer` : le fichier de l'enregistrement.
    pub enregistrer: Option<PathBuf>,
    /// Les interfaces de la machine ; lues du système si absentes (les tests en donnent).
    pub interfaces: Option<Vec<Interface>>,
    /// La page de démarrage (en attendant celle du lot 10b) ; sans elle, celle qui charge
    /// le paquet, s'il y en a un, sinon une page minimale.
    pub page: Option<Vec<u8>>,
    /// `--paquet` : le paquet de la page, lu au lancement (lot 10a).
    pub paquet: Option<Paquet>,
    /// `--jeux` : les modules livrés, lus au lancement (lot 10a).
    pub jeux: Jeux,
}

impl Reglages {
    /// Les réglages d'une soirée : le port fixe, la garde dans ce dossier.
    pub fn soiree(garde: &Path) -> Reglages {
        Reglages {
            port: PORT,
            adresse: None,
            garde: garde.to_path_buf(),
            banc: Banc::Non,
            enregistrer: None,
            interfaces: None,
            page: None,
            paquet: None,
            jeux: Jeux::default(),
        }
    }
}

/// Le relais ne démarre pas.
#[derive(Debug)]
pub struct ErreurLancement(pub String);

impl core::fmt::Display for ErreurLancement {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

/// La page de démarrage minimale, jusqu'à la vraie (lot 10b).
const PAGE_MINIMALE: &[u8] = b"<!doctype html><meta charset=\"utf-8\"><title>soiree</title><p>Le relais de la soiree tourne. La page de demarrage vient au lot 10b.</p>\n";

/// La file d'un lien, partagée entre le fil des salles et l'écrivain du lien.
#[derive(Debug)]
struct FileLien {
    file: Mutex<FileSortante>,
    signal: Condvar,
    /// Une copie de la connexion, hors du verrou de l'écrivain : la couper débloque un
    /// écrivain arrêté sur un pair qui ne lit plus.
    connexion: TcpStream,
}

impl FileLien {
    fn new(borne: usize, connexion: TcpStream) -> FileLien {
        FileLien {
            file: Mutex::new(FileSortante::new(borne)),
            signal: Condvar::new(),
            connexion,
        }
    }

    /// Coupe la connexion tout de suite, sans rien attendre.
    fn couper(&self) {
        self.avec(FileSortante::fermer_sans_attendre);
        let _: io::Result<()> = self.connexion.shutdown(Shutdown::Both);
    }

    fn avec<R>(&self, f: impl FnOnce(&mut FileSortante) -> R) -> Option<R> {
        let r = self.file.lock().ok().map(|mut g| f(&mut g));
        self.signal.notify_all();
        r
    }
}

/// Ce que le fil de lecture apprend du fil des salles après le bonjour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// Le lien est la garde : il lit des cases jusqu'à `CASE_MAX`.
    Garde,
    /// Un autre rôle.
    Autre,
}

enum Evenement {
    Ouvert {
        lien: Lien,
        t_us: u64,
        provenance: Provenance,
        jointe: IpAddr,
        pair: SocketAddr,
        file: Arc<FileLien>,
        verdict: Sender<Verdict>,
    },
    Recu {
        lien: Lien,
        t_us: u64,
        octets: Vec<u8>,
    },
    Vivant {
        lien: Lien,
        t_us: u64,
    },
    Ferme {
        lien: Lien,
        cause: CauseFermeture,
    },
    GardeFaite {
        lien: Lien,
        fait: Fait,
    },
}

/// Ce que partagent les fils du relais.
struct Partage {
    contexte: Contexte,
    page: Vec<u8>,
    paquet: Option<Paquet>,
    jeux: Jeux,
    dossier: Dossier,
    admission: Mutex<Admission>,
    prochain: AtomicU32,
    salles: Sender<Evenement>,
    arret: AtomicBool,
}

/// Un relais lancé.
pub struct Relais {
    port: u16,
    annoncees: Vec<IpAddr>,
    ecoutes: Vec<SocketAddr>,
    partage: Arc<Partage>,
    fils: Vec<JoinHandle<()>>,
}

impl Relais {
    /// Le port où il écoute.
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// Les adresses du réseau local qu'il annonce ; vide sans réseau.
    pub fn annoncees(&self) -> &[IpAddr] {
        &self.annoncees
    }

    /// Les adresses où il écoute.
    pub fn ecoutes(&self) -> &[SocketAddr] {
        &self.ecoutes
    }

    /// Arrête le relais : il n'accepte plus rien. Les liens ouverts finissent avec le
    /// processus.
    pub fn arreter(self) {
        self.partage.arret.store(true, Ordering::SeqCst);
        // Réveille chaque fil d'écoute d'une connexion qui le fait sortir.
        for e in &self.ecoutes {
            let _: io::Result<TcpStream> = TcpStream::connect_timeout(e, Duration::from_secs(1));
        }
        for f in self.fils {
            let _: thread::Result<()> = f.join();
        }
    }

    /// Attend que le relais s'arrête (jamais, en soirée).
    pub fn attendre(self) {
        for f in self.fils {
            let _: thread::Result<()> = f.join();
        }
    }
}

/// L'époque de ce lancement, tirée neuve (SAL-25) : seize octets du hasard du système.
fn tirer_l_epoque() -> Result<[u8; 16], ErreurLancement> {
    let mut e = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut e))
        .map_err(|err| ErreurLancement(format!("tirer l'époque (/dev/urandom) : {err}")))?;
    Ok(e)
}

/// Écoute sur la boucle locale dans les deux familles, au même port (le port 0 en
/// choisit un libre aux deux).
fn ecouter_la_boucle(port: u16) -> Result<(TcpListener, TcpListener), ErreurLancement> {
    let v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    if port != 0 {
        let a = TcpListener::bind(v4).map_err(|e| {
            ErreurLancement(format!(
                "le port {port} est pris sur 127.0.0.1 ({e}) : le relais ne démarre pas"
            ))
        })?;
        let b = TcpListener::bind(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port))
            .map_err(|e| {
                ErreurLancement(format!(
                    "le port {port} est pris sur [::1] ({e}) : le relais ne démarre pas, un autre programme capterait « localhost »"
                ))
            })?;
        return Ok((a, b));
    }
    for _ in 0..32 {
        let a = TcpListener::bind(v4).map_err(|e| ErreurLancement(format!("127.0.0.1 : {e}")))?;
        let p = a
            .local_addr()
            .map_err(|e| ErreurLancement(format!("127.0.0.1 : {e}")))?
            .port();
        if let Ok(b) = TcpListener::bind(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), p)) {
            return Ok((a, b));
        }
    }
    Err(ErreurLancement(String::from(
        "aucun port libre à la fois sur 127.0.0.1 et [::1]",
    )))
}

/// La page de démarrage : celle des réglages, sinon celle qui charge le paquet, sinon une
/// page minimale.
fn page_de(r: &Reglages) -> Vec<u8> {
    r.page.clone().unwrap_or_else(|| {
        r.paquet.as_ref().map_or_else(
            || PAGE_MINIMALE.to_vec(),
            |p| paquet::page_de_demarrage(p, r.banc),
        )
    })
}

/// Lance le relais.
pub fn lancer(r: &Reglages) -> Result<Relais, ErreurLancement> {
    let interfaces = match r.interfaces.clone() {
        Some(i) => i,
        None => adresses::interfaces().map_err(ErreurLancement)?,
    };
    let annoncees = adresses::choisir(&interfaces, r.adresse).map_err(ErreurLancement)?;
    let dossier = Dossier::ouvrir(&r.garde).map_err(|e| {
        ErreurLancement(format!(
            "le dossier de la garde {} : {e}",
            r.garde.display()
        ))
    })?;
    let enregistreur = match &r.enregistrer {
        Some(c) => Some(
            Enregistreur::creer(c)
                .map_err(|e| ErreurLancement(format!("l'enregistrement {} : {e}", c.display())))?,
        ),
        None => None,
    };
    let epoque = tirer_l_epoque()?;
    let (boucle4, boucle6) = ecouter_la_boucle(r.port)?;
    let port = boucle4
        .local_addr()
        .map_err(|e| ErreurLancement(format!("127.0.0.1 : {e}")))?
        .port();
    let mut ecoutes = vec![boucle4, boucle6];
    for ip in &annoncees {
        let l = TcpListener::bind(SocketAddr::new(*ip, port)).map_err(|e| {
            ErreurLancement(format!(
                "{} : le relais n'y écoute pas ({e}) ; il ne l'annoncera pas (SAL-77)",
                http::hote(*ip, port)
            ))
        })?;
        ecoutes.push(l);
    }
    let adresses_annoncees: Vec<AdresseAnnoncee> = annoncees
        .iter()
        .map(|ip| AdresseAnnoncee {
            ip: adresses::en_octets(*ip),
            port,
        })
        .collect();
    let (vers_salles, des_liens) = mpsc::channel();
    let partage = Arc::new(Partage {
        contexte: Contexte {
            port,
            annoncees: annoncees.clone(),
            banc: r.banc,
        },
        page: page_de(r),
        paquet: r.paquet.clone(),
        jeux: r.jeux.clone(),
        dossier: dossier.clone(),
        admission: Mutex::new(Admission::new()),
        prochain: AtomicU32::new(0),
        salles: vers_salles.clone(),
        arret: AtomicBool::new(false),
    });
    let mut fils = Vec::new();
    // La garde sur disque, un seul fil, dans l'ordre.
    let (vers_disque, du_coeur) = mpsc::channel::<(CodeSalle, Lien, OpGarde)>();
    {
        let d = dossier.clone();
        let retour = vers_salles;
        fils.push(
            thread::Builder::new()
                .name(String::from("relais-garde"))
                .spawn(move || faire_la_garde(&d, &du_coeur, &retour))
                .map_err(|e| ErreurLancement(format!("le fil de la garde : {e}")))?,
        );
    }
    // Le fil qui possède les salles.
    {
        let coeur = Coeur::new(epoque, adresses_annoncees, PreuvesSurDisque(dossier));
        let p = Arc::clone(&partage);
        fils.push(
            thread::Builder::new()
                .name(String::from("relais-salles"))
                .spawn(move || tenir_les_salles(coeur, &des_liens, &vers_disque, enregistreur, &p))
                .map_err(|e| ErreurLancement(format!("le fil des salles : {e}")))?,
        );
    }
    let mut adresses_ecoutees = Vec::new();
    for l in ecoutes {
        let a = l
            .local_addr()
            .map_err(|e| ErreurLancement(format!("écoute : {e}")))?;
        adresses_ecoutees.push(a);
        let p = Arc::clone(&partage);
        fils.push(
            thread::Builder::new()
                .name(format!("relais-ecoute-{a}"))
                .spawn(move || accepter(&l, &p))
                .map_err(|e| ErreurLancement(format!("le fil d'écoute {a} : {e}")))?,
        );
    }
    Ok(Relais {
        port,
        annoncees,
        ecoutes: adresses_ecoutees,
        partage,
        fils,
    })
}

/// Une place admise, rendue quand la connexion finit (SAL-145).
struct Permis {
    partage: Arc<Partage>,
    pair: Pair,
}

impl Drop for Permis {
    fn drop(&mut self) {
        if let Ok(mut a) = self.partage.admission.lock() {
            a.liberer(self.pair);
        }
    }
}

fn accepter(l: &TcpListener, p: &Arc<Partage>) {
    for flux in l.incoming() {
        if p.arret.load(Ordering::SeqCst) {
            return;
        }
        let Ok(flux) = flux else {
            continue;
        };
        let Ok(pair) = flux.peer_addr() else {
            continue;
        };
        let sorte = if http::est_boucle(pair.ip()) {
            Pair::Boucle
        } else {
            Pair::Reseau
        };
        // L'admission, avant tout fil (SAL-143).
        let admise = p
            .admission
            .lock()
            .is_ok_and(|mut a| a.admettre(maintenant_us(), sorte).is_ok());
        if !admise {
            let _: io::Result<()> = flux.shutdown(Shutdown::Both);
            continue;
        }
        let permis = Permis {
            partage: Arc::clone(p),
            pair: sorte,
        };
        let p = Arc::clone(p);
        let lance = thread::Builder::new()
            .name(String::from("relais-lien"))
            .stack_size(PILE)
            .spawn(move || {
                // Une panique ne coûte que ce lien (SAL-79).
                let _: thread::Result<()> =
                    catch_unwind(AssertUnwindSafe(|| connexion(flux, pair, &p)));
                drop(permis);
            });
        let _: io::Result<JoinHandle<()>> = lance;
    }
}

/// Lit l'en-tête de la requête, dans le temps imparti.
fn lire_la_requete(flux: &mut TcpStream) -> Option<(http::Requete, Vec<u8>)> {
    flux.set_read_timeout(Some(REQUETE_MAX)).ok()?;
    let mut tampon = Vec::new();
    let mut morceau = [0u8; 2048];
    loop {
        let n = flux.read(&mut morceau).ok()?;
        if n == 0 {
            return None;
        }
        tampon.extend_from_slice(morceau.get(..n)?);
        match http::lire(&tampon) {
            Ok(Some((r, fin))) => {
                let reste = tampon.get(fin..).map(<[u8]>::to_vec).unwrap_or_default();
                return Some((r, reste));
            }
            Ok(None) => {}
            Err(_) => return None,
        }
    }
}

fn repondre(flux: &mut TcpStream, octets: &[u8]) {
    let _: io::Result<()> = flux.write_all(octets);
    let _: io::Result<()> = flux.flush();
    let _: io::Result<()> = flux.shutdown(Shutdown::Both);
}

fn connexion(mut flux: TcpStream, pair: SocketAddr, p: &Arc<Partage>) {
    let _: io::Result<()> = flux.set_nodelay(true);
    let Some((r, reste)) = lire_la_requete(&mut flux) else {
        return repondre(&mut flux, &http::erreur("400 Bad Request"));
    };
    let c = &p.contexte;
    match http::router(&r) {
        Route::Methode => repondre(&mut flux, &http::erreur("405 Method Not Allowed")),
        Route::Introuvable => repondre(&mut flux, &http::erreur("404 Not Found")),
        Route::Web(reste) => {
            // Rien n'est servi d'un chemin de fichier : la table est en mémoire (SAL-150).
            if !http::publique_permise(pair.ip(), &r, c) {
                return repondre(&mut flux, &http::erreur("403 Forbidden"));
            }
            match p.paquet.as_ref().and_then(|q| q.fichier(&reste)) {
                Some((type_, octets)) => {
                    repondre(&mut flux, &http::fichier_du_paquet(type_, octets));
                }
                None => repondre(&mut flux, &http::erreur("404 Not Found")),
            }
        }
        Route::Jeux(reste) => {
            if !http::boucle_locale_permise(pair.ip(), &r, c) {
                return repondre(&mut flux, &http::erreur("403 Forbidden"));
            }
            match p.jeux.servir(&reste) {
                Some((type_, octets)) => repondre(
                    &mut flux,
                    &http::reponse("200 OK", type_, &octets, &[("Cache-Control", "no-store")]),
                ),
                None => repondre(&mut flux, &http::erreur("404 Not Found")),
            }
        }
        Route::Demarrage => {
            if http::boucle_locale_permise(pair.ip(), &r, c) {
                repondre(&mut flux, &http::page(&p.page, Isolement::Oui));
            } else {
                repondre(&mut flux, &http::erreur("403 Forbidden"));
            }
        }
        Route::Telephone(_) => {
            if http::publique_permise(pair.ip(), &r, c) {
                repondre(&mut flux, &http::page(&p.page, Isolement::Non));
            } else {
                repondre(&mut flux, &http::erreur("403 Forbidden"));
            }
        }
        Route::Salle => {
            if !http::boucle_locale_permise(pair.ip(), &r, c) {
                return repondre(&mut flux, &http::erreur("403 Forbidden"));
            }
            match p.dossier.salle() {
                Some(code) => repondre(
                    &mut flux,
                    &http::reponse(
                        "200 OK",
                        "text/plain; charset=utf-8",
                        code.to_string().as_bytes(),
                        &[("Cache-Control", "no-store")],
                    ),
                ),
                None => repondre(&mut flux, &http::erreur("404 Not Found")),
            }
        }
        Route::Ws => {
            let provenance = match http::juger_websocket(pair.ip(), &r, c) {
                Jugement::Admise(pr) => pr,
                Jugement::Etrangere(_) => {
                    return repondre(&mut flux, &http::erreur("403 Forbidden"));
                }
            };
            let Some(accepte) = http::accepter_websocket(&r) else {
                return repondre(&mut flux, &http::erreur("400 Bad Request"));
            };
            if flux.write_all(&accepte).is_err() {
                return;
            }
            lien_websocket(flux, pair, provenance, reste, p);
        }
    }
}

/// Le côté lecture d'un lien : il lit sa moitié de la connexion, et n'écrit (les réponses
/// automatiques de WebSocket) que sous le verrou de l'écrivain, des tampons entiers.
struct FluxLecture {
    lecture: TcpStream,
    ecriture: Arc<Mutex<TcpStream>>,
}

impl Read for FluxLecture {
    fn read(&mut self, tampon: &mut [u8]) -> io::Result<usize> {
        self.lecture.read(tampon)
    }
}

impl Write for FluxLecture {
    fn write(&mut self, tampon: &[u8]) -> io::Result<usize> {
        let mut e = self
            .ecriture
            .lock()
            .map_err(|_| io::Error::other("verrou de l'écrivain"))?;
        e.write_all(tampon)?;
        Ok(tampon.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// La taille lue au plus avant le bonjour : une trame, et son enveloppe.
const LECTURE_MAX: usize = TRAME_MAX as usize + 4096;

fn configuration(max: usize) -> WebSocketConfig {
    WebSocketConfig::default()
        .read_buffer_size(16 * 1024)
        .write_buffer_size(0)
        .max_message_size(Some(max))
        .max_frame_size(Some(max))
}

#[expect(
    clippy::too_many_lines,
    reason = "la vie d'un lien se lit d'un trait : ouvrir, lire, finir"
)]
fn lien_websocket(
    flux: TcpStream,
    pair: SocketAddr,
    provenance: Provenance,
    reste: Vec<u8>,
    p: &Arc<Partage>,
) {
    if flux.set_read_timeout(None).is_err() {
        return;
    }
    let jointe = flux
        .local_addr()
        .map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |a| a.ip());
    let (Ok(ecriture), Ok(coupe)) = (flux.try_clone(), flux.try_clone()) else {
        return;
    };
    let ecriture = Arc::new(Mutex::new(ecriture));
    let lien = Lien(p.prochain.fetch_add(1, Ordering::SeqCst).saturating_add(1));
    let file = Arc::new(FileLien::new(FILE_APPAREIL, coupe));
    let ecrivain = {
        let f = Arc::clone(&file);
        let e = Arc::clone(&ecriture);
        thread::Builder::new()
            .name(String::from("relais-ecrivain"))
            .stack_size(PILE)
            .spawn(move || ecrire(&f, &e))
    };
    let Ok(ecrivain) = ecrivain else {
        return;
    };
    let (verdict_tx, verdict_rx) = mpsc::channel();
    let ouvert = p.salles.send(Evenement::Ouvert {
        lien,
        t_us: maintenant_us(),
        provenance,
        jointe,
        pair,
        file: Arc::clone(&file),
        verdict: verdict_tx,
    });
    let mut ws = WebSocket::from_partially_read(
        FluxLecture {
            lecture: flux,
            ecriture: Arc::clone(&ecriture),
        },
        reste,
        Role::Server,
        Some(configuration(LECTURE_MAX)),
    );
    let mut premier = true;
    let mut cause = CauseFermeture::Fermee;
    if ouvert.is_ok() {
        loop {
            match ws.read() {
                Ok(Message::Binary(b)) => {
                    let envoye = p.salles.send(Evenement::Recu {
                        lien,
                        t_us: maintenant_us(),
                        octets: b.to_vec(),
                    });
                    if envoye.is_err() {
                        break;
                    }
                    if premier {
                        premier = false;
                        // Le bonjour : la garde lit des cases, les autres des trames.
                        match verdict_rx.recv_timeout(Duration::from_secs(10)) {
                            Ok(Verdict::Garde) => {
                                let m = CASE_MAX.saturating_add(4096);
                                ws.set_config(|c| {
                                    c.max_message_size = Some(m);
                                    c.max_frame_size = Some(m);
                                });
                            }
                            // Refusé ou fermé : l'écrivain envoie le refus, puis ferme ;
                            // la lecture finit alors d'elle-même. Couper ici jetterait le
                            // refus encore en file (SAL-42, SAL-148).
                            Ok(Verdict::Autre) | Err(_) => {}
                        }
                    }
                }
                Ok(Message::Text(t)) => {
                    // Le relais ne parle qu'en binaire : un texte est illisible.
                    let envoye = p.salles.send(Evenement::Recu {
                        lien,
                        t_us: maintenant_us(),
                        octets: t.as_bytes().to_vec(),
                    });
                    if envoye.is_err() {
                        break;
                    }
                }
                Ok(Message::Pong(_) | Message::Ping(_)) => {
                    let envoye = p.salles.send(Evenement::Vivant {
                        lien,
                        t_us: maintenant_us(),
                    });
                    if envoye.is_err() {
                        break;
                    }
                }
                Ok(Message::Frame(_)) => {}
                Err(tungstenite::Error::Capacity(_)) => {
                    cause = CauseFermeture::TropGros;
                    break;
                }
                Ok(Message::Close(_)) | Err(_) => break,
            }
        }
    }
    let _: Result<(), mpsc::SendError<Evenement>> = p.salles.send(Evenement::Ferme { lien, cause });
    file.couper();
    let _: thread::Result<()> = ecrivain.join();
}

/// Une trame de serveur (sans masque), entière.
fn trame(f: Frame) -> Vec<u8> {
    let mut o = Vec::new();
    let _: tungstenite::Result<()> = f.format(&mut o);
    o
}

/// L'écrivain d'un lien : il prend dans la file et écrit des trames entières, sous le
/// verrou ; à la fin, la trame de fermeture, puis la connexion fermée.
fn ecrire(file: &FileLien, ecriture: &Mutex<TcpStream>) {
    loop {
        let envoi = {
            let Ok(mut f) = file.file.lock() else {
                return;
            };
            loop {
                if let Some(e) = f.prendre() {
                    break e;
                }
                match file.signal.wait(f) {
                    Ok(g) => f = g,
                    Err(_) => return,
                }
            }
        };
        let (octets, fin) = match envoi {
            Envoi::Fiable(o) | Envoi::Remplacable(_, o) => (
                trame(Frame::message(
                    Bytes::from(o),
                    OpCode::Data(Data::Binary),
                    true,
                )),
                false,
            ),
            Envoi::Battement => (trame(Frame::ping(Bytes::new())), false),
            Envoi::Fin => (trame(Frame::close(None)), true),
        };
        let Ok(mut e) = ecriture.lock() else {
            return;
        };
        let ecrit = e.write_all(&octets).and_then(|()| e.flush());
        if fin || ecrit.is_err() {
            let _: io::Result<()> = e.shutdown(Shutdown::Both);
            return;
        }
    }
}

/// Le fil de la garde : chaque opération dans l'ordre, sur disque.
fn faire_la_garde(
    d: &Dossier,
    demandes: &Receiver<(CodeSalle, Lien, OpGarde)>,
    retour: &Sender<Evenement>,
) {
    while let Ok((salle, lien, op)) = demandes.recv() {
        let fait = d.faire(salle, &op);
        if retour.send(Evenement::GardeFaite { lien, fait }).is_err() {
            return;
        }
    }
}

struct Salles<'a> {
    coeur: Coeur<PreuvesSurDisque>,
    files: BTreeMap<Lien, Arc<FileLien>>,
    verdicts: BTreeMap<Lien, Sender<Verdict>>,
    encombres: BTreeMap<Lien, bool>,
    fermetures: BTreeMap<Lien, u64>,
    disque: &'a Sender<(CodeSalle, Lien, OpGarde)>,
    enregistreur: Option<Enregistreur>,
}

impl Salles<'_> {
    fn noter(&mut self, lien: Lien, sens: Sens, octets: &[u8]) {
        if let Some(e) = self.enregistreur.as_mut() {
            e.noter(maintenant_us(), lien, sens, octets);
        }
    }

    fn evenement(&mut self, ev: Evenement) {
        match ev {
            Evenement::Ouvert {
                lien,
                t_us,
                provenance,
                jointe,
                pair,
                file,
                verdict,
            } => {
                self.noter(lien, Sens::Ouvert, pair.to_string().as_bytes());
                self.files.insert(lien, file);
                self.verdicts.insert(lien, verdict);
                self.coeur
                    .ouvrir(t_us, lien, provenance, adresses::en_octets(jointe));
            }
            Evenement::Recu { lien, t_us, octets } => {
                self.noter(lien, Sens::Recu, &octets);
                let coeur = &mut self.coeur;
                let traite = catch_unwind(AssertUnwindSafe(|| {
                    coeur.recevoir(t_us, maintenant_us(), lien, &octets);
                }));
                if traite.is_err() {
                    // Une panique sur l'entrée d'un lien ne coûte que ce lien (SAL-79).
                    self.coeur.ferme(lien, CauseFermeture::Illisible);
                    self.fermer_la_file(lien);
                }
            }
            Evenement::Vivant { lien, t_us } => self.coeur.vivant(t_us, lien),
            Evenement::Ferme { lien, cause } => {
                self.noter(lien, Sens::Ferme, &[]);
                self.coeur.ferme(lien, cause);
                self.fermer_la_file(lien);
            }
            Evenement::GardeFaite { lien, fait } => self.coeur.garde_faite(lien, fait),
        }
    }

    fn fermer_la_file(&mut self, lien: Lien) {
        if let Some(f) = self.files.remove(&lien) {
            f.couper();
        }
        self.verdicts.remove(&lien);
        self.encombres.remove(&lien);
        self.fermetures.remove(&lien);
    }

    /// Un lien que le cœur a fermé et qui n'est pas parti en 2 s (un pair qui ne lit plus
    /// sa fin) est coupé : aucune attente sans échéance (SAL-49).
    fn couper_les_fermetures_lentes(&mut self, t_us: u64) {
        let lents: Vec<Lien> = self
            .fermetures
            .iter()
            .filter(|(_, depuis)| t_us.saturating_sub(**depuis) > FERMETURE_MAX_US)
            .map(|(l, _)| *l)
            .collect();
        for l in lents {
            self.fermetures.remove(&l);
            if let Some(f) = self.files.get(&l) {
                f.couper();
            }
        }
    }

    /// L'arrêt du relais : chaque lien est coupé.
    fn tout_couper(&mut self) {
        for f in self.files.values() {
            f.couper();
        }
    }

    fn sorties(&mut self) {
        loop {
            let s = self.coeur.sorties();
            if s.is_empty() {
                return;
            }
            for sortie in s {
                self.sortie(sortie);
            }
        }
    }

    fn sortie(&mut self, s: Sortie) {
        match s {
            Sortie::Envoyer {
                lien,
                place,
                octets,
            } => {
                self.noter(lien, Sens::Envoye, &octets);
                let Some(f) = self.files.get(&lien).cloned() else {
                    return;
                };
                let r = f.avec(|file| {
                    let r = match place {
                        Some(pl) => {
                            file.remplacable(pl, octets);
                            Ok(())
                        }
                        None => file.fiable(octets),
                    };
                    (r, file.fiables(), file.borne())
                });
                match r {
                    Some((Ok(()), fiables, borne)) => {
                        // Au-delà de la moitié de sa borne, l'autorité l'apprend, une fois
                        // par montée (PHASE-3, § 2.5).
                        let haut = fiables > borne / 2;
                        let avant = self.encombres.insert(lien, haut).unwrap_or(false);
                        if haut && !avant {
                            self.coeur
                                .encombre(lien, u32::try_from(fiables).unwrap_or(u32::MAX));
                        }
                    }
                    Some((Err(_), _, _)) | None => {
                        // Une file pleine ferme le lien ; il reviendra (SAL-07).
                        self.coeur.ferme(lien, CauseFermeture::FilePleine);
                        self.fermer_la_file(lien);
                    }
                }
            }
            Sortie::Battre { lien } => {
                if let Some(f) = self.files.get(&lien) {
                    f.avec(FileSortante::battement);
                }
            }
            Sortie::Fermer { lien } => {
                if let Some(f) = self.files.get(&lien) {
                    f.avec(FileSortante::fermer);
                    self.fermetures.insert(lien, maintenant_us());
                }
                self.verdicts.remove(&lien);
            }
            Sortie::Admis { lien, role } => {
                let v = match role {
                    RoleAdmis::Garde => {
                        if let Some(f) = self.files.get(&lien) {
                            f.avec(|file| file.changer_de_borne(FILE_GARDE));
                        }
                        Verdict::Garde
                    }
                    RoleAdmis::Autorite => {
                        if let Some(f) = self.files.get(&lien) {
                            f.avec(|file| file.changer_de_borne(FILE_AUTORITE));
                        }
                        Verdict::Autre
                    }
                    RoleAdmis::Appareil => Verdict::Autre,
                };
                if let Some(tx) = self.verdicts.remove(&lien) {
                    let _: Result<(), mpsc::SendError<Verdict>> = tx.send(v);
                }
            }
            Sortie::Garde { salle, lien, op } => {
                let _: Result<(), mpsc::SendError<(CodeSalle, Lien, OpGarde)>> =
                    self.disque.send((salle, lien, op));
            }
        }
    }
}

/// Le fil qui possède les salles.
fn tenir_les_salles(
    coeur: Coeur<PreuvesSurDisque>,
    evenements: &Receiver<Evenement>,
    disque: &Sender<(CodeSalle, Lien, OpGarde)>,
    enregistreur: Option<Enregistreur>,
    p: &Partage,
) {
    let mut s = Salles {
        coeur,
        files: BTreeMap::new(),
        verdicts: BTreeMap::new(),
        encombres: BTreeMap::new(),
        fermetures: BTreeMap::new(),
        disque,
        enregistreur,
    };
    let mut dernier_tic = 0u64;
    loop {
        if p.arret.load(Ordering::SeqCst) {
            s.tout_couper();
            return;
        }
        match evenements.recv_timeout(Duration::from_millis(10)) {
            Ok(ev) => s.evenement(ev),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let t = maintenant_us();
        if t.saturating_sub(dernier_tic) >= 10_000 {
            dernier_tic = t;
            s.coeur.tic(t);
            s.couper_les_fermetures_lentes(t);
        }
        s.sorties();
    }
}
