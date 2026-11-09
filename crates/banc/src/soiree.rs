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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use jeu_abi::{ApplicableFil, Capacites};
use liaison::CodeSalle;
use moteur_wasmtime::{FabriqueWasm, InstantaneWasm, ModuleCharge, Moteur, ReglagesMoteur};
use salle::appareil::Appareil;
use salle::chiffre::{CleDeGarde, Fragment, Hasard};
use salle::garde::relire;
use salle::horloge::us_de_duree;
use salle::page::JoueurDePage;
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
    /// Les pages du téléphone dans ces navigateurs, en plus des téléphones natifs (lot 12) :
    /// une page par navigateur, pilotée par sa chaîne d'entrées.
    pub pages: Vec<Navigateur>,
    /// Le passage dure au moins ce nombre de secondes depuis la partie ouverte : la partie
    /// continue (et le pilote des pages) avant les marques F8 (*correction du lot 12* :
    /// « Sidequest 5 min »).
    pub duree_min_s: u64,
    /// Les pages arrivent en retard, une à une, quand tous les téléphones natifs sont
    /// accueillis et ont reçu le module ; pendant que chacune reçoit le sien, les
    /// allers-retours d'écho des natifs ne doivent pas croître (*correction du lot 12* :
    /// « un téléphone qui rejoint Sidequest à 71 appareils ne retarde personne »).
    pub retardataires: bool,
    /// `changer` : les autres jeux du catalogue, que la salle joue ensuite, l'un après
    /// l'autre, depuis l'écran de démarrage (lot 12, fin).
    pub autres_jeux: Vec<PathBuf>,
    /// Un défaut voulu (`?fautif=…`, au banc seulement), pour le contrôle négatif d'un
    /// scénario (`changer` : `changer-sans-livrer`).
    pub fautif: Option<String>,
    /// Le navigateur avec sa fenêtre (`--avec-interface`, M6, lot 13 ; Chrome aussi
    /// depuis ce lot).
    pub interface: crate::navigateur::Interface,
    /// `arriere-plan` : comment l'écran passe à l'arrière-plan (lot 13, M6).
    pub cacher: Cacher,
    /// `soiree-complete` (lot 14) : les jeux à la suite, « On rejoue » deux fois chacun.
    pub complete: bool,
}

/// Comment `arriere-plan` passe l'écran à l'arrière-plan (M6, lot 13).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Cacher {
    /// Un onglet vierge passé devant (le scénario du lot 10a).
    #[default]
    Onglet,
    /// Une autre fenêtre du navigateur ouverte par-dessus (avec interface).
    Recouverte,
    /// La fenêtre de l'écran réduite dans le Dock (avec interface).
    Reduite,
}

impl Cacher {
    /// Son nom, au rapport et dans le dossier de travail.
    pub const fn nom(self) -> &'static str {
        match self {
            Cacher::Onglet => "onglet",
            Cacher::Recouverte => "recouverte",
            Cacher::Reduite => "reduite",
        }
    }
}

/// Un téléphone qui rejoint retarde les autres si la médiane ou le 99ᵉ centile de leurs
/// allers-retours d'écho, pendant qu'il reçoit le module, dépasse celle ou celui des 20 s
/// d'avant de plus d'un pas à 60 Hz (choisi, correction du lot 12 : une vue qui arrive un
/// pas plus tard).
pub const RETARD_TOLERE_US: u64 = 16_667;

