//! `soiree-banc soiree` et `soiree-banc arriere-plan` (PHASE-3, § 8.2, § 8.3 ; lot 10a) :
//! **la vraie autorité dans le navigateur**. Le relais (le même code, dans ce processus,
//! avec `--banc`) sert la page de démarrage, le paquet `web/` et le module ; un navigateur
//! sans interface ouvre l'écran ; son Worker tient la partie ; des téléphones natifs
//! (`salle::Appareil`) jouent les coups des bots par le vrai chemin, de vraies WebSocket
//! sur la boucle (« gestes directs » : la page des téléphones vient au lot 12).
//!
//! Ce que la soirée joue, dans chaque navigateur : la partie ouverte neuve ; l'écran
//! **rechargé** en pleine manche ; le Worker **tué** par le crochet de test de la page,
//! relancé comme le chien de garde le relance ; un **second onglet** de l'écran, qui prend
//! la salle (le premier reçoit `Remplacee`), puis se ferme, et le premier reprend ici ;
//! des marques **F8** par la vraie chaîne du clavier ; la partie jusqu'à une phase
//! terminale (le podium) ; puis la partie **figée**, sa garde écrite en entier, et G
//! exportée par la commande du diagnostic.
//!
//! Ce que le banc juge, sans croire la page sur parole :
//!
//! - chaque reprise (rechargement, Worker tué, second onglet, « Reprendre ici ») a la
//!   **même forme canonique** et la même empreinte que la partie que wasmtime reprend des
//!   mêmes cases de la garde (relevées dans l'enregistrement du relais) ;
//! - la **chaîne entière** des cases, toutes autorités comprises, s'ouvre case après case
//!   et porte un journal d'un seul tenant, sans geste deux fois ; rejouée dans wasmtime
//!   depuis la première case, **chaque empreinte notée est retrouvée**, et la partie finit
//!   de la forme que le Worker figé dit ; la garde finale sur disque, de même ;
//! - le premier onglet a reçu `Remplacee` ;
//! - `rejouer-garde`, sur la garde finale et la clé exportée, **retrouve chaque marque F8**
//!   posée, et la rejoue ;
//! - **`connect-src 'self'`** : la page porte la politique, le Worker s'est relié à
//!   `ws://localhost:<port>/ws`, et aucune violation n'est dite ;
//! - tout geste accusé à un téléphone est écrit ; le chien de garde n'a rien tué ; aucune
//!   panne de l'outil ;
//! - une phase terminale.
//!
//! `--m12 <tours>` mesure la durée d'une reprise au pire moment (M12) : le Worker tué juste
//! avant un point (au moins 575 pas depuis le dernier, `POINT_TOUS_LES` valant 600), et le
//! temps que met le Worker neuf, de sa création à la partie reprise.
//!
//! `arriere-plan` (§ 8.3, d'après `cadence.mjs`) : l'onglet caché `--cache` secondes,
//! `--fois` passages par navigateur ; échoue si l'onglet n'était pas vraiment caché (la
//! page ne dit pas `hidden`, ou un pas s'y est joué au signal d'une image), si le journal
//! note un `Saut` que rien n'explique, si la partie n'a pas suivi l'horloge pendant ce
//! temps, ou si la page ne signale pas un retard injecté dans le Worker pendant que
//! l'onglet est caché (`CommandeEcran::Retarder`). Le Worker dit le pas où il gèle : les
//! sauts de ce retard, et eux seuls, lui sont rattachés.
//!
//! Dans `soiree` comme dans `arriere-plan`, **chaque saut du journal est rattaché** à ce
//! qui l'explique (une reprise, un retard injecté), ou il est une faute : un tour trop
//! long ou une image trop tardive, onglet visible, ne passe pas inaperçu (vérification du
//! lot 10a). Les tours longs et les trous entre deux pas que le Worker rapporte sont
//! notés, sans jugement.

#![expect(
    clippy::many_single_char_names,
    reason = "le banc nomme comme ses autres scénarios : o les options, n le navigateur, s la session, p le passage, l une ligne, g la clé de garde"
)]

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use jeu_abi::{ApplicableFil, Capacites};
use liaison::CodeSalle;
use moteur_wasmtime::{InstantaneWasm, ModuleCharge, Moteur, ReglagesMoteur};
use salle::appareil::Appareil;
use salle::chiffre::{CleDeGarde, Fragment, Hasard};
use salle::garde::relire;
use salle::horloge::us_de_duree;
use salle::simulation::{
    Graine, Historique, PAS_US, TelephoneAccuse, accuses_hors_de_l_historique,
    reprendre_de_l_historique, reprendre_de_la_garde, verifier_l_historique,
};
use serde_json::Value as Json;

use crate::horloge::{maintenant, reste};
use crate::navigateur::{Lance, Navigateur};
use crate::soiree_native::{Chaine, LECTURE_MAX, chaine_ecrite, port_libre};
use crate::ws::Ws;

/// Le port de l'écran en soirée (P14) : le banc le prend s'il est libre, pour que le
/// constat de `connect-src 'self'` porte sur `ws://localhost:7878/ws`.
pub const PORT_DE_LA_SOIREE: u16 = 7878;

/// Ce que la soirée joue.
#[derive(Debug, Clone)]
pub struct Options {
    /// Le module livré du jeu.
    pub module: PathBuf,
    /// Les navigateurs.
    pub navigateurs: Vec<Navigateur>,
    /// Les téléphones ; par défaut, ceux que le jeu déclare, au plus 6.
    pub telephones: Option<usize>,
    /// Construire le paquet `web/` (sinon, le dernier construit).
    pub construire: bool,
    /// La plus longue attente d'une phase terminale, en secondes.
    pub duree_max_s: u64,
    /// Le dossier de travail.
    pub travail: PathBuf,
    /// M12 : ce nombre de reprises au pire moment, au lieu de la soirée.
    pub m12: u32,
    /// `arriere-plan` : l'onglet caché ce nombre de secondes, au lieu de la soirée.
    pub cache_s: u64,
    /// `arriere-plan` : les passages dans chaque navigateur (un « réussi » d'un seul
    /// passage ne dit pas qu'un navigateur tient : vérification du lot 10a).
    pub fois: u32,
}

/// Une ligne que la page écrit pour le banc : `banc <onglet> <json>`.
#[derive(Debug, Clone)]
pub struct Ligne {
    /// L'onglet de l'écran qui l'a écrite.
    pub onglet: String,
    /// Ce qu'elle dit.
    pub json: Json,
    /// Quand le banc l'a lue, depuis le début du passage.
    pub ms: u128,
}

impl Ligne {
    fn quoi(&self) -> &str {
        self.json.get("quoi").and_then(Json::as_str).unwrap_or("")
    }
}

/// Ce qu'un passage (un navigateur) a fait et constaté.
#[derive(Debug, Clone, Default)]
pub struct Passage {
    /// Le navigateur, et sa version.
    pub navigateur: String,
    /// Le moteur de dessin de la page.
    pub moteur: String,
    /// Le port de l'écran.
    pub port: u16,
    /// Les reprises : (pourquoi, case relue, forme et empreinte du Worker, celles de
    /// wasmtime sur les mêmes cases).
    pub reprises: Vec<(String, u64, String, String)>,
    /// Les durées de reprise (M12), en ms : (de la création du Worker à la partie
    /// reprise ; pas depuis le point au moment du meurtre ; les étapes).
    pub m12: Vec<(f64, u64, String)>,
    /// La chaîne entière.
    pub historique: Historique,
    /// Les phases terminales vues.
    pub terminales: u32,
    /// Les marques posées, et celles que `rejouer-garde` a rejouées.
    pub marques: (u32, usize),
    /// Les pas joués pendant que l'onglet était caché (`arriere-plan`), et le temps caché.
    pub cache: Option<(u64, u64)>,
    /// Les lignes du banc lues.
    pub lignes: usize,
    /// Ce que le banc a constaté sans en juger : les sauts rattachés, les tours longs.
    pub notes: Vec<String>,
    /// Ce qui n'a pas tenu.
    pub fautes: Vec<String>,
    /// Le passage n'a pas pu jouer (le navigateur absent, refusé).
    pub pas_execute: Option<String>,
}

