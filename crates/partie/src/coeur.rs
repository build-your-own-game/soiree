//! L'état du cœur : ce qu'un point de reprise garde à côté de la mémoire de la
//! définitive (§ 4.5), et que le rejeu du journal reconstruit à l'identique.
//!
//! Rien ici ne vient de l'horloge ni du réseau : tout change par une arrivée jouée
//! à son pas, par une issue du jeu, ou par une ligne du journal (saut, pause).

use alloc::vec::Vec;

use jeu_abi::{
    Appareil, Capacites, Duree, Instant, Manette, Oui, Pas, Presence, RegardFil, ReveilFil,
    RoleAppareil,
};

use crate::journal::ValeurTelephone;
use crate::regles::{EtatTenu, Origine};

/// L'état du cœur de la partie qui fait foi, hors de la mémoire du jeu.
///
/// Opaque : il ne se lit qu'à travers les méthodes de [`crate::Partie`]. Deux états
/// égaux donnent la même suite à la partie pour les mêmes arrivées.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EtatCoeur {
    /// Le numéro de la partie dans la soirée : 0, puis un de plus à chaque
    /// nouvelle partie.
    pub(crate) partie: u32,
    /// Le dernier pas définitif.
    pub(crate) pas: Pas,
    /// L'ordre de la prochaine entrée dans la partie.
    pub(crate) ordre: u64,
    /// Le numéro de la prochaine arrivée. Il avance à l'arrivée, et le rejeu le
    /// retrouve par les arrivées jouées.
    pub(crate) sequence: u64,
    /// Le numéro de la prochaine entrée synthétisée. Il n'avance qu'au pas joué :
    /// le rejeu le retrouve à l'identique.
    pub(crate) synthese: u64,
    /// Quand le jeu veut recevoir le temps.
    pub(crate) reveil: ReveilFil,
    /// Le temps jeté par un saut, pas encore dit par un `Temps`.
    pub(crate) saut: Duree,
    /// Une pause du commun est-elle en cours ?
    pub(crate) en_pause: Oui,
    /// Les tenus restent-ils à relâcher pour la pause qui commence ?
    pub(crate) relacher_pour_pause: Oui,
    /// Les `Temps` écartés de suite.
    pub(crate) temps_ecartes: u8,
    /// Trois `Temps` écartés de suite : plus de temps (§ 4.6).
    pub(crate) en_panne: Oui,
    /// Un `Temps` a-t-il été accepté depuis le dernier point à intervalle ? Il en
    /// fait alors le point sain d'où [`crate::Partie::repartir`] repart.
    pub(crate) periodique_confirme: Oui,
    /// Les appareils vus, dans l'ordre où le cœur les a vus la première fois.
    pub(crate) fiches: Vec<Fiche>,
    /// Le rang du prochain appareil neuf.
    pub(crate) prochain_rang: u32,
    /// Les tenus des téléphones, par appareil et par clé.
    pub(crate) tenus: Vec<TenuTelephone>,
    /// Les manettes.
    pub(crate) manettes: Vec<TenuManette>,
    /// Les demandes lancées que le jeu liste encore.
    pub(crate) demandes: Vec<DemandeEnCours>,
    /// Toutes les demandes lancées dans la soirée, par leur identité exposée
    /// ([`EtatCoeur::exposer`]), triées : une demande ne part qu'une fois par
    /// identité, même à travers une reprise (CAT-76), et la demande 1 d'une nouvelle
    /// partie n'est pas celle de la partie d'avant.
    pub(crate) lancees: Vec<u64>,
    /// Le pas du dernier point de reprise.
    pub(crate) dernier_point: Pas,
    /// Le pas de la dernière vue produite pour chaque regard : ce qu'un geste peut
    /// prétendre avoir vu (§ 4.3). Écrit au journal, pour que la reprise le retrouve.
    pub(crate) vues: Vec<(RegardFil, Pas)>,
}

/// Ce que le cœur sait d'un appareil.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Fiche {
    /// L'appareil.
    pub(crate) appareil: Appareil,
    /// Son rang interne : l'ordre où le cœur l'a vu la première fois (CAT-27).
    pub(crate) rang: u32,
    /// Son rôle et ses capacités, s'il s'est annoncé.
    pub(crate) presence: Option<(RoleAppareil, Capacites)>,
    /// Est-il présent ?
    pub(crate) present: Oui,
    /// L'instant et la place de sa dernière arrivée, pour garder son ordre.
    pub(crate) derniere: Option<(Instant, Pas)>,
    /// Le dernier pas où il a donné signe de vie (SQ-01).
    pub(crate) derniere_nouvelle: Pas,
}

