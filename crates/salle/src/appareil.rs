//! Un appareil (PHASE-3, § 2.10) : le cœur du téléphone, sans sa page.
//!
//! **Sans entrées-sorties**, comme l'autorité : la coque (la page du téléphone au lot 12,
//! le banc natif aux lots 4b et 5) lui donne les trames du relais, l'heure et un hasard,
//! et envoie ce qu'il rend. Il tient :
//!
//! - **le lien** : le bonjour au relais, la poignée de main Noise, puis le bonjour chiffré
//!   à l'autorité, son premier message de transport (SAL-17) ; une autorité qui revient
//!   (`AutoritePresente`) refait la poignée de main ;
//! - **les gestes** : numérotés dans l'onglet à partir de 1, gardés en file jusqu'à leur
//!   accusé durable (SAL-56), renvoyés après chaque accueil au-delà du dernier placé ;
//!   une file pleine se dit ([`Envoi::FilePleine`], SAL-07) ;
//! - **le tenu** : le dernier gagne, numéroté au-dessus de ce que l'accueil dit reçu
//!   (SAL-38, REM-14), redit à sa période (P27) ;
//! - **l'heure de l'autorité**, estimée par écho au plus petit aller-retour (SAL-128) :
//!   `fait` et la trace d'un geste en viennent ;
//! - **ce qu'il reçoit** : la vue entière (SAL-28), les avis, les refus typés et
//!   définitifs (SAL-42), l'attente.
//!
//! Ce que la page conserve d'un rechargement (`sessionStorage`, P26 : le jeton, l'onglet,
//! les gestes en file) reste à travers [`Appareil::recharger`] ; le reste repart de zéro.
//!
//! **En mode banc** (PHASE-3, § 8.1, lots 4b et 5), l'appareil joue tels quels les coups
//! que l'autorité lui envoie (`VersAppareil::Banc`), par numérotation, file, chiffrement
//! et relais. Au lot 12, ils passeront par la page.

use jeu_abi::{Capacites, Instant, Pas};
use liaison::{
    BATTEMENT_MS, Classe, CodeSalle, DuRelais, PROTOCOLE_RELAIS, RefusRelais, Role, VersRelais,
};

use crate::chiffre::{Fragment, Hasard, Initiateur, Session};
use crate::fil::{
    AttenteCommun, CoupBanc, LimitesSalle, PROTOCOLE, RefusSalle, TraceTelephone, VersAppareil,
    VersAutorite,
};
use crate::horloge::{Estimation, instant_de_la_partie};

/// La file d'un appareil avant le premier accueil, qui seul dit `limites.file` : celle de
/// Sidequest, la plus petite des trois jeux (choisi).
pub const FILE_AVANT_ACCUEIL: usize = 32;

/// Le canal remplaçable des tenus : le dernier gagne (SAL-70).
pub const CANAL_TENU: u8 = 0;

/// Le bonjour chiffré se redit s'il reste sans accueil 2 s (choisi) : un accueil perdu
/// pousse l'appareil à répéter sa demande, et rejoindre est idempotent (SAL-29).
pub const BONJOUR_REDIT_US: u64 = 2_000_000;

/// Pourquoi un geste n'est pas parti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Envoi {
    /// La file a `limites.file` gestes qui attendent leur accusé (SAL-07).
    FilePleine,
    /// Le geste dépasse `limites.geste`.
    TropLong,
}

/// Où en est le lien de l'appareil.
enum Lien {
    /// Aucun lien au relais.
    Hors,
    /// Le bonjour envoyé au relais, sans réponse encore.
    Bonjour,
    /// Relié au relais ; l'autorité est absente, ou la poignée de main pas commencée.
    Relie,
    /// La poignée de main commencée.
    Poignee(Box<Initiateur>),
    /// La session ouverte ; accueilli, ou le bonjour chiffré envoyé.
    Session {
        session: Box<Session>,
        accueil: Option<u32>,
    },
    /// Un refus final : l'appareil ne se reconnecte pas seul (SAL-18).
    Fini,
}

