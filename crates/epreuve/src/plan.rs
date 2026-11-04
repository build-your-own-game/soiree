//! Ce que l'épreuve joue : le plan (§ 8.8), ses pannes et ses contrôles.

use std::fmt;

use jeu_abi::{Duree, Oui};

/// Un contrôle de l'épreuve (§ 7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdControle {
    /// Constat : réécriture acceptée, aucun import, déclaration cohérente, budgets.
    E1,
    /// Parcours : les bots vont jusqu'au bout, sans faute.
    E2,
    /// Couverture : chaque phase et chaque cas atteints.
    E3,
    /// Invariants, à chaque pas définitif ; transition à chaque entrée acceptée.
    E4,
    /// Jamais figée.
    E5,
    /// Refus sans trace.
    E6,
    /// Lecture sans trace.
    E7,
    /// Secrets.
    E8,
    /// Déterminisme.
    E9,
    /// Permutation des appareils.
    E10,
    /// Coupure transparente.
    E11,
    /// Fenêtre.
    E12,
    /// Entrées absurdes.
    E13,
    /// Reprise.
    E14,
    /// Mémoire.
    E15,
    /// Pires cas.
    E16,
    /// Flottants.
    E17,
    /// Gel.
    E18,
    /// Budgets.
    E19,
}

impl IdControle {
    /// Tous les contrôles, dans l'ordre.
    pub const TOUS: [IdControle; 19] = [
        IdControle::E1,
        IdControle::E2,
        IdControle::E3,
        IdControle::E4,
        IdControle::E5,
        IdControle::E6,
        IdControle::E7,
        IdControle::E8,
        IdControle::E9,
        IdControle::E10,
        IdControle::E11,
        IdControle::E12,
        IdControle::E13,
        IdControle::E14,
        IdControle::E15,
        IdControle::E16,
        IdControle::E17,
        IdControle::E18,
        IdControle::E19,
    ];

    /// Les contrôles écrits : tous, depuis les étapes 5 et 6 du § 8.9.
    pub const ECRITS: [IdControle; 19] = IdControle::TOUS;

    /// Son nom court, « E1 » à « E19 ».
    pub const fn nom(self) -> &'static str {
        match self {
            IdControle::E1 => "E1",
            IdControle::E2 => "E2",
            IdControle::E3 => "E3",
            IdControle::E4 => "E4",
            IdControle::E5 => "E5",
            IdControle::E6 => "E6",
            IdControle::E7 => "E7",
            IdControle::E8 => "E8",
            IdControle::E9 => "E9",
            IdControle::E10 => "E10",
            IdControle::E11 => "E11",
            IdControle::E12 => "E12",
            IdControle::E13 => "E13",
            IdControle::E14 => "E14",
            IdControle::E15 => "E15",
            IdControle::E16 => "E16",
            IdControle::E17 => "E17",
            IdControle::E18 => "E18",
            IdControle::E19 => "E19",
        }
    }

    /// Son titre, celui du tableau du § 7.2.
    pub const fn titre(self) -> &'static str {
        match self {
            IdControle::E1 => "Constat",
            IdControle::E2 => "Parcours",
            IdControle::E3 => "Couverture",
            IdControle::E4 => "Invariants",
            IdControle::E5 => "Jamais figée",
            IdControle::E6 => "Refus sans trace",
            IdControle::E7 => "Lecture sans trace",
            IdControle::E8 => "Secrets",
            IdControle::E9 => "Déterminisme",
            IdControle::E10 => "Permutation",
            IdControle::E11 => "Coupure transparente",
            IdControle::E12 => "Fenêtre",
            IdControle::E13 => "Entrées absurdes",
            IdControle::E14 => "Reprise",
            IdControle::E15 => "Mémoire",
            IdControle::E16 => "Pires cas",
            IdControle::E17 => "Flottants",
            IdControle::E18 => "Gel",
            IdControle::E19 => "Budgets",
        }
    }

    /// Le contrôle de ce nom, « E1 » à « E19 ».
    pub fn depuis_nom(nom: &str) -> Option<IdControle> {
        IdControle::TOUS
            .into_iter()
            .find(|c| c.nom().eq_ignore_ascii_case(nom.trim()))
    }

    /// Est-il écrit dans cette version de l'épreuve ?
    pub fn ecrit(self) -> bool {
        IdControle::ECRITS.contains(&self)
    }
}

impl fmt::Display for IdControle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.nom())
    }
}

/// Une panne que la soirée simulée fait subir (§ 7.1 ; CONTRAT, « Ce que le réel
/// impose » ; SAL-180, OUT-77, OUT-79). Chacune est choisie dans le plan, programmée à
/// un pas tiré, puis constatée : une panne qui n'a pas eu lieu ne compte pas
/// (OUT-26).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panne {
    /// La connexion d'un téléphone se ferme : le commun annonce son départ, puis son
    /// retour ; ses événements non confirmés repartent (SAL-15, REM-10).
    Coupure {
        /// La durée de la coupure.
        duree: Duree,
    },
    /// Un téléphone se tait sans fermer sa connexion : le commun ne le sait qu'au
    /// bout de son délai de silence (SQ-01, REM-11).
    Silence {
        /// La durée du silence.
        duree: Duree,
    },
    /// Tout ce qu'un téléphone envoie se perd sans bruit ; ses événements repartent
    /// ensuite, ses états tenus non.
    TrouNoir {
        /// La durée du trou noir.
        duree: Duree,
    },
    /// Le réseau retient les paquets d'un téléphone, puis les livre d'un coup.
    Retenu {
        /// La durée de la rétention.
        duree: Duree,
    },
    /// Le Wi-Fi d'un téléphone livre par rafales de 100 ms, toute la soirée (CONTRAT,
    /// « Le Wi-Fi d'un téléphone arrive par rafales »).
    Rafales,
    /// Un onglet se recharge : départ, ce qui n'était pas parti se perd, retour.
    Rechargement,
    /// Un second onglet du même téléphone joue en même temps que le premier (SAL-15,
    /// SAL-154, SQ-88).
    Jumeau,
    /// L'autorité redémarre depuis son dernier point et son journal (§ 4.5).
    RedemarrageAutorite,
    /// L'ordinateur de la soirée se fige : plus de pas, puis le rattrapage et le saut
    /// (§ 4.1, CAT-15, SQ-62).
    Gel {
        /// La durée du gel.
        duree: Duree,
    },
}