// ---------------------------------------------------------------------------
// Le paquet
// ---------------------------------------------------------------------------

/// Construit le paquet de la page (`web/` pour `wasm32`, `ecran` et `autorite`, leur colle
/// `wasm-bindgen`, et `demarrage.js`) dans `<soiree>/target/soiree-web/<empreinte>/` ;
/// rend ce dossier. L'empreinte est le BLAKE3 des noms et des octets, tronqué à 16 octets.
/// Ce que fera `soiree-relais preparer` (lot 10b).
pub fn construire_le_paquet(soiree: &Path) -> Result<PathBuf, String> {
    let web = soiree.join("web");
    let cible = web.join("target");
    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(&web)
        .args([
            "build",
            "-p",
            "ecran",
            "-p",
            "autorite",
            "--lib",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--target-dir",
        ])
        .arg(&cible);
    crate::machine::lancer(&mut cargo, "cargo build de ecran et autorite")?;
    let brouillon = soiree.join("target/soiree-web/brouillon");
    let _: std::io::Result<()> = std::fs::remove_dir_all(&brouillon);
    std::fs::create_dir_all(&brouillon).map_err(|e| format!("{e}"))?;
    for nom in ["ecran", "autorite"] {
        let wasm = cible.join(format!("wasm32-unknown-unknown/release/{nom}.wasm"));
        crate::machine::lancer(
            Command::new("wasm-bindgen")
                .args(["--target", "web", "--no-typescript", "--out-dir"])
                .arg(&brouillon)
                .arg(&wasm),
            &format!("wasm-bindgen de {nom}"),
        )?;
    }
    std::fs::copy(web.join("demarrage.js"), brouillon.join("demarrage.js"))
        .map_err(|e| format!("demarrage.js : {e}"))?;
    let mut noms: Vec<PathBuf> = std::fs::read_dir(&brouillon)
        .map_err(|e| format!("{e}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    noms.sort();
    let mut h = blake3::Hasher::new();
    for n in &noms {
        let o = std::fs::read(n).map_err(|e| format!("{} : {e}", n.display()))?;
        h.update(
            n.file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("")
                .as_bytes(),
        );
        h.update(&u64::try_from(o.len()).unwrap_or(0).to_le_bytes());
        h.update(&o);
    }
    let empreinte = h.finalize().to_hex();
    let dossier = soiree
        .join("target/soiree-web")
        .join(empreinte.get(..32).unwrap_or(""));
    let _: std::io::Result<()> = std::fs::remove_dir_all(&dossier);
    std::fs::rename(&brouillon, &dossier).map_err(|e| format!("{e}"))?;
    Ok(dossier)
}

/// Le dernier paquet construit.
pub fn dernier_paquet(soiree: &Path) -> Result<PathBuf, String> {
    let racine = soiree.join("target/soiree-web");
    let mut paquets: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&racine)
        .map_err(|e| format!("{} : {e}", racine.display()))?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.len() == 32))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    paquets.sort();
    paquets
        .pop()
        .map(|(_, p)| p)
        .ok_or_else(|| format!("aucun paquet sous {}", racine.display()))
}

// ---------------------------------------------------------------------------
// Les téléphones natifs
// ---------------------------------------------------------------------------

struct Telephone {
    appareil: Appareil<Graine>,
    ws: Option<Ws>,
    jeton: [u8; 32],
    revient: Option<Instant>,
    prochain_reveil: Instant,
}

// ---------------------------------------------------------------------------
// Une session : le relais, le navigateur, les téléphones, les lignes de la page
// ---------------------------------------------------------------------------

struct Session {
    relais: Option<relais::coque::Relais>,
    port: u16,
    garde: PathBuf,
    enregistrement: PathBuf,
    navigateur: Lance,
    lignes: Vec<Ligne>,
    autres: Vec<String>,
    telephones: Vec<Telephone>,
    redite_us: Option<u64>,
    salle: Option<CodeSalle>,
    fragment: Option<Fragment>,
    depart: Instant,
    graine: u64,
}

fn heure_locale_us(depart: Instant) -> u64 {
    // Les téléphones ont leur propre horloge ; seul compte qu'elle avance.
    u64::try_from(maintenant().saturating_duration_since(depart).as_micros()).unwrap_or(u64::MAX)
}