/// Un tenu de téléphone, par appareil et par clé.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TenuTelephone {
    pub(crate) appareil: Appareil,
    pub(crate) rang: u32,
    pub(crate) cle: Vec<u8>,
    pub(crate) etat: EtatTenu<ValeurTelephone>,
}

/// Une manette.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TenuManette {
    pub(crate) appareil: Appareil,
    pub(crate) rang: u32,
    pub(crate) etat: EtatTenu<Manette>,
}

/// L'identité exposée de la demande `id` de la partie `partie` (voir
/// [`EtatCoeur::exposer`]).
pub(crate) fn exposer(partie: u32, id: u64) -> Option<u64> {
    (id >> 32 == 0).then_some((u64::from(partie) << 32) | id)
}

/// Une demande lancée, que le jeu liste encore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct DemandeEnCours {
    pub(crate) id: u64,
    pub(crate) repondue: Oui,
}

/// Ce qu'il faut redire au jeu quand la définitive repart d'un point où les fiches
/// étaient `avant` alors qu'elles sont `apres` (§ 4.6) : les arrivées, retours et
/// changements de capacités, puis les départs, dans l'ordre des fiches.
pub(crate) fn presences_changees(avant: &[Fiche], apres: &[Fiche]) -> Vec<Presence> {
    let mut v = Vec::new();
    for f in apres.iter().filter(|f| f.present == Oui::Oui) {
        let Some((role, capacites)) = f.presence else {
            continue;
        };
        let appareil = f.appareil;
        match avant.iter().find(|a| a.appareil == appareil) {
            None => v.push(Presence::Arrivee {
                appareil,
                role,
                capacites,
            }),
            Some(a) if a.present == Oui::Non => v.push(Presence::Retour {
                appareil,
                capacites,
            }),
            Some(a) if a.presence.map(|(_, c)| c) != Some(capacites) => {
                v.push(Presence::Capacites {
                    appareil,
                    capacites,
                });
            }
            Some(_) => {}
        }
    }
    for a in avant.iter().filter(|a| a.present == Oui::Oui) {
        if !apres
            .iter()
            .any(|f| f.appareil == a.appareil && f.present == Oui::Oui)
        {
            v.push(Presence::Depart {
                appareil: a.appareil,
                cause: jeu_abi::CauseDepart::Fermeture,
            });
        }
    }
    v
}

impl EtatCoeur {
    /// Quand le jeu a demandé à recevoir le temps : ce qui dit si une échéance qu'il
    /// déclare peut encore arriver (E5).
    pub const fn reveil(&self) -> ReveilFil {
        self.reveil
    }

    /// L'état d'une partie créée au pas `pas`.
    pub(crate) const fn neuf(pas: Pas, reveil: ReveilFil) -> EtatCoeur {
        EtatCoeur {
            partie: 0,
            pas,
            ordre: 0,
            sequence: 0,
            synthese: 0,
            reveil,
            saut: Duree::ZERO,
            en_pause: Oui::Non,
            relacher_pour_pause: Oui::Non,
            temps_ecartes: 0,
            en_panne: Oui::Non,
            periodique_confirme: Oui::Non,
            fiches: Vec::new(),
            prochain_rang: 0,
            tenus: Vec::new(),
            manettes: Vec::new(),
            demandes: Vec::new(),
            lancees: Vec::new(),
            dernier_point: pas,
            vues: Vec::new(),
        }
    }

    /// Le pas de la dernière vue produite pour ce regard ; zéro s'il n'en a jamais eu.
    pub(crate) fn derniere_vue(&self, r: RegardFil) -> Pas {
        self.vues
            .iter()
            .find(|(q, _)| *q == r)
            .map_or(Pas::ZERO, |(_, p)| *p)
    }

    /// Note la vue produite pour ce regard.
    pub(crate) fn voir(&mut self, r: RegardFil, pas: Pas) {
        match self.vues.iter_mut().find(|(q, _)| *q == r) {
            Some((_, p)) => *p = pas,
            None => self.vues.push((r, pas)),
        }
    }

    /// Un numéro neuf, pour une arrivée.
    pub(crate) fn sequence(&mut self) -> u64 {
        let s = self.sequence;
        self.sequence = s.saturating_add(1);
        s
    }

