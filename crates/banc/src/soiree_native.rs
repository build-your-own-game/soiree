//! `soiree-banc soiree-native` (PHASE-3, § 8.1, lot 5) : une soirée par **le vrai relais**
//! (le binaire `soiree-relais`, lancé en sous-processus, ou le même code dans le
//! processus pour les tests), **l'autorité native** (`salle::Autorite` sur
//! `moteur-wasmtime`), des **téléphones natifs** (`salle::Appareil`) qui jouent les coups
//! des bots par le vrai chemin (numérotation, file, chiffrement), de vraies WebSocket sur
//! la boucle, et le **mandataire de pannes** entre les téléphones et le relais (SAL-86).
//!
//! Les pannes, tirées de la graine : téléphone coupé, retenu, rechargé, en rafales,
//! fermé lentement ; onglet jumeau ; poignée de main rejouée par un invité (SAL-17) ;
//! autorité tuée puis relancée depuis la garde ; **`kill -9` du relais** puis relance.
//! Puis une fin calme.
//!
//! Les jugements, les mêmes que ceux de la soirée en mémoire (`salle::simulation`) :
//! aucune autorité arrêtée ; la chaîne entière des cases écrites (relue dans
//! l'enregistrement du relais, la garde sur disque n'en gardant que les dernières) s'ouvre
//! case après case et porte un journal d'un seul tenant ; aucun geste deux fois ; aucune
//! entrée écartée ; tout geste accusé écrit ; plus rien en file à la fin ; la garde finale,
//! relue sur le disque du relais, reprend une partie de même forme canonique et de même
//! empreinte que l'autorité vivante ; **aucun secret dans ce que le relais voit**
//! (`le_relais_ne_voit_aucun_secret`) ; et, si c'est demandé, une partie menée jusqu'à
//! sa fin.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use jeu_abi::{ApplicableFil, Capacites, Instant, Oui, Pas};
use liaison::{Case, CodeSalle, DuRelais, Lien, Role, VersRelais};
use moteur_wasmtime::{FabriqueWasm, InstantaneWasm, ModuleCharge, Moteur, ReglagesMoteur};
use salle::appareil::Appareil;
use salle::autorite::{Autorite, Cles, Comptes, ReglagesSalle};
use salle::chiffre::{CleDeGarde, Fragment, Hasard, PaireAutorite, SecretDEntree, TAILLE_POIGNEE};
use salle::ecran::{DeLaPage, VersPage};
use salle::fil::{TraceTelephone, VersAppareil, VersAutorite};
use salle::horloge::us_de_duree;
use salle::simulation::{
    Graine, Historique, PAS_US, TelephoneAccuse, accuses_hors_de_l_historique, forme_et_empreinte,
    reponse_par_defaut, reprendre_de_la_garde, verifier_l_historique,
};

use crate::horloge::{heure_us, maintenant};
use crate::mandataire::{self, Mandataire};

/// Le relais de la soirée : le vrai binaire, ou le même code dans ce processus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SorteRelais {
    /// `soiree-relais`, en sous-processus : on peut le tuer (`kill -9`).
    Processus(PathBuf),
    /// `relais::coque`, dans ce processus (les tests) : on ne le tue pas.
    DansLeProcessus,
}

/// Ce que la soirée joue.
#[derive(Debug, Clone)]
pub struct Options {
    /// Le module livré du jeu.
    pub module: PathBuf,
    /// Les téléphones ; par défaut, ceux que le jeu déclare, au plus 8.
    pub telephones: Option<usize>,
    /// La durée, avant la fin calme, en secondes.
    pub duree_s: u64,
    /// La graine.
    pub graine: u64,
    /// Les pannes des téléphones et de l'autorité.
    pub pannes: Oui,
    /// Combien de fois le relais est tué (`kill -9`) et relancé.
    pub tuer_le_relais: u32,
    /// Exiger qu'une partie aille jusqu'à une phase terminale.
    pub partie_entiere: Oui,
    /// Le relais.
    pub relais: SorteRelais,
    /// Le dossier de travail (la garde, l'enregistrement, la sortie du relais).
    pub travail: PathBuf,
    /// La fin calme la plus longue, en secondes.
    pub calme_max_s: u64,
}

/// Ce que la soirée a fait et constaté.
#[derive(Debug, Clone, Default)]
pub struct Bilan {
    /// Le nom du jeu, et le blake3 du module.
    pub jeu: String,
    /// Les téléphones.
    pub telephones: usize,
    /// Les images jouées par l'autorité.
    pub images: u64,
    /// Les autorités relancées depuis la garde (tuées, ou perdues avec le relais).
    pub redemarrages: u32,
    /// Les relais tués et relancés.
    pub relais_tues: u32,
    /// Les pannes des téléphones : coupures, retenues, rechargements, rafales, fermetures
    /// lentes, jumeaux, poignées rejouées.
    pub pannes: BTreeMap<&'static str, u32>,
    /// Les gestes émis, accusés.
    pub gestes_emis: u64,
    /// Les gestes accusés.
    pub gestes_accuses: u64,
    /// Ce que les autorités successives ont compté.
    pub comptes: Comptes,
    /// La chaîne écrite.
    pub historique: Historique,
    /// Les phases terminales vues.
    pub terminales: u32,
    /// Les aiguilles cherchées dans ce que le relais a vu, et les octets fouillés.
    pub secrets: (usize, u64),
    /// Les aiguilles cherchées, par sorte : `cles`, `jetons`, `gestes` (les octets d'un
    /// geste écrit, d'au moins 8 octets), `messages de geste` (chaque
    /// `VersAutorite::Geste` en clair, jusqu'aux octets du geste, échantillonné sur les
    /// téléphones), `vues` (les octets d'une vue, d'au moins 16 octets) et `messages de
    /// vue` (chaque `VersAppareil::Vue` en clair, entier, échantillonné).
    pub aiguilles: BTreeMap<&'static str, usize>,
    /// Les fils et la mémoire résidente (Kio) du relais au plus (M15), s'ils sont lus.
    pub relais_max: Option<(usize, u64)>,
    /// Ce qui n'a pas tenu.
    pub fautes: Vec<String>,
}

fn ajouter(c: &mut Comptes, d: Comptes) {
    c.gestes_places = c.gestes_places.saturating_add(d.gestes_places);
    c.gestes_en_double = c.gestes_en_double.saturating_add(d.gestes_en_double);
    c.gestes_hors_suite = c.gestes_hors_suite.saturating_add(d.gestes_hors_suite);
    c.gestes_refuses = c.gestes_refuses.saturating_add(d.gestes_refuses);
    c.accuses = c.accuses.saturating_add(d.accuses);
    c.cases_journal = c.cases_journal.saturating_add(d.cases_journal);
    c.points = c.points.saturating_add(d.points);
    c.trames_illisibles = c.trames_illisibles.saturating_add(d.trames_illisibles);
    c.poignees_inachevees = c.poignees_inachevees.saturating_add(d.poignees_inachevees);
    c.remplaces = c.remplaces.saturating_add(d.remplaces);
    c.vues = c.vues.saturating_add(d.vues);
    c.coups_banc = c.coups_banc.saturating_add(d.coups_banc);
    c.coups_ecran = c.coups_ecran.saturating_add(d.coups_ecran);
}