impl Session {
    fn adresse(&self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), self.port)
    }

    /// Lit la console de la page pendant `delai`, et fait tourner les téléphones.
    fn tourner(&mut self, delai: Duration) -> Result<(), String> {
        let fin = maintenant().checked_add(delai).ok_or("délai")?;
        loop {
            self.telephones_tournent();
            let lignes = self.navigateur.ecouter(Duration::from_millis(8))?;
            for l in lignes {
                self.noter(&l);
            }
            if reste(fin).is_zero() {
                return Ok(());
            }
        }
    }

    fn noter(&mut self, l: &str) {
        let ms = maintenant()
            .saturating_duration_since(self.depart)
            .as_millis();
        if let Some(reste) = l.split_once("banc ").map(|(_, r)| r)
            && l.starts_with("console.")
            && let Some((onglet, json)) = reste.split_once(' ')
            && let Ok(json) = serde_json::from_str::<Json>(json)
        {
            self.lignes.push(Ligne {
                onglet: onglet.to_owned(),
                json,
                ms,
            });
            return;
        }
        self.autres.push(l.to_owned());
    }

    /// Attend une ligne d'indice au moins `depuis` qui vérifie `p`, au plus `delai`.
    fn attendre(
        &mut self,
        depuis: usize,
        delai: Duration,
        quoi: &str,
        p: impl Fn(&Ligne) -> bool,
    ) -> Result<Ligne, String> {
        let fin = maintenant().checked_add(delai).ok_or("délai")?;
        loop {
            if let Some(l) = self.lignes.iter().skip(depuis).find(|l| p(l)) {
                return Ok(l.clone());
            }
            if reste(fin).is_zero() {
                let derniers: Vec<String> = self
                    .lignes
                    .iter()
                    .rev()
                    .take(6)
                    .map(|l| l.json.to_string())
                    .chain(self.autres.iter().rev().take(6).cloned())
                    .collect();
                return Err(format!(
                    "{quoi} : rien en {delai:?} ; derniers : {}",
                    derniers.join(" | ")
                ));
            }
            self.tourner(Duration::from_millis(20))?;
        }
    }

    /// La dernière ligne `etat` d'un onglet.
    fn etat(&self, onglet: &str) -> Option<&Json> {
        self.lignes
            .iter()
            .rev()
            .find(|l| l.onglet == onglet && l.quoi() == "etat")
            .map(|l| &l.json)
    }

    fn evaluer(&mut self, onglet: usize, js: &str) -> Result<Json, String> {
        self.navigateur.evaluer(onglet, js)
    }

    fn ajouter_les_telephones(&mut self, n: usize) -> Result<(), String> {
        let (Some(fragment), Some(salle)) = (self.fragment.clone(), self.salle) else {
            return Err(String::from("pas d'accueil : ni fragment ni salle"));
        };
        for k in 0..n {
            let mut h = Graine::new(
                "telephone",
                self.graine.wrapping_mul(1000).wrapping_add(k as u64),
            );
            let mut jeton = [0u8; 32];
            h.remplir(&mut jeton);
            let onglet = h.entier();
            let appareil = Appareil::new(
                fragment.clone(),
                salle,
                (jeton, onglet),
                Capacites::AUCUNE,
                self.redite_us,
                h,
            );
            self.telephones.push(Telephone {
                appareil,
                ws: None,
                jeton,
                revient: Some(maintenant()),
                prochain_reveil: maintenant(),
            });
        }
        Ok(())
    }

    fn telephones_tournent(&mut self) {
        let adresse = self.adresse();
        let local = heure_locale_us(self.depart);
        for t in &mut self.telephones {
            if t.ws.is_none() {
                if t.revient.is_some_and(|r| maintenant() >= r) && !t.appareil.fini() {
                    match Ws::relier(adresse, LECTURE_MAX) {
                        Ok(ws) => {
                            t.ws = Some(ws);
                            t.revient = None;
                            t.appareil.relier();
                        }
                        Err(_) => t.revient = maintenant().checked_add(Duration::from_millis(500)),
                    }
                }
                if t.ws.is_none() {
                    continue;
                }
            }
            let mut perdu = false;
            if let Some(ws) = t.ws.as_mut() {
                loop {
                    match ws.recevoir() {
                        Ok(Some(o)) => t.appareil.recevoir(local, &o),
                        Ok(None) => break,
                        Err(_) => {
                            perdu = true;
                            break;
                        }
                    }
                }
            }
            if maintenant() >= t.prochain_reveil {
                t.prochain_reveil = maintenant()
                    .checked_add(Duration::from_micros(PAS_US))
                    .unwrap_or_else(maintenant);
                t.appareil.reveiller(local);
            }
            for o in t.appareil.sorties() {
                if let Some(ws) = t.ws.as_mut()
                    && ws.envoyer(o).is_err()
                {
                    perdu = true;
                }
            }
            if let Some(ws) = t.ws.as_mut()
                && ws.vider().is_err()
            {
                perdu = true;
            }
            if perdu {
                if let Some(ws) = t.ws.take() {
                    ws.couper();
                }
                t.appareil.couper();
                t.revient = maintenant().checked_add(Duration::from_secs(1));
            }
        }
    }

    /// Le relais arrêté, les téléphones coupés, le navigateur fermé : la garde sur disque
    /// et l'enregistrement restent.
    fn fermer(mut self) {
        for t in &mut self.telephones {
            if let Some(ws) = t.ws.take() {
                ws.couper();
            }
        }
        self.navigateur.fermer();
        if let Some(r) = self.relais.take() {
            r.arreter();
        }
    }

    /// Les cases de la garde sur disque.
    fn garde_sur_disque(&self, salle: CodeSalle) -> Result<Chaine, String> {
        let d = relais::garde::Dossier::ouvrir(&self.garde).map_err(|e| format!("{e}"))?;
        let mut cases = Vec::new();
        for c in d.lister(salle).map_err(|e| format!("{e}"))? {
            if let Some(o) = d.lire(salle, c).map_err(|e| format!("{e}"))? {
                cases.push((c, o));
            }
        }
        Ok(cases)
    }
}

/// Une requête HTTP au relais, depuis la boucle : la réponse entière.
fn http(port: u16, chemin: &str) -> Result<String, String> {
    let mut f = TcpStream::connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
        .map_err(|e| format!("{e}"))?;
    f.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("{e}"))?;
    f.write_all(format!("GET {chemin} HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n").as_bytes())
        .map_err(|e| format!("{e}"))?;
    let mut o = Vec::new();
    let _: std::io::Result<usize> = f.read_to_end(&mut o);
    Ok(String::from_utf8_lossy(&o).into_owned())
}

/// Ouvre une session : le relais avec le paquet et le module, le navigateur sur l'écran.
fn ouvrir(
    o: &Options,
    n: Navigateur,
    paquet: &Path,
    module: &[u8],
    dossier: &Path,
    redite_us: Option<u64>,
) -> Result<Session, String> {
    let _: std::io::Result<()> = std::fs::remove_dir_all(dossier);
    let jeux = dossier.join("jeux");
    std::fs::create_dir_all(&jeux).map_err(|e| format!("{e}"))?;
    let empreinte = blake3::hash(module).to_hex();
    std::fs::write(jeux.join(format!("{empreinte}.wasm")), module).map_err(|e| format!("{e}"))?;
    let garde = dossier.join("garde");
    let enregistrement = dossier.join("enregistrement.bin");
    let port = if std::net::TcpListener::bind(("127.0.0.1", PORT_DE_LA_SOIREE)).is_ok()
        && std::net::TcpListener::bind(("::1", PORT_DE_LA_SOIREE)).is_ok()
    {
        PORT_DE_LA_SOIREE
    } else {
        port_libre()?
    };
    let relais = relais::coque::lancer(&relais::coque::Reglages {
        port,
        adresse: None,
        garde: garde.clone(),
        banc: relais::http::Banc::Oui,
        enregistrer: Some(enregistrement.clone()),
        interfaces: Some(Vec::new()),
        page: None,
        paquet: Some(relais::paquet::Paquet::lire(paquet)?),
        jeux: relais::paquet::Jeux::lire(&jeux)?,
    })
    .map_err(|e| format!("le relais : {e}"))?;
    let profils = o.travail.join("profils");
    let mut navigateur = match Lance::lancer(n, None, &profils) {
        Ok(l) => l,
        Err(e) => {
            relais.arreter();
            return Err(format!("pas exécuté : {e}"));
        }
    };
    let url = format!("http://localhost:{port}/?jeu={empreinte}");
    if let Err(e) = navigateur.ouvrir(&url) {
        navigateur.fermer();
        relais.arreter();
        return Err(e);
    }
    Ok(Session {
        relais: Some(relais),
        port,
        garde,
        enregistrement,
        navigateur,
        lignes: Vec::new(),
        autres: Vec::new(),
        telephones: Vec::new(),
        redite_us,
        salle: None,
        fragment: None,
        depart: maintenant(),
        graine: 1,
    })
}

/// La partie neuve s'ouvre : la page, l'accueil, la partie en jeu ; puis les téléphones.
fn debuter(s: &mut Session, p: &mut Passage, telephones: usize) -> Result<String, String> {
    let page = s.attendre(0, Duration::from_secs(60), "la page", |l| {
        l.quoi() == "page"
    })?;
    page.json
        .get("moteur")
        .and_then(Json::as_str)
        .unwrap_or("?")
        .clone_into(&mut p.moteur);
    let onglet = page.onglet.clone();
    let accueil = s.attendre(0, Duration::from_secs(60), "l'accueil", |l| {
        l.quoi() == "accueil"
    })?;
    let fragment = accueil
        .json
        .get("fragment")
        .and_then(Json::as_str)
        .ok_or("accueil sans fragment")?;
    s.fragment = Some(Fragment::lire(fragment).map_err(|e| format!("fragment : {e:?}"))?);
    s.salle = accueil
        .json
        .get("salle")
        .and_then(Json::as_str)
        .and_then(CodeSalle::lire);
    let en_jeu = s.attendre(0, Duration::from_secs(60), "la partie neuve", |l| {
        l.quoi() == "en-jeu"
    })?;
    if !en_jeu.json.get("relue").is_some_and(Json::is_null) {
        p.fautes.push(format!(
            "la première partie n'est pas neuve : {}",
            en_jeu.json
        ));
    }
    s.ajouter_les_telephones(telephones)?;
    Ok(onglet)
}