/// Un geste qui attend son accusé.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EnFile {
    numero: u64,
    octets: Vec<u8>,
    fait: Instant,
    vu: Pas,
    trace: TraceTelephone,
}

/// Ce que l'appareil a compté.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComptesAppareil {
    /// Les gestes émis.
    pub gestes: u64,
    /// Les gestes envoyés, renvois compris.
    pub envois: u64,
    /// Les gestes refusés faute de place (SAL-07).
    pub files_pleines: u64,
    /// Les tenus envoyés, redites comprises.
    pub tenus: u64,
    /// Les accueils reçus.
    pub accueils: u64,
    /// Les vues reçues.
    pub vues: u64,
    /// Les poignées de main commencées.
    pub poignees: u64,
    /// Les trames de l'autorité qui ne s'ouvrent pas.
    pub illisibles: u64,
}

/// Le cœur d'un téléphone.
pub struct Appareil<H: Hasard> {
    fragment: Fragment,
    salle: CodeSalle,
    jeton: [u8; 32],
    onglet: u64,
    capacites: Capacites,
    hasard: H,
    lien: Lien,
    file: Vec<EnFile>,
    prochain_geste: u64,
    accuse: u64,
    tenu: Option<Vec<u8>>,
    numero_tenu: u64,
    redite_us: Option<u64>,
    prochaine_redite_us: u64,
    limites: Option<LimitesSalle>,
    horloge: Estimation,
    prochain_echo_us: u64,
    vue: Option<(Pas, Vec<u8>)>,
    avis: Vec<Vec<u8>>,
    refus: Option<RefusSalle>,
    refus_relais: Option<RefusRelais>,
    attente: Option<AttenteCommun>,
    sorties: Vec<Vec<u8>>,
    comptes: ComptesAppareil,
    banc: bool,
    /// Quand redire le bonjour, s'il reste sans accueil.
    redire_bonjour_us: Option<u64>,
}

impl<H: Hasard> Appareil<H> {
    /// Un appareil neuf : le fragment du QR, le code de salle de l'adresse, son jeton
    /// (tiré une fois, gardé dans son `localStorage`) et son onglet. `redite_us` : la
    /// période de redite du tenu (`liaison::periode_de_redite`), aucune sinon.
    pub fn new(
        fragment: Fragment,
        salle: CodeSalle,
        (jeton, onglet): ([u8; 32], u64),
        capacites: Capacites,
        redite_us: Option<u64>,
        hasard: H,
    ) -> Appareil<H> {
        Appareil {
            fragment,
            salle,
            jeton,
            onglet,
            capacites,
            hasard,
            lien: Lien::Hors,
            file: Vec::new(),
            prochain_geste: 1,
            accuse: 0,
            tenu: None,
            numero_tenu: 0,
            redite_us,
            prochaine_redite_us: 0,
            limites: None,
            horloge: Estimation::default(),
            prochain_echo_us: 0,
            vue: None,
            avis: Vec::new(),
            refus: None,
            refus_relais: None,
            attente: None,
            sorties: Vec::new(),
            comptes: ComptesAppareil::default(),
            banc: true,
            redire_bonjour_us: None,
        }
    }

    /// L'appareil ne joue plus les coups que le banc lui envoie : la fin d'une soirée du
    /// banc, où tout ce qui attend doit être accusé.
    pub fn taire_le_banc(&mut self) {
        self.banc = false;
    }

    /// La page se recharge : même jeton, même onglet, mêmes gestes en file
    /// (`sessionStorage`, P26) ; le lien, la session, la vue, l'horloge et la
    /// numérotation des tenus repartent de zéro. L'accueil réaligne les tenus (SAL-38).
    pub fn recharger(&mut self) {
        self.lien = Lien::Hors;
        self.tenu = None;
        self.numero_tenu = 0;
        self.prochaine_redite_us = 0;
        self.limites = None;
        self.horloge = Estimation::default();
        self.prochain_echo_us = 0;
        self.vue = None;
        self.avis.clear();
        self.refus = None;
        self.refus_relais = None;
        self.attente = None;
        self.sorties.clear();
    }