    /// L'identité d'une entrée synthétisée : le bit de poids fort la sépare des
    /// numéros d'arrivée.
    pub(crate) fn synthese(&mut self) -> u64 {
        let s = self.synthese;
        self.synthese = s.saturating_add(1);
        s | (1 << 63)
    }

    pub(crate) fn fiche(&self, a: Appareil) -> Option<&Fiche> {
        self.fiches.iter().find(|f| f.appareil == a)
    }

    pub(crate) fn fiche_mut(&mut self, a: Appareil) -> Option<&mut Fiche> {
        self.fiches.iter_mut().find(|f| f.appareil == a)
    }

    /// Le rang de cet appareil, attribué à sa première arrivée.
    pub(crate) fn rang(&mut self, a: Appareil) -> u32 {
        if let Some(f) = self.fiche(a) {
            return f.rang;
        }
        let rang = self.prochain_rang;
        self.inscrire(a, rang);
        rang
    }

    /// Inscrit l'appareil sous ce rang s'il est inconnu : au rejeu, le rang vient
    /// du journal.
    pub(crate) fn inscrire(&mut self, a: Appareil, rang: u32) {
        if self.fiche(a).is_some() {
            return;
        }
        self.prochain_rang = self.prochain_rang.max(rang.saturating_add(1));
        // Rangées par rang : l'ordre de la table ne dépend pas de celui où le rejeu
        // retrouve les appareils.
        let i = self.fiches.partition_point(|f| f.rang < rang);
        self.fiches.insert(
            i,
            Fiche {
                appareil: a,
                rang,
                presence: None,
                present: Oui::Non,
                derniere: None,
                // Le signe de vie vient de l'arrivée jouée (`suivre`), pas de l'heure
                // de l'inscription : en direct comme au rejeu, la même valeur.
                derniere_nouvelle: Pas::ZERO,
            },
        );
    }

    /// Une arrivée de cet appareil est jouée : il a donné signe de vie, et son
    /// ordre avance.
    pub(crate) fn suivre(
        &mut self,
        a: Appareil,
        origine: Origine,
        fait: Instant,
        recu: Instant,
        place: Pas,
    ) {
        if let Origine::Appareil(rang) = origine {
            self.inscrire(a, rang);
        }
        if let Some(f) = self.fiche_mut(a) {
            f.derniere_nouvelle = f.derniere_nouvelle.max(recu.pas);
            f.derniere = Some(match f.derniere {
                Some((df, dp)) => (df.max(fait), dp.max(place)),
                None => (fait, place),
            });
        }
    }

    /// Une présence est jouée.
    pub(crate) fn presence(&mut self, p: &Presence) {
        match *p {
            Presence::Arrivee {
                appareil,
                role,
                capacites,
            } => {
                if let Some(f) = self.fiche_mut(appareil) {
                    f.presence = Some((role, capacites));
                    f.present = Oui::Oui;
                }
            }
            Presence::Retour {
                appareil,
                capacites,
            }
            | Presence::Capacites {
                appareil,
                capacites,
            } => {
                if let Some(f) = self.fiche_mut(appareil) {
                    f.present = Oui::Oui;
                    if let Some((_, c)) = f.presence.as_mut() {
                        *c = capacites;
                    }
                }
            }
            Presence::Depart { appareil, .. } => {
                if let Some(f) = self.fiche_mut(appareil) {
                    f.present = Oui::Non;
                }
            }
        }
    }

    /// Les appareils présents, dans l'ordre de leurs rangs, comme `Creer` les reçoit.
    pub(crate) fn presents(&self) -> Vec<Presence> {
        let mut f: Vec<&Fiche> = self
            .fiches
            .iter()
            .filter(|f| f.present == Oui::Oui)
            .collect();
        f.sort_unstable_by_key(|f| f.rang);
        f.into_iter()
            .filter_map(|f| {
                f.presence.map(|(role, capacites)| Presence::Arrivee {
                    appareil: f.appareil,
                    role,
                    capacites,
                })
            })
            .collect()
    }

    /// Oublie un appareil : il reviendra comme un neuf (PUN-15, SQ-76).
    pub(crate) fn oublier(&mut self, a: Appareil) {
        self.fiches.retain(|f| f.appareil != a);
        self.tenus.retain(|t| t.appareil != a);
        self.manettes.retain(|m| m.appareil != a);
        self.vues
            .retain(|(r, _)| *r != RegardFil::Appareil(a.fil()));
    }