/// Attend une reprise (« en-jeu », relue d'une case) d'un onglet, après la ligne `depuis`,
/// et la note.
fn reprise(
    s: &mut Session,
    p: &mut Passage,
    depuis: usize,
    onglet: Option<&str>,
    pourquoi: &str,
) -> Result<Ligne, String> {
    let l = s.attendre(depuis, Duration::from_secs(60), pourquoi, |l| {
        l.quoi() == "en-jeu" && onglet.is_none_or(|o| l.onglet == o)
    })?;
    let relue = l.json.get("relue").and_then(Json::as_u64);
    let forme = l.json.get("forme").and_then(Json::as_str).unwrap_or("");
    let empreinte = l.json.get("empreinte").and_then(Json::as_str).unwrap_or("");
    match relue {
        Some(n) => p.reprises.push((
            pourquoi.to_owned(),
            n,
            format!("{forme}/{empreinte}"),
            String::new(),
        )),
        None => p.fautes.push(format!(
            "{pourquoi} : la partie n'est pas reprise de la garde : {}",
            l.json
        )),
    }
    Ok(l)
}

fn hexa(o: &[u8; 32]) -> String {
    blake3::Hash::from(*o).to_hex().to_string()
}

/// Les jugements de la fin : chaque reprise contre wasmtime, la chaîne entière, la garde
/// finale, les accusés.
fn juger(
    s: &Session,
    p: &mut Passage,
    charge: &ModuleCharge,
    g: &CleDeGarde,
    fige: Option<&Ligne>,
) -> Result<Chaine, String> {
    let salle = s.salle.ok_or("pas de salle")?;
    let disque = s.garde_sur_disque(salle)?;
    let (chaine, coupes) = chaine_ecrite(std::slice::from_ref(&s.enregistrement), &disque)?;
    if coupes > 0 {
        p.fautes
            .push(String::from("l'enregistrement du relais est coupé"));
    }
    match verifier_l_historique::<InstantaneWasm>(g, salle, &chaine) {
        Ok(h) => {
            if !h.en_double.is_empty() {
                p.fautes
                    .push(format!("des gestes deux fois : {:?}", h.en_double));
            }
            p.historique = h;
        }
        Err(e) => p.fautes.push(format!("la chaîne de la garde : {e}")),
    }
    for r in &mut p.reprises {
        let n = usize::try_from(r.1).unwrap_or(usize::MAX);
        let jusqu = chaine.get(..=n).unwrap_or(&[]);
        match reprendre_de_la_garde(charge.fabrique(), g, salle, jusqu, None) {
            Ok((f, e)) => {
                r.3 = format!("{}/{}", hexa(&f), hexa(&e));
                if r.2 != r.3 {
                    p.fautes.push(format!(
                        "{} : la forme canonique diffère de celle que wasmtime reprend des cases 0 à {n} : navigateur {}, wasmtime {}",
                        r.0, r.2, r.3
                    ));
                }
            }
            Err(e) => p.fautes.push(format!(
                "{} : wasmtime ne reprend pas les cases 0 à {n} : {e}",
                r.0
            )),
        }
    }
    if let Some(fige) = fige {
        let du_worker = format!(
            "{}/{}",
            fige.json.get("forme").and_then(Json::as_str).unwrap_or(""),
            fige.json
                .get("empreinte")
                .and_then(Json::as_str)
                .unwrap_or("")
        );
        match reprendre_de_la_garde(charge.fabrique(), g, salle, &disque, None) {
            Ok((f, e)) if format!("{}/{}", hexa(&f), hexa(&e)) == du_worker => {}
            Ok((f, e)) => p.fautes.push(format!(
                "la garde finale reprend une autre partie : wasmtime {}/{}, le Worker figé {du_worker}",
                hexa(&f),
                hexa(&e)
            )),
            Err(e) => p.fautes.push(format!("la garde finale : {e}")),
        }
        match reprendre_de_l_historique(charge.fabrique(), g, salle, &chaine) {
            Ok((f, e)) if format!("{}/{}", hexa(&f), hexa(&e)) == du_worker => {}
            Ok((f, e)) => p.fautes.push(format!(
                "la soirée entière rejouée dans wasmtime finit autrement : {}/{}, le Worker figé {du_worker}",
                hexa(&f),
                hexa(&e)
            )),
            Err(e) => p.fautes.push(format!(
                "la soirée entière ne se rejoue pas dans wasmtime (une empreinte notée non retrouvée ?) : {e}"
            )),
        }
        let registre = relire::<InstantaneWasm>(g, salle, &disque, None)
            .map(|l| l.registre)
            .map_err(|e| format!("{e:?}"))?;
        let telephones: Vec<TelephoneAccuse> = s
            .telephones
            .iter()
            .map(|t| TelephoneAccuse {
                jeton: t.jeton,
                onglet: t.appareil.onglet(),
                accuse: t.appareil.accuse(),
            })
            .collect();
        p.fautes.extend(accuses_hors_de_l_historique(
            Some(&registre),
            &telephones,
            &p.historique,
        ));
    }
    Ok(chaine)
}

/// Ce qui ne doit jamais apparaître : un arrêt du chien de garde, une panne de l'outil,
/// une violation de la politique de contenu, une exception.
fn controler_les_lignes(s: &Session, p: &mut Passage, attendues: &[&str]) {
    for l in &s.lignes {
        match l.quoi() {
            "relance" | "commun-en-panne" => {
                p.fautes
                    .push(format!("le chien de garde a sonné : {}", l.json));
            }
            "panne" => {
                let sorte = l.json.get("sorte").and_then(Json::as_str).unwrap_or("");
                if !attendues.contains(&sorte) {
                    p.fautes.push(format!("une panne : {}", l.json));
                }
            }
            _ => {}
        }
    }
    for a in &s.autres {
        let csp = a.contains("Content Security Policy")
            || a.contains("Content-Security-Policy")
            || a.contains("Refused to");
        if csp || a.starts_with("exception") || a.contains("console.error") {
            p.fautes.push(format!("la page : {a}"));
        }
    }
}

fn charger_le_module(module: &[u8]) -> Result<ModuleCharge, String> {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).map_err(|e| format!("le moteur : {e}"))?;
    moteur
        .charger(module)
        .map_err(|e| format!("le module est refusé : {e}"))
}

fn telephones_et_redite(o: &Options, charge: &ModuleCharge) -> (usize, Option<u64>) {
    let d = charge.declaration();
    let n = o.telephones.unwrap_or(match d.telephones {
        ApplicableFil::Oui(_) => usize::from(d.places.appareils_max()).min(6),
        ApplicableFil::SansObjet(_) => 0,
    });
    let redite = match &d.telephones {
        ApplicableFil::Oui(t) => liaison::periode_de_redite(&t.tenu).map(us_de_duree),
        ApplicableFil::SansObjet(_) => None,
    };
    (n, redite)
}

/// La clé exportée par la page : la commande du diagnostic, puis la ligne `cle`.
fn exporter_la_cle(s: &mut Session, onglet: usize, nom: &str) -> Result<CleDeGarde, String> {
    let depuis = s.lignes.len();
    s.evaluer(onglet, "window.__banc.exporter_la_cle(), true")?;
    let l = s.attendre(depuis, Duration::from_secs(20), "la clé exportée", |l| {
        l.quoi() == "cle" && l.onglet == nom
    })?;
    let hex = l.json.get("g").and_then(Json::as_str).ok_or("cle sans g")?;
    let mut o = [0u8; 32];
    for (k, c) in o.iter_mut().enumerate() {
        let d = hex
            .get(k.saturating_mul(2)..k.saturating_mul(2).saturating_add(2))
            .ok_or("g illisible")?;
        *c = u8::from_str_radix(d, 16).map_err(|e| format!("{e}"))?;
    }
    Ok(CleDeGarde::depuis_octets(o))
}

