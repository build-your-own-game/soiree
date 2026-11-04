//! Un téléphone simulé et son lien avec l'ordinateur de la soirée (§ 7.1).
//!
//! Le transport simulé livre ce que le commun livrerait (CONTRAT, « Ce que le réel
//! impose ») :
//!
//! - **un événement exactement une fois, dans l'ordre de l'appareil.** Un geste reste
//!   dans la file du téléphone jusqu'à ce que son pas soit définitif ; une coupure,
//!   un trou noir ou un redémarrage de l'autorité le font repartir, jamais deux fois
//!   (REM-13, SAL-15) ;
//! - **un état tenu ne vit que par son dernier** : il peut se perdre, arriver dans le
//!   désordre ou par rafales, le numéro départage (REM-14) ;
//! - **avec latence, gigue et rafales de Wi-Fi** : 16 à 66 ms dans chaque sens, une
//!   gigue de deux pas, et, sur un téléphone qui en souffre, des paquets groupés par
//!   100 ms ;
//! - **des présences annoncées par le commun** : départ à la fermeture d'une
//!   connexion (après 0,5 s), ou après 3 s de silence ; retour, ou arrivée d'un
//!   appareil oublié (REM-43).
//!
//! Ces durées sont des points de départ, non mesurées.

use jeu_abi::{Oui, StyleFil};

/// Les pas avant que le commun voie une connexion fermée.
pub(crate) const FERMETURE: u64 = 30;
/// Les pas de silence avant que le commun annonce un départ (SQ-01).
pub(crate) const SILENCE_MAX: u64 = 180;
/// La rafale du Wi-Fi d'un téléphone, en pas : 100 ms.
pub(crate) const RAFALE: u64 = 6;

/// L'état du lien d'un téléphone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lien {
    /// Pas encore arrivé, ou parti pour de bon.
    Hors,
    /// Tout passe.
    Ok,
    /// La connexion est fermée jusqu'à ce pas.
    Coupe { fin: u64 },
    /// Le téléphone dort jusqu'à ce pas, connexion ouverte.
    Silence { fin: u64 },
    /// Ce qu'il envoie se perd jusqu'à ce pas.
    TrouNoir { fin: u64 },
    /// Ses paquets sont retenus jusqu'à ce pas.
    Retenu { fin: u64 },
    /// L'onglet se recharge jusqu'à ce pas.
    Recharge { fin: u64 },
}

impl Lien {
    /// Le téléphone peut-il agir (son écran est allumé, sa page vit) ?
    pub(crate) const fn agit(self) -> bool {
        matches!(
            self,
            Lien::Ok | Lien::Coupe { .. } | Lien::TrouNoir { .. } | Lien::Retenu { .. }
        )
    }

    /// Ses paquets partent-ils sur le réseau ?
    pub(crate) const fn envoie(self) -> bool {
        matches!(self, Lien::Ok | Lien::Retenu { .. })
    }
}

/// Où en est un événement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EtatEvenement {
    /// À envoyer dès que le lien le permet.
    AEnvoyer,
    /// Sur le réseau.
    EnVol,
    /// Placé par le cœur à ce pas, pas encore définitif.
    Place(u64),
}

/// Un événement d'un téléphone : un geste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Evenement {
    pub(crate) id: u64,
    pub(crate) octets: Vec<u8>,
    pub(crate) fait: (u64, u16),
    pub(crate) etat: EtatEvenement,
    /// Vient-il de l'onglet jumeau ?
    pub(crate) jumeau: Oui,
}

/// Ce qui voyage sur le réseau, du téléphone vers l'ordinateur de la soirée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Paquet {
    /// Un geste, par son identité.
    Geste { id: u64 },
    /// Un état tenu.
    Tenu {
        octets: Vec<u8>,
        numero: u64,
        fait: (u64, u16),
    },
}

/// Un paquet en route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnRoute {
    pub(crate) livre_a: u64,
    pub(crate) paquet: Paquet,
}

/// Une vue en route vers le téléphone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VueEnRoute {
    pub(crate) livre_a: u64,
    pub(crate) pas: u64,
    pub(crate) octets: Vec<u8>,
}

/// Un téléphone simulé.
#[derive(Debug, Clone)]
pub(crate) struct Telephone {
    pub(crate) appareil: u64,
    pub(crate) style: StyleFil,
    /// Le pas de sa première connexion.
    pub(crate) arrive: u64,
    /// Le pas où il part pour de bon, s'il part.
    pub(crate) part: Option<u64>,
    pub(crate) lien: Lien,
    /// Le jeu le connaît-il (arrivé, pas oublié) ?
    pub(crate) connu: Oui,
    /// Le commun l'a-t-il annoncé présent ?
    pub(crate) present: Oui,
    /// Sa latence dans chaque sens, en pas.
    pub(crate) latence: u64,
    pub(crate) rafales: Oui,
    /// Son onglet jumeau joue jusqu'à ce pas.
    pub(crate) jumeau_jusqua: u64,
    pub(crate) reseau: Vec<EnRoute>,
    pub(crate) evenements: Vec<Evenement>,
    /// Le dernier état tenu, à renvoyer à chaque reconnexion.
    pub(crate) tenu: Option<(Vec<u8>, (u64, u16))>,
    pub(crate) prochain_id: u64,
    pub(crate) numero_tenu: u64,
    /// La dernière livraison d'un geste : l'ordre de l'appareil se garde.
    pub(crate) dernier_geste_livre: u64,
    /// Le pas de la vue qu'il montre.
    pub(crate) vu: u64,
    /// Ses octets.
    pub(crate) vue: Vec<u8>,
    /// Le pas où elle a changé.
    pub(crate) vue_depuis: u64,
    pub(crate) vues: Vec<VueEnRoute>,
    /// Le prochain pas où son bot joue.
    pub(crate) prochain_bot: u64,
    /// La dernière vue envoyée, pour ne renvoyer que ce qui change.
    pub(crate) envoyee: Vec<u8>,
}

