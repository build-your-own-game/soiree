//! Le cœur du relais, **sans entrées-sorties** (PHASE-3, § 2.5 ; SAL-01) : les salles,
//! leurs liens et leurs rôles, le routage des trames qu'il ne sait pas lire, les refus,
//! les échéances et les battements. Le temps y entre en paramètre, absolu, en
//! microsecondes depuis le lancement du relais (SAL-04) ; les tests le font tourner sur
//! une horloge simulée qui saute d'échéance en échéance (SAL-09).
//!
//! La coque ([`crate::coque`]) lui donne ce qui arrive (un lien ouvert, une trame, un
//! battement rendu, une connexion fermée, ce que la garde a fait sur disque, l'heure) et
//! fait ce qu'il rend ([`Sortie`]) : envoyer, battre, fermer, écrire ou lire la garde.
//!
//! # Ce que l'autorité et les appareils attendent du relais
//!
//! Le relais fait ce que fait `salle::simulation::RelaisEnMemoire`, le modèle sur lequel
//! l'autorité et les appareils sont éprouvés (PHASE-3, § 2.5) : après l'accueil d'un
//! appareil, `AutoritePresente` ou `AutoriteAbsente` ; à l'autorité qui se présente, un
//! `Ouvert` pour chaque appareil déjà là, et `AutoritePresente` à chacun ; à la fermeture
//! du lien de l'autorité, `AutoriteAbsente` à chaque appareil (SAL-23) ; `Ferme` à
//! l'autorité quand le lien d'un appareil se ferme, y compris après son `Couper` ;
//! `Ecrit` une fois la case écrite, dans l'ordre des `Ecrire` (la coque le garantit :
//! un seul fil fait la garde, dans l'ordre).
//!
//! # Une seule autorité par salle
//!
//! **La plus récente gagne**, dans l'ordre où le relais a ouvert les liens (P36, SAL-18,
//! SAL-19) : l'ancienne reçoit `Refus(Remplacee)` sur ses deux liens ; un bonjour
//! d'autorité venu d'un lien plus ancien que celui de l'autorité courante est refusé de
//! même ; une garde qui nomme une autorité passée aussi, et rien de ce qu'une garde
//! passée envoie ne s'écrit (SAL-20).

use std::collections::{BTreeMap, BTreeSet};

use liaison::{
    AdresseAnnoncee, AdresseIp, BATTEMENT_MS, Case, CauseFermeture, Classe, CodeSalle, DuRelais,
    LIENS_MAX_SANS_AUTORITE, Lien, PROTOCOLE_RELAIS, RefusRelais, Role, SILENCE_MAX_MS, TRAME_MAX,
    VersRelais, protocole_d_un_bonjour,
};

use crate::file::Place;

/// Un lien sans `Bonjour` en 10 s est fermé (choisi, PHASE-3, § 2.5 ; SAL-49).
pub const BONJOUR_MAX_US: u64 = 10_000_000;

/// La plus grande case de garde qu'un `Ecrire` porte : 64 Mio (choisi). La case de point
/// de Tier list au pire mesuré fait 16,8 Mio (M3, lot 2), au-delà de [`TRAME_MAX`] : le
/// lien de la garde a donc sa propre borne (PHASE-3, § 2.4, décidé au lot 5), et la
/// garde ne découpe pas un point.
pub const CASE_MAX: usize = 64 * 1024 * 1024;

/// Ce que le relais sait de la provenance d'une connexion, jugée par la coque avant le
/// WebSocket (`crate::http`, SAL-33).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// La boucle locale, d'un `Host` local, d'une `Origin` de l'écran (ou sans `Origin`,
    /// pour les outils natifs du banc), sans en-tête de mandataire : tous les rôles.
    BoucleLocale,
    /// Le réseau, d'une `Origin` qu'il annonce : le rôle `Appareil` seulement.
    Reseau,
}

/// Les preuves d'autorité que le relais garde, salle par salle (P36, P41). Lues à
/// chaque bonjour d'autorité : `soiree-relais oublier` les efface sur disque, et le
/// relais qui tourne le voit.
pub trait Preuves {
    /// La preuve enregistrée pour cette salle, s'il y en a une.
    fn preuve(&mut self, salle: CodeSalle) -> Option<[u8; 32]>;
    /// Enregistre la preuve de la première autorité d'une salle. `false` si elle ne s'est
    /// pas écrite.
    fn enregistrer(&mut self, salle: CodeSalle, preuve: [u8; 32]) -> bool;
}