/// Fige la partie (banc) : sa garde écrite, puis sa forme canonique.
fn figer(s: &mut Session, onglet: usize, nom: &str) -> Result<Ligne, String> {
    let depuis = s.lignes.len();
    s.evaluer(onglet, "window.__banc.figer(), true")?;
    s.attendre(depuis, Duration::from_secs(60), "la partie figée", |l| {
        l.quoi() == "fige" && l.onglet == nom
    })
}

/// Attend que la partie de l'onglet ait joué au moins `pas`.
fn attendre_le_pas(s: &mut Session, nom: &str, pas: u64, delai: Duration) -> Result<(), String> {
    let n = nom.to_owned();
    s.attendre(0, delai, &format!("le pas {pas}"), move |l| {
        l.onglet == n
            && l.quoi() == "etat"
            && l.json
                .get("pas")
                .and_then(Json::as_u64)
                .is_some_and(|x| x >= pas)
    })
    .map(|_| ())
}

/// Le constat de `connect-src 'self'` : la page porte la politique, et le Worker s'est
/// relié (l'accueil du relais est arrivé à la page).
fn constater_la_politique(s: &Session, p: &mut Passage) {
    match http(s.port, "/") {
        Ok(r) => {
            let entete = r
                .lines()
                .find(|l| l.starts_with("Content-Security-Policy:"))
                .unwrap_or("");
            if !entete.contains("connect-src 'self'") {
                p.fautes
                    .push(format!("la page n'a pas connect-src 'self' : {entete}"));
            }
        }
        Err(e) => p.fautes.push(format!("GET / : {e}")),
    }
    if !s.lignes.iter().any(|l| l.quoi() == "accueil") {
        p.fautes.push(String::from(
            "le Worker ne s'est pas relié à ws://localhost:<port>/ws",
        ));
    }
}

/// Le passage d'une soirée dans un navigateur.
#[expect(
    clippy::too_many_lines,
    reason = "la soirée se lit d'un trait, dans l'ordre où elle se joue"
)]
fn soiree(
    o: &Options,
    n: Navigateur,
    paquet: &Path,
    module: &[u8],
    charge: &ModuleCharge,
) -> Result<Passage, String> {
    let (telephones, redite) = telephones_et_redite(o, charge);
    let dossier = o.travail.join(n.nom());
    let mut s = ouvrir(o, n, paquet, module, &dossier, redite)?;
    let mut p = Passage {
        navigateur: s.navigateur.version().to_owned(),
        port: s.port,
        ..Passage::default()
    };
    let deroule = (|| -> Result<Option<Ligne>, String> {
        let premier = debuter(&mut s, &mut p, telephones)?;
        // L'écran rechargé en pleine manche.
        attendre_le_pas(&mut s, &premier, 1_200, Duration::from_secs(60))?;
        let depuis = s.lignes.len();
        s.navigateur.recharger(0)?;
        let l = reprise(&mut s, &mut p, depuis, None, "écran rechargé")?;
        let onglet0 = l.onglet.clone();
        // Le Worker tué par le crochet de test, relancé comme le chien de garde le relance.
        let pas = s
            .etat(&onglet0)
            .and_then(|e| e.get("pas"))
            .and_then(Json::as_u64)
            .unwrap_or(0);
        attendre_le_pas(
            &mut s,
            &onglet0,
            pas.saturating_add(600),
            Duration::from_secs(60),
        )?;
        let depuis = s.lignes.len();
        s.evaluer(0, "window.__banc.tuer(), true")?;
        reprise(&mut s, &mut p, depuis, Some(&onglet0), "Worker tué")?;
        // Le second onglet de l'écran : il prend la salle, le premier est remplacé.
        let pas = s
            .etat(&onglet0)
            .and_then(|e| e.get("pas"))
            .and_then(Json::as_u64)
            .unwrap_or(0);
        attendre_le_pas(
            &mut s,
            &onglet0,
            pas.saturating_add(600),
            Duration::from_secs(60),
        )?;
        let depuis = s.lignes.len();
        let url = format!(
            "http://localhost:{}/?jeu={}",
            s.port,
            blake3::hash(module).to_hex()
        );
        let second = s.navigateur.nouvel_onglet(&url)?;
        let o0 = onglet0.clone();
        let l = reprise(&mut s, &mut p, depuis, None, "second onglet")?;
        let onglet1 = l.onglet.clone();
        if onglet1 == onglet0 {
            p.fautes.push(String::from(
                "le second onglet n'a pas repris la salle : le premier a repris",
            ));
        }
        s.attendre(
            depuis,
            Duration::from_secs(30),
            "Remplacee au premier onglet",
            move |l| {
                l.onglet == o0
                    && l.quoi() == "panne"
                    && l.json.get("sorte").and_then(Json::as_str) == Some("Relais(Remplacee)")
            },
        )?;
        let pas = s
            .etat(&onglet1)
            .and_then(|e| e.get("pas"))
            .and_then(Json::as_u64)
            .unwrap_or(0);
        attendre_le_pas(
            &mut s,
            &onglet1,
            pas.saturating_add(300),
            Duration::from_secs(60),
        )?;
        s.navigateur.fermer_onglet(second)?;
        s.tourner(Duration::from_secs(1))?;
        // « Reprendre ici » au premier onglet.
        let depuis = s.lignes.len();
        s.evaluer(0, "window.__banc.reprendre_ici(), true")?;
        reprise(&mut s, &mut p, depuis, Some(&onglet0), "Reprendre ici")?;
        // La partie jusqu'à une phase terminale.
        let o0 = onglet0.clone();
        s.attendre(
            0,
            Duration::from_secs(o.duree_max_s),
            "une phase terminale",
            move |l| {
                l.onglet == o0
                    && l.quoi() == "etat"
                    && l.json.get("terminale").and_then(Json::as_bool) == Some(true)
            },
        )?;
        // Les marques F8, juste après un point, pour que la garde finale les rejoue.
        let o0 = onglet0.clone();
        let depuis = s.lignes.len();
        s.attendre(
            depuis,
            Duration::from_secs(30),
            "un point frais",
            move |l| {
                l.onglet == o0
                    && l.quoi() == "etat"
                    && l.json
                        .get("depuis_le_point")
                        .and_then(Json::as_u64)
                        .is_some_and(|d| d <= 90)
            },
        )?;
        for _ in 0..3 {
            s.navigateur.touche_f8(0)?;
            p.marques.0 = p.marques.0.saturating_add(1);
            s.tourner(Duration::from_millis(1_000))?;
        }
        s.tourner(Duration::from_millis(500))?;
        let fige = figer(&mut s, 0, &onglet0)?;
        Ok(Some(fige))
    })();
    let fige = match deroule {
        Ok(f) => f,
        Err(e) => {
            p.fautes.push(e);
            None
        }
    };
    p.terminales = u32::try_from(
        s.lignes
            .windows(2)
            .filter(|w| {
                let t = |l: &Ligne| l.json.get("terminale").and_then(Json::as_bool) == Some(true);
                matches!(w, [a, b] if !t(a) && t(b) && b.quoi() == "etat")
            })
            .count(),
    )
    .unwrap_or(u32::MAX);
    let cle = match s.lignes.iter().find(|l| l.quoi() == "page") {
        Some(_) if fige.is_some() => {
            let nom = fige.as_ref().map(|l| l.onglet.clone()).unwrap_or_default();
            exporter_la_cle(&mut s, 0, &nom)
        }
        _ => Err(String::from("la partie n'a pas été figée")),
    };
    constater_la_politique(&s, &mut p);
    controler_les_lignes(&s, &mut p, &["Relais(Remplacee)"]);
    p.lignes = s.lignes.len();
    let lignes = s.lignes.clone();
    let autres = s.autres.clone();
    let g = match cle {
        Ok(g) => Some(g),
        Err(e) => {
            p.fautes.push(e);
            None
        }
    };
    if let Some(g) = g.as_ref() {
        match juger(&s, &mut p, charge, g, fige.as_ref()) {
            // Chaque saut de la soirée, onglet visible, doit venir d'une reprise : le
            // premier pas joué après la partie reprise. Un autre (un tour trop long, une
            // image trop tardive) est une faute.
            Ok(chaine) => match s.salle.map(|salle| sauts(&chaine, g, salle)) {
                Some(Ok(v)) => {
                    let (rattaches, seuls) = rattacher_les_sauts(&v, &s.lignes);
                    p.notes.push(format!(
                        "sauts rattachés : {}",
                        rattaches
                            .iter()
                            .map(|((de, a), pourquoi)| format!("({de}, {a}) {pourquoi}"))
                            .collect::<Vec<_>>()
                            .join(" ; ")
                    ));
                    if !seuls.is_empty() {
                        p.fautes.push(format!(
                            "le journal note {} saut(s) qu'aucune reprise n'explique : {seuls:?}",
                            seuls.len()
                        ));
                    }
                }
                Some(Err(e)) => p.fautes.push(e),
                None => p.fautes.push(String::from("pas de salle")),
            },
            Err(e) => p.fautes.push(e),
        }
    }
    let salle = s.salle;
    let garde = s.garde.clone();
    s.fermer();
    if let (Some(g), Some(salle)) = (g.as_ref(), salle) {
        let cle = dossier.join("cle.txt");
        let _: std::io::Result<()> = std::fs::write(&cle, hexa(g.octets()));
        match crate::rejouer_garde::rejouer(
            &garde,
            Some(salle),
            g,
            module,
            &dossier.join("marques"),
        ) {
            Ok(r) => {
                let rejouees = r.marques.iter().filter(|m| m.issue.is_ok()).count();
                p.marques.1 = rejouees;
                if r.marques.len() != usize::try_from(p.marques.0).unwrap_or(0)
                    || rejouees != r.marques.len()
                {
                    p.fautes.push(format!(
                        "rejouer-garde : {} marques posées, {} trouvées, {rejouees} rejouées\n{r}",
                        p.marques.0,
                        r.marques.len()
                    ));
                }
            }
            Err(e) => p.fautes.push(format!("rejouer-garde : {e}")),
        }
    }
    if p.terminales == 0 {
        p.fautes.push(String::from("aucune phase terminale"));
    }
    ecrire_les_lignes(&dossier, &lignes, &autres);
    Ok(p)
}