    /// « Reprendre ici » : après un remplacement, l'appareil peut se relier de nouveau,
    /// sur un geste du joueur et jamais de lui-même (SAL-18).
    pub fn reprendre_ici(&mut self) {
        if matches!(self.lien, Lien::Fini) {
            self.lien = Lien::Hors;
            self.refus = None;
        }
    }

    /// Ce qu'il faut envoyer au relais, depuis le dernier appel.
    pub fn sorties(&mut self) -> Vec<Vec<u8>> {
        core::mem::take(&mut self.sorties)
    }

    /// Ce que l'appareil a compté.
    pub const fn comptes(&self) -> ComptesAppareil {
        self.comptes
    }

    /// Son onglet.
    pub const fn onglet(&self) -> u64 {
        self.onglet
    }

    /// Le dernier geste accusé.
    pub const fn accuse(&self) -> u64 {
        self.accuse
    }

    /// Le dernier numéro de geste émis.
    pub const fn dernier_geste(&self) -> u64 {
        self.prochain_geste.saturating_sub(1)
    }

    /// Les gestes qui attendent leur accusé.
    pub fn en_file(&self) -> usize {
        self.file.len()
    }

    /// Le dernier numéro de tenu envoyé.
    pub const fn numero_tenu(&self) -> u64 {
        self.numero_tenu
    }

    /// La dernière vue reçue, et son pas.
    pub fn vue(&self) -> Option<(Pas, &[u8])> {
        self.vue.as_ref().map(|(p, v)| (*p, v.as_slice()))
    }

    /// Les avis reçus.
    pub fn avis(&self) -> &[Vec<u8>] {
        &self.avis
    }

    /// Le refus final de l'autorité, s'il y en a un.
    pub const fn refus(&self) -> Option<RefusSalle> {
        self.refus
    }

    /// Le refus du relais, s'il y en a un.
    pub const fn refus_relais(&self) -> Option<RefusRelais> {
        self.refus_relais
    }

    /// Ce que le commun fait attendre.
    pub const fn attente(&self) -> Option<AttenteCommun> {
        self.attente
    }

    /// Le numéro de la session de son dernier accueil.
    pub const fn session(&self) -> Option<u32> {
        match self.lien {
            Lien::Session { accueil, .. } => accueil,
            Lien::Hors | Lien::Bonjour | Lien::Relie | Lien::Poignee(_) | Lien::Fini => None,
        }
    }

    /// L'appareil est-il accueilli par l'autorité ?
    pub const fn accueilli(&self) -> bool {
        matches!(
            self.lien,
            Lien::Session {
                accueil: Some(_),
                ..
            }
        )
    }

    /// L'appareil a-t-il reçu un refus final ?
    pub const fn fini(&self) -> bool {
        matches!(self.lien, Lien::Fini)
    }

    /// L'appareil a-t-il un lien au relais (ou en demande-t-il un) ?
    pub const fn relie(&self) -> bool {
        !matches!(self.lien, Lien::Hors | Lien::Fini)
    }

    fn vers_relais(&mut self, m: &VersRelais<'_>) {
        if let Ok(o) = m.encoder() {
            self.sorties.push(o);
        }
    }

    fn vers_autorite(&mut self, classe: Classe, m: &VersAutorite<'_>) {
        let Ok(clair) = m.encoder() else {
            return;
        };
        let Lien::Session { session, .. } = &mut self.lien else {
            return;
        };
        if let Ok(trame) = session.sceller(classe, &clair) {
            self.vers_relais(&VersRelais::Trame {
                classe,
                trame: &trame,
            });
        }
    }

    /// Ouvre un lien au relais : le bonjour d'un appareil.
    pub fn relier(&mut self) {
        if matches!(self.lien, Lien::Fini) {
            return;
        }
        self.lien = Lien::Bonjour;
        self.vers_relais(&VersRelais::Bonjour {
            protocole: PROTOCOLE_RELAIS,
            salle: self.salle,
            role: Role::Appareil,
        });
    }