/// Des preuves en mémoire, pour les tests du cœur.
#[derive(Debug, Default, Clone)]
pub struct PreuvesEnMemoire(pub BTreeMap<CodeSalle, [u8; 32]>);

impl Preuves for PreuvesEnMemoire {
    fn preuve(&mut self, salle: CodeSalle) -> Option<[u8; 32]> {
        self.0.get(&salle).copied()
    }

    fn enregistrer(&mut self, salle: CodeSalle, preuve: [u8; 32]) -> bool {
        self.0.insert(salle, preuve);
        true
    }
}

/// Le rôle qu'un lien a pris : la coque donne au lien de la garde sa propre borne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleAdmis {
    /// L'autorité.
    Autorite,
    /// Sa garde.
    Garde,
    /// Un appareil.
    Appareil,
}

/// Une opération sur la garde d'une salle, que la coque fait sur disque, dans l'ordre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpGarde {
    /// Écrire une case (fichier temporaire, synchronisé, renommé, dossier synchronisé).
    Ecrire {
        /// La case.
        case: Case,
        /// Ses octets, chiffrés.
        octets: Vec<u8>,
    },
    /// Lister les cases.
    Lister,
    /// Lire une case.
    Lire {
        /// La case.
        case: Case,
    },
    /// Effacer les cases d'avant `avant`.
    Effacer {
        /// Le plus vieux numéro gardé.
        avant: u64,
    },
}

/// Ce que la garde a fait sur disque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fait {
    /// La case est écrite et synchronisée (SAL-26).
    Ecrit(Case),
    /// Les cases gardées.
    Cases(Vec<Case>),
    /// Une case lue.
    Lu(Case, Vec<u8>),
    /// La case n'existe pas.
    Absente(Case),
    /// Les cases d'avant sont effacées.
    Efface,
    /// L'opération est refusée : la garde est pleine, ou le disque a échoué.
    Refus(RefusRelais),
}

/// Ce que le cœur demande à la coque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sortie {
    /// Envoyer une trame sur un lien : dans la file fiable (`place` vide), ou à une place
    /// remplaçable.
    Envoyer {
        /// Le lien.
        lien: Lien,
        /// La place remplaçable, ou rien pour la file fiable.
        place: Option<Place>,
        /// Le message encodé, un [`DuRelais`].
        octets: Vec<u8>,
    },
    /// Un battement sur ce lien (SAL-43).
    Battre {
        /// Le lien.
        lien: Lien,
    },
    /// Fermer ce lien, après ce qui attend déjà dans sa file (un refus, SAL-148).
    Fermer {
        /// Le lien.
        lien: Lien,
    },
    /// Le lien a pris un rôle.
    Admis {
        /// Le lien.
        lien: Lien,
        /// Son rôle.
        role: RoleAdmis,
    },
    /// Une opération sur la garde d'une salle ; sa réponse revient par
    /// [`Coeur::garde_faite`].
    Garde {
        /// La salle.
        salle: CodeSalle,
        /// Le lien de la garde qui l'a demandée.
        lien: Lien,
        /// L'opération.
        op: OpGarde,
    },
}

/// Ce qu'un lien est pour le relais.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatLien {
    /// Ouvert, sans bonjour encore.
    Attente,
    /// L'autorité de cette salle.
    Autorite(CodeSalle),
    /// La garde de cette salle.
    Garde(CodeSalle),
    /// Un appareil de cette salle.
    Appareil(CodeSalle),
    /// Fermé : plus rien de lui ne compte (SAL-20).
    Ferme,
}

#[derive(Debug, Clone, Copy)]
struct LienRelais {
    ouvert_us: u64,
    recu_us: u64,
    battu_us: u64,
    provenance: Provenance,
    adresse_jointe: AdresseIp,
    etat: EtatLien,
}

#[derive(Debug, Clone)]
struct Salle {
    autorite: Option<Lien>,
    garde: Option<Lien>,
    /// Le lien de l'autorité la plus récente qui s'est présentée.
    plus_recente: Option<Lien>,
    appareils: BTreeSet<Lien>,
    liens_max: u16,
}

impl Salle {
    const fn neuve() -> Salle {
        Salle {
            autorite: None,
            garde: None,
            plus_recente: None,
            appareils: BTreeSet::new(),
            liens_max: LIENS_MAX_SANS_AUTORITE,
        }
    }
}