fn ecrire_les_lignes(dossier: &Path, lignes: &[Ligne], autres: &[String]) {
    let mut t = String::new();
    for l in lignes {
        let _: core::fmt::Result = writeln!(t, "{:>8} {} {}", l.ms, l.onglet, l.json);
    }
    for a in autres {
        let _: core::fmt::Result = writeln!(t, "autre {a}");
    }
    let _: std::io::Result<()> = std::fs::write(dossier.join("lignes.txt"), t);
}

/// M12 : la reprise au pire moment, `tours` fois.
fn m12(
    o: &Options,
    n: Navigateur,
    paquet: &Path,
    module: &[u8],
    charge: &ModuleCharge,
) -> Result<Passage, String> {
    let (telephones, redite) = telephones_et_redite(o, charge);
    let dossier = o.travail.join(format!("m12-{}", n.nom()));
    let mut s = ouvrir(o, n, paquet, module, &dossier, redite)?;
    let mut p = Passage {
        navigateur: s.navigateur.version().to_owned(),
        port: s.port,
        ..Passage::default()
    };
    let deroule = (|| -> Result<(), String> {
        let mut onglet = debuter(&mut s, &mut p, telephones)?;
        for tour in 0..o.m12 {
            let nom = onglet.clone();
            let depuis = s.lignes.len();
            let l = s.attendre(
                depuis,
                Duration::from_secs(60),
                "le pire moment",
                move |l| {
                    l.onglet == nom
                        && l.quoi() == "etat"
                        && l.json
                            .get("depuis_le_point")
                            .and_then(Json::as_u64)
                            .is_some_and(|d| d >= 575)
                },
            )?;
            let avant = l
                .json
                .get("depuis_le_point")
                .and_then(Json::as_u64)
                .unwrap_or(0);
            let depuis = s.lignes.len();
            s.evaluer(0, "window.__banc.tuer(), true")?;
            let r = reprise(&mut s, &mut p, depuis, None, &format!("M12, tour {tour}"))?;
            onglet.clone_from(&r.onglet);
            let ms = r.json.get("ms").cloned().unwrap_or(Json::Null);
            let total = ms.get("en_jeu").and_then(Json::as_f64).unwrap_or(f64::NAN);
            p.m12.push((total, avant, ms.to_string()));
        }
        Ok(())
    })();
    if let Err(e) = deroule {
        p.fautes.push(e);
    }
    controler_les_lignes(&s, &mut p, &[]);
    let dernier_onglet = s
        .lignes
        .last()
        .map(|l| l.onglet.clone())
        .unwrap_or_default();
    let cle = exporter_la_cle(&mut s, 0, &dernier_onglet);
    p.lignes = s.lignes.len();
    let (lignes, autres) = (s.lignes.clone(), s.autres.clone());
    match cle {
        Ok(g) => {
            if let Err(e) = juger(&s, &mut p, charge, &g, None) {
                p.fautes.push(e);
            }
        }
        Err(e) => p.fautes.push(e),
    }
    s.fermer();
    ecrire_les_lignes(&dossier, &lignes, &autres);
    Ok(p)
}

/// `arriere-plan` : l'onglet caché, la partie qui suit l'horloge, un retard signalé.
fn arriere_plan(
    o: &Options,
    (n, passe): (Navigateur, u32),
    paquet: &Path,
    module: &[u8],
    charge: &ModuleCharge,
) -> Result<Passage, String> {
    let (telephones, redite) = telephones_et_redite(o, charge);
    let dossier = if o.fois > 1 {
        o.travail.join(format!(
            "arriere-plan-{}-{}",
            n.nom(),
            passe.saturating_add(1)
        ))
    } else {
        o.travail.join(format!("arriere-plan-{}", n.nom()))
    };
    let mut s = ouvrir(o, n, paquet, module, &dossier, redite)?;
    let mut p = Passage {
        navigateur: if o.fois > 1 {
            format!(
                "{}, passage {} sur {}",
                s.navigateur.version(),
                passe.saturating_add(1),
                o.fois
            )
        } else {
            s.navigateur.version().to_owned()
        },
        port: s.port,
        ..Passage::default()
    };
    let mut fenetre: Option<(u64, u64)> = None;
    let deroule = (|| -> Result<Option<Ligne>, String> {
        let onglet = debuter(&mut s, &mut p, telephones)?;
        attendre_le_pas(&mut s, &onglet, 600, Duration::from_secs(60))?;
        fenetre = Some(la_fenetre_cachee(o, &mut s, &mut p, &onglet)?);
        s.tourner(Duration::from_secs(2))?;
        Ok(Some(figer(&mut s, 0, &onglet)?))
    })();
    let fige = match deroule {
        Ok(f) => f,
        Err(e) => {
            p.fautes.push(e);
            None
        }
    };
    controler_les_lignes(&s, &mut p, &[]);
    let nom = fige.as_ref().map(|l| l.onglet.clone()).unwrap_or_default();
    let cle = exporter_la_cle(&mut s, 0, &nom);
    p.lignes = s.lignes.len();
    let (lignes, autres) = (s.lignes.clone(), s.autres.clone());
    match cle {
        Ok(g) => match juger(&s, &mut p, charge, &g, fige.as_ref()) {
            Ok(chaine) => match (fenetre, s.salle) {
                (Some(f), Some(salle)) => {
                    juger_les_sauts_caches(&s, &mut p, (&chaine, &g, salle), f);
                }
                _ => p.fautes.push(String::from("pas de fenêtre cachée à juger")),
            },
            Err(e) => p.fautes.push(e),
        },
        Err(e) => p.fautes.push(e),
    }
    s.fermer();
    ecrire_les_lignes(&dossier, &lignes, &autres);
    Ok(p)
}