impl Panne {
    /// Son nom, pour le rapport.
    pub fn nom(self) -> String {
        match self {
            Panne::Coupure { duree } => format!("coupure de {} ms", duree.en_ms()),
            Panne::Silence { duree } => format!("silence de {} ms", duree.en_ms()),
            Panne::TrouNoir { duree } => format!("trou noir de {} ms", duree.en_ms()),
            Panne::Retenu { duree } => format!("paquets retenus {} ms", duree.en_ms()),
            Panne::Rafales => String::from("rafales de Wi-Fi"),
            Panne::Rechargement => String::from("rechargement d'un onglet"),
            Panne::Jumeau => String::from("onglet jumeau"),
            Panne::RedemarrageAutorite => String::from("redémarrage de l'autorité"),
            Panne::Gel { duree } => format!("gel de {} ms", duree.en_ms()),
        }
    }

    /// Est-ce une panne d'un téléphone ? Un jeu sans téléphone (`telephones:
    /// SansObjet`) ne peut pas la vivre : elle y est sans objet, et l'épreuve ne la
    /// programme pas (besoin 2 des jeux, `conception/BESOINS-DES-JEUX.md`).
    pub const fn du_telephone(self) -> Oui {
        match self {
            Panne::Coupure { .. }
            | Panne::Silence { .. }
            | Panne::TrouNoir { .. }
            | Panne::Retenu { .. }
            | Panne::Rafales
            | Panne::Rechargement
            | Panne::Jumeau => Oui::Oui,
            Panne::RedemarrageAutorite | Panne::Gel { .. } => Oui::Non,
        }
    }

    /// La durée pendant laquelle la panne dure, quand elle en a une.
    pub const fn duree(self) -> Option<Duree> {
        match self {
            Panne::Coupure { duree }
            | Panne::Silence { duree }
            | Panne::TrouNoir { duree }
            | Panne::Retenu { duree }
            | Panne::Gel { duree } => Some(duree),
            Panne::Rafales | Panne::Rechargement | Panne::Jumeau | Panne::RedemarrageAutorite => {
                None
            }
        }
    }
}

/// Ce que l'épreuve joue (§ 8.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Les soirées principales, chacune avec ses pannes.
    pub soirees: u32,
    /// La durée de chaque soirée principale.
    pub duree: Duree,
    /// La graine du plan : celle de chaque soirée s'en déduit. La même graine rejoue
    /// les mêmes soirées, pas pour pas.
    pub graine: [u8; 32],
    /// Les pannes de chaque soirée principale.
    pub pannes: Vec<Panne>,
    /// Les contrôles demandés. Un périmètre réduit reste visible dans le rapport :
    /// chaque contrôle y figure, et ceux qui ne sont pas demandés sont « pas exécuté »
    /// (OUT-10).
    pub controles: Vec<IdControle>,
    /// La durée de la soirée longue de E15 : six heures au § 7.2.
    pub memoire: Duree,
}

impl Plan {
    /// Les pannes de chaque soirée principale, par défaut : toutes celles du § 7.1,
    /// mêlées (OUT-77). Des points de départ, non mesurés.
    pub fn pannes_standard() -> Vec<Panne> {
        vec![
            Panne::Coupure {
                duree: Duree::ms(1500),
            },
            Panne::Coupure {
                duree: Duree::s(20),
            },
            Panne::Silence { duree: Duree::s(6) },
            Panne::TrouNoir { duree: Duree::s(2) },
            Panne::Retenu {
                duree: Duree::ms(800),
            },
            Panne::Rafales,
            Panne::Rechargement,
            Panne::Jumeau,
            Panne::RedemarrageAutorite,
            Panne::Gel { duree: Duree::s(3) },
            Panne::Gel {
                duree: Duree::s(40),
            },
        ]
    }

    /// Le plan de l'épreuve, pour `soiree-epreuve` : deux soirées de trois minutes,
    /// toutes les pannes, tous les contrôles, six heures pour E15.
    pub fn standard(graine: [u8; 32]) -> Plan {
        Plan {
            soirees: 2,
            duree: Duree::s(180),
            graine,
            pannes: Plan::pannes_standard(),
            controles: IdControle::TOUS.to_vec(),
            memoire: Duree::s(6 * 3600),
        }
    }

    /// Un plan court, pour les tests : une soirée de deux minutes, toutes les pannes,
    /// tous les contrôles, et une soirée longue d'une heure. Il ne remplace pas le
    /// plan standard : E15 y regarde une pente sur une heure, pas sur six.
    pub fn court(graine: [u8; 32]) -> Plan {
        Plan {
            soirees: 1,
            duree: Duree::s(120),
            graine,
            pannes: Plan::pannes_standard(),
            controles: IdControle::TOUS.to_vec(),
            memoire: Duree::s(3600),
        }
    }

    /// Ce contrôle est-il demandé ?
    pub fn demande(&self, c: IdControle) -> bool {
        self.controles.contains(&c)
    }
}