/// Les clés de la soirée, en octets : de quoi relancer l'autorité avec les mêmes.
struct ClesBrutes {
    privee: [u8; 32],
    secret: [u8; 16],
    garde: [u8; 32],
}

impl ClesBrutes {
    fn cles(&self) -> Result<Cles, String> {
        Ok(Cles {
            paire: PaireAutorite::depuis_privee(self.privee)
                .map_err(|e| format!("la paire de l'autorité : {e:?}"))?,
            secret: SecretDEntree::depuis_octets(self.secret),
            garde: CleDeGarde::depuis_octets(self.garde),
        })
    }
}

/// Un port libre à la fois sur `127.0.0.1` et `[::1]`.
pub(crate) fn port_libre() -> Result<u16, String> {
    for _ in 0..32 {
        let a = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("{e}"))?;
        let p = a.local_addr().map_err(|e| format!("{e}"))?.port();
        if TcpListener::bind(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), p)).is_ok() {
            return Ok(p);
        }
    }
    Err(String::from("aucun port libre sur les deux familles"))
}

enum RelaisLance {
    Processus(Child),
    Interne(relais::coque::Relais),
}

impl RelaisLance {
    fn pid(&self) -> Option<u32> {
        match self {
            RelaisLance::Processus(c) => Some(c.id()),
            RelaisLance::Interne(_) => None,
        }
    }

    /// `kill -9`, ou l'arrêt du relais interne.
    fn tuer(self) {
        match self {
            RelaisLance::Processus(mut c) => {
                let _: io::Result<()> = c.kill();
                let _: io::Result<std::process::ExitStatus> = c.wait();
            }
            RelaisLance::Interne(r) => r.arreter(),
        }
    }
}

fn joignable(adresse: SocketAddr) -> bool {
    TcpStream::connect_timeout(&adresse, Duration::from_millis(200)).is_ok()
}