/// L'onglet caché `o.cache_s` secondes, un retard injecté pendant ce temps ; rend la
/// fenêtre cachée en pas (du dernier rapport avant de cacher au dernier avant de montrer).
fn la_fenetre_cachee(
    o: &Options,
    s: &mut Session,
    p: &mut Passage,
    onglet: &str,
) -> Result<(u64, u64), String> {
    let pas_avant = s
        .etat(onglet)
        .and_then(|e| e.get("pas"))
        .and_then(Json::as_u64)
        .unwrap_or(0);
    let cache = maintenant();
    let cache_ms = cache.saturating_duration_since(s.depart).as_millis();
    s.navigateur.cacher()?;
    s.tourner(Duration::from_secs(2))?;
    // L'onglet est vraiment caché : la page le dit, et (plus bas) aucun pas ne s'y
    // joue au signal d'une image.
    constater_cache(s, p, "au début")?;
    // Un retard injecté dans le Worker pendant que l'onglet est caché (§ 2.3, le cas
    // que vise le signalement) : trois réveils gelés de 150 ms, vingt secondes avant
    // la fin ; la page doit le signaler. Les sauts qu'il fait sont voulus : le Worker
    // dit le pas où il gèle, et seuls ceux-là sont rattachés à lui.
    s.tourner(Duration::from_secs(o.cache_s.saturating_sub(22)))?;
    let depuis = s.lignes.len();
    s.evaluer(0, "window.__banc.retarder(150, 3), true")?;
    let nom = onglet.to_owned();
    if let Err(e) = s.attendre(
        depuis,
        Duration::from_secs(15),
        "le retard signalé, onglet caché",
        move |l| l.onglet == nom && l.quoi() == "retard-signale",
    ) {
        p.fautes.push(e);
    }
    let reste_cache = Duration::from_secs(o.cache_s)
        .saturating_sub(maintenant().saturating_duration_since(cache));
    s.tourner(reste_cache)?;
    constater_cache(s, p, "à la fin")?;
    let pas_apres = s
        .etat(onglet)
        .and_then(|e| e.get("pas"))
        .and_then(Json::as_u64)
        .unwrap_or(0);
    let duree = maintenant().saturating_duration_since(cache);
    let montre_ms = maintenant().saturating_duration_since(s.depart).as_millis();
    s.navigateur.montrer(0)?;
    p.cache = Some((pas_apres.saturating_sub(pas_avant), duree.as_secs()));
    // La partie a suivi l'horloge : au moins 90 % des pas du temps caché.
    let attendus = duree
        .as_micros()
        .saturating_mul(9)
        .checked_div(10u128.saturating_mul(u128::from(PAS_US)))
        .unwrap_or(0);
    if u128::from(pas_apres.saturating_sub(pas_avant)) < attendus {
        p.fautes.push(format!(
            "caché {} s, la partie n'a joué que {} pas (au moins {attendus} attendus)",
            duree.as_secs(),
            pas_apres.saturating_sub(pas_avant)
        ));
    }
    juger_la_fenetre(s, p, onglet, (cache_ms.saturating_add(2_000), montre_ms));
    Ok((pas_avant, pas_apres))
}

/// Un saut du journal : (le dernier pas joué, le pas où reprend le numéro).
type SautDuJournal = (u64, u64);