/// Le cœur du relais.
#[derive(Debug)]
pub struct Coeur<P: Preuves> {
    epoque: [u8; 16],
    adresses: Vec<AdresseAnnoncee>,
    preuves: P,
    liens: BTreeMap<Lien, LienRelais>,
    salles: BTreeMap<CodeSalle, Salle>,
    sorties: Vec<Sortie>,
}

fn encoder(m: &DuRelais<'_>) -> Option<Vec<u8>> {
    m.encoder().ok()
}

const fn place_de(origine: Lien, classe: Classe) -> Option<Place> {
    match classe {
        Classe::Fiable => None,
        Classe::Remplacable(canal) => Some(Place {
            origine: origine.0,
            canal,
        }),
    }
}

fn depuis(t_us: u64, avant_us: u64) -> u64 {
    t_us.saturating_sub(avant_us)
}

impl<P: Preuves> Coeur<P> {
    /// Un cœur neuf : son époque, tirée neuve à ce lancement (SAL-25), les adresses qu'il
    /// annonce, et ses preuves.
    pub fn new(epoque: [u8; 16], adresses: Vec<AdresseAnnoncee>, preuves: P) -> Coeur<P> {
        Coeur {
            epoque,
            adresses,
            preuves,
            liens: BTreeMap::new(),
            salles: BTreeMap::new(),
            sorties: Vec::new(),
        }
    }

    /// Ce qu'il faut faire, depuis le dernier appel.
    pub fn sorties(&mut self) -> Vec<Sortie> {
        core::mem::take(&mut self.sorties)
    }

    /// Ce qu'est un lien, s'il est connu.
    pub fn etat(&self, lien: Lien) -> Option<EtatLien> {
        self.liens.get(&lien).map(|l| l.etat)
    }

    /// Les liens connus, fermés en attente de leur fin compris.
    pub fn liens(&self) -> usize {
        self.liens.len()
    }

    /// Le lien de l'autorité d'une salle.
    pub fn autorite(&self, salle: CodeSalle) -> Option<Lien> {
        self.salles.get(&salle).and_then(|s| s.autorite)
    }

    /// Les appareils d'une salle.
    pub fn appareils(&self, salle: CodeSalle) -> usize {
        self.salles.get(&salle).map_or(0, |s| s.appareils.len())
    }

    /// La borne des appareils d'une salle.
    pub fn liens_max(&self, salle: CodeSalle) -> u16 {
        self.salles
            .get(&salle)
            .map_or(LIENS_MAX_SANS_AUTORITE, |s| s.liens_max)
    }

    fn envoyer(&mut self, lien: Lien, place: Option<Place>, m: &DuRelais<'_>) {
        if let Some(octets) = encoder(m) {
            self.sorties.push(Sortie::Envoyer {
                lien,
                place,
                octets,
            });
        }
    }

    fn accueillir(&mut self, lien: Lien) {
        let m = DuRelais::Accueil {
            epoque: self.epoque,
            lien,
            adresses: self.adresses.clone(),
        };
        self.envoyer(lien, None, &m);
    }

    /// Un refus typé et définitif, puis la fermeture du lien (SAL-42, SAL-148).
    fn refuser(&mut self, lien: Lien, r: RefusRelais) {
        if self.etat(lien).is_none_or(|e| e == EtatLien::Ferme) {
            return;
        }
        self.envoyer(lien, None, &DuRelais::Refus(r));
        self.fermer_lien(lien, CauseFermeture::Coupe);
    }

    /// Ferme un lien de l'intérieur : la coque ferme sa connexion après ce qui attend, et
    /// chacun l'apprend selon son rôle.
    fn fermer_lien(&mut self, lien: Lien, cause: CauseFermeture) {
        let Some(l) = self.liens.get_mut(&lien) else {
            return;
        };
        let etat = l.etat;
        if etat == EtatLien::Ferme {
            return;
        }
        l.etat = EtatLien::Ferme;
        self.sorties.push(Sortie::Fermer { lien });
        match etat {
            EtatLien::Appareil(code) => {
                let autorite = self.salles.get_mut(&code).and_then(|s| {
                    s.appareils.remove(&lien);
                    s.autorite
                });
                if let Some(a) = autorite {
                    self.envoyer(a, None, &DuRelais::Ferme { lien, cause });
                }
            }
            EtatLien::Autorite(code) => {
                let appareils: Vec<Lien> = match self.salles.get_mut(&code) {
                    Some(s) if s.autorite == Some(lien) => {
                        s.autorite = None;
                        s.appareils.iter().copied().collect()
                    }
                    Some(_) | None => Vec::new(),
                };
                // Quand l'autorité s'arrête, chaque appareil l'apprend (SAL-23).
                for a in appareils {
                    self.envoyer(a, None, &DuRelais::AutoriteAbsente);
                }
            }
            EtatLien::Garde(code) => {
                if let Some(s) = self.salles.get_mut(&code)
                    && s.garde == Some(lien)
                {
                    s.garde = None;
                }
            }
            EtatLien::Attente | EtatLien::Ferme => {}
        }
    }