impl Telephone {
    pub(crate) fn new(appareil: u64, style: StyleFil, arrive: u64, latence: u64) -> Telephone {
        Telephone {
            appareil,
            style,
            arrive,
            part: None,
            lien: Lien::Hors,
            connu: Oui::Non,
            present: Oui::Non,
            latence,
            rafales: Oui::Non,
            jumeau_jusqua: 0,
            reseau: Vec::new(),
            evenements: Vec::new(),
            tenu: None,
            prochain_id: 1,
            numero_tenu: 0,
            dernier_geste_livre: 0,
            vu: 0,
            vue: Vec::new(),
            vue_depuis: 0,
            vues: Vec::new(),
            prochain_bot: arrive,
            envoyee: Vec::new(),
        }
    }

    /// Le pas où un paquet envoyé maintenant arrive, gigue et rafales comprises.
    pub(crate) fn livraison(&self, maintenant: u64, gigue: u64) -> u64 {
        let mut a = maintenant
            .saturating_add(self.latence)
            .saturating_add(gigue);
        if self.rafales == Oui::Oui {
            let reste = a.checked_rem(RAFALE).unwrap_or(0);
            if reste != 0 {
                a = a.saturating_add(RAFALE.saturating_sub(reste));
            }
        }
        if let Lien::Retenu { fin } = self.lien {
            a = a.max(fin);
        }
        a
    }

    /// Un geste neuf : il entre dans la file, et part si le lien le permet.
    pub(crate) fn emettre(
        &mut self,
        octets: Vec<u8>,
        fait: (u64, u16),
        jumeau: Oui,
        maintenant: u64,
        gigue: u64,
    ) {
        let base = self.prochain_id;
        self.prochain_id = base.saturating_add(1);
        let id = match jumeau {
            Oui::Oui => base | (1 << 62),
            Oui::Non => base,
        };
        self.evenements.push(Evenement {
            id,
            octets,
            fait,
            etat: EtatEvenement::AEnvoyer,
            jumeau,
        });
        self.envoyer_les_gestes(maintenant, gigue);
    }

    /// Envoie les gestes qui attendent, dans leur ordre, si le lien le permet.
    pub(crate) fn envoyer_les_gestes(&mut self, maintenant: u64, gigue: u64) {
        if !self.lien.envoie() {
            return;
        }
        let mut a = self
            .livraison(maintenant, gigue)
            .max(self.dernier_geste_livre);
        let mut partis = Vec::new();
        for e in &mut self.evenements {
            if e.etat == EtatEvenement::AEnvoyer {
                e.etat = EtatEvenement::EnVol;
                partis.push(EnRoute {
                    livre_a: a,
                    paquet: Paquet::Geste { id: e.id },
                });
                a = a.max(self.dernier_geste_livre);
            }
        }
        if let Some(dernier) = partis.last() {
            self.dernier_geste_livre = dernier.livre_a;
        }
        self.reseau.extend(partis);
    }

    /// Un état tenu neuf : le dernier gagne ; il part s'il le peut, sinon il attend
    /// la reconnexion.
    pub(crate) fn tenir(&mut self, octets: Vec<u8>, fait: (u64, u16), maintenant: u64, gigue: u64) {
        self.tenu = Some((octets, fait));
        self.envoyer_le_tenu(maintenant, gigue);
    }

    /// Renvoie l'état tenu, avec un numéro neuf.
    pub(crate) fn envoyer_le_tenu(&mut self, maintenant: u64, gigue: u64) {
        if !self.lien.envoie() {
            return;
        }
        let Some((octets, fait)) = self.tenu.clone() else {
            return;
        };
        self.numero_tenu = self.numero_tenu.saturating_add(1);
        let livre_a = self.livraison(maintenant, gigue);
        self.reseau.push(EnRoute {
            livre_a,
            paquet: Paquet::Tenu {
                octets,
                numero: self.numero_tenu,
                fait,
            },
        });
    }

    /// Le réseau perd tout ce qui est en route : les gestes repartiront.
    pub(crate) fn perdre_le_reseau(&mut self) {
        self.reseau.clear();
        for e in &mut self.evenements {
            if e.etat == EtatEvenement::EnVol {
                e.etat = EtatEvenement::AEnvoyer;
            }
        }
    }

    /// Les paquets qui arrivent à ce pas, dans leur ordre d'arrivée.
    pub(crate) fn arrivent(&mut self, maintenant: u64) -> Vec<Paquet> {
        let mut arrives: Vec<EnRoute> = Vec::new();
        let mut restent = Vec::new();
        for p in std::mem::take(&mut self.reseau) {
            if p.livre_a <= maintenant {
                arrives.push(p);
            } else {
                restent.push(p);
            }
        }
        self.reseau = restent;
        arrives.sort_by_key(|p| p.livre_a);
        arrives.into_iter().map(|p| p.paquet).collect()
    }

    /// Les gestes confirmés par un pas définitif quittent la file.
    pub(crate) fn confirmer(&mut self, definitif: u64) {
        self.evenements
            .retain(|e| !matches!(e.etat, EtatEvenement::Place(p) if p <= definitif));
    }

    /// Après un redémarrage de l'autorité : ce qui n'était pas définitif repart.
    pub(crate) fn renvoyer_le_provisoire(&mut self, definitif: u64) {
        for e in &mut self.evenements {
            if matches!(e.etat, EtatEvenement::Place(p) if p > definitif) {
                e.etat = EtatEvenement::AEnvoyer;
            }
        }
    }
}