/// M13 : la part du commun dans le Worker à chaque tour doit tenir dans la moitié de la
/// part d'image que E19 laisse au commun (PHASE-3, § 3, R2) : une image courante de
/// 1/60 s, la moitié au jeu (`PART_DU_JEU_POUR_CENT`), la moitié du reste, 4,17 ms ; jugée
/// au 99ᵉ centile, comme l'image courante de E19.
pub const M13_MAX_MS: f64 = 1000.0 / 60.0 / 2.0 / 2.0;

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
    pub(crate) fn quoi(&self) -> &str {
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
    /// Le scénario ne relit pas la chaîne et juge la garde finale à la place (`changer`,
    /// `soiree-complete`) : sa ligne « chaîne » le dit au lieu d'une ligne de zéros. Un
    /// scénario qui relit la chaîne et la trouve vide garde sa ligne de zéros (lot 14
    /// corrigé).
    pub chaine_non_relue: bool,
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
/// rend ce dossier. Depuis le lot 10b, la seule définition est celle de
/// `soiree-relais preparer` (`relais::preparer`).
pub fn construire_le_paquet(soiree: &Path) -> Result<PathBuf, String> {
    relais::preparer::construire_le_paquet(soiree, None)
}

/// Le dernier paquet construit.
pub fn dernier_paquet(soiree: &Path) -> Result<PathBuf, String> {
    relais::preparer::dernier_paquet(&soiree.join("target/soiree-web"))
}

// ---------------------------------------------------------------------------
// Les téléphones natifs
// ---------------------------------------------------------------------------

pub(crate) struct Telephone {
    pub(crate) appareil: Appareil<Graine>,
    ws: Option<Ws>,
    jeton: [u8; 32],
    revient: Option<Instant>,
    prochain_reveil: Instant,
    /// La page du téléphone (lot 12) : les coups du bot partent du bouton qui les porte ;
    /// le premier téléphone est en plus un singe.
    pub(crate) joueur: JoueurDePage<FabriqueWasm>,
    /// Le hasard du singe.
    hasard: Graine,
}

// ---------------------------------------------------------------------------
// Une session : le relais, le navigateur, les téléphones, les lignes de la page
// ---------------------------------------------------------------------------

pub(crate) struct Session {
    pub(crate) relais: Option<relais::coque::Relais>,
    pub(crate) port: u16,
    pub(crate) garde: PathBuf,
    pub(crate) enregistrement: PathBuf,
    pub(crate) navigateur: Lance,
    pub(crate) lignes: Vec<Ligne>,
    pub(crate) autres: Vec<String>,
    /// Les téléphones natifs, sur leur propre fil (lot 12, fin).
    pub(crate) natifs: Natifs,
    pub(crate) redite_us: Option<u64>,
    pub(crate) salle: Option<CodeSalle>,
    pub(crate) fragment: Option<Fragment>,
    pub(crate) depart: Instant,
    pub(crate) graine: u64,
    /// Quand le relais du banc a été lancé : son horloge (celle de son enregistrement) part
    /// de là, à quelques millisecondes près.
    pub(crate) relais_lance: Instant,
    /// L'heure du relais à `relais_lance`, en microsecondes : son horloge (celle de son
    /// enregistrement) part du premier relais lancé par ce processus, pas de chacun
    /// (`relais::horloge`). *Correction du lot 12* : au second passage, M7 comparait les
    /// croix tenues à des instants décalés de tout le premier passage, et n'y lisait rien.
    pub(crate) relais_us0: u64,
    /// Les pages dans les navigateurs (lot 12).
    pub(crate) pages: Vec<crate::telephones::PageTel>,
    pub(crate) prochain_pilotage: Instant,
}

/// Un tour des téléphones natifs de plus de ce temps après le précédent est un silence noté
/// (choisi : quinze pas).
pub(crate) const SILENCE_NOTE: Duration = Duration::from_millis(250);

/// Un silence des téléphones natifs au-delà de ce temps est une faute du banc (choisi : sous
/// la coupure d'un téléphone, `salle::appareil::Appareil::coupe`, 1,5 s, après laquelle
/// l'autorité peut le perdre).
pub(crate) const SILENCE_MAX: Duration = Duration::from_secs(1);

/// Un tour du fil des natifs relie au plus ce nombre de téléphones (choisi). Relier un
/// téléphone (la connexion et la poignée WebSocket, bloquantes) coûte environ 23 ms : les
/// 69 natifs de `soiree-71` reliés dans un seul tour taisaient tous les autres 1,6 à 1,7 s
/// (relevé à la fin du lot 12, la première preuve `soiree-71`, au-delà de `SILENCE_MAX`).
const RELIER_PAR_TOUR: usize = 4;

/// Le fil des natifs dort ce temps entre deux tours (choisi).
const SOMME_DES_NATIFS: Duration = Duration::from_millis(2);

/// Ce que partagent le fil des téléphones natifs et celui du banc.
pub(crate) struct Troupe {
    pub(crate) telephones: Vec<Telephone>,
    /// Les allers-retours d'écho des téléphones natifs : (reçu à, durée), en microsecondes
    /// depuis `depart` (correction du lot 12).
    pub(crate) allers_retours: Vec<(u64, u64)>,
    /// Les téléphones natifs chargent le module reçu par l'une de ces fabriques, celle
    /// dont le BLAKE3 est le sien (lot 12 : ils passent par la page ; depuis la fin du lot,
    /// une par jeu du catalogue, puisque la salle change de jeu).
    pub(crate) fabriques: Vec<(FabriqueWasm, [u8; 32])>,
    port: u16,
    depart: Instant,
    /// Les tours joués par le fil.
    pub(crate) tours: u64,
    /// Les silences de plus de [`SILENCE_NOTE`] : (début, durée), en millisecondes depuis
    /// `depart`.
    pub(crate) silences: Vec<(u64, u64)>,
    dernier_tour: Option<Instant>,
}

/// Les téléphones natifs tournent **sur leur propre fil** (lot 12, fin). Avant, le fil du
/// banc les faisait tourner entre deux lectures de la console : toute attente du banc les
/// taisait. Dans `soiree-71`, l'essai d'une session Safari (`safaridriver`, 30 s avant de
/// refuser) les a tus 30,3 s, et l'autorité, qui ne recevait plus leurs échos, a perdu
/// leurs 69 liens (M13 : de 71 appareils accueillis à 2) ; chaque évaluation `BiDi` sur une
/// page Firefox neuve retardait d'autant tous les échos à la fois.
pub(crate) struct Natifs {
    troupe: Arc<Mutex<Troupe>>,
    arret: Arc<AtomicBool>,
    fil: Option<JoinHandle<()>>,
}

impl Natifs {
    /// Lance le fil, sans téléphone.
    pub(crate) fn lancer(
        port: u16,
        depart: Instant,
        fabriques: Vec<(FabriqueWasm, [u8; 32])>,
    ) -> Natifs {
        let troupe = Arc::new(Mutex::new(Troupe {
            telephones: Vec::new(),
            allers_retours: Vec::new(),
            fabriques,
            port,
            depart,
            tours: 0,
            silences: Vec::new(),
            dernier_tour: None,
        }));
        let arret = Arc::new(AtomicBool::new(false));
        let (t, a) = (Arc::clone(&troupe), Arc::clone(&arret));
        let fil = std::thread::Builder::new()
            .name(String::from("natifs"))
            .spawn(move || {
                while !a.load(Ordering::Relaxed) {
                    t.lock().unwrap_or_else(PoisonError::into_inner).tourner();
                    std::thread::sleep(SOMME_DES_NATIFS);
                }
            })
            .ok();
        Natifs { troupe, arret, fil }
    }

    /// Les téléphones natifs, le temps d'y lire ou d'y ajouter : le fil attend.
    pub(crate) fn troupe(&self) -> MutexGuard<'_, Troupe> {
        self.troupe.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Arrête le fil, puis coupe chaque téléphone.
    pub(crate) fn arreter(&mut self) {
        self.arret.store(true, Ordering::Relaxed);
        if let Some(f) = self.fil.take() {
            let _: std::thread::Result<()> = f.join();
        }
        for t in &mut self.troupe().telephones {
            if let Some(ws) = t.ws.take() {
                ws.couper();
            }
        }
    }
}

impl Drop for Natifs {
    fn drop(&mut self) {
        self.arreter();
    }
}

fn heure_locale_us(depart: Instant) -> u64 {
    // Les téléphones ont leur propre horloge ; seul compte qu'elle avance.
    u64::try_from(maintenant().saturating_duration_since(depart).as_micros()).unwrap_or(u64::MAX)
}

impl Troupe {
    /// Note le tour qui commence, et le silence qui le précède s'il dépasse
    /// [`SILENCE_NOTE`].
    fn noter_le_tour(&mut self) {
        let ici = maintenant();
        if let Some(d) = self.dernier_tour
            && ici.saturating_duration_since(d) > SILENCE_NOTE
        {
            let ms = |i: Instant| {
                u64::try_from(i.saturating_duration_since(self.depart).as_millis())
                    .unwrap_or(u64::MAX)
            };
            self.silences.push((
                ms(d),
                u64::try_from(ici.saturating_duration_since(d).as_millis()).unwrap_or(u64::MAX),
            ));
        }
        self.dernier_tour = Some(ici);
        self.tours = self.tours.saturating_add(1);
    }

    /// Un tour : chaque téléphone lit ce qui lui arrive, se réveille à son pas, joue par
    /// sa page, et envoie.
    fn tourner(&mut self) {
        let adresse = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), self.port);
        let local = heure_locale_us(self.depart);
        self.noter_le_tour();
        // § 8.1, le singe partage le salon (lot 12, fin) : le singe (le premier téléphone)
        // ne joue rien tant que chaque autre natif n'a pas vu accuser un premier geste.
        // Le dernier accusé d'un natif ne redescend pas (aucun ne recharge sa page) : le
        // salon au complet le reste.
        let salon_au_complet = salle::page::salon_au_complet(
            self.telephones.iter().skip(1).map(|t| t.appareil.accuse()),
        );
        let mut a_relier = RELIER_PAR_TOUR;
        for t in &mut self.telephones {
            if t.ws.is_none() {
                if a_relier > 0
                    && t.revient.is_some_and(|r| maintenant() >= r)
                    && !t.appareil.fini()
                {
                    a_relier = a_relier.saturating_sub(1);
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
                        Ok(Some(o)) => {
                            t.appareil.recevoir(local, &o);
                            self.allers_retours
                                .extend(t.appareil.prendre_les_allers_retours());
                        }
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
                t.joueur.retenir_le_singe(if salon_au_complet {
                    jeu_abi::Oui::Non
                } else {
                    jeu_abi::Oui::Oui
                });
                let fabriques = &self.fabriques;
                t.joueur.jouer(
                    local,
                    &mut t.appareil,
                    |m| {
                        let e = blake3::hash(m);
                        fabriques
                            .iter()
                            .find(|(_, x)| e.as_bytes() == x)
                            .map(|(f, _)| f.clone())
                    },
                    &mut t.hasard,
                );
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
}

impl Session {
    /// Lit la console de la page pendant `delai`, et fait tourner les téléphones.
    pub(crate) fn tourner(&mut self, delai: Duration) -> Result<(), String> {
        let fin = maintenant().checked_add(delai).ok_or("délai")?;
        loop {
            if maintenant() >= self.prochain_pilotage && !self.pages.is_empty() {
                self.prochain_pilotage = maintenant()
                    .checked_add(crate::telephones::PILOTAGE)
                    .unwrap_or_else(maintenant);
                crate::telephones::piloter(self);
            }
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
    pub(crate) fn attendre(
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

    pub(crate) fn evaluer(&mut self, onglet: usize, js: &str) -> Result<Json, String> {
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
            let singe = Graine::new(
                "singe",
                self.graine.wrapping_mul(1000).wrapping_add(k as u64),
            );
            let mut appareil = Appareil::new(
                fragment.clone(),
                salle,
                (jeton, onglet),
                Capacites::AUCUNE,
                self.redite_us,
                h,
            );
            appareil.coups_par_la_page();
            self.natifs.troupe().telephones.push(Telephone {
                appareil,
                ws: None,
                jeton,
                revient: Some(maintenant()),
                prochain_reveil: maintenant(),
                joueur: if k == 0 {
                    JoueurDePage::singe(salle::simulation::SINGE_US)
                } else {
                    JoueurDePage::default()
                },
                hasard: singe,
            });
        }
        Ok(())
    }

    /// Le relais arrêté, les téléphones coupés, le navigateur fermé : la garde sur disque
    /// et l'enregistrement restent.
    pub(crate) fn fermer(mut self) {
        self.natifs.arreter();
        for p in self.pages.drain(..) {
            p.fermer();
        }
        self.navigateur.fermer();
        if let Some(r) = self.relais.take() {
            r.arreter();
        }
    }

    /// Les cases de la garde sur disque.
    pub(crate) fn garde_sur_disque(&self, salle: CodeSalle) -> Result<Chaine, String> {
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

/// Ouvre une session : le relais avec le paquet et le module, le navigateur sur l'écran ;
/// `requete` s'ajoute à l'adresse de la page (`&bots=non`, `&moteur=webgl2`, lot 11).
pub(crate) fn ouvrir(
    o: &Options,
    n: Navigateur,
    paquet: &Path,
    module: &[u8],
    dossier: &Path,
    (redite_us, requete): (Option<u64>, &str),
) -> Result<Session, String> {
    let _: std::io::Result<()> = std::fs::remove_dir_all(dossier);
    let jeux = dossier.join("jeux");
    std::fs::create_dir_all(&jeux).map_err(|e| format!("{e}"))?;
    let empreinte = blake3::hash(module).to_hex();
    std::fs::write(jeux.join(format!("{empreinte}.wasm")), module).map_err(|e| format!("{e}"))?;
    let mut fabriques: Vec<(FabriqueWasm, [u8; 32])> = charger_le_module(module)
        .ok()
        .map(|c| (c.fabrique(), *blake3::hash(module).as_bytes()))
        .into_iter()
        .collect();
    // `changer` (lot 12, fin) : les autres jeux au catalogue du relais.
    for autre in &o.autres_jeux {
        let octets = std::fs::read(autre).map_err(|e| format!("{} : {e}", autre.display()))?;
        let e = blake3::hash(&octets);
        std::fs::write(jeux.join(format!("{}.wasm", e.to_hex())), &octets)
            .map_err(|e| format!("{e}"))?;
        if let Ok(c) = charger_le_module(&octets) {
            fabriques.push((c.fabrique(), *e.as_bytes()));
        }
    }
    let garde = dossier.join("garde");
    let enregistrement = dossier.join("enregistrement.bin");
    let port = if std::net::TcpListener::bind(("127.0.0.1", PORT_DE_LA_SOIREE)).is_ok()
        && std::net::TcpListener::bind(("::1", PORT_DE_LA_SOIREE)).is_ok()
    {
        PORT_DE_LA_SOIREE
    } else {
        port_libre()?
    };
    let relais_lance = maintenant();
    let relais_us0 = relais::horloge::maintenant_us();
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
        annonce_du_banc: None,
        retard_d_echo_du_banc: None,
    })
    .map_err(|e| format!("le relais : {e}"))?;
    let profils = o.travail.join("profils");
    let mut navigateur = match Lance::lancer_avec(n, None, &profils, o.interface) {
        Ok(l) => l,
        Err(e) => {
            relais.arreter();
            return Err(format!("pas exécuté : {e}"));
        }
    };
    let fautif = o
        .fautif
        .as_ref()
        .map_or_else(String::new, |f| format!("&fautif={f}"));
    let url = format!("http://localhost:{port}/?jeu={empreinte}{requete}{fautif}");
    if let Err(e) = navigateur.ouvrir(&url) {
        navigateur.fermer();
        relais.arreter();
        return Err(e);
    }
    let depart = maintenant();
    Ok(Session {
        relais: Some(relais),
        port,
        garde,
        enregistrement,
        navigateur,
        lignes: Vec::new(),
        autres: Vec::new(),
        natifs: Natifs::lancer(port, depart, fabriques),
        redite_us,
        salle: None,
        fragment: None,
        depart,
        graine: 1,
        pages: Vec::new(),
        prochain_pilotage: maintenant(),
        relais_lance,
        relais_us0,
    })
}

/// La partie neuve s'ouvre : la page, l'accueil, la partie en jeu ; puis les téléphones.
pub(crate) fn debuter(
    s: &mut Session,
    p: &mut Passage,
    telephones: usize,
) -> Result<String, String> {
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

pub(crate) fn hexa(o: &[u8; 32]) -> String {
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
            .natifs
            .troupe()
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
pub(crate) fn controler_les_lignes(s: &Session, p: &mut Passage, attendues: &[&str]) {
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

pub(crate) fn charger_le_module(module: &[u8]) -> Result<ModuleCharge, String> {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).map_err(|e| format!("le moteur : {e}"))?;
    moteur
        .charger(module)
        .map_err(|e| format!("le module est refusé : {e}"))
}

pub(crate) fn telephones_et_redite(o: &Options, charge: &ModuleCharge) -> (usize, Option<u64>) {
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
pub(crate) fn exporter_la_cle(
    s: &mut Session,
    onglet: usize,
    nom: &str,
) -> Result<CleDeGarde, String> {
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
pub(crate) fn figer(s: &mut Session, onglet: usize, nom: &str) -> Result<Ligne, String> {
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
    let mut s = ouvrir(o, n, paquet, module, &dossier, (redite, ""))?;
    let mut p = Passage {
        navigateur: s.navigateur.version().to_owned(),
        port: s.port,
        ..Passage::default()
    };
    let deroule = (|| -> Result<Option<Ligne>, String> {
        let premier = debuter(&mut s, &mut p, telephones)?;
        let ouverte = maintenant();
        if o.retardataires {
            ouvrir_les_pages_en_retard(o, &mut s, &mut p)?;
        } else {
            ouvrir_les_pages(o, &mut s, &mut p)?;
        }
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
        // Les pages du téléphone : rechargée en pleine écriture, puis un onglet jumeau.
        recharger_une_page_en_pleine_ecriture(&mut s, &mut p);
        onglet_jumeau(&mut s, &mut p);
        couper_une_page_qui_tient_la_croix(&mut s, &mut p);
        remplir_la_file_d_une_page(&mut s, &mut p);
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
        // Avec des pages : le podium, puis « On rejoue » (une partie neuve qui n'est plus
        // terminale).
        if !s.pages.is_empty() {
            let o0 = onglet0.clone();
            let depuis = s.lignes.len();
            match s.attendre(
                depuis,
                Duration::from_secs(180),
                "« On rejoue »",
                move |l| {
                    l.onglet == o0
                        && l.quoi() == "etat"
                        && l.json.get("terminale").and_then(Json::as_bool) == Some(false)
                },
            ) {
                Ok(_) => p.notes.push(String::from(
                    "« On rejoue » : une partie neuve après le podium",
                )),
                Err(e) => p.fautes.push(e),
            }
        }
        // La partie prolongée jusqu'à la durée demandée (« Sidequest 5 min »).
        if o.duree_min_s > 0 {
            let fin = ouverte
                .checked_add(Duration::from_secs(o.duree_min_s))
                .ok_or("délai")?;
            while maintenant() < fin {
                s.tourner(Duration::from_secs(1))?;
            }
            p.notes.push(format!(
                "partie jouée {} s depuis son ouverture (au moins {} s demandées)",
                maintenant().saturating_duration_since(ouverte).as_secs(),
                o.duree_min_s
            ));
        }
        // Les marques F8, juste après un point, pour que la garde finale les rejoue.
        // *Correction de la fin du lot 12* : les pilotes des pages s'arrêtent d'abord. Un
        // toucher que Chrome n'accuse pas (« CDP Input.dispatchTouchEvent : pas de réponse
        // en 20s ») tenait le fil du banc 20 s à chaque tour : les trois marques s'étaient
        // posées à 20 s l'une de l'autre, la partie figée plus de deux points après la
        // première, et la garde ne gardait plus le point d'où les rejouer.
        for pg in &mut s.pages {
            pg.arreter_le_pilote();
        }
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
    juger_les_pages(&mut s, &mut p, g_pour_les_pages(&cle));
    juger_les_natifs(&s, &mut p);
    noter_m13(o, &s, &mut p);
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
            // Chaque saut de la soirée, onglet visible, doit venir d'une reprise : les
            // premiers pas joués après la partie reprise, autant que le jeu en rattrape.
            // Un autre (un tour trop long, une image trop tardive) est une faute.
            Ok(chaine) => match s.salle.map(|salle| sauts(&chaine, g, salle)) {
                Some(Ok(v)) => {
                    let (rattaches, seuls) =
                        rattacher_les_sauts(&v, &s.lignes, charge.declaration().rattrapage);
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

// ---------------------------------------------------------------------------
// Les pages du téléphone (lot 12)
// ---------------------------------------------------------------------------

#[expect(
    clippy::cast_precision_loss,
    reason = "des microsecondes en millisecondes, pour le rapport"
)]
fn ms(us: u64) -> f64 {
    us as f64 / 1000.0
}

fn g_pour_les_pages(cle: &Result<CleDeGarde, String>) -> Option<[u8; 32]> {
    cle.as_ref().ok().map(|g| *g.octets())
}

/// Ouvre une page du téléphone dans chaque navigateur de `--pages`, et attend que chacune
/// soit accueillie, ait reçu le module par la connexion et posé une page (M4 relevé).
pub(crate) fn ouvrir_les_pages(
    o: &Options,
    s: &mut Session,
    p: &mut Passage,
) -> Result<(), String> {
    if o.pages.is_empty() {
        return Ok(());
    }
    let (Some(salle), Some(fragment)) = (s.salle, s.fragment.clone()) else {
        return Err(String::from("pages : ni salle ni fragment"));
    };
    let profils = o.travail.join("profils-pages");
    for n in &o.pages {
        match crate::telephones::PageTel::ouvrir(
            *n,
            s.port,
            &salle.to_string(),
            &fragment.ecrire(),
            &profils,
        ) {
            Ok(page) => s.pages.push(page),
            Err(e) => p.notes.push(format!("page {} : {e}", n.nom())),
        }
    }
    if s.pages.is_empty() {
        p.notes.push(String::from("pages : aucune n'a pu s'ouvrir"));
        return Ok(());
    }
    let fin = maintenant()
        .checked_add(Duration::from_secs(90))
        .ok_or("délai")?;
    loop {
        s.tourner(Duration::from_millis(300))?;
        let pretes = s.pages.iter().all(|pg| {
            pg.etat.as_ref().is_some_and(|e| {
                e.get("accueilli").and_then(Json::as_bool) == Some(true)
                    && e.get("noeuds").and_then(Json::as_u64).unwrap_or(0) > 0
            })
        });
        if pretes {
            break;
        }
        if reste(fin).is_zero() {
            for pg in &s.pages {
                p.fautes.push(format!(
                    "page {} : pas accueillie avec une page en 90 s ; état {} ; console {:?}",
                    pg.nom,
                    pg.etat
                        .as_ref()
                        .map_or_else(|| String::from("aucun"), Json::to_string),
                    pg.console.iter().rev().take(8).collect::<Vec<_>>()
                ));
            }
            return Err(String::from("les pages ne sont pas prêtes"));
        }
    }
    for pg in &s.pages {
        p.notes.push(format!(
            "page {} ({}) accueillie ; M4 {}",
            pg.nom,
            pg.version,
            pg.m4
                .as_ref()
                .map_or_else(|| String::from("non dit"), Json::to_string)
        ));
    }
    Ok(())
}

/// Les quantiles d'une liste de durées en microsecondes : (nombre, médiane, 99ᵉ centile,
/// maximum).
fn quantiles(mut v: Vec<u64>) -> (usize, u64, u64, u64) {
    v.sort_unstable();
    let q = |f: usize| {
        v.get(v.len().saturating_sub(1).saturating_mul(f) / 100)
            .copied()
            .unwrap_or(0)
    };
    (v.len(), q(50), q(99), v.last().copied().unwrap_or(0))
}

/// Les retardataires (*correction du lot 12*, « un téléphone qui rejoint Sidequest à 71
/// appareils ne retarde personne ») : quand chaque téléphone natif est accueilli et a reçu
/// le module, les pages arrivent une à une. Pour chacune, le navigateur est lancé, puis la
/// page ouverte ; pendant qu'elle reçoit le module (de son ouverture à son module entier,
/// à l'état qu'elle dit), les allers-retours d'écho des natifs (chacun toutes les 250 ms,
/// SAL-129) se comparent à ceux des 20 s d'avant le lancement : ni leur médiane ni leur
/// 99ᵉ centile ne doivent croître de plus de [`RETARD_TOLERE_US`]. Le banc tourne sur un
/// seul fil : ses propres arrêts (une évaluation dans une page) comptent dans les deux.
#[expect(
    clippy::too_many_lines,
    reason = "les retardataires se lisent d'un trait : attendre, lancer, ouvrir, mesurer, juger"
)]
fn ouvrir_les_pages_en_retard(o: &Options, s: &mut Session, p: &mut Passage) -> Result<(), String> {
    if o.pages.is_empty() {
        return Ok(());
    }
    let (Some(salle), Some(fragment)) = (s.salle, s.fragment.clone()) else {
        return Err(String::from("pages : ni salle ni fragment"));
    };
    let fin = maintenant()
        .checked_add(Duration::from_secs(300))
        .ok_or("délai")?;
    loop {
        s.tourner(Duration::from_millis(500))?;
        let (prets, natifs) = {
            let troupe = s.natifs.troupe();
            let prets = troupe
                .telephones
                .iter()
                .filter(|t| t.appareil.accueilli() && t.appareil.module().is_some())
                .count();
            (prets, troupe.telephones.len())
        };
        if prets == natifs {
            p.notes.push(format!(
                "retardataires : les {prets} téléphones natifs accueillis, leur module reçu"
            ));
            break;
        }
        if reste(fin).is_zero() {
            return Err(format!(
                "retardataires : {prets} téléphones natifs sur {natifs} accueillis avec leur module en 300 s"
            ));
        }
    }
    let profils = o.travail.join("profils-pages");
    for n in &o.pages {
        // Les 20 s d'avant, sans rien de nouveau.
        s.tourner(Duration::from_secs(20))?;
        let base_jusqu_a = heure_locale_us(s.depart);
        let lance = match Lance::lancer(*n, None, &profils) {
            Ok(l) => l,
            Err(e) => {
                p.notes
                    .push(format!("page {} : pas exécuté : {e}", n.nom()));
                continue;
            }
        };
        s.tourner(Duration::from_secs(3))?;
        let debut = heure_locale_us(s.depart);
        let page = crate::telephones::PageTel::sur(
            lance,
            *n,
            s.port,
            &salle.to_string(),
            &fragment.ecrire(),
        )?;
        s.pages.push(page);
        let k = s.pages.len().saturating_sub(1);
        let fin = maintenant()
            .checked_add(Duration::from_secs(90))
            .ok_or("délai")?;
        loop {
            s.tourner(Duration::from_millis(50))?;
            let pg = s.pages.get_mut(k).ok_or("pas de page")?;
            let e = pg.etat_de(0)?;
            let progres: Vec<u64> = e
                .get("progres")
                .and_then(Json::as_array)
                .map(|a| a.iter().filter_map(Json::as_u64).collect())
                .unwrap_or_default();
            if let [recus, total] = progres[..]
                && total > 0
                && recus >= total
                && e.get("noeuds").and_then(Json::as_u64).unwrap_or(0) > 0
            {
                pg.etat = Some(e);
                break;
            }
            if reste(fin).is_zero() {
                return Err(format!(
                    "retardataire {} : le module n'est pas reçu en 90 s ; état {e}",
                    pg.nom
                ));
            }
        }
        let fin_us = heure_locale_us(s.depart);
        let troupe = s.natifs.troupe();
        let avant: Vec<u64> = troupe
            .allers_retours
            .iter()
            .filter(|(t, _)| t.saturating_add(20_000_000) >= base_jusqu_a && *t < base_jusqu_a)
            .map(|(_, d)| *d)
            .collect();
        let pendant: Vec<u64> = troupe
            .allers_retours
            .iter()
            .filter(|(t, _)| *t >= debut && *t <= fin_us)
            .map(|(_, d)| *d)
            .collect();
        let natifs = troupe.telephones.len();
        drop(troupe);
        let (na, ma, ca, xa) = quantiles(avant);
        let (np, mp, cp, xp) = quantiles(pendant);
        let pg = s.pages.get(k).ok_or("pas de page")?;
        let texte = format!(
            "retardataire {} ({} téléphones natifs, {} pages) : module reçu en {:.0} ms ; allers-retours d'écho des natifs, 20 s avant : {na}, médiane {:.1} ms, 99ᵉ centile {:.1} ms, maximum {:.1} ms ; pendant la livraison : {np}, médiane {:.1} ms, 99ᵉ centile {:.1} ms, maximum {:.1} ms ; M4 {}",
            pg.nom,
            natifs,
            s.pages.len(),
            ms(fin_us.saturating_sub(debut)),
            ms(ma),
            ms(ca),
            ms(xa),
            ms(mp),
            ms(cp),
            ms(xp),
            pg.m4
                .as_ref()
                .or_else(|| pg.etat.as_ref().and_then(|e| e.get("m4")))
                .map_or_else(|| String::from("non dit"), Json::to_string)
        );
        if na == 0 || np == 0 {
            p.fautes
                .push(format!("{texte} : pas assez d'allers-retours pour juger"));
        } else if cp > ca.saturating_add(RETARD_TOLERE_US)
            || mp > ma.saturating_add(RETARD_TOLERE_US)
        {
            p.fautes.push(format!(
                "{texte} : le retardataire retarde les autres (médiane + {:.1} ms, 99ᵉ centile + {:.1} ms ; au plus {:.1} ms chacun)",
                ms(mp.saturating_sub(ma)),
                ms(cp.saturating_sub(ca)),
                ms(RETARD_TOLERE_US)
            ));
        } else {
            p.notes.push(texte);
        }
    }
    Ok(())
}

/// M13 (§ 8.6, *correction du lot 12*) : la part du commun dans le Worker à chaque tour qui
/// joue un pas (les vues de l'écran et des téléphones et leur chiffrement, la garde), que
/// le Worker répartit et rapporte dans ses lignes `etat`. Le Worker qui a joué le plus de
/// tours fait foi (une reprise en relance un neuf). Jugée contre [`M13_MAX_MS`] au 99ᵉ
/// centile avec les retardataires (le scénario des 71 appareils) ; notée sinon.
fn noter_m13(o: &Options, s: &Session, p: &mut Passage) {
    let tours = |l: &Ligne| {
        l.json
            .pointer("/m13/commun/tours")
            .and_then(Json::as_u64)
            .unwrap_or(0)
    };
    let Some(l) = s
        .lignes
        .iter()
        .filter(|l| l.quoi() == "etat")
        .max_by_key(|l| tours(l))
    else {
        p.notes.push(String::from("M13 : aucune ligne `etat`"));
        return;
    };
    let m = |chemin: &str| l.json.pointer(chemin).and_then(Json::as_f64).unwrap_or(0.0);
    let texte = format!(
        "M13 : {} tours, {} appareils accueillis au dernier rapport ; la part du commun (vues, chiffrement, garde) : médiane {:.1} ms, 99ᵉ centile {:.1} ms, maximum {:.2} ms ; le tour entier : médiane {:.1} ms, 99ᵉ centile {:.1} ms, maximum {:.2} ms (cases de 0,1 ms, bornes par excès)",
        tours(l),
        l.json
            .pointer("/m13/appareils")
            .and_then(Json::as_u64)
            .unwrap_or(0),
        m("/m13/commun/mediane"),
        m("/m13/commun/c99"),
        m("/m13/commun/max"),
        m("/m13/tour/mediane"),
        m("/m13/tour/c99"),
        m("/m13/tour/max"),
    );
    if o.retardataires && m("/m13/commun/c99") > M13_MAX_MS {
        p.fautes.push(format!(
            "{texte} : au-delà de {M13_MAX_MS:.2} ms au 99ᵉ centile"
        ));
    } else {
        p.notes.push(texte);
    }
}

/// La première page, rechargée en pleine écriture : aucun geste perdu (P26, SAL-56). Le
/// banc gèle le réseau de la page (`__banc.geler` : rien ne part, rien n'arrive), y tape
/// une réponse et touche un bouton (« Envoyer » s'il y est) : le geste reste dans la file,
/// jamais parti ; puis il recharge la page. La page reprend sa file de `sessionStorage`,
/// la renvoie après l'accueil, et le geste doit être accusé ; la chaîne finale n'en compte
/// aucun en double (`juger`).
fn recharger_une_page_en_pleine_ecriture(s: &mut Session, p: &mut Passage) {
    if s.pages.is_empty() {
        return;
    }
    let r = (|| -> Result<String, String> {
        let fin = maintenant()
            .checked_add(Duration::from_secs(120))
            .ok_or("délai")?;
        loop {
            s.tourner(Duration::from_millis(300))?;
            let pg = s.pages.first_mut().ok_or("pas de page")?;
            let noeuds = pg.evaluer_json(0, "window.__banc.noeuds()")?;
            let a = noeuds.as_array().cloned().unwrap_or_default();
            let sorte = |n: &Json, x: &str| n.get("sorte").and_then(Json::as_str) == Some(x);
            let champ = a.iter().find(|n| sorte(n, "champ"));
            let bouton = a.iter().filter(|n| sorte(n, "bouton")).max_by_key(|n| {
                n.get("nom")
                    .and_then(Json::as_str)
                    .is_some_and(|t| t.contains("Envoyer"))
            });
            let e = pg.etat_de(0)?;
            let ecran_du_jeu = e.get("ecran").is_some_and(Json::is_null);
            if let (Some(c), Some(b), true) = (champ, bouton, ecran_du_jeu) {
                let c = c
                    .get("cle")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let b = b
                    .get("cle")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .to_owned();
                pg.arreter_le_pilote();
                let avant = e.get("dernier_geste").and_then(Json::as_u64).unwrap_or(0);
                pg.lance.evaluer(0, "window.__banc.geler(20000)")?;
                pg.toucher(0, &c)?;
                pg.lance.taper(0, "au rechargement")?;
                pg.textes_tapes.push(String::from("au rechargement"));
                pg.toucher(0, &b)?;
                let e = pg.etat_de(0)?;
                let n = e.get("dernier_geste").and_then(Json::as_u64).unwrap_or(0);
                if n <= avant {
                    // Ce bouton n'a rien envoyé (une page qui attend autre chose) : on
                    // dégèle et on réessaie plus tard.
                    pg.lance.evaluer(0, "window.__banc.geler(0)")?;
                    pg.pilote = true;
                    if reste(fin).is_zero() {
                        return Err(String::from("page : aucun geste d'écriture en 120 s"));
                    }
                    continue;
                }
                // Lot 12, fin : ce que la page a retenu (`Locaux::retenu`) survit au
                // rechargement (`localStorage`, par salle et par jeu). *Correction de la fin
                // du lot 12* : la page rechargée doit le tenir **du stockage**
                // (`retenu_repris`, lu avant toute vue) ; une page qui le recalcule depuis la
                // vue, comme la manette de Sidequest, le retrouverait sans stockage.
                let e0 = pg.etat_de(0)?;
                let retenu = e0.get("retenu").cloned().unwrap_or(Json::Null);
                let cle_retenu = e0
                    .get("retenu_cle")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .to_owned();
                if !retenu.is_null() {
                    let garde = pg.lance.evaluer(
                        0,
                        &format!("localStorage.getItem({})", Json::from(cle_retenu.as_str())),
                    )?;
                    if garde != retenu {
                        return Err(format!(
                            "page {} : le retenu {retenu} n'est pas gardé dans localStorage sous « {cle_retenu} » (on y lit {garde})",
                            pg.nom
                        ));
                    }
                }
                let mut note = recharger_et_reprendre(s, n, &retenu)?;
                // Le contrôle négatif du retenu : le stockage vidé de sa clé, la page
                // rechargée ne doit rien reprendre ; sinon `retenu_repris` ne dirait pas le
                // stockage, et le constat d'avant ne prouverait rien.
                if !retenu.is_null() {
                    note.push_str(&recharger_sans_le_retenu(s, &cle_retenu, &retenu)?);
                }
                return Ok(note);
            }
            if reste(fin).is_zero() {
                return Err(String::from("page : aucune écriture en 120 s"));
            }
        }
    })();
    match r {
        Ok(n) => p.notes.push(n),
        Err(e) => p.fautes.push(e),
    }
    if let Some(pg) = s.pages.first_mut() {
        pg.pilote = true;
    }
}

/// La première page rechargée en pleine écriture : le geste `n`, jamais parti, doit être
/// accusé après le rechargement, et ce qu'elle avait retenu (`retenu`, s'il y en a un)
/// repris de `localStorage` avant toute vue. Rend ce qu'il a constaté.
fn recharger_et_reprendre(s: &mut Session, n: u64, retenu: &Json) -> Result<String, String> {
    let pg = s.pages.first_mut().ok_or("pas de page")?;
    pg.lance.recharger(0)?;
    let fin = maintenant()
        .checked_add(Duration::from_secs(60))
        .ok_or("délai")?;
    loop {
        s.tourner(Duration::from_millis(300))?;
        let pg = s.pages.first_mut().ok_or("pas de page")?;
        if let Ok(e) = pg.etat_de(0) {
            let accuse = e.get("accuse").and_then(Json::as_u64).unwrap_or(0);
            let repris = e.get("retenu_repris").cloned().unwrap_or(Json::Null);
            if e.get("accueilli").and_then(Json::as_bool) == Some(true)
                && accuse >= n
                && (retenu.is_null() || repris == *retenu)
            {
                let r = if retenu.is_null() {
                    String::from("rien de retenu avant (sans objet pour ce jeu)")
                } else {
                    format!(
                        "ce que la page avait retenu ({retenu}), repris de localStorage avant toute vue"
                    )
                };
                return Ok(format!(
                    "page {} rechargée en pleine écriture, réseau gelé : le geste {n}, jamais parti avant le rechargement, est accusé après lui (accusé {accuse}) ; {r}",
                    pg.nom
                ));
            }
        }
        if reste(fin).is_zero() {
            let e = pg.etat_de(0).unwrap_or(Json::Null);
            return Err(format!(
                "page rechargée : en 60 s, le geste {n} n'est pas accusé (geste perdu), ou le retenu {retenu} n'est pas repris de localStorage ; état {e}"
            ));
        }
    }
}

/// Le contrôle négatif du retenu (correction de la fin du lot 12) : la clé du retenu ôtée de
/// `localStorage`, la première page rechargée ne doit rien en reprendre (`retenu_repris`
/// nul), même si sa vue lui redonne le même retenu. Rend ce qu'il a constaté.
fn recharger_sans_le_retenu(
    s: &mut Session,
    cle_retenu: &str,
    retenu: &Json,
) -> Result<String, String> {
    let pg = s.pages.first_mut().ok_or("pas de page")?;
    pg.lance.evaluer(
        0,
        &format!(
            "localStorage.removeItem({}) === undefined",
            Json::from(cle_retenu)
        ),
    )?;
    pg.lance.recharger(0)?;
    let fin = maintenant()
        .checked_add(Duration::from_secs(60))
        .ok_or("délai")?;
    loop {
        s.tourner(Duration::from_millis(300))?;
        let pg = s.pages.first_mut().ok_or("pas de page")?;
        if let Ok(e) = pg.etat_de(0)
            && e.get("accueilli").and_then(Json::as_bool) == Some(true)
            && e.get("noeuds").and_then(Json::as_u64).unwrap_or(0) > 0
        {
            let repris = e.get("retenu_repris").cloned().unwrap_or(Json::Null);
            if !repris.is_null() {
                return Err(format!(
                    "CONTRÔLE NÉGATIF RATÉ : page {}, sa clé ôtée de localStorage, rechargée, dit encore avoir repris {repris}",
                    pg.nom
                ));
            }
            let apres = e.get("retenu").cloned().unwrap_or(Json::Null);
            return Ok(format!(
                " ; contrôle négatif : la clé ôtée de localStorage, la page rechargée n'a rien repris{}",
                if apres == *retenu {
                    " (sa vue lui redonne pourtant le même retenu : c'est le stockage qui se juge)"
                } else {
                    ""
                }
            ));
        }
        if reste(fin).is_zero() {
            let e = pg.etat_de(0).unwrap_or(Json::Null);
            return Err(format!(
                "page rechargée sans son retenu : pas revenue en 60 s ; état {e}"
            ));
        }
    }
}

/// Un onglet jumeau de la dernière page (même jeton, un autre onglet) : le plus récent
/// gagne, l'ancien montre « Remplacé — Reprendre ici » (SAL-18, SAL-19, SAL-21) ; toucher
/// « Reprendre ici » le rend à l'ancien, et le jumeau est remplacé à son tour.
fn onglet_jumeau(s: &mut Session, p: &mut Passage) {
    if s.pages.is_empty() {
        return;
    }
    let r = (|| -> Result<String, String> {
        let k = s.pages.len().saturating_sub(1);
        let url = s
            .pages
            .get(k)
            .map(|pg| pg.url.clone())
            .ok_or("pas de page")?;
        let jumeau = {
            let pg = s.pages.get_mut(k).ok_or("pas de page")?;
            pg.arreter_le_pilote();
            let j = pg.lance.nouvel_onglet(&url)?;
            pg.onglets.push(j);
            j
        };
        let attendre =
            |s: &mut Session, quoi: &str, f: &dyn Fn(&Json, &Json) -> bool| -> Result<(), String> {
                let fin = maintenant()
                    .checked_add(Duration::from_secs(45))
                    .ok_or("délai")?;
                loop {
                    s.tourner(Duration::from_millis(300))?;
                    let pg = s.pages.get_mut(k).ok_or("pas de page")?;
                    let (Ok(a), Ok(b)) = (pg.etat_de(0), pg.etat_de(1)) else {
                        continue;
                    };
                    if f(&a, &b) {
                        return Ok(());
                    }
                    if reste(fin).is_zero() {
                        return Err(format!(
                            "onglet jumeau : {quoi} en 45 s ; premier {a}, jumeau {b}"
                        ));
                    }
                }
            };
        let remplace = |e: &Json| e.get("ecran").and_then(Json::as_str) == Some("remplace");
        let accueilli = |e: &Json| e.get("accueilli").and_then(Json::as_bool) == Some(true);
        attendre(s, "le jumeau ne prend pas la place", &|a, b| {
            remplace(a) && accueilli(b)
        })?;
        let visible = {
            let pg = s.pages.get_mut(k).ok_or("pas de page")?;
            pg.lance.montrer(0)?;
            let v = pg.lance.evaluer(0, "document.visibilityState")?;
            pg.toucher(0, "reprendre")?;
            v
        };
        if attendre(s, "« Reprendre ici » touché", &|a, b| {
            accueilli(a) && remplace(b)
        })
        .is_err()
        {
            // Un doigt n'a rien fait (l'onglet ramené devant était « {visible} ») : un clic de
            // souris, par la même chaîne d'entrées.
            let pg = s.pages.get_mut(k).ok_or("pas de page")?;
            let c = pg.evaluer_json(0, "window.__banc.centre('reprendre')")?;
            let (x, y) = (
                c.get("x").and_then(Json::as_f64).unwrap_or(0.0),
                c.get("y").and_then(Json::as_f64).unwrap_or(0.0),
            );
            pg.lance.cliquer(0, x, y)?;
            p.notes.push(format!(
                "onglet jumeau : le doigt sur « Reprendre ici » n'a rien fait dans {} (onglet {visible}) ; un clic de souris",
                pg.nom
            ));
            attendre(s, "« Reprendre ici » ne rend pas la place", &|a, b| {
                accueilli(a) && remplace(b)
            })?;
        }
        let pg = s.pages.get_mut(k).ok_or("pas de page")?;
        pg.lance.fermer_onglet(jumeau)?;
        pg.onglets.truncate(1);
        pg.actif = 0;
        pg.pilote = true;
        Ok(format!(
            "page {} : onglet jumeau remplacé puis « Reprendre ici » (SAL-19, SAL-21)",
            pg.nom
        ))
    })();
    match r {
        Ok(n) => p.notes.push(n),
        Err(e) => p.fautes.push(e),
    }
    if let Some(pg) = s.pages.last_mut() {
        pg.pilote = true;
    }
}

/// P28 : une page qui tient la croix d'un jeu dont le tenu déclare une fraîcheur
/// (Sidequest) perd son réseau 2,5 s (`__banc.geler`, en mode banc : ce qui arrive se perd,
/// rien ne part) ; l'écho reste sans réponse 1,5 s, la page relâche de son côté (son compte
/// `coupures`), reconnecte, et le doigt resté posé ne compte plus. Sans croix (Punchline),
/// rien à couper : c'est dit.
fn couper_une_page_qui_tient_la_croix(s: &mut Session, p: &mut Passage) {
    if s.pages.is_empty() {
        return;
    }
    let r = (|| -> Result<Option<String>, String> {
        let fin = maintenant()
            .checked_add(Duration::from_secs(60))
            .ok_or("délai")?;
        loop {
            s.tourner(Duration::from_millis(300))?;
            let tenue = s.pages.iter().position(|pg| {
                pg.croix
                    .is_some_and(|(t, _, _)| t.elapsed() >= Duration::from_secs(1))
            });
            if let Some(k) = tenue {
                let pg = s.pages.get_mut(k).ok_or("pas de page")?;
                let avant = pg
                    .etat_de(0)?
                    .get("coupures")
                    .and_then(Json::as_u64)
                    .unwrap_or(0);
                let o = pg.onglets.first().copied().unwrap_or(0);
                pg.lance.evaluer(o, "window.__banc.geler(2500)")?;
                s.tourner(Duration::from_secs(6))?;
                let pg = s.pages.get_mut(k).ok_or("pas de page")?;
                let e = pg.etat_de(0)?;
                let apres = e.get("coupures").and_then(Json::as_u64).unwrap_or(0);
                if apres <= avant {
                    return Err(format!(
                        "page {} : réseau gelé 2,5 s en tenant la croix, aucun relâchement côté page (P28) ; état {e}",
                        pg.nom
                    ));
                }
                if e.get("accueilli").and_then(Json::as_bool) != Some(true) {
                    return Err(format!(
                        "page {} : pas revenue après la coupure ; état {e}",
                        pg.nom
                    ));
                }
                return Ok(Some(format!(
                    "page {} : coupure en tenant la croix, relâchement côté page (P28 : coupures {avant} → {apres}), revenue",
                    pg.nom
                )));
            }
            if reste(fin).is_zero() {
                return Ok(None);
            }
        }
    })();
    match r {
        Ok(Some(n)) => p.notes.push(n),
        Ok(None) => p.notes.push(String::from(
            "coupure (P28) : aucune page n'a tenu de croix en 60 s (un jeu sans croix)",
        )),
        Err(e) => p.fautes.push(e),
    }
}

/// SAL-07 (*correction du lot 12* : au lot 12, `FilePleine` n'était que compté, jamais
/// provoqué) : la première page, son réseau gelé (`__banc.geler`), reçoit autant de touchers
/// d'un bouton que sa file tient de gestes, et huit de plus. Chaque toucher est un geste
/// tenté : placé en file tant qu'il y a de la place, sinon refusé et compté
/// (`files_pleines`), et dit à l'écran (« Trop de gestes en attente »). Le compte doit être
/// exact : placés + refusés = touchers, la file à sa limite. Puis le réseau revient, et
/// chaque geste placé doit être accusé (aucun perdu ; la chaîne finale n'en compte aucun en
/// double, `juger`).
#[expect(
    clippy::too_many_lines,
    reason = "la file remplie, comptée, puis vidée : un constat, d'un trait"
)]
fn remplir_la_file_d_une_page(s: &mut Session, p: &mut Passage) {
    if s.pages.is_empty() {
        return;
    }
    let r = (|| -> Result<String, String> {
        let fin = maintenant()
            .checked_add(Duration::from_secs(120))
            .ok_or("délai")?;
        loop {
            s.tourner(Duration::from_millis(300))?;
            if reste(fin).is_zero() {
                return Err(String::from(
                    "file pleine : aucun bouton sur une page du jeu, file vide, en 120 s",
                ));
            }
            let pg = s.pages.first_mut().ok_or("pas de page")?;
            if pg.croix.is_some() {
                continue;
            }
            let o = pg.onglets.first().copied().unwrap_or(0);
            let e = pg.etat_de(0)?;
            let lire = |e: &Json, c: &str| e.get(c).and_then(Json::as_u64).unwrap_or(0);
            let noeuds = pg.evaluer_json(o, "window.__banc.noeuds()")?;
            let boutons: Vec<String> = noeuds
                .as_array()
                .into_iter()
                .flatten()
                .filter(|n| n.get("sorte").and_then(Json::as_str) == Some("bouton"))
                .filter_map(|n| n.get("cle").and_then(Json::as_str))
                .map(str::to_owned)
                .collect();
            let pret = e.get("ecran").is_some_and(Json::is_null)
                && e.get("accueilli").and_then(Json::as_bool) == Some(true)
                && lire(&e, "en_file") == 0
                && lire(&e, "file_max") > 0;
            if !pret || boutons.is_empty() {
                continue;
            }
            pg.arreter_le_pilote();
            let limite = lire(&e, "file_max");
            let touchers = limite.saturating_add(8);
            // Ce que le doigt touche, pour dire pourquoi un bouton ne répond pas.
            let _: Json = pg.lance.evaluer(
                o,
                "window.__banc_doigt = []; for (const n of ['pointerdown', 'touchstart', 'click']) document.addEventListener(n, e => window.__banc_doigt.push(n + ' ' + (e.pointerType || '') + ' ' + ((e.target && e.target.outerHTML) || '').slice(0, 120)), true); true",
            )?;
            // Le réseau gelé d'abord : aucune vue n'arrive, la page ne change pas sous le
            // doigt (un geste change souvent la page qui le porte : le personnage choisi
            // dans le salon de Punchline, le vote envoyé). Le premier toucher qui place un
            // geste désigne le bouton ; un bouton qui ne répond pas au doigt se dit.
            pg.lance.evaluer(o, "window.__banc.geler(60000)")?;
            let mut bouton = None;
            for b in &boutons {
                let avant = lire(&pg.etat_de(0)?, "gestes");
                let _: Json = pg.lance.evaluer(o, "window.__banc_doigt = []; true")?;
                pg.toucher(o, b)?;
                if lire(&pg.etat_de(0)?, "gestes") > avant {
                    bouton = Some(b.clone());
                    break;
                }
                let doigt = pg
                    .lance
                    .evaluer(o, "JSON.stringify(window.__banc_doigt)")
                    .unwrap_or(Json::Null);
                p.notes.push(format!(
                    "file pleine : le bouton « {b} » de la page {} n'a émis aucun geste au toucher ({:?}) ; le doigt a touché : {doigt}",
                    pg.nom,
                    noeuds
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|n| n.get("cle").and_then(Json::as_str) == Some(b.as_str()))
                ));
            }
            let Some(bouton) = bouton else {
                pg.lance.evaluer(o, "window.__banc.geler(0)")?;
                pg.pilote = true;
                continue;
            };
            let mut faits = 1u64;
            for _ in 1..touchers {
                if pg.toucher(o, &bouton).is_err() {
                    break;
                }
                faits = faits.saturating_add(1);
            }
            let e1 = pg.etat_de(0)?;
            let places = lire(&e1, "gestes").saturating_sub(lire(&e, "gestes"));
            let refuses = lire(&e1, "files_pleines").saturating_sub(lire(&e, "files_pleines"));
            let en_file = lire(&e1, "en_file");
            let bandeau = pg
                .lance
                .evaluer(
                    o,
                    "document.body.innerText.includes('Trop de gestes en attente')",
                )
                .ok()
                .and_then(|v| v.as_bool())
                == Some(true);
            if faits < touchers {
                // Le bouton a disparu sous le doigt (une page qui change) : on dégèle, on
                // attend que tout soit accusé, et on réessaie plus tard.
                pg.lance.evaluer(o, "window.__banc.geler(0)")?;
                pg.pilote = true;
                continue;
            }
            let compte = format!(
                "{faits} touchers de « {bouton} », file de {limite} : {places} placés, {refuses} refusés, {en_file} en file, bandeau {}",
                if bandeau { "dit" } else { "non dit" }
            );
            pg.lance.evaluer(o, "window.__banc.geler(0)")?;
            if refuses == 0
                || places.saturating_add(refuses) != faits
                || en_file != limite
                || !bandeau
            {
                pg.pilote = true;
                return Err(format!(
                    "file pleine : le compte n'est pas exact : {compte}"
                ));
            }
            let dernier = lire(&e1, "dernier_geste");
            let fin2 = maintenant()
                .checked_add(Duration::from_secs(60))
                .ok_or("délai")?;
            loop {
                s.tourner(Duration::from_millis(300))?;
                let pg = s.pages.first_mut().ok_or("pas de page")?;
                if let Ok(e2) = pg.etat_de(0)
                    && lire(&e2, "accuse") >= dernier
                    && lire(&e2, "en_file") == 0
                {
                    pg.pilote = true;
                    return Ok(format!(
                        "page {} : file pleine provoquée, réseau gelé (SAL-07) : {compte} ; chaque geste placé accusé après le dégel (jusqu'au {dernier})",
                        pg.nom
                    ));
                }
                if reste(fin2).is_zero() {
                    return Err(format!(
                        "file pleine : après le dégel, les gestes jusqu'au {dernier} ne sont pas accusés en 60 s ({compte})"
                    ));
                }
            }
        }
    })();
    match r {
        Ok(n) => p.notes.push(n),
        Err(e) => p.fautes.push(e),
    }
    if let Some(pg) = s.pages.first_mut() {
        pg.pilote = true;
    }
}

/// Ce que les pages ont fait : chacune a joué (des gestes émis), dit M4, sans erreur du
/// pilote ni de sa console ; et le relais n'a vu aucun secret (le secret d'entrée, G, les
/// jetons des téléphones natifs et des pages, les textes tapés).
#[expect(clippy::too_many_lines, reason = "un jugement par constat, à la suite")]
fn juger_les_pages(s: &mut Session, p: &mut Passage, g: Option<[u8; 32]>) {
    if s.pages.is_empty() {
        return;
    }
    let mut aiguilles: Vec<(String, Vec<u8>)> = Vec::new();
    for pg in &s.pages {
        if pg.gestes_max == 0 {
            p.fautes.push(format!("page {} : aucun geste émis", pg.nom));
        }
        if pg.m4.is_none() {
            p.fautes.push(format!("page {} : M4 non dit", pg.nom));
        }
        if !pg.erreurs.is_empty() {
            p.notes.push(format!(
                "page {} : le pilote a buté {} fois : {:?}",
                pg.nom,
                pg.erreurs.len(),
                pg.erreurs.iter().take(4).collect::<Vec<_>>()
            ));
        }
        let erreurs: Vec<&String> = pg
            .console
            .iter()
            .filter(|l| l.contains("error") || l.contains("exception") || l.contains("panicked"))
            // Le navigateur demande `/favicon.ico` de lui-même ; le relais n'en sert pas.
            .filter(|l| !l.contains("/favicon.ico"))
            .collect();
        if !erreurs.is_empty() {
            p.fautes.push(format!(
                "page {} : la console dit des erreurs : {:?}",
                pg.nom,
                erreurs.iter().take(6).collect::<Vec<_>>()
            ));
        }
        p.notes.push(format!(
            "page {} : {} gestes au plus ; dernier état {}",
            pg.nom,
            pg.gestes_max,
            pg.etat
                .as_ref()
                .map_or_else(|| String::from("aucun"), Json::to_string)
        ));
        for t in &pg.textes_tapes {
            // Correction du lot 12 : quatre octets se trouvent au hasard dans un
            // enregistrement chiffré de 62 Mo une fois sur 70 (62·10⁶ / 2³²), et une soirée
            // de cinq minutes en tape plusieurs (« Hugo » y a été vu une fois, au milieu
            // d'octets chiffrés). Un texte de moins de 8 octets se cherche donc tel qu'un
            // message le porterait, précédé de sa longueur postcard (5 octets au moins : une
            // fois sur 18 000 au hasard) ; un plus long, tel quel.
            if t.len() >= 8 {
                aiguilles.push((format!("le texte tapé « {t} »"), t.as_bytes().to_vec()));
            } else if let Ok(n) = u8::try_from(t.len())
                && n >= 4
            {
                let mut a = vec![n];
                a.extend_from_slice(t.as_bytes());
                aiguilles.push((format!("le texte tapé « {t} », précédé de sa longueur"), a));
            }
        }
    }
    if let Some(f) = s.fragment.as_ref() {
        aiguilles.push((
            String::from("le secret d'entrée"),
            f.secret.octets().to_vec(),
        ));
    }
    if let Some(g) = g {
        aiguilles.push((String::from("G"), g.to_vec()));
    }
    for (k, t) in s.natifs.troupe().telephones.iter().enumerate() {
        aiguilles.push((format!("le jeton du téléphone natif {k}"), t.jeton.to_vec()));
    }
    // Les jetons des pages, lus dans leur `localStorage` (correction du lot 12) : en octets,
    // et tels que la page les écrit (en hexadécimal).
    for pg in &mut s.pages {
        let o = pg.onglets.get(pg.actif).copied().unwrap_or(0);
        let lu = pg
            .lance
            .evaluer(o, "localStorage.getItem('soiree-jeton') || ''");
        match lu
            .as_ref()
            .ok()
            .and_then(Json::as_str)
            .filter(|h| h.len() == 64)
        {
            Some(h) => match crate::telephones::octets_de_l_hexa(h) {
                Some(j) => {
                    aiguilles.push((format!("le jeton de la page {}", pg.nom), j));
                    aiguilles.push((
                        format!("le jeton de la page {} en hexadécimal", pg.nom),
                        h.as_bytes().to_vec(),
                    ));
                }
                None => p
                    .fautes
                    .push(format!("page {} : jeton illisible « {h} »", pg.nom)),
            },
            None => p.fautes.push(format!(
                "page {} : le jeton de `localStorage` ne se lit pas ({lu:?})",
                pg.nom
            )),
        }
    }
    // Les textes courts tels quels : un constat, pas un jugement (le hasard les y met).
    let courts: Vec<(String, Vec<u8>)> = s
        .pages
        .iter()
        .flat_map(|pg| pg.textes_tapes.iter())
        .filter(|t| t.len() < 8)
        .map(|t| (format!("« {t} »"), t.as_bytes().to_vec()))
        .collect();
    if let Ok(v) = crate::telephones::secrets_vus(&s.enregistrement, &courts)
        && !v.is_empty()
    {
        let taille = std::fs::metadata(&s.enregistrement).map_or(0, |m| m.len());
        p.notes.push(format!(
            "secrets : des textes courts tapés se trouvent tels quels dans l'enregistrement ({} octets), sans leur longueur : {v:?} (au hasard, un texte de 4 octets s'y trouve avec une probabilité d'environ {:.1} %)",
            taille,
            // Un constat : la probabilité, en pour cent, qu'une suite de 4 octets donnée
            // apparaisse au hasard dans autant d'octets.
            f64::from(u32::try_from(taille / 1000).unwrap_or(u32::MAX)) * 1000.0 / 4_294_967_296.0
                * 100.0
        ));
    }
    match crate::telephones::secrets_vus(&s.enregistrement, &aiguilles) {
        Ok(v) if v.is_empty() => p.notes.push(format!(
            "le relais n'a vu aucun des {} secrets cherchés dans son enregistrement",
            aiguilles.len()
        )),
        Ok(v) => p.fautes.push(format!("le relais a vu : {v:?}")),
        Err(e) => p.fautes.push(e),
    }
    // Le témoin positif : des octets qu'une page a envoyés, que la même recherche doit voir.
    let liens: std::collections::BTreeSet<u32> = s
        .pages
        .iter()
        .flat_map(|pg| pg.liens.iter().copied())
        .collect();
    match crate::telephones::temoin_des_pages(&s.enregistrement, &liens) {
        Ok(Some(t)) => match crate::telephones::secrets_vus(
            &s.enregistrement,
            &[(String::from("témoin"), t)],
        ) {
            Ok(v) if v.len() == 1 => p.notes.push(String::from(
                "secrets : le témoin positif (24 octets d'une trame d'une page) est vu",
            )),
            Ok(_) => p.fautes.push(String::from(
                "secrets : TÉMOIN POSITIF RATÉ, la recherche ne voit pas ce qu'une page a envoyé",
            )),
            Err(e) => p.fautes.push(e),
        },
        Ok(None) => p.fautes.push(String::from(
            "secrets : aucune trame d'une page dans l'enregistrement, pas de témoin positif",
        )),
        Err(e) => p.fautes.push(e),
    }
    // Le scénario `pages` : les débordements vus, à chaque taille ; puis son contrôle
    // négatif, un texte qui déborde ajouté par la page (`__banc.deborder`).
    for pg in &s.pages {
        p.notes.push(format!(
            "pages {} : {} pages jugées (taille × suite de clés)",
            pg.nom,
            pg.pages_jugees.len()
        ));
        for d in &pg.debordements {
            p.fautes.push(format!("débordement : {d}"));
        }
    }
    if let Some(pg) = s.pages.first_mut() {
        let o = pg.onglets.first().copied().unwrap_or(0);
        let vu = pg
            .lance
            .evaluer(o, "window.__banc.deborder()")
            .and_then(|_| pg.evaluer_json(o, "window.__banc.debordements()"));
        match vu {
            Ok(d) if d.to_string().contains("00000000deb0deb0") => {
                p.notes.push(String::from(
                    "pages : le contrôle négatif (un texte qui déborde) est vu",
                ));
            }
            Ok(d) => p.fautes.push(format!(
                "pages : CONTRÔLE NÉGATIF RATÉ, le texte qui déborde n'est pas vu ({d})"
            )),
            Err(e) => p.fautes.push(format!("pages : contrôle négatif : {e}")),
        }
    }
    // M7 : la redite du tenu par la vraie page, telle que le relais l'a reçue : les écarts
    // entre deux trames de tenu d'un lien de la page, pendant que le pilote tenait la croix
    // (passé ses 300 premières ms, où le tenu change), à l'horloge du relais.
    match crate::telephones::trames_de_tenu(&s.enregistrement) {
        Ok(r) => {
            for pg in &s.pages {
                let fenetres: Vec<(u64, u64)> = pg
                    .tenues
                    .iter()
                    .map(|(a, b)| {
                        let us = |t: &Instant| {
                            u64::try_from(t.saturating_duration_since(s.relais_lance).as_micros())
                                .unwrap_or(u64::MAX)
                                .saturating_add(s.relais_us0)
                        };
                        (us(a).saturating_add(300_000), us(b))
                    })
                    .collect();
                let mut e: Vec<u64> = Vec::new();
                for l in &pg.liens {
                    let Some(t) = r.get(l) else {
                        continue;
                    };
                    for w in t.windows(2) {
                        if let [a, b] = w
                            && fenetres.iter().any(|(de, jusqu)| a >= de && b <= jusqu)
                        {
                            e.push(b.saturating_sub(*a));
                        }
                    }
                }
                if e.is_empty() {
                    // Correction du lot 12 : « aucune croix tenue » ne se dit que s'il n'y
                    // en a eu aucune ; des croix tenues sans trame de tenu se disent telles.
                    p.notes.push(if pg.tenues.is_empty() {
                        format!("M7, page {} : aucune croix tenue, rien à lire", pg.nom)
                    } else {
                        format!(
                            "M7, page {} : {} croix tenues, mais aucun écart entre deux trames de tenu reçues pendant elles (liens {:?})",
                            pg.nom,
                            pg.tenues.len(),
                            pg.liens
                        )
                    });
                    continue;
                }
                e.sort_unstable();
                let q = |f: usize| {
                    e.get(e.len().saturating_sub(1).saturating_mul(f) / 100)
                        .copied()
                        .unwrap_or(0)
                };
                p.notes.push(format!(
                    "M7, page {} ({}) : {} écarts entre deux tenus reçus par le relais pendant {} croix tenues ; minimum {:.1} ms, médiane {:.1} ms, 99ᵉ centile {:.1} ms, maximum {:.1} ms",
                    pg.nom,
                    pg.version,
                    e.len(),
                    pg.tenues.len(),
                    ms(e.first().copied().unwrap_or(0)),
                    ms(q(50)),
                    ms(q(99)),
                    ms(e.last().copied().unwrap_or(0))
                ));
                // Correction du lot 12 : les mêmes écarts, sur les seuls moments où la page
                // disait, à deux coups de suite du pilote, un tenu qui n'est pas sa valeur
                // relâchée (un tenu relâché part une fois et ne se redit pas : les commandes
                // désactivées de Sidequest le relâchent sous un doigt resté posé).
                let us = |t: &Instant| {
                    u64::try_from(t.saturating_duration_since(s.relais_lance).as_micros())
                        .unwrap_or(u64::MAX)
                        .saturating_add(s.relais_us0)
                };
                let actifs: Vec<(u64, u64)> = pg
                    .tenus_vus
                    .windows(2)
                    .filter_map(|w| match w {
                        [(a, false), (b, false)] => Some((us(a), us(b))),
                        _ => None,
                    })
                    .collect();
                let mut ea: Vec<u64> = Vec::new();
                for l in &pg.liens {
                    let Some(t) = r.get(l) else {
                        continue;
                    };
                    for w in t.windows(2) {
                        if let [a, b] = w
                            && actifs.iter().any(|(de, jusqu)| a >= de && b <= jusqu)
                        {
                            ea.push(b.saturating_sub(*a));
                        }
                    }
                }
                ea.sort_unstable();
                let relaches = pg.tenus_vus.iter().filter(|(_, r)| *r).count();
                let qa = |f: usize| {
                    ea.get(ea.len().saturating_sub(1).saturating_mul(f) / 100)
                        .copied()
                        .unwrap_or(0)
                };
                p.notes.push(format!(
                    "M7, page {}, tenu non relâché seulement : {} écarts ({} coups du pilote sous la croix, dont {relaches} où le tenu était relâché) ; médiane {:.1} ms, 99ᵉ centile {:.1} ms, maximum {:.1} ms",
                    pg.nom,
                    ea.len(),
                    pg.tenus_vus.len(),
                    ms(qa(50)),
                    ms(qa(99)),
                    ms(ea.last().copied().unwrap_or(0))
                ));
            }
        }
        Err(e) => p.notes.push(format!("M7 : {e}")),
    }
}

/// Les téléphones natifs, qui passent par la page depuis le lot 12 : ce que chacun a fait
/// des coups du bot, et, *depuis la correction du lot 12*, une faute pour chaque téléphone
/// qui n'a envoyé aucun geste par la page de toute la soirée (au lot 12, cinq téléphones
/// sur six de Sidequest n'en envoyaient aucun, leur `Rejoindre` sans chemin, sans que le
/// scénario le voie).
pub(crate) fn juger_les_natifs(s: &Session, p: &mut Passage) {
    let troupe = s.natifs.troupe();
    juger_les_silences(&troupe, p);
    if troupe.telephones.is_empty() {
        return;
    }
    let natifs: Vec<String> = troupe
        .telephones
        .iter()
        .enumerate()
        .map(|(k, t)| {
            format!(
                "{k}: {:?}, module {:?}, accueilli {}, panne {:?}",
                t.joueur.comptes,
                t.appareil.progres_du_module(),
                t.appareil.accueilli(),
                t.joueur.panne
            )
        })
        .collect();
    p.notes.push(format!(
        "téléphones natifs par la page : {}",
        natifs.join(" ; ")
    ));
    for (k, t) in troupe.telephones.iter().enumerate() {
        let c = &t.joueur.comptes;
        if c.gestes_par_la_page == 0 && c.gestes_du_singe == 0 {
            p.fautes.push(format!(
                "téléphone natif {k} : aucun geste envoyé par la page ({c:?})"
            ));
        }
    }
}

/// Les silences des téléphones natifs (lot 12, fin) : notés au-delà de [`SILENCE_NOTE`],
/// une faute au-delà de [`SILENCE_MAX`] : un natif que le banc ne sert plus est perdu par
/// l'autorité, et ce que le scénario mesure (les échos, M13) ne mesure plus la salle.
fn juger_les_silences(troupe: &Troupe, p: &mut Passage) {
    let plus_long = troupe.silences.iter().map(|(_, d)| *d).max().unwrap_or(0);
    p.notes.push(format!(
        "fil des natifs : {} tours, {} silences de plus de {} ms, le plus long {plus_long} ms",
        troupe.tours,
        troupe.silences.len(),
        SILENCE_NOTE.as_millis()
    ));
    let max = u64::try_from(SILENCE_MAX.as_millis()).unwrap_or(u64::MAX);
    for (debut, duree) in troupe.silences.iter().filter(|(_, d)| *d > max) {
        p.fautes.push(format!(
            "les téléphones natifs sont restés {duree} ms sans être servis (à {debut} ms) : au-delà de {max} ms, l'autorité peut les perdre"
        ));
    }
}

pub(crate) fn ecrire_les_lignes(dossier: &Path, lignes: &[Ligne], autres: &[String]) {
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
    let mut s = ouvrir(o, n, paquet, module, &dossier, (redite, ""))?;
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
    let mode = match o.cacher {
        Cacher::Onglet => String::new(),
        autre @ (Cacher::Recouverte | Cacher::Reduite) => format!("-{}", autre.nom()),
    };
    let dossier = if o.fois > 1 {
        o.travail.join(format!(
            "arriere-plan{mode}-{}-{}",
            n.nom(),
            passe.saturating_add(1)
        ))
    } else {
        o.travail.join(format!("arriere-plan{mode}-{}", n.nom()))
    };
    let mut s = ouvrir(o, n, paquet, module, &dossier, (redite, ""))?;
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
                    let r = charge.declaration().rattrapage;
                    juger_les_sauts_caches(&s, &mut p, (&chaine, &g, salle, r), f);
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
    if o.cacher != Cacher::Onglet {
        // M6 (lot 13b) : la page avant de cacher, et la session de l'écran (verrouillée,
        // macOS peut tenir chaque fenêtre pour cachée).
        let v = s.evaluer(0, "document.visibilityState")?;
        p.notes
            .push(format!("M6 : document.visibilityState vaut {v} avant"));
        p.notes.push(crate::navigateur::note_de_la_session());
    }
    let cache = maintenant();
    let cache_ms = cache.saturating_duration_since(s.depart).as_millis();
    match o.cacher {
        Cacher::Onglet => s.navigateur.cacher()?,
        Cacher::Recouverte => s.navigateur.recouvrir()?,
        // Lot 13b : réduite, l'état de la fenêtre est relu, et une fenêtre qui ne l'est
        // pas fait échouer le passage (l'erreur remonte).
        Cacher::Reduite => {
            let fait = s.navigateur.reduire(0)?;
            p.notes.push(format!("M6 : réduite : {fait}"));
        }
    }
    p.notes.push(format!(
        "à l'arrière-plan par : {}{}",
        o.cacher.nom(),
        if o.interface == crate::navigateur::Interface::Avec {
            ", avec la fenêtre du navigateur"
        } else {
            ", sans interface"
        }
    ));
    s.tourner(Duration::from_secs(2))?;
    // L'onglet est vraiment caché : la page le dit, et (plus bas) aucun pas ne s'y
    // joue au signal d'une image.
    constater_cache(s, p, "au début", o.cacher)?;
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
    constater_cache(s, p, "à la fin", o.cacher)?;
    let pas_apres = s
        .etat(onglet)
        .and_then(|e| e.get("pas"))
        .and_then(Json::as_u64)
        .unwrap_or(0);
    let duree = maintenant().saturating_duration_since(cache);
    let montre_ms = maintenant().saturating_duration_since(s.depart).as_millis();
    if o.cacher == Cacher::Reduite {
        let fait = s.navigateur.restaurer(0)?;
        p.notes.push(format!("M6 : rendue : {fait}"));
    }
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
    juger_la_fenetre(
        s,
        p,
        (onglet, o.cacher),
        (cache_ms.saturating_add(2_000), montre_ms),
    );
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
    (chaine, g, salle, rattrapage): (&Chaine, &CleDeGarde, CodeSalle, u8),
    (de, a): (u64, u64),
) {
    let v = match sauts(chaine, g, salle) {
        Ok(v) => v,
        Err(e) => return p.fautes.push(e),
    };
    let (rattaches, seuls) = rattacher_les_sauts(&v, &s.lignes, rattrapage);
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
fn constater_cache(
    s: &mut Session,
    p: &mut Passage,
    quand: &str,
    cacher: Cacher,
) -> Result<(), String> {
    let v = s.evaluer(0, "document.visibilityState")?;
    // M6 (lot 13) : une fenêtre recouverte ou réduite, ce que le navigateur en dit est
    // ce qu'on mesure, pas une faute du banc.
    if cacher != Cacher::Onglet {
        p.notes
            .push(format!("M6 : document.visibilityState vaut {v} {quand}"));
    } else if v.as_str() != Some("hidden") {
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
fn juger_la_fenetre(
    s: &Session,
    p: &mut Passage,
    (onglet, cacher): (&str, Cacher),
    (de_ms, a_ms): (u128, u128),
) {
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
            if cacher != Cacher::Onglet {
                // M6 : ce que la fenêtre recouverte ou réduite reçoit d'images se mesure.
            } else if images.saturating_mul(100) > images.saturating_add(minuterie)
                || minuterie == 0
            {
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
/// rattrape l'horloge d'un saut au premier tour, après avoir joué au plus `rattrapage` pas,
/// celui du jeu ; `de` va de `p` à `p + rattrapage`), ou un **retard injecté** par le banc
/// (une ligne « retarde » au pas `p` : le Worker gelé 150 ms saute dans les pas qui
/// suivent, `de` de `p` à `p + rattrapage + 1`). Le cœur joue `rattrapage` pas (au moins
/// un) avant de sauter (`partie::regles::cible`) : Punchline et Tier list en rattrapent 1,
/// Sidequest 3. Rend les sauts rattachés, avec leur raison, et ceux que rien n'explique.
fn rattacher_les_sauts(
    sauts: &[SautDuJournal],
    lignes: &[Ligne],
    rattrapage: u8,
) -> (Vec<(SautDuJournal, String)>, Vec<SautDuJournal>) {
    // Après une reprise, le jeu rattrape `rattrapage` pas par tour avant de sauter le reste
    // (sa déclaration) : le saut tombe au plus `rattrapage` pas après la partie reprise
    // (corrigé au lot 11 et au lot 12 : la règle du lot 10a supposait le rattrapage de
    // Punchline, 1 ; Sidequest en déclare 3, et ses sauts tombent trois pas après chaque
    // reprise). Un retard injecté se rattrape de même, un pas plus loin (lot 11, § 8.3).
    let apres = u64::from(rattrapage.max(1));
    let mut raisons: Vec<(u64, u64, String)> = lignes
        .iter()
        .filter_map(|l| {
            let pas = l.json.get("pas").and_then(Json::as_u64)?;
            match l.quoi() {
                "en-jeu" if l.json.get("relue").and_then(Json::as_u64).is_some() => Some((
                    pas,
                    pas.saturating_add(apres),
                    format!("reprise au pas {pas}"),
                )),
                "retarde" => Some((
                    pas,
                    pas.saturating_add(apres).saturating_add(1),
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
        let r = if o.complete {
            crate::soiree_complete::soiree_complete(o, *n, &paquet, &module, &charge)
        } else if !o.autres_jeux.is_empty() {
            crate::changer::changer(o, *n, &paquet, &module, &charge)
        } else if o.m12 > 0 {
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
        // `changer` et `soiree-complete` ne relisent pas la chaîne (ils jugent la garde
        // finale) : une ligne de zéros n'y dirait rien (lot 14). Ailleurs, une chaîne vide
        // garde sa ligne de zéros (lot 14 corrigé).
        if self.chaine_non_relue && h.cases == 0 {
            writeln!(
                f,
                "  chaîne : non relue par ce scénario (la garde finale est jugée)"
            )?;
        } else {
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
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_chaine_vide_se_dit_sauf_si_le_scenario_ne_la_relit_pas() {
        let relue = Passage::default().to_string();
        assert!(relue.contains("chaîne : 0 cases"), "{relue}");
        let non_relue = Passage {
            chaine_non_relue: true,
            ..Passage::default()
        }
        .to_string();
        assert!(non_relue.contains("chaîne : non relue"), "{non_relue}");
        assert!(!non_relue.contains("0 cases"), "{non_relue}");
    }

    fn ligne(json: &str) -> Ligne {
        Ligne {
            onglet: String::from("a"),
            json: serde_json::from_str(json).unwrap(),
            ms: 0,
        }
    }

    /// Lot 12, fin (`soiree-71`, M13 : de 71 appareils accueillis à 2) : les téléphones
    /// natifs tournent sur leur propre fil, et continuent pendant que le fil du banc attend
    /// (ici 300 ms ; là, `safaridriver` 30 s). Quand le fil du banc les faisait tourner
    /// entre deux lectures de la console, ils ne jouaient aucun tour pendant ses attentes.
    #[test]
    fn les_natifs_tournent_pendant_que_le_banc_attend() {
        let mut n = Natifs::lancer(port_libre().unwrap(), maintenant(), Vec::new());
        let avant = n.troupe().tours;
        std::thread::sleep(Duration::from_millis(300));
        {
            let t = n.troupe();
            assert!(t.tours.saturating_sub(avant) >= 30, "{} tours", t.tours);
            assert_eq!(t.silences, Vec::new());
        }
        n.arreter();
        let fin = n.troupe().tours;
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(n.troupe().tours, fin, "le fil arrêté ne tourne plus");
    }

    /// Lot 12, fin : un silence des natifs au-delà de `SILENCE_MAX` est une faute qui dit
    /// quand et combien ; en deçà, il est noté.
    #[test]
    fn un_long_silence_des_natifs_est_une_faute() {
        let n = Natifs::lancer(port_libre().unwrap(), maintenant(), Vec::new());
        let mut p = Passage::default();
        n.troupe().silences = vec![(1_000, 300), (73_026, 30_287)];
        juger_les_silences(&n.troupe(), &mut p);
        assert_eq!(p.fautes.len(), 1, "{:?}", p.fautes);
        assert!(p.fautes[0].contains("30287 ms") && p.fautes[0].contains("73026 ms"));
        assert!(p.notes[0].contains("2 silences"), "{:?}", p.notes);
        let mut p = Passage::default();
        n.troupe().silences = vec![(1_000, 999)];
        juger_les_silences(&n.troupe(), &mut p);
        assert_eq!(p.fautes, Vec::<String>::new());
    }

    /// Correction du lot 12 (vérification, constat 8) : le saut d'une reprise se rattache
    /// à la fenêtre du rattrapage que le jeu déclare, `de` de `p` à `p + rattrapage`, et pas
    /// plus loin ; chaque reprise n'explique qu'un saut ; un retard injecté au pas `p`
    /// explique un saut de `p` à `p + rattrapage + 1` (fusion du lot 11 : sa règle, § 8.3 ;
    /// le lot 12 gardait `p + 2` quel que soit le rattrapage, sans l'avoir éprouvé ailleurs
    /// qu'à `rattrapage: 1`, où les deux règles donnent la même fenêtre).
    #[test]
    fn un_saut_se_rattache_au_rattrapage_du_jeu_une_fois() {
        let lignes = [
            ligne(r#"{"quoi":"en-jeu","pas":100,"relue":7}"#),
            ligne(r#"{"quoi":"en-jeu","pas":500}"#),
            ligne(r#"{"quoi":"retarde","pas":900}"#),
        ];
        // Sidequest, rattrapage 3 : un saut trois pas après la reprise s'y rattache.
        let (r, seuls) = rattacher_les_sauts(&[(103, 120)], &lignes, 3);
        assert_eq!(r.len(), 1);
        assert_eq!(seuls, Vec::<SautDuJournal>::new());
        // Quatre pas après : rien ne l'explique.
        let (r, seuls) = rattacher_les_sauts(&[(104, 120)], &lignes, 3);
        assert_eq!(r.len(), 0);
        assert_eq!(seuls, [(104, 120)]);
        // Punchline, rattrapage 1 : la fenêtre du lot 10a, `p` ou `p + 1`.
        let (_, seuls) = rattacher_les_sauts(&[(101, 120)], &lignes, 1);
        assert_eq!(seuls, Vec::<SautDuJournal>::new());
        let (_, seuls) = rattacher_les_sauts(&[(102, 120)], &lignes, 1);
        assert_eq!(seuls, [(102, 120)]);
        // Un rattrapage nul se lit comme 1.
        let (_, seuls) = rattacher_les_sauts(&[(101, 120)], &lignes, 0);
        assert_eq!(seuls, Vec::<SautDuJournal>::new());
        // Une reprise n'absorbe qu'un saut, même dans sa fenêtre.
        let (r, seuls) = rattacher_les_sauts(&[(100, 101), (102, 120)], &lignes, 3);
        assert_eq!(r.len(), 1);
        assert_eq!(seuls, [(102, 120)]);
        // Une ligne « en-jeu » sans case relue n'est pas une reprise.
        let (_, seuls) = rattacher_les_sauts(&[(500, 510)], &lignes, 3);
        assert_eq!(seuls, [(500, 510)]);
        // Le retard injecté : de `p` à `p + rattrapage + 1`, et pas un pas plus loin.
        let (_, seuls) = rattacher_les_sauts(&[(904, 906)], &lignes, 3);
        assert_eq!(seuls, Vec::<SautDuJournal>::new());
        let (_, seuls) = rattacher_les_sauts(&[(905, 906)], &lignes, 3);
        assert_eq!(seuls, [(905, 906)]);
        let (_, seuls) = rattacher_les_sauts(&[(902, 905)], &lignes, 1);
        assert_eq!(seuls, Vec::<SautDuJournal>::new());
        let (_, seuls) = rattacher_les_sauts(&[(903, 905)], &lignes, 1);
        assert_eq!(seuls, [(903, 905)]);
    }

    /// Correction du lot 11 : la partie reprise joue jusqu'à `rattrapage` pas avant de
    /// sauter (`partie::regles::cible`, SQ-62). Sidequest (`rattrapage: 3`) saute trois pas après
    /// sa reprise (pas 1 253, saut de 1 256, constaté dans Chrome et Firefox) : ce saut est
    /// celui de la reprise ; un saut plus loin n'en est pas un. À `rattrapage: 1`
    /// (Punchline, Tier list), la règle reste celle du lot 10a : `p` ou `p + 1`.
    #[test]
    fn un_saut_se_rattache_a_sa_reprise_selon_le_rattrapage_du_jeu() {
        let lignes = [
            ligne(r#"{"quoi":"en-jeu","pas":1253,"relue":4}"#),
            ligne(r#"{"quoi":"en-jeu","pas":1312,"relue":6}"#),
            ligne(r#"{"quoi":"retarde","pas":3000}"#),
        ];
        let sauts = [(1256, 1267), (1316, 1331), (3004, 3010)];
        let (rattaches, seuls) = rattacher_les_sauts(&sauts, &lignes, 3);
        assert_eq!(
            rattaches,
            vec![
                ((1256, 1267), String::from("reprise au pas 1253")),
                ((3004, 3010), String::from("retard injecté au pas 3000")),
            ]
        );
        assert_eq!(seuls, vec![(1316, 1331)]);
        // Le même journal, d'un jeu qui ne rattrape qu'un pas : aucun ne se rattache.
        let (rattaches, seuls) = rattacher_les_sauts(&sauts, &lignes, 1);
        assert!(rattaches.is_empty(), "{rattaches:?}");
        assert_eq!(seuls.len(), 3);
        let (rattaches, seuls) =
            rattacher_les_sauts(&[(1253, 1260), (1313, 1320), (3002, 3009)], &lignes, 1);
        assert_eq!(rattaches.len(), 3, "{rattaches:?}");
        assert_eq!(seuls, Vec::<(u64, u64)>::new());
        // Chaque reprise n'explique qu'un saut.
        let (rattaches, seuls) =
            rattacher_les_sauts(&[(1254, 1258), (1255, 1290)], &lignes[..1], 3);
        assert_eq!(rattaches.len(), 1);
        assert_eq!(seuls, vec![(1255, 1290)]);
        // Un rattrapage déclaré à 0 vaut 1 : un pas avance toujours.
        let (rattaches, _) = rattacher_les_sauts(&[(1254, 1258)], &lignes[..1], 0);
        assert_eq!(rattaches.len(), 1);
    }
}