    /// Une connexion est ouverte, et admise : son lien, numéroté par la coque dans
    /// l'ordre d'ouverture (SAL-19), sa provenance, l'adresse locale qu'elle a jointe
    /// (SAL-75).
    pub fn ouvrir(
        &mut self,
        t_us: u64,
        lien: Lien,
        provenance: Provenance,
        adresse_jointe: AdresseIp,
    ) {
        self.liens.insert(
            lien,
            LienRelais {
                ouvert_us: t_us,
                recu_us: t_us,
                battu_us: t_us,
                provenance,
                adresse_jointe,
                etat: EtatLien::Attente,
            },
        );
    }

    /// Un battement rendu (un `Pong`) : le lien est vivant (SAL-43).
    pub fn vivant(&mut self, t_us: u64, lien: Lien) {
        if let Some(l) = self.liens.get_mut(&lien) {
            l.recu_us = l.recu_us.max(t_us);
        }
    }

    /// La connexion d'un lien s'est fermée, ou la coque l'a fermée : le lien est oublié.
    pub fn ferme(&mut self, lien: Lien, cause: CauseFermeture) {
        self.fermer_lien(lien, cause);
        let _: Option<LienRelais> = self.liens.remove(&lien);
        self.sorties
            .retain(|s| !matches!(s, Sortie::Fermer { lien: l } if *l == lien));
    }

    /// La file d'un appareil se remplit : son autorité l'apprend (PHASE-3, § 2.5).
    pub fn encombre(&mut self, lien: Lien, octets_en_attente: u32) {
        let Some(EtatLien::Appareil(code)) = self.etat(lien) else {
            return;
        };
        if let Some(a) = self.autorite(code) {
            self.envoyer(
                a,
                None,
                &DuRelais::Encombre {
                    lien,
                    octets_en_attente,
                },
            );
        }
    }

    /// L'heure passe : les liens sans bonjour en 10 s (SAL-49) et les liens muets depuis
    /// `SILENCE_MAX_MS` (SAL-43) se ferment ; chaque lien bat toutes les `BATTEMENT_MS`.
    pub fn tic(&mut self, t_us: u64) {
        let silence = u64::from(SILENCE_MAX_MS).saturating_mul(1000);
        let battement = u64::from(BATTEMENT_MS).saturating_mul(1000);
        let mut a_fermer = Vec::new();
        let mut a_battre = Vec::new();
        for (lien, l) in &mut self.liens {
            match l.etat {
                EtatLien::Ferme => continue,
                EtatLien::Attente if depuis(t_us, l.ouvert_us) >= BONJOUR_MAX_US => {
                    a_fermer.push(*lien);
                    continue;
                }
                EtatLien::Attente
                | EtatLien::Autorite(_)
                | EtatLien::Garde(_)
                | EtatLien::Appareil(_) => {}
            }
            if depuis(t_us, l.recu_us) > silence {
                a_fermer.push(*lien);
                continue;
            }
            if depuis(t_us, l.battu_us) >= battement {
                l.battu_us = t_us;
                a_battre.push(*lien);
            }
        }
        for lien in a_battre {
            self.sorties.push(Sortie::Battre { lien });
        }
        for lien in a_fermer {
            self.fermer_lien(lien, CauseFermeture::Muet);
        }
    }