    /// Le lien au relais est perdu : rien de ce qui attend n'est perdu.
    pub fn couper(&mut self) {
        if !matches!(self.lien, Lien::Fini) {
            self.lien = Lien::Hors;
        }
    }

    fn commencer_la_poignee(&mut self) {
        match Initiateur::commencer(&self.fragment, &mut self.hasard) {
            Ok((initiateur, premier)) => {
                self.comptes.poignees = self.comptes.poignees.saturating_add(1);
                self.lien = Lien::Poignee(Box::new(initiateur));
                self.vers_relais(&VersRelais::Trame {
                    classe: Classe::Fiable,
                    trame: &premier,
                });
            }
            Err(_) => self.lien = Lien::Relie,
        }
    }

    /// Une trame du relais.
    pub fn recevoir(&mut self, t_us: u64, octets: &[u8]) {
        let Ok(m) = DuRelais::decoder(octets) else {
            return;
        };
        match m {
            DuRelais::Accueil { .. } => {
                if matches!(self.lien, Lien::Bonjour) {
                    self.lien = Lien::Relie;
                }
            }
            DuRelais::Refus(r) => {
                self.refus_relais = Some(r);
                self.lien = Lien::Hors;
            }
            DuRelais::AutoriteAbsente => {
                self.attente = Some(AttenteCommun::Reprise);
                if !matches!(self.lien, Lien::Hors | Lien::Bonjour | Lien::Fini) {
                    self.lien = Lien::Relie;
                }
            }
            DuRelais::AutoritePresente => {
                if !matches!(self.lien, Lien::Hors | Lien::Bonjour | Lien::Fini) {
                    self.commencer_la_poignee();
                }
            }
            DuRelais::Pour { classe, trame } => self.pour(t_us, classe, trame),
            DuRelais::Ouvert { .. }
            | DuRelais::De { .. }
            | DuRelais::Ferme { .. }
            | DuRelais::Encombre { .. }
            | DuRelais::Echo { .. }
            | DuRelais::Ecrit { .. }
            | DuRelais::Cases(_)
            | DuRelais::Lu { .. } => {}
        }
    }

    fn pour(&mut self, t_us: u64, classe: Classe, trame: &[u8]) {
        match core::mem::replace(&mut self.lien, Lien::Relie) {
            Lien::Poignee(initiateur) => {
                if let Ok(session) = initiateur.finir(trame) {
                    self.lien = Lien::Session {
                        session: Box::new(session),
                        accueil: None,
                    };
                    self.redire_le_bonjour();
                    self.redire_bonjour_us = Some(t_us.saturating_add(BONJOUR_REDIT_US));
                } else {
                    // Une réponse que seule l'autorité du QR sait écrire : celle-ci ne
                    // l'est pas (P12). On recommence.
                    self.comptes.illisibles = self.comptes.illisibles.saturating_add(1);
                    self.commencer_la_poignee();
                }
            }
            Lien::Session {
                mut session,
                accueil,
            } => {
                let ouvert = session.ouvrir(classe, trame);
                self.lien = Lien::Session { session, accueil };
                match ouvert {
                    Ok(clair) => self.message(t_us, &clair),
                    Err(_) => self.comptes.illisibles = self.comptes.illisibles.saturating_add(1),
                }
            }
            autre @ (Lien::Hors | Lien::Bonjour | Lien::Relie | Lien::Fini) => self.lien = autre,
        }
    }