fn lancer_le_relais(
    o: &Options,
    port: u16,
    garde: &Path,
    rang: u32,
) -> Result<RelaisLance, String> {
    let enregistrement = o.travail.join(format!("enregistrement-{rang}.bin"));
    let lance = match &o.relais {
        SorteRelais::Processus(bin) => {
            let sortie = File::create(o.travail.join(format!("relais-{rang}.txt")))
                .map_err(|e| format!("{e}"))?;
            let erreur = sortie.try_clone().map_err(|e| format!("{e}"))?;
            let enfant = Command::new(bin)
                .args(["--port", &port.to_string(), "--sans-reseau", "--banc"])
                .arg("--garde")
                .arg(garde)
                .arg("--enregistrer")
                .arg(&enregistrement)
                .stdin(Stdio::null())
                .stdout(sortie)
                .stderr(erreur)
                .spawn()
                .map_err(|e| format!("lancer {} : {e}", bin.display()))?;
            RelaisLance::Processus(enfant)
        }
        SorteRelais::DansLeProcessus => {
            let r = relais::coque::lancer(&relais::coque::Reglages {
                port,
                adresse: None,
                garde: garde.to_path_buf(),
                banc: relais::http::Banc::Oui,
                enregistrer: Some(enregistrement),
                interfaces: Some(Vec::new()),
                page: None,
                paquet: None,
                jeux: relais::paquet::Jeux::default(),
                annonce_du_banc: None,
                retard_d_echo_du_banc: None,
            })
            .map_err(|e| format!("le relais : {e}"))?;
            RelaisLance::Interne(r)
        }
    };
    let adresse = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let fin = maintenant().checked_add(Duration::from_secs(10));
    while !joignable(adresse) {
        if fin.is_some_and(|f| maintenant() >= f) {
            lance.tuer();
            return Err(format!("le relais n'écoute pas sur {adresse} en 10 s"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(lance)
}

/// L'autorité native et ses deux liens.
struct AutoriteNative {
    a: Autorite<FabriqueWasm, Graine>,
    temps_reel: crate::ws::Ws,
    garde: crate::ws::Ws,
    prochaine_image_us: u64,
}

/// Un téléphone natif.
struct Telephone {
    appareil: Appareil<Graine>,
    ws: Option<crate::ws::Ws>,
    jeton: [u8; 32],
    decalage_us: u64,
    revient_us: Option<u64>,
    part_us: Option<u64>,
    jumeau: bool,
    premiere_poignee: Option<Vec<u8>>,
    prochain_reveil_us: u64,
    /// Le plus haut numéro de geste dont le message en clair est déjà échantillonné.
    geste_echantillonne: u64,
}

/// Un invité qui a capté la poignée de main d'un téléphone, et la rejoue (SAL-17).
struct Invite {
    ws: crate::ws::Ws,
    part_us: u64,
}

/// La taille lue au plus par un client : une case de garde et son enveloppe.
pub(crate) const LECTURE_MAX: usize = relais::coeur::CASE_MAX + 64 * 1024;

/// Les vues, et les messages en clair de geste et de vue, gardés au plus pour la fouille
/// des secrets (chaque sorte à part).
const ECHANTILLON_MAX: usize = 400;

/// Le début d'un `VersAutorite::Geste` en clair, tel que l'appareil le scelle : tout ce
/// qui précède `fait`, les octets du geste compris. C'est ce que le relais verrait d'un
/// geste si le chiffrement manquait (le prénom tapé, la réponse de Punchline), même
/// quand le geste lui-même est trop court pour être cherché seul : l'onglet, tiré au
/// hasard sur 64 bits, le précède. Le message entier encodé deux fois, `fait` à deux pas
/// différents : leur préfixe commun s'arrête juste avant `fait`.
fn message_du_geste(onglet: u64, numero: u64, octets: &[u8]) -> Option<Vec<u8>> {
    let message = |pas: u64| {
        VersAutorite::Geste {
            onglet,
            numero,
            octets,
            fait: Instant::de(Pas::du_fil(pas)),
            vu: Pas::ZERO,
            trace: TraceTelephone {
                evenement_us: 0,
                envoi_us: 0,
                incertitude_us: 0,
            },
        }
        .encoder()
        .ok()
    };
    let (a, b) = (message(0)?, message(1)?);
    let commun = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let tete = a.get(..commun)?;
    (tete.len() >= 8 && tete.ends_with(octets)).then(|| tete.to_vec())
}

struct Soiree<'a> {
    o: &'a Options,
    charge: &'a ModuleCharge,
    cles: ClesBrutes,
    salle: CodeSalle,
    reglages: ReglagesSalle,
    redite_us: Option<u64>,
    port: u16,
    garde: PathBuf,
    relais: Option<RelaisLance>,
    relais_rang: u32,
    mandataire: Mandataire,
    autorite: Option<AutoriteNative>,
    relance_us: Option<u64>,
    dernier_n: Option<u64>,
    relances: u64,
    telephones: Vec<Telephone>,
    invites: Vec<Invite>,
    hasard: Graine,
    depart_us: u64,
    bilan: Bilan,
    vues: BTreeSet<Vec<u8>>,
    /// Les `VersAppareil::Vue` en clair que les téléphones ont reçus, échantillonnés.
    messages_de_vue: BTreeSet<Vec<u8>>,
    /// Les `VersAutorite::Geste` en clair, jusqu'aux octets du geste, échantillonnés.
    messages_de_geste: BTreeSet<Vec<u8>>,
    terminale: bool,
    /// La page ne donne plus d'images : la partie ne joue plus (la fin, où l'autorité
    /// écrit tout son journal).
    figee: bool,
}

impl Soiree<'_> {
    fn t(&self) -> u64 {
        let _: &Self = self;
        heure_us()
    }

    fn ecoule_us(&self) -> u64 {
        self.t().saturating_sub(self.depart_us)
    }

    fn adresse_du_relais(&self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), self.port)
    }

    fn fragment(&self) -> Result<Fragment, String> {
        Ok(Fragment {
            secret: SecretDEntree::depuis_octets(self.cles.secret),
            autorite: PaireAutorite::depuis_privee(self.cles.privee)
                .map_err(|e| format!("{e:?}"))?
                .publique(),
        })
    }

    // -------------------------------------------------------------------
    // L'autorité
    // -------------------------------------------------------------------

    fn lancer_l_autorite(&mut self) -> Result<(), String> {
        let adresse = self.adresse_du_relais();
        let temps_reel = crate::ws::Ws::relier(adresse, LECTURE_MAX).map_err(|e| e.0)?;
        let garde = crate::ws::Ws::relier(adresse, LECTURE_MAX).map_err(|e| e.0)?;
        self.relances = self.relances.saturating_add(1);
        let hasard = Graine::new("autorite", self.o.graine.wrapping_add(self.relances));
        let a = Autorite::demarrer(
            self.charge.fabrique(),
            self.cles.cles()?,
            self.reglages,
            hasard,
            self.dernier_n,
        );
        let t = self.t();
        self.autorite = Some(AutoriteNative {
            a,
            temps_reel,
            garde,
            prochaine_image_us: t,
        });
        self.vider_l_autorite();
        Ok(())
    }

    /// L'autorité meurt sans rien finir : ses liens coupés, sa partie perdue ; elle se
    /// relance depuis la garde `apres_us` plus tard.
    fn tuer_l_autorite(&mut self, apres_us: u64, pourquoi: &str) {
        let Some(n) = self.autorite.take() else {
            return;
        };
        if let Some(arret) = n.a.arret() {
            self.bilan
                .fautes
                .push(format!("l'autorité s'est arrêtée ({pourquoi}) : {arret:?}"));
        }
        ajouter(&mut self.bilan.comptes, n.a.comptes());
        self.dernier_n = n.a.dernier_ecrit().or(self.dernier_n);
        n.temps_reel.couper();
        n.garde.couper();
        self.relance_us = Some(self.t().saturating_add(apres_us));
        self.bilan.redemarrages = self.bilan.redemarrages.saturating_add(1);
    }

    fn vider_l_autorite(&mut self) {
        let Some(n) = self.autorite.as_mut() else {
            return;
        };
        let s = n.a.sorties();
        let mut perdu = None;
        for o in s.temps_reel {
            if let Err(e) = n.temps_reel.envoyer(o) {
                perdu = Some(e.0);
            }
        }
        for o in s.garde {
            if let Err(e) = n.garde.envoyer(o) {
                perdu = Some(e.0);
            }
        }
        let t = heure_us();
        for o in s.page {
            // La page du banc répond à chaque demande, faute de la servir (lots 9 à 11).
            if let Ok(VersPage::Demande { id, quoi }) = VersPage::decoder(&o) {
                let m = DeLaPage::Reponse {
                    id,
                    quoi: reponse_par_defaut(&quoi),
                };
                if let Ok(r) = m.encoder() {
                    n.a.page(t, &r);
                }
            }
        }
        if let Some(e) = perdu {
            self.tuer_l_autorite(300_000, &e);
        }
    }

    fn faire_tourner_l_autorite(&mut self) {
        if self.autorite.is_none() {
            if self.relance_us.is_some_and(|r| self.t() >= r) {
                if self.lancer_l_autorite().is_ok() {
                    self.relance_us = None;
                } else {
                    // Le relais n'est pas encore revenu : on retente.
                    self.relance_us = Some(self.t().saturating_add(200_000));
                }
            }
            return;
        }
        let t = self.t();
        let mut perdu = None;
        if let Some(n) = self.autorite.as_mut() {
            loop {
                match n.temps_reel.recevoir() {
                    Ok(Some(o)) => n.a.temps_reel(t, &o),
                    Ok(None) => break,
                    Err(e) => {
                        perdu = Some(e.0);
                        break;
                    }
                }
            }
            loop {
                match n.garde.recevoir() {
                    Ok(Some(o)) => n.a.garde(t, &o),
                    Ok(None) => break,
                    Err(e) => {
                        perdu = Some(e.0);
                        break;
                    }
                }
            }
            if !self.figee && t >= n.prochaine_image_us {
                n.prochaine_image_us = n.prochaine_image_us.saturating_add(PAS_US).max(t);
                let m = DeLaPage::Image {
                    t_us: t,
                    periode_us: 16_667,
                };
                if let Ok(o) = m.encoder() {
                    n.a.page(t, &o);
                }
                self.bilan.images = self.bilan.images.saturating_add(1);
            }
        }
        if let Some(e) = perdu {
            self.tuer_l_autorite(300_000, &e);
            return;
        }
        self.vider_l_autorite();
        if let Some(n) = self.autorite.as_mut() {
            let _: Result<(), crate::ws::Perdu> = n.temps_reel.vider();
            let _: Result<(), crate::ws::Perdu> = n.garde.vider();
        }
    }

    // -------------------------------------------------------------------
    // Les téléphones
    // -------------------------------------------------------------------

    fn ajouter_telephone(
        &mut self,
        jeton: Option<[u8; 32]>,
        part_us: Option<u64>,
    ) -> Result<usize, String> {
        let k = self.telephones.len();
        let mut h = Graine::new(
            "telephone",
            self.o.graine.wrapping_mul(1000).wrapping_add(k as u64),
        );
        let jumeau = jeton.is_some();
        let jeton = jeton.unwrap_or_else(|| {
            let mut j = [0u8; 32];
            h.remplir(&mut j);
            j
        });
        let onglet = h.entier();
        let decalage_us = h.sous(60_000_000);
        let appareil = Appareil::new(
            self.fragment()?,
            self.salle,
            (jeton, onglet),
            Capacites::AUCUNE,
            self.redite_us,
            h,
        );
        self.telephones.push(Telephone {
            appareil,
            ws: None,
            jeton,
            decalage_us,
            revient_us: Some(0),
            part_us,
            jumeau,
            premiere_poignee: None,
            prochain_reveil_us: 0,
            geste_echantillonne: 0,
        });
        Ok(k)
    }

    fn relier(&mut self, k: usize) {
        let adresse = self.mandataire.adresse();
        let relais = self.adresse_du_relais();
        let t = self.t();
        let Some(tel) = self.telephones.get_mut(k) else {
            return;
        };
        match crate::ws::Ws::relier_par(adresse, relais, LECTURE_MAX) {
            Ok(ws) => {
                tel.ws = Some(ws);
                tel.revient_us = None;
                tel.appareil.relier();
            }
            Err(_) => tel.revient_us = Some(t.saturating_add(500_000)),
        }
    }

    fn couper_le_telephone(&mut self, k: usize, revient_apres_us: Option<u64>) {
        let t = self.t();
        let Some(tel) = self.telephones.get_mut(k) else {
            return;
        };
        if let Some(ws) = tel.ws.take() {
            ws.couper();
        }
        tel.appareil.couper();
        tel.revient_us = revient_apres_us.map(|d| t.saturating_add(d));
    }

    fn faire_tourner_les_telephones(&mut self) {
        let t = self.t();
        for k in 0..self.telephones.len() {
            let Some(tel) = self.telephones.get_mut(k) else {
                continue;
            };
            if tel.part_us.is_some_and(|p| t >= p) {
                tel.part_us = None;
                self.couper_le_telephone(k, None);
                continue;
            }
            if tel.ws.is_none() {
                if tel.revient_us.is_some_and(|r| t >= r) && !tel.appareil.fini() {
                    self.relier(k);
                } else {
                    continue;
                }
            }
            let Some(tel) = self.telephones.get_mut(k) else {
                continue;
            };
            let local = t.saturating_add(tel.decalage_us);
            let mut perdu = false;
            if let Some(ws) = tel.ws.as_mut() {
                loop {
                    match ws.recevoir() {
                        Ok(Some(o)) => tel.appareil.recevoir(local, &o),
                        Ok(None) => break,
                        Err(_) => {
                            perdu = true;
                            break;
                        }
                    }
                }
            }
            if t >= tel.prochain_reveil_us {
                tel.prochain_reveil_us = t.saturating_add(PAS_US);
                tel.appareil.reveiller(local);
            }
            if self.messages_de_geste.len() < ECHANTILLON_MAX {
                let onglet = tel.appareil.onglet();
                for (numero, octets) in tel.appareil.gestes_en_file() {
                    if numero > tel.geste_echantillonne {
                        tel.geste_echantillonne = numero;
                        if let Some(m) = message_du_geste(onglet, numero, octets) {
                            self.messages_de_geste.insert(m);
                        }
                    }
                }
            }
            for o in tel.appareil.sorties() {
                if tel.premiere_poignee.is_none()
                    && let Ok(VersRelais::Trame { trame, .. }) = VersRelais::decoder(&o)
                    && trame.len() == TAILLE_POIGNEE
                {
                    tel.premiere_poignee = Some(trame.to_vec());
                }
                if let Some(ws) = tel.ws.as_mut()
                    && ws.envoyer(o).is_err()
                {
                    perdu = true;
                }
            }
            if let Some(ws) = tel.ws.as_mut()
                && ws.vider().is_err()
            {
                perdu = true;
            }
            if perdu {
                // Le relais ou le mandataire a fermé : le téléphone revient seul, sauf
                // s'il a reçu un refus final (« Remplacé », SAL-18).
                let revient = if tel.jumeau { None } else { Some(1_000_000) };
                self.couper_le_telephone(k, revient);
            }
        }
        let invites = core::mem::take(&mut self.invites);
        for mut i in invites {
            let fini = t >= i.part_us || i.ws.recevoir().is_err();
            if fini {
                i.ws.couper();
            } else {
                self.invites.push(i);
            }
        }
    }

    // -------------------------------------------------------------------
    // Les pannes
    // -------------------------------------------------------------------

    fn compter(&mut self, quoi: &'static str) {
        let n = self.bilan.pannes.entry(quoi).or_insert(0);
        *n = n.saturating_add(1);
    }

    fn panne(&mut self) {
        let n = self.telephones.len() as u64;
        let sorte = self.hasard.sous(10);
        let k = usize::try_from(self.hasard.sous(n.max(1))).unwrap_or(0);
        let apres = self.hasard.entre(300_000, 3_000_000);
        let locale = self
            .telephones
            .get(k)
            .and_then(|t| t.ws.as_ref())
            .and_then(crate::ws::Ws::locale);
        match (sorte, locale) {
            (0 | 1, _) if self.autorite.is_some() => {
                let apres = self.hasard.entre(200_000, 2_000_000);
                self.tuer_l_autorite(apres, "tuée par le banc");
                self.compter("autorité tuée");
            }
            (2, Some(l)) => {
                self.mandataire.couper(l);
                self.compter("coupure");
            }
            (3, Some(l)) => {
                let d = self.hasard.entre(500_000, 2_000_000);
                self.mandataire.retenir(l, mandataire::Sens::VersRelais, d);
                self.mandataire
                    .retenir(l, mandataire::Sens::VersTelephone, d);
                self.compter("retenue");
            }
            (4 | 5, Some(_)) => {
                self.couper_le_telephone(k, Some(apres));
                if let Some(t) = self.telephones.get_mut(k) {
                    t.appareil.recharger();
                }
                self.compter("rechargement");
            }
            (6, Some(_)) => {
                let jeton = self.telephones.get(k).map(|t| t.jeton);
                let part = self.t().saturating_add(apres);
                if self.ajouter_telephone(jeton, Some(part)).is_ok() {
                    self.compter("jumeau");
                }
            }
            (7, _) => {
                let premiere = self
                    .telephones
                    .get(k)
                    .and_then(|t| t.premiere_poignee.clone());
                if let Some(p) = premiere
                    && let Ok(mut ws) = crate::ws::Ws::relier_par(
                        self.mandataire.adresse(),
                        self.adresse_du_relais(),
                        LECTURE_MAX,
                    )
                {
                    for m in [
                        VersRelais::Bonjour {
                            protocole: liaison::PROTOCOLE_RELAIS,
                            salle: self.salle,
                            role: Role::Appareil,
                        },
                        VersRelais::Trame {
                            classe: liaison::Classe::Fiable,
                            trame: &p,
                        },
                    ] {
                        if let Ok(o) = m.encoder() {
                            let _: Result<(), crate::ws::Perdu> = ws.envoyer(o);
                        }
                    }
                    self.invites.push(Invite {
                        ws,
                        part_us: self.t().saturating_add(12_000_000),
                    });
                    self.compter("poignée rejouée");
                }
            }
            (8, Some(l)) => {
                let d = self.hasard.entre(2_000_000, 5_000_000);
                self.mandataire.rafales(l, d);
                self.compter("rafales");
            }
            (9, Some(l)) => {
                // Une page rechargée derrière une fermeture lente (SAL-60) : la fin de
                // l'ancienne connexion arrive après la nouvelle.
                self.mandataire.fermer_lentement(l);
                self.couper_le_telephone(k, Some(50_000));
                self.compter("fermeture lente");
            }
            _ => {
                // Un téléphone remplacé reprend sa place (« Reprendre ici », SAL-18).
                if let Some(j) = self
                    .telephones
                    .iter()
                    .position(|t| t.appareil.fini() && !t.jumeau)
                {
                    if let Some(t) = self.telephones.get_mut(j) {
                        t.appareil.reprendre_ici();
                        t.revient_us = Some(0);
                    }
                    self.compter("reprendre ici");
                }
            }
        }
    }

    fn tuer_le_relais(&mut self) -> Result<(), String> {
        let Some(r) = self.relais.take() else {
            return Ok(());
        };
        r.tuer();
        self.bilan.relais_tues = self.bilan.relais_tues.saturating_add(1);
        std::thread::sleep(Duration::from_millis(500));
        self.relais_rang = self.relais_rang.saturating_add(1);
        self.relais = Some(lancer_le_relais(
            self.o,
            self.port,
            &self.garde,
            self.relais_rang,
        )?);
        Ok(())
    }

    // -------------------------------------------------------------------
    // Ce qu'on observe en chemin
    // -------------------------------------------------------------------

    fn observer(&mut self) {
        if let Some(p) = self.autorite.as_mut().and_then(|n| n.a.partie_mut())
            && let Ok(e) = p.etat()
        {
            let terminale = e.terminale == Oui::Oui;
            if terminale && !self.terminale {
                self.bilan.terminales = self.bilan.terminales.saturating_add(1);
            }
            self.terminale = terminale;
        }
        for t in &self.telephones {
            let Some((pas, v)) = t.appareil.vue() else {
                continue;
            };
            if self.vues.len() < ECHANTILLON_MAX && v.len() >= 16 {
                self.vues.insert(v.to_vec());
            }
            if self.messages_de_vue.len() < ECHANTILLON_MAX
                && let Ok(m) = (VersAppareil::Vue { pas, montre: v }).encoder()
                && m.len() >= 8
            {
                self.messages_de_vue.insert(m);
            }
        }
    }

    fn mesurer_le_relais(&mut self) {
        let Some(pid) = self.relais.as_ref().and_then(RelaisLance::pid) else {
            return;
        };
        let Some((fils, rss)) = fils_et_memoire(pid) else {
            return;
        };
        let (f, m) = self.bilan.relais_max.unwrap_or((0, 0));
        self.bilan.relais_max = Some((f.max(fils), m.max(rss)));
    }

    // -------------------------------------------------------------------
    // La boucle
    // -------------------------------------------------------------------

    fn tourner_jusqu_a(
        &mut self,
        fin_us: u64,
        pannes: Oui,
        tuer_a: &mut Vec<u64>,
    ) -> Result<(), String> {
        let mut prochaine_panne = self
            .ecoule_us()
            .saturating_add(self.hasard.entre(2_000_000, 4_000_000));
        let mut prochaine_observation = 0u64;
        let mut prochaine_mesure = 0u64;
        let mut tour = 0u64;
        while self.ecoule_us() < fin_us {
            self.faire_tourner_l_autorite();
            // Les téléphones, une milliseconde sur deux : 71 sockets lues à chaque tour
            // coûtent plus au banc qu'elles n'apprennent.
            tour = tour.wrapping_add(1);
            if tour.is_multiple_of(2) {
                self.faire_tourner_les_telephones();
            }
            let e = self.ecoule_us();
            if pannes == Oui::Oui && e >= prochaine_panne {
                prochaine_panne = e.saturating_add(self.hasard.entre(2_000_000, 6_000_000));
                self.panne();
            }
            if tuer_a.first().is_some_and(|t| e >= *t) {
                tuer_a.remove(0);
                self.tuer_le_relais()?;
            }
            if e >= prochaine_observation {
                prochaine_observation = e.saturating_add(1_000_000);
                self.observer();
            }
            if e >= prochaine_mesure {
                prochaine_mesure = e.saturating_add(5_000_000);
                self.mesurer_le_relais();
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    /// La fin calme : plus de pannes ; les jumeaux partent ; chaque téléphone revient
    /// (« Reprendre ici » s'il a été remplacé) et ne joue plus ; tout ce qui attend doit
    /// être accusé.
    fn calmer(&mut self) -> Result<(), String> {
        let mut aucun: Vec<u64> = Vec::new();
        let e = self.ecoule_us();
        self.tourner_jusqu_a(e.saturating_add(3_000_000), Oui::Non, &mut aucun)?;
        for k in 0..self.telephones.len() {
            let jumeau = self.telephones.get(k).is_some_and(|t| t.jumeau);
            if jumeau {
                self.couper_le_telephone(k, None);
                continue;
            }
            if let Some(t) = self.telephones.get_mut(k) {
                t.appareil.taire_le_banc();
                if t.appareil.fini() {
                    t.appareil.reprendre_ici();
                }
                if t.ws.is_none() {
                    t.revient_us = Some(0);
                }
            }
        }
        let fin = self
            .ecoule_us()
            .saturating_add(self.o.calme_max_s.saturating_mul(1_000_000));
        loop {
            let e = self.ecoule_us();
            self.tourner_jusqu_a(e.saturating_add(500_000), Oui::Non, &mut aucun)?;
            let tout_accuse = self
                .telephones
                .iter()
                .filter(|t| !t.jumeau)
                .all(|t| t.appareil.en_file() == 0 && t.appareil.accueilli());
            if tout_accuse && self.autorite.is_some() {
                return Ok(());
            }
            if self.ecoule_us() >= fin {
                for (k, t) in self.telephones.iter().enumerate() {
                    if !t.jumeau && !t.appareil.accueilli() {
                        self.bilan.fautes.push(format!(
                            "téléphone {k} : pas accueilli à la fin du calme ({} s)",
                            self.o.calme_max_s
                        ));
                    }
                }
                return Ok(());
            }
        }
    }
}

/// Les fils et la mémoire résidente (Kio) d'un processus, lus par `ps` (M15).
fn fils_et_memoire(pid: u32) -> Option<(usize, u64)> {
    let p = pid.to_string();
    let fils = Command::new("ps").args(["-M", "-p", &p]).output().ok()?;
    let lignes = String::from_utf8_lossy(&fils.stdout).lines().count();
    let rss = Command::new("ps")
        .args(["-o", "rss=", "-p", &p])
        .output()
        .ok()?;
    let rss: u64 = String::from_utf8_lossy(&rss.stdout).trim().parse().ok()?;
    Some((lignes.saturating_sub(1), rss))
}

// -----------------------------------------------------------------------
// L'enregistrement du relais : la chaîne écrite, et les secrets
// -----------------------------------------------------------------------

/// Lit un enregistrement du relais note après note, sans le charger entier. Rend `false`
/// s'il finit sur une note coupée (un `kill -9`).
fn lire_l_enregistrement(
    chemin: &Path,
    mut note: impl FnMut(Lien, relais::enregistrement::Sens, &[u8]),
) -> Result<bool, String> {
    let f = File::open(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
    let mut r = BufReader::with_capacity(1 << 20, f);
    let mut entete = vec![0u8; relais::enregistrement::ENTETE.len()];
    r.read_exact(&mut entete).map_err(|e| format!("{e}"))?;
    if entete != relais::enregistrement::ENTETE {
        return Err(format!(
            "{} n'est pas un enregistrement du relais",
            chemin.display()
        ));
    }
    let mut corps = Vec::new();
    loop {
        if std::io::BufRead::fill_buf(&mut r)
            .map_err(|e| format!("{e}"))?
            .is_empty()
        {
            return Ok(true);
        }
        let mut tete = [0u8; 17];
        match r.read_exact(&mut tete) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(e) => return Err(format!("{e}")),
        }
        let (_, suite) = tete.split_first_chunk::<8>().unwrap_or((&[0; 8], &[]));
        let (lien, suite) = suite.split_first_chunk::<4>().unwrap_or((&[0; 4], &[]));
        let (sens, suite) = suite.split_first_chunk::<1>().unwrap_or((&[0; 1], &[]));
        let (n, _) = suite.split_first_chunk::<4>().unwrap_or((&[0; 4], &[]));
        let sens = match sens {
            [0] => relais::enregistrement::Sens::Recu,
            [1] => relais::enregistrement::Sens::Envoye,
            [2] => relais::enregistrement::Sens::Ouvert,
            _ => relais::enregistrement::Sens::Ferme,
        };
        corps.resize(usize::try_from(u32::from_le_bytes(*n)).unwrap_or(0), 0);
        match r.read_exact(&mut corps) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(e) => return Err(format!("{e}")),
        }
        note(Lien(u32::from_le_bytes(*lien)), sens, &corps);
    }
}

/// La chaîne entière des cases écrites dans la soirée, relue dans les enregistrements
/// successifs du relais : une case est écrite quand le relais l'accuse (`Ecrit`), ou quand
/// il la lit sur son disque (`Lu`, après une relance) ; la garde finale sur disque a le
/// dernier mot.
pub(crate) fn chaine_ecrite(
    enregistrements: &[PathBuf],
    disque: &[(Case, Vec<u8>)],
) -> Result<(Chaine, u32), String> {
    let mut chaine: BTreeMap<u64, (Case, Vec<u8>)> = BTreeMap::new();
    let mut coupes = 0u32;
    for chemin in enregistrements {
        let mut gardes: BTreeSet<Lien> = BTreeSet::new();
        let mut en_cours: BTreeMap<Lien, Vec<(Case, Vec<u8>)>> = BTreeMap::new();
        let complet = lire_l_enregistrement(chemin, |lien, sens, o| match sens {
            relais::enregistrement::Sens::Recu => match VersRelais::decoder(o) {
                Ok(VersRelais::Bonjour {
                    role: Role::Garde { .. },
                    ..
                }) => {
                    gardes.insert(lien);
                }
                Ok(VersRelais::Ecrire { case, octets }) if gardes.contains(&lien) => {
                    en_cours
                        .entry(lien)
                        .or_default()
                        .push((case, octets.to_vec()));
                }
                Ok(_) | Err(_) => {}
            },
            relais::enregistrement::Sens::Envoye if gardes.contains(&lien) => {
                match DuRelais::decoder(o) {
                    Ok(DuRelais::Ecrit { case }) => {
                        let file = en_cours.entry(lien).or_default();
                        if let Some(i) = file.iter().position(|(c, _)| *c == case) {
                            let (c, octets) = file.remove(i);
                            chaine.insert(numero(c), (c, octets));
                        }
                    }
                    Ok(DuRelais::Lu { case, octets }) => {
                        chaine.insert(numero(case), (case, octets.to_vec()));
                    }
                    Ok(_) | Err(_) => {}
                }
            }
            relais::enregistrement::Sens::Envoye
            | relais::enregistrement::Sens::Ouvert
            | relais::enregistrement::Sens::Ferme => {}
        })?;
        if !complet {
            coupes = coupes.saturating_add(1);
        }
    }
    for (c, o) in disque {
        chaine.insert(numero(*c), (*c, o.clone()));
    }
    Ok((chaine.into_values().collect(), coupes))
}

/// Des cases et leurs octets.
pub(crate) type Chaine = Vec<(Case, Vec<u8>)>;

const fn numero(c: Case) -> u64 {
    match c {
        Case::Point(n) | Case::Journal(n) => n,
    }
}

/// Des aiguilles (au moins 8 octets chacune) dans des meules : chaque aiguille trouvée,
/// nommée, avec la meule. Un filtre sur les 8 premiers octets écarte presque tout.
struct Fouille {
    aiguilles: Vec<(String, Vec<u8>)>,
    prefixes: Vec<(u64, usize)>,
    filtre: Vec<u64>,
    trouvees: BTreeSet<String>,
    octets: u64,
}

const FILTRE_BITS: u32 = 24;

fn indice_filtre(p: u64) -> usize {
    usize::try_from(p.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - FILTRE_BITS)).unwrap_or(0)
}

impl Fouille {
    fn new(aiguilles: Vec<(String, Vec<u8>)>) -> Fouille {
        let mut filtre = vec![0u64; (1usize << FILTRE_BITS) / 64];
        let mut prefixes = Vec::new();
        for (k, (_, a)) in aiguilles.iter().enumerate() {
            if let Some(p) = a.first_chunk::<8>() {
                let p = u64::from_le_bytes(*p);
                prefixes.push((p, k));
                let i = indice_filtre(p);
                if let Some(m) = filtre.get_mut(i / 64) {
                    *m |= 1u64 << (i % 64);
                }
            }
        }
        prefixes.sort_unstable();
        Fouille {
            aiguilles,
            prefixes,
            filtre,
            trouvees: BTreeSet::new(),
            octets: 0,
        }
    }

    fn fouiller(&mut self, meule: &str, o: &[u8]) {
        self.octets = self.octets.saturating_add(o.len() as u64);
        for (i, fenetre) in o.windows(8).enumerate() {
            let Some(p) = fenetre.first_chunk::<8>().map(|p| u64::from_le_bytes(*p)) else {
                continue;
            };
            let f = indice_filtre(p);
            if self
                .filtre
                .get(f / 64)
                .is_none_or(|m| m & (1u64 << (f % 64)) == 0)
            {
                continue;
            }
            let debut = self.prefixes.partition_point(|(x, _)| *x < p);
            for (x, k) in self.prefixes.get(debut..).unwrap_or(&[]) {
                if *x != p {
                    break;
                }
                if let Some((nom, a)) = self.aiguilles.get(*k)
                    && o.get(i..).is_some_and(|reste| reste.starts_with(a))
                {
                    self.trouvees.insert(format!("{nom}, dans {meule}"));
                }
            }
        }
    }
}

// -----------------------------------------------------------------------
// La soirée entière
// -----------------------------------------------------------------------

/// Joue la soirée, et rend son bilan. `Err` : l'outil n'a pas pu jouer (code 2).
#[expect(
    clippy::too_many_lines,
    reason = "la soirée se lit d'un trait : préparer, jouer, calmer, juger"
)]
pub fn jouer(o: &Options) -> Result<Bilan, String> {
    let octets =
        std::fs::read(&o.module).map_err(|e| format!("lire {} : {e}", o.module.display()))?;
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).map_err(|e| format!("le moteur : {e}"))?;
    let charge = moteur
        .charger(&octets)
        .map_err(|e| format!("le module est refusé : {e}"))?;
    let declaration = charge.declaration();
    let telephones = o.telephones.unwrap_or(match declaration.telephones {
        ApplicableFil::Oui(_) => usize::from(declaration.places.appareils_max()).min(8),
        ApplicableFil::SansObjet(_) => 0,
    });
    let redite_us = match &declaration.telephones {
        ApplicableFil::Oui(t) => liaison::periode_de_redite(&t.tenu).map(us_de_duree),
        ApplicableFil::SansObjet(_) => None,
    };
    let bilan = Bilan {
        jeu: format!(
            "{} (blake3 {})",
            declaration.nom.as_str(),
            blake3::hash(&octets).to_hex()
        ),
        telephones,
        ..Bilan::default()
    };
    std::fs::create_dir_all(&o.travail).map_err(|e| format!("{e}"))?;
    let garde = o.travail.join("garde");
    let mut cles_hasard = Graine::new("cles", o.graine);
    let paire = PaireAutorite::tirer(&mut cles_hasard).map_err(|e| format!("{e:?}"))?;
    let secret = SecretDEntree::tirer(&mut cles_hasard);
    let cle_garde = CleDeGarde::tirer(&mut cles_hasard);
    let cles = ClesBrutes {
        privee: *paire.privee(),
        secret: *secret.octets(),
        garde: *cle_garde.octets(),
    };
    let mut hasard = Graine::new("soiree", o.graine);
    let mut code = [0u8; 4];
    hasard.remplir(&mut code);
    let salle = CodeSalle(code);
    let mut reglages = ReglagesSalle::soiree(salle);
    reglages.banc = Oui::Oui;
    let port = port_libre()?;
    let relais = lancer_le_relais(o, port, &garde, 0)?;
    let mandataire = Mandataire::lancer(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
        o.graine,
    )
    .map_err(|e| format!("le mandataire : {e}"))?;
    let mut soiree = Soiree {
        o,
        charge: &charge,
        cles,
        salle,
        reglages,
        redite_us,
        port,
        garde: garde.clone(),
        relais: Some(relais),
        relais_rang: 0,
        mandataire,
        autorite: None,
        relance_us: None,
        dernier_n: None,
        relances: 0,
        telephones: Vec::new(),
        invites: Vec::new(),
        hasard,
        depart_us: heure_us(),
        bilan,
        vues: BTreeSet::new(),
        messages_de_vue: BTreeSet::new(),
        messages_de_geste: BTreeSet::new(),
        terminale: false,
        figee: false,
    };
    soiree.lancer_l_autorite()?;
    for _ in 0..telephones {
        soiree.ajouter_telephone(None, None)?;
    }
    let duree_us = o.duree_s.saturating_mul(1_000_000);
    let mut tuer_a: Vec<u64> = (1..=u64::from(o.tuer_le_relais))
        .map(|k| {
            duree_us
                .saturating_mul(k)
                .checked_div(u64::from(o.tuer_le_relais).saturating_add(1))
                .unwrap_or(0)
        })
        .collect();
    if o.tuer_le_relais > 0 && matches!(o.relais, SorteRelais::DansLeProcessus) {
        return Err(String::from(
            "le relais interne ne se tue pas : --tuer-le-relais demande le binaire",
        ));
    }
    soiree.tourner_jusqu_a(duree_us, o.pannes, &mut tuer_a)?;
    soiree.calmer()?;
    // La partie ne joue plus ; l'autorité écrit tout son journal, et attend chaque
    // `Ecrit`.
    soiree.figee = true;
    let t = heure_us();
    if let Some(n) = soiree.autorite.as_mut() {
        n.a.ecrire_la_garde(t);
    }
    soiree.vider_l_autorite();
    let mut dernier = None;
    let mut stable = 0u32;
    let limite = soiree.ecoule_us().saturating_add(20_000_000);
    while stable < 20 && soiree.ecoule_us() < limite {
        let e = soiree.ecoule_us();
        soiree.tourner_jusqu_a(e.saturating_add(50_000), Oui::Non, &mut Vec::new())?;
        let d = soiree.autorite.as_ref().and_then(|n| n.a.dernier_ecrit());
        if d == dernier {
            stable = stable.saturating_add(1);
        } else {
            stable = 0;
            dernier = d;
        }
    }
    let mut fautes: Vec<String> = Vec::new();
    // Ce que l'autorité vivante tient.
    let mut vivante = None;
    let mut registre = None;
    if let Some(n) = soiree.autorite.as_mut() {
        if let Some(arret) = n.a.arret() {
            fautes.push(format!("l'autorité finale s'est arrêtée : {arret:?}"));
        }
        registre = n.a.registre();
        dernier = n.a.dernier_ecrit();
        if let Some(p) = n.a.partie_mut() {
            vivante = Some(forme_et_empreinte(p));
        }
        ajouter(&mut soiree.bilan.comptes, n.a.comptes());
    } else {
        fautes.push(String::from("aucune autorité vivante à la fin"));
    }
    let telephones_accuses: Vec<TelephoneAccuse> = soiree
        .telephones
        .iter()
        .map(|t| TelephoneAccuse {
            jeton: t.jeton,
            onglet: t.appareil.onglet(),
            accuse: t.appareil.accuse(),
        })
        .collect();
    for (k, t) in soiree.telephones.iter().enumerate() {
        soiree.bilan.gestes_emis = soiree
            .bilan
            .gestes_emis
            .saturating_add(t.appareil.comptes().gestes);
        soiree.bilan.gestes_accuses = soiree
            .bilan
            .gestes_accuses
            .saturating_add(t.appareil.accuse());
        if !t.jumeau && t.appareil.en_file() > 0 {
            fautes.push(format!(
                "téléphone {k} : {} geste(s) jamais accusés",
                t.appareil.en_file()
            ));
        }
    }
    let jetons: Vec<[u8; 32]> = soiree.telephones.iter().map(|t| t.jeton).collect();
    let vues: Vec<Vec<u8>> = soiree.vues.iter().cloned().collect();
    let messages_de_vue: Vec<Vec<u8>> = soiree.messages_de_vue.iter().cloned().collect();
    let messages_de_geste: Vec<Vec<u8>> = soiree.messages_de_geste.iter().cloned().collect();
    // Tout s'arrête.
    for k in 0..soiree.telephones.len() {
        soiree.couper_le_telephone(k, None);
    }
    if let Some(n) = soiree.autorite.take() {
        n.temps_reel.couper();
        n.garde.couper();
    }
    soiree.mesurer_le_relais();
    if let Some(r) = soiree.relais.take() {
        r.tuer();
    }
    let rangs = soiree.relais_rang;
    soiree.mandataire.arreter();
    let mut bilan = soiree.bilan;
    bilan.fautes.append(&mut fautes);
    // La garde finale, sur le disque du relais.
    let dossier_garde = relais::garde::Dossier::ouvrir(&garde).map_err(|e| format!("{e}"))?;
    let mut disque = Vec::new();
    for c in dossier_garde.lister(salle).map_err(|e| format!("{e}"))? {
        if let Some(o) = dossier_garde.lire(salle, c).map_err(|e| format!("{e}"))? {
            disque.push((c, o));
        }
    }
    match vivante {
        Some(Ok(v)) => {
            match reprendre_de_la_garde(charge.fabrique(), &cle_garde, salle, &disque, dernier) {
                Ok(r) if r == v => {}
                Ok(r) => bilan.fautes.push(format!(
                    "la garde finale reprend une autre partie : vivante {v:?}, reprise {r:?}"
                )),
                Err(e) => bilan.fautes.push(format!("la garde finale : {e}")),
            }
        }
        Some(Err(e)) => bilan.fautes.push(format!("la partie vivante : {e}")),
        None => bilan
            .fautes
            .push(String::from("aucune partie vivante à la fin")),
    }
    // La chaîne entière, relue dans l'enregistrement du relais.
    let enregistrements: Vec<PathBuf> = (0..=rangs)
        .map(|k| o.travail.join(format!("enregistrement-{k}.bin")))
        .collect();
    let (chaine, coupes) = chaine_ecrite(&enregistrements, &disque)?;
    let attendues = bilan.relais_tues;
    if coupes > attendues {
        bilan.fautes.push(format!(
            "{coupes} enregistrement(s) coupés, pour {attendues} relais tués"
        ));
    }
    match verifier_l_historique::<InstantaneWasm>(&cle_garde, salle, &chaine) {
        Ok(h) => {
            if !h.en_double.is_empty() {
                bilan.fautes.push(format!(
                    "{} geste(s) écrits deux fois (SAL-57)",
                    h.en_double.len()
                ));
            }
            if h.ecartees > 0 {
                bilan.fautes.push(format!(
                    "{} entrée(s) écartées : le jeu a fauté",
                    h.ecartees
                ));
            }
            bilan.fautes.extend(accuses_hors_de_l_historique(
                registre.as_ref(),
                &telephones_accuses,
                &h,
            ));
            bilan.historique = h;
        }
        Err(e) => bilan.fautes.push(format!("la chaîne de la soirée : {e}")),
    }
    if o.partie_entiere == Oui::Oui && bilan.terminales == 0 {
        bilan.fautes.push(String::from(
            "aucune phase terminale : la partie n'est pas allée à sa fin",
        ));
    }
    if bilan.gestes_emis == 0 && bilan.comptes.coups_ecran == 0 {
        bilan.fautes.push(String::from("personne n'a joué"));
    }
    // Aucun secret dans ce que le relais a vu, ni dans sa garde (PHASE-3, § 2.5).
    let mut aiguilles: Vec<(String, Vec<u8>)> = vec![
        (String::from("le secret d'entrée"), secret.octets().to_vec()),
        (String::from("la psk"), secret.psk().to_vec()),
        (
            String::from("la clé de garde G"),
            cle_garde.octets().to_vec(),
        ),
        (
            String::from("la clé privée de l'autorité"),
            paire.privee().to_vec(),
        ),
    ];
    let mut par_sorte: BTreeMap<&'static str, usize> = BTreeMap::new();
    par_sorte.insert("cles", aiguilles.len());
    for (k, j) in jetons.iter().enumerate() {
        aiguilles.push((format!("le jeton du téléphone {k}"), j.to_vec()));
    }
    par_sorte.insert("jetons", jetons.len());
    let gestes: Vec<&Vec<u8>> = bilan
        .historique
        .octets_des_gestes
        .iter()
        .filter(|o| o.len() >= 8)
        .collect();
    for (k, o) in gestes.iter().enumerate() {
        aiguilles.push((format!("le geste {k} ({} octets)", o.len()), (*o).clone()));
    }
    par_sorte.insert("gestes", gestes.len());
    for (k, m) in messages_de_geste.iter().enumerate() {
        aiguilles.push((
            format!("le message du geste {k} ({} octets)", m.len()),
            m.clone(),
        ));
    }
    par_sorte.insert("messages de geste", messages_de_geste.len());
    for (k, v) in vues.iter().enumerate() {
        aiguilles.push((format!("la vue {k} ({} octets)", v.len()), v.clone()));
    }
    par_sorte.insert("vues", vues.len());
    for (k, m) in messages_de_vue.iter().enumerate() {
        aiguilles.push((
            format!("le message de la vue {k} ({} octets)", m.len()),
            m.clone(),
        ));
    }
    par_sorte.insert("messages de vue", messages_de_vue.len());
    bilan.aiguilles = par_sorte;
    let mut fouille = Fouille::new(aiguilles);
    for chemin in &enregistrements {
        let nom = chemin
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        lire_l_enregistrement(chemin, |_, _, octets| fouille.fouiller(&nom, octets))?;
    }
    for (c, o) in &disque {
        fouille.fouiller(&format!("la garde, case {c:?}"), o);
    }
    bilan.secrets = (fouille.aiguilles.len(), fouille.octets);
    for t in &fouille.trouvees {
        bilan.fautes.push(format!("le relais voit un secret : {t}"));
    }
    Ok(bilan)
}