    /// Une trame d'un lien, reçue à `recu_us`, traitée à `maintenant_us`.
    pub fn recevoir(&mut self, recu_us: u64, maintenant_us: u64, lien: Lien, octets: &[u8]) {
        let Some(l) = self.liens.get_mut(&lien) else {
            return;
        };
        l.recu_us = l.recu_us.max(recu_us);
        let etat = l.etat;
        match etat {
            EtatLien::Ferme => {}
            EtatLien::Attente => self.premier_message(lien, octets),
            EtatLien::Autorite(code) => self.de_l_autorite(maintenant_us, lien, code, octets),
            EtatLien::Garde(code) => self.de_la_garde(maintenant_us, lien, code, octets),
            EtatLien::Appareil(code) => {
                self.d_un_appareil(recu_us, maintenant_us, lien, code, octets);
            }
        }
    }

    /// Le premier message d'un lien : un bonjour, ou rien de bon.
    fn premier_message(&mut self, lien: Lien, octets: &[u8]) {
        match VersRelais::decoder(octets) {
            Ok(VersRelais::Bonjour {
                protocole,
                salle,
                role,
            }) => {
                if protocole == PROTOCOLE_RELAIS {
                    self.bonjour(lien, salle, role);
                } else {
                    self.refuser(
                        lien,
                        RefusRelais::Protocole {
                            relais: PROTOCOLE_RELAIS,
                        },
                    );
                }
            }
            Ok(_) => self.fermer_lien(lien, CauseFermeture::Illisible),
            Err(_) => match protocole_d_un_bonjour(octets) {
                // Un bonjour d'un autre protocole se reconnaît même cassé (SAL-40).
                Some(p) if p != PROTOCOLE_RELAIS => self.refuser(
                    lien,
                    RefusRelais::Protocole {
                        relais: PROTOCOLE_RELAIS,
                    },
                ),
                Some(_) | None => self.fermer_lien(lien, CauseFermeture::Illisible),
            },
        }
    }

    fn bonjour(&mut self, lien: Lien, code: CodeSalle, role: Role) {
        let Some(l) = self.liens.get(&lien).copied() else {
            return;
        };
        match role {
            Role::Autorite { preuve } => self.bonjour_autorite(lien, l, code, preuve),
            Role::Garde { preuve, autorite } => {
                self.bonjour_garde(lien, l, code, preuve, autorite);
            }
            Role::Appareil => self.bonjour_appareil(lien, l, code),
        }
    }

    fn bonjour_autorite(&mut self, lien: Lien, l: LienRelais, code: CodeSalle, preuve: [u8; 32]) {
        // Seul l'ordinateur de la soirée tient le rôle de la télé (SAL-33).
        if l.provenance != Provenance::BoucleLocale {
            return self.refuser(lien, RefusRelais::HorsBoucleLocale);
        }
        match self.preuves.preuve(code) {
            Some(p) if p != preuve => return self.refuser(lien, RefusRelais::PreuveFausse),
            Some(_) => {}
            None => {
                if !self.preuves.enregistrer(code, preuve) {
                    return self.refuser(lien, RefusRelais::GardePleine);
                }
            }
        }
        let salle = self.salles.entry(code).or_insert_with(Salle::neuve);
        // « Le plus récent » se lit dans l'ordre d'ouverture des liens (SAL-19).
        if salle.plus_recente.is_some_and(|r| lien < r) {
            return self.refuser(lien, RefusRelais::Remplacee);
        }
        let anciens = [salle.autorite.take(), salle.garde.take()];
        salle.plus_recente = Some(lien);
        // La plus récente gagne (P36, SAL-18) : l'ancienne l'apprend sur ses deux liens.
        // Les appareils, eux, ne voient pas d'absence : la nouvelle se présente aussitôt.
        for ancien in anciens.into_iter().flatten() {
            self.refuser(ancien, RefusRelais::Remplacee);
        }
        let appareils: Vec<Lien> = self
            .salles
            .get_mut(&code)
            .map(|s| {
                s.autorite = Some(lien);
                s.appareils.iter().copied().collect()
            })
            .unwrap_or_default();
        if let Some(x) = self.liens.get_mut(&lien) {
            x.etat = EtatLien::Autorite(code);
        }
        self.accueillir(lien);
        self.sorties.push(Sortie::Admis {
            lien,
            role: RoleAdmis::Autorite,
        });
        for a in appareils {
            let adresse_jointe = self
                .liens
                .get(&a)
                .map_or(AdresseIp::V4([0; 4]), |x| x.adresse_jointe);
            self.envoyer(
                lien,
                None,
                &DuRelais::Ouvert {
                    lien: a,
                    adresse_jointe,
                },
            );
            self.envoyer(a, None, &DuRelais::AutoritePresente);
        }
    }