    fn message(&mut self, t_us: u64, clair: &[u8]) {
        let Ok(m) = VersAppareil::decoder(clair) else {
            self.comptes.illisibles = self.comptes.illisibles.saturating_add(1);
            return;
        };
        match m {
            VersAppareil::Accueil {
                session,
                dernier_geste,
                dernier_tenu,
                limites,
                ..
            } => self.accueil(t_us, session, dernier_geste, dernier_tenu, limites),
            VersAppareil::Refus(r) => {
                self.refus = Some(r);
                self.lien = Lien::Fini;
            }
            VersAppareil::Vue { pas, montre } => {
                self.comptes.vues = self.comptes.vues.saturating_add(1);
                self.attente = None;
                self.vue = Some((pas, montre.to_vec()));
            }
            VersAppareil::Avis { octets } => self.avis.push(octets.to_vec()),
            VersAppareil::Ecarte { numero } => self.file.retain(|g| g.numero != numero),
            VersAppareil::Accuse { jusqu_a } => {
                self.accuse = self.accuse.max(jusqu_a);
                self.file.retain(|g| g.numero > jusqu_a);
            }
            VersAppareil::Attente(a) => self.attente = Some(a),
            VersAppareil::Echo { t_us: t, echo_us } => {
                if echo_us == 0 {
                    // Un écho de l'autorité : on y répond tout de suite (SAL-129).
                    self.vers_autorite(Classe::Fiable, &VersAutorite::Echo { t_us, echo_us: t });
                } else {
                    self.horloge.noter(echo_us, t, t_us);
                }
            }
            VersAppareil::Banc(CoupBanc::Geste(o)) => {
                if self.banc {
                    let _: Result<u64, Envoi> = self.geste(t_us, o);
                }
            }
            VersAppareil::Banc(CoupBanc::Tenu(o)) => {
                if self.banc {
                    self.tenir(t_us, o);
                }
            }
            // Le module et les ressources de la page viennent avec elle (lots 10a et 12).
            VersAppareil::Module { .. } | VersAppareil::Ressource { .. } => {}
        }
    }

    fn accueil(
        &mut self,
        t_us: u64,
        session: u32,
        dernier_geste: u64,
        dernier_tenu: Option<u64>,
        limites: LimitesSalle,
    ) {
        if let Lien::Session { accueil, .. } = &mut self.lien {
            *accueil = Some(session);
        }
        self.comptes.accueils = self.comptes.accueils.saturating_add(1);
        self.limites = Some(limites);
        self.attente = None;
        // Les tenus se numérotent au-dessus de ce que l'autorité a reçu (SAL-38).
        self.numero_tenu = self.numero_tenu.max(dernier_tenu.unwrap_or(0));
        // Les gestes au-delà du dernier placé repartent, dans l'ordre (SAL-56).
        let a_renvoyer: Vec<EnFile> = self
            .file
            .iter()
            .filter(|g| g.numero > dernier_geste)
            .cloned()
            .collect();
        for g in a_renvoyer {
            self.envoyer_le_geste(&g);
        }
        // L'état tenu entier, avec un numéro neuf (REM-13).
        if self.tenu.is_some() {
            self.envoyer_le_tenu(t_us);
        }
        self.prochain_echo_us = t_us;
        self.echo(t_us);
    }

    fn envoyer_le_geste(&mut self, g: &EnFile) {
        self.comptes.envois = self.comptes.envois.saturating_add(1);
        self.vers_autorite(
            Classe::Fiable,
            &VersAutorite::Geste {
                onglet: self.onglet,
                numero: g.numero,
                octets: &g.octets,
                fait: g.fait,
                vu: g.vu,
                trace: g.trace,
            },
        );
    }

    fn envoyer_le_tenu(&mut self, t_us: u64) {
        let Some(octets) = self.tenu.clone() else {
            return;
        };
        self.numero_tenu = self.numero_tenu.saturating_add(1);
        self.comptes.tenus = self.comptes.tenus.saturating_add(1);
        let fait = self.instant(t_us);
        self.prochaine_redite_us = self.redite_us.map_or(u64::MAX, |r| t_us.saturating_add(r));
        let numero = self.numero_tenu;
        self.vers_autorite(
            Classe::Remplacable(CANAL_TENU),
            &VersAutorite::Tenu {
                numero,
                octets: &octets,
                fait,
            },
        );
    }

    /// L'instant de la partie maintenant, tel que l'appareil l'estime ; le pas de sa vue
    /// tant qu'aucun écho n'est revenu.
    fn instant(&self, t_us: u64) -> Instant {
        self.horloge.autorite_us(t_us).map_or_else(
            || Instant::de(self.vue.as_ref().map_or(Pas::ZERO, |(p, _)| *p)),
            instant_de_la_partie,
        )
    }