/// Le plus grand de deux durées positives (`f64::max` est hors des règles d'écriture).
fn plus_grand(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// Les sauts de `arriere-plan` : chacun rattaché à un retard injecté, ou une faute, qu'il
/// tombe dans la fenêtre cachée (`de` à `a`) ou onglet visible.
fn juger_les_sauts_caches(
    s: &Session,
    p: &mut Passage,
    (chaine, g, salle): (&Chaine, &CleDeGarde, CodeSalle),
    (de, a): (u64, u64),
) {
    let v = match sauts(chaine, g, salle) {
        Ok(v) => v,
        Err(e) => return p.fautes.push(e),
    };
    let (rattaches, seuls) = rattacher_les_sauts(&v, &s.lignes);
    p.notes.push(format!(
        "sauts rattachés : {}",
        rattaches
            .iter()
            .map(|((x, y), pourquoi)| format!("({x}, {y}) {pourquoi}"))
            .collect::<Vec<_>>()
            .join(" ; ")
    ));
    let (caches, visibles): (Vec<SautDuJournal>, Vec<SautDuJournal>) =
        seuls.iter().partition(|(x, _)| *x >= de && *x <= a);
    if !caches.is_empty() {
        p.fautes.push(format!(
            "le journal note {} saut(s) pendant que l'onglet était caché (pas {de} à {a}) : {caches:?}",
            caches.len()
        ));
    }
    if !visibles.is_empty() {
        p.fautes.push(format!(
            "le journal note {} saut(s) onglet visible, qu'aucune reprise ni aucun retard injecté n'explique : {visibles:?}",
            visibles.len()
        ));
    }
}

/// L'onglet est caché : la page le dit (`document.visibilityState`).
fn constater_cache(s: &mut Session, p: &mut Passage, quand: &str) -> Result<(), String> {
    let v = s.evaluer(0, "document.visibilityState")?;
    if v.as_str() != Some("hidden") {
        p.fautes.push(format!(
            "l'onglet n'est pas caché {quand} : document.visibilityState vaut {v}"
        ));
    }
    Ok(())
}

/// La fenêtre cachée, de `de_ms` à `a_ms` (l'heure des lignes du banc) : presque aucun
/// pas ne s'y joue au signal d'une image (au plus 1 % : la page ne reçoit plus d'images,
/// l'onglet était bien caché, et le « réussi » n'est pas vide) ; ce que les tours longs et les trous entre
/// deux pas ont été, noté.
fn juger_la_fenetre(s: &Session, p: &mut Passage, onglet: &str, (de_ms, a_ms): (u128, u128)) {
    let dans = |l: &&Ligne| l.onglet == onglet && l.ms >= de_ms && l.ms <= a_ms;
    let etats: Vec<&Ligne> = s
        .lignes
        .iter()
        .filter(dans)
        .filter(|l| l.quoi() == "etat")
        .collect();
    let compte = |l: &Ligne, k: &str| l.json.get(k).and_then(Json::as_u64).unwrap_or(0);
    match (etats.first(), etats.last()) {
        (Some(a), Some(b)) if etats.len() >= 2 => {
            let images = compte(b, "par_image").saturating_sub(compte(a, "par_image"));
            let minuterie = compte(b, "par_minuterie").saturating_sub(compte(a, "par_minuterie"));
            p.notes.push(format!(
                "fenêtre cachée : {minuterie} pas joués par la minuterie, {images} au signal d'une image"
            ));
            // Firefox donne encore, de loin en loin, une image à un onglet caché
            // (constaté : de 0 à 3 en 360 s, `document.visibilityState` valant `hidden`) ;
            // un onglet visible en donne une par pas, des milliers. Au plus 1 % des pas.
            if images.saturating_mul(100) > images.saturating_add(minuterie) || minuterie == 0 {
                p.fautes.push(format!(
                    "l'onglet n'était pas caché : {images} pas joués au signal d'une image, {minuterie} par la minuterie"
                ));
            }
        }
        _ => p.fautes.push(String::from(
            "la fenêtre cachée n'a pas deux rapports d'état",
        )),
    }
    let tours: Vec<&Ligne> = s
        .lignes
        .iter()
        .filter(dans)
        .filter(|l| l.quoi() == "tour")
        .collect();
    let ms = |l: &Ligne, k: &str| l.json.get(k).and_then(Json::as_f64).unwrap_or(0.0);
    let points: Vec<&&Ligne> = tours
        .iter()
        .filter(|l| l.json.get("point").and_then(Json::as_bool) == Some(true))
        .collect();
    let plus_long = tours.iter().map(|l| ms(l, "ms")).fold(0.0, plus_grand);
    let trous: Vec<String> = tours
        .iter()
        .filter(|l| ms(l, "trou_ms") >= 30.0)
        .map(|l| format!("{} ({:.1} ms)", compte(l, "pas"), ms(l, "trou_ms")))
        .collect();
    p.notes.push(format!(
        "fenêtre cachée : {} tours de plus de 10 ms (le plus long {plus_long:.1} ms ; {} où un point commence, au plus {:.1} ms) ; {} trous de 30 ms ou plus entre deux pas : {}",
        tours.iter().filter(|l| ms(l, "ms") >= 10.0).count(),
        points.len(),
        points.iter().map(|l| ms(l, "ms")).fold(0.0, plus_grand),
        trous.len(),
        trous.join(", ")
    ));
}

/// Les sauts du journal de toute la soirée : (le dernier pas joué, le pas où reprend le
/// numéro).
fn sauts(chaine: &Chaine, g: &CleDeGarde, salle: CodeSalle) -> Result<Vec<(u64, u64)>, String> {
    let lue = salle::garde::relire_l_historique::<InstantaneWasm>(g, salle, chaine)
        .map_err(|e| format!("l'historique : {e:?}"))?;
    Ok(lue
        .journal
        .lignes()
        .iter()
        .filter_map(|l| match l {
            partie::Ligne::Saut { de, a } => Some((de.numero(), a.numero())),
            partie::Ligne::Arrivees { .. }
            | partie::Ligne::Entree(_)
            | partie::Ligne::Empreinte { .. }
            | partie::Ligne::Pause { .. }
            | partie::Ligne::Vue { .. }
            | partie::Ligne::Partie { .. }
            | partie::Ligne::Reprise { .. } => None,
        })
        .collect())
}

/// Rattache chaque saut du journal à ce qui l'explique, chaque explication une fois : une
/// **reprise** (une ligne « en-jeu » relue d'une case : la partie reprise à son pas `p`
/// rattrape l'horloge d'un saut au premier tour, `de` valant `p` ou `p + 1`), ou un
/// **retard injecté** par le banc (une ligne « retarde » au pas `p` : le Worker gelé
/// 150 ms saute dans les deux pas qui suivent, `de` de `p` à `p + 2`). Rend les sauts
/// rattachés, avec leur raison, et ceux que rien n'explique.
fn rattacher_les_sauts(
    sauts: &[SautDuJournal],
    lignes: &[Ligne],
) -> (Vec<(SautDuJournal, String)>, Vec<SautDuJournal>) {
    let mut raisons: Vec<(u64, u64, String)> = lignes
        .iter()
        .filter_map(|l| {
            let pas = l.json.get("pas").and_then(Json::as_u64)?;
            match l.quoi() {
                "en-jeu" if l.json.get("relue").and_then(Json::as_u64).is_some() => {
                    Some((pas, pas.saturating_add(1), format!("reprise au pas {pas}")))
                }
                "retarde" => Some((
                    pas,
                    pas.saturating_add(2),
                    format!("retard injecté au pas {pas}"),
                )),
                _ => None,
            }
        })
        .collect();
    let mut rattaches = Vec::new();
    let mut seuls = Vec::new();
    for &(de, a) in sauts {
        match raisons
            .iter()
            .position(|(min, max, _)| de >= *min && de <= *max)
        {
            Some(i) => {
                let (_, _, pourquoi) = raisons.remove(i);
                rattaches.push(((de, a), pourquoi));
            }
            None => seuls.push((de, a)),
        }
    }
    (rattaches, seuls)
}

/// Joue la soirée (ou M12, ou l'arrière-plan) dans chaque navigateur, l'un après l'autre.
pub fn jouer(o: &Options, soiree_dir: &Path) -> Result<Vec<Passage>, String> {
    let module = std::fs::read(&o.module).map_err(|e| format!("{} : {e}", o.module.display()))?;
    let charge = charger_le_module(&module)?;
    let paquet = if o.construire {
        construire_le_paquet(soiree_dir)?
    } else {
        dernier_paquet(soiree_dir)?
    };
    std::fs::create_dir_all(&o.travail).map_err(|e| format!("{e}"))?;
    let mut passages = Vec::new();
    let mut tours: Vec<(Navigateur, u32)> = Vec::new();
    for n in &o.navigateurs {
        let fois = if o.cache_s > 0 { o.fois.max(1) } else { 1 };
        tours.extend((0..fois).map(|k| (*n, k)));
    }
    for (n, passe) in tours {
        let n = &n;
        let r = if o.m12 > 0 {
            m12(o, *n, &paquet, &module, &charge)
        } else if o.cache_s > 0 {
            arriere_plan(o, (*n, passe), &paquet, &module, &charge)
        } else {
            soiree(o, *n, &paquet, &module, &charge)
        };
        match r {
            Ok(p) => passages.push(p),
            Err(e) if e.starts_with("pas exécuté") => passages.push(Passage {
                navigateur: n.nom().to_owned(),
                pas_execute: Some(e),
                ..Passage::default()
            }),
            Err(e) => passages.push(Passage {
                navigateur: n.nom().to_owned(),
                fautes: vec![e],
                ..Passage::default()
            }),
        }
    }
    Ok(passages)
}

impl core::fmt::Display for Passage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(e) = &self.pas_execute {
            return writeln!(f, "== {} : {e}", self.navigateur);
        }
        writeln!(
            f,
            "== {} (moteur de dessin {}, port {}) : {} lignes du banc",
            self.navigateur, self.moteur, self.port, self.lignes
        )?;
        for (pourquoi, n, navigateur, wasmtime) in &self.reprises {
            let marque = if navigateur == wasmtime {
                "même forme"
            } else {
                "ÉCART"
            };
            writeln!(
                f,
                "  reprise « {pourquoi} » (cases 0 à {n}) : {marque} ; navigateur {}…",
                navigateur.get(..16).unwrap_or(navigateur)
            )?;
        }
        for (ms, avant, etapes) in &self.m12 {
            writeln!(
                f,
                "  M12 : {ms:.1} ms, {avant} pas depuis le point ; {etapes}"
            )?;
        }
        let h = &self.historique;
        writeln!(
            f,
            "  chaîne : {} cases, {} points, {} lignes, {} gestes, {} en double, {} écartées, {} sauts",
            h.cases,
            h.points,
            h.lignes,
            h.gestes.len(),
            h.en_double.len(),
            h.ecartees,
            h.sauts
        )?;
        writeln!(
            f,
            "  phases terminales : {} ; marques F8 posées {}, rejouées {}",
            self.terminales, self.marques.0, self.marques.1
        )?;
        if let Some((pas, s)) = self.cache {
            writeln!(f, "  caché {s} s : {pas} pas joués")?;
        }
        for n in &self.notes {
            writeln!(f, "  {n}")?;
        }
        if self.fautes.is_empty() {
            writeln!(f, "  verdict : réussi")
        } else {
            writeln!(f, "  verdict : ÉCHEC")?;
            for e in &self.fautes {
                writeln!(f, "    {e}")?;
            }
            Ok(())
        }
    }
}