    fn bonjour_garde(
        &mut self,
        lien: Lien,
        l: LienRelais,
        code: CodeSalle,
        preuve: [u8; 32],
        autorite: Lien,
    ) {
        if l.provenance != Provenance::BoucleLocale {
            return self.refuser(lien, RefusRelais::HorsBoucleLocale);
        }
        if self.preuves.preuve(code) != Some(preuve) {
            return self.refuser(lien, RefusRelais::PreuveFausse);
        }
        // Une garde d'une autorité passée n'écrit plus (SAL-20).
        let courante = self.autorite(code);
        if courante != Some(autorite) {
            return self.refuser(lien, RefusRelais::Remplacee);
        }
        let ancienne = self.salles.get_mut(&code).and_then(|s| s.garde.take());
        if let Some(a) = ancienne {
            self.refuser(a, RefusRelais::Remplacee);
        }
        if let Some(s) = self.salles.get_mut(&code) {
            s.garde = Some(lien);
        }
        if let Some(x) = self.liens.get_mut(&lien) {
            x.etat = EtatLien::Garde(code);
        }
        self.accueillir(lien);
        self.sorties.push(Sortie::Admis {
            lien,
            role: RoleAdmis::Garde,
        });
    }

    fn bonjour_appareil(&mut self, lien: Lien, l: LienRelais, code: CodeSalle) {
        // Une salle connue en mémoire, ou dont la preuve est sur disque : un relais
        // relancé reprend les téléphones avant que l'autorité revienne.
        if !self.salles.contains_key(&code) && self.preuves.preuve(code).is_none() {
            return self.refuser(lien, RefusRelais::SalleInconnue);
        }
        let salle = self.salles.entry(code).or_insert_with(Salle::neuve);
        if salle.appareils.len() >= usize::from(salle.liens_max) {
            return self.refuser(lien, RefusRelais::Pleine);
        }
        salle.appareils.insert(lien);
        let autorite = salle.autorite;
        if let Some(x) = self.liens.get_mut(&lien) {
            x.etat = EtatLien::Appareil(code);
        }
        self.accueillir(lien);
        self.sorties.push(Sortie::Admis {
            lien,
            role: RoleAdmis::Appareil,
        });
        match autorite {
            Some(a) => {
                self.envoyer(
                    a,
                    None,
                    &DuRelais::Ouvert {
                        lien,
                        adresse_jointe: l.adresse_jointe,
                    },
                );
                self.envoyer(lien, None, &DuRelais::AutoritePresente);
            }
            None => self.envoyer(lien, None, &DuRelais::AutoriteAbsente),
        }
    }

    fn echo(&mut self, lien: Lien, t_envoi_us: u64, maintenant_us: u64) {
        self.envoyer(
            lien,
            None,
            &DuRelais::Echo {
                t_envoi_us,
                t_relais_us: maintenant_us,
            },
        );
    }

    fn de_l_autorite(&mut self, maintenant_us: u64, lien: Lien, code: CodeSalle, octets: &[u8]) {
        let Ok(m) = VersRelais::decoder(octets) else {
            return self.fermer_lien(lien, CauseFermeture::Illisible);
        };
        match m {
            VersRelais::Vers {
                lien: vers,
                classe,
                trame,
            } => {
                if trame.len() > TRAME_MAX as usize {
                    return self.refuser(lien, RefusRelais::TropGros);
                }
                if self.etat(vers) == Some(EtatLien::Appareil(code)) {
                    self.envoyer(
                        vers,
                        place_de(Lien(0), classe),
                        &DuRelais::Pour { classe, trame },
                    );
                }
            }
            VersRelais::Couper { lien: vers, .. } => {
                if self.etat(vers) == Some(EtatLien::Appareil(code)) {
                    self.fermer_lien(vers, CauseFermeture::Coupe);
                }
            }
            VersRelais::Limites { liens_max } => {
                if let Some(s) = self.salles.get_mut(&code) {
                    s.liens_max = liens_max;
                }
            }
            VersRelais::Echo { t_envoi_us } => self.echo(lien, t_envoi_us, maintenant_us),
            VersRelais::AuRevoir => self.fermer_lien(lien, CauseFermeture::AuRevoir),
            // Les lignes des flux viennent avec la demande `Ligne` (lot 11) ; le reste
            // n'est pas de ce rôle.
            VersRelais::AjouterLigne { .. }
            | VersRelais::Bonjour { .. }
            | VersRelais::Trame { .. }
            | VersRelais::Ecrire { .. }
            | VersRelais::Lister
            | VersRelais::Lire { .. }
            | VersRelais::Effacer { .. } => {}
        }
    }