    fn echo(&mut self, t_us: u64) {
        if !self.accueilli() || t_us < self.prochain_echo_us {
            return;
        }
        self.prochain_echo_us = t_us.saturating_add(u64::from(BATTEMENT_MS).saturating_mul(1000));
        self.vers_autorite(Classe::Fiable, &VersAutorite::Echo { t_us, echo_us: 0 });
    }

    /// Un geste : numéroté, mis en file, envoyé si l'appareil est accueilli. Rend son
    /// numéro.
    pub fn geste(&mut self, t_us: u64, octets: &[u8]) -> Result<u64, Envoi> {
        let (file, longueur) = self.limites.map_or((FILE_AVANT_ACCUEIL, u32::MAX), |l| {
            (usize::from(l.file), l.geste)
        });
        if u32::try_from(octets.len()).map_or(true, |n| n > longueur) {
            return Err(Envoi::TropLong);
        }
        if self.file.len() >= file {
            self.comptes.files_pleines = self.comptes.files_pleines.saturating_add(1);
            return Err(Envoi::FilePleine);
        }
        let numero = self.prochain_geste;
        self.prochain_geste = numero.saturating_add(1);
        self.comptes.gestes = self.comptes.gestes.saturating_add(1);
        let evenement = self.horloge.autorite_us(t_us).unwrap_or(0);
        let g = EnFile {
            numero,
            octets: octets.to_vec(),
            fait: self.instant(t_us),
            vu: self.vue.as_ref().map_or(Pas::ZERO, |(p, _)| *p),
            trace: TraceTelephone {
                evenement_us: evenement,
                envoi_us: evenement,
                incertitude_us: self
                    .horloge
                    .incertitude_us()
                    .map_or(u32::MAX, |i| u32::try_from(i).unwrap_or(u32::MAX)),
            },
        };
        if self.accueilli() {
            self.envoyer_le_geste(&g);
        }
        self.file.push(g);
        Ok(numero)
    }

    /// Renvoie chaque geste en file, déjà placé ou non : ce que ferait une page qui se
    /// trompe, ou un appareil hostile. Le banc s'en sert pour éprouver le
    /// dédoublonnage de l'autorité (SAL-57) ; une page ne l'appelle jamais.
    pub fn renvoyer_la_file(&mut self) {
        if !self.accueilli() {
            return;
        }
        let file = self.file.clone();
        for g in &file {
            self.envoyer_le_geste(g);
        }
    }

    /// L'état tenu change : le dernier gagne.
    pub fn tenir(&mut self, t_us: u64, octets: &[u8]) {
        self.tenu = Some(octets.to_vec());
        if self.accueilli() {
            self.envoyer_le_tenu(t_us);
        }
    }

    /// Le bonjour chiffré, le premier message de transport (SAL-17), dit ou redit sur la
    /// session ouverte. L'autorité y répond par le même accueil (SAL-29).
    pub fn redire_le_bonjour(&mut self) {
        let jeton = self.jeton;
        self.vers_autorite(
            Classe::Fiable,
            &VersAutorite::Bonjour {
                protocole: PROTOCOLE,
                jeton,
                onglet: self.onglet,
                capacites: self.capacites,
                module_connu: None,
            },
        );
    }

    /// Un réveil : l'écho, la redite du tenu, et le bonjour redit s'il reste sans accueil.
    pub fn reveiller(&mut self, t_us: u64) {
        if let Some(r) = self.redire_bonjour_us
            && t_us >= r
        {
            if matches!(self.lien, Lien::Session { accueil: None, .. }) {
                self.redire_le_bonjour();
                self.redire_bonjour_us = Some(t_us.saturating_add(BONJOUR_REDIT_US));
            } else {
                self.redire_bonjour_us = None;
            }
        }
        self.echo(t_us);
        if self.accueilli() && self.tenu.is_some() && t_us >= self.prochaine_redite_us {
            self.envoyer_le_tenu(t_us);
        }
    }
}