impl core::fmt::Display for Bilan {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut pannes = String::new();
        for (k, n) in &self.pannes {
            let _: core::fmt::Result = write!(pannes, "{k} {n}, ");
        }
        writeln!(f, "jeu : {}", self.jeu)?;
        writeln!(
            f,
            "  {} téléphone(s) ; {} images ; {} redémarrage(s) de l'autorité ; {} relais tué(s) ; pannes : {}",
            self.telephones, self.images, self.redemarrages, self.relais_tues, pannes
        )?;
        writeln!(
            f,
            "  gestes : {} émis, {} accusés, {} écrits ; {} tenus écrits ; {} phase(s) terminale(s) vues",
            self.gestes_emis,
            self.gestes_accuses,
            self.historique.gestes.len(),
            self.historique.tenus,
            self.terminales
        )?;
        writeln!(
            f,
            "  chaîne : {} cases, {} points, {} lignes ; {} entrées écartées ; {} sauts",
            self.historique.cases,
            self.historique.points,
            self.historique.lignes,
            self.historique.ecartees,
            self.historique.sauts
        )?;
        writeln!(f, "  autorités : {:?}", self.comptes)?;
        let mut sortes = String::new();
        for (k, n) in &self.aiguilles {
            let _: core::fmt::Result = write!(sortes, "{k} {n}, ");
        }
        writeln!(
            f,
            "  secrets : {} aiguilles cherchées ({}) dans {} octets vus par le relais",
            self.secrets.0,
            sortes.trim_end_matches(", "),
            self.secrets.1
        )?;
        if let Some((fils, rss)) = self.relais_max {
            writeln!(
                f,
                "  relais (M15) : {fils} fils au plus, {rss} Kio résidents au plus"
            )?;
        }
        Ok(())
    }
}