    fn de_la_garde(&mut self, maintenant_us: u64, lien: Lien, code: CodeSalle, octets: &[u8]) {
        let Ok(m) = VersRelais::decoder(octets) else {
            return self.fermer_lien(lien, CauseFermeture::Illisible);
        };
        // Rien d'une garde qui n'est plus celle de la salle (SAL-20).
        let courante = self.salles.get(&code).and_then(|s| s.garde) == Some(lien);
        let op = match m {
            VersRelais::Ecrire { case, octets } => {
                if octets.len() > CASE_MAX {
                    return self.refuser(lien, RefusRelais::TropGros);
                }
                Some(OpGarde::Ecrire {
                    case,
                    octets: octets.to_vec(),
                })
            }
            VersRelais::Lister => Some(OpGarde::Lister),
            VersRelais::Lire { case } => Some(OpGarde::Lire { case }),
            VersRelais::Effacer { avant } => Some(OpGarde::Effacer { avant }),
            VersRelais::Echo { t_envoi_us } => {
                self.echo(lien, t_envoi_us, maintenant_us);
                None
            }
            VersRelais::AuRevoir => {
                self.fermer_lien(lien, CauseFermeture::AuRevoir);
                None
            }
            VersRelais::Bonjour { .. }
            | VersRelais::Vers { .. }
            | VersRelais::Trame { .. }
            | VersRelais::Couper { .. }
            | VersRelais::AjouterLigne { .. }
            | VersRelais::Limites { .. } => None,
        };
        if let Some(op) = op
            && courante
        {
            self.sorties.push(Sortie::Garde {
                salle: code,
                lien,
                op,
            });
        }
    }

    fn d_un_appareil(
        &mut self,
        recu_us: u64,
        maintenant_us: u64,
        lien: Lien,
        code: CodeSalle,
        octets: &[u8],
    ) {
        let Ok(m) = VersRelais::decoder(octets) else {
            return self.fermer_lien(lien, CauseFermeture::Illisible);
        };
        match m {
            VersRelais::Trame { classe, trame } => {
                if trame.len() > TRAME_MAX as usize {
                    return self.fermer_lien(lien, CauseFermeture::TropGros);
                }
                if let Some(a) = self.autorite(code) {
                    self.envoyer(
                        a,
                        place_de(lien, classe),
                        &DuRelais::De {
                            lien,
                            classe,
                            trame,
                            recu_us,
                            renvoye_us: maintenant_us,
                        },
                    );
                }
            }
            VersRelais::Echo { t_envoi_us } => self.echo(lien, t_envoi_us, maintenant_us),
            VersRelais::AuRevoir => self.fermer_lien(lien, CauseFermeture::AuRevoir),
            VersRelais::Bonjour { .. }
            | VersRelais::Vers { .. }
            | VersRelais::Couper { .. }
            | VersRelais::Ecrire { .. }
            | VersRelais::Lister
            | VersRelais::Lire { .. }
            | VersRelais::Effacer { .. }
            | VersRelais::AjouterLigne { .. }
            | VersRelais::Limites { .. } => {}
        }
    }

    /// Ce que la garde a fait sur disque, pour le lien qui l'avait demandé. Rien ne va à
    /// une garde qui n'est plus celle de sa salle (SAL-20).
    pub fn garde_faite(&mut self, lien: Lien, fait: Fait) {
        let Some(EtatLien::Garde(code)) = self.etat(lien) else {
            return;
        };
        if self.salles.get(&code).and_then(|s| s.garde) != Some(lien) {
            return;
        }
        match fait {
            Fait::Ecrit(case) => self.envoyer(lien, None, &DuRelais::Ecrit { case }),
            Fait::Cases(cases) => self.envoyer(lien, None, &DuRelais::Cases(cases)),
            Fait::Lu(case, octets) => self.envoyer(
                lien,
                None,
                &DuRelais::Lu {
                    case,
                    octets: &octets,
                },
            ),
            Fait::Absente(_) | Fait::Efface => {}
            Fait::Refus(r) => self.refuser(lien, r),
        }
    }
}