    /// Applique `f` au tenu de cet appareil et de cette clé, créé s'il manque.
    pub(crate) fn avec_tenu<R>(
        &mut self,
        a: Appareil,
        rang: u32,
        cle: &[u8],
        f: impl FnOnce(&mut EtatTenu<ValeurTelephone>) -> R,
    ) -> R {
        let trouve = self
            .tenus
            .iter()
            .position(|t| t.appareil == a && t.cle == cle);
        let t = if let Some(i) = trouve {
            self.tenus.get_mut(i)
        } else {
            self.tenus.push(TenuTelephone {
                appareil: a,
                rang,
                cle: cle.to_vec(),
                etat: EtatTenu::neuf(),
            });
            self.tenus.last_mut()
        };
        match t {
            Some(t) => f(&mut t.etat),
            // Inatteignable : l'élément vient d'être trouvé ou poussé. Sans `unwrap`,
            // la règle s'applique alors à un tenu jetable.
            None => f(&mut EtatTenu::neuf()),
        }
    }

    /// Applique `f` à la manette de cet appareil, créée si elle manque.
    pub(crate) fn avec_manette<R>(
        &mut self,
        a: Appareil,
        rang: u32,
        f: impl FnOnce(&mut EtatTenu<Manette>) -> R,
    ) -> R {
        let trouve = self.manettes.iter().position(|m| m.appareil == a);
        let m = if let Some(i) = trouve {
            self.manettes.get_mut(i)
        } else {
            self.manettes.push(TenuManette {
                appareil: a,
                rang,
                etat: EtatTenu::neuf(),
            });
            self.manettes.last_mut()
        };
        match m {
            Some(m) => f(&mut m.etat),
            None => f(&mut EtatTenu::neuf()),
        }
    }

    /// Le dernier numéro reçu d'un tenu, s'il en a un.
    pub(crate) fn numero_tenu(&self, a: Appareil, cle: &[u8]) -> Option<u64> {
        self.tenus
            .iter()
            .find(|t| t.appareil == a && t.cle == cle)
            .and_then(|t| t.etat.numero)
    }

    /// Le dernier numéro reçu d'une manette, s'il en a un.
    pub(crate) fn numero_manette(&self, a: Appareil) -> Option<u64> {
        self.manettes
            .iter()
            .find(|m| m.appareil == a)
            .and_then(|m| m.etat.numero)
    }

    /// L'identité qu'une demande du jeu porte hors du cœur : le numéro de la partie
    /// dans les 32 bits hauts, celle du jeu dans les 32 bits bas. Le compteur du jeu
    /// repart de 1 à chaque nouvelle partie, sur une instance neuve : sans le numéro
    /// de partie, une réponse tardive à la demande 1 de la partie d'avant serait
    /// livrée à la demande 1 de la nouvelle, et celle-ci ne partirait jamais
    /// (CAT-76). `None` pour une identité du jeu qui ne tient pas sur 32 bits.
    pub(crate) fn exposer(&self, id: u64) -> Option<u64> {
        exposer(self.partie, id)
    }

    /// L'identité du jeu d'une identité exposée, si elle est de la partie en cours.
    pub(crate) fn interne(&self, exposee: u64) -> Option<u64> {
        (exposee >> 32 == u64::from(self.partie)).then_some(exposee & u64::from(u32::MAX))
    }

    /// La demande est-elle lancée, encore listée, et sans réponse ?
    pub(crate) fn attend_reponse(&self, id: u64) -> bool {
        self.demandes
            .iter()
            .any(|d| d.id == id && d.repondue == Oui::Non)
    }

    /// Note qu'une demande a reçu sa réponse.
    pub(crate) fn repondre(&mut self, id: u64) {
        for d in &mut self.demandes {
            if d.id == id {
                d.repondue = Oui::Oui;
            }
        }
    }

    /// Note qu'une demande a été lancée, par son identité exposée.
    pub(crate) fn lancer(&mut self, id: u64) {
        if let Err(i) = self.lancees.binary_search(&id) {
            self.lancees.insert(i, id);
        }
    }

    /// A-t-elle déjà été lancée dans la soirée ? Par son identité exposée.
    pub(crate) fn deja_lancee(&self, id: u64) -> bool {
        self.lancees.binary_search(&id).is_ok()
    }
}
